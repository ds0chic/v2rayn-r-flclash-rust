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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use application::monitor::{epoch_day, LogService, StatsService};
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

/// Generic result for select/close actions.
#[derive(Clone)]
pub struct MonitorActionResult {
    pub ok: bool,
    pub supported: bool,
    pub message: Option<String>,
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
            error: None,
        }
    })
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
        if lines.is_empty() || h.log_subscribers.is_empty() {
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
}

fn clash_config() -> ClashConfig {
    with_hub(|h| ClashConfig {
        base: h.clash_base_override.clone(),
        port: h.state_port2,
        secret: h.secret.clone(),
        refresh: Duration::from_millis(u64::from(h.refresh_ms.max(200))),
        supported: ClashApiService::supported(h.core),
    })
}

fn build_clash(cfg: &ClashConfig) -> Result<ClashApiService, core_adapters::clash_api::ClashError> {
    match &cfg.base {
        Some(base) => Ok(ClashApiService::from_client(
            ClashApiClient::from_base(base.clone(), cfg.secret.clone(), CLASH_TIMEOUT)?,
            cfg.refresh,
        )),
        None => ClashApiService::new(cfg.port, cfg.secret.clone(), CLASH_TIMEOUT, cfg.refresh),
    }
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
    match service.proxies().await {
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

/// Start the 1 Hz statistics poller once. No-op for unsupported cores.
#[frb(sync)]
pub fn monitor_start_polling() {
    POLL_STARTED.get_or_init(|| {
        let shared = Arc::clone(hub());
        let _ = std::thread::Builder::new()
            .name("monitor-poll".into())
            .spawn(move || poll_loop(shared));
    });
}

fn poll_loop(shared: Arc<Mutex<MonitorHub>>) {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
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
            std::thread::sleep(Duration::from_millis(500));
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
            source = build_source(&rt, core, p1, p2);
        }
        if let Some(src) = source.as_mut() {
            if let Ok(samples) = rt.block_on(src.poll()) {
                let generation = src.generation();
                let today = epoch_day(now_unix());
                let (dto, sinks) = {
                    let mut h = shared.lock().unwrap_or_else(|p| p.into_inner());
                    let update = h.stats.apply(&samples, generation, today, Instant::now());
                    if update.applied {
                        let _ = h.stats.flush_store();
                        (Some(h.traffic_dto()), h.traffic_subscribers.clone())
                    } else {
                        (None, Vec::new())
                    }
                };
                if let Some(dto) = dto {
                    for sink in sinks {
                        let _ = sink.add(dto.clone());
                    }
                }
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn build_source(
    rt: &tokio::runtime::Runtime,
    core: CoreType,
    state_port: u16,
    state_port2: u16,
) -> Option<Box<dyn StatsSource>> {
    match core {
        CoreType::Xray | CoreType::V2fly | CoreType::V2flyV5 => {
            XrayStatsSource::new(state_port, STATS_TIMEOUT)
                .ok()
                .map(|s| Box::new(s) as Box<dyn StatsSource>)
        }
        CoreType::SingBox => rt
            .block_on(async { SingboxTrafficSource::new(TrafficConfig::new(state_port2)) })
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
}
