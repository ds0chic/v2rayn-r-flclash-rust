//! T15a monitor pipeline: statistics, log stream and Clash API wiring.
//!
//! This module turns the low-level [`core_adapters`] primitives into the three
//! application services the UI consumes:
//!
//! * [`StatsService`] — accumulates raw byte deltas (never lost to the 1 Hz
//!   emit budget), classifies proxy/direct, tracks per-node today/total
//!   `ServerStatItem` rows, and keeps a fresh baseline when a core restart
//!   bumps the source generation.
//! * [`LogService`] — ring-buffered log lines with level/keyword filtering,
//!   separate collect/scroll pause, and a protected control-event queue.
//! * [`ClashApiService`] — read/observe/select/close against the sing-box or
//!   mihomo Clash controller.
//!
//! No process or socket is owned here: the runtime facts (core type, StatePort,
//! StatePort2, secret) are supplied by the caller. Network calls only happen
//! when an explicit async method is invoked.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use core_adapters::clash_api::{
    ClashApiClient, ClashConnections, ClashError, ClashItem, ClashProxy, DEFAULT_DELAY_TIMEOUT_MS,
};
use core_adapters::log_stream::{LogBuffer, LogFilter, LogLevel, LogLine};
use core_adapters::stats::{classify_tag, CounterSample, StatsAggregator, TagClass};
use domain::event::{EventEnvelope, EventKind};
use domain::{CoreType, DomainError, TrafficStats};

/// Default Clash delay-probe URL (upstream `ClashApiManager` default).
pub const DELAY_TEST_URL: &str = "https://www.gstatic.com/generate_204";

/// Seconds in one day, for the `DateNow` epoch-day bucket.
pub const SECONDS_PER_DAY: i64 = 86_400;

/// Epoch-day bucket for a Unix timestamp.
pub fn epoch_day(unix_seconds: i64) -> i64 {
    unix_seconds.div_euclid(SECONDS_PER_DAY)
}

/// The upstream monitor pipeline data never crosses 100 MiB/s in display
/// terms; the ring keeps bytes as `usize` and the UI formats them.
pub const DEFAULT_MAX_LOG_LINES: usize = 10_000;
pub const DEFAULT_MAX_LOG_BYTES: usize = 10 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Per-node statistics persistence
// ---------------------------------------------------------------------------

/// Persistence boundary for `ServerStatItem` rows (keyed by node `IndexId`).
pub trait TrafficStore: Send {
    fn load(&self) -> Result<Vec<TrafficStats>, DomainError>;
    fn upsert(&mut self, stat: &TrafficStats) -> Result<(), DomainError>;
    fn remove(&mut self, index_id: &str) -> Result<(), DomainError>;
    fn clear(&mut self) -> Result<(), DomainError>;
}

/// In-memory store used by tests and by the bridge when no data dir is set.
#[derive(Default)]
pub struct InMemoryTrafficStore {
    rows: BTreeMap<String, TrafficStats>,
}

impl InMemoryTrafficStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl TrafficStore for InMemoryTrafficStore {
    fn load(&self) -> Result<Vec<TrafficStats>, DomainError> {
        Ok(self.rows.values().cloned().collect())
    }

    fn upsert(&mut self, stat: &TrafficStats) -> Result<(), DomainError> {
        self.rows.insert(stat.index_id.clone(), stat.clone());
        Ok(())
    }

    fn remove(&mut self, index_id: &str) -> Result<(), DomainError> {
        self.rows.remove(index_id);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), DomainError> {
        self.rows.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Cumulative proxy/direct byte counters for the current core generation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BucketTotals {
    pub up: u64,
    pub down: u64,
}

/// Result of one [`StatsService::apply`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsUpdate {
    /// Whether the 1 Hz throttle let this snapshot through to the UI.
    pub applied: bool,
    pub generation: u64,
    pub session_proxy: BucketTotals,
    pub session_direct: BucketTotals,
    pub proxy_bps: BucketTotals,
    pub direct_bps: BucketTotals,
}

/// Statistics aggregator with per-node accounting.
pub struct StatsService {
    aggregator: StatsAggregator,
    generation: u64,
    last: HashMap<String, (u64, u64)>,
    session_proxy: BucketTotals,
    session_direct: BucketTotals,
    proxy_bps: BucketTotals,
    direct_bps: BucketTotals,
    nodes: BTreeMap<String, TrafficStats>,
    date_now: i64,
    active_index_id: Option<String>,
    enabled: bool,
    display_speed: bool,
    store: Box<dyn TrafficStore>,
}

impl StatsService {
    /// Build a service. `store` is loaded lazily by [`Self::load`].
    pub fn new(store: Box<dyn TrafficStore>, enabled: bool, display_speed: bool) -> Self {
        Self {
            aggregator: StatsAggregator::one_hz(),
            generation: 0,
            last: HashMap::new(),
            session_proxy: BucketTotals::default(),
            session_direct: BucketTotals::default(),
            proxy_bps: BucketTotals::default(),
            direct_bps: BucketTotals::default(),
            nodes: BTreeMap::new(),
            date_now: 0,
            active_index_id: None,
            enabled,
            display_speed,
            store,
        }
    }

