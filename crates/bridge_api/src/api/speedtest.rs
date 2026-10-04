//! T15b FRB surface: speedtest jobs and the `ProfileExItem` result closure.
//!
//! TCPing runs entirely in `application` (direct TCP). RealPing / download /
//! mixed tests open a **restricted temporary core through net-host**
//! (`IpcOperation::TestSession`), on an isolated port `>= 11808`, and measure
//! through its local SOCKS proxy. Nothing here starts a kernel directly.
//!
//! The hub keeps one process-global result table (`ProfileExItem`) plus the
//! last batch stream. Batches are pushed through an FRB [`StreamSink`]; the
//! worker thread is joined-by-token (cancel is cooperative at safe points).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use application::codegen::{build_input, generate, CodegenOptions};
use application::net_host_client::NetHostClient;
use application::speedtest::{
    http_get_via_socks, release_test_port, reserve_free_test_port, DownloadOutcome, ProbeError,
    ProfileExStore, SpeedTestJobs, SpeedTestResult, SpeedTestRunner, SpeedTestSession,
    SpeedTestSettings, SpeedTestSnapshot, TestNode, TestSession, TlsTrust,
};
use domain::{
    ConfigSource, ContentHash, CoreType, DomainError, NetworkPolicy, OutboundGraph, PortRequest,
    ProcessGraph, RuntimePlan, RuntimeTarget, SpeedTestAction,
};
use flutter_rust_bridge::frb;

use crate::api::contract::{ErrorDto, SimpleResult};
use crate::api::engine::engine;
use crate::frb_generated::StreamSink;

/// Result row delivered to the UI (`ProfileExItem`).
#[derive(Clone)]
pub struct SpeedTestResultDto {
    pub index_id: String,
    pub delay: i32,
    pub speed: f64,
    pub message: String,
    pub ip_info: String,
}

/// One 50-100 ms result batch.
#[derive(Clone)]
pub struct SpeedTestBatchDto {
    pub job_id: String,
    pub kind: i32,
    pub results: Vec<SpeedTestResultDto>,
    pub done: bool,
    pub cancelled: bool,
}

/// Result of `speedtest_start`.
#[derive(Clone)]
pub struct SpeedTestStartDto {
    pub ok: bool,
    pub job_id: Option<String>,
    pub total: u32,
    pub error: Option<ErrorDto>,
}

/// Which test actions the current build can really perform.
#[derive(Clone)]
pub struct SpeedTestSupportDto {
    pub tcp_ping: bool,
    pub real_ping: bool,
    pub download: bool,
    pub mixed: bool,
    pub fast_real_ping: bool,
    /// UDP latency test (RE-PROF-08). Supported on every desktop build through
    /// a direct datagram probe; the entry stays visible and reports the reason
    /// when the target/platform cannot answer instead of being permanently
    /// disabled.
    pub udp: bool,
}

/// Result of a full client-config export (upstream
/// `CoreConfigHandler.GenerateClientConfig` / `Export2ClientConfigAsync`).
#[derive(Clone)]
pub struct ExportClientConfigDto {
    pub ok: bool,
    pub text: String,
    pub core_type: String,
    pub file_name: String,
    pub error: Option<ErrorDto>,
}

fn action_from_value(value: i32) -> Option<SpeedTestAction> {
    Some(match value {
        0 => SpeedTestAction::Tcping,
        1 => SpeedTestAction::Realping,
        2 => SpeedTestAction::UdpTest,
        3 => SpeedTestAction::Speedtest,
        4 => SpeedTestAction::Mixedtest,
        5 => SpeedTestAction::FastRealping,
        _ => return None,
    })
}

fn action_value(action: SpeedTestAction) -> i32 {
    match action {
        SpeedTestAction::Tcping => 0,
        SpeedTestAction::Realping => 1,
        SpeedTestAction::UdpTest => 2,
        SpeedTestAction::Speedtest => 3,
        SpeedTestAction::Mixedtest => 4,
        SpeedTestAction::FastRealping => 5,
    }
}

