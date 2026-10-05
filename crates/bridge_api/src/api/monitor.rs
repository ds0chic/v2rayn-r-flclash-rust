//! T15a FRB surface: statistics, logs and the Clash API monitor.
//!
//! The process keeps one [`MonitorHub`] guarded by a mutex. Traffic/log streams
//! are pushed through FRB [`StreamSink`]s; control functions are synchronous;
//! the Clash controller calls are `async` (they only ever touch loopback, with
//! `no_proxy`, and never the user's live proxy port).
//!
//! Live polling is opt-in via [`monitor_start_polling`]; tests and the widget
//! harness drive the hub through the `#[frb(ignore)]` helpers so no socket or
//! process is created.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use application::monitor::{epoch_day, persist_rows, LogService, StatsService, TrafficStore};
use application::{ClashApiService, InMemoryTrafficStore};
use core_adapters::clash_api::ClashApiClient;
use core_adapters::log_stream::{LogLevel, LogLine};
use core_adapters::stats::{SingboxTrafficSource, StatsSource, TrafficConfig, XrayStatsSource};
use domain::event::{EventEnvelope, EventKind};
use domain::CoreType;
use flutter_rust_bridge::frb;
use serde_json::Value;

use crate::api::contract::{ErrorDto, SimpleResult};
use crate::frb_generated::StreamSink;

/// Default Xray `/debug/vars` poll timeout.
const STATS_TIMEOUT: Duration = Duration::from_secs(5);
/// Clash controller request timeout.
const CLASH_TIMEOUT: Duration = Duration::from_secs(5);
/// Poll cadence (plan §14: traffic at 1 Hz).
const POLL_INTERVAL: Duration = Duration::from_millis(1_000);
/// Stable code surfaced when the persistent `ServerStatItem` store cannot be
/// opened or loaded; a later sync retries while it stays unbound.
const STORE_BIND_ERROR_CODE: &str = "E_MONITOR_STORE";
const STORE_BIND_ERROR_KEY: &str = "error.monitor.store_bind_failed";

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// One node's `ServerStatItem` as delivered to the UI.
#[derive(Clone)]
pub struct NodeTrafficDto {
    pub index_id: String,
    pub total_up: i64,
    pub total_down: i64,
    pub today_up: i64,
    pub today_down: i64,
    pub date_now: i64,
}

/// Live traffic counters + rates (status bar / node table).
#[derive(Clone)]
pub struct TrafficBatchDto {
    pub epoch: u64,
    pub seq: u64,
    pub generation: u64,
    pub proxy_up: u64,
    pub proxy_down: u64,
    pub direct_up: u64,
    pub direct_down: u64,
    pub proxy_up_bps: u64,
    pub proxy_down_bps: u64,
    pub direct_up_bps: u64,
    pub direct_down_bps: u64,
    pub nodes: Vec<NodeTrafficDto>,
}

/// Point-in-time statistics snapshot.
#[derive(Clone)]
pub struct StatsSnapshotDto {
    pub ok: bool,
    pub enabled: bool,
    pub display_speed: bool,
    pub generation: u64,
    pub proxy_up: u64,
    pub proxy_down: u64,
    pub direct_up: u64,
    pub direct_down: u64,
    pub nodes: Vec<NodeTrafficDto>,
    pub error: Option<ErrorDto>,
}

/// One log line as delivered to the UI.
#[derive(Clone)]
pub struct LogLineDto {
    pub text: String,
    /// 0 trace, 1 debug, 2 info, 3 warn, 4 error, 5 fatal, 6 unknown.
    pub level: i32,
    pub truncated: bool,
}

/// Live log batch with the overflow counters.
#[derive(Clone)]
pub struct LogBatchDto {
    pub epoch: u64,
    pub seq: u64,
    pub lines: Vec<LogLineDto>,
    pub dropped_lines: u64,
    pub dropped_bytes: u64,
    pub truncated_lines: u64,
    pub collecting_paused: bool,
    pub scroll_paused: bool,
}

/// Page of historical log lines (`get_logs`).
#[derive(Clone)]
pub struct LogPageDto {
    pub ok: bool,
    pub lines: Vec<LogLineDto>,
    pub total: u32,
    pub dropped_lines: u64,
    pub dropped_bytes: u64,
    pub truncated_lines: u64,
    pub collecting_paused: bool,
    pub scroll_paused: bool,
    pub error: Option<ErrorDto>,
}

/// One Clash proxy or group.
#[derive(Clone)]
pub struct ClashProxyDto {
    pub name: String,
    pub proxy_type: String,
    pub is_group: bool,
    pub now: Option<String>,
    pub all: Vec<String>,
    pub delay: i32,
    pub provider: Option<String>,
}

/// `clash_proxies` result.
#[derive(Clone)]
pub struct ClashProxiesDto {
    pub ok: bool,
    pub supported: bool,
    pub message: Option<String>,
    pub epoch: u64,
    pub seq: u64,
    pub items: Vec<ClashProxyDto>,
    pub error: Option<ErrorDto>,
}

/// One delay probe.
#[derive(Clone)]
pub struct DelayResultDto {
    pub name: String,
    pub delay: i32,
}

/// `clash_group_delay` result.
#[derive(Clone)]
pub struct GroupDelayDto {
    pub ok: bool,
    pub supported: bool,
    pub group: String,
    pub items: Vec<DelayResultDto>,
    pub error: Option<ErrorDto>,
}

/// One Clash connection row.
#[derive(Clone)]
pub struct ClashConnectionDto {
    pub id: String,
    pub host: Option<String>,
    pub network: Option<String>,
    pub connection_type: Option<String>,
    pub chains: Vec<String>,
    pub rule: Option<String>,
    pub process_path: Option<String>,
    pub source: Option<String>,
    pub destination: Option<String>,
    pub upload: u64,
    pub download: u64,
    pub start: Option<String>,
}

/// `clash_connections` result.
#[derive(Clone)]
pub struct ClashConnectionsDto {
    pub ok: bool,
    pub supported: bool,
    pub message: Option<String>,
    pub upload_total: u64,
    pub download_total: u64,
    pub items: Vec<ClashConnectionDto>,
    pub error: Option<ErrorDto>,
}

/// Generic result for select/close/mode actions.
#[derive(Clone)]
pub struct MonitorActionResult {
    pub ok: bool,
    pub supported: bool,
    pub message: Option<String>,
    pub error: Option<ErrorDto>,
}

/// `/configs` mode state: the live mode plus the selectable mode list.
#[derive(Clone)]
pub struct ClashModeDto {
    pub ok: bool,
    pub supported: bool,
    pub message: Option<String>,
    /// Currently applied mode (`Rule`/`Global`/`Direct`, core-cased).
    pub mode: Option<String>,
    /// Modes advertised by the core (`mode-list`/`modes`).
    pub modes: Vec<String>,
    pub error: Option<ErrorDto>,
}

/// Result of `set_page_visible`.
#[derive(Clone)]
pub struct PageVisibilityDto {
    pub visible: bool,
    pub subscribed: bool,
    pub refresh_interval_ms: u32,
}

fn not_supported() -> Option<String> {
    Some("当前内核不提供 Clash API".to_string())
}

