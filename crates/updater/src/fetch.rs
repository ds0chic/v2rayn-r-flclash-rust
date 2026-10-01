//! Fetch release metadata over HTTP.
//!
//! Wraps a [`reqwest`] client around the GitHub releases endpoint described by
//! [`crate::metadata::ReleasesClient`]. The client never reads the environment
//! proxy — loopback test servers must be reached directly.

use std::time::Duration;

use crate::error::UpdateError;
use crate::metadata::{parse_releases, ReleaseInfo, ReleasesClient};

/// A source that can return the releases list for a repository.
pub trait ReleaseSource: Send + Sync {
    fn releases(&self, client: &ReleasesClient) -> Result<Vec<ReleaseInfo>, UpdateError>;
}

/// HTTP-backed release source.
#[derive(Debug, Clone)]
pub struct CoreReleaseApi {
    http: reqwest::Client,
    pub user_agent: String,
}

impl CoreReleaseApi {
    /// Build with a total timeout and no environment proxy.
    pub fn new(timeout: Duration) -> Result<Self, UpdateError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(10))
            .no_proxy()
            .build()
            .map_err(|e| UpdateError::Download(format!("client build: {e}")))?;
        Ok(Self {
            http,
            user_agent: "v2rayN-updater".to_string(),
        })
    }

    /// Fetch and parse `GET {releases_url}`.
    pub async fn fetch(&self, client: &ReleasesClient) -> Result<Vec<ReleaseInfo>, UpdateError> {
        let response = self
            .http
            .get(client.releases_url())
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    UpdateError::Timeout
                } else {
                    UpdateError::Download(e.to_string())
                }
            })?;
        if !response.status().is_success() {
            return Err(UpdateError::Download(format!(
                "releases status {}",
                response.status().as_u16()
            )));
        }
        let body = response
            .text()
            .await
            .map_err(|e| UpdateError::Download(e.to_string()))?;
        parse_releases(&body)
    }
}
