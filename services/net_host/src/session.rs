//! Session lifecycle: the state machine that owns exactly one managed core.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::event::{EventKind, RuntimeStateChanged};
use domain::runtime_plan::ConfigSource;
use domain::{CoreType, DomainError, RuntimePlan, RuntimeState};
use ipc_contract::stable::CoreExitFact;
use ipc_contract::{OperationStatus, RecoveryStage, RecoveryStatus, RuntimeSnapshot};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::sync::{Mutex, Notify};

use runtime::tun::{tun_spec_from_plan, TunSpec};
use runtime::{
    adapter_for, matches_identity, process_creation_time_ms, sha256_hex, CoreAdapter, CoreLocator,
    JobGuard, ProcessIdentity, RuntimeDetail, RuntimeTunDetail, ServerFrame, NET_HOST_PIPE_NAME,
    RUNTIME_DETAIL_EVENT,
};

use crate::events::EventBus;
use crate::helper_client::{
    build_helper_link, tun_helper_unavailable, HelperConfig, HelperLink, TunLease,
};
use crate::journal::{self, JournalEntry};
use crate::lifecycle;
use crate::managed_process;
use crate::tun_lease::{self, PendingCleanup};

/// Runtime configuration, overridable from the environment for tests.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub pipe_name: String,
    pub run_root: PathBuf,
    pub disconnect_grace: Duration,
    pub heartbeat_interval: Duration,
    pub readiness_timeout: Duration,
    pub readiness_interval: Duration,
    pub helper: HelperConfig,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

fn env_ms(name: &str, default_ms: u64) -> Duration {
    let ms = std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_ms);
    Duration::from_millis(ms)
}

impl HostConfig {
    pub fn from_env() -> Self {
        let run_root = std::env::var_os("V2RAYN_R_RUN_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(|l| PathBuf::from(l).join("v2rayn-r").join("run"))
            })
            .unwrap_or_else(|| PathBuf::from("run"));
        let mut helper = HelperConfig::from_env();
        // A TUN session stages its elevated core under the run root; the helper
        // rejects `RunElevatedCore` outside every allowed root, so the
        // packaged default (no env override) must still allow this run root.
        if helper.allowed_run_roots.is_empty() {
            helper
                .allowed_run_roots
                .push(run_root.to_string_lossy().into_owned());
        }
        Self {
            pipe_name: std::env::var("V2RAYN_R_PIPE")
                .unwrap_or_else(|_| NET_HOST_PIPE_NAME.to_string()),
            run_root,
            disconnect_grace: env_ms("V2RAYN_R_DISCONNECT_GRACE_MS", 6_000),
            heartbeat_interval: env_ms("V2RAYN_R_HEARTBEAT_MS", 2_000),
            readiness_timeout: env_ms("V2RAYN_R_READY_TIMEOUT_MS", 20_000),
            readiness_interval: env_ms("V2RAYN_R_READY_INTERVAL_MS", 500),
            helper,
        }
    }
}

/// A plan that passed every side-effect-free precheck (RR-10): adapter, hash,
/// port, core executable, TUN spec and a real `test_args` config check. It is
/// built *before* the running session is stopped.
struct PreparedPlan {
    adapter: Box<dyn CoreAdapter>,
    exe: PathBuf,
    body: String,
    actual_hash: String,
    port: u16,
    tun_spec: Option<TunSpec>,
    /// R3-04: TUN is enabled but the adapter does not exist yet. The core is
    /// spawned first (its tun inbound creates the device), then net-host
    /// discovers the interface and applies this descriptor through the helper.
    deferred_tun: Option<TunSpec>,
    /// Nested process-graph nodes resolved in start order (RR-06). Each has a
    /// located executable and validated config body, ready to stage and spawn.
    sidecars: Vec<PreparedSidecar>,
}

/// A process-graph node other than the plan target, prechecked before the old
/// session is stopped.
struct PreparedSidecar {
    id: String,
    adapter: Box<dyn CoreAdapter>,
    exe: PathBuf,
    body: String,
    port: u16,
    /// The node's config carries a TUN inbound, so it must run elevated
    /// through the helper (a normal spawn cannot create the Wintun adapter).
    elevated: bool,
}

/// One additional managed process in the plan's [`ProcessGraph`] (RR-06):
/// today the pre-SOCKS / LegacyProtect sidecar, distinguished so its logs are
/// labeled and it can be stopped in reverse start order. A sidecar is either a
/// normal child process owned by net-host, or an elevated process owned by the
/// helper session kept open in `helper`.
pub struct SidecarSession {
    /// Process-graph node id (e.g. `pre-socks`), for exit attribution.
    pub id: String,
    pub child: Option<tokio::process::Child>,
    /// Owns the sidecar's Job Object; dropping it (on stop or session drop)
    /// guarantees the sidecar tree dies even if net-host is hard-killed.
    pub _job: Option<JobGuard>,
    /// Helper session that owns an elevated sidecar. Kept open for the whole
    /// session so the helper lease keeps owning the process; the pipe drop on
    /// cleanup triggers the helper's `CleanOwned` stop.
    pub helper: Option<Box<dyn HelperLink>>,
    pub handle: Option<u64>,
    /// Helper-reported PID of an elevated sidecar (SP-10). The helper owns
    /// the process, so this is attribution identity for exit facts, never a
    /// local kill target: net-host must not signal it by PID.
    pub elevated_pid: Option<u32>,
    /// Helper-observed exit of an elevated sidecar (SP-10, CP-12 / TUN-A05).
    /// Elevated sidecars have no pollable local handle, so their death
    /// arrives through helper observation (A02 `PollCoreExits`, staged via
    /// [`HostState::note_elevated_sidecar_exit`]); `reconcile_exits` consumes
    /// it through the same Degraded path as an ordinary sidecar exit.
    pub elevated_exit_code: Option<Option<i32>>,
}

impl SidecarSession {
    async fn terminate_and_wait(&mut self, grace: Duration) {
        if let (Some(link), Some(handle)) = (self.helper.as_mut(), self.handle) {
            let _ = link.stop_core(handle);
            self.helper = None;
            self.handle = None;
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
            match tokio::time::timeout(grace, child.wait()).await {
                Ok(_) => {}
                Err(_) => {
                    let _ = child.kill().await;
                }
            }
        }
        self._job = None;
    }
}

/// One live managed core plus the facts needed to own and recover it.
pub struct Session {
    pub session_id: String,
    pub plan_id: String,
    pub desired_revision: u64,
    pub port: u16,
    pub config_sha256: String,
    /// The executable this session runs, retained so a failed switch restores
    /// the exact same core (RR-10).
    pub exe: PathBuf,
    pub child: tokio::process::Child,
    pub identity: ProcessIdentity,
    /// The nested process-graph nodes started before the main core, in start
    /// order. They are stopped in reverse order (RR-06).
    pub sidecars: Vec<SidecarSession>,
    pub job: JobGuard,
}

impl Session {
    /// Terminate the stopped process graph in reverse start order (sidecars
    /// first, then the main core); closing the job guarantees the tree dies.
    pub async fn terminate_and_wait(&mut self, grace: Duration) {
        for sidecar in self.sidecars.iter_mut().rev() {
            sidecar.terminate_and_wait(grace).await;
        }
        let _ = self.child.start_kill();
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(_) => {}
            Err(_) => {
                let _ = self.child.kill().await;
            }
        }
    }
}

/// A temporary test core. It is owned separately from the managed `Session`
/// so a speedtest can never disturb the live runtime.
pub struct TestSessionChild {
    pub core: domain::CoreType,
    pub port: u16,
    pub child: tokio::process::Child,
    pub identity: ProcessIdentity,
    /// RAII ownership job: dropping it guarantees the temp core tree dies.
    pub _job: JobGuard,
}

impl TestSessionChild {
    async fn terminate_and_wait(&mut self, grace: Duration) {
        let _ = self.child.start_kill();
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(_) => {}
            Err(_) => {
                let _ = self.child.kill().await;
            }
        }
    }
}

pub struct Inner {
    pub session: Option<Session>,
    /// Plan of the currently-running (or last applied) session, retained so a
    /// failed switch can restore the previous good session (RR-10).
    pub last_plan: Option<RuntimePlan>,
    /// Resolved executable of the running session, reused verbatim on restore.
    pub last_exe: Option<PathBuf>,
    pub detail: RuntimeDetail,
    pub active_operation: Option<String>,
    pub operations: HashMap<String, OperationStatus>,
    pub active_connections: usize,
    pub no_client_since: Option<Instant>,
    pub recovery: Option<RecoveryStatus>,
    /// Isolated temporary cores for restricted test sessions (never the
    /// managed runtime).
    pub test_sessions: HashMap<String, TestSessionChild>,
    /// Active TUN lease for the managed session, if the applied plan requested
    /// TUN. The helper session lives exactly as long as this record.
    pub tun_lease: Option<TunLease>,
    /// Session id the TUN lease journal was written under. Kept separately so
    /// a stop arriving before the core is spawned can still find the journal.
    pub tun_session_id: Option<String>,
    /// The helper connection that owns `tun_lease`. It is kept open for the
    /// whole runtime session: dropping it lets the helper reclaim the lease
    /// via its disconnect cleanup, so it must not be dropped early.
    pub tun_link: Option<Box<dyn HelperLink>>,
    /// SP-08: unconfirmed TUN cleanups retained in memory (mirrored on disk
    /// by the pending record). A stop with a failed cleanup keeps its lease
    /// here for explicit retry instead of reporting a clean shutdown.
    pub pending_tun_cleanup: Vec<PendingCleanup>,
    /// SP-06: fact generation advanced by every unsolicited exit, even though
    /// the desired plan did not change. Surfaced on the wire as an event-epoch
    /// bump plus the Stopped/Degraded transition; kept here for the future
    /// `RuntimeActualDescriptor` FRB exposure.
    pub actual_generation: u64,
    /// SP-06: the most recent unsolicited managed-process exit
    /// (`pid/exit_code/at_ms`). Cleared by the next successful apply; a
    /// user-initiated stop keeps it as history.
    pub last_exit: Option<CoreExitFact>,
    /// Sidecar id for `last_exit`, when the exit was a sidecar.
    pub last_exit_sidecar: Option<String>,
}

/// Factory for helper links. Production builds a pipe (or dry-run) link from
/// the host config; tests inject an in-memory fake. Never touches the OS.
pub type HelperLinkFactory =
    std::sync::Arc<dyn Fn(&HelperConfig) -> Box<dyn HelperLink> + Send + Sync>;

/// Injectable seam for the TUN adapter lookup (R3-04). Production queries the
/// OS; tests inject a stub so "the interface appears only after the core
/// starts" is exercised without touching the host.
pub type TunInterfaceDiscovery = std::sync::Arc<dyn Fn(&str) -> Option<u32> + Send + Sync>;

pub struct HostState {
    pub inner: Mutex<Inner>,
    pub bus: EventBus,
    pub config: HostConfig,
    pub shutdown: Arc<Notify>,
    /// SP-04: the single authoritative command sequence for the managed
    /// session. Apply, stop and shutdown serialize here in admission order;
    /// a stop is a cleanup barrier that can never be overtaken by a later
    /// apply. Queries (`ipc_snapshot`, `operation_status`, `detail_frame`)
    /// never take this gate, so an in-flight command cannot block the
    /// reconcile reads an unknown outcome depends on.
    command_gate: tokio::sync::Mutex<()>,
    helper_factory: std::sync::Mutex<Option<HelperLinkFactory>>,
    tun_discovery: std::sync::Mutex<Option<TunInterfaceDiscovery>>,
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_operation_id() -> String {
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
    format!("op-{}-{}", journal::now_ms(), seq)
}

/// Process-node id carrying a deferred TUN descriptor (R3-04). Mirrors
/// `application::tun_plan::TUN_DEFERRED_PROCESS_ID`; net-host does not depend on
/// the application crate, so the literal is duplicated here.
const TUN_DEFERRED_PROCESS_ID: &str = "tun-deferred";

/// Parse a deferred TUN descriptor from the plan, if present. The embedded
/// `TunSpec` still has `interface_index = 0`; net-host fills it in after the
/// core's tun inbound creates the adapter.
fn deferred_tun_from_plan(plan: &RuntimePlan) -> Result<Option<TunSpec>, DomainError> {
    let Some(node) = plan
        .process_graph
        .nodes
        .iter()
        .find(|node| node.id == TUN_DEFERRED_PROCESS_ID)
    else {
        return Ok(None);
    };
    let body = match &node.config {
        ConfigSource::Inline { body } => body,
        ConfigSource::ControlledFile { .. } => {
            return Err(
                DomainError::new(domain::codes::INVALID_PLAN, "error.invalid_plan")
                    .with_detail("`tun-deferred` descriptor must be inline"),
            );
        }
    };
    let spec: TunSpec = serde_json::from_str(body).map_err(|error| {
        DomainError::new(domain::codes::INVALID_PLAN, "error.invalid_plan").with_detail(format!(
            "`tun-deferred` descriptor is not valid JSON: {error}"
        ))
    })?;
    Ok(Some(spec))
}

/// Bounded window for the core to create the TUN adapter (R3-04).
fn tun_discovery_timeout() -> Duration {
    env_ms("V2RAYN_R_TUN_DISCOVERY_TIMEOUT_MS", 15_000)
}

/// Poll interval while waiting for the TUN adapter to appear (R3-04).
fn tun_discovery_interval() -> Duration {
    env_ms("V2RAYN_R_TUN_DISCOVERY_INTERVAL_MS", 500)
}

/// Parse `netsh interface ipv4 show interfaces` output for the adapter's index.
/// Mirrors `application::tun_plan::parse_interface_index` (same column layout).
fn parse_tun_interface_index(output: &str, adapter_name: &str) -> Option<u32> {
    let want = adapter_name.trim().to_ascii_lowercase();
    if want.is_empty() {
        return None;
    }
    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let Some(index) = parts.next().and_then(|token| token.parse::<u32>().ok()) else {
            continue;
        };
        let rest: Vec<&str> = parts.collect();
        if rest.len() < 4 {
            continue;
        }
        let name = rest[3..].join(" ");
        if name.trim().to_ascii_lowercase() == want {
            return Some(index);
        }
    }
    None
}

/// Query the OS for an adapter's index (Windows-only; see R3-04).
#[cfg(windows)]
fn discover_interface_index_os(adapter_name: &str) -> Option<u32> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("netsh")
        .args(["interface", "ipv4", "show", "interfaces"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_tun_interface_index(&String::from_utf8_lossy(&output.stdout), adapter_name)
}

#[cfg(not(windows))]
fn discover_interface_index_os(_adapter_name: &str) -> Option<u32> {
    None
}

/// Stub-core tests opt out of the real `test_args` config check.
fn skip_config_check() -> bool {
    std::env::var_os("V2RAYN_R_SKIP_CONFIG_CHECK").is_some()
}

/// Whether a generated core config declares a TUN inbound (sing-box
/// `{"type":"tun"}`). Such a node creates the Wintun adapter and must run
/// elevated; the check is structural, never a substring match.
fn body_has_tun_inbound(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("inbounds")
                .and_then(serde_json::Value::as_array)
                .map(|inbounds| {
                    inbounds
                        .iter()
                        .any(|entry| entry.get("type").and_then(|t| t.as_str()) == Some("tun"))
                })
        })
        .unwrap_or(false)
}

/// Helper core id for the elevated-core allow-list. `CoreType::as_str` uses
/// project tokens (`sing_box`), while the helper allow-list uses the upstream
/// executable names (`sing-box`); this maps between them.
fn elevated_core_id(core: CoreType) -> Option<&'static str> {
    match core {
        CoreType::Xray => Some("xray"),
        CoreType::SingBox => Some("sing-box"),
        CoreType::Mihomo => Some("mihomo"),
        CoreType::V2fly | CoreType::V2flyV5 => Some("v2fly"),
        _ => None,
    }
}

/// Hard deadline for the core's own `test_args` validation. A core whose
/// precheck hangs must not keep a managed child alive after the client gives up
/// (D28); the client-side 60s IPC limit cannot stop it.
fn config_check_timeout() -> Duration {
    env_ms("V2RAYN_R_CONFIG_CHECK_TIMEOUT_MS", 30_000)
}

/// How a bounded precheck child finished.
enum RunBoundedError {
    Spawn(String),
    Timeout,
}

/// Run a project-owned child to completion within a hard deadline. The child is
/// `kill_on_drop`, so cancelling the await on expiry terminates it instead of
/// leaking a blocked precheck process. net-host owns this child directly; no
/// process is matched or killed by name.
async fn run_bounded_output(
    exe: &Path,
    args: &[std::ffi::OsString],
    limit: Duration,
) -> Result<std::process::Output, RunBoundedError> {
    let mut command = Command::new(exe);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    match tokio::time::timeout(limit, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(RunBoundedError::Spawn(e.to_string())),
        Err(_) => Err(RunBoundedError::Timeout),
    }
}

/// Structured error for a core that could not be bound into the ownership job.
/// Non-retryable: the caller must not keep an unowned core alive.
fn job_assign_failed(operation_id: &str, detail: impl Into<String>) -> DomainError {
    DomainError::new(domain::codes::JOB_ASSIGN_FAILED, "error.job_assign_failed")
        .with_operation(operation_id)
        .with_detail(detail)
}

impl HostState {
    /// Read-only lookup of a recorded operation's structured status (R4-04).
    pub async fn operation_status(&self, operation_id: &str) -> Option<OperationStatus> {
        self.reconcile_exits().await;
        self.inner
            .lock()
            .await
            .operations
            .get(operation_id)
            .cloned()
    }

    pub fn new(config: HostConfig) -> Self {
        let recovery = journal::recover_stale(&config.run_root);
        let recovery_status = if recovery.needed {
            Some(RecoveryStatus {
                recovery_needed: true,
                last_stage: recovery.last_stage.unwrap_or(RecoveryStage::Prepared),
                restored: recovery.restored,
                pending: recovery.pending,
            })
        } else {
            None
        };
        let state = Self {
            inner: Mutex::new(Inner {
                session: None,
                last_plan: None,
                last_exe: None,
                detail: RuntimeDetail::default(),
                active_operation: None,
                operations: HashMap::new(),
                active_connections: 0,
                no_client_since: None,
                recovery: recovery_status,
                test_sessions: HashMap::new(),
                tun_lease: None,
                tun_session_id: None,
                tun_link: None,
                pending_tun_cleanup: Vec::new(),
                actual_generation: 0,
                last_exit: None,
                last_exit_sidecar: None,
            }),
            bus: EventBus::new(),
            config,
            shutdown: Arc::new(Notify::new()),
            command_gate: tokio::sync::Mutex::new(()),
            helper_factory: std::sync::Mutex::new(None),
            tun_discovery: std::sync::Mutex::new(None),
        };
        // Reconcile TUN leases from a previous process. Only session dirs that
        // still carry a lease journal are touched; with no stale leases no
        // helper connection is even attempted.
        state.reconcile_tun_leases();
        state
    }

    /// Test seam (SP-04): the serial command gate itself.
    #[cfg(test)]
    pub(crate) fn command_gate(&self) -> &tokio::sync::Mutex<()> {
        &self.command_gate
    }

    /// Test seam: replace helper-link construction (in-memory fake, no pipe).
    #[cfg(test)]
    pub fn set_helper_factory(&self, factory: HelperLinkFactory) {
        if let Ok(mut slot) = self.helper_factory.lock() {
            *slot = Some(factory);
        }
    }

