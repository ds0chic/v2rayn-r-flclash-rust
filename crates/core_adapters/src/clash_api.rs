//! Clash API client for sing-box/mihomo (`ClashApiManager`).
//!
//! Only the read/observe surface used by the monitor UI is implemented here:
//! proxies (with provider merge), delay probes, connections and their close
//! operations, plus `/configs`. Config mutation (`SetActiveProxy`,
//! `UpdateClashMode`) belongs to the runtime/process-owner layer.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{classify_reqwest, HttpError};

/// Upstream `ClashApiManager.TestProxyDelay` hard-codes `timeout=10000`.
pub const DEFAULT_DELAY_TIMEOUT_MS: u32 = 10_000;

/// `ClashProxy` (`ClashItem.cs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashProxy {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type", default)]
    pub proxy_type: Option<String>,
    #[serde(default)]
    pub udp: bool,
    #[serde(default)]
    pub now: Option<String>,
    /// Only present on selector/url-test groups.
    #[serde(default)]
    pub all: Option<Vec<String>>,
    #[serde(default)]
    pub history: Option<Vec<HistoryItem>>,
    #[serde(default)]
    pub delay: i32,
}

/// `ClashProxy.HistoryItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub delay: i32,
}

/// `ClashProvider`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashProvider {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub vehicle_type: Option<String>,
    #[serde(default)]
    pub proxies: Option<Vec<ClashProxy>>,
}

/// `ClashProxies` (`/proxies`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashProxies {
    #[serde(default)]
    pub proxies: BTreeMap<String, ClashProxy>,
}

/// `ClashProviders` (`/providers/proxies`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashProviders {
    #[serde(default)]
    pub providers: BTreeMap<String, ClashProvider>,
}

/// `ClashItem`: merged `/proxies` + `/providers/proxies`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashItem {
    pub proxies: BTreeMap<String, ClashProxy>,
    /// proxy name -> provider key, for provider-scoped delay probes.
    pub provider_index_map: BTreeMap<String, String>,
}

impl ClashItem {
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty()
    }

    fn merge(&mut self, proxies: ClashProxies, providers: ClashProviders) {
        for (name, proxy) in proxies.proxies {
            self.proxies.insert(name, proxy);
        }
        for (provider_name, provider) in providers.providers {
            for proxy in provider.proxies.unwrap_or_default() {
                let Some(name) = proxy.name.clone() else {
                    continue;
                };
                if name.is_empty() || self.proxies.contains_key(&name) {
                    continue;
                }
                self.proxies.insert(name.clone(), proxy);
                self.provider_index_map.insert(name, provider_name.clone());
            }
        }
    }
}

/// `ClashConnections`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClashConnections {
    #[serde(default, rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(default, rename = "uploadTotal")]
    pub upload_total: u64,
    #[serde(default)]
    pub connections: Option<Vec<ConnectionItem>>,
}

/// `ConnectionItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnectionItem {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub metadata: Option<MetadataItem>,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    /// Upstream models this as `DateTime`; kept as the raw string so the crate
    /// stays free of a date-time dependency.
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub chains: Option<Vec<String>>,
    #[serde(default)]
    pub rule: Option<String>,
    #[serde(default, rename = "rulePayload")]
    pub rule_payload: Option<String>,
}

/// `MetadataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetadataItem {
    #[serde(default)]
    pub network: Option<String>,
    #[serde(rename = "type", default)]
    pub connection_type: Option<String>,
    #[serde(default, rename = "sourceIP")]
    pub source_ip: Option<String>,
    #[serde(default, rename = "destinationIP")]
    pub destination_ip: Option<String>,
    #[serde(default, rename = "sourcePort")]
    pub source_port: Option<String>,
    #[serde(default, rename = "destinationPort")]
    pub destination_port: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default, rename = "nsMode")]
    pub ns_mode: Option<String>,
    /// Clash emits `uid` as either a number or a string.
    #[serde(default)]
    pub uid: Option<Value>,
    #[serde(default)]
    pub process: Option<String>,
    #[serde(default, rename = "processPath")]
    pub process_path: Option<String>,
    #[serde(default, rename = "remoteDestination")]
    pub remote_destination: Option<String>,
    #[serde(default, rename = "sniffHost")]
    pub sniff_host: Option<String>,
}