fn clash_error(e: core_adapters::clash_api::ClashError) -> ErrorDto {
    ErrorDto {
        code: "E_CLASH".to_string(),
        message_key: "error.clash_api".to_string(),
        field_path: None,
        retryable: true,
        operation_id: None,
        detail: Some(e.to_string()),
    }
}

fn domain_error(e: domain::DomainError) -> ErrorDto {
    ErrorDto::from(e)
}

fn level_value(level: LogLevel) -> i32 {
    match level {
        LogLevel::Trace => 0,
        LogLevel::Debug => 1,
        LogLevel::Info => 2,
        LogLevel::Warn => 3,
        LogLevel::Error => 4,
        LogLevel::Fatal => 5,
        LogLevel::Unknown => 6,
    }
}

fn level_from_value(value: i32) -> Option<LogLevel> {
    Some(match value {
        0 => LogLevel::Trace,
        1 => LogLevel::Debug,
        2 => LogLevel::Info,
        3 => LogLevel::Warn,
        4 => LogLevel::Error,
        5 => LogLevel::Fatal,
        _ => return None,
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Hub
// ---------------------------------------------------------------------------

/// Applied-session signature: `(core, state_port, state_port2, active node,
/// api secret, enable_statistics, display_real_time_speed)`.
type SessionSignature = (i32, u16, u16, Option<String>, Option<String>, bool, bool);

struct MonitorHub {
    core: CoreType,
    state_port: u16,
    state_port2: u16,
    secret: Option<String>,
    refresh_ms: u32,
    stats: StatsService,
    logs: LogService,
    traffic_subscribers: Vec<StreamSink<TrafficBatchDto>>,
    log_subscribers: Vec<StreamSink<LogBatchDto>>,
    logs_visible: bool,
    proxies_visible: bool,
    connections_visible: bool,
    source_sig: Option<(i32, u16, u16, Option<String>)>,
    /// Test seam: when set, Clash calls use this base URL instead of the port.
    clash_base_override: Option<String>,
    /// Configured delay-probe URL (settings `SpeedPingTestUrl`). `None` uses
    /// the upstream default in [`ClashApiService`].
    delay_test_url: Option<String>,
    /// Whether the persistent `ServerStatItem` store has been bound to the
    /// engine's data directory (cleared if a bind fails so it is retried).
    store_bound: bool,
    /// The engine data dir the current store is bound to. A change (engine
    /// swap / restore to a different data dir) forces a rebind.
    store_dir: Option<PathBuf>,
    /// Last store open/load failure, surfaced through `stats_snapshot().error`.
    store_error: Option<ErrorDto>,
    /// Applied-session signature `(core, state_port, state_port2, active node,
    /// api secret, enable_statistics, display_real_time_speed)`. Only a real
    /// change forces the poller to rebuild its `StatsSource`; unrelated
    /// RuntimeView churn (log lines, heartbeat seq) never does.
    session_sig: Option<SessionSignature>,
    epoch: u64,
    seq: u64,
}

impl MonitorHub {
    fn new() -> Self {
        Self {
            core: CoreType::Xray,
            state_port: 0,
            state_port2: 0,
            secret: None,
            refresh_ms: 2_000,
            stats: StatsService::new(Box::new(InMemoryTrafficStore::new()), false, false),
            logs: LogService::with_defaults(),
            traffic_subscribers: Vec::new(),
            log_subscribers: Vec::new(),
            logs_visible: false,
            proxies_visible: false,
            connections_visible: false,
            source_sig: None,
            clash_base_override: None,
            delay_test_url: None,
            store_bound: false,
            store_dir: None,
            store_error: None,
            session_sig: None,
            epoch: 1,
            seq: 0,
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    fn node_dtos(&self) -> Vec<NodeTrafficDto> {
        self.stats
            .nodes()
            .map(|n| NodeTrafficDto {
                index_id: n.index_id.clone(),
                total_up: n.total_up,
                total_down: n.total_down,
                today_up: n.today_up,
                today_down: n.today_down,
                date_now: n.date_now,
            })
            .collect()
    }

    fn traffic_dto(&mut self) -> TrafficBatchDto {
        let proxy = self.stats.session_proxy();
        let direct = self.stats.session_direct();
        let (pbps, dbps) = self.stats.rates();
        let epoch = self.epoch;
        let seq = self.next_seq();
        TrafficBatchDto {
            epoch,
            seq,
            generation: self.stats.generation(),
            proxy_up: proxy.up,
            proxy_down: proxy.down,
            direct_up: direct.up,
            direct_down: direct.down,
            proxy_up_bps: pbps.up,
            proxy_down_bps: pbps.down,
            direct_up_bps: dbps.up,
            direct_down_bps: dbps.down,
            nodes: self.node_dtos(),
        }
    }
}

fn hub() -> &'static Arc<Mutex<MonitorHub>> {
    static HUB: OnceLock<Arc<Mutex<MonitorHub>>> = OnceLock::new();
    HUB.get_or_init(|| Arc::new(Mutex::new(MonitorHub::new())))
}

fn with_hub<T>(f: impl FnOnce(&mut MonitorHub) -> T) -> T {
    let mut guard = hub().lock().unwrap_or_else(|p| p.into_inner());
    f(&mut guard)
}

// ---------------------------------------------------------------------------
// Configuration / statistics
// ---------------------------------------------------------------------------

/// Configure the monitor with the running session's facts.
///
/// `core` is the domain `CoreType` numeric value; `state_port` is Xray's
/// `/debug/vars` port and `state_port2` the Clash/`traffic` port. Nothing is
/// contacted here.
#[frb(sync)]
pub fn monitor_configure(
    core: i32,
    state_port: u32,
    state_port2: u32,
    secret: Option<String>,
    enable_statistics: bool,
    display_real_time_speed: bool,
    refresh_interval_ms: u32,
) -> SimpleResult {
    with_hub(|h| {
        h.core = CoreType::from_value(core).unwrap_or(CoreType::Xray);
        h.state_port = state_port.min(u16::MAX as u32) as u16;
        h.state_port2 = state_port2.min(u16::MAX as u32) as u16;
        h.secret = secret;
        h.refresh_ms = refresh_interval_ms.max(200);
        h.stats.set_enabled(enable_statistics);
        h.stats.set_display_speed(display_real_time_speed);
        h.source_sig = None;
        h.session_sig = None;
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Set the active node id that receives the proxy byte attribution.
#[frb(sync)]
pub fn monitor_set_active_node(index_id: Option<String>) -> SimpleResult {
    with_hub(|h| {
        h.stats.set_active_index(index_id);
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Whether statistics collection is active.
#[frb(sync)]
pub fn monitor_enabled() -> bool {
    with_hub(|h| h.stats.active())
}

/// Current statistics snapshot.
#[frb(sync)]
pub fn stats_snapshot() -> StatsSnapshotDto {
    with_hub(|h| {
        let proxy = h.stats.session_proxy();
        let direct = h.stats.session_direct();
        StatsSnapshotDto {
            ok: true,
            enabled: h.stats.is_enabled(),
            display_speed: h.stats.display_speed(),
            generation: h.stats.generation(),
            proxy_up: proxy.up,
            proxy_down: proxy.down,
            direct_up: direct.up,
            direct_down: direct.down,
            nodes: h.node_dtos(),
            error: h.store_error.clone(),
        }
    })
}

/// Drain the in-memory statistics rows into the persistent store.
///
/// The rows and the store handle are copied under the hub lock, then the write
/// happens after the lock is released (the same D25 seam the poller uses) so a
/// slow disk cannot block `stats_snapshot`/`get_logs`.
fn persist_stats(h: &MonitorHub) -> Result<(), domain::DomainError> {
    persist_rows(&h.stats.store_handle(), &h.stats.snapshot_rows())
}

/// Flush the statistics store before a real exit (R4-05).
///
/// A reopen must see the final counters instead of losing the last interval.
/// A store failure is returned as a structured error, never reported as a
/// successful flush.
#[frb(sync)]
pub fn stats_flush() -> SimpleResult {
    match with_hub(|h| persist_stats(h)) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(domain_error(e)),
        },
    }
}

/// `ClearAllServerStatistics`: clear memory + persisted rows.
#[frb(sync)]
pub fn clear_stats() -> SimpleResult {
    with_hub(|h| match h.stats.clear_all() {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(domain_error(e)),
        },
    })
}

/// Register a traffic stream. Sends the current epoch/seq header first.
pub fn subscribe_traffic(sink: StreamSink<TrafficBatchDto>) {
    let header = with_hub(|h| {
        let mut dto = h.traffic_dto();
        dto.generation = 0;
        dto
    });
    if sink.add(header).is_err() {
        return;
    }
    with_hub(|h| h.traffic_subscribers.push(sink));
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

/// Number of log lines currently buffered.
#[frb(sync)]
pub fn log_total() -> u32 {
    with_hub(|h| h.logs.snapshot(0, 0).total as u32)
}

/// Read a page of buffered log lines.
#[frb(sync)]
pub fn get_logs(offset: u32, limit: u32) -> LogPageDto {
    with_hub(|h| {
        let page = h.logs.snapshot(offset as usize, limit as usize);
        LogPageDto {
            ok: true,
            lines: page
                .entries
                .into_iter()
                .map(|e| LogLineDto {
                    text: e.text,
                    level: level_value(e.level),
                    truncated: e.truncated,
                })
                .collect(),
            total: page.total as u32,
            dropped_lines: page.dropped_lines,
            dropped_bytes: page.dropped_bytes,
            truncated_lines: page.truncated_lines,
            collecting_paused: page.collecting_paused,
            scroll_paused: page.scroll_paused,
            error: None,
        }
    })
}

/// Clear the visible logs (control queue is preserved).
#[frb(sync)]
pub fn clear_logs() -> SimpleResult {
    with_hub(|h| {
        h.logs.clear();
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Set the level/keyword filter applied to newly collected lines.
#[frb(sync)]
pub fn set_log_filter(min_level: i32, include: Vec<String>, exclude: Vec<String>) -> SimpleResult {
    with_hub(|h| {
        h.logs
            .set_filter(level_from_value(min_level), include, exclude);
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Pause collection and/or scroll independently.
#[frb(sync)]
pub fn set_log_pause(collecting_paused: bool, scroll_paused: bool) -> SimpleResult {
    with_hub(|h| {
        h.logs.set_collecting_paused(collecting_paused);
        h.logs.set_scroll_paused(scroll_paused);
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Register a log stream. The current buffer is replayed first.
pub fn subscribe_logs(sink: StreamSink<LogBatchDto>) {
    let header = with_hub(|h| {
        let page = h.logs.snapshot(0, 2_000);
        let epoch = h.epoch;
        let seq = h.next_seq();
        LogBatchDto {
            epoch,
            seq,
            lines: page
                .entries
                .into_iter()
                .map(|e| LogLineDto {
                    text: e.text,
                    level: level_value(e.level),
                    truncated: e.truncated,
                })
                .collect(),
            dropped_lines: page.dropped_lines,
            dropped_bytes: page.dropped_bytes,
            truncated_lines: page.truncated_lines,
            collecting_paused: page.collecting_paused,
            scroll_paused: page.scroll_paused,
        }
    });
    if sink.add(header).is_err() {
        return;
    }
    with_hub(|h| h.log_subscribers.push(sink));
}

/// Feed a runtime event into the log service and fan it out to log streams.
///
/// Called from the engine's single event sink so net-host `log_line` /
/// `log_batch` events reach the UI without a second subscription.
pub(crate) fn ingest_runtime_event(event: &EventEnvelope) {
    let lines = lines_from_envelope(event);
    with_hub(|h| {
        h.logs.ingest_envelope(event);
        // A paused collection drops lines from the ring; do not leak the raw
        // envelope lines to subscribers either (RT-19: collection vs view).
        if lines.is_empty() || h.logs.is_collecting_paused() || h.log_subscribers.is_empty() {
            return;
        }
        let epoch = h.epoch;
        let seq = h.next_seq();
        let dropped_lines = h.logs.dropped_lines();
        let dropped_bytes = h.logs.dropped_bytes();
        let truncated = h.logs.truncated();
        let collecting_paused = h.logs.is_collecting_paused();
        let scroll_paused = h.logs.is_scroll_paused();
        let batch = LogBatchDto {
            epoch,
            seq,
            lines,
            dropped_lines,
            dropped_bytes,
            truncated_lines: truncated,
            collecting_paused,
            scroll_paused,
        };
        h.log_subscribers
            .retain(|sink| sink.add(batch.clone()).is_ok());
    });
}

fn lines_from_envelope(event: &EventEnvelope) -> Vec<LogLineDto> {
    let mut out = Vec::new();
    match &event.kind {
        EventKind::LogBatch => {
            if let Some(entries) = event.payload.get("entries").and_then(Value::as_array) {
                for entry in entries {
                    let text = entry
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let truncated = entry
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let line = LogLine { text, truncated };
                    out.push(LogLineDto {
                        level: level_value(line.level()),
                        text: line.text,
                        truncated: line.truncated,
                    });
                }
            } else if let Some(lines) = event.payload.get("lines").and_then(Value::as_array) {
                for value in lines {
                    if let Some(text) = value.as_str() {
                        out.push(LogLineDto {
                            level: level_value(LogLine::new(text).level()),
                            text: text.to_string(),
                            truncated: false,
                        });
                    }
                }
            }
        }
        EventKind::Other(name) if name == "log_line" => {
            if let Some(text) = event.payload.get("text").and_then(Value::as_str) {
                out.push(LogLineDto {
                    level: level_value(LogLine::new(text).level()),
                    text: text.to_string(),
                    truncated: false,
                });
            }
        }
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// Page visibility
// ---------------------------------------------------------------------------

/// Toggle a page's visibility. Only visible pages are subscribed (plan §14).
#[frb(sync)]
pub fn set_page_visible(page: String, visible: bool) -> PageVisibilityDto {
    with_hub(|h| match page.as_str() {
        "logs" => {
            h.logs_visible = visible;
            PageVisibilityDto {
                visible,
                subscribed: visible,
                refresh_interval_ms: 250,
            }
        }
        "proxies" => {
            h.proxies_visible = visible;
            PageVisibilityDto {
                visible,
                subscribed: visible,
                refresh_interval_ms: h.refresh_ms,
            }
        }
        "connections" => {
            h.connections_visible = visible;
            PageVisibilityDto {
                visible,
                subscribed: visible,
                refresh_interval_ms: h.refresh_ms,
            }
        }
        _ => PageVisibilityDto {
            visible: false,
            subscribed: false,
            refresh_interval_ms: h.refresh_ms,
        },
    })
}

// ---------------------------------------------------------------------------
// Clash API
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ClashConfig {
    base: Option<String>,
    port: u16,
    secret: Option<String>,
    refresh: Duration,
    supported: bool,
    delay_url: Option<String>,
}

fn clash_config() -> ClashConfig {
    with_hub(|h| ClashConfig {
        base: h.clash_base_override.clone(),
        port: h.state_port2,
        secret: h.secret.clone(),
        refresh: Duration::from_millis(u64::from(h.refresh_ms.max(200))),
        supported: ClashApiService::supported(h.core),
        delay_url: h.delay_test_url.clone(),
    })
}

fn build_clash(cfg: &ClashConfig) -> Result<ClashApiService, core_adapters::clash_api::ClashError> {
    let mut service = match &cfg.base {
        Some(base) => ClashApiService::from_client(
            ClashApiClient::from_base(base.clone(), cfg.secret.clone(), CLASH_TIMEOUT)?,
            cfg.refresh,
        ),
        None => ClashApiService::new(cfg.port, cfg.secret.clone(), CLASH_TIMEOUT, cfg.refresh)?,
    };
    if let Some(url) = &cfg.delay_url {
        service.set_delay_url(url.clone());
    }
    Ok(service)
}

/// Set the delay-probe URL from settings (`SpeedPingTestUrl`). Empty clears it
/// back to the upstream default.
#[frb(sync)]
pub fn monitor_set_delay_url(url: Option<String>) -> SimpleResult {
    with_hub(|h| {
        h.delay_test_url = url.filter(|value| !value.trim().is_empty());
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// Whether the configured core exposes the Clash API.
#[frb(sync)]
pub fn clash_supported() -> bool {
    with_hub(|h| ClashApiService::supported(h.core))
}

/// Fetch proxies/groups from the Clash controller.
pub async fn clash_proxies() -> ClashProxiesDto {
    let cfg = clash_config();
    if !cfg.supported {
        return ClashProxiesDto {
            ok: false,
            supported: false,
            message: not_supported(),
            epoch: 0,
            seq: 0,
            items: Vec::new(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return ClashProxiesDto {
                ok: false,
                supported: true,
                message: None,
                epoch: 0,
                seq: 0,
                items: Vec::new(),
                error: Some(clash_error(e)),
            }
        }
    };
    match service.proxies_with_retry().await {
        Ok(item) => {
            let providers = item.provider_index_map.clone();
            let items = item
                .proxies
                .into_iter()
                .map(|(name, proxy)| {
                    let provider = providers.get(&name).cloned();
                    let all = proxy.all.clone().unwrap_or_default();
                    let is_group = proxy.all.is_some();
                    ClashProxyDto {
                        name,
                        proxy_type: proxy.proxy_type.unwrap_or_default(),
                        is_group,
                        now: proxy.now,
                        all,
                        delay: proxy.delay,
                        provider,
                    }
                })
                .collect();
            let (epoch, seq) = with_hub(|h| {
                let epoch = h.epoch;
                let seq = h.next_seq();
                (epoch, seq)
            });
            ClashProxiesDto {
                ok: true,
                supported: true,
                message: None,
                epoch,
                seq,
                items,
                error: None,
            }
        }
        Err(e) => ClashProxiesDto {
            ok: false,
            supported: true,
            message: None,
            epoch: 0,
            seq: 0,
            items: Vec::new(),
            error: Some(clash_error(e)),
        },
    }
}

/// Read the live `/configs` mode and the selectable mode list.
pub async fn clash_mode_state() -> ClashModeDto {
    let cfg = clash_config();
    if !cfg.supported {
        return ClashModeDto {
            ok: false,
            supported: false,
            message: not_supported(),
            mode: None,
            modes: Vec::new(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return ClashModeDto {
                ok: false,
                supported: true,
                message: None,
                mode: None,
                modes: Vec::new(),
                error: Some(clash_error(e)),
            }
        }
    };
    let mode = service.mode().await;
    let modes = service.modes().await;
    match (mode, modes) {
        (Ok(mode), Ok(modes)) => ClashModeDto {
            ok: true,
            supported: true,
            message: None,
            mode,
            modes,
            error: None,
        },
        (Err(e), _) | (_, Err(e)) => ClashModeDto {
            ok: false,
            supported: true,
            message: None,
            mode: None,
            modes: Vec::new(),
            error: Some(clash_error(e)),
        },
    }
}

/// Switch the Clash routing mode (`PATCH /configs`).
pub async fn update_clash_mode(mode: String) -> MonitorActionResult {
    let cfg = clash_config();
    if !cfg.supported {
        return MonitorActionResult {
            ok: false,
            supported: false,
            message: not_supported(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return MonitorActionResult {
                ok: false,
                supported: true,
                message: None,
                error: Some(clash_error(e)),
            }
        }
    };
    match service.update_mode(&mode).await {
        Ok(()) => MonitorActionResult {
            ok: true,
            supported: true,
            message: None,
            error: None,
        },
        Err(e) => MonitorActionResult {
            ok: false,
            supported: true,
            message: None,
            error: Some(clash_error(e)),
        },
    }
}

/// Select `name` inside proxy group `group`.
pub async fn select_clash_proxy(group: String, name: String) -> MonitorActionResult {
    let cfg = clash_config();
    if !cfg.supported {
        return MonitorActionResult {
            ok: false,
            supported: false,
            message: not_supported(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return MonitorActionResult {
                ok: false,
                supported: true,
                message: None,
                error: Some(clash_error(e)),
            }
        }
    };
    match service.select(&group, &name).await {
        Ok(()) => MonitorActionResult {
            ok: true,
            supported: true,
            message: None,
            error: None,
        },
        Err(e) => MonitorActionResult {
            ok: false,
            supported: true,
            message: None,
            error: Some(clash_error(e)),
        },
    }
}

/// Single-node delay probe (`-1` means timeout/unreachable).
pub async fn clash_proxy_delay(name: String) -> DelayResultDto {
    let cfg = clash_config();
    let delay = if !cfg.supported {
        -1
    } else {
        match build_clash(&cfg) {
            Ok(service) => service.proxy_delay(&name).await,
            Err(_) => -1,
        }
    };
    DelayResultDto { name, delay }
}

/// Probe every child of a proxy group.
pub async fn clash_group_delay(group: String) -> GroupDelayDto {
    let cfg = clash_config();
    if !cfg.supported {
        return GroupDelayDto {
            ok: false,
            supported: false,
            group,
            items: Vec::new(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return GroupDelayDto {
                ok: false,
                supported: true,
                group,
                items: Vec::new(),
                error: Some(clash_error(e)),
            }
        }
    };
    match service.group_delay(&group).await {
        Ok(items) => GroupDelayDto {
            ok: true,
            supported: true,
            group,
            items: items
                .into_iter()
                .map(|(name, delay)| DelayResultDto { name, delay })
                .collect(),
            error: None,
        },
        Err(e) => GroupDelayDto {
            ok: false,
            supported: true,
            group,
            items: Vec::new(),
            error: Some(clash_error(e)),
        },
    }
}

/// Fetch the active connection table.
pub async fn clash_connections() -> ClashConnectionsDto {
    let cfg = clash_config();
    if !cfg.supported {
        return ClashConnectionsDto {
            ok: false,
            supported: false,
            message: not_supported(),
            upload_total: 0,
            download_total: 0,
            items: Vec::new(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return ClashConnectionsDto {
                ok: false,
                supported: true,
                message: None,
                upload_total: 0,
                download_total: 0,
                items: Vec::new(),
                error: Some(clash_error(e)),
            }
        }
    };
    match service.connections().await {
        Ok(conns) => ClashConnectionsDto {
            ok: true,
            supported: true,
            message: None,
            upload_total: conns.upload_total,
            download_total: conns.download_total,
            items: conns
                .connections
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    let metadata = c.metadata.unwrap_or_default();
                    let source = join_host_port(metadata.source_ip, metadata.source_port);
                    let destination =
                        join_host_port(metadata.destination_ip, metadata.destination_port);
                    ClashConnectionDto {
                        id: c.id.unwrap_or_default(),
                        host: metadata.host,
                        network: metadata.network,
                        connection_type: metadata.connection_type,
                        chains: c.chains.unwrap_or_default(),
                        rule: c.rule,
                        process_path: metadata.process_path.or(metadata.process),
                        source,
                        destination,
                        upload: c.upload,
                        download: c.download,
                        start: c.start,
                    }
                })
                .collect(),
            error: None,
        },
        Err(e) => ClashConnectionsDto {
            ok: false,
            supported: true,
            message: None,
            upload_total: 0,
            download_total: 0,
            items: Vec::new(),
            error: Some(clash_error(e)),
        },
    }
}

fn join_host_port(host: Option<String>, port: Option<String>) -> Option<String> {
    match (host, port) {
        (Some(host), Some(port)) if !host.is_empty() => Some(format!("{host}:{port}")),
        (Some(host), None) if !host.is_empty() => Some(host),
        _ => None,
    }
}

/// Close one connection by id.
pub async fn close_clash_connection(id: String) -> MonitorActionResult {
    let cfg = clash_config();
    if !cfg.supported {
        return MonitorActionResult {
            ok: false,
            supported: false,
            message: not_supported(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return MonitorActionResult {
                ok: false,
                supported: true,
                message: None,
                error: Some(clash_error(e)),
            }
        }
    };
    match service.close_connection(&id).await {
        Ok(()) => MonitorActionResult {
            ok: true,
            supported: true,
            message: None,
            error: None,
        },
        Err(e) => MonitorActionResult {
            ok: false,
            supported: true,
            message: None,
            error: Some(clash_error(e)),
        },
    }
}

/// Close every connection.
pub async fn close_all_clash_connections() -> MonitorActionResult {
    let cfg = clash_config();
    if !cfg.supported {
        return MonitorActionResult {
            ok: false,
            supported: false,
            message: not_supported(),
            error: None,
        };
    }
    let service = match build_clash(&cfg) {
        Ok(service) => service,
        Err(e) => {
            return MonitorActionResult {
                ok: false,
                supported: true,
                message: None,
                error: Some(clash_error(e)),
            }
        }
    };
    match service.close_all().await {
        Ok(()) => MonitorActionResult {
            ok: true,
            supported: true,
            message: None,
            error: None,
        },
        Err(e) => MonitorActionResult {
            ok: false,
            supported: true,
            message: None,
            error: Some(clash_error(e)),
        },
    }
}

// ---------------------------------------------------------------------------
// Live polling (opt-in)
// ---------------------------------------------------------------------------

static POLL_STARTED: OnceLock<()> = OnceLock::new();
static EPOCH_COUNTER: OnceLock<AtomicU64> = OnceLock::new();
static SEQ_COUNTER: OnceLock<AtomicU64> = OnceLock::new();

/// Pull the applied-session facts from the engine into the hub.
///
/// This is the normal user entry for statistics: after a managed core is
/// applied, the UI calls `monitor_start_polling`, which syncs the running
/// core's statistics ports and active node so the 1 Hz poller and per-node
/// `ServerStatItem` persistence run without any test/diagnostic hook. When no
/// core is running the ports are cleared and the poller idles, which stops
/// collecting the old session; hiding a page never reaches this path.
fn sync_from_engine_session(h: &mut MonitorHub) {
    let engine = crate::api::engine::engine();
    let store_dir = engine.data_dir().map(PathBuf::from);
    match engine.monitor_session() {
        Some(session) => {
            let (enabled, speed) = engine.monitor_settings();
            // Only a real applied-session change (core / endpoint ports /
            // statistics toggle / active node) forces a source rebuild; an
            // unrelated RuntimeView churn (heartbeat seq, log lines) leaves
            // `source_sig` untouched so the poller keeps its source.
            let signature = (
                session.core.value(),
                session.state_port,
                session.state_port2,
                session.active_index_id.clone(),
                session.api_secret.clone(),
                enabled,
                speed,
            );
            if h.session_sig.as_ref() != Some(&signature) {
                h.session_sig = Some(signature);
                h.source_sig = None;
            }
            h.core = session.core;
            h.state_port = session.state_port;
            h.state_port2 = session.state_port2;
            // R3-07: a full Custom config carries its own Clash secret; the
            // normal apply path must publish it to the hub, not only the
            // dedicated `monitor_configure` hook. Never logged.
            h.secret = session.api_secret.clone();
            h.stats.set_active_index(session.active_index_id);
            h.stats.set_enabled(enabled);
            h.stats.set_display_speed(speed);
        }
        None => {
            h.state_port = 0;
            h.state_port2 = 0;
            h.secret = None;
            h.source_sig = None;
            h.session_sig = None;
            h.stats.set_active_index(None);
        }
    }
    rebind_store(h, store_dir, || engine.traffic_store());
}

/// Bind (or re-bind) the persistent `ServerStatItem` store.
///
/// A failed open/load is never silently reported as bound: the error is kept
/// in `store_error` (surfaced through `stats_snapshot().error`) and
/// `store_bound` stays `false` so a later sync retries. A change of engine data
/// dir (engine swap / restore) drops the old binding and forces a fresh one.
fn rebind_store(
    h: &mut MonitorHub,
    store_dir: Option<PathBuf>,
    open: impl FnOnce() -> Option<Box<dyn TrafficStore>>,
) {
    if h.store_dir != store_dir {
        h.store_dir = store_dir.clone();
        h.store_bound = false;
        h.store_error = None;
        h.stats.set_store(Box::new(InMemoryTrafficStore::new()));
    }
    if h.store_bound {
        return;
    }
    if store_dir.is_none() {
        // In-memory engine: there is no persistent store to bind or retry.
        h.store_bound = true;
        h.store_error = None;
        return;
    }
    match open() {
        Some(store) => {
            h.stats.set_store(store);
            match h.stats.load() {
                Ok(()) => {
                    h.store_bound = true;
                    h.store_error = None;
                }
                Err(e) => {
                    h.store_error = Some(ErrorDto::from(e));
                }
            }
        }
        None => {
            h.store_error = Some(ErrorDto {
                code: STORE_BIND_ERROR_CODE.to_string(),
                message_key: STORE_BIND_ERROR_KEY.to_string(),
                field_path: None,
                retryable: true,
                operation_id: None,
                detail: None,
            });
        }
    }
}

/// Start the 1 Hz statistics poller once. No-op for unsupported cores.
///
/// Each call first (re)synchronizes the hub with the applied session so a
/// normal GUI apply enables real collection and a stop/switch clears the old
/// session's ports.
#[frb(sync)]
pub fn monitor_start_polling() {
    // R4-05: a stop/switch clears the session; drain the in-memory rows into
    // the persistent store before the UI exits so the last interval survives a
    // reopen. A store failure is recorded readably, never hidden.
    let flush = with_hub(|h| {
        sync_from_engine_session(h);
        persist_stats(h)
    });
    if let Err(error) = flush {
        with_hub(|h| h.store_error = Some(ErrorDto::from(error)));
    }
    POLL_STARTED.get_or_init(|| {
        let shared = Arc::clone(hub());
        let _ = std::thread::Builder::new()
            .name("monitor-poll".into())
            .spawn(move || poll_loop(shared));
    });
}

fn poll_loop(shared: Arc<Mutex<MonitorHub>>) {
    // R3-06: the loop must run *inside* a continuously driven async executor.
    // The previous sync loop called `rt.block_on(src.poll())` once (an
    // immediate in-memory snapshot) and then blocked the thread with
    // `std::thread::sleep`. That never let the runtime poll the tasks
    // `SingboxTrafficSource` spawns for the WS connect/pump, so sing-box
    // statistics could stay at zero forever. Keeping the whole loop as one
    // async future on the same current-thread runtime lets those spawned tasks
    // run while the loop awaits its cadence timer, and hiding the UI is
    // irrelevant because this thread is independent of the widget tree.
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    rt.block_on(poll_loop_async(shared));
}

async fn poll_loop_async(shared: Arc<Mutex<MonitorHub>>) {
    let mut source: Option<Box<dyn StatsSource>> = None;
    loop {
        let (core, p1, p2, signature, active) = {
            let h = shared.lock().unwrap_or_else(|p| p.into_inner());
            let signature = (
                h.core.value(),
                h.state_port,
                h.state_port2,
                h.secret.clone(),
            );
            (
                h.core,
                h.state_port,
                h.state_port2,
                signature,
                h.stats.active(),
            )
        };
        if !active || (p1 == 0 && p2 == 0) {
            // Drop any stale source so a stopped session's WS task is cancelled
            // instead of accumulating against a dead endpoint.
            source = None;
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }
        let changed = {
            let mut h = shared.lock().unwrap_or_else(|p| p.into_inner());
            if h.source_sig.as_ref() != Some(&signature) {
                h.source_sig = Some(signature);
                true
            } else {
                false
            }
        };
        if changed || source.is_none() {
            source = build_source(core, p1, p2);
        }
        if let Some(src) = source.as_mut() {
            if let Ok(samples) = src.poll().await {
                let generation = src.generation();
                let today = epoch_day(now_unix());
                // D25: the hub lock only updates memory and clones the rows to
                // persist; the SQLite write runs after the lock is released so a
                // slow disk cannot block `stats_snapshot`/`get_logs` or the UI.
                let (dto, sinks, flush) = {
                    let mut h = shared.lock().unwrap_or_else(|p| p.into_inner());
                    let update = h.stats.apply(&samples, generation, today, Instant::now());
                    if update.applied {
                        let rows = h.stats.snapshot_rows();
                        let handle = h.stats.store_handle();
                        (
                            Some(h.traffic_dto()),
                            h.traffic_subscribers.clone(),
                            Some((handle, rows)),
                        )
                    } else {
                        (None, Vec::new(), None)
                    }
                };
                if let Some((handle, rows)) = flush {
                    if let Err(error) = persist_rows(&handle, &rows) {
                        // Bounded single poller: record the failure readably and
                        // retry on the next applied snapshot, never silently drop.
                        let mut h = shared.lock().unwrap_or_else(|p| p.into_inner());
                        h.store_error = Some(ErrorDto::from(error));
                    }
                }
                if let Some(dto) = dto {
                    for sink in sinks {
                        let _ = sink.add(dto.clone());
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn build_source(core: CoreType, state_port: u16, state_port2: u16) -> Option<Box<dyn StatsSource>> {
    match core {
        CoreType::Xray | CoreType::V2fly | CoreType::V2flyV5 => {
            XrayStatsSource::new(state_port, STATS_TIMEOUT)
                .ok()
                .map(|s| Box::new(s) as Box<dyn StatsSource>)
        }
        // Requires the ambient runtime: the caller runs inside `poll_loop`'s
        // `rt.block_on`, so the WS task is spawned onto the same executor.
        CoreType::SingBox => SingboxTrafficSource::new(TrafficConfig::new(state_port2))
            .ok()
            .map(|s| Box::new(s) as Box<dyn StatsSource>),
        _ => None,
    }
}

/// Look up the next epoch/sequence (used by stream headers in tests).
#[frb(ignore)]
pub fn take_seq() -> (u64, u64) {
    let epoch = EPOCH_COUNTER
        .get_or_init(|| AtomicU64::new(1))
        .load(Ordering::Acquire);
    let seq = SEQ_COUNTER
        .get_or_init(|| AtomicU64::new(0))
        .fetch_add(1, Ordering::AcqRel)
        + 1;
    (epoch, seq)
}

// ---------------------------------------------------------------------------
// Test seams (not exported to Dart)
// ---------------------------------------------------------------------------

#[frb(ignore)]
pub fn reset_monitor_for_test() {
    let mut guard = hub().lock().unwrap_or_else(|p| p.into_inner());
    *guard = MonitorHub::new();
}

#[frb(ignore)]
pub fn set_clash_base_for_test(base: Option<String>) {
    with_hub(|h| h.clash_base_override = base);
}

#[frb(ignore)]
pub fn ingest_log_text_for_test(text: &str) {
    with_hub(|h| h.logs.ingest_text(text));
}

#[frb(ignore)]
pub fn ingest_stats_for_test(proxy_up: u64, proxy_down: u64, generation: u64) -> StatsSnapshotDto {
    use core_adapters::stats::CounterSample;
    with_hub(|h| {
        let now = Instant::now();
        let samples = vec![
            CounterSample {
                tag: "proxy".to_string(),
                up: proxy_up,
                down: proxy_down,
            },
            CounterSample {
                tag: "direct".to_string(),
                up: 0,
                down: 0,
            },
        ];
        h.stats
            .apply(&samples, generation, epoch_day(now_unix()), now);
        // Apply again after the throttle window so a delta is visible in
        // tests that call this twice.
        let proxy = h.stats.session_proxy();
        let direct = h.stats.session_direct();
        StatsSnapshotDto {
            ok: true,
            enabled: h.stats.is_enabled(),
            display_speed: h.stats.display_speed(),
            generation: h.stats.generation(),
            proxy_up: proxy.up,
            proxy_down: proxy.down,
            direct_up: direct.up,
            direct_down: direct.down,
            nodes: h.node_dtos(),
            error: None,
        }
    })
}

#[frb(ignore)]
pub fn log_subscriber_count_for_test() -> u32 {
    with_hub(|h| h.log_subscribers.len() as u32)
}

#[frb(ignore)]
pub fn traffic_subscriber_count_for_test() -> u32 {
    with_hub(|h| h.traffic_subscribers.len() as u32)
}

#[frb(ignore)]
pub fn inject_log_batch_for_test(entries: Vec<(String, bool)>) {
    let payload = serde_json::json!({
        "entries": entries
            .into_iter()
            .map(|(text, truncated)| serde_json::json!({ "text": text, "truncated": truncated }))
            .collect::<Vec<_>>()
    });
    let event = EventEnvelope::new(
        domain::event::EventEpoch(1),
        domain::event::EventSeq(1),
        EventKind::LogBatch,
        payload,
    );
    ingest_runtime_event(&event);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The monitor hub is process-global; serialize these tests like the
    /// engine tests do so parallel runs cannot interleave configuration.
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn setup() {
        reset_monitor_for_test();
        set_clash_base_for_test(None);
    }

    #[test]
    fn configure_round_trips_and_flags() {
        let _guard = lock();
        setup();
        let result = monitor_configure(
            CoreType::SingBox.value(),
            11809,
            11810,
            None,
            true,
            true,
            2_000,
        );
        assert!(result.ok);
        assert!(monitor_enabled());
        let snap = stats_snapshot();
        assert!(snap.enabled && snap.display_speed);
    }

    #[test]
    fn test_injection_updates_snapshot() {
        let _guard = lock();
        setup();
        monitor_configure(CoreType::Xray.value(), 11809, 0, None, true, true, 2_000);
        monitor_set_active_node(Some("n1".into()));
        // First sample establishes the baseline.
        ingest_stats_for_test(100, 200, 0);
        let snap = ingest_stats_for_test(300, 600, 0);
        assert_eq!(snap.proxy_up, 200);
        assert_eq!(snap.proxy_down, 400);
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].index_id, "n1");
        assert_eq!(snap.nodes[0].total_up, 200);

        // Clear resets.
        assert!(clear_stats().ok);
        assert_eq!(stats_snapshot().nodes.len(), 0);
    }

    #[test]
    fn log_pipeline_filters_and_pages() {
        let _guard = lock();
        setup();
        set_log_filter(3, vec![], vec![]); // Warn+
        ingest_log_text_for_test("info dropped");
        ingest_log_text_for_test("warn kept");
        ingest_log_text_for_test("error kept");
        let page = get_logs(0, 10);
        assert!(page.ok);
        assert_eq!(page.lines.len(), 2);
        assert_eq!(page.lines[0].text, "warn kept");
        assert_eq!(page.lines[0].level, 3);

        assert!(set_log_pause(true, true).ok);
        let page = get_logs(0, 10);
        assert!(page.collecting_paused && page.scroll_paused);
        assert!(clear_logs().ok);
        assert_eq!(get_logs(0, 10).total, 0);
    }

    #[test]
    fn injected_log_batch_reaches_service() {
        let _guard = lock();
        setup();
        inject_log_batch_for_test(vec![
            ("[Info] hello".to_string(), false),
            ("[Error] boom".to_string(), true),
        ]);
        let page = get_logs(0, 10);
        assert_eq!(page.lines.len(), 2);
        assert_eq!(page.truncated_lines, 1);
    }

    #[test]
    fn paused_collection_drops_incoming_batch() {
        let _guard = lock();
        setup();
        assert!(set_log_pause(true, false).ok);
        inject_log_batch_for_test(vec![("dropped while paused".to_string(), false)]);
        assert_eq!(log_total(), 0);

        assert!(set_log_pause(false, false).ok);
        inject_log_batch_for_test(vec![("kept after resume".to_string(), false)]);
        assert_eq!(log_total(), 1);
    }

    #[test]
    fn unsupported_core_reports_clash_not_available() {
        let _guard = lock();
        setup();
        monitor_configure(CoreType::Xray.value(), 11809, 0, None, false, false, 2_000);
        assert!(!clash_supported());
        // Async DTO path is exercised by the application-layer mock tests; the
        // sync support flag is the decisive UI branch.
    }

    #[test]
    fn page_visibility_uses_refresh_interval() {
        let _guard = lock();
        setup();
        monitor_configure(
            CoreType::SingBox.value(),
            11809,
            11810,
            None,
            true,
            true,
            3_000,
        );
        let page = set_page_visible("proxies".to_string(), true);
        assert!(page.visible && page.subscribed);
        assert_eq!(page.refresh_interval_ms, 3_000);
        let page = set_page_visible("unknown".to_string(), true);
        assert!(!page.visible);
    }

    #[test]
    fn store_bind_failure_is_visible_and_retryable() {
        let _guard = lock();
        let mut hub = MonitorHub::new();
        let dir_a = PathBuf::from("synthetic-data-a");
        let dir_b = PathBuf::from("synthetic-data-b");

        // Open failure: not reported as bound, error visible, retried later.
        rebind_store(&mut hub, Some(dir_a.clone()), || None);
        assert!(!hub.store_bound);
        assert_eq!(
            hub.store_error.as_ref().map(|e| e.code.as_str()),
            Some(STORE_BIND_ERROR_CODE)
        );

        // A retry with a working store binds and clears the error.
        rebind_store(&mut hub, Some(dir_a), || {
            Some(Box::new(InMemoryTrafficStore::new()) as Box<dyn TrafficStore>)
        });
        assert!(hub.store_bound);
        assert!(hub.store_error.is_none());

        // Changing the engine data dir drops the old binding and re-attempts.
        rebind_store(&mut hub, Some(dir_b), || None);
        assert!(!hub.store_bound);
        assert!(hub.store_error.is_some());

        // An in-memory engine needs no store and must not loop forever.
        rebind_store(&mut hub, None, || None);
        assert!(hub.store_bound);
        assert!(hub.store_error.is_none());
    }

    // -----------------------------------------------------------------------
    // R3-06: the production hub must continuously drive the sing-box WS task.
    //
    // The mock here is deliberately std-only (a hand-rolled WebSocket server):
    // `bridge_api` has no tungstennite/net dependency and the point is to drive
    // the real `monitor_start_polling` entry, not a test-only source.
    // -----------------------------------------------------------------------

    /// Bind the first free `127.0.0.1` port `>= 11808`.
    fn bind_floor_listener() -> std::net::TcpListener {
        (11808u16..13000)
            .find_map(|port| {
                if port == 10_808 {
                    return None;
                }
                std::net::TcpListener::bind(("127.0.0.1", port)).ok()
            })
            .expect("no free loopback port >= 11808")
    }

    struct TrafficWsServer {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl TrafficWsServer {
        fn start() -> Self {
            let listener = bind_floor_listener();
            let port = listener.local_addr().expect("ws addr").port();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = std::sync::Arc::clone(&stop);
            let handle = std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    let Ok((stream, _)) = listener.accept() else {
                        return;
                    };
                    let _ = serve_ws_connection(stream, &flag);
                }
            });
            Self {
                port,
                stop,
                handle: Some(handle),
            }
        }
    }

    impl Drop for TrafficWsServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            // Wake the blocking `accept()` so the thread observes `stop`.
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn serve_ws_connection(
        mut stream: std::net::TcpStream,
        stop: &std::sync::atomic::AtomicBool,
    ) -> std::io::Result<()> {
        use std::io::{Read, Write};
        stream.set_read_timeout(Some(Duration::from_millis(200)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        let mut request = Vec::new();
        let mut buf = [0u8; 1024];
        while !request.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf)?;
            if n == 0 || request.len() > 16 * 1024 {
                return Ok(());
            }
            request.extend_from_slice(&buf[..n]);
        }
        let text = String::from_utf8_lossy(&request);
        let Some(key) = text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("sec-websocket-key")
                .then(|| value.trim().to_string())
        }) else {
            return Ok(());
        };
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            ws_accept_key(&key)
        );
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        // Server->client frames are unmasked; push a non-zero delta forever.
        while !stop.load(Ordering::SeqCst) {
            if stream
                .write_all(&ws_text_frame(r#"{"up":100,"down":200}"#))
                .is_err()
            {
                return Ok(());
            }
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(60));
        }
        Ok(())
    }

    fn ws_text_frame(body: &str) -> Vec<u8> {
        let payload = body.as_bytes();
        assert!(payload.len() < 126);
        let mut frame = Vec::with_capacity(payload.len() + 2);
        frame.push(0x81);
        frame.push(payload.len() as u8);
        frame.extend_from_slice(payload);
        frame
    }

    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    fn ws_accept_key(key: &str) -> String {
        let mut data = Vec::with_capacity(key.len() + WS_GUID.len());
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(WS_GUID.as_bytes());
        base64(&sha1(&data))
    }

    #[allow(clippy::needless_range_loop)]
    fn sha1(data: &[u8]) -> [u8; 20] {
        let mut h: [u32; 5] = [
            0x6745_2301,
            0xEFCD_AB89,
            0x98BA_DCFE,
            0x1032_5476,
            0xC3D2_E1F0,
        ];
        let bit_len = (data.len() as u64) * 8;
        let mut msg = data.to_vec();
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[4 * i],
                    chunk[4 * i + 1],
                    chunk[4 * i + 2],
                    chunk[4 * i + 3],
                ]);
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }
            let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
            for (i, wi) in w.iter().enumerate() {
                let (f, k) = match i {
                    0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999u32),
                    20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                    40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                    _ => (b ^ c ^ d, 0xCA62_C1D6),
                };
                let temp = a
                    .rotate_left(5)
                    .wrapping_add(f)
                    .wrapping_add(e)
                    .wrapping_add(k)
                    .wrapping_add(*wi);
                e = d;
                d = c;
                c = b.rotate_left(30);
                b = a;
                a = temp;
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
        }
        let mut out = [0u8; 20];
        for (i, word) in h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn base64(data: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(TABLE[((n >> 18) & 63) as usize] as char);
            out.push(TABLE[((n >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(TABLE[((n >> 6) & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(TABLE[(n & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    #[test]
    fn production_hub_drives_singbox_ws_statistics() {
        let _guard = lock();
        setup();
        let server = TrafficWsServer::start();
        // `monitor_start_polling` starts the process-global thread and syncs it
        // from the engine (no session here), so configure the hub after it.
        monitor_start_polling();
        monitor_configure(
            CoreType::SingBox.value(),
            0,
            server.port as u32,
            None,
            true,
            false,
            200,
        );
        monitor_set_active_node(Some("r3-06-node".into()));

        let mut observed = 0u64;
        for _ in 0..240 {
            observed = observed.max(stats_snapshot().proxy_up);
            if observed > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            observed > 0,
            "production hub never collected a non-zero WS delta"
        );
        let snap = stats_snapshot();
        assert!(
            snap.nodes
                .iter()
                .any(|n| n.index_id == "r3-06-node" && n.total_up > 0),
            "active node did not receive the proxy attribution"
        );
    }

    // -----------------------------------------------------------------------
    // R4-23: the connection list must come from a real Clash HTTP controller
    // (here a local mock on a port >= 11808), and closing a connection must hit
    // the same API with the id.
    // -----------------------------------------------------------------------

    const CONN_JSON: &str = r#"{"downloadTotal":200,"uploadTotal":100,"connections":[{"id":"c1","upload":10,"download":20,"start":"2026-10-05T00:00:00Z","chains":["PROXY","DIRECT"],"rule":"MATCH","metadata":{"host":"example.com","network":"tcp","type":"HTTP","sourceIP":"127.0.0.1","sourcePort":"12345","destinationIP":"127.0.0.1","destinationPort":"443","processPath":"C:/app.exe"}}]}"#;

    struct ClashHttpMock {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        closed: std::sync::Arc<Mutex<Vec<String>>>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl ClashHttpMock {
        fn start() -> Self {
            let listener = bind_floor_listener();
            let port = listener.local_addr().expect("clash mock addr").port();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let closed = std::sync::Arc::new(Mutex::new(Vec::new()));
            let flag = std::sync::Arc::clone(&stop);
            let closed_out = std::sync::Arc::clone(&closed);
            let handle = std::thread::spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    let _ = serve_clash_http(&mut stream, &closed_out);
                }
            });
            Self {
                port,
                stop,
                closed,
                handle: Some(handle),
            }
        }

        fn base(&self) -> String {
            format!("http://127.0.0.1:{}", self.port)
        }
    }

    impl Drop for ClashHttpMock {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn serve_clash_http(
        stream: &mut std::net::TcpStream,
        closed: &Mutex<Vec<String>>,
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
            ("GET", "/connections") => ("200 OK", CONN_JSON.to_string()),
            ("DELETE", p) if p.starts_with("/connections") => {
                let id = p.trim_start_matches("/connections").trim_matches('/');
                if !id.is_empty() {
                    closed
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(id.to_string());
                }
                ("204 No Content", String::new())
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

    #[test]
    fn clash_connections_mock_lists_and_closes_over_floor_port() {
        let _guard = lock();
        setup();
        let mock = ClashHttpMock::start();
        assert!(mock.port >= 11808 && mock.port != 10_808);

        monitor_configure(CoreType::SingBox.value(), 0, 0, None, true, false, 2_000);
        set_clash_base_for_test(Some(mock.base()));

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");

        let conns = rt.block_on(clash_connections());
        assert!(conns.ok && conns.supported);
        assert_eq!(conns.upload_total, 100);
        assert_eq!(conns.download_total, 200);
        assert_eq!(conns.items.len(), 1);
        assert_eq!(conns.items[0].id, "c1");
        assert_eq!(conns.items[0].host.as_deref(), Some("example.com"));
        assert_eq!(conns.items[0].chains, vec!["PROXY", "DIRECT"]);

        let closed = rt.block_on(close_clash_connection("c1".to_string()));
        assert!(closed.ok);
        assert_eq!(
            mock.closed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_slice(),
            ["c1"]
        );

        let all = rt.block_on(close_all_clash_connections());
        assert!(all.ok);
        set_clash_base_for_test(None);
    }
}