    /// Test seam: replace the TUN adapter lookup (R3-04).
    #[cfg(test)]
    pub fn set_tun_discovery(&self, discovery: TunInterfaceDiscovery) {
        if let Ok(mut slot) = self.tun_discovery.lock() {
            *slot = Some(discovery);
        }
    }

    /// Discover the TUN adapter's OS interface index. Production queries the OS
    /// (`netsh`); tests override with [`set_tun_discovery`].
    fn discover_tun_interface(&self, adapter_name: &str) -> Option<u32> {
        if let Ok(slot) = self.tun_discovery.lock() {
            if let Some(discovery) = slot.as_ref() {
                return discovery(adapter_name);
            }
        }
        discover_interface_index_os(adapter_name)
    }

    fn make_helper_link(&self) -> Box<dyn HelperLink> {
        if let Ok(slot) = self.helper_factory.lock() {
            if let Some(factory) = slot.as_ref() {
                return factory(&self.config.helper);
            }
        }
        build_helper_link(&self.config.helper)
    }

    /// Reconcile stale TUN leases left by a previous net-host process.
    /// Best-effort: never fails startup; counts are logged for the audit trail.
    /// Leases the previous process could not confirm stay pending — on disk
    /// and in memory — so an explicit retry (once the helper is reachable)
    /// can finish them (SP-08 recovery entry).
    pub fn reconcile_tun_leases(&self) {
        let mut link = self.make_helper_link();
        let report = tun_lease::reconcile_stale_tun(&self.config.run_root, &mut *link);
        if report.scanned > 0 {
            eprintln!(
                "[net_host] tun recovery: scanned={} cleaned={} pending={}",
                report.scanned, report.cleaned, report.pending
            );
        }
        let pending = tun_lease::list_pending_cleanup(&self.config.run_root);
        if !pending.is_empty() {
            // Construction-time: the lock is uncontended, so a non-blocking
            // acquisition always succeeds here (and keeps this callable from
            // inside an async runtime, where blocking would panic).
            match self.inner.try_lock() {
                Ok(mut inner) => inner.pending_tun_cleanup = pending,
                Err(_) => {
                    eprintln!("[net_host] tun pending state deferred; retry surface stays on disk")
                }
            }
        }
    }

    /// Host-side pending-cleanup surface for the future IPC/FRB wiring
    /// (interface need N-H1): in-memory entries plus durable records,
    /// de-duplicated by session. The stable snapshot DTO is owned by the
    /// SP-00 integrator; this method is the data source, not the wire shape.
    /// Staged until the integrator wires it; covered by SP-08 tests.
    #[allow(dead_code)]
    pub async fn pending_cleanup_snapshot(&self) -> Vec<PendingCleanup> {
        let mut merged = self.inner.lock().await.pending_tun_cleanup.clone();
        for pending in tun_lease::list_pending_cleanup(&self.config.run_root) {
            if !merged
                .iter()
                .any(|known| known.session_id == pending.session_id)
            {
                merged.push(pending);
            }
        }
        merged.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        merged
    }

    pub async fn connection_opened(&self) {
        let mut inner = self.inner.lock().await;
        inner.active_connections += 1;
        inner.no_client_since = None;
    }

    pub async fn connection_closed(&self) {
        let mut inner = self.inner.lock().await;
        inner.active_connections = inner.active_connections.saturating_sub(1);
        if inner.active_connections == 0 {
            inner.no_client_since = Some(Instant::now());
        }
    }

    /// Whether the disconnect/unresponsive policy says the lease must be
    /// reclaimed (used by the watchdog and asserted by tests).
    pub async fn should_reclaim(&self) -> bool {
        let inner = self.inner.lock().await;
        reclaim_due(
            inner.session.is_some(),
            inner.active_connections,
            inner.no_client_since,
            self.config.disconnect_grace,
        )
    }

    /// Whether the host has been idle long enough to terminate itself so a real
    /// exit leaves no residual service process. Never true while a managed
    /// session, TUN lease or test session is outstanding: the core tree must be
    /// gone first, so this can only run after the reclaim path.
    pub async fn should_exit_idle(&self) -> bool {
        let inner = self.inner.lock().await;
        idle_exit_due(
            inner.session.is_some(),
            inner.tun_lease.is_some(),
            inner.test_sessions.len(),
            inner.active_connections,
            inner.no_client_since,
            self.config.disconnect_grace,
        )
    }

    /// Snapshot of runtime facts for an IPC `GetSnapshot`.
    pub async fn ipc_snapshot(&self) -> RuntimeSnapshot {
        self.reconcile_exits().await;
        let inner = self.inner.lock().await;
        RuntimeSnapshot {
            state: inner.detail.state,
            applied_revision: inner.detail.applied_revision,
            epoch: self.bus.epoch(),
            last_seq: self.bus.last_seq(),
            active_operation: inner.active_operation.clone(),
            recovery: inner.recovery.clone(),
            host_alive: true,
        }
    }

    /// A fresh detail event frame for the requesting connection (not
    /// broadcast; the client only uses it to learn PID/ports).
    pub async fn detail_frame(&self) -> ServerFrame {
        self.reconcile_exits().await;
        let inner = self.inner.lock().await;
        let payload = serde_json::to_value(&inner.detail).unwrap_or(json!({}));
        ServerFrame::Event(
            self.bus
                .make(EventKind::Other(RUNTIME_DETAIL_EVENT.to_string()), payload),
        )
    }

    /// Record a helper-observed elevated-sidecar exit (SP-10, TUN-A05).
    ///
    /// Staged until the A02 `PollCoreExits` observation channel lands; until
    /// then nothing calls this in production and elevated exits stay
    /// unobserved (registered gap: without observation the session cannot
    /// claim full readiness, see `lifecycle::session_readiness`). Returns
    /// false for unknown sidecar ids so a stray report can never fabricate
    /// an exit. The next read path consumes the report through
    /// `reconcile_exits`, advancing the fact generation like an ordinary
    /// sidecar exit.
    #[allow(dead_code)]
    pub async fn note_elevated_sidecar_exit(&self, id: &str, code: Option<i32>) -> bool {
        let mut inner = self.inner.lock().await;
        let Some(session) = inner.session.as_mut() else {
            return false;
        };
        let Some(sidecar) = session.sidecars.iter_mut().find(|sidecar| {
            sidecar.id == id && sidecar.child.is_none() && sidecar.helper.is_some()
        }) else {
            return false;
        };
        sidecar.elevated_exit_code = Some(code);
        true
    }

    /// SP-06 continuous exit observation.
    ///
    /// Every read path (`ipc_snapshot`, `detail_frame`, `operation_status`)
    /// reconciles first, so a post-readiness exit can never survive as a
    /// cached Running behind a port that is already dead (CP-01 ghost).
    ///
    /// Observation is handle-authoritative (`try_wait` on the owned `Child`):
    /// never a PID scan, so a recycled PID can never cause a miskill and an
    /// unreadable status never fabricates a transition. Elevated sidecars
    /// owned by the helper have no pollable handle on this side, so they are
    /// left alone here (no miskill); their observation is the registered
    /// `HelperOp::PollCoreExits` gap for the SP-00 integrator.
    ///
    /// Reconcile runs only when no command is in flight: an in-flight apply
    /// owns the session through readiness and must not lose it to a read.
    pub async fn reconcile_exits(&self) {
        struct MainExit {
            pid: u32,
            created_at_ms: i64,
            port: u16,
            plan_id: String,
            desired_revision: u64,
            config_sha256: String,
            session_id: String,
            code: Option<i32>,
        }
        struct SidecarExit {
            index: usize,
            id: String,
            pid: u32,
            code: Option<i32>,
        }

        let (session, main_exit, _sidecar_exits, emit) = {
            let mut inner = self.inner.lock().await;
            if !lifecycle::should_reconcile(
                inner.active_operation.as_deref(),
                inner.session.is_some(),
            ) {
                return;
            }
            let at_ms = journal::now_ms();
            let session = inner.session.as_mut().expect("reconcile gated a session");
            // An unreadable status never fabricates a transition.
            let main_status = session.child.try_wait().ok().flatten();
            let mut sidecar_exits = Vec::new();
            for (index, sidecar) in session.sidecars.iter_mut().enumerate() {
                if sidecar.child.is_none() {
                    // SP-10: an elevated sidecar has no pollable local
                    // handle, so only a helper-observed exit (recorded via
                    // `note_elevated_sidecar_exit`) may transition it. The
                    // helper-reported PID is attribution identity, never a
                    // local kill target. Without a report the sidecar is left
                    // alone here (no miskill).
                    if let Some(code) = sidecar.elevated_exit_code {
                        sidecar_exits.push(SidecarExit {
                            index,
                            id: sidecar.id.clone(),
                            pid: sidecar.elevated_pid.unwrap_or(0),
                            code,
                        });
                    }
                    continue;
                }
                let Some(child) = sidecar.child.as_mut() else {
                    continue;
                };
                // Alive needs nothing; unreadable must never be fabricated
                // into a transition.
                if let Ok(Some(status)) = child.try_wait() {
                    sidecar_exits.push(SidecarExit {
                        index,
                        id: sidecar.id.clone(),
                        pid: child.id().unwrap_or_default(),
                        code: status.code(),
                    });
                }
            }
            if let Some(status) = main_status {
                let exit = MainExit {
                    pid: session.identity.pid,
                    created_at_ms: session.identity.created_at_ms,
                    port: session.port,
                    plan_id: session.plan_id.clone(),
                    desired_revision: session.desired_revision,
                    config_sha256: session.config_sha256.clone(),
                    session_id: session.session_id.clone(),
                    code: status.code(),
                };
                let error = managed_process::main_exit_error(exit.pid, exit.code);
                let observed = managed_process::ObservedExit {
                    pid: exit.pid,
                    exit_code: exit.code,
                    at_ms,
                };
                let session = inner.session.take().expect("reconcile gated a session");
                // Withdraw the live endpoint; the applied revision stays as
                // history of what actually ran.
                inner.detail.state = RuntimeState::Stopped;
                inner.detail.pid = None;
                inner.detail.created_at_ms = None;
                inner.detail.ports.clear();
                inner.detail.session_id = None;
                inner.detail.config_sha256 = None;
                inner.detail.error = Some(error.clone());
                inner.actual_generation = lifecycle::next_generation(inner.actual_generation);
                inner.last_exit = Some(CoreExitFact::observed(
                    observed.pid,
                    observed.exit_code,
                    observed.at_ms,
                ));
                inner.last_exit_sidecar = None;
                // Phase 2 below terminates the torn-down sidecars in reverse
                // order (helper-owned ones via their link) and drops the job.
                let emit = (RuntimeState::Stopped, error, inner.detail.applied_revision);
                (Some(session), Some(exit), sidecar_exits, Some(emit))
            } else if !sidecar_exits.is_empty() {
                let first = &sidecar_exits[0];
                let observed = managed_process::ObservedExit {
                    pid: first.pid,
                    exit_code: first.code,
                    at_ms,
                };
                let error = managed_process::sidecar_exit_error(&first.id, observed.exit_code);
                // Drop the reaped sidecars (highest index first); the live
                // main core keeps its endpoint.
                let mut indices: Vec<usize> = sidecar_exits.iter().map(|exit| exit.index).collect();
                indices.sort_unstable_by(|a, b| b.cmp(a));
                let session = inner.session.as_mut().expect("reconcile gated a session");
                for index in indices {
                    session.sidecars.remove(index);
                }
                inner.detail.state = lifecycle::state_for_sidecar_exit(true);
                inner.detail.error = Some(error.clone());
                inner.actual_generation = lifecycle::next_generation(inner.actual_generation);
                inner.last_exit = Some(CoreExitFact::observed(
                    observed.pid,
                    observed.exit_code,
                    observed.at_ms,
                ));
                inner.last_exit_sidecar = Some(first.id.clone());
                let emit = (
                    lifecycle::state_for_sidecar_exit(true),
                    error,
                    inner.detail.applied_revision,
                );
                (None, None, sidecar_exits, Some(emit))
            } else {
                return;
            }
        };

        // Outside the inner lock: terminate leftovers, journal, then push the
        // fact generation so readers observe the exit.
        if let (Some(mut session), Some(exit)) = (session, main_exit) {
            for sidecar in session.sidecars.iter_mut().rev() {
                sidecar.terminate_and_wait(Duration::from_secs(5)).await;
            }
            drop(session.job);
            let _ = journal::write_entry(
                &self.config.run_root,
                &JournalEntry {
                    session_id: exit.session_id.clone(),
                    plan_id: exit.plan_id,
                    desired_revision: exit.desired_revision,
                    config_sha256: exit.config_sha256,
                    stage: RecoveryStage::Finalized,
                    pid: Some(exit.pid),
                    created_at_ms: Some(exit.created_at_ms),
                    port: exit.port,
                    updated_at_ms: journal::now_ms(),
                },
            );
            journal::remove_staged_artifacts(&self.config.run_root, &exit.session_id);
            eprintln!(
                "[net_host] session {} EXITED pid={} code={:?}; endpoint withdrawn",
                exit.session_id, exit.pid, exit.code
            );
        }
        if let Some((state, error, applied_revision)) = emit {
            self.bus.bump_epoch();
            self.bus.emit_named(
                "runtime_state_changed",
                serde_json::to_value(RuntimeStateChanged {
                    state,
                    applied_revision,
                    message_key: None,
                })
                .unwrap_or(json!({})),
            );
            self.bus.emit_named(
                "error_raised",
                json!({
                    "code": error.code,
                    "message_key": error.message_key,
                    "detail": error.detail,
                }),
            );
        }
    }

    async fn fail_operation(&self, operation_id: &str, error: &DomainError) {
        let mut inner = self.inner.lock().await;
        inner.detail.error = Some(error.clone());
        inner.detail.state = RuntimeState::Stopped;
        // RR-10: a failed candidate must not leave a published endpoint that no
        // listener backs.
        inner.detail.pid = None;
        inner.detail.created_at_ms = None;
        inner.detail.session_id = None;
        inner.detail.config_sha256 = None;
        inner.detail.ports.clear();
        inner.active_operation = None;
        inner.operations.insert(
            operation_id.to_string(),
            OperationStatus {
                operation_id: operation_id.to_string(),
                job_id: None,
                state: domain::JobState::Failed,
                cancel: None,
                error: Some(error.clone()),
            },
        );
    }

    /// Mark a partially prepared session as finalized so a later boot does not
    /// report a phantom recovery for a failure that never started a process.
    async fn finalize_journal(&self, entry: &JournalEntry) {
        let _ = journal::write_entry(
            &self.config.run_root,
            &JournalEntry {
                stage: RecoveryStage::Finalized,
                updated_at_ms: journal::now_ms(),
                ..entry.clone()
            },
        );
    }