    /// Whether collection is active at all (`EnableStatistics || DisplayRealTimeSpeed`).
    pub fn active(&self) -> bool {
        self.enabled || self.display_speed
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn display_speed(&self) -> bool {
        self.display_speed
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_display_speed(&mut self, display: bool) {
        self.display_speed = display;
    }

    pub fn set_active_index(&mut self, index_id: Option<String>) {
        self.active_index_id = index_id;
    }

    pub fn active_index(&self) -> Option<&str> {
        self.active_index_id.as_deref()
    }

    /// Replace the persistence boundary. Used when the bridge binds the real
    /// SQLite `ServerStatItem` store for a persistent data directory; the
    /// caller is responsible for a follow-up [`Self::load`].
    pub fn set_store(&mut self, store: Box<dyn TrafficStore>) {
        self.store = store;
    }

    /// Load persisted `ServerStatItem` rows into memory.
    pub fn load(&mut self) -> Result<(), DomainError> {
        self.nodes.clear();
        for stat in self.store.load()? {
            self.date_now = self.date_now.max(stat.date_now);
            self.nodes.insert(stat.index_id.clone(), stat);
        }
        Ok(())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TrafficStats> {
        self.nodes.values()
    }

    pub fn node(&self, index_id: &str) -> Option<&TrafficStats> {
        self.nodes.get(index_id)
    }

    pub fn session_proxy(&self) -> BucketTotals {
        self.session_proxy
    }

    pub fn session_direct(&self) -> BucketTotals {
        self.session_direct
    }

    pub fn rates(&self) -> (BucketTotals, BucketTotals) {
        (self.proxy_bps, self.direct_bps)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Ingest a snapshot.
    ///
    /// Raw deltas are accumulated on **every** call so throttling never loses
    /// bytes; only the rate/emit decision is 1 Hz. A generation change resets
    /// the session counters but keeps the persisted per-node totals. `now` and
    /// `today_epoch` are injected for deterministic tests.
    pub fn apply(
        &mut self,
        samples: &[CounterSample],
        generation: u64,
        today_epoch: i64,
        now: Instant,
    ) -> StatsUpdate {
        if !self.active() {
            return StatsUpdate {
                applied: false,
                generation: self.generation,
                session_proxy: self.session_proxy,
                session_direct: self.session_direct,
                proxy_bps: self.proxy_bps,
                direct_bps: self.direct_bps,
            };
        }
        if generation != self.generation {
            self.generation = generation;
            self.last.clear();
            self.session_proxy = BucketTotals::default();
            self.session_direct = BucketTotals::default();
            self.proxy_bps = BucketTotals::default();
            self.direct_bps = BucketTotals::default();
            self.aggregator = StatsAggregator::one_hz();
        }
        if today_epoch != self.date_now {
            // Cross-day: today resets, cumulative totals carry over.
            for node in self.nodes.values_mut() {
                node.today_up = 0;
                node.today_down = 0;
                node.date_now = today_epoch;
            }
            self.date_now = today_epoch;
        }

        let mut proxy_delta = BucketTotals::default();
        let mut direct_delta = BucketTotals::default();
        for sample in samples {
            let previous = self
                .last
                .insert(sample.tag.clone(), (sample.up, sample.down));
            let Some((prev_up, prev_down)) = previous else {
                continue;
            };
            let delta = BucketTotals {
                up: sample.up.saturating_sub(prev_up),
                down: sample.down.saturating_sub(prev_down),
            };
            match classify_tag(&sample.tag) {
                TagClass::Proxy => {
                    proxy_delta.up = proxy_delta.up.saturating_add(delta.up);
                    proxy_delta.down = proxy_delta.down.saturating_add(delta.down);
                }
                TagClass::Direct => {
                    direct_delta.up = direct_delta.up.saturating_add(delta.up);
                    direct_delta.down = direct_delta.down.saturating_add(delta.down);
                }
                TagClass::Other => {}
            }
        }
        self.session_proxy.up = self.session_proxy.up.saturating_add(proxy_delta.up);
        self.session_proxy.down = self.session_proxy.down.saturating_add(proxy_delta.down);
        self.session_direct.up = self.session_direct.up.saturating_add(direct_delta.up);
        self.session_direct.down = self.session_direct.down.saturating_add(direct_delta.down);

        if proxy_delta != BucketTotals::default() {
            if let Some(id) = self.active_index_id.clone() {
                let node = self
                    .nodes
                    .entry(id.clone())
                    .or_insert_with(|| TrafficStats {
                        index_id: id,
                        date_now: today_epoch,
                        ..Default::default()
                    });
                node.today_up = node.today_up.saturating_add(bump(proxy_delta.up));
                node.today_down = node.today_down.saturating_add(bump(proxy_delta.down));
                node.total_up = node.total_up.saturating_add(bump(proxy_delta.up));
                node.total_down = node.total_down.saturating_add(bump(proxy_delta.down));
                node.date_now = today_epoch;
            }
        }

        let applied = self.aggregator.apply(samples, generation, now);
        if applied {
            let (mut pu, mut pd, mut du, mut dd) = (0u64, 0u64, 0u64, 0u64);
            for stat in self.aggregator.stats().values() {
                match classify_tag(&stat.tag) {
                    TagClass::Proxy => {
                        pu = pu.saturating_add(stat.up_bps);
                        pd = pd.saturating_add(stat.down_bps);
                    }
                    TagClass::Direct => {
                        du = du.saturating_add(stat.up_bps);
                        dd = dd.saturating_add(stat.down_bps);
                    }
                    TagClass::Other => {}
                }
            }
            self.proxy_bps = BucketTotals { up: pu, down: pd };
            self.direct_bps = BucketTotals { up: du, down: dd };
        }

        StatsUpdate {
            applied,
            generation: self.generation,
            session_proxy: self.session_proxy,
            session_direct: self.session_direct,
            proxy_bps: self.proxy_bps,
            direct_bps: self.direct_bps,
        }
    }

    /// Persist every in-memory node row. Called after a successful apply so the
    /// store mirrors memory.
    pub fn flush_store(&mut self) -> Result<(), DomainError> {
        let rows: Vec<TrafficStats> = self.nodes.values().cloned().collect();
        for row in rows {
            self.store.upsert(&row)?;
        }
        Ok(())
    }

    /// `ClearAllServerStatistics`: remove every row and persist the clear.
    pub fn clear_all(&mut self) -> Result<(), DomainError> {
        self.nodes.clear();
        self.session_proxy = BucketTotals::default();
        self.session_direct = BucketTotals::default();
        self.store.clear()
    }

    /// Clear one node's counters.
    pub fn clear_node(&mut self, index_id: &str) -> Result<(), DomainError> {
        self.nodes.remove(index_id);
        self.store.remove(index_id)
    }
}

fn bump(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

/// One UI-facing log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub text: String,
    pub level: LogLevel,
    pub truncated: bool,
    pub control: bool,
}

/// A page of log lines plus the overflow counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogPage {
    pub entries: Vec<LogEntry>,
    pub total: usize,
    pub dropped_lines: u64,
    pub dropped_bytes: u64,
    pub truncated_lines: u64,
    pub control_lines: usize,
    pub collecting_paused: bool,
    pub scroll_paused: bool,
}

/// Log stream consumer with ring cap, filtering and pause semantics.
pub struct LogService {
    buffer: LogBuffer,
    filter: LogFilter,
    max_lines: usize,
    max_bytes: usize,
    collecting_paused: bool,
    scroll_paused: bool,
    accepted: u64,
    rejected: u64,
    truncated: u64,
}

impl LogService {
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            buffer: LogBuffer::new(max_lines, max_bytes),
            filter: LogFilter::new(),
            max_lines,
            max_bytes,
            collecting_paused: false,
            scroll_paused: false,
            accepted: 0,
            rejected: 0,
            truncated: 0,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_MAX_LOG_LINES, DEFAULT_MAX_LOG_BYTES)
    }

    pub fn is_collecting_paused(&self) -> bool {
        self.collecting_paused
    }

    pub fn is_scroll_paused(&self) -> bool {
        self.scroll_paused
    }

    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    pub fn rejected(&self) -> u64 {
        self.rejected
    }

    pub fn truncated(&self) -> u64 {
        self.truncated
    }

    /// Replace the active filter.
    pub fn set_filter(
        &mut self,
        min_level: Option<LogLevel>,
        include: Vec<String>,
        exclude: Vec<String>,
    ) {
        self.filter = LogFilter {
            min_level,
            include,
            exclude,
        };
    }

    /// Pause/resume collection (lines are dropped while paused, matching
    /// upstream "暂停采集") independently of the scroll pause.
    pub fn set_collecting_paused(&mut self, paused: bool) {
        self.collecting_paused = paused;
    }

    /// Pause/resume UI auto-scroll only; collection continues.
    pub fn set_scroll_paused(&mut self, paused: bool) {
        self.scroll_paused = paused;
    }

    /// Ingest one decoded line.
    pub fn ingest_line(&mut self, line: LogLine) {
        if self.collecting_paused {
            return;
        }
        if line.truncated {
            self.truncated = self.truncated.saturating_add(1);
        }
        if self.filter.accept(&line) {
            self.accepted = self.accepted.saturating_add(1);
            self.buffer.push_line(line);
        } else {
            self.rejected = self.rejected.saturating_add(1);
        }
    }

    /// Ingest a raw text line (application log).
    pub fn ingest_text(&mut self, text: impl Into<String>) {
        self.ingest_line(LogLine::new(text));
    }

    /// Ingest a run-event envelope (core/application logs + control markers).
    ///
    /// `log_batch` payloads carry `entries: [{text, truncated?}]` or
    /// `lines: [string]`; the `log_line` named event carries `{text}`. Control
    /// event kinds are recorded on the protected control queue.
    pub fn ingest_envelope(&mut self, event: &EventEnvelope) {
        match &event.kind {
            EventKind::LogBatch => self.ingest_log_payload(&event.payload),
            EventKind::Other(name) if name == "log_line" => {
                if let Some(text) = event.payload.get("text").and_then(|v| v.as_str()) {
                    self.ingest_text(text);
                }
            }
            EventKind::ErrorRaised => {
                if let Some(detail) = event.payload.get("detail").and_then(|v| v.as_str()) {
                    self.buffer
                        .push_control(core_adapters::log_stream::ControlEvent::StreamError(
                            detail.to_string(),
                        ));
                }
            }
            _ => {}
        }
    }

    fn ingest_log_payload(&mut self, payload: &serde_json::Value) {
        if let Some(entries) = payload.get("entries").and_then(|v| v.as_array()) {
            for entry in entries {
                let text = entry
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let truncated = entry
                    .get("truncated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.ingest_line(LogLine {
                    text: text.to_string(),
                    truncated,
                });
            }
            return;
        }
        if let Some(lines) = payload.get("lines").and_then(|v| v.as_array()) {
            for line in lines {
                if let Some(text) = line.as_str() {
                    self.ingest_text(text);
                }
            }
        }
    }

