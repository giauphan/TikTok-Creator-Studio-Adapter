//! Unit tests for TikTok Creator Studio domain invariants, application use cases
//! (via in-memory port fakes), and protocol serialization.

use super::ports::TikTokClientPort;
use super::protocol::{
    build_item_list_request_body, build_publish_request_body, extract_best_download_url,
    parse_item_list_response, parse_publish_response, parse_upload_auth_response,
    parse_user_response, resolve_tiktok_credentials,
};
use super::service::TikTokStudioService;
use super::types::{
    is_valid_tiktok_identifier, TikTokCredentials, TikTokDownloadInfo, TikTokError,
    TikTokItemListQuery, TikTokItemListResponse, TikTokPublishRequest, TikTokPublishResponse,
    TikTokStatusResponse, TikTokUploadAuth, TikTokUserProfile, TikTokVideoItem, TikTokVideoStats,
};
use crate::config::Config;

/// In-memory fake implementation of [`TikTokClientPort`] for deterministic unit testing
/// of application use cases without network or socket I/O.
struct FakeTikTokClient {
    studio_user_ok: bool,
    items: Vec<TikTokVideoItem>,
}

impl TikTokClientPort for FakeTikTokClient {
    fn fetch_studio_user(
        &self,
        _creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError> {
        if self.studio_user_ok {
            Ok(TikTokUserProfile {
                user_id: "studio_u1".to_string(),
                nickname: "Studio Primary".to_string(),
                status_code: 0,
                ..Default::default()
            })
        } else {
            Err(TikTokError::Upstream(
                502,
                "studio endpoint unavailable".to_string(),
            ))
        }
    }

    fn fetch_upload_profile(
        &self,
        _creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError> {
        Ok(TikTokUserProfile {
            user_id: "fallback_u2".to_string(),
            nickname: "Fallback Creator".to_string(),
            status_code: 0,
            ..Default::default()
        })
    }

    fn fetch_item_list(
        &self,
        _creds: &TikTokCredentials,
        query: &TikTokItemListQuery,
    ) -> Result<TikTokItemListResponse, TikTokError> {
        Ok(TikTokItemListResponse {
            cursor: query.cursor,
            has_more: false,
            status_code: 0,
            items: self.items.clone(),
        })
    }

    fn fetch_upload_auth(
        &self,
        _creds: &TikTokCredentials,
    ) -> Result<TikTokUploadAuth, TikTokError> {
        Ok(TikTokUploadAuth {
            video_token_v5: "fake_v5".to_string(),
            audio_token_v5: "fake_a5".to_string(),
            store_region: "alisg".to_string(),
            status_code: 0,
        })
    }

    fn send_publish_post(
        &self,
        _creds: &TikTokCredentials,
        post_req: &TikTokPublishRequest,
    ) -> Result<TikTokPublishResponse, TikTokError> {
        Ok(TikTokPublishResponse {
            project_id: format!("proj_{}", post_req.video_id),
            project_status: 1,
            status_code: 0,
        })
    }
}

#[test]
fn test_use_case_user_profile_fallback_via_port_fake() {
    let creds = TikTokCredentials::default();
    let svc_primary = TikTokStudioService::with_client(FakeTikTokClient {
        studio_user_ok: true,
        items: vec![],
    });
    assert_eq!(
        svc_primary.fetch_user_profile(&creds).unwrap().user_id,
        "studio_u1"
    );

    let svc_fallback = TikTokStudioService::with_client(FakeTikTokClient {
        studio_user_ok: false,
        items: vec![],
    });
    assert_eq!(
        svc_fallback.fetch_user_profile(&creds).unwrap().user_id,
        "fallback_u2"
    );
}

#[test]
fn test_use_case_download_eligibility_and_boundary_validation() {
    let creds = TikTokCredentials::default();
    let items = vec![
        TikTokVideoItem {
            item_id: "item_allowed_1".to_string(),
            download_info: TikTokDownloadInfo {
                allow_download: true,
                disable_reason: String::new(),
                direct_download_url: Some("https://webapp-sg.tiktok.com/clean.mp4".to_string()),
                play_addrs: vec!["https://webapp-sg.tiktok.com/clean.mp4".to_string()],
            },
            stats: TikTokVideoStats::default(),
            ..Default::default()
        },
        TikTokVideoItem {
            item_id: "item_disabled_2".to_string(),
            download_info: TikTokDownloadInfo {
                allow_download: false,
                disable_reason: "Creator restricted downloads".to_string(),
                direct_download_url: None,
                play_addrs: vec![],
            },
            stats: TikTokVideoStats::default(),
            ..Default::default()
        },
    ];

    let svc = TikTokStudioService::with_client(FakeTikTokClient {
        studio_user_ok: true,
        items,
    });

    // 1. Valid allowed item
    let dl = svc
        .get_video_download(&creds, "item_allowed_1")
        .expect("allowed download");
    assert_eq!(
        dl.direct_download_url.as_deref(),
        Some("https://webapp-sg.tiktok.com/clean.mp4")
    );

    // 2. Disabled item without direct URL returns 403 Forbidden
    let err_disabled = svc
        .get_video_download(&creds, "item_disabled_2")
        .expect_err("disabled item rejected");
    assert_eq!(err_disabled.0, 403);

    // 3. Unknown item returns 404 Not Found
    let err_missing = svc
        .get_video_download(&creds, "item_unknown_999")
        .expect_err("missing item rejected");
    assert_eq!(err_missing.0, 404);

    // 4. Malformed / path-traversal item_id fails closed with 400 Bad Request
    let err_bad_id = svc
        .get_video_download(&creds, "../etc/passwd")
        .expect_err("malformed id rejected");
    assert_eq!(err_bad_id.0, 400);
}

#[test]
fn test_domain_boundary_validation_rules() {
    assert!(is_valid_tiktok_identifier("7488016605237675526"));
    assert!(is_valid_tiktok_identifier("v09044_test-1"));
    assert!(!is_valid_tiktok_identifier(""));
    assert!(!is_valid_tiktok_identifier("bad/id"));
    assert!(!is_valid_tiktok_identifier("id with spaces"));

    let q_ok = TikTokItemListQuery {
        cursor: 0,
        size: 20,
    };
    let q_zero = TikTokItemListQuery { cursor: 0, size: 0 };
    let q_big = TikTokItemListQuery {
        cursor: 0,
        size: 101,
    };
    assert!(q_ok.validate().is_ok());
    assert!(q_zero.validate().is_err());
    assert!(q_big.validate().is_err());

    // Publish validation
    let valid_pub = TikTokPublishRequest {
        video_id: "vid_100".to_string(),
        title: "Title".to_string(),
        text: "Text".to_string(),
        visibility_type: 0,
    };
    assert!(valid_pub.validate().is_ok());

    let bad_vis = TikTokPublishRequest {
        visibility_type: 99,
        ..valid_pub
    };
    assert_eq!(bad_vis.validate().unwrap_err().status_code(), 400);
}

#[test]
fn test_extract_best_download_url_prefers_cdn_over_aweme() {
    let play_addrs = vec![
        "https://www.tiktok.com/aweme/v1/play/?video_id=v09044g40000ctest".to_string(),
        "https://webapp-sg.tiktok.com/e5ac1860c935/tos-alisg-pve-0037/video_stream_hd".to_string(),
    ];

    let best = extract_best_download_url(&play_addrs);
    assert_eq!(
        best,
        Some(
            "https://webapp-sg.tiktok.com/e5ac1860c935/tos-alisg-pve-0037/video_stream_hd"
                .to_string()
        )
    );
}

#[test]
fn test_extract_best_download_url_falls_back_to_aweme_play() {
    let play_addrs =
        vec!["https://www.tiktok.com/aweme/v1/play/?video_id=v09044g40000ctest".to_string()];

    let best = extract_best_download_url(&play_addrs);
    assert_eq!(
        best,
        Some("https://www.tiktok.com/aweme/v1/play/?video_id=v09044g40000ctest".to_string())
    );
}

#[test]
fn test_extract_best_download_url_empty_list() {
    let empty: Vec<String> = vec![];
    assert_eq!(extract_best_download_url(&empty), None);
}

#[test]
fn test_parse_item_list_response_from_har_fixture() {
    let fixture = r#"{
        "cursor": 0,
        "has_more": false,
        "status_code": 0,
        "item_list": [
            {
                "item_id": "7488016605237675526",
                "desc": "Axolotl swimming underwater #nature #axolotl",
                "duration": 15,
                "create_time": "1730001122",
                "post_time": "1730001200",
                "status": 1,
                "visibility": 0,
                "cover_url": [
                    "https://p16-sign-va.tiktokcdn.com/tos-maliva-p-0068/cover1.jpeg",
                    "https://p16-sign-va.tiktokcdn.com/tos-maliva-p-0068/cover2.jpeg"
                ],
                "play_addr": [
                    "https://webapp-sg.tiktok.com/cache/tos-alisg-pve-0037/video_hd.mp4",
                    "https://www.tiktok.com/aweme/v1/play/?video_id=v09044"
                ],
                "download_info": {
                    "allow_download": true,
                    "disable_reason": ""
                },
                "play_count": "15420",
                "like_count": "1250",
                "comment_count": "84",
                "share_count": "42",
                "favorite_count": "110"
            }
        ]
    }"#;

    let res = parse_item_list_response(fixture).expect("item list parsed");
    assert_eq!(res.status_code, 0);
    assert_eq!(res.cursor, 0);
    assert!(!res.has_more);
    assert_eq!(res.items.len(), 1);

    let item = &res.items[0];
    assert_eq!(item.item_id, "7488016605237675526");
    assert_eq!(item.title, "Axolotl swimming underwater #nature #axolotl");
    assert_eq!(item.duration, 15);
    assert_eq!(item.create_time, 1730001122);
    assert_eq!(item.post_time, 1730001200);
    assert!(item.download_info.allow_download);
    assert_eq!(
        item.download_info.direct_download_url,
        Some("https://webapp-sg.tiktok.com/cache/tos-alisg-pve-0037/video_hd.mp4".to_string())
    );
    assert_eq!(item.stats.play_count, 15420);
    assert_eq!(item.stats.like_count, 1250);
    assert_eq!(item.stats.comment_count, 84);
    assert_eq!(item.cover_urls.len(), 2);
}

#[test]
fn test_parse_user_response_web_user_shape() {
    let fixture = r#"{
        "userId": "7123456789012345678",
        "tt-csrf-token": "test-csrf-token-abc123",
        "statusCode": 0,
        "userBaseInfo": {
            "nickname": "Nature Creator",
            "avatarUrl": "https://p16-sign-va.tiktokcdn.com/avatar.jpeg"
        }
    }"#;