fn default_core() -> CoreType {
    CoreType::Xray
}

// ---------------------------------------------------------------------------
// Restricted test session (net-host)
// ---------------------------------------------------------------------------

/// Production [`SpeedTestSession`]: every node runs in its own temporary
/// net-host-managed core on a fresh `>= 11808` port.
struct NetHostTestSession {
    client: NetHostClient,
}

impl NetHostTestSession {
    fn new() -> Self {
        Self {
            client: NetHostClient::new(),
        }
    }
}

impl SpeedTestSession for NetHostTestSession {
    fn open(&self, node: &TestNode) -> Result<TestSession, DomainError> {
        // Reserve a port block up front so concurrent per-node sessions never
        // race for the same ephemeral port (`error.port_conflict`). Every error
        // path releases the reservation; `close` releases the successful one.
        let port = reserve_free_test_port()
            .ok_or_else(|| DomainError::new(domain::codes::UNAVAILABLE, "error.no_free_port"))?;
        match self.open_reserved(node, port) {
            Ok(session) => Ok(session),
            Err(error) => {
                release_test_port(port);
                Err(error)
            }
        }
    }

    fn real_ping(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        trust: &TlsTrust,
        ct: &domain::CancellationToken,
    ) -> Result<i32, ProbeError> {
        let mut best: Option<i32> = None;
        for _ in 0..2 {
            if ct.is_cancelled() {
                return Err(ProbeError::new(
                    application::speedtest::ProbeFailureKind::Cancelled,
                    "cancelled",
                ));
            }
            if let Ok(probe) = http_get_via_socks(session.port, url, trust, timeout, 0, ct) {
                if probe.success {
                    let ms = (probe.header_ms as i32).max(1);
                    best = Some(best.map_or(ms, |b| b.min(ms)));
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        best.ok_or_else(|| {
            ProbeError::new(
                application::speedtest::ProbeFailureKind::Connect,
                "no sample",
            )
        })
    }

    fn download(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        max_bytes: u64,
        trust: &TlsTrust,
        ct: &domain::CancellationToken,
    ) -> Result<DownloadOutcome, ProbeError> {
        let probe = http_get_via_socks(session.port, url, trust, timeout, max_bytes, ct)?;
        let secs = probe.total.as_secs_f64().max(0.001);
        Ok(DownloadOutcome {
            mb_s: probe.body_bytes as f64 / 1_000_000.0 / secs,
            bytes: probe.body_bytes,
            elapsed: probe.total,
        })
    }

    /// UDP latency test. Restricted-scope implementation: a direct datagram
    /// round trip to the effective `UdpTestTarget` (protocol keyword or
    /// `host:port`). It does not route through the session's SOCKS UDP
    /// associate; node-routed UDP remains an isolated acceptance item
    /// (RE-PROF-08).
    fn udp_ping(
        &self,
        _session: &TestSession,
        target: &str,
        timeout: Duration,
        ct: &domain::CancellationToken,
    ) -> Option<i32> {
        application::speedtest::udp_ping(target, timeout, ct)
    }

    fn close(&self, session: TestSession) {
        let _ = self.client.close_test_session(&session.handle_id);
        release_test_port(session.port);
    }
}

impl NetHostTestSession {
    /// Codegen + net-host open for an already-reserved port.
    fn open_reserved(&self, node: &TestNode, port: u16) -> Result<TestSession, DomainError> {
        let profile = engine()
            .profile_by_id(&node.index_id)?
            .ok_or_else(|| DomainError::not_found("profile", &node.index_id))?;
        let core = profile.core_type.unwrap_or_else(default_core);
        let opts = CodegenOptions {
            local_port: port as i32,
            state_port: port.saturating_add(1) as i32,
            state_port2: port.saturating_add(2) as i32,
            ..CodegenOptions::default()
        };
        let input = build_input(
            &profile,
            std::slice::from_ref(&profile),
            None,
            BTreeMap::new(),
            None,
            &opts,
        );
        let generated = generate(core, &input).map_err(|e| {
            DomainError::new(domain::codes::INVALID_PLAN, "error.codegen_failed")
                .with_detail(e.to_string())
        })?;
        let body = serde_json::to_string(&generated.main).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.codegen_failed")
                .with_detail(e.to_string())
        })?;
        let plan = RuntimePlan {
            plan_id: format!("speedtest-{}", node.index_id),
            desired_revision: 0,
            target: RuntimeTarget {
                core_type: core,
                version: None,
                config: ConfigSource::Inline { body },
                // Hash left empty: net-host then trusts the staged body.
                config_sha256: ContentHash::new(""),
            },
            process_graph: ProcessGraph::default(),
            outbound_graph: OutboundGraph::default(),
            ports: vec![PortRequest::tcp(port, "speedtest")],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        };
        let session_id = self.client.open_test_session(&plan, 30_000)?;
        Ok(TestSession {
            node: node.clone(),
            port,
            handle_id: session_id,
        })
    }
}

// ---------------------------------------------------------------------------
// Hub
// ---------------------------------------------------------------------------

struct SpeedTestHub {
    jobs: SpeedTestJobs,
    results: ProfileExStore,
    settings: SpeedTestSettings,
    source_revision: u64,
    subscribers: Vec<StreamSink<SpeedTestBatchDto>>,
    /// Whether the persisted `ProfileExItem` table has been loaded once.
    loaded: bool,
}

impl SpeedTestHub {
    fn new() -> Self {
        Self {
            jobs: SpeedTestJobs::new(),
            results: ProfileExStore::new(),
            settings: SpeedTestSettings::default(),
            source_revision: 0,
            subscribers: Vec::new(),
            loaded: false,
        }
    }
}

/// Load the persisted `ProfileExItem` table into the hub exactly once.
///
/// A reopened process must show the previous run's delay/speed/sort (PR-17).
/// The engine owns the SQLite store; the overlay is reloaded lazily on the
/// first read so an engine without data (pure unit tests) stays in-memory.
fn ensure_profile_ex_loaded(h: &mut SpeedTestHub) {
    if h.loaded {
        return;
    }
    h.loaded = true;
    if let Ok(rows) = engine().profile_ex_all() {
        if !rows.is_empty() {
            h.results.replace_all(rows);
        }
    }
}

/// Persist the hub's `ProfileExItem` table through the engine (SQLite).
fn flush_profile_ex(h: &SpeedTestHub) {
    let _ = engine().profile_ex_flush(&h.results.all());
}

fn hub() -> &'static Mutex<SpeedTestHub> {
    static HUB: OnceLock<Mutex<SpeedTestHub>> = OnceLock::new();
    HUB.get_or_init(|| Mutex::new(SpeedTestHub::new()))
}