    /// Snapshot a page of lines plus counters/counters. `offset` counts from
    /// the oldest retained line.
    pub fn snapshot(&self, offset: usize, limit: usize) -> LogPage {
        let total = self.buffer.lines.len();
        let entries: Vec<LogEntry> = self
            .buffer
            .lines
            .iter()
            .skip(offset)
            .take(limit)
            .map(|line| LogEntry {
                text: line.text.clone(),
                level: line.level(),
                truncated: line.truncated,
                control: false,
            })
            .collect();
        LogPage {
            entries,
            total,
            dropped_lines: self.buffer.lines.dropped_lines(),
            dropped_bytes: self.buffer.lines.dropped_bytes(),
            truncated_lines: self.truncated,
            control_lines: self.buffer.control_len(),
            collecting_paused: self.collecting_paused,
            scroll_paused: self.scroll_paused,
        }
    }

    /// Clear the visible lines while keeping the protected control queue.
    pub fn clear(&mut self) {
        self.buffer.lines =
            core_adapters::log_stream::RingBuffer::new(self.max_lines, self.max_bytes);
        self.accepted = 0;
        self.rejected = 0;
        self.truncated = 0;
    }

    pub fn dropped_lines(&self) -> u64 {
        self.buffer.lines.dropped_lines()
    }