    /// Release the active TUN lease in reverse order (helper resources, then
    /// the journal), idempotently (SP-08 / CP-04).
    ///
    /// `Ok` means the helper confirmed the release (or there was nothing to
    /// release). On failure the journal is kept, the lease stays in memory,
    /// and a pending entry is recorded — the caller must surface the error
    /// instead of reporting a clean shutdown. Failure paths that already carry
    /// a root-cause error may ignore the `Result`, but the lease still stays
    /// retryable; nothing is silently dropped.
    async fn release_tun_lease(&self) -> Result<(), DomainError> {
        let (session_id, lease, link) = {
            let mut inner = self.inner.lock().await;
            inner.detail.tun = None;
            (
                inner.tun_session_id.take(),
                inner.tun_lease.take(),
                inner.tun_link.take(),
            )
        };
        let (Some(session_id), Some(lease)) = (session_id, lease) else {
            return Ok(());
        };
        let run_root = self.config.run_root.clone();
        let attempt_session = session_id.clone();
        let attempt_lease = lease.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            if let Some(mut link) = link {
                let result = tun_lease::cleanup_tun_lease(
                    &run_root,
                    &attempt_session,
                    &attempt_lease,
                    &mut *link,
                );
                (Some(link), result)
            } else {
                // The link was lost without cleanup (should not happen: the
                // link lives exactly as long as the lease). Without a helper
                // confirmation the journal must stay so the lease is retried,
                // not forgotten.
                let error =
                    tun_helper_unavailable("tun link lost before cleanup; journal retained")
                        .retryable();
                tun_lease::note_cleanup_failure(
                    &run_root,
                    &attempt_session,
                    &attempt_lease,
                    &error,
                );
                (None, Err(error))
            }
        })
        .await;
        match outcome {
            Ok((link, Ok(()))) => {
                drop(link);
                let mut inner = self.inner.lock().await;
                inner
                    .pending_tun_cleanup
                    .retain(|pending| pending.session_id != session_id);
                Ok(())
            }
            Ok((link, Err(error))) => {
                // Keep the lease retryable in memory. Memory follows the
                // journal: the owned record is authoritative for the retry.
                let mut inner = self.inner.lock().await;
                let owned = tun_lease::read_tun_journal(&self.config.run_root, &session_id)
                    .map(|journal| journal.lease)
                    .unwrap_or(lease);
                inner.tun_session_id = Some(session_id);
                inner.tun_lease = Some(owned);
                if let Some(link) = link {
                    inner.tun_link = Some(link);
                }
                inner.pending_tun_cleanup = tun_lease::list_pending_cleanup(&self.config.run_root);
                Err(error)
            }
            Err(join) => {
                let error = DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                    .with_detail(format!("tun release task failed: {join}"));
                let mut inner = self.inner.lock().await;
                inner.tun_session_id = Some(session_id);
                inner.tun_lease = Some(lease);
                inner.pending_tun_cleanup = tun_lease::list_pending_cleanup(&self.config.run_root);
                Err(error)
            }
        }
    }

    /// Retry every durable pending TUN cleanup without re-applying anything
    /// (SP-08 explicit retry entry). The in-memory lease is tried first with
    /// its live link; journals left by a previous process get a fresh link
    /// each. `Ok` means every pending lease is confirmed released; otherwise
    /// the first error is returned and the rest stay pending. Staged until
    /// the SP-00 integrator exposes it over IPC/FRB (need N-H1); covered by
    /// SP-08 tests.
    #[allow(dead_code)]
    pub async fn retry_tun_cleanup(&self) -> Result<(), DomainError> {
        let mut first_error: Option<DomainError> = None;

        let memory = {
            let mut inner = self.inner.lock().await;
            match (
                inner.tun_session_id.take(),
                inner.tun_lease.take(),
                inner.tun_link.take(),
            ) {
                (Some(session_id), Some(lease), link) => Some((session_id, lease, link)),
                (session_id, lease, link) => {
                    inner.tun_session_id = session_id;
                    inner.tun_lease = lease;
                    if let Some(link) = link {
                        inner.tun_link = Some(link);
                    }
                    None
                }
            }
        };
        if let Some((session_id, lease, link)) = memory {
            if let Err(error) = self.retry_one_tun_cleanup(session_id, lease, link).await {
                first_error = Some(error);
            }
        }

        for pending in tun_lease::list_pending_cleanup(&self.config.run_root) {
            let held = self.inner.lock().await.tun_session_id.clone();
            if held.as_deref() == Some(pending.session_id.as_str()) {
                continue;
            }
            if tun_lease::read_tun_journal(&self.config.run_root, &pending.session_id).is_none() {
                continue;
            }
            let link = self.make_helper_link();
            let run_root = self.config.run_root.clone();
            let session_id = pending.session_id.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let mut link = link;
                tun_lease::retry_tun_cleanup(&run_root, &session_id, &mut *link)
            })
            .await;
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
                Err(join) => {
                    if first_error.is_none() {
                        first_error = Some(
                            DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                                .with_detail(format!("tun retry task failed: {join}")),
                        );
                    }
                }
            }
        }

        {
            let mut inner = self.inner.lock().await;
            inner.pending_tun_cleanup = tun_lease::list_pending_cleanup(&self.config.run_root);
        }
        match first_error {
            Some(error) => Err(error),
            None => {
                // A confirmed retry with nothing left outstanding restores the
                // closed state; a live session keeps whatever state it has.
                let mut inner = self.inner.lock().await;
                if inner.session.is_none()
                    && inner.tun_lease.is_none()
                    && inner.pending_tun_cleanup.is_empty()
                {
                    inner.detail.state = RuntimeState::Stopped;
                    inner.detail.error = None;
                }
                Ok(())
            }
        }
    }

    /// Retry one in-memory lease against its journaled owned record. On
    /// success the triple is consumed; on failure it is restored so the next
    /// retry (or the next boot) can continue.
    #[allow(dead_code)]
    async fn retry_one_tun_cleanup(
        &self,
        session_id: String,
        lease: TunLease,
        link: Option<Box<dyn HelperLink>>,
    ) -> Result<(), DomainError> {
        let run_root = self.config.run_root.clone();
        let mut link = match link {
            Some(link) => link,
            None => self.make_helper_link(),
        };
        let attempt_session = session_id.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let result = tun_lease::retry_tun_cleanup(&run_root, &attempt_session, &mut *link);
            (link, result)
        })
        .await;
        match outcome {
            Ok((link, Ok(()))) => {
                drop(link);
                Ok(())
            }
            Ok((link, Err(error))) => {
                let mut inner = self.inner.lock().await;
                let owned = tun_lease::read_tun_journal(&self.config.run_root, &session_id)
                    .map(|journal| journal.lease)
                    .unwrap_or(lease);
                inner.tun_session_id = Some(session_id);
                inner.tun_lease = Some(owned);
                inner.tun_link = Some(link);
                inner.pending_tun_cleanup = tun_lease::list_pending_cleanup(&self.config.run_root);
                Err(error)
            }
            Err(join) => {
                let error = DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                    .with_detail(format!("tun retry task failed: {join}"));
                let mut inner = self.inner.lock().await;
                inner.tun_session_id = Some(session_id);
                inner.tun_lease = Some(lease);
                inner.pending_tun_cleanup = tun_lease::list_pending_cleanup(&self.config.run_root);
                Err(error)
            }
        }
    }

    /// Wait for the TUN adapter the core just created, then fill in its index.
    /// R3-04: the core's own tun inbound creates the device, so discovery runs
    /// *after* the core is spawned and is bounded by `tun_discovery_timeout`.
    async fn resolve_deferred_tun(
        &self,
        operation_id: &str,
        base: &TunSpec,
    ) -> Result<TunSpec, DomainError> {
        let adapter = base.adapter_name.clone();
        let timeout = tun_discovery_timeout();
        let interval = tun_discovery_interval();
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(index) = self.discover_tun_interface(&adapter) {
                if index != 0 {
                    let mut spec = base.clone();
                    spec.interface_index = index;
                    for route in &mut spec.routes {
                        route.interface_index = index;
                    }
                    spec.validate()
                        .map_err(|error| error.with_operation(operation_id))?;
                    return Ok(spec);
                }
            }
            if Instant::now() >= deadline {
                return Err(DomainError::new(
                    domain::codes::TIMEOUT,
                    "error.tun_interface_timeout",
                )
                .with_operation(operation_id)
                .with_field("adapter_name")
                .with_detail(format!(
                    "TUN adapter `{adapter}` was not created within {timeout:?}"
                )));
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// Apply a fully resolved TUN descriptor through the helper and record the
    /// lease on the inner state. Same helper contract as the pre-core path; the
    /// caller rolls back (R3-05 scope) on any failure.
    async fn apply_tun_spec(
        &self,
        operation_id: &str,
        session_id: &str,
        spec: TunSpec,
    ) -> Result<(), DomainError> {
        let run_root = self.config.run_root.clone();
        let lease_session = session_id.to_string();
        let mut link = self.make_helper_link();
        let outcome = tokio::task::spawn_blocking(move || {
            let result = tun_lease::apply_tun_lease(&run_root, &lease_session, &spec, &mut *link);
            (link, result)
        })
        .await;
        let (link, lease) = match outcome {
            Ok((link, Ok(lease))) => (link, lease),
            Ok((link, Err(error))) => {
                eprintln!(
                    "[net_host] deferred tun apply failed (dry_run={}): {:?}",
                    link.dry_run(),
                    link.audit()
                );
                return Err(error.with_operation(operation_id));
            }
            Err(join) => {
                return Err(
                    DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                        .with_operation(operation_id)
                        .with_detail(format!("tun apply task failed: {join}")),
                );
            }
        };
        eprintln!(
            "[net_host] deferred tun lease applied (dry_run={}): {}",
            link.dry_run(),
            lease.summary()
        );
        let mut inner = self.inner.lock().await;
        inner.detail.tun = Some(tun_detail_from_lease(&lease));
        inner.tun_lease = Some(lease);
        inner.tun_session_id = Some(session_id.to_string());
        inner.tun_link = Some(link);
        Ok(())
    }

    /// Abort a session whose freshly spawned core could not be bound into the
    /// ownership job. Without the job the core could outlive net-host, so it is
    /// killed now and the failure surfaced as a structured, fatal error.
    async fn abort_job_assign(
        &self,
        operation_id: &str,
        child: &mut tokio::process::Child,
        journal_entry: &JournalEntry,
        cause: std::io::Error,
    ) -> Result<String, DomainError> {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        // The core never became owned; its TUN lease (if any) is released in
        // reverse before the journal is finalized.
        let _ = self.release_tun_lease().await;
        self.finalize_journal(journal_entry).await;
        journal::remove_staged_artifacts(&self.config.run_root, &journal_entry.session_id);
        let error = job_assign_failed(operation_id, cause.to_string());
        self.fail_operation(operation_id, &error).await;
        self.bus.emit_named(
            "error_raised",
            json!({
                "code": error.code,
                "message_key": error.message_key,
                "detail": error.detail,
            }),
        );
        eprintln!(
            "[net_host] job assign failed, session aborted: {}",
            error.code
        );
        Err(error)
    }

    /// Side-effect-free preflight of a plan (RR-10). None of it touches the
    /// running session, so a doomed switch is rejected before the old core is
    /// stopped.
    async fn precheck_plan(&self, plan: &RuntimePlan) -> Result<PreparedPlan, DomainError> {
        self.precheck_plan_with_exe(plan, None).await
    }

    async fn precheck_plan_with_exe(
        &self,
        plan: &RuntimePlan,
        exe_override: Option<PathBuf>,
    ) -> Result<PreparedPlan, DomainError> {
        let core = plan.target.core_type;
        let adapter = adapter_for(core).ok_or_else(|| {
            DomainError::new(domain::codes::NOT_FOUND, "error.core_not_supported")
                .with_detail(format!("no adapter for {}", core.as_str()))
        })?;
        let body = match &plan.target.config {
            ConfigSource::Inline { body } => body.clone(),
            ConfigSource::ControlledFile { .. } => {
                return Err(DomainError::new(
                    domain::codes::INVALID_PLAN,
                    "error.plan_unsupported",
                )
                .with_detail("controlled-file configs are not supported in T03"));
            }
        };
        let actual_hash = sha256_hex(body.as_bytes());
        let declared = plan.target.config_sha256.as_str();
        if !declared.is_empty() && declared != actual_hash {
            return Err(DomainError::new(
                domain::codes::INVALID_PLAN,
                "error.config_hash_mismatch",
            )
            .with_field("config_sha256")
            .with_detail(format!("declared {declared}, computed {actual_hash}")));
        }

        let port = plan
            .ports
            .iter()
            .find(|p| {
                matches!(
                    p.transport,
                    domain::PortTransport::Tcp | domain::PortTransport::Both
                )
            })
            .map(|p| p.port)
            .or_else(|| plan.ports.first().map(|p| p.port))
            .unwrap_or(0);

        // The running session may still own the target port (an in-place
        // switch). Only probe ports that are not currently ours.
        let owned = self.inner.lock().await.session.as_ref().map(|s| s.port);
        if port != 0 && Some(port) != owned {
            preflight_port(port)?;
        }

        let exe = match exe_override {
            Some(exe) if exe.is_file() => exe,
            _ => {
                let locator = CoreLocator::from_env();
                locator.resolve(core, plan.target.version.as_deref())?
            }
        };
        // R3-04: a deferred descriptor is not resolved yet, so the strict
        // resolver must not reject the plan for a missing `tun` node.
        let deferred_tun = deferred_tun_from_plan(plan)?;
        let tun_spec = if deferred_tun.is_some() {
            None
        } else {
            tun_spec_from_plan(plan)?
        };

        // Real config validation (`xray run -test` / `sing-box check`): a bad
        // config must fail before the old core is stopped. Stub-core tests can
        // opt out with `V2RAYN_R_SKIP_CONFIG_CHECK=1`.
        if !skip_config_check() {
            self.run_config_check(&*adapter, &exe, &body, &actual_hash)
                .await?;
        }

        // Every nested process-graph node (the pre-SOCKS sidecar today) is
        // resolved and validated here too: a doomed secondary process must fail
        // before the old session stops, never after the main core is up.
        let sidecars = self.prepare_sidecars(plan).await?;

        Ok(PreparedPlan {
            adapter,
            exe,
            body,
            actual_hash,
            port,
            tun_spec,
            deferred_tun,
            sidecars,
        })
    }

    /// Resolve adapters, executables and validated configs for every nested
    /// process-graph node except the plan target. The target's process-node id
    /// is the core token (`CoreType::as_str`).
    async fn prepare_sidecars(
        &self,
        plan: &RuntimePlan,
    ) -> Result<Vec<PreparedSidecar>, DomainError> {
        let core_id = plan.target.core_type.as_str();
        let order = plan.process_graph.start_order()?;
        let mut prepared = Vec::new();
        for id in order {
            if id == core_id || id == runtime::TUN_PROCESS_ID || id == TUN_DEFERRED_PROCESS_ID {
                continue;
            }
            let Some(node) = plan.process_graph.nodes.iter().find(|node| node.id == id) else {
                continue;
            };
            let adapter = adapter_for(node.core_type).ok_or_else(|| {
                DomainError::new(domain::codes::NOT_FOUND, "error.core_not_supported").with_detail(
                    format!("no adapter for sidecar {}", node.core_type.as_str()),
                )
            })?;
            let node_body = match &node.config {
                ConfigSource::Inline { body } => body.clone(),
                ConfigSource::ControlledFile { .. } => {
                    return Err(DomainError::new(
                        domain::codes::INVALID_PLAN,
                        "error.plan_unsupported",
                    )
                    .with_detail("controlled-file sidecar configs are not supported"));
                }
            };
            let node_hash = sha256_hex(node_body.as_bytes());
            let node_port = plan
                .ports
                .iter()
                .find(|candidate| candidate.owner == node.id)
                .or_else(|| node.ports.first())
                .map(|candidate| candidate.port)
                .unwrap_or(0);
            let locator = CoreLocator::from_env();
            // The sidecar may be a different core than the target (e.g. a
            // sing-box pre-SOCKS sidecar under an Xray node), so the target's
            // pinned version must not be applied to it; use the installed core.
            let node_exe = locator.resolve(node.core_type, None)?;
            if !skip_config_check() {
                self.run_config_check(&*adapter, &node_exe, &node_body, &node_hash)
                    .await?;
            }
            let elevated = plan.network_policy.tun_enabled && body_has_tun_inbound(&node_body);
            prepared.push(PreparedSidecar {
                id: node.id.clone(),
                adapter,
                exe: node_exe,
                body: node_body,
                port: node_port,
                elevated,
            });
        }
        Ok(prepared)
    }

    /// Validate a staged config with the core's own `test_args`, bounded and
    /// without binding any listener.
    async fn run_config_check(
        &self,
        adapter: &dyn CoreAdapter,
        exe: &Path,
        body: &str,
        hash: &str,
    ) -> Result<(), DomainError> {
        let dir = self.config.run_root.join("precheck");
        std::fs::create_dir_all(&dir).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("create precheck dir failed: {e}"))
        })?;
        let path = dir.join(format!("check-{hash}.json"));
        std::fs::write(&path, body.as_bytes()).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("write precheck config failed: {e}"))
        })?;
        let args: Vec<std::ffi::OsString> = adapter.test_args(&path);
        let exe = exe.to_path_buf();
        let limit = config_check_timeout();
        let result = run_bounded_output(&exe, &args, limit).await;
        let _ = std::fs::remove_file(&path);
        match result {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
                text.push_str(&String::from_utf8_lossy(&output.stdout));
                let tail = text
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ");
                Err(
                    DomainError::new(domain::codes::INVALID_PLAN, "error.config_check_failed")
                        .with_field("config")
                        .with_detail(format!("core config check failed: {tail}")),
                )
            }
            Err(RunBoundedError::Spawn(detail)) => Err(DomainError::new(
                domain::codes::UNAVAILABLE,
                "error.core_spawn_failed",
            )
            .with_detail(format!("config check spawn failed: {detail}"))),
            Err(RunBoundedError::Timeout) => Err(DomainError::new(
                domain::codes::TIMEOUT,
                "error.config_check_timeout",
            )
            .with_field("config")
            .with_detail(format!(
                "core config check exceeded {limit:?} and the managed process was terminated"
            ))),
        }
    }

    /// Apply an immutable plan.
    ///
    /// RR-10 ordering: precheck (adapter/locator/hash/port/TUN/`test_args`)
    /// runs before the old session is stopped; a precheck failure keeps the old
    /// session running. If the switch still fails after the stop, the previous
    /// good plan is restored when possible, otherwise the runtime truthfully
    /// stays stopped.
    pub async fn apply_plan(&self, plan: RuntimePlan) -> Result<String, DomainError> {
        if let Err(error) = plan.validate() {
            let mut inner = self.inner.lock().await;
            inner.detail.error = Some(error.clone());
            drop(inner);
            return Err(error);
        }

        // SP-04: join the single command sequence. Precheck, stop of the old
        // session, spawn and readiness all happen while holding the gate, so
        // a concurrent stop (or a second apply from another connection) can
        // neither interleave nor be swallowed. The gate is always released on
        // return, including every failure path below.
        let _command = self.command_gate.lock().await;

        let prepared = match self.precheck_plan(&plan).await {
            Ok(prepared) => prepared,
            Err(error) => {
                {
                    let mut inner = self.inner.lock().await;
                    inner.detail.error = Some(error.clone());
                }
                self.bus.emit_named(
                    "error_raised",
                    json!({
                        "code": error.code,
                        "message_key": error.message_key,
                        "detail": error.detail,
                    }),
                );
                eprintln!(
                    "[net_host] switch precheck failed, keeping current session: {}",
                    error.code
                );
                return Err(error);
            }
        };

        let (previous_plan, previous_exe) = {
            let inner = self.inner.lock().await;
            (inner.last_plan.clone(), inner.last_exe.clone())
        };
        let had_session = { self.inner.lock().await.session.is_some() };

        match self.start_prepared(plan.clone(), prepared).await {
            Ok(operation_id) => {
                let mut inner = self.inner.lock().await;
                inner.last_plan = Some(plan);
                Ok(operation_id)
            }
            Err(error) => {
                if had_session {
                    self.try_restore(previous_plan, previous_exe).await;
                }
                Err(error)
            }
        }
    }

    /// Best-effort restore of the previous good session after a failed switch.
    /// The previous executable is reused verbatim. When it cannot be restored
    /// the runtime truthfully stays stopped.
    async fn try_restore(&self, previous: Option<RuntimePlan>, previous_exe: Option<PathBuf>) {
        let Some(previous) = previous else {
            return;
        };
        match self.precheck_plan_with_exe(&previous, previous_exe).await {
            Ok(prepared) => match self.start_prepared(previous.clone(), prepared).await {
                Ok(_) => {
                    let mut inner = self.inner.lock().await;
                    inner.last_plan = Some(previous);
                    eprintln!("[net_host] restored previous session after failed switch");
                }
                Err(error) => eprintln!(
                    "[net_host] previous session could not be restored: {}",
                    error.code
                ),
            },
            Err(error) => eprintln!(
                "[net_host] previous session precheck failed during restore: {}",
                error.code
            ),
        }
    }

    /// Stage, spawn and probe a prechecked plan. The running session (if any)
    /// is stopped first.
    async fn start_prepared(
        &self,
        plan: RuntimePlan,
        prepared: PreparedPlan,
    ) -> Result<String, DomainError> {
        let operation_id = next_operation_id();
        if let Err(error) = plan.validate() {
            let mut inner = self.inner.lock().await;
            inner.detail.error = Some(error.clone());
            drop(inner);
            return Err(error);
        }

        // Stop any existing managed session before switching. The gate is
        // already held, so this takes the inner (non-gated) path: taking the
        // public `stop_managed` here would deadlock on the same gate.
        let _ = self.stop_managed_inner(None).await;
        // A fresh runtime generation gets a new event epoch.
        self.bus.bump_epoch();

        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::Validating;
            inner.detail.error = None;
            inner.detail.operation_id = Some(operation_id.clone());
            inner.detail.tun = None;
            inner.active_operation = Some(operation_id.clone());
            inner.operations.insert(
                operation_id.clone(),
                OperationStatus {
                    operation_id: operation_id.clone(),
                    job_id: None,
                    state: domain::JobState::Running,
                    cancel: None,
                    error: None,
                },
            );
        }
        self.bus.emit_named(
            "runtime_state_changed",
            serde_json::to_value(RuntimeStateChanged {
                state: RuntimeState::Validating,
                applied_revision: 0,
                message_key: None,
            })
            .unwrap_or(json!({})),
        );

        // --- Preparing: stage config, verify hash, journal ---
        // Every value here was resolved by `precheck_plan` before the old
        // session was stopped.
        let PreparedPlan {
            adapter,
            exe,
            body,
            actual_hash,
            port,
            tun_spec,
            deferred_tun,
            sidecars,
        } = prepared;

        let session_seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
        let session_id = format!("s-{}-{}", journal::now_ms(), session_seq);
        let dir = journal::session_dir(&self.config.run_root, &session_id);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("create session dir failed: {e}"));
            self.fail_operation(&operation_id, &error).await;
            return Err(error);
        }
        // The staged config carries inline credentials: restrict the dir (and
        // inheritable files) to the current user before writing the body.
        if let Err(e) = crate::dacl::restrict_to_current_user(&dir, true) {
            eprintln!("[net_host] staged dir ACL not applied: {e}");
        }
        let config_path = dir.join("config.json");
        if let Err(e) = std::fs::write(&config_path, body.as_bytes()) {
            let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("write config failed: {e}"));
            self.fail_operation(&operation_id, &error).await;
            return Err(error);
        }
        if let Err(e) = crate::dacl::restrict_to_current_user(&config_path, false) {
            eprintln!("[net_host] staged config ACL not applied: {e}");
        }
        let log_path = dir.join("core.log");
        let journal_entry = JournalEntry {
            session_id: session_id.clone(),
            plan_id: plan.plan_id.clone(),
            desired_revision: plan.desired_revision,
            config_sha256: actual_hash.clone(),
            stage: RecoveryStage::Prepared,
            pid: None,
            created_at_ms: None,
            port,
            updated_at_ms: journal::now_ms(),
        };
        let _ = journal::write_entry(&self.config.run_root, &journal_entry);

        // Preflight: a port that cannot be bound now will never bind later.
        if port != 0 {
            if let Err(error) = preflight_port(port) {
                let error = error.with_operation(&operation_id);
                let _ = journal::write_entry(
                    &self.config.run_root,
                    &JournalEntry {
                        stage: RecoveryStage::Finalized,
                        ..journal_entry.clone()
                    },
                );
                self.fail_operation(&operation_id, &error).await;
                return Err(error);
            }
        }

        // --- TUN: validated descriptor -> helper routes/adapter (T14) ---
        //
        // Order is prepare-config -> helper -> spawn-core -> ready -> Applied.
        // The helper owns the elevated work; net-host only records the lease.
        // Any helper failure is structural (`E_TUN_HELPER_UNAVAILABLE`): the
        // plan fails here, before any core is spawned, and never degrades to
        // a direct TUN path. `tun_spec` was validated during precheck.
        if let Some(spec) = tun_spec {
            let run_root = self.config.run_root.clone();
            let lease_session = session_id.clone();
            let mut link = self.make_helper_link();
            // Blocking pipe I/O must not stall the async runtime.
            let outcome = tokio::task::spawn_blocking(move || {
                let result =
                    tun_lease::apply_tun_lease(&run_root, &lease_session, &spec, &mut *link);
                (link, result)
            })
            .await;
            let (link, lease) = match outcome {
                Ok((link, Ok(lease))) => (link, lease),
                Ok((link, Err(error))) => {
                    let error = error.with_operation(&operation_id);
                    eprintln!(
                        "[net_host] tun apply failed (dry_run={}): {:?}",
                        link.dry_run(),
                        link.audit()
                    );
                    let _ = journal::write_entry(
                        &self.config.run_root,
                        &JournalEntry {
                            stage: RecoveryStage::Finalized,
                            ..journal_entry.clone()
                        },
                    );
                    journal::remove_staged_artifacts(&self.config.run_root, &session_id);
                    self.fail_operation(&operation_id, &error).await;
                    self.bus.emit_named(
                        "error_raised",
                        json!({
                            "code": error.code,
                            "message_key": error.message_key,
                            "detail": error.detail,
                        }),
                    );
                    eprintln!(
                        "[net_host] tun helper unavailable, rolled back before core start: {}",
                        error.code
                    );
                    return Err(error);
                }
                Err(join) => {
                    let error = DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                        .with_operation(&operation_id)
                        .with_detail(format!("tun apply task failed: {join}"));
                    let _ = journal::write_entry(
                        &self.config.run_root,
                        &JournalEntry {
                            stage: RecoveryStage::Finalized,
                            ..journal_entry.clone()
                        },
                    );
                    journal::remove_staged_artifacts(&self.config.run_root, &session_id);
                    self.fail_operation(&operation_id, &error).await;
                    return Err(error);
                }
            };
            eprintln!(
                "[net_host] tun lease applied (dry_run={}): {}",
                link.dry_run(),
                lease.summary()
            );
            {
                let mut inner = self.inner.lock().await;
                inner.detail.tun = Some(tun_detail_from_lease(&lease));
                inner.tun_lease = Some(lease);
                inner.tun_session_id = Some(session_id.clone());
                // Keep the helper connection open for the whole runtime
                // session: dropping it lets the helper reclaim the lease via
                // its disconnect cleanup.
                inner.tun_link = Some(link);
            }
        }

        // --- Nested process graph (RR-06 / R3-02) ---
        // Frozen order (`CoreManager.LoadCore`): start the main core, wait for
        // its proxy port (`WaitForProxyPort`), then start the pre-SOCKS
        // sidecar(s). The sidecars are started in the `Ready` arm below, after
        // the main core is up, so the sidecar can dial the main core's port
        // that the engine baked into its config.

        // --- Starting ---
        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::Starting;
            inner.detail.session_id = Some(session_id.clone());
            inner.detail.config_sha256 = Some(actual_hash.clone());
            inner.detail.ports = vec![port];
        }
        let _ = journal::write_entry(
            &self.config.run_root,
            &JournalEntry {
                stage: RecoveryStage::Applying,
                ..journal_entry.clone()
            },
        );

        let job = match JobGuard::create_kill_on_close() {
            Ok(job) => job,
            Err(e) => {
                let error = DomainError::new(domain::codes::UNAVAILABLE, "error.job_create_failed")
                    .with_operation(&operation_id)
                    .with_detail(e.to_string());
                let _ = self.release_tun_lease().await;
                self.finalize_journal(&journal_entry).await;
                self.fail_operation(&operation_id, &error).await;
                return Err(error);
            }
        };

        let mut command = Command::new(&exe);
        command
            .args(adapter.run_args(&config_path))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // R3-03: cores like mieru take their config path only via the
        // environment (`MIERU_CONFIG_JSON_FILE`), and some cores expect a
        // specific working directory. Both come from the adapter contract.
        for (key, value) in adapter.env_vars(&config_path) {
            command.env(key, value);
        }
        if let Some(dir) = adapter.working_dir(&config_path) {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let error = DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                    .with_operation(&operation_id)
                    .with_detail(format!("{}: {e}", exe.display()));
                let _ = self.release_tun_lease().await;
                self.finalize_journal(&journal_entry).await;
                self.fail_operation(&operation_id, &error).await;
                return Err(error);
            }
        };

        let pid = child.id().unwrap_or(0);
        let created_at_ms = process_creation_time_ms(pid).unwrap_or(0);
        let identity = ProcessIdentity::new(pid, created_at_ms);
        #[cfg(windows)]
        {
            // Binding the fresh core into the ownership job is mandatory: an
            // unbound core would survive a net-host crash and leak a port. A
            // failure therefore aborts the session and kills the child.
            let assignment = match child.raw_handle() {
                Some(handle) => job.assign(handle),
                None => Err(std::io::Error::other("core process handle unavailable")),
            };
            if let Err(e) = assignment {
                let result = self
                    .abort_job_assign(&operation_id, &mut child, &journal_entry, e)
                    .await;
                return result;
            }
        }

        // Stream stdout/stderr to the session log, with bounded memory, and
        // forward each line as a `log_line` event (T15a minimal change) so the
        // application log pipeline does not have to tail the file.
        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(stdout, log_path.clone(), "stdout", self.bus.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(stderr, log_path.clone(), "stderr", self.bus.clone());
        }

        let _ = journal::write_entry(
            &self.config.run_root,
            &JournalEntry {
                stage: RecoveryStage::Applying,
                pid: Some(pid),
                created_at_ms: Some(created_at_ms),
                ..journal_entry.clone()
            },
        );

        // --- Checking: readiness probe ---
        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::Checking;
            inner.detail.pid = Some(pid);
            inner.detail.created_at_ms = Some(created_at_ms);
        }
        let outcome = wait_ready(
            &mut child,
            port,
            self.config.readiness_timeout,
            self.config.readiness_interval,
        )
        .await;

        match outcome {
            ReadyOutcome::Ready => {
                // R3-02/R3-05: the main core is up (and its proxy port is
                // ready). Start the pre-SOCKS sidecar(s) now. Any failure here
                // runs the same failure cleanup scope as the core branches:
                // stop the started sidecars, kill the main core, release the
                // TUN lease and finalize the journal.
                let mut sidecar_sessions = match self
                    .start_sidecars(&operation_id, &session_id, sidecars)
                    .await
                {
                    Ok(sessions) => sessions,
                    Err(error) => {
                        return self
                            .rollback(&operation_id, &mut child, &session_id, error)
                            .await;
                    }
                };
                // --- R3-04: deferred TUN path, after the TUN-providing
                // sidecar is up. The sidecar's tun inbound creates the adapter
                // (elevated through the helper); only then can discovery match
                // the interface. Any failure stops the sidecars, kills the
                // main core, releases the lease and finalizes the journal.
                if let Some(base_spec) = deferred_tun {
                    let spec = match self.resolve_deferred_tun(&operation_id, &base_spec).await {
                        Ok(spec) => spec,
                        Err(error) => {
                            self.stop_sidecars(&mut sidecar_sessions).await;
                            return self
                                .rollback(&operation_id, &mut child, &session_id, error)
                                .await;
                        }
                    };
                    if let Err(error) = self.apply_tun_spec(&operation_id, &session_id, spec).await
                    {
                        self.stop_sidecars(&mut sidecar_sessions).await;
                        return self
                            .rollback(&operation_id, &mut child, &session_id, error)
                            .await;
                    }
                }
                let session = Session {
                    session_id: session_id.clone(),
                    plan_id: plan.plan_id.clone(),
                    desired_revision: plan.desired_revision,
                    port,
                    config_sha256: actual_hash.clone(),
                    exe,
                    child,
                    identity,
                    sidecars: sidecar_sessions,
                    job,
                };
                // Stay `Applied` (not `Finalized`) while the core runs: if
                // net-host dies now, the next boot must be able to reclaim the
                // core by `(pid, creation_time)`. Only stop/rollback finalize.
                let _ = journal::write_entry(
                    &self.config.run_root,
                    &JournalEntry {
                        stage: RecoveryStage::Applied,
                        pid: Some(pid),
                        created_at_ms: Some(created_at_ms),
                        updated_at_ms: journal::now_ms(),
                        ..journal_entry.clone()
                    },
                );
                {
                    let mut inner = self.inner.lock().await;
                    inner.detail.state = RuntimeState::Running;
                    inner.detail.applied_revision = plan.desired_revision;
                    inner.detail.pid = Some(pid);
                    inner.detail.created_at_ms = Some(created_at_ms);
                    inner.detail.ports = vec![port];
                    inner.detail.error = None;
                    inner.active_operation = None;
                    inner.operations.insert(
                        operation_id.clone(),
                        OperationStatus {
                            operation_id: operation_id.clone(),
                            job_id: None,
                            state: domain::JobState::Done,
                            cancel: None,
                            error: None,
                        },
                    );
                    inner.last_exe = Some(session.exe.clone());
                    inner.session = Some(session);
                    // A fresh session clears the previous exit fact: the new
                    // generation has no unsolicited exit yet.
                    inner.last_exit = None;
                    inner.last_exit_sidecar = None;
                    inner.actual_generation = 0;
                }
                eprintln!(
                    "[net_host] session {session_id} RUNNING pid={pid} created_at_ms={created_at_ms} port={port} rev={}",
                    plan.desired_revision
                );
                self.bus.emit_named(
                    "runtime_state_changed",
                    serde_json::to_value(RuntimeStateChanged {
                        state: RuntimeState::Running,
                        applied_revision: plan.desired_revision,
                        message_key: None,
                    })
                    .unwrap_or(json!({})),
                );
                Ok(operation_id)
            }
            ReadyOutcome::Exited(code) => {
                let tail = tail_log(&log_path, 8);
                let error = DomainError::new(domain::codes::INTERNAL, "error.core_exited")
                    .with_operation(&operation_id)
                    .with_detail(format!("core exited early (code {code:?}): {tail}"));
                self.rollback(&operation_id, &mut child, &session_id, error)
                    .await
            }
            ReadyOutcome::Timeout => {
                // SP-10: the deadline fired, but the outcome is unknown until
                // the actual result is re-checked: an exit at the edge must
                // report `core_exited`, not a timeout.
                if let Some(status) = child.try_wait().ok().flatten() {
                    let tail = tail_log(&log_path, 8);
                    let error = DomainError::new(domain::codes::INTERNAL, "error.core_exited")
                        .with_operation(&operation_id)
                        .with_detail(format!("core exited (code {:?}): {tail}", status.code()));
                    return self
                        .rollback(&operation_id, &mut child, &session_id, error)
                        .await;
                }
                let error = DomainError::new(domain::codes::TIMEOUT, "error.readiness_timeout")
                    .with_operation(&operation_id)
                    .with_field("port")
                    .with_detail(format!(
                        "port {port} not ready within {:?}",
                        self.config.readiness_timeout
                    ));
                self.rollback(&operation_id, &mut child, &session_id, error)
                    .await
            }
        }
    }

    /// Start the prepared sidecar processes (RR-06 / R3-02). Called after the
    /// main core is spawned and its proxy port is ready, matching the frozen
    /// `CoreManager.LoadCore` order: main core -> `WaitForProxyPort` ->
    /// `CoreStartPreService`. On any failure the already-started sidecars are
    /// stopped and the error is returned; the caller then rolls back the main
    /// core, TUN lease and journal (R3-05).
    async fn start_sidecars(
        &self,
        operation_id: &str,
        session_id: &str,
        sidecars: Vec<PreparedSidecar>,
    ) -> Result<Vec<SidecarSession>, DomainError> {
        let mut started: Vec<SidecarSession> = Vec::new();
        for sidecar in sidecars {
            match self
                .start_one_sidecar(operation_id, session_id, &sidecar)
                .await
            {
                Ok(session) => started.push(session),
                Err(error) => {
                    self.stop_sidecars(&mut started).await;
                    return Err(error);
                }
            }
        }
        Ok(started)
    }

    /// Start a TUN-providing sidecar elevated through the helper. The core
    /// executable and its runtime DLLs are staged inside the controlled
    /// sidecar directory (the helper requires `exe_parent == run_dir` and
    /// `run_dir` under an allowed root), then the helper session stays open for
    /// the sidecar's lifetime so the lease keeps owning the process.
    async fn start_elevated_sidecar(
        &self,
        operation_id: &str,
        sidecar: &PreparedSidecar,
        sidecar_dir: &std::path::Path,
        sidecar_config: &std::path::Path,
    ) -> Result<SidecarSession, DomainError> {
        let core_id = elevated_core_id(sidecar.adapter.core_type()).ok_or_else(|| {
            DomainError::new(domain::codes::INVALID_PLAN, "error.core_not_supported").with_detail(
                format!(
                    "core {} cannot run elevated for TUN",
                    sidecar.adapter.core_type().as_str()
                ),
            )
        })?;
        let exe_name = sidecar.exe.file_name().ok_or_else(|| {
            DomainError::new(domain::codes::INVALID_PLAN, "error.plan_unsupported")
                .with_detail("sidecar executable has no file name")
        })?;
        let staged_exe = sidecar_dir.join(exe_name);
        std::fs::copy(&sidecar.exe, &staged_exe).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_operation(operation_id)
                .with_detail(format!("stage elevated core failed: {e}"))
        })?;
        // Runtime DLLs (wintun.dll for sing-box) must sit next to the staged
        // executable; copy the sibling runtime files but never archives/logs.
        if let Some(parent) = sidecar.exe.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.eq_ignore_ascii_case(&exe_name.to_string_lossy()) {
                        continue;
                    }
                    let lower = name_str.to_ascii_lowercase();
                    if lower.ends_with(".zip") || lower.ends_with(".log") {
                        continue;
                    }
                    let _ = std::fs::copy(&path, sidecar_dir.join(&name));
                }
            }
        }
        let args: Vec<String> = sidecar
            .adapter
            .run_args(sidecar_config)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let spec = ipc_contract::ElevatedCoreSpec {
            core: core_id.to_string(),
            exe_path: staged_exe.to_string_lossy().into_owned(),
            args,
            run_dir: sidecar_dir.to_string_lossy().into_owned(),
        };
        // The helper launch (UAC) and pipe handshake block; keep them off the
        // async worker.
        let mut link = self.make_helper_link();
        let spec_for_task = spec.clone();
        let (link, outcome) = tokio::task::spawn_blocking(move || {
            let result = link.run_core(&spec_for_task);
            (link, result)
        })
        .await
        .map_err(|join| {
            DomainError::new(domain::codes::INTERNAL, "error.tun_apply_failed")
                .with_operation(operation_id)
                .with_detail(format!("elevated core task failed: {join}"))
        })?;
        let (handle, pid) = outcome.map_err(|error| error.with_operation(operation_id))?;
        // SP-10: a real launch without a helper process identity is not
        // ready. Dry-run links report handle 0 by design (simulated, no
        // process owned); only the real path must carry an identity.
        let dry_run = link.dry_run();
        elevated_launch_verdict(handle, dry_run)
            .map_err(|error| error.with_operation(operation_id))?;
        Ok(SidecarSession {
            id: sidecar.id.clone(),
            child: None,
            _job: None,
            helper: Some(link),
            handle: Some(handle),
            elevated_pid: if pid == 0 { None } else { Some(pid) },
            elevated_exit_code: None,
        })
    }

    /// Stage, spawn and readiness-probe one sidecar. The sidecar is bound into
    /// the kill-on-close job exactly like the main core.
    async fn start_one_sidecar(
        &self,
        operation_id: &str,
        session_id: &str,
        sidecar: &PreparedSidecar,
    ) -> Result<SidecarSession, DomainError> {
        let sidecar_dir = journal::session_dir(&self.config.run_root, session_id)
            .join("processes")
            .join(&sidecar.id);
        std::fs::create_dir_all(&sidecar_dir).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_operation(operation_id)
                .with_detail(format!("create sidecar dir failed: {e}"))
        })?;
        let sidecar_config = sidecar_dir.join("config.json");
        std::fs::write(&sidecar_config, sidecar.body.as_bytes()).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_operation(operation_id)
                .with_detail(format!("write sidecar config failed: {e}"))
        })?;
        if sidecar.elevated {
            return self
                .start_elevated_sidecar(operation_id, sidecar, &sidecar_dir, &sidecar_config)
                .await;
        }
        let sidecar_log = sidecar_dir.join("core.log");
        let mut command = Command::new(&sidecar.exe);
        command
            .args(sidecar.adapter.run_args(&sidecar_config))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in sidecar.adapter.env_vars(&sidecar_config) {
            command.env(key, value);
        }
        if let Some(dir) = sidecar.adapter.working_dir(&sidecar_config) {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|e| {
            DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                .with_operation(operation_id)
                .with_detail(format!("{}: {e}", sidecar.exe.display()))
        })?;
        let sidecar_job = match JobGuard::create_kill_on_close() {
            Ok(job) => job,
            Err(e) => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                return Err(DomainError::new(
                    domain::codes::UNAVAILABLE,
                    "error.job_create_failed",
                )
                .with_operation(operation_id)
                .with_detail(e.to_string()));
            }
        };
        #[cfg(windows)]
        {
            let assignment = match child.raw_handle() {
                Some(handle) => sidecar_job.assign(handle),
                None => Err(std::io::Error::other("sidecar process handle unavailable")),
            };
            if let Err(e) = assignment {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                return Err(job_assign_failed(operation_id, e.to_string()));
            }
        }
        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(
                stdout,
                sidecar_log.clone(),
                Box::leak(sidecar.id.clone().into_boxed_str()),
                self.bus.clone(),
            );
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(
                stderr,
                sidecar_log.clone(),
                Box::leak(sidecar.id.clone().into_boxed_str()),
                self.bus.clone(),
            );
        }
        let mut session = SidecarSession {
            id: sidecar.id.clone(),
            child: Some(child),
            _job: Some(sidecar_job),
            helper: None,
            handle: None,
            elevated_pid: None,
            elevated_exit_code: None,
        };
        // Wait for the sidecar to accept the same SOCKS greeting the main core
        // accepts (`WaitForProxyPort` upstream). A TUN-only sidecar reports
        // port 0 and needs no TCP probe.
        if sidecar.port != 0 {
            let ready = wait_socks_port(
                sidecar.port,
                self.config.readiness_timeout,
                self.config.readiness_interval,
            )
            .await;
            if !ready {
                // SP-10: re-check the actual result before attributing the
                // failure: an exited sidecar reports `core_exited` with its
                // code; only a live-but-silent one is a readiness timeout.
                let exited = session
                    .child
                    .as_mut()
                    .and_then(|child| child.try_wait().ok())
                    .flatten();
                session.terminate_and_wait(Duration::from_secs(5)).await;
                if let Some(status) = exited {
                    return Err(
                        DomainError::new(domain::codes::INTERNAL, "error.core_exited")
                            .with_operation(operation_id)
                            .with_detail(format!(
                                "sidecar {} exited (code {:?})",
                                sidecar.id,
                                status.code()
                            )),
                    );
                }
                return Err(
                    DomainError::new(domain::codes::TIMEOUT, "error.readiness_timeout")
                        .with_operation(operation_id)
                        .with_field("port")
                        .with_detail(format!(
                            "sidecar {} port {} not ready within {:?}",
                            sidecar.id, sidecar.port, self.config.readiness_timeout
                        )),
                );
            }
        }
        Ok(session)
    }

    /// Stop a set of started sidecars in reverse order (RR-06). Used when a
    /// later step of the same session fails after the sidecars are up.
    async fn stop_sidecars(&self, sidecars: &mut Vec<SidecarSession>) {
        for sidecar in sidecars.iter_mut().rev() {
            sidecar.terminate_and_wait(Duration::from_secs(5)).await;
        }
        sidecars.clear();
    }

    /// Roll back a partially started session and surface the root cause.
    async fn rollback(
        &self,
        operation_id: &str,
        child: &mut tokio::process::Child,
        session_id: &str,
        error: DomainError,
    ) -> Result<String, DomainError> {
        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::RollingBack;
        }
        let _ = child.start_kill();
        let killed = tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .is_ok();
        // Reverse cleanup: the core is dead, so its TUN resources go next.
        let _ = self.release_tun_lease().await;
        let _ = journal::write_entry(
            &self.config.run_root,
            &JournalEntry {
                session_id: session_id.to_string(),
                plan_id: String::new(),
                desired_revision: 0,
                config_sha256: String::new(),
                stage: RecoveryStage::Finalized,
                pid: None,
                created_at_ms: None,
                port: 0,
                updated_at_ms: journal::now_ms(),
            },
        );
        journal::remove_staged_artifacts(&self.config.run_root, session_id);
        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = if killed {
                RuntimeState::Stopped
            } else {
                RuntimeState::Degraded
            };
            inner.detail.pid = None;
            inner.detail.created_at_ms = None;
            inner.active_operation = None;
            // A failed candidate must not publish an endpoint.
            inner.detail.ports.clear();
            inner.detail.session_id = None;
            inner.detail.config_sha256 = None;
            inner.detail.error = Some(error.clone());
            inner.operations.insert(
                operation_id.to_string(),
                OperationStatus {
                    operation_id: operation_id.to_string(),
                    job_id: None,
                    state: domain::JobState::Failed,
                    cancel: None,
                    error: Some(error.clone()),
                },
            );
        }
        self.bus.emit_named(
            "error_raised",
            json!({
                "code": error.code,
                "message_key": error.message_key,
                "detail": error.detail,
            }),
        );
        eprintln!(
            "[net_host] apply failed: {} {}",
            error.code, error.message_key
        );
        Err(error)
    }

    /// Stop the managed session (idempotent).
    ///
    /// SP-04: joins the single command sequence behind any in-flight apply:
    /// a stop admitted later always runs after the earlier command reaches
    /// its safe point, and is never overtaken by a later apply. An in-flight
    /// apply itself cannot be cancelled mid-flight (the client reports
    /// `NotCancellable`); the stop waits for it instead of interrupting it.
    pub async fn stop_managed(&self, operation_id: Option<String>) -> Option<String> {
        let _command = self.command_gate.lock().await;
        self.stop_managed_inner(operation_id).await
    }

    /// Record one stop as a terminal operation entry (SP-05). Earlier apply
    /// entries are never touched: a stop only appends its own id, so history
    /// still shows what actually ran. SP-08: when the TUN release failed, the
    /// stop is recorded as Failed with the cleanup error — a stop with an
    /// unconfirmed cleanup is not a clean shutdown.
    async fn record_stop_operation(
        &self,
        operation_id: &Option<String>,
        tun_result: &Result<(), DomainError>,
    ) {
        let Some(operation_id) = operation_id else {
            return;
        };
        let mut inner = self.inner.lock().await;
        let entry = match tun_result {
            Ok(()) => OperationStatus {
                operation_id: operation_id.clone(),
                job_id: None,
                state: domain::JobState::Done,
                cancel: None,
                error: None,
            },
            Err(error) => OperationStatus {
                operation_id: operation_id.clone(),
                job_id: None,
                state: domain::JobState::Failed,
                cancel: None,
                error: Some(error.clone()),
            },
        };
        inner.operations.insert(operation_id.clone(), entry);
    }

    /// The stop body. The caller must hold [`command_gate`](HostState::command_gate).
    ///
    /// SP-05: a stop withdraws the live endpoint but never rewrites operation
    /// history. When the caller passes an operation id it is recorded as a
    /// terminal `Done` entry so a later `GetOperation` reconciles instead of
    /// reporting not-found; entries of earlier applies are retained.
    async fn stop_managed_inner(&self, operation_id: Option<String>) -> Option<String> {
        let mut session = {
            let mut inner = self.inner.lock().await;
            match inner.session.take() {
                Some(session) => {
                    inner.detail.state = RuntimeState::RollingBack;
                    session
                }
                None => {
                    inner.detail.pid = None;
                    inner.detail.created_at_ms = None;
                    // No running session to restore from.
                    inner.last_plan = None;
                    inner.last_exe = None;
                    inner.detail.state = RuntimeState::Stopped;
                    // Withdraw the published endpoint: a stopped runtime must not keep
                    // reporting a stale listening port/config.
                    inner.detail.ports.clear();
                    inner.detail.session_id = None;
                    inner.detail.config_sha256 = None;
                    drop(inner);
                    // No core, but a TUN lease may still be pending (stop arriving
                    // between helper-apply and core-spawn); always release. A
                    // failed release degrades the stop: desired=false with an
                    // unconfirmed lease must not read "closed".
                    let tun_result = self.release_tun_lease().await;
                    if let Err(error) = &tun_result {
                        let mut inner = self.inner.lock().await;
                        inner.detail.state = RuntimeState::Degraded;
                        inner.detail.error = Some(error.clone());
                    }
                    self.record_stop_operation(&operation_id, &tun_result).await;
                    return None;
                }
            }
        };
        let session_id = session.session_id.clone();
        let pid = session.identity.pid;
        // Terminate outside the inner lock (SP-04): snapshot/operation queries
        // only need that lock briefly, so reconciliation reads stay responsive
        // while the core tree shuts down. The command gate is still held, so
        // no other apply/stop can interleave with this stop.
        session.terminate_and_wait(Duration::from_secs(5)).await;
        drop(session.job);
        let final_stage = JournalEntry {
            session_id: session_id.clone(),
            plan_id: session.plan_id.clone(),
            desired_revision: session.desired_revision,
            config_sha256: session.config_sha256.clone(),
            stage: RecoveryStage::Finalized,
            pid: Some(pid),
            created_at_ms: Some(session.identity.created_at_ms),
            port: session.port,
            updated_at_ms: journal::now_ms(),
        };
        let _ = journal::write_entry(&self.config.run_root, &final_stage);
        journal::remove_staged_artifacts(&self.config.run_root, &session_id);
        {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::Stopped;
            inner.detail.pid = None;
            inner.detail.created_at_ms = None;
            inner.detail.error = None;
            inner.active_operation = None;
            // Withdraw the published endpoint on stop and drop the restore source:
            // a stopped session has nothing to fall back to.
            inner.detail.ports.clear();
            inner.detail.session_id = None;
            inner.detail.config_sha256 = None;
            inner.last_plan = None;
            inner.last_exe = None;
        }
        eprintln!("[net_host] session {session_id} STOPPED pid={pid}");
        // Reverse cleanup after the core tree is gone. An unconfirmed TUN
        // release degrades the stop instead of reporting a clean shutdown.
        let tun_result = self.release_tun_lease().await;
        if let Err(error) = &tun_result {
            let mut inner = self.inner.lock().await;
            inner.detail.state = RuntimeState::Degraded;
            inner.detail.error = Some(error.clone());
        }
        self.record_stop_operation(&operation_id, &tun_result).await;
        self.bus.emit_named(
            "runtime_state_changed",
            serde_json::to_value(RuntimeStateChanged {
                state: RuntimeState::Stopped,
                applied_revision: 0,
                message_key: None,
            })
            .unwrap_or(json!({})),
        );
        Some(session_id)
    }

    /// Open a restricted temporary test core, fully isolated from the managed
    /// runtime. The caller passes an already-generated config and a bounded
    /// duration; net-host never lets a test session outlive `max_duration_ms`.
    pub async fn open_test_session(
        self: &Arc<Self>,
        plan: RuntimePlan,
        max_duration_ms: u64,
    ) -> Result<String, DomainError> {
        plan.validate()?;
        let core = plan.target.core_type;
        let adapter = adapter_for(core).ok_or_else(|| {
            DomainError::new(domain::codes::NOT_FOUND, "error.core_not_supported")
                .with_detail(format!("no adapter for {}", core.as_str()))
        })?;
        let body = match &plan.target.config {
            ConfigSource::Inline { body } => body.clone(),
            ConfigSource::ControlledFile { .. } => {
                return Err(DomainError::new(
                    domain::codes::INVALID_ARGUMENT,
                    "error.test_session_unsupported",
                )
                .with_detail("controlled-file test configs are not supported"));
            }
        };
        let actual_hash = sha256_hex(body.as_bytes());
        let declared = plan.target.config_sha256.as_str();
        if !declared.is_empty() && declared != actual_hash {
            return Err(DomainError::new(
                domain::codes::INVALID_PLAN,
                "error.config_hash_mismatch",
            )
            .with_field("config_sha256")
            .with_detail(format!("declared {declared}, computed {actual_hash}")));
        }
        let port = plan
            .ports
            .iter()
            .find(|p| p.port >= 11_808)
            .map(|p| p.port)
            .ok_or_else(|| {
                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.test_port_floor")
                    .with_field("port")
                    .with_detail("test session needs a port >= 11808")
            })?;

        let session_seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
        let session_id = format!("test-{}-{}", journal::now_ms(), session_seq);
        let dir = journal::session_dir(&self.config.run_root, &session_id);
        std::fs::create_dir_all(&dir).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("create test session dir failed: {e}"))
        })?;
        if let Err(e) = crate::dacl::restrict_to_current_user(&dir, true) {
            eprintln!("[net_host] test session dir ACL not applied: {e}");
        }
        let config_path = dir.join("config.json");
        std::fs::write(&config_path, body.as_bytes()).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("write test config failed: {e}"))
        })?;
        if let Err(e) = crate::dacl::restrict_to_current_user(&config_path, false) {
            eprintln!("[net_host] test config ACL not applied: {e}");
        }

        preflight_port(port).inspect_err(|_| {
            journal::remove_staged_artifacts(&self.config.run_root, &session_id);
        })?;

        let locator = CoreLocator::from_env();
        let exe = locator
            .resolve(core, plan.target.version.as_deref())
            .inspect_err(|_| {
                journal::remove_staged_artifacts(&self.config.run_root, &session_id);
            })?;

        let job = JobGuard::create_kill_on_close().map_err(|e| {
            journal::remove_staged_artifacts(&self.config.run_root, &session_id);
            DomainError::new(domain::codes::UNAVAILABLE, "error.job_create_failed")
                .with_detail(e.to_string())
        })?;

        let mut command = Command::new(&exe);
        command
            .args(adapter.run_args(&config_path))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|e| {
            journal::remove_staged_artifacts(&self.config.run_root, &session_id);
            DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                .with_detail(format!("{}: {e}", exe.display()))
        })?;

        let pid = child.id().unwrap_or(0);
        let created_at_ms = process_creation_time_ms(pid).unwrap_or(0);
        let identity = ProcessIdentity::new(pid, created_at_ms);
        #[cfg(windows)]
        {
            let assignment = match child.raw_handle() {
                Some(handle) => job.assign(handle),
                None => Err(std::io::Error::other("core process handle unavailable")),
            };
            if let Err(e) = assignment {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                journal::remove_staged_artifacts(&self.config.run_root, &session_id);
                return Err(DomainError::new(
                    domain::codes::JOB_ASSIGN_FAILED,
                    "error.job_assign_failed",
                )
                .with_detail(e.to_string()));
            }
        }

        let log_path = dir.join("core.log");
        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(stdout, log_path.clone(), "test-stdout", self.bus.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(stderr, log_path.clone(), "test-stderr", self.bus.clone());
        }
        let _ = journal::write_entry(
            &self.config.run_root,
            &JournalEntry {
                session_id: session_id.clone(),
                plan_id: plan.plan_id.clone(),
                desired_revision: 0,
                config_sha256: actual_hash.clone(),
                stage: RecoveryStage::Applying,
                pid: Some(pid),
                created_at_ms: Some(created_at_ms),
                port,
                updated_at_ms: journal::now_ms(),
            },
        );

        let outcome = wait_ready(
            &mut child,
            port,
            self.config.readiness_timeout,
            self.config.readiness_interval,
        )
        .await;
        match outcome {
            ReadyOutcome::Ready => {}
            ReadyOutcome::Exited(code) => {
                let tail = tail_log(&log_path, 8);
                journal::remove_staged_artifacts(&self.config.run_root, &session_id);
                return Err(
                    DomainError::new(domain::codes::INTERNAL, "error.core_exited")
                        .with_detail(format!("test core exited early (code {code:?}): {tail}")),
                );
            }
            ReadyOutcome::Timeout => {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                journal::remove_staged_artifacts(&self.config.run_root, &session_id);
                return Err(
                    DomainError::new(domain::codes::TIMEOUT, "error.readiness_timeout")
                        .with_field("port")
                        .with_detail(format!("test port {port} not ready")),
                );
            }
        }

        let child = TestSessionChild {
            core,
            port,
            child,
            identity,
            _job: job,
        };
        {
            let mut inner = self.inner.lock().await;
            inner.test_sessions.insert(session_id.clone(), child);
        }
        eprintln!("[net_host] test session {session_id} RUNNING pid={pid} port={port}");

        // Safety cap: never let a test core linger past the caller's bound.
        let weak = Arc::downgrade(self);
        let watchdog_id = session_id.clone();
        let cap = max_duration_ms.clamp(1_000, 60_000);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(cap)).await;
            if let Some(state) = weak.upgrade() {
                let _ = state.close_test_session(&watchdog_id).await;
            }
        });
        Ok(session_id)
    }

    /// Close one test session (idempotent).
    pub async fn close_test_session(&self, session_id: &str) -> Result<(), DomainError> {
        let mut child = {
            let mut inner = self.inner.lock().await;
            inner.test_sessions.remove(session_id)
        };
        let Some(child) = child.as_mut() else {
            return Ok(());
        };
        child.terminate_and_wait(Duration::from_secs(5)).await;
        let pid = child.identity.pid;
        let port = child.port;
        let core = child.core;
        // Release-before-reuse: the listen socket is gone, but accepted
        // connections can linger in `TIME_WAIT` and block the next bind on
        // Windows. Confirm (bounded) before the caller may reserve it again.
        wait_port_released(port, Duration::from_millis(1500)).await;
        journal::remove_staged_artifacts(&self.config.run_root, session_id);
        eprintln!(
            "[net_host] test session {session_id} STOPPED pid={pid} core={core:?} port={port}"
        );
        Ok(())
    }

    /// Query a test session's liveness.
    pub async fn test_session_status(&self, session_id: &str) -> Option<domain::JobState> {
        let mut inner = self.inner.lock().await;
        let child = inner.test_sessions.get_mut(session_id)?;
        match child.child.try_wait() {
            Ok(None) => Some(domain::JobState::Running),
            Ok(Some(_)) => Some(domain::JobState::Done),
            Err(_) => Some(domain::JobState::Failed),
        }
    }

    /// Close every test session (shutdown / lease reclaim).
    pub async fn stop_all_test_sessions(&self) {
        let ids: Vec<String> = {
            let inner = self.inner.lock().await;
            inner.test_sessions.keys().cloned().collect()
        };
        for id in ids {
            let _ = self.close_test_session(&id).await;
        }
    }
}

