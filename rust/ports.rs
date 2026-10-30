//! Outbound port traits for the TikTok Creator Studio application layer.
//!
//! Enforces the Dependency Inversion Principle (DIP): application use cases depend on
//! these pure abstractions rather than concrete HTTP/network drivers.

use super::types::{
    TikTokCredentials, TikTokError, TikTokItemListQuery, TikTokItemListResponse,
    TikTokPublishRequest, TikTokPublishResponse, TikTokUploadAuth, TikTokUserProfile,
};

/// Outbound port for communicating with TikTok Creator Studio upstream APIs.
pub trait TikTokClientPort: Send + Sync {
    /// Fetches the creator profile from the primary Studio Web User endpoint.
    fn fetch_studio_user(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError>;

    /// Fetches the creator profile from the Upload Profile fallback endpoint.
    fn fetch_upload_profile(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError>;

    /// Queries the paginated creator studio item list.
    fn fetch_item_list(
        &self,
        creds: &TikTokCredentials,
        query: &TikTokItemListQuery,
    ) -> Result<TikTokItemListResponse, TikTokError>;

    /// Requests upload authorization tokens and storage region.
    fn fetch_upload_auth(&self, creds: &TikTokCredentials)
        -> Result<TikTokUploadAuth, TikTokError>;

    /// Submits a video post project to Creator Studio.
    fn send_publish_post(
        &self,
        creds: &TikTokCredentials,
        post_req: &TikTokPublishRequest,
    ) -> Result<TikTokPublishResponse, TikTokError>;
}
