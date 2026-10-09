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

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use application::codegen::generate;
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
    /// UDP latency test (RE-PROF-08 / R3-PROF-05). Gated on the real
    /// node-routed capability (the SOCKS5 UDP associate probe over the test
    /// core's local port), not the old host-direct datagram. The entry stays
    /// visible and reports the reason when the association/target cannot
    /// answer instead of being permanently disabled.
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

    /// UDP latency test through the node's temporary core: a SOCKS5 UDP
    /// association over `session.port` (upstream `UdpTestService` +
    /// `Socks5UdpChannel`). A host-direct datagram is never surfaced as a node
    /// result, so a returned latency is attributable to the selected node
    /// (R3-PROF-05).
    fn udp_ping(
        &self,
        session: &TestSession,
        target: &str,
        timeout: Duration,
        ct: &domain::CancellationToken,
    ) -> Option<i32> {
        application::speedtest::udp_ping_via_socks(session.port, target, timeout, ct)
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
        let app = engine();
        let core = app.resolve_target_core(&profile)?;
        let mut opts = app.runtime_codegen_options();
        opts.local_port = port as i32;
        opts.state_port = port.saturating_add(1) as i32;
        opts.state_port2 = port.saturating_add(2) as i32;
        let mut input = app.build_codegen_input(&profile.index_id, core, &opts)?;
        // A speed-test core is isolated behind a temporary local proxy and
        // never owns the user's TUN interface.
        input.settings.tun.enabled = false;
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
    /// Per-node run generation of the last accepted result. A batch from an
    /// older generation may never overwrite a node already claimed by a newer
    /// run (R4-22), so a late response from a cancelled job is evicted.
    result_generation: HashMap<String, u64>,
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
            result_generation: HashMap::new(),
        }
    }
}

/// Claim a run's node set: clear its stale results and record the run's
/// generation so no older in-flight run can write them afterwards.
fn begin_job_results(h: &mut SpeedTestHub, generation: u64, ids: &[String]) {
    h.results.clear_test_results_for(ids);
    for id in ids {
        h.result_generation.insert(id.clone(), generation);
    }
}