fn with_hub<T>(f: impl FnOnce(&mut SpeedTestHub) -> T) -> T {
    let mut guard = hub().lock().unwrap_or_else(|p| p.into_inner());
    f(&mut guard)
}

fn dto(result: &SpeedTestResult) -> SpeedTestResultDto {
    SpeedTestResultDto {
        index_id: result.index_id.clone(),
        delay: result.delay.unwrap_or(0),
        speed: result.speed.unwrap_or(0.0),
        message: result.message.clone().unwrap_or_default(),
        ip_info: result.ip_info.clone().unwrap_or_default(),
    }
}

fn from_ex(row: &application::speedtest::ProfileExItem) -> SpeedTestResultDto {
    SpeedTestResultDto {
        index_id: row.index_id.clone(),
        delay: row.delay,
        speed: row.speed,
        message: row.message.clone(),
        ip_info: row.ip_info.clone(),
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Apply the effective `SpeedTestItem` settings.
#[frb(sync)]
#[allow(clippy::too_many_arguments)]
pub fn speedtest_configure(
    page_size: u32,
    mixed_concurrency: u32,
    timeout_secs: u32,
    speed_test_url: String,
    speed_ping_test_url: String,
    ipapi_url: Option<String>,
    udp_test_target: Option<String>,
    delay_interval_secs: u32,
) -> SimpleResult {
    with_hub(|h| {
        h.settings = SpeedTestSettings {
            page_size: page_size.max(1) as usize,
            mixed_concurrency: mixed_concurrency.max(1) as usize,
            timeout: Duration::from_secs(timeout_secs.max(10) as u64),
            speed_test_url,
            speed_ping_test_url,
            ipapi_url: ipapi_url.filter(|s| !s.trim().is_empty()),
            udp_test_target: udp_test_target.filter(|s| !s.trim().is_empty()),
            delay_interval: Duration::from_secs(delay_interval_secs as u64),
        };
        SimpleResult {
            ok: true,
            error: None,
        }
    })
}

/// What this build can really do. UDP is supported by the direct loopback
/// probe (RE-PROF-08); a failed target is reported as a structured failure,
/// never a fake delay.
#[frb(sync)]
pub fn speedtest_supported() -> SpeedTestSupportDto {
    SpeedTestSupportDto {
        tcp_ping: true,
        real_ping: true,
        download: true,
        mixed: true,
        fast_real_ping: true,
        udp: true,
    }
}

// ---------------------------------------------------------------------------
// Start / cancel
// ---------------------------------------------------------------------------

fn profile_to_node(profile: &domain::Profile) -> Option<TestNode> {
    if profile.config_type.is_complex()
        || profile.config_type == domain::ConfigType::Outbound
        || profile.port <= 0
        || profile.address.trim().is_empty()
    {
        return None;
    }
    Some(TestNode {
        index_id: profile.index_id.clone(),
        address: profile.address.clone(),
        port: profile.port,
        config_type: profile.config_type.value(),
        core_type: profile.core_type.unwrap_or_else(default_core).value(),
    })
}

/// Start a speedtest job over an explicit node id set.
///
/// The UI resolves the scope itself (Mixed/Fast = the current group's filtered
/// list; the other actions = the selection), so an empty set means "nothing to
/// test" and never widens to the whole database (PR-16).
#[frb(sync)]
pub fn speedtest_start(kind: i32, index_ids: Vec<String>) -> SpeedTestStartDto {
    let Some(action) = action_from_value(kind) else {
        return SpeedTestStartDto {
            ok: false,
            job_id: None,
            total: 0,
            error: Some(ErrorDto::from(DomainError::invalid_enum(
                "SpeedTestAction",
                kind.to_string(),
            ))),
        };
    };

    let filter: std::collections::HashSet<&str> = index_ids.iter().map(String::as_str).collect();
    let test_nodes: Vec<TestNode> = {
        let page = engine().query_profiles(
            application::ProfileFilter::default(),
            application::ProfileSort::IndexId,
            application::PageRequest {
                cursor: 0,
                page_size: 100_000,
            },
        );
        match page {
            Ok(page) => page
                .items
                .iter()
                .filter(|profile| filter.contains(profile.index_id.as_str()))
                .filter_map(profile_to_node)
                .collect(),
            Err(error) => {
                return SpeedTestStartDto {
                    ok: false,
                    job_id: None,
                    total: 0,
                    error: Some(ErrorDto::from(error)),
                }
            }
        }
    };
    if test_nodes.is_empty() {
        return SpeedTestStartDto {
            ok: true,
            job_id: None,
            total: 0,
            error: None,
        };
    }

    let source_revision = engine().desired_revision();
    let (settings, job) = with_hub(|h| {
        h.source_revision = source_revision;
        let job = h.jobs.start(
            action,
            SpeedTestSnapshot {
                source_revision,
                nodes: test_nodes.clone(),
            },
        );
        (h.settings.clone(), job)
    });

    let job_id = job.job_id.as_str().to_string();
    let job_id_out = job_id.clone();
    let total = test_nodes.len() as u32;
    let token = job.token.clone();
    let kind_value = action_value(action);
    let runner = SpeedTestRunner::new(settings, Arc::new(NetHostTestSession::new()));

    let _ = std::thread::Builder::new()
        .name("speedtest-job".into())
        .spawn(move || {
            let job_id_for_batch = job_id.clone();
            let outcome = runner.run(action, &test_nodes, &token, |batch| {
                let dtos: Vec<SpeedTestResultDto> = batch.iter().map(dto).collect();
                with_hub(|h| {
                    h.results.apply_all(batch);
                    let dto = SpeedTestBatchDto {
                        job_id: job_id_for_batch.clone(),
                        kind: kind_value,
                        results: dtos.clone(),
                        done: false,
                        cancelled: false,
                    };
                    h.subscribers.retain(|sink| sink.add(dto.clone()).is_ok());
                });
            });

            with_hub(|h| {
                h.jobs.finish(&job_id_for_batch);
                flush_profile_ex(h);
                let final_batch = SpeedTestBatchDto {
                    job_id: job_id_for_batch.clone(),
                    kind: kind_value,
                    results: Vec::new(),
                    done: true,
                    cancelled: outcome.cancelled(),
                };
                h.subscribers
                    .retain(|sink| sink.add(final_batch.clone()).is_ok());
            });
        });

    SpeedTestStartDto {
        ok: true,
        job_id: Some(job_id_out),
        total,
        error: None,
    }
}

/// Idempotent cancel of a running speedtest job.
#[frb(sync)]
pub fn speedtest_cancel(job_id: String) -> SimpleResult {
    let cancelled = with_hub(|h| h.jobs.cancel(&job_id));
    SimpleResult {
        ok: cancelled,
        error: None,
    }
}

/// All `ProfileExItem` result rows (UI overlay).
#[frb(sync)]
pub fn speedtest_results() -> Vec<SpeedTestResultDto> {
    with_hub(|h| {
        ensure_profile_ex_loaded(h);
        h.results.all().iter().map(from_ex).collect()
    })
}

/// Number of speedtest jobs still running. The UI polls this to end live
/// refresh without depending on the FRB stream lifecycle.
#[frb(sync)]
pub fn speedtest_active_jobs() -> u32 {
    with_hub(|h| h.jobs.active_count() as u32)
}

/// `RemoveInvalidServerResult`: prune result rows whose delay failed (`-1`)
/// **and** whose profile no longer exists. The real `ProfileItem` deletion of
/// the current group is issued by the UI through `delete_profiles`; this only
/// drops the now-orphaned result rows. Another group's failed node is still
/// stored, so its evidence survives, and a failed/partial delete keeps the
/// current group's evidence too (RE-PROF-06).
#[frb(sync)]
pub fn speedtest_remove_invalid() -> u32 {
    with_hub(|h| {
        ensure_profile_ex_loaded(h);
        let stored: std::collections::HashSet<String> = engine()
            .query_profiles(
                application::ProfileFilter::default(),
                application::ProfileSort::IndexId,
                application::PageRequest {
                    cursor: 0,
                    page_size: u32::MAX,
                },
            )
            .map(|page| page.items.into_iter().map(|p| p.index_id).collect())
            .unwrap_or_default();
        let stored_refs: std::collections::HashSet<&str> =
            stored.iter().map(String::as_str).collect();
        let removed = h.results.remove_invalid_orphans(&stored_refs) as u32;
        flush_profile_ex(h);
        removed
    })
}

/// RE-PROF-06 group-scoped variant: delete the current group's failed,
/// non-complex `ProfileItem`s (reusing `AppEngine::remove_invalid_profiles`)
/// and prune only those result rows, preserving other groups' evidence.
///
/// This is a **new FRB surface**: `frb_generated.*` must be regenerated by the
/// root agent before Dart can call it. Until then the live Dart path uses
/// [`speedtest_remove_invalid`] (orphan-prune), which is equivalent once the
/// UI has already deleted the targeted profiles. Returns the deleted profile
/// count, or `-1` on storage failure (upstream `RemoveInvalidServerResult`).
#[frb(sync)]
pub fn speedtest_remove_invalid_group(subid: String) -> i32 {
    match engine().remove_invalid_profiles(&subid) {
        Ok(removed) => {
            with_hub(|h| {
                ensure_profile_ex_loaded(h);
                flush_profile_ex(h);
            });
            removed as i32
        }
        Err(_) => -1,
    }
}

/// Persist the table's display order (PR-15): rewrite `ProfileExItem.Sort`
/// for the full id list and flush, mirroring upstream `MoveServer` /
/// `SortServers`. Ids missing from the result table get a sort-only row so a
/// later test result still lands on the right order.
#[frb(sync)]
pub fn speedtest_apply_profile_order(ordered_ids: Vec<String>) -> SimpleResult {
    with_hub(|h| {
        ensure_profile_ex_loaded(h);
        h.results.apply_order(&ordered_ids);
        flush_profile_ex(h);
    });
    SimpleResult {
        ok: true,
        error: None,
    }
}

/// `export_client_config` — generate the complete client config text for one
/// node, upstream `Export2ClientConfigAsync` / `Export2ClientConfigResult`.
///
/// This is a **new FRB surface**: `frb_generated.*` must be regenerated by the
/// root agent before Dart can call it. The text is produced by the same
/// `AppEngine::build_codegen_input` + `generate` path the runtime plan uses, so
/// it honors the persisted settings, routing, DNS and template. Pretty-printed
/// for a readable saved file / clipboard text.
#[frb(sync)]
pub fn export_client_config(index_id: String) -> ExportClientConfigDto {
    let profile = match engine().profile_by_id(&index_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return ExportClientConfigDto {
                ok: false,
                text: String::new(),
                core_type: String::new(),
                file_name: String::new(),
                error: Some(ErrorDto::from(DomainError::not_found("profile", &index_id))),
            }
        }
        Err(error) => {
            return ExportClientConfigDto {
                ok: false,
                text: String::new(),
                core_type: String::new(),
                file_name: String::new(),
                error: Some(ErrorDto::from(error)),
            }
        }
    };
    let core = profile.core_type.unwrap_or_else(default_core);
    let opts = engine().runtime_codegen_options();
    let input = match engine().build_codegen_input(&index_id, core, &opts) {
        Ok(input) => input,
        Err(error) => {
            return ExportClientConfigDto {
                ok: false,
                text: String::new(),
                core_type: core.value().to_string(),
                file_name: String::new(),
                error: Some(ErrorDto::from(error)),
            }
        }
    };
    let generated = match generate(core, &input) {
        Ok(generated) => generated,
        Err(error) => {
            return ExportClientConfigDto {
                ok: false,
                text: String::new(),
                core_type: core.value().to_string(),
                file_name: String::new(),
                error: Some(
                    DomainError::new(domain::codes::INVALID_PLAN, "error.codegen_failed")
                        .with_detail(error.to_string())
                        .into(),
                ),
            }
        }
    };
    match serde_json::to_string_pretty(&generated.main) {
        Ok(text) => ExportClientConfigDto {
            ok: true,
            text,
            core_type: core.value().to_string(),
            file_name: format!("{}.json", sanitize_export_name(&index_id)),
            error: None,
        },
        Err(error) => ExportClientConfigDto {
            ok: false,
            text: String::new(),
            core_type: core.value().to_string(),
            file_name: String::new(),
            error: Some(
                DomainError::new(domain::codes::INTERNAL, "error.config_serialize_failed")
                    .with_detail(error.to_string())
                    .into(),
            ),
        },
    }
}

fn sanitize_export_name(index_id: &str) -> String {
    let mut out = String::with_capacity(index_id.len());
    for ch in index_id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "config".to_string()
    } else {
        out
    }
}