    pub fn dropped_bytes(&self) -> u64 {
        self.buffer.lines.dropped_bytes()
    }
}

// ---------------------------------------------------------------------------
// Clash API
// ---------------------------------------------------------------------------

/// Read/observe/select client wrapper for the sing-box/mihomo controller.
pub struct ClashApiService {
    client: ClashApiClient,
    refresh: Duration,
    visible: bool,
}

impl ClashApiService {
    pub fn new(
        port: u16,
        secret: Option<String>,
        timeout: Duration,
        refresh: Duration,
    ) -> Result<Self, ClashError> {
        Ok(Self {
            client: ClashApiClient::new(port, secret, timeout)?,
            refresh,
            visible: false,
        })
    }

    pub fn from_client(client: ClashApiClient, refresh: Duration) -> Self {
        Self {
            client,
            refresh,
            visible: false,
        }
    }

    /// Clash API is only served by mihomo and sing-box (upstream
    /// `ClashApiManager`); Xray and the rest report "not applicable".
    pub fn supported(core: CoreType) -> bool {
        matches!(core, CoreType::Mihomo | CoreType::SingBox)
    }

    pub fn refresh_interval(&self) -> Duration {
        self.refresh
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub async fn proxies(&self) -> Result<ClashItem, ClashError> {
        self.client.get_proxies().await
    }

    pub async fn proxy(&self, name: &str) -> Result<ClashProxy, ClashError> {
        self.client.get_proxy(name).await
    }

    pub async fn select(&self, group: &str, name: &str) -> Result<(), ClashError> {
        self.client.select_proxy(group, name).await
    }

    /// Single-node delay. `-1` means timeout/unreachable, as upstream.
    pub async fn proxy_delay(&self, name: &str) -> i32 {
        self.client
            .get_proxy_delay(name, DEFAULT_DELAY_TIMEOUT_MS, DELAY_TEST_URL)
            .await
    }

    /// Group delay: probe every child of a selector/url-test group, using the
    /// provider healthcheck endpoint when the child came from a provider.
    pub async fn group_delay(&self, group: &str) -> Result<Vec<(String, i32)>, ClashError> {
        let item = self.client.get_proxies().await?;
        let Some(proxy) = item.proxies.get(group) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for name in proxy.all.clone().unwrap_or_default() {
            let provider = item.provider_index_map.get(&name).cloned();
            let delay = match provider {
                Some(provider) => self
                    .client
                    .try_get_provider_proxy_delay(
                        &provider,
                        &name,
                        DEFAULT_DELAY_TIMEOUT_MS,
                        DELAY_TEST_URL,
                    )
                    .await
                    .unwrap_or(-1),
                None => self.proxy_delay(&name).await,
            };
            out.push((name, delay));
        }
        Ok(out)
    }

    pub async fn connections(&self) -> Result<ClashConnections, ClashError> {
        self.client.get_connections().await
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), ClashError> {
        self.client.close_connection(id).await
    }

    pub async fn close_all(&self) -> Result<(), ClashError> {
        self.client.close_all_connections().await
    }

    /// Test/access helper: the wrapped client.
    pub fn client(&self) -> &ClashApiClient {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_adapters::log_stream::RingBuffer;

    fn sample(tag: &str, up: u64, down: u64) -> CounterSample {
        CounterSample {
            tag: tag.to_string(),
            up,
            down,
        }
    }

    #[test]
    fn generation_change_resets_session_but_keeps_nodes() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
        service.set_active_index(Some("n1".into()));
        let now = Instant::now();
        // Establish baseline then add bytes.
        service.apply(&[sample("proxy", 100, 100)], 0, 1, now);
        service.apply(
            &[sample("proxy", 300, 300)],
            0,
            1,
            now + Duration::from_secs(2),
        );
        assert_eq!(service.session_proxy().up, 200);
        assert_eq!(service.node("n1").unwrap().total_up, 200);

        // Core restart bumps the generation: session resets, node total stays.
        service.apply(
            &[sample("proxy", 10, 10)],
            1,
            1,
            now + Duration::from_secs(4),
        );
        assert_eq!(service.generation(), 1);
        assert_eq!(service.session_proxy().up, 0);
        assert_eq!(service.node("n1").unwrap().total_up, 200);
    }

    #[test]
    fn throttle_does_not_lose_cumulative_bytes() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
        service.set_active_index(Some("n1".into()));
        let now = Instant::now();
        service.apply(&[sample("proxy", 0, 0)], 0, 1, now);
        // Second call is throttled (same instant) but its delta is still counted.
        let update = service.apply(&[sample("proxy", 500, 700)], 0, 1, now);
        assert!(!update.applied);
        assert_eq!(update.session_proxy.up, 500);
        assert_eq!(update.session_proxy.down, 700);
        // A later due call applies again and updates rates.
        let next = service.apply(
            &[sample("proxy", 900, 1200)],
            0,
            1,
            now + Duration::from_secs(2),
        );
        assert!(next.applied);
        assert_eq!(next.session_proxy.up, 900);
        assert_eq!(next.session_proxy.down, 1200);
        assert!(next.proxy_bps.down > 0);
    }