/// Apply one job's result batch under the generation guard. A result is
/// accepted only when its generation is at least the node's last accepted
/// generation; accepted nodes are stamped with the (possibly newer) run.
/// Returns the accepted results (in input order) for the FRB stream.
fn apply_job_batch(
    h: &mut SpeedTestHub,
    generation: u64,
    batch: &[SpeedTestResult],
) -> Vec<SpeedTestResult> {
    let mut accepted = Vec::with_capacity(batch.len());
    for result in batch {
        let current = h
            .result_generation
            .get(&result.index_id)
            .copied()
            .unwrap_or(0);
        if generation >= current {
            h.result_generation
                .insert(result.index_id.clone(), generation);
            accepted.push(result.clone());
        }
    }
    h.results.apply_all(&accepted);
    accepted
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

pub(crate) fn reload_profile_ex_after_restore() {
    with_hub(|h| {
        let jobs = h.jobs.cancel_all();
        let barrier = jobs.iter().map(|job| job.generation).max().unwrap_or(0) + 1;
        for job in jobs {
            for node in job.snapshot.nodes.iter() {
                h.result_generation.insert(node.index_id.clone(), barrier);
            }
        }
        h.results = ProfileExStore::new();
        h.loaded = false;
        ensure_profile_ex_loaded(h);
    });
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

/// What this build can really do (R3-PROF-05). UDP is gated on the real
/// node-routed SOCKS5 UDP associate capability; a failed target is reported as
/// a structured failure, never a fake delay.
#[frb(sync)]
pub fn speedtest_supported() -> SpeedTestSupportDto {
    SpeedTestSupportDto {
        tcp_ping: true,
        real_ping: true,
        download: true,
        mixed: true,
        fast_real_ping: true,
        udp: application::speedtest::udp_via_socks_supported(),
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
        // Clear the run's stale results under its generation; an older
        // in-flight run can no longer write these node ids (R4-22).
        ensure_profile_ex_loaded(h);
        let ids: Vec<String> = test_nodes.iter().map(|n| n.index_id.clone()).collect();
        begin_job_results(h, job.generation, &ids);
        (h.settings.clone(), job)
    });

    let job_id = job.job_id.as_str().to_string();
    let job_id_out = job_id.clone();
    let job_generation = job.generation;
    let total = test_nodes.len() as u32;
    let token = job.token.clone();
    let kind_value = action_value(action);
    let runner = SpeedTestRunner::new(settings, Arc::new(NetHostTestSession::new()));

    let _ = std::thread::Builder::new()
        .name("speedtest-job".into())
        .spawn(move || {
            let job_id_for_batch = job_id.clone();
            let outcome = runner.run(action, &test_nodes, &token, |batch| {
                with_hub(|h| {
                    let accepted = apply_job_batch(h, job_generation, batch);
                    let dtos: Vec<SpeedTestResultDto> = accepted.iter().map(dto).collect();
                    let dto = SpeedTestBatchDto {
                        job_id: job_id_for_batch.clone(),
                        kind: kind_value,
                        results: dtos,
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
    // R3-PROF-06: a Custom profile's stored payload is the original config file.
    // Upstream `CoreConfigHandler.GenerateClientCustomConfig` copies that file
    // verbatim (`File.Copy`); running it through the JSON generator would wrap
    // YAML / raw text as a quoted JSON string and corrupt it. Return the raw
    // text byte-for-byte and keep the original file extension.
    if matches!(profile.config_type, domain::ConfigType::Custom) {
        return raw_custom_export(
            &index_id,
            &profile.address,
            input.profile.custom_config.as_deref(),
            core,
        );
    }
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

/// R3-PROF-06: return a Custom profile's stored config verbatim. Upstream
/// `GenerateClientCustomConfig` copies the file (`File.Copy`), so the export
/// text must be byte-identical and must never be re-serialized as a JSON
/// string. A missing payload is an explicit failure (upstream
/// `FailedGenDefaultConfiguration`), not a silent empty success.
fn raw_custom_export(
    index_id: &str,
    address: &str,
    raw: Option<&str>,
    core: CoreType,
) -> ExportClientConfigDto {
    match raw {
        Some(text) => ExportClientConfigDto {
            ok: true,
            text: text.to_string(),
            core_type: core.value().to_string(),
            file_name: custom_export_file_name(address, index_id),
            error: None,
        },
        None => ExportClientConfigDto {
            ok: false,
            text: String::new(),
            core_type: core.value().to_string(),
            file_name: String::new(),
            error: Some(
                DomainError::new(domain::codes::NOT_FOUND, "error.custom_config_file_missing")
                    .with_detail(address.to_string())
                    .into(),
            ),
        },
    }
}

/// A suggested file name that keeps the stored config's original extension
/// (e.g. `.yaml`), so the save dialog does not offer a hard-coded `.json` for a
/// raw Custom file.
fn custom_export_file_name(address: &str, index_id: &str) -> String {
    let stem = sanitize_export_name(index_id);
    let ext = std::path::Path::new(address.trim())
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty());
    match ext {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
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
    fn late_old_generation_batch_cannot_overwrite_a_newer_run() {
        // R4-22: run 2 claims nodes a/b and measures a=50; a late batch from run
        // 1 (generation 1) is evicted instead of clobbering the newer result,
        // while a node no run claimed is still filled by the older run.
        let mut h = SpeedTestHub::new();
        let ids = vec!["a".to_string(), "b".to_string()];
        begin_job_results(&mut h, 2, &ids);

        let accepted_new = apply_job_batch(&mut h, 2, &[SpeedTestResult::delay("a", 50)]);
        assert_eq!(accepted_new.len(), 1);
        assert_eq!(h.results.get("a").unwrap().delay, 50);

        let accepted_old = apply_job_batch(&mut h, 1, &[SpeedTestResult::delay("a", 7)]);
        assert!(accepted_old.is_empty(), "stale generation must be dropped");
        assert_eq!(
            h.results.get("a").unwrap().delay,
            50,
            "newer result survives"
        );

        let unclaimed = apply_job_batch(&mut h, 1, &[SpeedTestResult::delay("c", 9)]);
        assert_eq!(unclaimed.len(), 1, "an unclaimed node is not lost");
        assert_eq!(h.results.get("c").unwrap().delay, 9);
    }

    #[test]
    fn starting_a_run_clears_stale_delay_before_measuring() {
        // R4-22: a stage-2 run clearing a/b must not leave a stage-1 success in
        // place; a cancelled run therefore reports "unknown", never fake success.
        let mut h = SpeedTestHub::new();
        apply_job_batch(&mut h, 1, &[SpeedTestResult::delay("a", 88)]);
        assert_eq!(h.results.get("a").unwrap().delay, 88);
        begin_job_results(&mut h, 2, &["a".to_string()]);
        assert_eq!(
            h.results.get("a").unwrap().delay,
            0,
            "stale success is cleared at the next run start"
        );
    }

    #[test]
    fn udp_support_is_gated_by_the_socks_associate_path() {
        let support = speedtest_supported();
        assert!(support.tcp_ping && support.real_ping && support.download);
        assert_eq!(
            support.udp,
            application::speedtest::udp_via_socks_supported()
        );
        assert!(
            support.udp,
            "R3-PROF-05: node-routed UDP is supported on desktop"
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

    #[test]
    fn raw_custom_export_preserves_bytes_and_original_extension() {
        // R3-PROF-06: a Custom YAML/raw payload must survive the export API
        // byte-for-byte (upstream `File.Copy`), never quoted/escaped as a JSON
        // string, and the suggested name keeps the original extension.
        let yaml = "mixed-port: 11980\nproxies:\n  - name: \"node\"\n    type: ss\n";
        let dto = raw_custom_export("idx-1", "custom.yaml", Some(yaml), CoreType::Xray);
        assert!(dto.ok);
        assert_eq!(dto.text, yaml, "raw custom text must be byte-identical");
        assert_eq!(dto.file_name, "idx-1.yaml");
        assert!(
            !dto.text.starts_with('"') && !dto.text.contains("\\n"),
            "must not be wrapped as a JSON string"
        );

        let missing = raw_custom_export("idx-2", "missing.yaml", None, CoreType::Xray);
        assert!(!missing.ok);
        assert!(missing.text.is_empty());
        assert_eq!(
            missing.error.as_ref().map(|e| e.code.as_str()),
            Some(domain::codes::NOT_FOUND)
        );
    }

    /// Minimal std-only SOCKS5 UDP server: completes `UDP ASSOCIATE`, echoes the
    /// datagram back and records the requested target. Binds TCP+UDP on the same
    /// free port `>= 11808`.
    fn socks_udp_server() -> (u16, std::thread::JoinHandle<()>, Arc<Mutex<Option<String>>>) {
        use std::io::{Read, Write};
        let (tcp, udp, port) = (11808u16..12408)
            .find_map(|candidate| {
                if candidate == 10_808 {
                    return None;
                }
                let tcp = std::net::TcpListener::bind(("127.0.0.1", candidate)).ok()?;
                let udp = std::net::UdpSocket::bind(("127.0.0.1", candidate)).ok()?;
                Some((tcp, udp, candidate))
            })
            .expect("no free tcp+udp test port >= 11808");
        udp.set_read_timeout(Some(Duration::from_millis(500)))
            .expect("udp timeout");
        let seen = Arc::new(Mutex::new(None));
        let seen_udp = Arc::clone(&seen);
        let handle = std::thread::spawn(move || {
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop_udp = Arc::clone(&stop);
            let echo_socket = udp.try_clone().expect("clone udp");
            let echo = std::thread::spawn(move || {
                let mut buf = [0u8; 2048];
                while !stop_udp.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Ok((n, peer)) = echo_socket.recv_from(&mut buf) {
                        if let Some(target) = parse_socks_udp_target(&buf[..n]) {
                            *seen_udp.lock().unwrap() = Some(target);
                        }
                        let _ = echo_socket.send_to(&buf[..n], peer);
                    }
                }
            });
            if let Ok((mut control, _)) = tcp.accept() {
                let _ = control.set_read_timeout(Some(Duration::from_secs(2)));
                let mut greeting = [0u8; 3];
                if control.read_exact(&mut greeting).is_ok() {
                    let _ = control.write_all(&[0x05, 0x00]);
                    let mut request = [0u8; 10];
                    if control.read_exact(&mut request).is_ok() {
                        let reply = [
                            0x05,
                            0x00,
                            0x00,
                            0x01,
                            127,
                            0,
                            0,
                            1,
                            (port >> 8) as u8,
                            (port & 0xff) as u8,
                        ];
                        let _ = control.write_all(&reply);
                        std::thread::sleep(Duration::from_millis(400));
                    }
                }
            }
            stop.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = echo.join();
        });
        (port, handle, seen)
    }

    fn parse_socks_udp_target(datagram: &[u8]) -> Option<String> {
        if datagram.len() < 10 || datagram[2] != 0x00 || datagram[3] != 0x01 {
            return None;
        }
        let port = u16::from_be_bytes([datagram[8], datagram[9]]);
        Some(format!(
            "{}.{}.{}.{}:{}",
            datagram[4], datagram[5], datagram[6], datagram[7], port
        ))
    }

    #[test]
    fn udp_ping_routes_through_the_session_port() {
        // R3-PROF-05: the probe must carry `session.port`, proving the old
        // host-direct `udp_ping(target)` is no longer used for a node result.
        let _guard = lock();
        let (port, handle, seen) = socks_udp_server();
        let provider = NetHostTestSession::new();
        let session = TestSession {
            node: TestNode {
                index_id: "r3-prof-05".into(),
                address: "192.0.2.1".into(),
                port: 443,
                config_type: domain::ConfigType::Vless.value(),
                core_type: CoreType::Xray.value(),
            },
            port,
            handle_id: "synthetic".into(),
        };
        let ct = domain::CancellationToken::new();
        let delay = provider.udp_ping(&session, "192.0.2.9:11999", Duration::from_secs(2), &ct);
        assert!(delay.is_some_and(|d| d >= 1), "delay={delay:?}");
        let _ = handle.join();
        assert_eq!(
            seen.lock().unwrap().as_deref(),
            Some("192.0.2.9:11999"),
            "datagram must be relayed through session.port"
        );
    }
}