/// Clash API failure; wraps the shared [`HttpError`] classification.
#[derive(Debug, thiserror::Error)]
pub enum ClashError {
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("clash endpoint returned an empty body")]
    Empty,
}

/// HTTP client for the Clash controller (`StatePort2` for sing-box).
pub struct ClashApiClient {
    client: reqwest::Client,
    base: String,
    secret: Option<String>,
    timeout: Duration,
}

impl ClashApiClient {
    /// Build a client for `http://127.0.0.1:{port}`.
    pub fn new(port: u16, secret: Option<String>, timeout: Duration) -> Result<Self, ClashError> {
        Self::from_base(format!("http://127.0.0.1:{port}"), secret, timeout)
    }

    /// Build from an explicit base URL (mock servers in tests).
    pub fn from_base(
        base: impl Into<String>,
        secret: Option<String>,
        timeout: Duration,
    ) -> Result<Self, ClashError> {
        // Never let an environment proxy (the user's live 10808) intercept a
        // loopback control request.
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|err| ClashError::Http(classify_reqwest(&err)))?;
        Ok(Self {
            client,
            base: base.into(),
            secret,
            timeout,
        })
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .request(method, format!("{}{}", self.base, path))
            .timeout(self.timeout);
        if let Some(secret) = &self.secret {
            request = request.header(AUTHORIZATION, format!("Bearer {secret}"));
        }
        request
    }

    async fn get_text(&self, path: &str) -> Result<String, ClashError> {
        let response = self
            .request(reqwest::Method::GET, path)
            .send()
            .await
            .map_err(|err| ClashError::Http(classify_reqwest(&err)))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClashError::Http(HttpError::Unauthorized));
        }
        let body = response
            .text()
            .await
            .map_err(|err| ClashError::Http(classify_reqwest(&err)))?;
        if !status.is_success() {
            return Err(ClashError::Http(HttpError::Status(status.as_u16())));
        }
        Ok(body)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ClashError> {
        let body = self.get_text(path).await?;
        serde_json::from_str(&body)
            .map_err(|err| ClashError::Http(HttpError::Decode(err.to_string())))
    }

    async fn delete(&self, path: &str) -> Result<(), ClashError> {
        let response = self
            .request(reqwest::Method::DELETE, path)
            .send()
            .await
            .map_err(|err| ClashError::Http(classify_reqwest(&err)))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClashError::Http(HttpError::Unauthorized));
        }
        if !status.is_success() {
            return Err(ClashError::Http(HttpError::Status(status.as_u16())));
        }
        Ok(())
    }

    /// `GET /proxies` merged with `GET /providers/proxies`.
    ///
    /// Unlike upstream, a partial failure is tolerated: if only one endpoint
    /// answers, the other is skipped (upstream retries when *both* are null).
    pub async fn get_proxies(&self) -> Result<ClashItem, ClashError> {
        let proxies = self.get_json::<ClashProxies>("/proxies");
        let providers = self.get_json::<ClashProviders>("/providers/proxies");
        let (proxies, providers) = tokio::join!(proxies, providers);
        match (proxies, providers) {
            (Ok(proxies), Ok(providers)) => {
                let mut item = ClashItem::default();
                item.merge(proxies, providers);
                Ok(item)
            }
            (Ok(proxies), Err(_)) => {
                let mut item = ClashItem::default();
                item.merge(proxies, ClashProviders::default());
                Ok(item)
            }
            (Err(_), Ok(providers)) => {
                let mut item = ClashItem::default();
                item.merge(ClashProxies::default(), providers);
                Ok(item)
            }
            (Err(err), Err(_)) => Err(err),
        }
    }

    /// Upstream retries 3 times with a 2s pause when both endpoints are null.
    pub async fn get_proxies_with_retry(
        &self,
        attempts: usize,
        delay: Duration,
    ) -> Result<ClashItem, ClashError> {
        let mut last = ClashError::Empty;
        for attempt in 0..attempts.max(1) {
            match self.get_proxies().await {
                Ok(item) if !item.is_empty() => return Ok(item),
                Ok(item) => return Ok(item),
                Err(err) => {
                    last = err;
                    if attempt + 1 < attempts {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Err(last)
    }

    /// `GET /proxies/{name}`.
    pub async fn get_proxy(&self, name: &str) -> Result<ClashProxy, ClashError> {
        self.get_json(&format!("/proxies/{}", urlencoding::encode(name)))
            .await
    }

    /// `PUT /proxies/{group}` selecting `name` (upstream
    /// `ClashApiManager.SetActiveProxy`). Additive T15a method on the otherwise
    /// read-only client; existing reads/tests are unchanged.
    pub async fn select_proxy(&self, group: &str, name: &str) -> Result<(), ClashError> {
        let body = serde_json::json!({ "name": name }).to_string();
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/proxies/{}", urlencoding::encode(group)),
            )
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|err| ClashError::Http(classify_reqwest(&err)))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClashError::Http(HttpError::Unauthorized));
        }
        if !status.is_success() {
            return Err(ClashError::Http(HttpError::Status(status.as_u16())));
        }
        Ok(())
    }

    /// `GET /proxies/{name}/delay?timeout=&url=` returning `-1` on any failure.
    pub async fn get_proxy_delay(&self, name: &str, timeout_ms: u32, test_url: &str) -> i32 {
        self.try_get_proxy_delay(name, timeout_ms, test_url)
            .await
            .unwrap_or(-1)
    }

    /// Same as [`Self::get_proxy_delay`] but surfaces the failure reason.
    pub async fn try_get_proxy_delay(
        &self,
        name: &str,
        timeout_ms: u32,
        test_url: &str,
    ) -> Result<i32, ClashError> {
        let path = format!(
            "/proxies/{}/delay?timeout={}&url={}",
            urlencoding::encode(name),
            timeout_ms,
            urlencoding::encode(test_url)
        );
        self.delay_value(&path).await
    }

    /// `GET /providers/proxies/{provider}/{name}/healthcheck?timeout=&url=`.
    pub async fn try_get_provider_proxy_delay(
        &self,
        provider: &str,
        name: &str,
        timeout_ms: u32,
        test_url: &str,
    ) -> Result<i32, ClashError> {
        let path = format!(
            "/providers/proxies/{}/{}/healthcheck?timeout={}&url={}",
            urlencoding::encode(provider),
            urlencoding::encode(name),
            timeout_ms,
            urlencoding::encode(test_url)
        );
        self.delay_value(&path).await
    }

    async fn delay_value(&self, path: &str) -> Result<i32, ClashError> {
        let value: Value = self.get_json(path).await?;
        // Upstream: `jsonObject?["delay"]` must be a JSON number, else -1.
        Ok(value
            .get("delay")
            .and_then(Value::as_i64)
            .and_then(|delay| i32::try_from(delay).ok())
            .unwrap_or(-1))
    }

    /// `GET /connections`.
    pub async fn get_connections(&self) -> Result<ClashConnections, ClashError> {
        self.get_json("/connections").await
    }

    /// `DELETE /connections/{id}`.
    pub async fn close_connection(&self, id: &str) -> Result<(), ClashError> {
        self.delete(&format!("/connections/{}", urlencoding::encode(id)))
            .await
    }

    /// `DELETE /connections`.
    pub async fn close_all_connections(&self) -> Result<(), ClashError> {
        self.delete("/connections").await
    }

    /// `GET /configs` as a raw JSON object (mode list/mode live here).
    pub async fn get_config(&self) -> Result<Value, ClashError> {
        let body = self.get_text("/configs").await?;
        serde_json::from_str(&body)
            .map_err(|err| ClashError::Http(HttpError::Decode(err.to_string())))
    }
}