enum ReadyOutcome {
    Ready,
    Exited(Option<i32>),
    Timeout,
}

/// Verdict of re-checking the actual result after a readiness timeout
/// (SP-10). A timeout never fails an actually-ready session and never masks
/// a real exit as a timeout. Staged contract: call sites re-observe first;
/// covered by SP-10 unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TimeoutConfirm {
    ActuallyReady,
    Exited(Option<i32>),
    ConfirmedTimeout,
}

/// Re-check the actual result after a readiness timeout (SP-10).
///
/// `exited` is the final `try_wait` observation (`Some(code)` when the
/// process is gone); `port_ready_now` is one last immediate probe. Pure and
/// side-effect-free; callers perform the final observation. Staged contract,
/// covered by SP-10 unit tests.
#[allow(dead_code)]
pub fn confirm_readiness_timeout(
    exited: Option<Option<i32>>,
    port_ready_now: bool,
) -> TimeoutConfirm {
    if let Some(code) = exited {
        return TimeoutConfirm::Exited(code);
    }
    if port_ready_now {
        return TimeoutConfirm::ActuallyReady;
    }
    TimeoutConfirm::ConfirmedTimeout
}

/// Readiness verdict for an elevated-sidecar launch (SP-10, TUN-A05).
///
/// The helper's `RunElevatedCore` success only dispatches; readiness needs a
/// helper-owned process identity. A zero handle on the real path is not
/// ready. Dry-run links report `(0, 0)` by design and stay simulated.
pub fn elevated_launch_verdict(handle: u64, dry_run: bool) -> Result<(), DomainError> {
    if handle == 0 && !dry_run {
        return Err(
            DomainError::new(domain::codes::UNAVAILABLE, "error.tun_helper_denied")
                .with_detail("helper launch returned no elevated process identity"),
        );
    }
    Ok(())
}

