//! Wire protocol parsing, serialization, and header resolution for TikTok Creator Studio.

use super::types::{
    TikTokCredentials, TikTokDownloadInfo, TikTokItemListQuery, TikTokItemListResponse,
    TikTokPublishRequest, TikTokPublishResponse, TikTokUploadAuth, TikTokUserProfile,
    TikTokVideoItem, TikTokVideoStats,
};
use crate::config::Config;
use crate::http::header_value;
use serde_json::Value;

/// Resolves TikTok credentials from the incoming HTTP request headers, falling back
/// to the gateway configuration values.
pub fn resolve_tiktok_credentials(
    headers: &[(String, String)],
    config: &Config,
) -> TikTokCredentials {
    let mut creds = TikTokCredentials::default();

    if !config.tiktok_base_url.is_empty() {
        creds.base_url = config.tiktok_base_url.clone();
    }

    // Cookie precedence: Inbound request Cookie header > Gateway config
    if let Some(cookie_hdr) = header_value(headers, "cookie") {
        creds.cookie = cookie_hdr.to_string();
    } else if !config.tiktok_cookie.is_empty() {
        creds.cookie = config.tiktok_cookie.clone();
    }

    // CSRF token precedence: tt-csrf-token > x-tiktok-csrf-token > Gateway config
    if let Some(csrf) = header_value(headers, "tt-csrf-token") {
        creds.csrf_token = csrf.to_string();
    } else if let Some(csrf) = header_value(headers, "x-tiktok-csrf-token") {
        creds.csrf_token = csrf.to_string();
    } else if !config.tiktok_csrf_token.is_empty() {
        creds.csrf_token = config.tiktok_csrf_token.clone();
    }

    // Optional user-agent passthrough
    if let Some(ua) = header_value(headers, "user-agent") {
        creds.user_agent = ua.to_string();
    }

    creds
}

/// Extracts the optimal watermark-free playback/download link from candidate play addresses.
/// Prefers high-speed CDN URLs (e.g. `webapp-sg.tiktok.com`, `tos-alisg...`) over redirected aweme URLs.
pub fn extract_best_download_url(play_addrs: &[String]) -> Option<String> {
    if play_addrs.is_empty() {
        return None;
    }

    // Priority 1: Direct CDN playback URLs (typically tos-alisg or webapp-sg)
    for addr in play_addrs {
        let lower = addr.to_ascii_lowercase();
        if (lower.contains("webapp-sg.tiktok.com") || lower.contains("tos-alisg"))
            && lower.starts_with("http")
        {
            return Some(addr.clone());
        }
    }

    // Priority 2: Standard aweme playback endpoint
    for addr in play_addrs {
        if addr.to_ascii_lowercase().contains("/aweme/v1/play") {
            return Some(addr.clone());
        }
    }

    // Priority 3: First valid HTTPS URL
    play_addrs
        .iter()
        .find(|a| a.starts_with("https://") || a.starts_with("http://"))
        .cloned()
}

/// Builds the POST request body for the `/tiktok/creator/manage/item_list/v1/` endpoint.
pub fn build_item_list_request_body(query: &TikTokItemListQuery) -> String {
    serde_json::json!({
        "cursor": query.cursor,
        "size": query.size,
        "query": {
            "conditions": [],
            "sort_orders": [
                {
                    "field_name": "post_time",
                    "order": 2
                }
            ],
            "is_recent_posts": false
        }
    })
    .to_string()
}

