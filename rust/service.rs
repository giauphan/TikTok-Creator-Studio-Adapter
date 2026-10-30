//! Application use cases and service orchestration for TikTok Creator Studio.
//!
//! Pure application layer: orchestrates domain logic through the [`TikTokClientPort`]
//! abstraction without coupling to concrete HTTP drivers or socket transports.

use super::client::UreqTikTokClient;
use super::ports::TikTokClientPort;
use super::types::{
    is_valid_tiktok_identifier, TikTokCredentials, TikTokDownloadInfo, TikTokError,
    TikTokItemListQuery, TikTokItemListResponse, TikTokPublishRequest, TikTokPublishResponse,
    TikTokUploadAuth, TikTokUserProfile,
};
use std::time::Duration;

/// Application service orchestrating TikTok Creator Studio use cases.
pub struct TikTokStudioService<C: TikTokClientPort = UreqTikTokClient> {
    client: C,
}

impl Default for TikTokStudioService<UreqTikTokClient> {
    fn default() -> Self {
        Self::new(Some(Duration::from_secs(30)))
    }
}

impl TikTokStudioService<UreqTikTokClient> {
    pub fn new(timeout: Option<Duration>) -> Self {
        Self {
            client: UreqTikTokClient::new(timeout),
        }
    }
}

impl<C: TikTokClientPort> TikTokStudioService<C> {
    /// Constructs a service instance with an injected [`TikTokClientPort`] implementation.
    pub fn with_client(client: C) -> Self {
        Self { client }
    }

    /// Fetches the authenticated creator profile, falling back to the upload profile endpoint
    /// when the primary studio user endpoint does not return a valid profile.
    pub fn fetch_user_profile(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, (u16, String)> {
        match self.client.fetch_studio_user(creds) {
            Ok(profile) => Ok(profile),
            Err(_) => self
                .client
                .fetch_upload_profile(creds)
                .map_err(TikTokError::into_status_pair),
        }
    }

    /// Lists creator video items after validating pagination bounds.
    pub fn list_items(
        &self,
        creds: &TikTokCredentials,
        query: &TikTokItemListQuery,
    ) -> Result<TikTokItemListResponse, (u16, String)> {
        query.validate().map_err(TikTokError::into_status_pair)?;
        self.client
            .fetch_item_list(creds, query)
            .map_err(TikTokError::into_status_pair)
    }

    /// Resolves the direct watermark-free download link for a given video item ID.
    pub fn get_video_download(
        &self,
        creds: &TikTokCredentials,
        item_id: &str,
    ) -> Result<TikTokDownloadInfo, (u16, String)> {
        let clean_id = item_id.trim();
        if !is_valid_tiktok_identifier(clean_id) {
            return Err(
                TikTokError::BadRequest(format!("Invalid TikTok item_id '{item_id}'"))
                    .into_status_pair(),
            );
        }

        let query = TikTokItemListQuery {
            cursor: 0,
            size: 50,
        };
        let res = self
            .client
            .fetch_item_list(creds, &query)
            .map_err(TikTokError::into_status_pair)?;

        let matching_item = res.items.into_iter().find(|item| item.item_id == clean_id);

        match matching_item {
            Some(item) => {
                if !item.download_info.allow_download
                    && item.download_info.direct_download_url.is_none()
                {
                    return Err(TikTokError::Forbidden(format!(
                        "Video download is disabled for item '{clean_id}': {}",
                        item.download_info.disable_reason
                    ))
                    .into_status_pair());
                }
                Ok(item.download_info)
            }
            None => Err(TikTokError::NotFound(format!(
                "TikTok video item with ID '{clean_id}' not found in Creator Studio"
            ))
            .into_status_pair()),
        }
    }

    /// Obtains video upload authorization credentials from Creator Studio.
    pub fn get_upload_auth(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUploadAuth, (u16, String)> {
        self.client
            .fetch_upload_auth(creds)
            .map_err(TikTokError::into_status_pair)
    }

    /// Validates and submits a video post project to Creator Studio.
    pub fn publish_post(
        &self,
        creds: &TikTokCredentials,
        post_req: &TikTokPublishRequest,
    ) -> Result<TikTokPublishResponse, (u16, String)> {
        post_req.validate().map_err(TikTokError::into_status_pair)?;
        self.client
            .send_publish_post(creds, post_req)
            .map_err(TikTokError::into_status_pair)
    }
}