    let user = parse_user_response(fixture).expect("user parsed");
    assert_eq!(user.user_id, "7123456789012345678");
    assert_eq!(user.nickname, "Nature Creator");
    assert_eq!(
        user.avatar_thumb,
        Some("https://p16-sign-va.tiktokcdn.com/avatar.jpeg".to_string())
    );
    assert_eq!(user.status_code, 0);
}

#[test]
fn test_parse_user_response_upload_profile_shape() {
    let fixture = r#"{
        "status_code": 0,
        "user": {
            "uid": "7890123456789012345",
            "nickname": "Video Maker",
            "sec_uid": "MS4wLjABAAAA...",
            "avatar_thumb": {
                "url_list": ["https://p16-sign-va.tiktokcdn.com/thumb.jpeg"]
            }
        }
    }"#;

    let user = parse_user_response(fixture).expect("upload user parsed");
    assert_eq!(user.user_id, "7890123456789012345");
    assert_eq!(user.nickname, "Video Maker");
    assert_eq!(
        user.avatar_thumb,
        Some("https://p16-sign-va.tiktokcdn.com/thumb.jpeg".to_string())
    );
    assert_eq!(user.status_code, 0);
}

#[test]
fn test_parse_upload_auth_response() {
    let fixture = r#"{
        "status_code": 0,
        "video_token_v5": "token_video_v5_xyz",
        "audio_token_v5": "token_audio_v5_abc",
        "store_region": "alisg"
    }"#;

    let auth = parse_upload_auth_response(fixture).expect("upload auth parsed");
    assert_eq!(auth.status_code, 0);
    assert_eq!(auth.video_token_v5, "token_video_v5_xyz");
    assert_eq!(auth.audio_token_v5, "token_audio_v5_abc");
    assert_eq!(auth.store_region, "alisg");
}

