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
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use core_adapters::clash_api::{
    ClashApiClient, ClashConnections, ClashError, ClashItem, ClashProxy, DEFAULT_DELAY_TIMEOUT_MS,
};
use core_adapters::log_stream::{LogBuffer, LogFilter, LogLevel, LogLine};
use core_adapters::stats::{classify_tag, CounterSample, StatsAggregator, TagClass};
use domain::event::{EventEnvelope, EventKind};
use domain::{CoreType, DomainError, TrafficStats};

/// Default Clash delay-probe URL. Upstream `ClashApiManager.TestProxyDelay`
/// uses `Config.SpeedTestItem.SpeedPingTestUrl`, whose configured default is
/// `Global.SpeedPingTestUrls.First()` = `https://www.google.com/generate_204`.
pub const DELAY_TEST_URL: &str = "https://www.google.com/generate_204";

/// Upstream `ClashApiManager.GetProxies` retry budget: 3 attempts, 2s pause.
pub const PROXY_RETRY_ATTEMPTS: usize = 3;
pub const PROXY_RETRY_DELAY: Duration = Duration::from_secs(2);

/// SP-22 backpressure budgets: group delay probes overlap instead of running
/// serially, and one log burst is capped to its tail instead of growing the
/// ring without bound in a single call.
pub const MAX_GROUP_DELAY_CONCURRENCY: usize = 8;
pub const MAX_LOG_BATCH_LINES: usize = 2000;

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
    /// Persistence boundary shared with the poller's off-lock flush (D25). The
    /// store has its own mutex so disk I/O never runs while the monitor hub lock
    /// is held.
    store: Arc<Mutex<Box<dyn TrafficStore>>>,
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
            store: Arc::new(Mutex::new(store)),
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
        self.store = Arc::new(Mutex::new(store));
    }

    /// Load persisted `ServerStatItem` rows into memory.
    pub fn load(&mut self) -> Result<(), DomainError> {
        let loaded = self
            .store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .load()?;
        self.nodes.clear();
        for stat in loaded {
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
    /// store mirrors memory. Prefer [`persist_rows`] via [`Self::store_handle`]
    /// so the hub lock is not held across disk I/O.
    pub fn flush_store(&mut self) -> Result<(), DomainError> {
        let rows = self.snapshot_rows();
        persist_rows(&self.store, &rows)
    }

    /// A cheap in-memory copy of every per-node row, for persisting off the
    /// shared hub lock.
    pub fn snapshot_rows(&self) -> Vec<TrafficStats> {
        self.nodes.values().cloned().collect()
    }

    /// A cloneable handle to the persistence boundary so the poller can write
    /// rows after releasing the hub lock (D25). UI reads never need this handle,
    /// so a slow disk cannot block them behind the hub lock.
    pub fn store_handle(&self) -> Arc<Mutex<Box<dyn TrafficStore>>> {
        Arc::clone(&self.store)
    }

    /// `ClearAllServerStatistics`: remove every row and persist the clear.
    pub fn clear_all(&mut self) -> Result<(), DomainError> {
        self.nodes.clear();
        self.session_proxy = BucketTotals::default();
        self.session_direct = BucketTotals::default();
        self.store.lock().unwrap_or_else(|p| p.into_inner()).clear()
    }

    /// Clear one node's counters.
    pub fn clear_node(&mut self, index_id: &str) -> Result<(), DomainError> {
        self.nodes.remove(index_id);
        self.store
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(index_id)
    }
}

/// Persist rows through a [`StatsService`] store handle without holding the
/// monitor hub lock. The single poller calls this sequentially (bounded), and a
/// failure is returned so the caller can surface it and retry next tick.
pub fn persist_rows(
    handle: &Mutex<Box<dyn TrafficStore>>,
    rows: &[TrafficStats],
) -> Result<(), DomainError> {
    let mut store = handle.lock().unwrap_or_else(|p| p.into_inner());
    for row in rows {
        store.upsert(row)?;
    }
    Ok(())
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

/// Incremental by-id overlay for speedtest delays: existing entries stay,
/// probed names overwrite, new names append. Deletions never happen here
/// (a probe only reports what it measured), so no row is lost to a refresh.
pub fn merge_delay_map(existing: &mut HashMap<String, i32>, items: Vec<(String, i32)>) {
    for (name, delay) in items {
        existing.insert(name, delay);
    }
}

/// Bound one log burst to its tail. A single huge batch (10k测速日志并发)
/// keeps the newest `max` lines and reports how many head lines were shed,
/// so one burst cannot grow memory without bound in a single call. Lifecycle
/// and final-result markers travel in later batches and are never shed here.
pub fn coalesce_log_batch(lines: Vec<LogLine>, max: usize) -> (Vec<LogLine>, usize) {
    if lines.len() <= max {
        return (lines, 0);
    }
    let dropped = lines.len() - max;
    (lines.into_iter().skip(dropped).collect(), dropped)
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

    /// Ingest one burst with a per-call cap: only the tail
    /// ([`MAX_LOG_BATCH_LINES`]) enters the ring, so a single 10k burst
    /// cannot grow memory without bound. The shed head is counted as
    /// rejected (visible overflow), never silently kept.
    pub fn ingest_lines(&mut self, lines: Vec<LogLine>) {
        let (kept, shed) = coalesce_log_batch(lines, MAX_LOG_BATCH_LINES);
        self.rejected = self.rejected.saturating_add(shed as u64);
        for line in kept {
            self.ingest_line(line);
        }
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
    client: Arc<ClashApiClient>,
    refresh: Duration,
    visible: bool,
    /// Delay-probe URL; upstream defaults to the configured SpeedPing test URL.
    delay_url: String,
}

impl ClashApiService {
    pub fn new(
        port: u16,
        secret: Option<String>,
        timeout: Duration,
        refresh: Duration,
    ) -> Result<Self, ClashError> {
        Ok(Self {
            client: Arc::new(ClashApiClient::new(port, secret, timeout)?),
            refresh,
            visible: false,
            delay_url: DELAY_TEST_URL.to_string(),
        })
    }

    pub fn from_client(client: ClashApiClient, refresh: Duration) -> Self {
        Self {
            client: Arc::new(client),
            refresh,
            visible: false,
            delay_url: DELAY_TEST_URL.to_string(),
        }
    }

    /// Override the delay-probe URL (settings `SpeedPingTestUrl`).
    pub fn set_delay_url(&mut self, url: impl Into<String>) {
        let url = url.into();
        if !url.trim().is_empty() {
            self.delay_url = url;
        }
    }

    pub fn delay_url(&self) -> &str {
        &self.delay_url
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

    /// Proxies with upstream's 3× / 2s retry (`ClashApiManager.GetProxies`).
    pub async fn proxies_with_retry(&self) -> Result<ClashItem, ClashError> {
        self.client
            .get_proxies_with_retry(PROXY_RETRY_ATTEMPTS, PROXY_RETRY_DELAY)
            .await
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
            .get_proxy_delay(name, DEFAULT_DELAY_TIMEOUT_MS, &self.delay_url)
            .await
    }

    /// Group delay: probe every child of a selector/url-test group, using the
    /// provider healthcheck endpoint when the child came from a provider.
    ///
    /// Probes overlap with bounded concurrency ([`MAX_GROUP_DELAY_CONCURRENCY`])
    /// instead of one serial pass, so it does not IO-block log/connection
    /// browsing; the final vector keeps the controller order and every item
    /// is reported (failures as `-1`, as upstream), so the final result
    /// cannot be squeezed out by an early slow probe.
    pub async fn group_delay(&self, group: &str) -> Result<Vec<(String, i32)>, ClashError> {
        let mut ordered: HashMap<String, i32> = HashMap::new();
        let names = self
            .group_delay_with_progress(group, &mut |name, delay| {
                ordered.insert(name, delay);
            })
            .await?;
        let mut out = Vec::with_capacity(names.len());
        for name in names {
            let delay = ordered.remove(&name).unwrap_or(-1);
            out.push((name, delay));
        }
        Ok(out)
    }

    /// Same probes as [`Self::group_delay`], but `progress` fires per probe
    /// as it completes (incremental overlay) while the returned names keep
    /// the controller order. Dropping the returned future cancels at a safe
    /// point: in-flight probes are aborted and nothing partial is published.
    pub async fn group_delay_with_progress(
        &self,
        group: &str,
        progress: &mut (dyn FnMut(String, i32) + Send),
    ) -> Result<Vec<String>, ClashError> {
        let item = self.client.get_proxies().await?;
        let Some(proxy) = item.proxies.get(group) else {
            return Ok(Vec::new());
        };
        let names: Vec<String> = proxy.all.clone().unwrap_or_default();
        let providers: Vec<Option<String>> = names
            .iter()
            .map(|name| item.provider_index_map.get(name).cloned())
            .collect();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_GROUP_DELAY_CONCURRENCY));
        let mut set = tokio::task::JoinSet::new();
        for (name, provider) in names.iter().cloned().zip(providers) {
            let client = Arc::clone(&self.client);
            let semaphore = Arc::clone(&semaphore);
            let url = self.delay_url.clone();
            set.spawn(async move {
                let _permit = semaphore.acquire_owned().await.map_err(|_| ())?;
                let delay = match provider {
                    Some(provider) => client
                        .try_get_provider_proxy_delay(
                            &provider,
                            &name,
                            DEFAULT_DELAY_TIMEOUT_MS,
                            &url,
                        )
                        .await
                        .unwrap_or(-1),
                    None => {
                        client
                            .get_proxy_delay(&name, DEFAULT_DELAY_TIMEOUT_MS, &url)
                            .await
                    }
                };
                Ok::<(String, i32), ()>((name, delay))
            });
        }
        while let Some(finished) = set.join_next().await {
            if let Ok(Ok((name, delay))) = finished {
                progress(name, delay);
            }
        }
        Ok(names)
    }

    /// Current `mode`/`mode-list` from `/configs` (`GetClashMode`).
    pub async fn mode(&self) -> Result<Option<String>, ClashError> {
        self.client.get_mode().await
    }

    /// Selectable mode names from `/configs` (`GetClashModes`).
    pub async fn modes(&self) -> Result<Vec<String>, ClashError> {
        self.client.get_modes().await
    }

    /// Switch mode via `PATCH /configs` (`UpdateClashMode`).
    pub async fn update_mode(&self, mode: &str) -> Result<(), ClashError> {
        self.client.update_mode(mode).await
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

// ---------------------------------------------------------------------------
// SP-20 prep: connection column layout + close-target freeze (no I/O).
//
// Upstream reference (v2rayN 7.25.4 / 7d6a967, read-only `work/`):
// `ClashConnectionsView.xaml` defaults Host=300 / Chain=500 / Network=80 /
// Type=160 / ProcessPath=100 / Elapsed=100; `RestoreUI` restores by
// `ConnectionsColumnItem.OrderBy(Index)` (width applied only when > 0);
// `StorageUI` writes Name/ActualWidth/DisplayIndex back on exit.
// `ClashConnectionsViewModel` freezes `SelectedSource.Id` for a single close
// (empty id is not executable) and uses a separate empty-id path for close-all.
//
// Prep scope only: pure layout normalization over `domain::ColumnDefinition`
// rows plus close-request freeze/staleness checks. Real connection management
// (endpoint+generation binding, FRB/runtime wiring) waits on SP-17 (A08) and
// lives outside this module. No socket, process, or OS state is touched here.

/// Canonical connection columns: `(name, default width)` in upstream order.
pub const DEFAULT_CONNECTION_COLUMNS: [(&str, i32); 6] = [
    ("Host", 300),
    ("Chain", 500),
    ("Network", 80),
    ("Type", 160),
    ("ProcessPath", 100),
    ("Elapsed", 100),
];

/// Build the upstream default column rows (`Index` 0..n-1).
pub fn default_connection_columns() -> Vec<domain::ColumnDefinition> {
    DEFAULT_CONNECTION_COLUMNS
        .iter()
        .enumerate()
        .map(|(index, (name, width))| domain::ColumnDefinition {
            name: (*name).to_string(),
            width: *width,
            index: index as i32,
            ..Default::default()
        })
        .collect()
}

/// Normalize persisted `ConnectionsColumnItem` rows into visible order.
///
/// Mirrors upstream `RestoreUI`: order by `Index`, keep known names only,
/// fall back to the upstream default width when `Width <= 0`, append missing
/// defaults in canonical order, then re-number `Index` 0..n-1 for the next
/// `StorageUI` write-back.
pub fn normalize_connection_columns(
    persisted: Vec<domain::ColumnDefinition>,
) -> Vec<domain::ColumnDefinition> {
    // De-duplicate by name, keeping the smallest (`Index`, first-seen) row and
    // remembering its effective width.
    let mut ranked: HashMap<String, (i32, usize, i32)> = HashMap::new();
    for (seen, row) in persisted.into_iter().enumerate() {
        let Some((_, default_width)) = DEFAULT_CONNECTION_COLUMNS
            .iter()
            .find(|(name, _)| *name == row.name.as_str())
        else {
            continue;
        };
        let width = if row.width > 0 {
            row.width
        } else {
            *default_width
        };
        match ranked.entry(row.name.clone()) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert((row.index, seen, width));
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                if (row.index, seen) < (slot.get().0, slot.get().1) {
                    slot.insert((row.index, seen, width));
                }
            }
        }
    }
    let mut ordered: Vec<(String, i32, i32)> = ranked
        .iter()
        .map(|(name, (index, _, width))| (name.clone(), *index, *width))
        .collect();
    ordered.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mut names: Vec<String> = ordered.iter().map(|(name, _, _)| name.clone()).collect();
    for (name, _) in DEFAULT_CONNECTION_COLUMNS {
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    let widths: HashMap<String, i32> = ordered
        .into_iter()
        .map(|(name, _, width)| (name, width))
        .collect();
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let width = widths.get(name.as_str()).copied().unwrap_or_else(|| {
                DEFAULT_CONNECTION_COLUMNS
                    .iter()
                    .find(|(n, _)| *n == name.as_str())
                    .map(|(_, w)| *w)
                    .unwrap_or(0)
            });
            domain::ColumnDefinition {
                name,
                width,
                index: index as i32,
                ..Default::default()
            }
        })
        .collect()
}

/// A frozen single-close request: `id` + session `generation` captured together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCloseRequest {
    pub id: String,
    pub generation: u64,
}