/// Parses the response from `/tiktok/creator/manage/item_list/v1/`.
pub fn parse_item_list_response(raw_json: &str) -> Result<TikTokItemListResponse, String> {
    let parsed: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let status_code = parsed
        .get("status_code")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;

    let cursor = parsed
        .get("cursor")
        .and_then(Value::as_u64)
        .or_else(|| {
            parsed
                .get("cursor")
                .and_then(Value::as_str)
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);

    let has_more = parsed
        .get("has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let items_array = parsed
        .get("item_list")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut items = Vec::new();
    for item_val in items_array {
        let item_id = item_val
            .get("item_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let desc = item_val
            .get("desc")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let duration = item_val
            .get("duration")
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let create_time = parse_u64_field(&item_val, "create_time");
        let post_time = parse_u64_field(&item_val, "post_time");

        let status = item_val.get("status").and_then(Value::as_i64).unwrap_or(0) as i32;
        let visibility = item_val
            .get("visibility")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;

        let cover_urls = item_val
            .get("cover_url")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let play_addrs: Vec<String> = item_val
            .get("play_addr")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let download_obj = item_val.get("download_info");
        let allow_download = download_obj
            .and_then(|d| d.get("allow_download"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let disable_reason = download_obj
            .and_then(|d| d.get("disable_reason"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let direct_download_url = extract_best_download_url(&play_addrs);

        let stats = TikTokVideoStats {
            play_count: parse_u64_field(&item_val, "play_count"),
            like_count: parse_u64_field(&item_val, "like_count"),
            comment_count: parse_u64_field(&item_val, "comment_count"),
            share_count: parse_u64_field(&item_val, "share_count"),
            favorite_count: parse_u64_field(&item_val, "favorite_count"),
        };

        let title = desc
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(64)
            .collect::<String>();

        items.push(TikTokVideoItem {
            item_id,
            title,
            desc,
            duration,
            create_time,
            post_time,
            status,
            visibility,
            cover_urls,
            download_info: TikTokDownloadInfo {
                allow_download,
                disable_reason,
                direct_download_url,
                play_addrs,
            },
            stats,
        });
    }

    Ok(TikTokItemListResponse {
        cursor,
        has_more,
        status_code,
        items,
    })
}

/// Parses user info from `/tiktokstudio/api/web/user` or `/api/v1/user/profile/upload/`.
pub fn parse_user_response(raw_json: &str) -> Result<TikTokUserProfile, String> {
    let parsed: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    // Try /tiktokstudio/api/web/user format first
    if let Some(user_id) = parsed.get("userId").and_then(Value::as_str) {
        let nickname = parsed
            .get("userBaseInfo")
            .and_then(|b| b.get("nickname"))
            .and_then(Value::as_str)
            .unwrap_or("Creator")
            .to_string();

        let avatar = parsed
            .get("userBaseInfo")
            .and_then(|b| b.get("avatarUrl"))
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let status_code = parsed
            .get("statusCode")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;

        return Ok(TikTokUserProfile {
            user_id: user_id.to_string(),
            nickname,
            avatar_thumb: avatar,
            sec_uid: None,
            follower_count: 0,
            following_count: 0,
            status_code,
        });
    }

    // Try /api/v1/user/profile/upload/ format
    if let Some(user) = parsed.get("user") {
        let nickname = user
            .get("nickname")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let sec_uid = user
            .get("sec_uid")
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let avatar = user
            .get("avatar_thumb")
            .and_then(|a| a.get("url_list"))
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let status_code = parsed
            .get("status_code")
            .and_then(Value::as_i64)
            .unwrap_or(0) as i32;

        return Ok(TikTokUserProfile {
            user_id: user
                .get("uid")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            nickname,
            avatar_thumb: avatar,
            sec_uid,
            follower_count: 0,
            following_count: 0,
            status_code,
        });
    }

    Err("Unrecognized TikTok user response payload structure".to_string())
}

/// Parses upload auth response from `/api/v1/video/upload/auth/`.
pub fn parse_upload_auth_response(raw_json: &str) -> Result<TikTokUploadAuth, String> {
    let parsed: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let status_code = parsed
        .get("status_code")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;

    let video_token_v5 = parsed
        .get("video_token_v5")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let audio_token_v5 = parsed
        .get("audio_token_v5")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let store_region = parsed
        .get("store_region")
        .and_then(Value::as_str)
        .unwrap_or("alisg")
        .to_string();

    Ok(TikTokUploadAuth {
        video_token_v5,
        audio_token_v5,
        store_region,
        status_code,
    })
}

/// Builds JSON payload for `/tiktok/web/project/post/v1/`.
pub fn build_publish_request_body(req: &TikTokPublishRequest) -> String {
    let creation_id = uuid::Uuid::new_v4().to_string();
    let text = if req.text.is_empty() {
        &req.title
    } else {
        &req.text
    };

    serde_json::json!({
        "post_common_info": {
            "creation_id": creation_id,
            "enter_post_page_from": "creator_center",
            "post_type": 1
        },
        "single_post_req_list": [
            {
                "batch_index": 0,
                "video_id": req.video_id,
                "is_long_video": 0,
                "single_post_feature_info": {
                    "text": text,
                    "markup_text": text,
                    "mature_theme_type": 0
                }
            }
        ]
    })
    .to_string()
}

/// Parses response from `/tiktok/web/project/post/v1/`.
pub fn parse_publish_response(raw_json: &str) -> Result<TikTokPublishResponse, String> {
    let parsed: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Failed to parse JSON: {e}"))?;

    let status_code = parsed
        .get("status_code")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;

    let project_id = parsed
        .get("project_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let project_status = parsed
        .get("project_status")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;

    Ok(TikTokPublishResponse {
        project_id,
        project_status,
        status_code,
    })
}

fn parse_u64_field(val: &Value, key: &str) -> u64 {
    val.get(key)
        .and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_str()
                    .map(str::trim)
                    .and_then(|s| s.parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}