/// Register a batch stream.
pub fn speedtest_subscribe(sink: StreamSink<SpeedTestBatchDto>) {
    with_hub(|h| h.subscribers.push(sink));
}

// ---------------------------------------------------------------------------
// Test seams (not exported to Dart)
// ---------------------------------------------------------------------------

#[frb(ignore)]
pub fn reset_speedtest_for_test() {
    let mut guard = hub().lock().unwrap_or_else(|p| p.into_inner());
    *guard = SpeedTestHub::new();
}

#[frb(ignore)]
pub fn apply_speedtest_result_for_test(
    index_id: String,
    delay: i32,
    speed: f64,
    message: String,
) -> Vec<SpeedTestResultDto> {
    with_hub(|h| {
        ensure_profile_ex_loaded(h);
        h.results.apply(&SpeedTestResult {
            index_id: index_id.clone(),
            delay: Some(delay),
            speed: Some(speed),
            message: Some(message),
            ip_info: None,
            failed: delay <= 0,
        });
        flush_profile_ex(h);
        h.results.all().iter().map(from_ex).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    #[test]
    fn action_mapping_round_trips() {
        for value in 0..=5 {
            let action = action_from_value(value).unwrap();
            assert_eq!(action_value(action), value);
        }
        assert!(action_from_value(99).is_none());
    }

    #[test]
    fn result_overlay_and_invalid_cleanup() {
        let _guard = lock();
        reset_speedtest_for_test();
        let rows = apply_speedtest_result_for_test("a".into(), 12, 8.5, "8.5".into());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].delay, 12);
        let rows = apply_speedtest_result_for_test("b".into(), -1, 0.0, "fail".into());
        assert_eq!(rows.len(), 2);
        assert_eq!(speedtest_remove_invalid(), 1);
        assert_eq!(speedtest_results().len(), 1);
    }

    #[test]
    fn results_follow_persisted_sort_order() {
        // RE-PROF-04: `speedtest_results()` must emit rows in persisted
        // `ProfileExItem.Sort` order so the Dart read chain can restore it.
        let _guard = lock();
        reset_speedtest_for_test();
        apply_speedtest_result_for_test("a".into(), 12, 1.0, String::new());
        apply_speedtest_result_for_test("b".into(), 20, 1.0, String::new());
        apply_speedtest_result_for_test("c".into(), 30, 1.0, String::new());

        let ids = || -> Vec<String> {
            speedtest_results()
                .into_iter()
                .map(|r| r.index_id)
                .collect()
        };
        assert_eq!(ids(), vec!["a", "b", "c"], "index order before any write");

        let ordered = vec!["c".to_string(), "a".to_string(), "b".to_string()];
        assert!(speedtest_apply_profile_order(ordered).ok);
        assert_eq!(ids(), vec!["c", "a", "b"], "read back in Sort order");

        let flipped = vec!["b".to_string(), "a".to_string(), "c".to_string()];
        assert!(speedtest_apply_profile_order(flipped).ok);
        assert_eq!(ids(), vec!["b", "a", "c"], "direction toggle read-back");
    }

    #[test]
    fn udp_is_supported_by_the_direct_probe() {
        let support = speedtest_supported();
        assert!(support.tcp_ping && support.real_ping && support.download);
        assert!(
            support.udp,
            "RE-PROF-08: UDP is no longer permanently disabled"
        );
    }

    #[test]
    fn export_client_config_reports_missing_profile() {
        let _guard = lock();
        let result = export_client_config("re-prof-08-missing".into());
        assert!(!result.ok);
        assert!(result.text.is_empty());
        assert_eq!(
            result.error.as_ref().map(|e| e.code.as_str()),
            Some(domain::codes::NOT_FOUND)
        );
    }

    #[test]
    fn configure_clamps_timeout_and_concurrency() {
        let _guard = lock();
        reset_speedtest_for_test();
        let result = speedtest_configure(
            10,
            0,
            1,
            "http://x/big".into(),
            "http://x/204".into(),
            Some("  ".into()),
            None,
            1,
        );
        assert!(result.ok);
        with_hub(|h| {
            assert_eq!(h.settings.timeout, Duration::from_secs(10));
            assert_eq!(h.settings.mixed_concurrency, 1);
            assert!(h.settings.ipapi_url.is_none());
        });
    }
}