/// Freeze a single-close target. Empty ids return `None` (mirrors upstream
/// `canEditRemove`); close-all uses its own path, never an empty single id.
pub fn freeze_close_request(id: &str, generation: u64) -> Option<ConnectionCloseRequest> {
    if id.is_empty() {
        return None;
    }
    Some(ConnectionCloseRequest {
        id: id.to_string(),
        generation,
    })
}

/// Only a response from the current generation may touch the live read model.
pub fn close_response_is_current(request_generation: u64, current_generation: u64) -> bool {
    request_generation == current_generation
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
        assert_eq!(
            service
                .store
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .load()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn persist_rows_writes_through_store_handle_without_the_service() {
        let mut service = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
        service.set_active_index(Some("n1".into()));
        let now = Instant::now();
        service.apply(&[sample("proxy", 0, 0)], 0, 7, now);
        service.apply(&[sample("proxy", 5, 8)], 0, 7, now + Duration::from_secs(2));
        let handle = service.store_handle();
        let rows = service.snapshot_rows();
        assert_eq!(rows.len(), 1);
        // The disk write only needs the shared store handle, not the service or
        // the monitor hub lock (D25): dropping the service still persists.
        drop(service);
        persist_rows(&handle, &rows).unwrap();
        let reloaded = handle
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .load()
            .unwrap();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].index_id, "n1");
        assert_eq!(reloaded[0].today_up, 5);
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
    fn clash_default_delay_url_matches_upstream_speed_ping_default() {
        assert_eq!(DELAY_TEST_URL, "https://www.google.com/generate_204");
        let service =
            ClashApiService::new(0, None, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
        assert_eq!(service.delay_url(), DELAY_TEST_URL);
    }

    #[test]
    fn clash_delay_url_override_ignores_blank() {
        let mut service =
            ClashApiService::new(0, None, Duration::from_secs(1), Duration::from_secs(1)).unwrap();
        service.set_delay_url("https://example.com/generate_204");
        assert_eq!(service.delay_url(), "https://example.com/generate_204");
        service.set_delay_url("   ");
        assert_eq!(service.delay_url(), "https://example.com/generate_204");
    }

    #[test]
    fn proxy_retry_budget_matches_upstream() {
        assert_eq!(PROXY_RETRY_ATTEMPTS, 3);
        assert_eq!(PROXY_RETRY_DELAY, Duration::from_secs(2));
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

    // -- SP-20 prep: column layout + close freeze (synthetic only, no I/O) --

    fn column(name: &str, width: i32, index: i32) -> domain::ColumnDefinition {
        domain::ColumnDefinition {
            name: name.to_string(),
            width,
            index,
            ..Default::default()
        }
    }

    #[test]
    fn connection_column_defaults_match_upstream_xaml() {
        let defaults = default_connection_columns();
        let pairs: Vec<(&str, i32, i32)> = defaults
            .iter()
            .map(|c| (c.name.as_str(), c.width, c.index))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("Host", 300, 0),
                ("Chain", 500, 1),
                ("Network", 80, 2),
                ("Type", 160, 3),
                ("ProcessPath", 100, 4),
                ("Elapsed", 100, 5),
            ]
        );
    }

    #[test]
    fn normalize_orders_drops_unknown_clamps_and_fills() {
        let out = normalize_connection_columns(vec![
            column("Elapsed", 120, 0),
            column("Nope", 50, 1),
            column("Host", 0, 5),
            column("Chain", -3, 2),
        ]);
        let names: Vec<&str> = out.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Elapsed", "Chain", "Host", "Network", "Type", "ProcessPath"]
        );
        let widths: Vec<i32> = out.iter().map(|c| c.width).collect();
        assert_eq!(widths, vec![120, 500, 300, 80, 160, 100]);
        let indexes: Vec<i32> = out.iter().map(|c| c.index).collect();
        assert_eq!(indexes, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn normalize_storage_roundtrip_is_stable() {
        let first =
            normalize_connection_columns(vec![column("Type", 170, 0), column("Host", 310, 1)]);
        // Independent reopen: feed the stored rows back.
        let second = normalize_connection_columns(first.clone());
        let names = |rows: &[domain::ColumnDefinition]| {
            rows.iter().map(|c| c.name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(names(&first), names(&second));
        assert_eq!(first[0].width, 170);
        assert_eq!(second[0].width, 170);
    }

    #[test]
    fn freeze_close_rejects_empty_id_and_checks_generation() {
        assert!(freeze_close_request("", 7).is_none());
        let request = freeze_close_request("conn-1", 7).expect("frozen");
        assert_eq!(request.id, "conn-1");
        assert!(close_response_is_current(request.generation, 7));
        assert!(!close_response_is_current(request.generation, 8));
    }

    // -- SP-20 full card: real endpoint management (no mocked success) --
    //
    // The stub below stands in for the Clash controller (like a real core's
    // HTTP endpoint); the client under test is the real `ClashApiService`
    // over real HTTP on a floor port >= 11808 (never 10808). A recorded
    // request line proves the frozen id reached the wire; a 500 proves a
    // failed close surfaces as `Err` instead of a fake success.

    const SP20_CONN_JSON: &str = r#"{"downloadTotal":200,"uploadTotal":100,"connections":[{"id":"c1","upload":10,"download":20,"start":"2026-10-05T00:00:00Z","chains":["PROXY","DIRECT"],"rule":"MATCH","metadata":{"host":"host-1.example.invalid","network":"tcp","type":"Shadowsocks","sourceIP":"10.255.0.1","sourcePort":"10000","destinationIP":"192.0.2.1","destinationPort":"443","processPath":"C:/synthetic/app.exe"}}]}"#;

    /// First free `127.0.0.1` port `>= 11808` (10808 is the user's live proxy).
    fn sp20_floor_listener() -> std::net::TcpListener {
        (11808u16..13000)
            .find_map(|port| {
                if port == 10_808 {
                    return None;
                }
                std::net::TcpListener::bind(("127.0.0.1", port)).ok()
            })
            .expect("no free loopback port >= 11808")
    }

    struct Sp20ClashStub {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        requests: std::sync::Arc<Mutex<Vec<String>>>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Sp20ClashStub {
        fn start(fail_close_id: Option<String>) -> Self {
            use std::sync::atomic::Ordering;
            let listener = sp20_floor_listener();
            let port = listener.local_addr().expect("sp20 stub addr").port();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
            let flag = std::sync::Arc::clone(&stop);
            let seen = std::sync::Arc::clone(&requests);
            let failing = fail_close_id.clone();
            let handle = std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    let _ = sp20_serve(&mut stream, &seen, failing.as_deref());
                }
            });
            Self {
                port,
                stop,
                requests,
                handle: Some(handle),
            }
        }

        fn took(&self) -> Vec<String> {
            self.requests
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone()
        }
    }

    impl Drop for Sp20ClashStub {
        fn drop(&mut self) {
            use std::sync::atomic::Ordering;
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn sp20_serve(
        stream: &mut std::net::TcpStream,
        requests: &Mutex<Vec<String>>,
        fail_close_id: Option<&str>,
    ) -> std::io::Result<()> {
        use std::io::{Read, Write};
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        let mut request = Vec::new();
        let mut buf = [0u8; 512];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf)?;
            if n == 0 || request.len() > 16 * 1024 {
                return Ok(());
            }
            request.extend_from_slice(&buf[..n]);
        }
        let text = String::from_utf8_lossy(&request);
        let line = text.lines().next().unwrap_or_default().to_string();
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default();
        let (status, body) = match (method, path) {
            ("GET", "/connections") => ("200 OK", SP20_CONN_JSON.to_string()),
            ("DELETE", "/connections/") => {
                requests
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push("DELETE /connections/".to_string());
                ("204 No Content", String::new())
            }
            ("DELETE", p) if p.starts_with("/connections/") => {
                let id = p.trim_start_matches("/connections/").to_string();
                requests
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(format!("DELETE /connections/{id}"));
                if fail_close_id == Some(id.as_str()) {
                    ("500 Internal Server Error", String::new())
                } else {
                    ("204 No Content", String::new())
                }
            }
            _ => ("404 Not Found", String::new()),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    fn sp20_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("sp20 test runtime")
    }

    #[test]
    fn sp20_list_and_close_use_real_endpoint_with_frozen_id() {
        let stub = Sp20ClashStub::start(None);
        assert!(stub.port >= 11808 && stub.port != 10_808);
        let service = ClashApiService::new(
            stub.port,
            None,
            Duration::from_secs(2),
            Duration::from_secs(1),
        )
        .expect("client builds");

        let rt = sp20_runtime();
        // List comes from the real HTTP controller, not a fabricated row.
        let conns = rt.block_on(service.connections()).expect("list ok");
        assert_eq!(conns.upload_total, 100);
        assert_eq!(conns.download_total, 200);
        let rows = conns.connections.unwrap_or_default();
        assert_eq!(rows.len(), 1);
        let id = rows[0].id.clone().unwrap_or_default();
        assert_eq!(id, "c1");

        // The frozen close target reaches the wire as `DELETE /connections/{id}`.
        let request = freeze_close_request(&id, 7).expect("frozen");
        rt.block_on(service.close_connection(&request.id))
            .expect("close ok");
        assert!(
            stub.took().contains(&"DELETE /connections/c1".to_string()),
            "single close must hit the frozen id, saw {:?}",
            stub.took()
        );

        // Close-all stays on its own path (`DELETE /connections/`, upstream
        // `CloseConnection("")`), never an empty single id.
        rt.block_on(service.close_all()).expect("close-all ok");
        assert!(
            stub.took().contains(&"DELETE /connections/".to_string()),
            "close-all must use its own path, saw {:?}",
            stub.took()
        );
    }

    #[test]
    fn sp20_close_failure_is_surfaced_not_fake_success() {
        let stub = Sp20ClashStub::start(Some("bad".to_string()));
        assert!(stub.port >= 11808 && stub.port != 10_808);
        let service = ClashApiService::new(
            stub.port,
            None,
            Duration::from_secs(2),
            Duration::from_secs(1),
        )
        .expect("client builds");

        let rt = sp20_runtime();
        let err = rt
            .block_on(service.close_connection("bad"))
            .expect_err("500 must fail");
        assert!(
            format!("{err:?}").contains("500"),
            "failure reason must be kept, got {err:?}"
        );
        // The attempt still reached the controller (no silent skip).
        assert!(
            stub.took().contains(&"DELETE /connections/bad".to_string()),
            "failed close must still hit the wire, saw {:?}",
            stub.took()
        );
    }

    #[test]
    fn sp20_column_layout_survives_real_json_roundtrip() {
        // Persist shape -> real serde through `ClashUiItem` -> normalize again:
        // an independent reopen must see the same order and widths.
        let first =
            normalize_connection_columns(vec![column("Elapsed", 120, 0), column("Host", 310, 1)]);
        let stored = serde_json::to_value(domain::ClashUiItem {
            connections_column_item: first.clone(),
            ..Default::default()
        })
        .expect("serialize");
        let names: Vec<&str> = stored["ConnectionsColumnItem"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|row| row["Name"].as_str().expect("name"))
            .collect();
        assert_eq!(names[0], "Elapsed");
        let reopened: domain::ClashUiItem = serde_json::from_value(stored).expect("deserialize");
        let second = normalize_connection_columns(reopened.connections_column_item);
        assert_eq!(
            second.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
            first.iter().map(|c| c.name.clone()).collect::<Vec<_>>()
        );
        assert_eq!(second[0].width, 120);
        assert_eq!(second[1].width, 310);
    }

    // -- SP-22: incremental/coalesced backpressure (synthetic only, no host net) --
    //
    // Red contracts: log/speedtest/connection updates must merge by id with a
    // bounded batch cap instead of one full overlay per event; high-frequency
    // log floods must not squeeze out lifecycle markers or the final result;
    // group probes must run with bounded concurrency and stream per-item
    // progress instead of blocking on one serial pass.

    #[test]
    fn sp22_merge_delay_map_overlays_by_id() {
        let mut existing = HashMap::from([("a".to_string(), 10), ("b".to_string(), -1)]);
        merge_delay_map(
            &mut existing,
            vec![("b".to_string(), 42), ("c".to_string(), 7)],
        );
        assert_eq!(existing.get("a"), Some(&10));
        assert_eq!(existing.get("b"), Some(&42));
        assert_eq!(existing.get("c"), Some(&7));
    }

    #[test]
    fn sp22_coalesce_log_batch_bounds_single_burst_to_tail() {
        let lines: Vec<LogLine> = (0..5000)
            .map(|i| LogLine::new(format!("line {i}")))
            .collect();
        let (kept, dropped) = coalesce_log_batch(lines, 2000);
        assert_eq!(kept.len(), 2000);
        assert_eq!(dropped, 3000);
        assert_eq!(kept.first().expect("head").text, "line 3000");
        assert_eq!(kept.last().expect("tail").text, "line 4999");
    }

    #[test]
    fn sp22_ingest_lines_caps_burst_and_keeps_control_and_final_marker() {
        let mut service = LogService::new(10_000, 0);
        service
            .buffer
            .push_control(core_adapters::log_stream::ControlEvent::ProcessExit(0));
        let burst: Vec<LogLine> = (0..5000)
            .map(|i| LogLine::new(format!("flood {i}")))
            .collect();
        service.ingest_lines(burst);
        assert!(service.snapshot(0, usize::MAX).total <= 10_000);
        assert_eq!(service.buffer.control_len(), 1);
        service.ingest_text("SPEEDTEST DONE id=syn-final delay=42ms");
        let page = service.snapshot(0, usize::MAX);
        assert!(
            page.entries
                .iter()
                .any(|e| e.text.contains("SPEEDTEST DONE")),
            "final result marker must survive the flood"
        );
    }

    /// Delay stub with per-probe latency: 4 probes x 300ms take ~1200ms
    /// serially, ~300ms with 8-way concurrency (budget 900ms).
    struct Sp22DelayStub {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl Sp22DelayStub {
        fn start() -> Self {
            use std::sync::atomic::Ordering;
            let listener = sp20_floor_listener();
            let port = listener.local_addr().expect("sp22 stub addr").port();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = std::sync::Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    std::thread::spawn(move || {
                        use std::io::{Read, Write};
                        let _ = stream.set_read_timeout(Some(Duration::from_millis(2000)));
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(2000)));
                        let mut request = Vec::new();
                        let mut buf = [0u8; 512];
                        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                            let Ok(n) = stream.read(&mut buf) else {
                                return;
                            };
                            if n == 0 || request.len() > 16 * 1024 {
                                return;
                            }
                            request.extend_from_slice(&buf[..n]);
                        }
                        let text = String::from_utf8_lossy(&request);
                        let line = text.lines().next().unwrap_or_default().to_string();
                        let mut parts = line.split_whitespace();
                        let method = parts.next().unwrap_or_default();
                        let path = parts.next().unwrap_or_default();
                        let (status, body) = match (method, path) {
                            ("GET", "/proxies") => (
                                "200 OK",
                                r#"{"proxies":{"g":{"name":"g","type":"Selector","now":"n0","all":["n0","n1","n2","n3"]}}}"#.to_string(),
                            ),
                            ("GET", p)
                                if p.starts_with("/proxies/")
                                    && p.contains("/delay") =>
                            {
                                std::thread::sleep(Duration::from_millis(300));
                                ("200 OK", r#"{"delay":42}"#.to_string())
                            }
                            _ => ("404 Not Found", String::new()),
                        };
                        let response = format!(
                            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    });
                }
            });
            Self {
                port,
                stop,
                handle: Some(handle),
            }
        }
    }

    impl Drop for Sp22DelayStub {
        fn drop(&mut self) {
            use std::sync::atomic::Ordering;
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[test]
    fn sp22_group_delay_is_concurrent_reports_progress_and_keeps_order() {
        let stub = Sp22DelayStub::start();
        assert!(stub.port >= 11808 && stub.port != 10_808);
        let service = ClashApiService::new(
            stub.port,
            None,
            Duration::from_secs(5),
            Duration::from_secs(1),
        )
        .expect("client builds");

        let rt = sp20_runtime();
        let started = Instant::now();
        let mut progress: Vec<(String, i32)> = Vec::new();
        let names = rt
            .block_on(service.group_delay_with_progress("g", &mut |name, delay| {
                progress.push((name, delay));
            }))
            .expect("group delay ok");
        let elapsed = started.elapsed();
        // Bounded concurrency: 4 x 300ms probes must not run serially (~1200ms).
        assert!(
            elapsed < Duration::from_millis(900),
            "probes must overlap, took {elapsed:?}"
        );
        // Incremental progress streams every item; nothing is lost.
        assert_eq!(progress.len(), 4, "progress must fire per item");
        assert!(progress.iter().all(|(_, d)| *d == 42));
        // The returned names keep the controller order.
        assert_eq!(
            names,
            vec![
                "n0".to_string(),
                "n1".to_string(),
                "n2".to_string(),
                "n3".to_string()
            ]
        );
        let mut merged = HashMap::new();
        merge_delay_map(&mut merged, progress);
        assert_eq!(merged.len(), 4);

        // The single-call overlay keeps the same order with all delays applied.
        let out = rt.block_on(service.group_delay("g")).expect("overlay ok");
        let ordered: Vec<&str> = out.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(ordered, vec!["n0", "n1", "n2", "n3"]);
        assert!(out.iter().all(|(_, d)| *d == 42));
    }
}
