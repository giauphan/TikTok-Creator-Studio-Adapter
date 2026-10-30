//! Configuration management and HAR credential extraction for TikTok Creator Studio.

use super::handlers::{write_json_response, write_tiktok_error};
use super::types::TikTokError;
use crate::config::{Config, ConfigStore};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io;
use std::net::TcpStream;

/// Payload for updating TikTok Creator Studio configuration via JSON API or HAR extraction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateTikTokConfigPayload {
    pub base_url: Option<String>,
    pub cookie: Option<String>,
    pub csrf_token: Option<String>,
    pub enabled: Option<bool>,
}

impl UpdateTikTokConfigPayload {
    /// Validates incoming configuration fields at the boundary.
    pub fn validate(&self) -> Result<(), TikTokError> {
        if let Some(ref base) = self.base_url {
            let trimmed = base.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("http://")
                && !trimmed.starts_with("https://")
            {
                return Err(TikTokError::BadRequest(
                    "base_url must start with http:// or https://".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Applies non-empty fields onto the runtime [`Config`].
    pub fn apply_to(&self, conf: &mut Config) {
        if let Some(ref base) = self.base_url {
            let trimmed = base.trim();
            if !trimmed.is_empty() {
                conf.tiktok_base_url = trimmed.to_string();
            }
        }
        if let Some(ref cookie) = self.cookie {
            let trimmed = cookie.trim();
            if !trimmed.is_empty() {
                conf.tiktok_cookie = trimmed.to_string();
            }
        }
        if let Some(ref csrf) = self.csrf_token {
            let trimmed = csrf.trim();
            if !trimmed.is_empty() {
                conf.tiktok_csrf_token = trimmed.to_string();
            }
        }
        if let Some(enabled) = self.enabled {
            conf.tiktok_enabled = enabled;
        }
    }
}

/// Masks a secret token or cookie for safe display in configuration read responses.
pub fn mask_tiktok_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let char_count = trimmed.chars().count();
    if char_count <= 8 {
        return "****".to_string();
    }
    let prefix: String = trimmed.chars().take(4).collect();
    let suffix: String = trimmed.chars().skip(char_count - 4).collect();
    format!("{prefix}...{suffix}")
}

/// Returns the authoritative UI configuration schema for the TikTok Creator Studio module.
pub fn tiktok_config_schema() -> Value {
    serde_json::json!({
        "title": "TikTok Creator Studio Config & HAR Import",
        "module_id": "tiktok",
        "namespace": "/tiktok",
        "har_endpoint": "/tiktok/v1/config/har",
        "har_hint": "Select download-tiktok.har to automatically extract cookies and CSRF token.",
        "fields": [
            { "key": "base_url", "label": "TikTok Base URL", "type": "text", "placeholder": "https://www.tiktok.com" },
            { "key": "enabled", "label": "Module Status", "type": "boolean", "default": true },
            { "key": "cookie", "label": "Session Cookie", "type": "password", "secret": true, "placeholder": "Leave empty to keep existing" },
            { "key": "csrf_token", "label": "CSRF Token (tt-csrf-token)", "type": "password", "secret": true, "placeholder": "Leave empty to keep existing" }
        ]
    })
}

/// Builds a sanitized JSON representation of the active TikTok configuration.
pub fn build_tiktok_config_view(store: &ConfigStore) -> Value {
    let snapshot = store.snapshot();
    serde_json::json!({
        "base_url": snapshot.tiktok_base_url,
        "enabled": snapshot.tiktok_enabled,
        "configured": !snapshot.tiktok_cookie.is_empty() || !snapshot.tiktok_csrf_token.is_empty(),
        "has_cookie": !snapshot.tiktok_cookie.is_empty(),
        "has_csrf_token": !snapshot.tiktok_csrf_token.is_empty(),
        "masked_cookie": mask_tiktok_secret(&snapshot.tiktok_cookie),
        "masked_csrf_token": mask_tiktok_secret(&snapshot.tiktok_csrf_token),
        "config_storage": store.config_file.file_name().unwrap_or_default().to_string_lossy(),
    })
}

/// Extracts TikTok Creator Studio credentials (`base_url`, `csrf_token`, `cookie`)
/// from a raw HAR capture.
pub fn extract_tiktok_config_from_har(
    raw_har: &[u8],
) -> Result<UpdateTikTokConfigPayload, TikTokError> {
    let root: Value = serde_json::from_slice(raw_har)
        .map_err(|_| TikTokError::BadRequest("HAR is not valid JSON".to_string()))?;

    let entries = root
        .get("log")
        .and_then(|l| l.get("entries"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TikTokError::BadRequest("Not a HAR file (missing log.entries)".to_string())
        })?;

    let mut found_tiktok = false;
    let mut base_url: Option<String> = None;
    let mut csrf_token: Option<String> = None;
    let mut cookie: Option<String> = None;

    for entry in entries {
        let Some(req) = entry.get("request") else {
            continue;
        };
        let url = req.get("url").and_then(Value::as_str).unwrap_or("");
        if !url.contains("tiktok.com/") {
            continue;
        }
        if url.contains("www.tiktok.com/") {
            found_tiktok = true;
            if base_url.is_none() {
                base_url = Some("https://www.tiktok.com".to_string());
            }
        }

        // Inspect request headers for Cookie and tt-csrf-token
        if let Some(headers) = req.get("headers").and_then(Value::as_array) {
            for h in headers {
                let name = h.get("name").and_then(Value::as_str).unwrap_or("");
                let val = h.get("value").and_then(Value::as_str).unwrap_or("").trim();
                if val.is_empty() {
                    continue;
                }
                if name.eq_ignore_ascii_case("tt-csrf-token")
                    || name.eq_ignore_ascii_case("x-tiktok-csrf-token")
                {
                    csrf_token = Some(val.to_string());
                } else if name.eq_ignore_ascii_case("cookie") {
                    cookie = Some(val.to_string());
                }
            }
        }

        // Also check request.cookies array if cookie header was omitted
        if cookie.is_none() {
            if let Some(cookies_arr) = req.get("cookies").and_then(Value::as_array) {
                let pairs: Vec<String> = cookies_arr
                    .iter()
                    .filter_map(|c| {
                        let k = c.get("name")?.as_str()?.trim();
                        let v = c.get("value")?.as_str()?.trim();
                        (!k.is_empty() && !v.is_empty()).then(|| format!("{k}={v}"))
                    })
                    .collect();
                if !pairs.is_empty() {
                    cookie = Some(pairs.join("; "));
                }
            }
        }

        // Also check /tiktokstudio/api/web/user response body for tt-csrf-token
        if csrf_token.is_none() && url.contains("/tiktokstudio/api/web/user") {
            if let Some(text) = entry
                .get("response")
                .and_then(|r| r.get("content"))
                .and_then(|c| c.get("text"))
                .and_then(Value::as_str)
            {
                if let Ok(resp_json) = serde_json::from_str::<Value>(text) {
                    if let Some(tok) = resp_json
                        .get("tt-csrf-token")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        csrf_token = Some(tok.to_string());
                    }
                }
            }
        }
    }

    if !found_tiktok {
        return Err(TikTokError::BadRequest(
            "No TikTok Creator Studio entries found in HAR".to_string(),
        ));
    }

    Ok(UpdateTikTokConfigPayload {
        base_url,
        cookie,
        csrf_token,
        enabled: Some(true),
    })
}

/// Handles `GET /tiktok/v1/config` (returns current masked configuration).
pub fn handle_get_tiktok_config(client: &mut TcpStream, store: &ConfigStore) -> io::Result<u16> {
    let view = serde_json::json!({
        "ok": true,
        "tiktok": build_tiktok_config_view(store),
    });
    write_json_response(client, 200, &view)
}

/// Handles `POST /tiktok/v1/config` and `POST /api/v1/dashboard/tiktok-config`.
pub fn handle_update_tiktok_config(
    client: &mut TcpStream,
    body: &[u8],
    store: &ConfigStore,
) -> io::Result<u16> {
    let payload: UpdateTikTokConfigPayload = match serde_json::from_slice(body) {
        Ok(p) => p,
        Err(err) => {
            return write_tiktok_error(client, 400, &format!("Invalid JSON: {err}"));
        }
    };

    if let Err(err) = payload.validate() {
        return write_tiktok_error(client, err.status_code(), err.message());
    }

    {
        let mut conf = store.write_guard();
        payload.apply_to(&mut conf);
    }
    let _ = store.save_to_persistent_file();

    let resp = serde_json::json!({
        "ok": true,
        "message": "TikTok Creator Studio configuration updated and saved to persistent storage",
        "tiktok": build_tiktok_config_view(store),
    });
    write_json_response(client, 200, &resp)
}

/// Handles `POST /tiktok/v1/config/har` (extracts credentials from HAR and persists).
pub fn handle_apply_tiktok_har(
    client: &mut TcpStream,
    body: &[u8],
    store: &ConfigStore,
) -> io::Result<u16> {
    let extracted = match extract_tiktok_config_from_har(body) {
        Ok(p) => p,
        Err(err) => {
            return write_tiktok_error(client, err.status_code(), err.message());
        }
    };

    {
        let mut conf = store.write_guard();
        extracted.apply_to(&mut conf);
    }
    let _ = store.save_to_persistent_file();

    let resp = serde_json::json!({
        "ok": true,
        "source": "har",
        "message": "TikTok Creator Studio HAR configuration extracted and saved to persistent storage",
        "tiktok": build_tiktok_config_view(store),
    });
    write_json_response(client, 200, &resp)
}
