//! Xray HTTP stats adapter (`StatisticsXrayService`).

use std::collections::HashMap;
use std::time::Duration;

use futures_util::future::BoxFuture;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;

use super::{CounterSample, StatsError, StatsSource};
use crate::error::{classify_reqwest, HttpError};

/// `Global.ProxyTag`.
pub const PROXY_TAG: &str = "proxy";
/// `Global.DirectTag`.
pub const DIRECT_TAG: &str = "direct";
/// `StatisticsXrayService.linkBase` (bytes -> the unit upstream feeds the UI).
pub const LINK_BASE: u64 = 1024;

/// Upstream scaling: `state.uplink / linkBase`. The raw `u64` counters stay
/// bytes; callers that want the v2rayN display unit call this.
pub fn to_display_units(bytes: u64) -> u64 {
    bytes / LINK_BASE
}

#[derive(Debug, Default, Deserialize)]
struct XrayVars {
    #[serde(default)]
    stats: Option<XrayStats>,
}

#[derive(Debug, Default, Deserialize)]
struct XrayStats {
    #[serde(default)]
    outbound: HashMap<String, XrayLink>,
}

#[derive(Debug, Default, Deserialize)]
struct XrayLink {
    #[serde(default)]
    downlink: i64,
    #[serde(default)]
    uplink: i64,
}

fn to_u64(value: i64) -> u64 {
    value.max(0) as u64
}

/// Parse the `/debug/vars` payload into cumulative per-outbound samples.
///
/// Mirrors `StatisticsXrayService.ParseOutput`, which reads
/// `stats.outbound[key].{uplink,downlink}` for every key and never fails (it
/// returns `null` on error). Here a missing `stats`/`stats.outbound` is an
/// empty snapshot rather than an error, matching "core up but idle"; genuinely
/// malformed JSON is a [`StatsError::Decode`].
pub fn parse_xray_vars(text: &str) -> Result<Vec<CounterSample>, StatsError> {
    let parsed: XrayVars = serde_json::from_str(text)?;
    let Some(stats) = parsed.stats else {
        return Ok(Vec::new());
    };
    // Deterministic order for tests and UI.
    let mut samples: Vec<CounterSample> = stats
        .outbound
        .into_iter()
        .map(|(tag, link)| CounterSample {
            tag,
            up: to_u64(link.uplink),
            down: to_u64(link.downlink),
        })
        .collect();
    samples.sort_by(|a, b| a.tag.cmp(&b.tag));
    Ok(samples)
}

/// HTTP source for Xray's `debug/vars`.
pub struct XrayStatsSource {
    client: reqwest::Client,
    url: String,
    timeout: Duration,
    authorization: Option<String>,
    generation: u64,
    last: HashMap<String, (u64, u64)>,
}

impl XrayStatsSource {
    /// Build a source for `http://127.0.0.1:{port}/debug/vars`.
    pub fn new(port: u16, timeout: Duration) -> Result<Self, StatsError> {
        Self::from_url(format!("http://127.0.0.1:{port}/debug/vars"), timeout)
    }

    /// Build from an explicit URL (mock servers in tests, non-default hosts).
    pub fn from_url(url: impl Into<String>, timeout: Duration) -> Result<Self, StatsError> {
        // `no_proxy` keeps loopback traffic off whatever proxy the host
        // environment configures (the user's live 10808 path must never be
        // touched by an observability poll).
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|err| StatsError::Http(classify_reqwest(&err)))?;
        Ok(Self {
            client,
            url: url.into(),
            timeout,
            authorization: None,
            generation: 0,
            last: HashMap::new(),
        })
    }

    /// Attach the Clash/Xray `Authorization` header. Xray's metrics endpoint is
    /// unauthenticated, but DPI/edge setups sometimes require it and the API
    /// stays uniform with [`crate::clash_api::ClashApiClient`].
    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.authorization = Some(format!("Bearer {}", secret.into()));
        self
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Detect a counter rebase (process restart) and bump the generation.
    fn observe(&mut self, samples: Vec<CounterSample>) -> Vec<CounterSample> {
        let rebased = samples.iter().any(|sample| {
            self.last
                .get(&sample.tag)
                .is_some_and(|(up, down)| sample.up < *up || sample.down < *down)
        });
        if rebased {
            self.generation = self.generation.wrapping_add(1);
            self.last.clear();
        }
        for sample in &samples {
            self.last
                .insert(sample.tag.clone(), (sample.up, sample.down));
        }
        samples
    }
}

impl StatsSource for XrayStatsSource {
    fn poll(&mut self) -> BoxFuture<'_, Result<Vec<CounterSample>, StatsError>> {
        Box::pin(async move {
            let mut request = self.client.get(&self.url).timeout(self.timeout);
            if let Some(value) = &self.authorization {
                request = request.header(AUTHORIZATION, value);
            }
            let response = request
                .send()
                .await
                .map_err(|err| StatsError::Http(classify_reqwest(&err)))?;
            let status = response.status();
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(StatsError::Http(HttpError::Unauthorized));
            }
            let body = response
                .text()
                .await
                .map_err(|err| StatsError::Http(classify_reqwest(&err)))?;
            if !status.is_success() {
                return Err(StatsError::Http(HttpError::Status(status.as_u16())));
            }
            let samples = parse_xray_vars(&body)?;
            Ok(self.observe(samples))
        })
    }

    fn generation(&self) -> u64 {
        self.generation
    }
}
