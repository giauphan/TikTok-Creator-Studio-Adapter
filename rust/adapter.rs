//! Inbound HTTP route adapter for dispatching requests to TikTok Creator Studio service.

use super::config::{
    handle_apply_tiktok_har, handle_get_tiktok_config, handle_update_tiktok_config,
};
use super::handlers::{write_json_response, write_redirect_response, write_tiktok_error};
use super::protocol::resolve_tiktok_credentials;
use super::service::TikTokStudioService;
use super::types::{TikTokItemListQuery, TikTokPublishRequest, TikTokStatusResponse};
use crate::config::{Config, ConfigStore};
use crate::http::Request;
use std::io;
use std::net::TcpStream;

fn percent_decode(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn extract_query_param(query_str: &str, key: &str) -> Option<String> {
    for pair in query_str.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(percent_decode(v));
            }
        }
    }
    None
}

/// Dispatches an inbound HTTP request destined for the TikTok module.
pub fn dispatch_builtin_tiktok(
    client: &mut TcpStream,
    request: &Request,
    store: &ConfigStore,
    config: &Config,
    clean_path: &str,
) -> io::Result<Option<u16>> {
    // Check if path belongs to tiktok namespace
    let subpath = if clean_path == "/tiktok" || clean_path == "/tiktok-studio" {
        ""
    } else if let Some(stripped) = clean_path.strip_prefix("/tiktok/") {
        stripped
    } else if let Some(stripped) = clean_path.strip_prefix("/tiktok-studio/") {
        stripped
    } else {
        return Ok(None);
    };

    let subpath_clean = subpath.trim_matches('/');
    let query_str = request.path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let creds = resolve_tiktok_credentials(&request.headers, config);
    let service = TikTokStudioService::default();

    // 0. Browser Configuration UI Page: /tiktok, /tiktok/settings, /tiktok/dashboard, /tiktok/config-page
    let accepts_html = crate::http::header_value(&request.headers, "accept")
        .is_some_and(|a| a.to_ascii_lowercase().contains("text/html"));
    if (subpath_clean.is_empty()
        || subpath_clean == "settings"
        || subpath_clean == "dashboard"
        || subpath_clean == "config-page"
        || subpath_clean == "v1/config-page"
        || (subpath_clean == "config" && accepts_html))
        && request.method.eq_ignore_ascii_case("get")
    {
        crate::video_template::handle_tiktok_config_page(client)?;
        return Ok(Some(200));
    }

    // 1. Configuration routes: /tiktok/v1/config, /tiktok/config, /tiktok/v1/config/har
    if subpath_clean == "config" || subpath_clean == "v1/config" {
        if request.method.eq_ignore_ascii_case("get") {
            let code = handle_get_tiktok_config(client, store)?;
            return Ok(Some(code));
        } else if request.method.eq_ignore_ascii_case("post") {
            let code = handle_update_tiktok_config(client, &request.body, store)?;
            return Ok(Some(code));
        } else {
            let code = write_tiktok_error(
                client,
                405,
                "Method Not Allowed: use GET or POST for config",
            )?;
            return Ok(Some(code));
        }
    }

    if subpath_clean == "config/har" || subpath_clean == "v1/config/har" {
        if !request.method.eq_ignore_ascii_case("post") {
            let code =
                write_tiktok_error(client, 405, "Method Not Allowed: use POST for HAR config")?;
            return Ok(Some(code));
        }
        let code = handle_apply_tiktok_har(client, &request.body, store)?;
        return Ok(Some(code));
    }

    // 2. Status route: /tiktok/status or /tiktok/v1/status
    if subpath_clean == "status" || subpath_clean == "v1/status" {
        let status_resp = TikTokStatusResponse {
            enabled: config.tiktok_enabled,
            base_url: creds.base_url.clone(),
            has_cookie: !creds.cookie.is_empty(),
            has_csrf_token: !creds.csrf_token.is_empty(),
            provider_count: 3,
        };
        let code = write_json_response(client, 200, &status_resp)?;
        return Ok(Some(code));
    }

    // 3. Catalog / models route
    if subpath_clean == "catalog" || subpath_clean == "v1/catalog" || subpath_clean == "models" {
        let catalog = serde_json::json!({
            "id": "tiktok",
            "name": "TikTok Creator Studio",
            "description": "Video items management, watermark-free download extraction, and publishing adapter",
            "endpoints": [
                { "path": "/tiktok/v1/config", "method": "GET/POST", "desc": "Read or update TikTok Creator Studio configuration (cookie, csrf_token, base_url)" },
                { "path": "/tiktok/v1/config/har", "method": "POST", "desc": "Ingest a TikTok Studio HAR capture to automatically extract and persist credentials" },
                { "path": "/tiktok/v1/user", "method": "GET", "desc": "Creator account and studio profile" },
                { "path": "/tiktok/v1/items", "method": "GET/POST", "desc": "Query creator studio items list" },
                { "path": "/tiktok/v1/download", "method": "GET", "desc": "Resolve watermark-free video download link (param: item_id, optional redirect=true)" },
                { "path": "/tiktok/v1/upload/auth", "method": "GET", "desc": "Obtain video upload authorization tokens" },
                { "path": "/tiktok/v1/publish", "method": "POST", "desc": "Publish or schedule video project in Creator Studio" },
                { "path": "/tiktok/v1/status", "method": "GET", "desc": "Module readiness and credentials health" }
            ]
        });
        let code = write_json_response(client, 200, &catalog)?;
        return Ok(Some(code));
    }

    // 3. User profile route: /tiktok/v1/user or /tiktok/studio/user or /tiktok/v1/profile
    if subpath_clean == "user"
        || subpath_clean == "v1/user"
        || subpath_clean == "studio/user"
        || subpath_clean == "profile"
        || subpath_clean == "v1/profile"
    {
        if !request.method.eq_ignore_ascii_case("get") {
            let code =
                write_tiktok_error(client, 405, "Method Not Allowed: use GET for user profile")?;
            return Ok(Some(code));
        }

        match service.fetch_user_profile(&creds) {
            Ok(profile) => {
                let code = write_json_response(client, 200, &profile)?;
                return Ok(Some(code));
            }
            Err((status, err)) => {
                let code = write_tiktok_error(client, status, &err)?;
                return Ok(Some(code));
            }
        }
    }

    // 4. Video items management route: /tiktok/v1/items or /tiktok/items
    if subpath_clean == "items" || subpath_clean == "v1/items" {
        let query = if request.method.eq_ignore_ascii_case("post") {
            if request.body.is_empty() {
                TikTokItemListQuery::default()
            } else {
                match serde_json::from_slice::<TikTokItemListQuery>(&request.body) {
                    Ok(q) => q,
                    Err(_) => {
                        let code =
                            write_tiktok_error(client, 400, "Invalid JSON body for items query")?;
                        return Ok(Some(code));
                    }
                }
            }
        } else if request.method.eq_ignore_ascii_case("get") {
            let cursor = extract_query_param(query_str, "cursor")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let size = extract_query_param(query_str, "size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(20);
            TikTokItemListQuery { cursor, size }
        } else {
            let code =
                write_tiktok_error(client, 405, "Method Not Allowed: use GET or POST for items")?;
            return Ok(Some(code));
        };

        match service.list_items(&creds, &query) {
            Ok(resp) => {
                let code = write_json_response(client, 200, &resp)?;
                return Ok(Some(code));
            }
            Err((status, err)) => {
                let code = write_tiktok_error(client, status, &err)?;
                return Ok(Some(code));
            }
        }
    }

    // 5. Download resolution route: /tiktok/v1/download or /tiktok/items/download
    if subpath_clean == "download"
        || subpath_clean == "v1/download"
        || subpath_clean == "items/download"
    {
        if !request.method.eq_ignore_ascii_case("get") {
            let code = write_tiktok_error(client, 405, "Method Not Allowed: use GET for download")?;
            return Ok(Some(code));
        }

        let item_id = match extract_query_param(query_str, "item_id") {
            Some(id) if !id.trim().is_empty() => id.trim().to_string(),
            _ => {
                let code =
                    write_tiktok_error(client, 400, "Missing required query parameter: 'item_id'")?;
                return Ok(Some(code));
            }
        };

        let redirect_requested = extract_query_param(query_str, "redirect")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);

        match service.get_video_download(&creds, &item_id) {
            Ok(info) => {
                if redirect_requested {
                    if let Some(ref direct_url) = info.direct_download_url {
                        let code = write_redirect_response(client, direct_url)?;
                        return Ok(Some(code));
                    }
                }
                let code = write_json_response(client, 200, &info)?;
                return Ok(Some(code));
            }
            Err((status, err)) => {
                let code = write_tiktok_error(client, status, &err)?;
                return Ok(Some(code));
            }
        }
    }

    // 6. Video upload authorization route: /tiktok/v1/upload/auth or /tiktok/publish/auth
    if subpath_clean == "upload/auth"
        || subpath_clean == "v1/upload/auth"
        || subpath_clean == "publish/auth"
    {
        if !request.method.eq_ignore_ascii_case("get") {
            let code =
                write_tiktok_error(client, 405, "Method Not Allowed: use GET for upload auth")?;
            return Ok(Some(code));
        }

        match service.get_upload_auth(&creds) {
            Ok(auth) => {
                let code = write_json_response(client, 200, &auth)?;
                return Ok(Some(code));
            }
            Err((status, err)) => {
                let code = write_tiktok_error(client, status, &err)?;
                return Ok(Some(code));
            }
        }
    }

    // 7. Publish project post route: /tiktok/v1/publish or /tiktok/publish/post
    if subpath_clean == "publish"
        || subpath_clean == "v1/publish"
        || subpath_clean == "publish/post"
    {
        if !request.method.eq_ignore_ascii_case("post") {
            let code = write_tiktok_error(client, 405, "Method Not Allowed: use POST for publish")?;
            return Ok(Some(code));
        }

        let post_req: TikTokPublishRequest = match serde_json::from_slice(&request.body) {
            Ok(p) => p,
            Err(_) => {
                let code =
                    write_tiktok_error(client, 400, "Invalid JSON body for publish request")?;
                return Ok(Some(code));
            }
        };

        if post_req.video_id.trim().is_empty() {
            let code = write_tiktok_error(
                client,
                400,
                "Missing required field 'video_id' in publish request",
            )?;
            return Ok(Some(code));
        }

        match service.publish_post(&creds, &post_req) {
            Ok(resp) => {
                let code = write_json_response(client, 200, &resp)?;
                return Ok(Some(code));
            }
            Err((status, err)) => {
                let code = write_tiktok_error(client, status, &err)?;
                return Ok(Some(code));
            }
        }
    }

    // Fallback within tiktok namespace: 404
    let code = write_tiktok_error(
        client,
        404,
        &format!("Unknown TikTok Creator Studio subpath: '/{subpath_clean}'"),
    )?;
    Ok(Some(code))
}