async fn wait_ready(
    child: &mut tokio::process::Child,
    port: u16,
    timeout: Duration,
    interval: Duration,
) -> ReadyOutcome {
    use std::net::{Ipv4Addr, SocketAddr};

    let deadline = Instant::now() + timeout;
    // Let an immediate bind failure surface before trusting a connect.
    tokio::time::sleep(interval.min(Duration::from_millis(500))).await;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return ReadyOutcome::Exited(status.code());
        }
        if Instant::now() >= deadline {
            // SP-10: the deadline fired, but the outcome is unknown until the
            // actual result is re-checked: an exit at the edge reports
            // Exited, and one last immediate probe accepts an
            // actually-ready core instead of failing it on a timeout.
            if let Ok(Some(status)) = child.try_wait() {
                return ReadyOutcome::Exited(status.code());
            }
            if port != 0 && TcpStream::connect(addr).await.is_ok() {
                if let Ok(Some(status)) = child.try_wait() {
                    return ReadyOutcome::Exited(status.code());
                }
                return ReadyOutcome::Ready;
            }
            return ReadyOutcome::Timeout;
        }
        if port != 0 && TcpStream::connect(addr).await.is_ok() {
            if let Ok(Some(status)) = child.try_wait() {
                return ReadyOutcome::Exited(status.code());
            }
            return ReadyOutcome::Ready;
        }
        if port == 0 {
            // No port to probe: treat a surviving process as ready.
            tokio::time::sleep(interval).await;
            if let Ok(Some(status)) = child.try_wait() {
                return ReadyOutcome::Exited(status.code());
            }
            return ReadyOutcome::Ready;
        }
        tokio::select! {
            _ = tokio::time::sleep(interval) => {}
            status = child.wait() => {
                return ReadyOutcome::Exited(status.ok().and_then(|s| s.code()));
            }
        }
    }
}