#[test]
fn test_parse_publish_response() {
    let fixture = r#"{
        "status_code": 0,
        "project_id": "project_987654321",
        "project_status": 1
    }"#;

    let pub_res = parse_publish_response(fixture).expect("publish parsed");
    assert_eq!(pub_res.status_code, 0);
    assert_eq!(pub_res.project_id, "project_987654321");
    assert_eq!(pub_res.project_status, 1);
}

#[test]
fn test_resolve_tiktok_credentials_precedence() {
    let config = Config {
        tiktok_cookie: "cfg-cookie".to_string(),
        tiktok_csrf_token: "cfg-csrf".to_string(),
        ..Config::default()
    };

    let empty_headers: Vec<(String, String)> = vec![];
    let creds1 = resolve_tiktok_credentials(&empty_headers, &config);
    assert_eq!(creds1.cookie, "cfg-cookie");
    assert_eq!(creds1.csrf_token, "cfg-csrf");

    let req_headers = vec![
        ("cookie".to_string(), "req-cookie".to_string()),
        ("tt-csrf-token".to_string(), "req-csrf".to_string()),
        ("user-agent".to_string(), "CustomUA/1.0".to_string()),
    ];
    let creds2 = resolve_tiktok_credentials(&req_headers, &config);
    assert_eq!(creds2.cookie, "req-cookie");
    assert_eq!(creds2.csrf_token, "req-csrf");
    assert_eq!(creds2.user_agent, "CustomUA/1.0");
}

