//! Outbound HTTP client adapter implementing [`TikTokClientPort`] via `ureq`.

use super::ports::TikTokClientPort;
use super::protocol::{
    build_item_list_request_body, build_publish_request_body, parse_item_list_response,
    parse_publish_response, parse_upload_auth_response, parse_user_response,
};
use super::types::{
    TikTokCredentials, TikTokError, TikTokItemListQuery, TikTokItemListResponse,
    TikTokPublishRequest, TikTokPublishResponse, TikTokUploadAuth, TikTokUserProfile,
};
use std::time::Duration;

/// Concrete `ureq`-based HTTP adapter for TikTok Creator Studio upstream APIs.
#[derive(Debug, Clone)]
pub struct UreqTikTokClient {
    timeout: Option<Duration>,
}

impl Default for UreqTikTokClient {
    fn default() -> Self {
        Self::new(Some(Duration::from_secs(30)))
    }
}

impl UreqTikTokClient {
    pub fn new(timeout: Option<Duration>) -> Self {
        Self { timeout }
    }

    fn build_agent(&self) -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .timeout_global(self.timeout)
            .max_redirects(3)
            .http_status_as_error(false)
            .build();
        ureq::Agent::new_with_config(config)
    }

    fn attach_standard_headers<B>(
        req: ureq::RequestBuilder<B>,
        creds: &TikTokCredentials,
    ) -> ureq::RequestBuilder<B> {
        let mut builder = req
            .header("User-Agent", &creds.user_agent)
            .header("Accept", "application/json, text/plain, */*")
            .header(
                "Referer",
                &format!("{}/tiktokstudio/content", creds.base_url),
            )
            .header("Origin", &creds.base_url);

        if !creds.cookie.is_empty() {
            builder = builder.header("Cookie", &creds.cookie);
        }
        if !creds.csrf_token.is_empty() {
            builder = builder.header("tt-csrf-token", &creds.csrf_token);
        }
        builder
    }
}

impl TikTokClientPort for UreqTikTokClient {
    fn fetch_studio_user(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError> {
        let agent = self.build_agent();
        let base = creds.base_url.trim_end_matches('/');
        let studio_url = format!("{base}/tiktokstudio/api/web/user");
        let req = Self::attach_standard_headers(agent.get(&studio_url), creds);

        match req.call() {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse_user_response(&body).map_err(|e| TikTokError::Upstream(502, e))
                } else {
                    Err(TikTokError::Upstream(
                        status,
                        format!("TikTok Studio web user endpoint returned status {status}"),
                    ))
                }
            }
            Err(e) => Err(TikTokError::Upstream(
                502,
                format!("TikTok Studio transport error: {e}"),
            )),
        }
    }

    fn fetch_upload_profile(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUserProfile, TikTokError> {
        let agent = self.build_agent();
        let base = creds.base_url.trim_end_matches('/');
        let upload_profile_url = format!(
            "{base}/api/v1/user/profile/upload/?locale={}&aid={}&priority_region={}",
            creds.locale, creds.aid, creds.priority_region
        );
        let req = Self::attach_standard_headers(agent.get(&upload_profile_url), creds);

        match req.call() {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse_user_response(&body).map_err(|e| TikTokError::Upstream(502, e))
                } else {
                    Err(TikTokError::Upstream(
                        status,
                        format!("TikTok Studio user profile request failed with status {status}"),
                    ))
                }
            }
            Err(e) => Err(TikTokError::Upstream(
                502,
                format!("TikTok Studio transport error: {e}"),
            )),
        }
    }

    fn fetch_item_list(
        &self,
        creds: &TikTokCredentials,
        query: &TikTokItemListQuery,
    ) -> Result<TikTokItemListResponse, TikTokError> {
        let agent = self.build_agent();
        let base = creds.base_url.trim_end_matches('/');
        let url = format!(
            "{base}/tiktok/creator/manage/item_list/v1/?locale={}&aid={}&priority_region={}&app_name=tiktok_creator_center",
            creds.locale, creds.aid, creds.priority_region
        );

        let body_str = build_item_list_request_body(query);
        let req = agent.post(&url).header("Content-Type", "application/json");
        let req = Self::attach_standard_headers(req, creds);

        match req.send(&body_str) {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse_item_list_response(&body).map_err(|e| TikTokError::Upstream(502, e))
                } else {
                    Err(TikTokError::Upstream(
                        status,
                        format!("TikTok Studio item list returned status {status}: {body}"),
                    ))
                }
            }
            Err(e) => Err(TikTokError::Upstream(
                502,
                format!("TikTok Studio transport error: {e}"),
            )),
        }
    }

    fn fetch_upload_auth(
        &self,
        creds: &TikTokCredentials,
    ) -> Result<TikTokUploadAuth, TikTokError> {
        let agent = self.build_agent();
        let base = creds.base_url.trim_end_matches('/');
        let url = format!("{base}/api/v1/video/upload/auth/?aid={}", creds.aid);
        let req = Self::attach_standard_headers(agent.get(&url), creds);

        match req.call() {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse_upload_auth_response(&body).map_err(|e| TikTokError::Upstream(502, e))
                } else {
                    Err(TikTokError::Upstream(
                        status,
                        format!("TikTok video upload auth returned status {status}"),
                    ))
                }
            }
            Err(e) => Err(TikTokError::Upstream(
                502,
                format!("TikTok Studio transport error: {e}"),
            )),
        }
    }

    fn send_publish_post(
        &self,
        creds: &TikTokCredentials,
        post_req: &TikTokPublishRequest,
    ) -> Result<TikTokPublishResponse, TikTokError> {
        let agent = self.build_agent();
        let base = creds.base_url.trim_end_matches('/');
        let url = format!(
            "{base}/tiktok/web/project/post/v1/?aid={}&app_name=tiktok_creator_center",
            creds.aid
        );

        let body_str = build_publish_request_body(post_req);
        let req = agent.post(&url).header("Content-Type", "application/json");
        let req = Self::attach_standard_headers(req, creds);

        match req.send(&body_str) {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                if status == 200 {
                    parse_publish_response(&body).map_err(|e| TikTokError::Upstream(502, e))
                } else {
                    Err(TikTokError::Upstream(
                        status,
                        format!("TikTok Studio publish returned status {status}: {body}"),
                    ))
                }
            }
            Err(e) => Err(TikTokError::Upstream(
                502,
                format!("TikTok Studio transport error: {e}"),
            )),
        }
    }
}