/// Wait until `port` answers a SOCKS5 greeting (`VER=5`, no auth), mirroring
/// upstream `CoreManager.WaitForProxyPort`. Best-effort and bounded: the caller
/// decides whether a timeout is fatal. Reuses the same async runtime as the
/// managed session.
async fn wait_socks_port(port: u16, timeout: Duration, interval: Duration) -> bool {
    use std::net::{Ipv4Addr, SocketAddr};
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(mut stream) = TcpStream::connect(addr).await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            if stream.write_all(&[0x05, 0x01, 0x00]).await.is_ok() {
                let mut buf = [0u8; 2];
                if let Ok(read) =
                    tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf)).await
                {
                    if matches!(read, Ok(n) if n == 2 && buf[0] == 0x05) {
                        return true;
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(interval.min(Duration::from_millis(200))).await;
    }
}

fn preflight_port(port: u16) -> Result<(), DomainError> {
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(e) => Err(
            DomainError::new(domain::codes::PORT_CONFLICT, "error.port_conflict")
                .with_field("port")
                .with_detail(format!("127.0.0.1:{port} unavailable: {e}")),
        ),
    }
}

/// Best-effort bounded wait for a test port to become bindable again, so a
/// stopped test session does not leave a `TIME_WAIT` port behind for reuse.
async fn wait_port_released(port: u16, timeout: Duration) {
    if port == 0 {
        return;
    }
    let deadline = Instant::now() + timeout;
    loop {
        if preflight_port(port).is_ok() {
            return;
        }
        if Instant::now() >= deadline {
            eprintln!("[net_host] test port {port} still not bindable after release");
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn spawn_log_reader<S>(stream: S, path: PathBuf, label: &'static str, bus: EventBus)
where
    S: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(stream).lines();
        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(file) => file,
            Err(_) => return,
        };
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = file
                .write_all(format!("[{label}] {line}\n").as_bytes())
                .await;
            let _ = file.flush().await;
            // T15a: forward the raw line as a telemetry event. Never includes
            // credentials beyond what the core itself printed.
            bus.emit_named(
                "log_line",
                serde_json::json!({ "text": line, "stream": label }),
            );
        }
    });
}

fn tail_log(path: &std::path::Path, max_lines: usize) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return "<no log>".to_string();
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join(" | ")
}

/// Verify a recorded identity is still alive.
#[allow(dead_code)]
pub fn identity_alive(identity: &ProcessIdentity) -> bool {
    matches_identity(identity)
}

/// Redacted TUN facts for [`RuntimeDetail`]: adapter label, interface index,
/// route count and the dry-run flag only. Never addresses, next hops or tokens.
fn tun_detail_from_lease(lease: &TunLease) -> RuntimeTunDetail {
    RuntimeTunDetail {
        adapter_name: lease.adapter_name.clone(),
        interface_index: lease.interface_index,
        route_count: lease.route_count,
        dry_run: lease.dry_run,
    }
}

/// Pure lease-reclaim decision (unit-testable without a process or runtime).
///
/// Client liveness on a local named pipe is the connection itself: a killed or
/// crashed GUI closes its handle and the OS reports it. The lease is reclaimed
/// only when no client is connected for longer than `grace`. Server->client
/// liveness (is net-host still alive?) is carried by the watchdog heartbeat.
fn reclaim_due(
    has_session: bool,
    active_connections: usize,
    no_client_since: Option<Instant>,
    grace: Duration,
) -> bool {
    if !has_session || active_connections > 0 {
        return false;
    }
    no_client_since
        .map(|since| since.elapsed() >= grace)
        .unwrap_or(false)
}

/// Minimum idle time before the host terminates itself. Longer than the lease
/// reclaim grace so a quick reopen within the NSI same-version window still
/// reuses the process; short enough that a real exit leaves no residual
/// service.
const IDLE_EXIT_MIN: Duration = Duration::from_secs(15);