#[test]
fn test_build_item_list_request_body() {
    let query = TikTokItemListQuery {
        cursor: 10,
        size: 30,
    };
    let json_str = build_item_list_request_body(&query);
    assert!(json_str.contains("\"cursor\":10"));
    assert!(json_str.contains("\"size\":30"));
    assert!(json_str.contains("post_time"));
}

#[test]
fn test_build_publish_request_body() {
    let req = TikTokPublishRequest {
        video_id: "vid_12345".to_string(),
        title: "Test Video".to_string(),
        text: "Test Video #nature".to_string(),
        visibility_type: 0,
    };
    let json_str = build_publish_request_body(&req);
    assert!(json_str.contains("\"video_id\":\"vid_12345\""));
    assert!(json_str.contains("\"text\":\"Test Video #nature\""));
    assert!(json_str.contains("single_post_req_list"));
}

#[test]
fn test_status_response_serialization() {
    let status = TikTokStatusResponse {
        enabled: true,
        base_url: "https://www.tiktok.com".to_string(),
        has_cookie: true,
        has_csrf_token: true,
        provider_count: 3,
    };
    let serialized = serde_json::to_string(&status).expect("serializes");
    assert!(serialized.contains("\"enabled\":true"));
    assert!(serialized.contains("\"provider_count\":3"));
}
