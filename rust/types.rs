//! Domain entities, value object invariants, and error types for TikTok Creator Studio.
//!
//! Pure domain layer: zero dependencies on HTTP frameworks, sockets, or transport drivers.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Domain-level error representing business rule violations or upstream failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TikTokError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Upstream(u16, String),
}

impl TikTokError {
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::Forbidden(_) => 403,
            Self::NotFound(_) => 404,
            Self::Upstream(code, _) => *code,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(msg)
            | Self::Unauthorized(msg)
            | Self::Forbidden(msg)
            | Self::NotFound(msg)
            | Self::Upstream(_, msg) => msg,
        }
    }

    pub fn into_status_pair(self) -> (u16, String) {
        let code = self.status_code();
        match self {
            Self::BadRequest(msg)
            | Self::Unauthorized(msg)
            | Self::Forbidden(msg)
            | Self::NotFound(msg)
            | Self::Upstream(_, msg) => (code, msg),
        }
    }
}

impl fmt::Display for TikTokError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.status_code(), self.message())
    }
}

impl std::error::Error for TikTokError {}

/// Validates that a TikTok identifier (`item_id` or `video_id`) is non-empty,
/// bounded in length, and contains only safe ASCII identifier characters.
pub fn is_valid_tiktok_identifier(id: &str) -> bool {
    let trimmed = id.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 128
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Credentials and request context for TikTok Creator Studio interactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TikTokCredentials {
    pub base_url: String,
    pub cookie: String,
    pub csrf_token: String,
    pub user_agent: String,
    pub aid: String,
    pub locale: String,
    pub priority_region: String,
}

impl Default for TikTokCredentials {
    fn default() -> Self {
        Self {
            base_url: "https://www.tiktok.com".to_string(),
            cookie: String::new(),
            csrf_token: String::new(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            aid: "1988".to_string(),
            locale: "en".to_string(),
            priority_region: "US".to_string(),
        }
    }
}

/// Creator account profile and audience metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokUserProfile {
    pub user_id: String,
    pub nickname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_thumb: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_uid: Option<String>,
    pub follower_count: u64,
    pub following_count: u64,
    pub status_code: i32,
}

/// Engagement metrics for a creator video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokVideoStats {
    pub play_count: u64,
    pub like_count: u64,
    pub comment_count: u64,
    pub share_count: u64,
    pub favorite_count: u64,
}

/// Download capability and resolved download links for a TikTok video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokDownloadInfo {
    pub allow_download: bool,
    pub disable_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_download_url: Option<String>,
    pub play_addrs: Vec<String>,
}

/// Detailed creator video item parsed from TikTok Creator Studio manage items API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokVideoItem {
    pub item_id: String,
    pub title: String,
    pub desc: String,
    pub duration: u64,
    pub create_time: u64,
    pub post_time: u64,
    pub status: i32,
    pub visibility: i32,
    pub cover_urls: Vec<String>,
    pub download_info: TikTokDownloadInfo,
    pub stats: TikTokVideoStats,
}

/// Query parameters for listing creator items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TikTokItemListQuery {
    pub cursor: u64,
    pub size: u32,
}

impl TikTokItemListQuery {
    /// Validates pagination boundaries (`1 <= size <= 100`).
    pub fn validate(&self) -> Result<(), TikTokError> {
        if self.size == 0 || self.size > 100 {
            return Err(TikTokError::BadRequest(format!(
                "Invalid page size '{}': must be between 1 and 100",
                self.size
            )));
        }
        Ok(())
    }
}

impl Default for TikTokItemListQuery {
    fn default() -> Self {
        Self {
            cursor: 0,
            size: 20,
        }
    }
}

/// Response containing paginated creator videos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokItemListResponse {
    pub cursor: u64,
    pub has_more: bool,
    pub status_code: i32,
    pub items: Vec<TikTokVideoItem>,
}

/// Video upload authorization tokens and storage region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokUploadAuth {
    pub video_token_v5: String,
    pub audio_token_v5: String,
    pub store_region: String,
    pub status_code: i32,
}

/// Request to publish or schedule a post in Creator Studio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokPublishRequest {
    pub video_id: String,
    pub title: String,
    pub text: String,
    pub visibility_type: u32,
}

impl TikTokPublishRequest {
    /// Validates domain invariants before publishing.
    pub fn validate(&self) -> Result<(), TikTokError> {
        if !is_valid_tiktok_identifier(&self.video_id) {
            return Err(TikTokError::BadRequest(
                "Missing or invalid 'video_id' in publish request".to_string(),
            ));
        }
        if self.visibility_type > 2 {
            return Err(TikTokError::BadRequest(format!(
                "Invalid visibility_type '{}': expected 0 (public), 1 (friends), or 2 (private)",
                self.visibility_type
            )));
        }
        Ok(())
    }
}

/// Response after initiating a project post in Creator Studio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TikTokPublishResponse {
    pub project_id: String,
    pub project_status: i32,
    pub status_code: i32,
}

/// Module readiness and credential status summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TikTokStatusResponse {
    pub enabled: bool,
    pub base_url: String,
    pub has_cookie: bool,
    pub has_csrf_token: bool,
    pub provider_count: usize,
}