/// Pure idle-exit decision (unit-testable without a process or runtime).
///
/// The host terminates itself only after every client is gone for at least
/// `max(grace, IDLE_EXIT_MIN)` and no managed session, TUN lease or test
/// session remains. A running core keeps it alive regardless of the client.
fn idle_exit_due(
    has_session: bool,
    has_tun_lease: bool,
    test_sessions: usize,
    active_connections: usize,
    no_client_since: Option<Instant>,
    grace: Duration,
) -> bool {
    if has_session || has_tun_lease || test_sessions > 0 || active_connections > 0 {
        return false;
    }
    let threshold = if grace > IDLE_EXIT_MIN {
        grace
    } else {
        IDLE_EXIT_MIN
    };
    no_client_since
        .map(|since| since.elapsed() >= threshold)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(has_session: bool, active: usize, no_client_since: Option<Instant>) -> bool {
        reclaim_due(
            has_session,
            active,
            no_client_since,
            Duration::from_millis(6000),
        )
    }

    #[test]
    fn no_session_never_reclaims() {
        assert!(!base(false, 0, None));
    }

    #[test]
    fn connected_client_never_reclaims() {
        let long_ago = Instant::now() - Duration::from_millis(7000);
        // An idle but connected client keeps the lease.
        assert!(!base(true, 1, Some(long_ago)));
    }

    #[test]
    fn disconnected_client_reclaims_only_after_grace() {
        assert!(!base(true, 0, Some(Instant::now())));
        let long_ago = Instant::now() - Duration::from_millis(7000);
        assert!(base(true, 0, Some(long_ago)));
        assert!(!base(true, 0, None));
    }

    fn idle(
        has_session: bool,
        has_tun_lease: bool,
        test_sessions: usize,
        active: usize,
        no_client_since: Option<Instant>,
    ) -> bool {
        idle_exit_due(
            has_session,
            has_tun_lease,
            test_sessions,
            active,
            no_client_since,
            Duration::from_millis(6000),
        )
    }

    #[test]
    fn idle_exit_requires_no_client_and_no_work_after_min() {
        let min_ago = Instant::now() - IDLE_EXIT_MIN - Duration::from_secs(1);
        let short_ago = Instant::now() - Duration::from_secs(7);
        // No client, no work, idle long enough -> exit.
        assert!(idle(false, false, 0, 0, Some(min_ago)));
        // Only just past the reclaim grace, not the idle minimum yet.
        assert!(!idle(false, false, 0, 0, Some(short_ago)));
        // A connected client or outstanding work always keeps it alive.
        assert!(!idle(false, false, 0, 1, Some(min_ago)));
        assert!(!idle(true, false, 0, 0, Some(min_ago)));
        assert!(!idle(false, true, 0, 0, Some(min_ago)));
        assert!(!idle(false, false, 1, 0, Some(min_ago)));
        // Never had a client -> stay up.
        assert!(!idle(false, false, 0, 0, None));
    }

    #[test]
    fn job_assign_failure_is_structured_and_non_retryable() {
        let error = job_assign_failed("op-1", "Access is denied.");
        assert_eq!(error.code, domain::codes::JOB_ASSIGN_FAILED);
        assert_eq!(error.message_key, "error.job_assign_failed");
        assert_eq!(error.operation_id.as_deref(), Some("op-1"));
        assert_eq!(error.detail.as_deref(), Some("Access is denied."));
        assert!(!error.retryable);
    }

    #[test]
    fn tun_detail_from_lease_is_redacted() {
        use runtime::tun::{TunAddress, TunRoute, TunSpec, TUN_CONFIG_KIND};

        let lease = TunLease::new(
            "helper-session-1",
            TunSpec {
                kind: TUN_CONFIG_KIND.into(),
                adapter_name: "v2rayn-tun".into(),
                interface_index: 9,
                addresses: vec![TunAddress {
                    address: "198.18.0.1".into(),
                    prefix_len: 16,
                }],
                mtu: Some(1400),
                routes: vec![TunRoute {
                    destination: "0.0.0.0/0".into(),
                    next_hop: "198.18.0.1".into(),
                    interface_index: 9,
                    metric: 1,
                }],
                route_exclude: vec![],
            },
            false,
        );
        let detail = tun_detail_from_lease(&lease);
        assert_eq!(detail.adapter_name, "v2rayn-tun");
        assert_eq!(detail.interface_index, 9);
        assert_eq!(detail.route_count, 1);
        assert!(!detail.dry_run);
    }

    #[test]
    fn helper_dry_run_flag_flows_from_env() {
        let key = "V2RAYN_R_DRY_RUN_TUN";
        let previous = std::env::var_os(key);
        std::env::set_var(key, "1");
        let config = HostConfig::from_env();
        assert!(config.helper.dry_run);
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        assert!(!HostConfig::from_env().helper.dry_run);
    }

    #[test]
    fn tun_inbound_detection_is_structural() {
        assert!(body_has_tun_inbound(
            r#"{"inbounds":[{"type":"socks"},{"type":"tun","interface_name":"v2rayn-tun"}]}"#
        ));
        assert!(!body_has_tun_inbound(r#"{"inbounds":[{"type":"socks"}]}"#));
        assert!(!body_has_tun_inbound("not json"));
        assert!(!body_has_tun_inbound(r#"{"inbounds":[]}"#));
    }

    #[test]
    fn elevated_core_ids_match_the_helper_allowlist() {
        for (core, expected) in [
            (CoreType::Xray, "xray"),
            (CoreType::SingBox, "sing-box"),
            (CoreType::Mihomo, "mihomo"),
            (CoreType::V2fly, "v2fly"),
        ] {
            let id = elevated_core_id(core).expect("mapped");
            assert_eq!(id, expected);
            assert!(ipc_contract::is_allowed_core_name(id));
        }
        assert!(elevated_core_id(CoreType::Hysteria2).is_none());
    }

    #[test]
    fn journal_entry_carries_pid_and_creation_time() {
        use runtime::current_identity;

        let identity = current_identity();
        let entry = JournalEntry {
            session_id: "s-tun".into(),
            plan_id: "p".into(),
            desired_revision: 1,
            config_sha256: "ab".into(),
            stage: ipc_contract::RecoveryStage::Applied,
            pid: Some(identity.pid),
            created_at_ms: Some(identity.created_at_ms),
            port: 11808,
            updated_at_ms: 0,
        };
        // The journaled identity round-trips and still matches the live
        // process: recovery may only act on this exact (pid, created_at).
        assert_eq!(entry.identity(), Some(identity));
        assert!(runtime::matches_identity(&identity));
    }

    #[test]
    fn bogus_identity_never_matches() {
        let bogus = runtime::ProcessIdentity::new(0xFFFF_FFF0, 1);
        assert!(!runtime::matches_identity(&bogus));
    }

    #[test]
    fn helper_factory_seam_replaces_link_construction() {
        use crate::helper_client::{DryRunHelperLink, FakeHelperLink, HelperLink};
        use std::sync::Arc;

        let root = std::env::temp_dir().join(format!(
            "v2rayn-t14-factory-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let config = HostConfig {
            pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
            run_root: root.clone(),
            disconnect_grace: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(1000),
            readiness_timeout: Duration::from_millis(100),
            readiness_interval: Duration::from_millis(10),
            helper: HelperConfig {
                pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
                token: String::new(),
                bin: None,
                auto_launch: false,
                allowed_run_roots: Vec::new(),
                dry_run: false,
            },
        };
        // No stale leases: construction performs no helper I/O.
        let state = HostState::new(config);
        // Without a factory the config selects the dry-run/pipe transport.
        state.set_helper_factory(Arc::new(|_| {
            let link: Box<dyn HelperLink> = Box::new(FakeHelperLink::new());
            link
        }));
        let link = state.make_helper_link();
        assert_eq!(link.session_id(), "helper-session-1");
        assert!(!link.dry_run());
        // The dry-run transport is selected purely from configuration.
        let dry: Box<dyn HelperLink> = Box::new(DryRunHelperLink::new());
        assert!(dry.dry_run());
        let _ = std::fs::remove_dir_all(&root);
    }

    // -- SP-08 host-side pending cleanup (CP-04) ------------------------------
    //
    // Synthetic fault injection only (in-memory links, temp run roots, no
    // helper pipe, no OS routes/DNS/adapter writes, no ports).

    /// HostConfig with an explicit run root and a helper transport that never
    /// touches the OS: `dry_run=true` records only, `dry_run=false` with no
    /// auto-launch reports the helper unreachable without dialing any pipe.
    fn sp08_config(root: PathBuf, dry_run: bool) -> HostConfig {
        HostConfig {
            pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
            run_root: root,
            disconnect_grace: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(1000),
            readiness_timeout: Duration::from_millis(100),
            readiness_interval: Duration::from_millis(10),
            helper: HelperConfig {
                pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
                token: String::new(),
                bin: None,
                auto_launch: false,
                allowed_run_roots: Vec::new(),
                dry_run,
            },
        }
    }

    fn sp08_tun_spec() -> runtime::tun::TunSpec {
        use runtime::tun::{TunAddress, TunRoute, TUN_CONFIG_KIND};
        runtime::tun::TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            addresses: vec![TunAddress {
                address: "198.18.0.1".into(),
                prefix_len: 16,
            }],
            mtu: Some(1400),
            routes: vec![TunRoute {
                destination: "0.0.0.0/0".into(),
                next_hop: "198.18.0.1".into(),
                interface_index: 9,
                metric: 1,
            }],
            route_exclude: vec![],
        }
    }

    /// In-memory link with a flippable cleanup fault: apply always succeeds
    /// (journaling proceeds), cleanup fails while `fail` is set.
    #[derive(Clone)]
    struct Sp08FlakyCleanupLink {
        fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl HelperLink for Sp08FlakyCleanupLink {
        fn session_id(&self) -> String {
            "helper-session-sp08".into()
        }

        fn dry_run(&self) -> bool {
            false
        }

        fn available(&mut self) -> Result<(), DomainError> {
            Ok(())
        }

        fn apply(&mut self, spec: &runtime::tun::TunSpec) -> Result<TunLease, DomainError> {
            Ok(TunLease::new("helper-session-sp08", spec.clone(), false))
        }

        fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(crate::helper_client::tun_apply_failed(
                    "synthetic sp08 cleanup failure",
                ));
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn sp08_stop_with_failed_cleanup_degrades_and_stays_retryable() {
        // No rr10_lock: this test never touches process-global env (explicit
        // HostConfig, temp run root, in-memory links), so it cannot interleave
        // with the env-mutating RR-10 harness; holding that std lock across an
        // await would also trip `await_holding_lock`.
        let root = std::env::temp_dir().join(format!(
            "v2rayn-sp08-stop-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = std::sync::Arc::new(HostState::new(sp08_config(root.clone(), false)));
        let fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let fail = fail.clone();
            state.set_helper_factory(std::sync::Arc::new(move |_| {
                let link: Box<dyn HelperLink> =
                    Box::new(Sp08FlakyCleanupLink { fail: fail.clone() });
                link
            }));
        }
        // Seed an applied lease (no core): the journal is the owned record.
        let lease = {
            let mut link = state.make_helper_link();
            crate::tun_lease::apply_tun_lease(&root, "sess-sp08", &sp08_tun_spec(), &mut *link)
                .expect("apply succeeds")
        };
        {
            let mut inner = state.inner.lock().await;
            inner.tun_session_id = Some("sess-sp08".into());
            inner.tun_lease = Some(lease);
            inner.tun_link = Some(state.make_helper_link());
        }

        state.stop_managed(Some("op-sp08-stop".into())).await;

        // The stop must not read "closed": the lease is unconfirmed.
        let snapshot = state.ipc_snapshot().await;
        assert_eq!(
            snapshot.state,
            RuntimeState::Degraded,
            "a stop with an unconfirmed TUN cleanup must degrade, not close"
        );
        let status = state
            .operation_status("op-sp08-stop")
            .await
            .expect("stop is recorded");
        assert_eq!(status.state, domain::JobState::Failed);
        let pending = state.pending_cleanup_snapshot().await;
        assert_eq!(pending.len(), 1, "the failed cleanup stays retryable");
        assert_eq!(pending[0].session_id, "sess-sp08");

        // The helper recovers: the explicit retry confirms the release and
        // every record converges.
        fail.store(false, std::sync::atomic::Ordering::SeqCst);
        state.retry_tun_cleanup().await.expect("retry converges");
        assert!(state.pending_cleanup_snapshot().await.is_empty());
        assert_eq!(
            state.ipc_snapshot().await.state,
            RuntimeState::Stopped,
            "a confirmed retry restores the closed state"
        );
        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn sp08_restart_keeps_unconfirmed_lease_pending() {
        use crate::helper_client::FakeHelperLink;

        // Env-independent like the test above: no rr10_lock across awaits.
        let root = std::env::temp_dir().join(format!(
            "v2rayn-sp08-restart-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        // Previous process: applied a lease (dry-run record) and died without
        // cleaning. The dry-run link never touches the helper or the OS.
        {
            let previous = HostState::new(sp08_config(root.clone(), true));
            let mut link = previous.make_helper_link();
            crate::tun_lease::apply_tun_lease(&root, "sess-prev", &sp08_tun_spec(), &mut *link)
                .expect("apply succeeds");
        }
        // New process, helper unreachable: boot reconciliation must keep the
        // journal and load the lease as pending — never report it cleaned.
        let state = std::sync::Arc::new(HostState::new(sp08_config(root.clone(), false)));
        let pending = state.pending_cleanup_snapshot().await;
        assert_eq!(pending.len(), 1, "the previous lease stays pending");
        assert_eq!(pending[0].session_id, "sess-prev");
        // A live helper finishes it on explicit retry.
        state.set_helper_factory(std::sync::Arc::new(|_| {
            let link: Box<dyn HelperLink> = Box::new(FakeHelperLink::new());
            link
        }));
        state.retry_tun_cleanup().await.expect("retry converges");
        assert!(state.pending_cleanup_snapshot().await.is_empty());
        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }

    // -- RR-10 precheck / restore ------------------------------------------

    /// The RR-10 tests mutate process-global env (`V2RAYN_R_XRAY_BIN`) and
    /// must not interleave when the harness runs tests in parallel.
    fn rr10_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A HostState whose run root is a fresh temp dir and whose core "exe" is
    /// `cmd.exe` via the xray override, so no real core is needed.
    fn test_state(tag: &str) -> HostState {
        let root = std::env::temp_dir().join(format!(
            "v2rayn-rr10-{tag}-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("V2RAYN_R_XRAY_BIN", "C:\\Windows\\System32\\cmd.exe");
        std::env::set_var("V2RAYN_R_SKIP_CONFIG_CHECK", "1");
        // The env binary override is dev-only (plan §3.7); the stub-core
        // harness is exactly that.
        std::env::set_var("V2RAYN_R_DEV_MODE", "1");
        let config = HostConfig {
            pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
            run_root: root,
            disconnect_grace: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(1000),
            readiness_timeout: Duration::from_millis(400),
            readiness_interval: Duration::from_millis(20),
            helper: HelperConfig {
                pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
                token: String::new(),
                bin: None,
                auto_launch: false,
                allowed_run_roots: Vec::new(),
                dry_run: true,
            },
        };
        HostState::new(config)
    }

    fn plan_with_body(id: &str, body: &str, port: u16, hash_override: Option<&str>) -> RuntimePlan {
        let actual = sha256_hex(body.as_bytes());
        RuntimePlan {
            plan_id: format!("plan-{id}"),
            desired_revision: 1,
            target: domain::runtime_plan::RuntimeTarget {
                core_type: domain::CoreType::Xray,
                version: None,
                config: ConfigSource::Inline {
                    body: body.to_string(),
                },
                config_sha256: domain::runtime_plan::ContentHash::new(
                    hash_override.map(str::to_string).unwrap_or(actual),
                ),
            },
            process_graph: Default::default(),
            outbound_graph: Default::default(),
            ports: vec![domain::runtime_plan::PortRequest::tcp(port, "inbound")],
            privileges: vec![domain::runtime_plan::RequiredPrivilege::None],
            network_policy: Default::default(),
            resources: vec![],
        }
    }

    #[test]
    fn rr10_precheck_rejects_bad_hash_without_stopping() {
        let _guard = rr10_lock();
        // Wrong declared hash fails precheck before the running session stops.
        let state = test_state("hash");
        let plan = plan_with_body(
            "p",
            "{\"inbounds\":[],\"outbounds\":[]}",
            11_908,
            Some("00"),
        );
        let error = match futures_block_on(state.precheck_plan(&plan)) {
            Ok(_) => panic!("hash mismatch must fail precheck"),
            Err(error) => error,
        };
        assert_eq!(error.code, domain::codes::INVALID_PLAN);
        assert_eq!(error.message_key, "error.config_hash_mismatch");
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn rr10_precheck_rejects_unsupported_core_without_stopping() {
        let _guard = rr10_lock();
        let state = test_state("unsupported");
        let mut plan = plan_with_body("p", "{}", 11_909, None);
        // `App` is the v2rayN self-update identity: the only core without a
        // proxy adapter (all `PROXY_CORES` now have one, RR-06).
        plan.target.core_type = domain::CoreType::App;
        let error = match futures_block_on(state.precheck_plan(&plan)) {
            Ok(_) => panic!("no adapter for App"),
            Err(error) => error,
        };
        assert_eq!(error.code, domain::codes::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn rr10_precheck_rejects_busy_port_owned_by_other() {
        let _guard = rr10_lock();
        let state = test_state("busy");
        // Occupy a real port with a listener we own (>= 11808).
        let listener = std::net::TcpListener::bind(("127.0.0.1", 11_907)).unwrap();
        let plan = plan_with_body("p", "{}", 11_907, None);
        let error = match futures_block_on(state.precheck_plan(&plan)) {
            Ok(_) => panic!("busy port must fail precheck"),
            Err(error) => error,
        };
        assert_eq!(error.code, domain::codes::PORT_CONFLICT);
        drop(listener);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    /// Minimal current-thread block_on so the sync `#[test]`s can await the
    /// async precheck without pulling a runtime into every test.
    fn futures_block_on<F: std::future::Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }

    /// D28: a hanging precheck child is terminated by the deadline instead of
    /// keeping the config check (and the UI) blocked past the client timeout.
    #[test]
    fn config_check_deadline_terminates_a_hung_child() {
        #[cfg(windows)]
        {
            let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:/Windows".to_string());
            let ping = PathBuf::from(root).join("System32").join("ping.exe");
            if !ping.is_file() {
                return;
            }
            let args = vec![
                std::ffi::OsString::from("-n"),
                std::ffi::OsString::from("30"),
                std::ffi::OsString::from("127.0.0.1"),
            ];
            let start = Instant::now();
            let result =
                futures_block_on(run_bounded_output(&ping, &args, Duration::from_millis(300)));
            assert!(matches!(result, Err(RunBoundedError::Timeout)));
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "the deadline must terminate the managed child promptly"
            );
        }
    }

    /// Write a `.cmd` core stub: `"stay"` pings for a long time (stays alive,
    /// no port -> Ready by liveness); `"exit"` exits immediately (post-stop
    /// readiness failure).
    fn core_stub(dir: &std::path::Path, name: &str, mode: &str) -> PathBuf {
        let path = dir.join(format!("{name}.cmd"));
        let body = if mode == "stay" {
            "@echo off\r\nping -n 5 127.0.0.1 >nul\r\n"
        } else {
            "@echo off\r\nexit /b 1\r\n"
        };
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn rr10_failed_switch_restores_previous_session() {
        let _guard = rr10_lock();
        let state = test_state("restore");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        let exit = core_stub(&dir, "exit", "exit");

        // Start the good session (no port -> readiness by process liveness).
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let good = plan_with_body("good", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        futures_block_on(state.apply_plan(good)).expect("good session starts");

        // Switch to a plan whose core exits immediately: precheck passes (stub
        // skipped config check) but readiness fails after the old stop.
        std::env::set_var("V2RAYN_R_XRAY_BIN", &exit);
        let bad = plan_with_body("bad", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        let error = futures_block_on(state.apply_plan(bad)).expect_err("switch must fail");
        assert_eq!(error.code, domain::codes::INTERNAL, "core exited early");

        // RR-10: the previous good session is restored, so a listener owned by
        // this project is running again.
        let inner = futures_block_on(state.inner.lock());
        assert!(
            inner.session.is_some(),
            "previous good session must be restored"
        );
        assert_eq!(
            inner.detail.state,
            RuntimeState::Running,
            "restored session is Running"
        );
        drop(inner);

        // Cleanup: stop the restored session.
        futures_block_on(state.stop_managed(None));
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    // -- SP-04 authoritative command sequence ----------------------------

    #[tokio::test]
    async fn sp04_stop_waits_for_the_command_gate() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let state = {
            let _guard = rr10_lock();
            std::sync::Arc::new(test_state("sp04-gate-stop"))
        };
        // Occupy the single command sequence from the test body: a stop
        // admitted now must wait behind the in-flight command instead of
        // interleaving with it.
        let gate = state.command_gate().lock().await;
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let probe = {
            let state = state.clone();
            let finished = finished.clone();
            tokio::spawn(async move {
                let _ = state.stop_managed(None).await;
                finished.store(true, Ordering::SeqCst);
            })
        };
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !finished.load(Ordering::SeqCst),
            "stop must wait for the in-flight command, never interleave"
        );
        drop(gate);
        tokio::time::timeout(Duration::from_secs(10), probe)
            .await
            .expect("stop completes once the gate releases")
            .expect("stop task panicked");
        assert!(finished.load(Ordering::SeqCst));
        let snapshot = state.ipc_snapshot().await;
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[tokio::test]
    async fn sp04_failed_apply_releases_the_command_gate() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let state = {
            let _guard = rr10_lock();
            std::sync::Arc::new(test_state("sp04-gate-apply"))
        };
        // A hash mismatch fails precheck (no listener needed: the hash check
        // runs before any port probe) after joining the command sequence.
        let bad = plan_with_body("bad", "{}", 11_911, Some("00"));
        let gate = state.command_gate().lock().await;
        let finished = std::sync::Arc::new(AtomicBool::new(false));
        let probe = {
            let state = state.clone();
            let finished = finished.clone();
            tokio::spawn(async move {
                let error = state
                    .apply_plan(bad)
                    .await
                    .expect_err("hash mismatch must fail");
                assert_eq!(error.code, domain::codes::INVALID_PLAN);
                finished.store(true, Ordering::SeqCst);
            })
        };
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !finished.load(Ordering::SeqCst),
            "apply must wait for the in-flight command, never interleave"
        );
        drop(gate);
        tokio::time::timeout(Duration::from_secs(10), probe)
            .await
            .expect("failed apply completes once the gate releases")
            .expect("apply task panicked");
        assert!(finished.load(Ordering::SeqCst));
        // The gate is usable again: a stop admitted after the failure runs.
        let stopped = state.stop_managed(None).await;
        assert!(stopped.is_none(), "nothing was running");
        let snapshot = state.ipc_snapshot().await;
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn sp04_snapshot_stays_responsive_during_stop() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _guard = rr10_lock();
        let state = std::sync::Arc::new(test_state("sp04-snap"));
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        // Port 0: readiness by process liveness, no listener is bound.
        let good = plan_with_body("good", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        futures_block_on(state.apply_plan(good)).expect("good session starts");

        // Queries only need the inner lock briefly: every read below must
        // complete while the stop shuts the core tree down.
        let reads = std::sync::Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            scope.spawn(|| {
                futures_block_on(state.stop_managed(None));
            });
            for _ in 0..20 {
                let snapshot = futures_block_on(state.ipc_snapshot());
                let _ = snapshot.state;
                reads.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        assert_eq!(reads.load(Ordering::SeqCst), 20);
        let snapshot = futures_block_on(state.ipc_snapshot());
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        assert!(futures_block_on(state.inner.lock()).session.is_none());
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    // -- SP-05 frozen target history and stop operation record -----------

    #[tokio::test]
    async fn sp05_stop_records_terminal_operation_and_keeps_apply_history() {
        let state = {
            let _guard = rr10_lock();
            std::sync::Arc::new(test_state("sp05-stop-op"))
        };
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        // Stub core, no listener port (port 0 -> readiness by liveness, no
        // socket, no OS port): the subject is operation history, not I/O.
        let stay = core_stub(&dir, "stay", "stay");
        {
            let _guard = rr10_lock();
            std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        }
        let plan = plan_with_body("sp05", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        let op = state.apply_plan(plan).await.expect("stub session starts");
        let status = state
            .operation_status(&op)
            .await
            .expect("accepted apply is queryable");
        assert_eq!(status.state, domain::JobState::Done);

        // A stop with an operation id withdraws the live endpoint but keeps
        // the apply history: neither entry may be rewritten or dropped.
        let stopped = state.stop_managed(Some("stop-sp05-1".to_string())).await;
        assert!(stopped.is_some(), "a session was running");
        let snapshot = state.ipc_snapshot().await;
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        {
            let inner = state.inner.lock().await;
            assert!(inner.session.is_none(), "live session withdrawn");
            assert!(inner.detail.session_id.is_none(), "no stale session fact");
            assert!(inner.detail.ports.is_empty(), "no stale endpoint");
            assert!(
                inner.operations.contains_key(&op),
                "apply history survives the stop"
            );
            assert_eq!(
                inner.operations.get(&op).map(|s| s.state),
                Some(domain::JobState::Done)
            );
        }
        let stop_status = state
            .operation_status("stop-sp05-1")
            .await
            .expect("stop operation id reconciles instead of not-found");
        assert_eq!(stop_status.state, domain::JobState::Done);
        assert!(state.operation_status("stop-sp05-unknown").await.is_none());
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[tokio::test]
    async fn sp05_idle_stop_is_idempotent_and_still_records() {
        let state = {
            let _guard = rr10_lock();
            std::sync::Arc::new(test_state("sp05-stop-idle"))
        };
        let stopped = state.stop_managed(Some("stop-sp05-idle".to_string())).await;
        assert!(stopped.is_none(), "nothing was running");
        let snapshot = state.ipc_snapshot().await;
        assert_eq!(snapshot.state, RuntimeState::Stopped);
        let status = state
            .operation_status("stop-sp05-idle")
            .await
            .expect("idle stop still records its terminal entry");
        assert_eq!(status.state, domain::JobState::Done);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    // -- RR-06 multi-process graph -----------------------------------------

    #[test]
    fn rr06_wait_socks_port_accepts_a_socks5_greeting() {
        let _guard = rr10_lock();
        // A synthetic SOCKS5 server on a test port (>= 11808).
        let listener = std::net::TcpListener::bind(("127.0.0.1", 11_906)).unwrap();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut greeting = [0u8; 3];
                let _ = stream.read_exact(&mut greeting);
                let _ = stream.write_all(&[0x05, 0x00]);
            }
        });
        let ready = futures_block_on(wait_socks_port(
            11_906,
            Duration::from_secs(2),
            Duration::from_millis(20),
        ));
        assert!(ready, "SOCKS5 server answers the greeting");
        let _ = handle.join();
    }

    #[test]
    fn rr06_wait_socks_port_times_out_without_a_server() {
        let _guard = rr10_lock();
        // No listener: the wait is bounded and returns false, never hangs.
        let ready = futures_block_on(wait_socks_port(
            11_905,
            Duration::from_millis(150),
            Duration::from_millis(20),
        ));
        assert!(!ready);
    }

    #[test]
    fn rr06_prepare_sidecars_requires_an_adapter_for_each_node() {
        let _guard = rr10_lock();
        let state = test_state("sidecar-precheck");
        let mut plan = plan_with_body("p", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        plan.process_graph
            .add_process(domain::runtime_plan::ProcessNode {
                id: "Xray".into(),
                core_type: domain::CoreType::Xray,
                config: ConfigSource::Inline {
                    body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                },
                ports: vec![],
                privileges: vec![],
            });
        plan.process_graph
            .add_process(domain::runtime_plan::ProcessNode {
                id: "sidecar".into(),
                core_type: domain::CoreType::App,
                config: ConfigSource::Inline { body: "{}".into() },
                ports: vec![],
                privileges: vec![],
            });
        plan.process_graph.depends_on("sidecar", "Xray");
        let error = match futures_block_on(state.precheck_plan(&plan)) {
            Ok(_) => panic!("sidecar without an adapter must fail precheck"),
            Err(error) => error,
        };
        assert_eq!(error.code, domain::codes::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn rr06_prepare_sidecars_builds_a_pre_socks_node() {
        let _guard = rr10_lock();
        let state = test_state("sidecar-ok");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        // Same stub executable for both the main core and the sidecar.
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let mut plan = plan_with_body("p", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        plan.process_graph
            .add_process(domain::runtime_plan::ProcessNode {
                id: "Xray".into(),
                core_type: domain::CoreType::Xray,
                config: ConfigSource::Inline {
                    body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                },
                ports: vec![],
                privileges: vec![],
            });
        plan.process_graph
            .add_process(domain::runtime_plan::ProcessNode {
                id: "pre-socks".into(),
                core_type: domain::CoreType::Xray,
                config: ConfigSource::Inline {
                    body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                },
                ports: vec![],
                privileges: vec![],
            });
        plan.process_graph.depends_on("pre-socks", "Xray");
        let prepared = futures_block_on(state.precheck_plan(&plan)).expect("sidecar prechecks");
        assert_eq!(prepared.sidecars.len(), 1);
        assert_eq!(prepared.sidecars[0].id, "pre-socks");
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn rr06_apply_tracks_sidecar_after_the_core_is_ready() {
        let _guard = rr10_lock();
        let state = test_state("sidecar-run");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let mut plan = plan_with_body("p", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        for id in ["Xray", "pre-socks"] {
            plan.process_graph
                .add_process(domain::runtime_plan::ProcessNode {
                    id: id.into(),
                    core_type: domain::CoreType::Xray,
                    config: ConfigSource::Inline {
                        body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                    },
                    ports: vec![],
                    privileges: vec![],
                });
        }
        // R3-02 frozen order (`CoreManager.LoadCore`): the main core starts
        // first, then its proxy port must be ready, then the pre-service
        // sidecar. The `pre-socks` node therefore depends on the core.
        plan.process_graph.depends_on("pre-socks", "Xray");

        futures_block_on(state.apply_plan(plan)).expect("multi-process session starts");
        {
            let inner = futures_block_on(state.inner.lock());
            let session = inner.session.as_ref().expect("running session");
            assert_eq!(session.sidecars.len(), 1, "one sidecar tracked");
            assert_eq!(inner.detail.state, RuntimeState::Running);
        }

        futures_block_on(state.stop_managed(None));
        let inner = futures_block_on(state.inner.lock());
        assert!(inner.session.is_none(), "session stopped");
        assert_eq!(inner.detail.state, RuntimeState::Stopped);
        drop(inner);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn rr10_failed_switch_without_restorable_previous_reports_stopped() {
        let _guard = rr10_lock();
        let state = test_state("cant-restore");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        let exit = core_stub(&dir, "exit", "exit");

        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let good = plan_with_body("good", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        futures_block_on(state.apply_plan(good)).expect("good session starts");

        // Remove the stub so the restore attempt itself fails to locate a core.
        std::fs::remove_file(&stay).unwrap();
        std::env::set_var("V2RAYN_R_XRAY_BIN", &exit);
        let bad = plan_with_body("bad", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        let _ = futures_block_on(state.apply_plan(bad));

        let inner = futures_block_on(state.inner.lock());
        // Cannot restore -> truthful Stopped, no fake endpoint.
        assert!(
            inner.session.is_none(),
            "unrestorable previous must not report a session"
        );
        assert_eq!(inner.detail.state, RuntimeState::Stopped);
        assert!(inner.detail.ports.is_empty());
        drop(inner);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    // -- R3-05: unified failure cleanup after a successful helper apply ------

    /// A TUN plan whose sidecar never becomes ready. The helper apply succeeds
    /// (recording fake link), then the sidecar readiness probe fails. R3-05
    /// requires the failure cleanup scope to release the TUN lease, drop the
    /// helper link/session id and finalize the journal instead of leaving a
    /// phantom lease while reporting `Stopped`.
    #[test]
    fn r305_sidecar_failure_after_helper_releases_tun_lease() {
        use crate::helper_client::{FakeHelperLink, HelperLink};
        use domain::runtime_plan::{
            ConfigSource, PortRequest, PortTransport, ProcessNode, RequiredPrivilege,
        };
        use runtime::tun::{TunAddress, TunSpec, TUN_CONFIG_KIND, TUN_PROCESS_ID};

        let _guard = rr10_lock();
        let state = test_state("r305-cleanup");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        state.set_helper_factory(std::sync::Arc::new(|_| {
            let link: Box<dyn HelperLink> = Box::new(FakeHelperLink::new());
            link
        }));

        // A sidecar port that will never answer the SOCKS greeting.
        let dead_sidecar_port = 11_913u16;
        let mut plan = plan_with_body("tun-sidecar", "{}", 0, None);
        plan.network_policy.tun_enabled = true;
        plan.privileges.push(RequiredPrivilege::Tun);
        let spec = TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            addresses: vec![TunAddress {
                address: "172.18.0.1".into(),
                prefix_len: 30,
            }],
            mtu: Some(1280),
            routes: vec![],
            route_exclude: vec![],
        };
        plan.process_graph.add_process(ProcessNode {
            id: TUN_PROCESS_ID.into(),
            core_type: domain::CoreType::Xray,
            config: ConfigSource::Inline {
                body: serde_json::to_string(&spec).unwrap(),
            },
            ports: vec![],
            privileges: vec![RequiredPrivilege::Tun],
        });
        plan.process_graph.add_process(ProcessNode {
            id: "Xray".into(),
            core_type: domain::CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![],
            privileges: vec![],
        });
        plan.process_graph.add_process(ProcessNode {
            id: "pre-socks".into(),
            core_type: domain::CoreType::Xray,
            config: ConfigSource::Inline { body: "{}".into() },
            ports: vec![],
            privileges: vec![],
        });
        plan.process_graph.depends_on("pre-socks", "Xray");
        plan.ports = vec![
            PortRequest::tcp(0, "inbound"),
            PortRequest {
                port: dead_sidecar_port,
                transport: PortTransport::Tcp,
                owner: "pre-socks".into(),
                exclusive: false,
            },
        ];

        let error = futures_block_on(state.apply_plan(plan)).expect_err("sidecar must fail");
        assert_eq!(error.code, domain::codes::TIMEOUT);

        let inner = futures_block_on(state.inner.lock());
        assert!(inner.session.is_none(), "no running session after failure");
        assert!(inner.tun_lease.is_none(), "TUN lease must be released");
        assert!(
            inner.tun_session_id.is_none(),
            "tun session id must be cleared"
        );
        assert!(inner.tun_link.is_none(), "helper link must be dropped");
        assert!(inner.detail.tun.is_none(), "tun detail must be withdrawn");
        assert_eq!(inner.detail.state, RuntimeState::Stopped);
        assert!(inner.detail.session_id.is_none());
        drop(inner);
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    // -- R3-04: first TUN, interface created by the core ---------------------

    /// A TUN plan that defers the helper work: the adapter does not exist yet.
    fn deferred_tun_plan(port: u16) -> RuntimePlan {
        use domain::runtime_plan::{ProcessNode, RequiredPrivilege};
        use runtime::tun::{TunAddress, TunSpec, TUN_CONFIG_KIND};

        let mut plan = plan_with_body("r304", "{}", port, None);
        plan.network_policy.tun_enabled = true;
        plan.privileges.push(RequiredPrivilege::Tun);
        let spec = TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 0,
            addresses: vec![TunAddress {
                address: "172.18.0.1".into(),
                prefix_len: 30,
            }],
            mtu: Some(1280),
            routes: vec![],
            route_exclude: vec![],
        };
        plan.process_graph.add_process(ProcessNode {
            id: TUN_DEFERRED_PROCESS_ID.into(),
            core_type: domain::CoreType::Xray,
            config: ConfigSource::Inline {
                body: serde_json::to_string(&spec).unwrap(),
            },
            ports: vec![],
            privileges: vec![RequiredPrivilege::Tun],
        });
        plan
    }

    fn set_discovery_env(timeout_ms: &str, interval_ms: &str) {
        std::env::set_var("V2RAYN_R_TUN_DISCOVERY_TIMEOUT_MS", timeout_ms);
        std::env::set_var("V2RAYN_R_TUN_DISCOVERY_INTERVAL_MS", interval_ms);
    }

    fn clear_discovery_env() {
        std::env::remove_var("V2RAYN_R_TUN_DISCOVERY_TIMEOUT_MS");
        std::env::remove_var("V2RAYN_R_TUN_DISCOVERY_INTERVAL_MS");
    }

    #[test]
    fn r304_defers_helper_until_the_core_created_interface_is_discovered() {
        use crate::helper_client::{FakeHelperLink, HelperLink};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let _guard = rr10_lock();
        let state = test_state("r304-ok");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("V2RAYN_R_XRAY_BIN", core_stub(&dir, "stay", "stay"));
        set_discovery_env("2000", "10");
        state.set_helper_factory(Arc::new(|_| {
            let link: Box<dyn HelperLink> = Box::new(FakeHelperLink::new());
            link
        }));
        // The adapter only appears after the core start: the first poll misses.
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        state.set_tun_discovery(Arc::new(move |_adapter| {
            let n = seen.fetch_add(1, Ordering::SeqCst);
            if n >= 1 {
                Some(9)
            } else {
                None
            }
        }));

        futures_block_on(state.apply_plan(deferred_tun_plan(0))).expect("first TUN starts");
        {
            let inner = futures_block_on(state.inner.lock());
            assert!(inner.session.is_some(), "core session running");
            assert_eq!(inner.detail.state, RuntimeState::Running);
            let lease = inner.tun_lease.as_ref().expect("tun lease recorded");
            assert_eq!(lease.spec.interface_index, 9, "discovered index applied");
            assert!(inner.detail.tun.is_some());
        }
        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "polls until the interface appears"
        );

        futures_block_on(state.stop_managed(None));
        let inner = futures_block_on(state.inner.lock());
        assert!(inner.tun_lease.is_none(), "lease released on stop");
        assert!(inner.detail.tun.is_none());
        drop(inner);
        clear_discovery_env();
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn r304_discovery_timeout_rolls_back_without_a_lease() {
        use crate::helper_client::{FakeHelperLink, HelperLink};
        use std::sync::Arc;

        let _guard = rr10_lock();
        let state = test_state("r304-timeout");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("V2RAYN_R_XRAY_BIN", core_stub(&dir, "stay", "stay"));
        set_discovery_env("80", "10");
        state.set_helper_factory(Arc::new(|_| {
            let link: Box<dyn HelperLink> = Box::new(FakeHelperLink::new());
            link
        }));
        // The core never creates the adapter.
        state.set_tun_discovery(Arc::new(|_adapter| None));

        let error = futures_block_on(state.apply_plan(deferred_tun_plan(0)))
            .expect_err("missing adapter must fail the session");
        assert_eq!(error.code, domain::codes::TIMEOUT);
        assert_eq!(error.message_key, "error.tun_interface_timeout");

        let inner = futures_block_on(state.inner.lock());
        assert!(inner.session.is_none(), "core killed on rollback");
        assert!(inner.tun_lease.is_none());
        assert!(inner.detail.tun.is_none());
        assert_eq!(inner.detail.state, RuntimeState::Stopped);
        drop(inner);
        clear_discovery_env();
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn r304_helper_denial_after_discovery_cleans_up() {
        use crate::helper_client::{FakeHelperFault, FakeHelperLink, HelperLink};
        use std::sync::Arc;

        let _guard = rr10_lock();
        let state = test_state("r304-deny");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("V2RAYN_R_XRAY_BIN", core_stub(&dir, "stay", "stay"));
        set_discovery_env("500", "10");
        state.set_helper_factory(Arc::new(|_| {
            let link: Box<dyn HelperLink> =
                Box::new(FakeHelperLink::with_fault(FakeHelperFault::Deny));
            link
        }));
        state.set_tun_discovery(Arc::new(|_adapter| Some(9)));

        let error = futures_block_on(state.apply_plan(deferred_tun_plan(0)))
            .expect_err("helper denial must fail the session");
        assert_eq!(error.code, crate::helper_client::E_TUN_HELPER_UNAVAILABLE);

        let inner = futures_block_on(state.inner.lock());
        assert!(inner.session.is_none());
        assert!(inner.tun_lease.is_none());
        assert!(inner.tun_session_id.is_none());
        assert!(inner.tun_link.is_none());
        assert_eq!(inner.detail.state, RuntimeState::Stopped);
        drop(inner);
        clear_discovery_env();
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }

    #[test]
    fn r304_parses_netsh_adapter_index_like_the_application_side() {
        let sample = "\r\n\
Idx     Met    MTU          State                Name\r\n\
---  ----------  ----------  ------------  ---------------------------\r\n\
  1          75  4294967295  connected     Loopback Pseudo-Interface 1\r\n\
  9          25  1500        connected     v2rayn-tun\r\n";
        assert_eq!(parse_tun_interface_index(sample, "v2rayn-tun"), Some(9));
        assert_eq!(parse_tun_interface_index(sample, "V2RAYN-TUN"), Some(9));
        assert_eq!(parse_tun_interface_index(sample, "nope"), None);
    }

    // -- SP-06 continuous exit observation -----------------------------------

    /// Ghost-Running regression (CP-01): a managed core killed after readiness
    /// must reconcile to Stopped with a structured exit fact on the next read,
    /// never keep reporting the dead PID and port.
    ///
    /// Holds the RR-10 env lock for the whole body: the stub executable
    /// travels through process-global env, and a concurrent stub swap would
    /// turn a must-fail switch into a false pass (or vice versa). The whole
    /// flow runs on one `block_on`: every runtime drop waits for the stub
    /// pipes to close, so intermediate drops would burn the stub's natural
    /// lifetime before the observation runs.
    #[test]
    fn sp06_main_exit_reconciles_to_stopped_with_last_exit() {
        let _guard = rr10_lock();
        let state = test_state("sp06-main-exit");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        // Port 0: readiness by process liveness; the subject is observation.
        let plan = plan_with_body("sp06", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        let run_root = state.config.run_root.clone();
        let facts = futures_block_on(async {
            let epoch_before = state.ipc_snapshot().await.epoch.0;
            state.apply_plan(plan).await.expect("stub session starts");
            let live_pid = {
                let inner = state.inner.lock().await;
                assert_eq!(inner.detail.state, RuntimeState::Running);
                inner.detail.pid.expect("running session publishes its pid")
            };
            // Fault injection: kill the project-owned core through the owned
            // handle (synthetic stub, ports >= 11808 rule untouched: port 0).
            {
                let mut inner = state.inner.lock().await;
                let session = inner.session.as_mut().expect("session runs");
                let _ = session.child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(10), session.child.wait()).await;
            }
            // The next read reconciles: no ghost Running, no stale endpoint.
            let snapshot = state.ipc_snapshot().await;
            let inner = state.inner.lock().await;
            let facts = (
                epoch_before,
                live_pid,
                snapshot.state,
                inner.session.is_none(),
                inner.detail.pid,
                inner.detail.ports.clone(),
                inner.detail.session_id.clone(),
                inner.last_exit.clone(),
                inner.last_exit_sidecar.clone(),
                inner.actual_generation,
                inner.detail.applied_revision,
                inner.detail.error.clone().map(|error| error.message_key),
            );
            drop(inner);
            let epoch_after = state.ipc_snapshot().await.epoch.0;
            (facts, epoch_after)
        });
        let (
            (
                epoch_before,
                live_pid,
                snapshot_state,
                session_gone,
                pid,
                ports,
                session_id,
                last_exit,
                last_exit_sidecar,
                generation,
                applied_revision,
                error_key,
            ),
            epoch_after,
        ) = facts;
        assert_eq!(
            snapshot_state,
            RuntimeState::Stopped,
            "an exited core must not report Running"
        );
        assert!(session_gone, "dead session is withdrawn");
        assert!(pid.is_none(), "no ghost PID");
        assert!(ports.is_empty(), "no stale endpoint");
        assert!(session_id.is_none(), "no stale session fact");
        let exit = last_exit.as_ref().expect("lastExit recorded");
        assert_eq!(exit.pid, live_pid, "exit keeps the owned identity");
        assert!(last_exit_sidecar.is_none(), "main exit, not sidecar");
        assert_eq!(generation, 1, "exit pushes generation");
        // History survives: the applied revision is a fact about what ran.
        assert_eq!(applied_revision, 1);
        assert_eq!(error_key.as_deref(), Some("error.core_exited"));
        assert!(
            epoch_after > epoch_before,
            "exit pushes the fact generation onto the wire"
        );
        let _ = std::fs::remove_dir_all(&run_root);
    }

    /// Sidecar regression: a dead pre-socks sidecar under a live main core
    /// degrades the session instead of reporting a clean Running.
    ///
    /// Holds the RR-10 env lock for the whole body (stub executable travels
    /// through process-global env).
    #[test]
    fn sp06_sidecar_exit_degrades_without_killing_main() {
        let _guard = rr10_lock();
        let state = test_state("sp06-sidecar-exit");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let mut plan = plan_with_body("sp06sc", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        for id in ["Xray", "pre-socks"] {
            plan.process_graph
                .add_process(domain::runtime_plan::ProcessNode {
                    id: id.into(),
                    core_type: domain::CoreType::Xray,
                    config: ConfigSource::Inline {
                        body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                    },
                    ports: vec![],
                    privileges: vec![],
                });
        }
        plan.process_graph.depends_on("pre-socks", "Xray");
        let run_root = state.config.run_root.clone();
        // One runtime for the whole flow (see the main-exit test): every
        // runtime drop waits for the stub pipes to close.
        let facts = futures_block_on(async {
            state.apply_plan(plan).await.expect("graph session starts");
            {
                let mut inner = state.inner.lock().await;
                let session = inner.session.as_mut().expect("session runs");
                assert_eq!(session.sidecars.len(), 1);
                let sidecar = session.sidecars.get_mut(0).expect("sidecar tracked");
                let child = sidecar.child.as_mut().expect("ordinary sidecar");
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(10), child.wait()).await;
            }
            let snapshot = state.ipc_snapshot().await;
            let inner = state.inner.lock().await;
            let facts = (
                snapshot.state,
                inner.session.is_some(),
                inner.session.as_ref().map(|session| session.sidecars.len()),
                inner.detail.pid,
                inner.detail.ports.clone(),
                inner.last_exit.clone(),
                inner.last_exit_sidecar.clone(),
                inner.actual_generation,
                inner.detail.error.clone().map(|error| error.message_key),
            );
            drop(inner);
            let _ = state.stop_managed(None).await;
            facts
        });
        let (
            snapshot_state,
            session_alive,
            sidecar_count,
            pid,
            ports,
            last_exit,
            last_exit_sidecar,
            generation,
            error_key,
        ) = facts;
        assert_eq!(
            snapshot_state,
            RuntimeState::Degraded,
            "a dead sidecar must not read as clean Running"
        );
        assert!(session_alive, "main core keeps running");
        assert_eq!(sidecar_count, Some(0), "dead sidecar withdrawn");
        assert!(pid.is_some(), "main endpoint kept");
        assert!(!ports.is_empty() || pid.is_some());
        assert!(last_exit.is_some(), "sidecar exit recorded");
        assert_eq!(last_exit_sidecar.as_deref(), Some("pre-socks"));
        assert_eq!(generation, 1, "exit pushes generation");
        assert_eq!(error_key.as_deref(), Some("error.sidecar_exited"));
        let _ = std::fs::remove_dir_all(&run_root);
    }

    // -- SP-10 readiness prep (CP-12 / TUN-A05) ------------------------------
    //
    // Red contracts: a readiness timeout must first re-check the actual
    // result (never fail an actually-ready session, never mask a real exit
    // as a timeout), a real elevated launch without a helper identity is not
    // ready, and a helper-observed elevated-sidecar exit degrades the live
    // session through the same fact generation as an ordinary sidecar exit.

    #[test]
    fn sp10_timeout_confirm_maps_actual_results() {
        // Exited while the probe timed out: the real exit wins, not Timeout.
        assert_eq!(
            confirm_readiness_timeout(Some(Some(3)), false),
            TimeoutConfirm::Exited(Some(3))
        );
        // Live process that answers now: actually ready, timeout suppressed.
        assert_eq!(
            confirm_readiness_timeout(None, true),
            TimeoutConfirm::ActuallyReady
        );
        // Live process, still silent: the timeout stands.
        assert_eq!(
            confirm_readiness_timeout(None, false),
            TimeoutConfirm::ConfirmedTimeout
        );
    }

    #[test]
    fn sp10_real_elevated_launch_requires_a_handle() {
        // The helper returned no process identity on the real path: not ready.
        assert!(elevated_launch_verdict(0, false).is_err());
        assert!(elevated_launch_verdict(7, false).is_ok());
        // Dry-run never owns a process: handle 0 stays simulated success.
        assert!(elevated_launch_verdict(0, true).is_ok());
    }

    #[test]
    fn sp10_elevated_sidecar_exit_degrades_live_session() {
        use crate::helper_client::FakeHelperLink;

        let _guard = rr10_lock();
        let state = test_state("sp10-elevated-exit");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        let plan = plan_with_body("sp10el", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        let run_root = state.config.run_root.clone();
        let facts = futures_block_on(async {
            state.apply_plan(plan).await.expect("session starts");
            {
                let mut inner = state.inner.lock().await;
                let session = inner.session.as_mut().expect("session runs");
                session.sidecars.push(SidecarSession {
                    id: "tun-sidecar".into(),
                    child: None,
                    _job: None,
                    helper: Some(Box::new(FakeHelperLink::new())),
                    handle: Some(7),
                    elevated_pid: Some(4242),
                    elevated_exit_code: None,
                });
            }
            assert!(
                state
                    .note_elevated_sidecar_exit("tun-sidecar", Some(1))
                    .await,
                "known elevated sidecar exit is recorded"
            );
            assert!(
                !state
                    .note_elevated_sidecar_exit("no-such-sidecar", None)
                    .await,
                "unknown sidecar ids never fabricate an exit"
            );
            let snapshot = state.ipc_snapshot().await;
            let inner = state.inner.lock().await;
            let facts = (
                snapshot.state,
                inner.session.is_some(),
                inner.detail.pid,
                inner.detail.ports.clone(),
                inner.last_exit.clone(),
                inner.last_exit_sidecar.clone(),
                inner.actual_generation,
                inner.detail.error.clone().map(|error| error.message_key),
            );
            drop(inner);
            let _ = state.stop_managed(None).await;
            facts
        });
        let (
            snapshot_state,
            session_alive,
            pid,
            ports,
            last_exit,
            last_exit_sidecar,
            generation,
            error_key,
        ) = facts;
        assert_eq!(
            snapshot_state,
            RuntimeState::Degraded,
            "an exited elevated sidecar must not read as clean Running"
        );
        assert!(session_alive, "main core keeps running");
        assert!(pid.is_some(), "main endpoint kept");
        assert!(!ports.is_empty() || pid.is_some());
        let exit = last_exit.expect("elevated exit recorded");
        assert_eq!(exit.pid, 4242, "exit keeps the helper-reported identity");
        assert_eq!(last_exit_sidecar.as_deref(), Some("tun-sidecar"));
        assert_eq!(generation, 1, "exit pushes generation");
        assert_eq!(error_key.as_deref(), Some("error.sidecar_exited"));
        let _ = std::fs::remove_dir_all(&run_root);
    }

    #[test]
    fn sp10_live_sidecar_timeout_is_not_an_exit() {
        let _guard = rr10_lock();
        let state = test_state("sp10-sidecar-timeout");
        let dir = state.config.run_root.join("stubs");
        std::fs::create_dir_all(&dir).unwrap();
        let stay = core_stub(&dir, "stay", "stay");
        std::env::set_var("V2RAYN_R_XRAY_BIN", &stay);
        // A free loopback port that nothing answers: the stay stub never
        // listens there, so the sidecar probe must time out while the
        // process is still alive — and report a timeout, not a fake exit.
        let dead_port = {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.local_addr().unwrap().port()
        };
        let mut plan = plan_with_body("sp10to", "{\"inbounds\":[],\"outbounds\":[]}", 0, None);
        for id in ["Xray", "pre-socks"] {
            plan.process_graph
                .add_process(domain::runtime_plan::ProcessNode {
                    id: id.into(),
                    core_type: domain::CoreType::Xray,
                    config: ConfigSource::Inline {
                        body: "{\"inbounds\":[],\"outbounds\":[]}".into(),
                    },
                    ports: if id == "pre-socks" {
                        vec![domain::runtime_plan::PortRequest::tcp(dead_port, "sidecar")]
                    } else {
                        vec![]
                    },
                    privileges: vec![],
                });
        }
        plan.process_graph.depends_on("pre-socks", "Xray");
        let error = futures_block_on(state.apply_plan(plan)).expect_err("sidecar must time out");
        assert_eq!(
            error.message_key, "error.readiness_timeout",
            "a live-but-silent sidecar is a timeout, never a fabricated exit"
        );
        let _ = std::fs::remove_dir_all(&state.config.run_root);
    }
}