    #[test]
    fn classifies_proxy_and_direct_buckets() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, false);
        let now = Instant::now();
        service.apply(&[sample("proxy", 0, 0), sample("direct", 0, 0)], 0, 1, now);
        service.apply(
            &[sample("proxy", 10, 20), sample("direct", 5, 6)],
            0,
            1,
            now + Duration::from_secs(2),
        );
        assert_eq!(service.session_proxy(), BucketTotals { up: 10, down: 20 });
        assert_eq!(service.session_direct(), BucketTotals { up: 5, down: 6 });
        // `nginx` is neither a proxy nor direct outbound.
        service.apply(&[sample("nginx", 1, 1)], 0, 1, now + Duration::from_secs(4));
        assert_eq!(service.session_proxy(), BucketTotals { up: 10, down: 20 });
    }

    #[test]
    fn today_rollover_resets_today_but_keeps_total() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
        service.set_active_index(Some("n1".into()));
        let now = Instant::now();
        service.apply(&[sample("proxy", 0, 0)], 0, 100, now);
        service.apply(
            &[sample("proxy", 100, 200)],
            0,
            100,
            now + Duration::from_secs(2),
        );
        assert_eq!(service.node("n1").unwrap().today_up, 100);
        assert_eq!(service.node("n1").unwrap().total_up, 100);

        // Next day bucket: today resets before the new delta lands.
        service.apply(
            &[sample("proxy", 150, 250)],
            0,
            101,
            now + Duration::from_secs(4),
        );
        let node = service.node("n1").unwrap();
        assert_eq!(node.date_now, 101);
        assert_eq!(node.today_up, 50);
        assert_eq!(node.total_up, 150);
    }

    #[test]
    fn clear_all_empties_memory_and_store() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
        service.set_active_index(Some("n1".into()));
        let now = Instant::now();
        service.apply(&[sample("proxy", 0, 0)], 0, 1, now);
        service.apply(
            &[sample("proxy", 10, 10)],
            0,
            1,
            now + Duration::from_secs(2),
        );
        service.flush_store().unwrap();
        assert_eq!(service.nodes().count(), 1);
        service.clear_all().unwrap();
        assert_eq!(service.nodes().count(), 0);
        assert_eq!(service.store.load().unwrap().len(), 0);
    }

    #[test]
    fn bound_sqlite_store_persists_and_reloads_server_stat_items() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("guiNDB.db");
        let now = Instant::now();

        let mut service = StatsService::new(
            Box::new(crate::store_repo::SqliteTrafficStore::from_store(
                persistence::Store::open(&db).unwrap(),
            )),
            true,
            true,
        );
        service.set_active_index(Some("n1".into()));
        service.apply(&[sample("proxy", 0, 0)], 0, 100, now);
        service.apply(
            &[sample("proxy", 100, 200)],
            0,
            100,
            now + Duration::from_secs(2),
        );
        service.flush_store().unwrap();
        assert_eq!(service.node("n1").unwrap().today_up, 100);

        // A fresh service over the same database (process reopen) reloads it.
        let mut reopened = StatsService::new(
            Box::new(crate::store_repo::SqliteTrafficStore::from_store(
                persistence::Store::open(&db).unwrap(),
            )),
            true,
            true,
        );
        reopened.load().unwrap();
        let row = reopened.node("n1").expect("persisted row");
        assert_eq!(row.total_up, 100);
        assert_eq!(row.today_up, 100);
        assert_eq!(row.date_now, 100);

        // Clear all persists the deletion.
        reopened.clear_all().unwrap();
        let mut third = StatsService::new(
            Box::new(crate::store_repo::SqliteTrafficStore::from_store(
                persistence::Store::open(&db).unwrap(),
            )),
            true,
            true,
        );
        third.load().unwrap();
        assert!(third.node("n1").is_none());
    }

    #[test]
    fn disabled_service_ignores_samples() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), false, false);
        let update = service.apply(&[sample("proxy", 9, 9)], 0, 1, Instant::now());
        assert!(!update.applied);
        assert_eq!(update.session_proxy, BucketTotals::default());
    }

    #[test]
    fn log_ring_drops_oldest_and_counts_overflow() {
        let mut service = LogService::new(3, 0);
        for i in 0..5 {
            service.ingest_text(format!("line {i}"));
        }
        let page = service.snapshot(0, 10);
        assert_eq!(page.total, 3);
        assert_eq!(page.dropped_lines, 2);
        assert_eq!(page.entries[0].text, "line 2");
    }

    #[test]
    fn log_filter_and_pause() {
        let mut service = LogService::new(100, 0);
        service.set_filter(Some(LogLevel::Warn), vec![], vec!["noise".into()]);
        service.ingest_text("info hello");
        service.ingest_text("warn keep");
        service.ingest_text("error keep noise");
        service.ingest_text("fatal boom");
        let page = service.snapshot(0, 10);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(service.rejected(), 2);

        service.set_collecting_paused(true);
        service.ingest_text("warn dropped while paused");
        assert_eq!(service.snapshot(0, 10).total, 2);
        assert!(service.snapshot(0, 10).collecting_paused);

        service.set_scroll_paused(true);
        assert!(service.snapshot(0, 10).scroll_paused);
    }

    #[test]
    fn control_events_survive_log_flood() {
        let mut service = LogService::new(2, 0);
        service
            .buffer
            .push_control(core_adapters::log_stream::ControlEvent::ProcessExit(0));
        for i in 0..50 {
            service.ingest_text(format!("flood {i}"));
        }
        assert_eq!(service.buffer.control_len(), 1);
        assert_eq!(service.snapshot(0, 10).total, 2);
    }

    #[test]
    fn log_batch_envelope_ingests_entries() {
        let mut service = LogService::new(100, 0);
        let event = EventEnvelope::new(
            domain::event::EventEpoch(1),
            domain::event::EventSeq(1),
            EventKind::LogBatch,
            serde_json::json!({
                "entries": [
                    {"text": "[Info] core started"},
                    {"text": "[Error] boom", "truncated": true}
                ]
            }),
        );
        service.ingest_envelope(&event);
        let page = service.snapshot(0, 10);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.truncated_lines, 1);
        assert_eq!(service.accepted(), 2);
    }

    #[test]
    fn clash_support_is_mihomo_singbox_only() {
        assert!(ClashApiService::supported(CoreType::Mihomo));
        assert!(ClashApiService::supported(CoreType::SingBox));
        assert!(!ClashApiService::supported(CoreType::Xray));
        assert!(!ClashApiService::supported(CoreType::Hysteria2));
    }

    #[test]
    fn epoch_day_buckets() {
        assert_eq!(epoch_day(0), 0);
        assert_eq!(epoch_day(SECONDS_PER_DAY - 1), 0);
        assert_eq!(epoch_day(SECONDS_PER_DAY), 1);
        assert_eq!(epoch_day(-1), -1);
    }

    #[test]
    fn ring_buffer_type_is_public() {
        // Compile-time check that `LogService::clear` can rebuild a ring.
        let ring = RingBuffer::new(1, 0);
        assert!(ring.is_empty());
    }
}
