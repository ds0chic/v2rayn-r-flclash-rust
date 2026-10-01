//! Statistics adapters for the two kernels (upstream `StatisticsManager`).
//!
//! * Xray exposes cumulative per-outbound byte counters over HTTP
//!   `GET http://127.0.0.1:{StatePort}/debug/vars`
//!   (`StatisticsXrayService`, `V2rayMetricsVars`).
//! * sing-box pushes per-second byte *deltas* over the WebSocket
//!   `ws://127.0.0.1:{StatePort2}/traffic`
//!   (`StatisticsSingboxService`, `TrafficItem`).
//!
//! Both sources normalise to cumulative [`CounterSample`]s; the delta/rate
//! maths lives in [`StatsAggregator`] so a single UI path can consume either
//! kernel.

mod aggregator;
mod singbox;
mod xray;

pub use aggregator::{StatsAggregator, TagStat};
pub use singbox::{parse_traffic, SingboxTrafficSource, TrafficConfig};
pub use xray::{
    parse_xray_vars, to_display_units, XrayStatsSource, DIRECT_TAG, LINK_BASE, PROXY_TAG,
};

use futures_util::future::BoxFuture;

/// Cumulative bytes observed for one tag since the core process started.
///
/// `up`/`down` are raw byte counters (the units upstream divides by 1024 for
/// Xray and 1000 for sing-box are left to the presentation layer). `u64` keeps
/// the counters wide enough for multi-terabyte sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CounterSample {
    pub tag: String,
    pub up: u64,
    pub down: u64,
}

/// How an outbound tag maps onto the upstream proxy/direct buckets
/// (`StatisticsXrayService.ParseOutput`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagClass {
    /// `key.StartsWith(Global.ProxyTag)` where `ProxyTag == "proxy"`.
    Proxy,
    /// `key == Global.DirectTag` where `DirectTag == "direct"`.
    Direct,
    /// Any other tag up- or inbound.
    Other,
}

/// Classify a tag exactly like `StatisticsXrayService`.
pub fn classify_tag(tag: &str) -> TagClass {
    if tag.starts_with(PROXY_TAG) {
        TagClass::Proxy
    } else if tag == DIRECT_TAG {
        TagClass::Direct
    } else {
        TagClass::Other
    }
}

/// A snapshot source of cumulative counters.
///
/// Implementations are `Send`; `poll` is object-safe (returns a boxed future)
/// so the runtime can hold `Box<dyn StatsSource>` and swap Xray/sing-box behind
/// a single generation counter.
pub trait StatsSource: Send {
    /// Fetch the current cumulative counters. Empty when the core has not
    /// produced any traffic yet.
    fn poll(&mut self) -> BoxFuture<'_, Result<Vec<CounterSample>, StatsError>>;

    /// Monotonic generation. It increments whenever the source notices that the
    /// underlying counters were rebased (core restart), which tells the
    /// aggregator to drop its baseline instead of computing a huge negative
    /// delta.
    fn generation(&self) -> u64;
}

/// Statistics adapter failure.
#[derive(Debug, thiserror::Error)]
pub enum StatsError {
    #[error(transparent)]
    Http(#[from] crate::error::HttpError),
    #[error("websocket error: {0}")]
    WebSocket(String),
    #[error("statistics payload could not be decoded: {0}")]
    Decode(String),
    #[error("statistics source was cancelled")]
    Cancelled,
}

impl From<serde_json::Error> for StatsError {
    fn from(err: serde_json::Error) -> Self {
        StatsError::Decode(err.to_string())
    }
}
