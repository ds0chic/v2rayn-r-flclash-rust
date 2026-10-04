//! Session lifecycle: the state machine that owns exactly one managed core.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::event::{EventKind, RuntimeStateChanged};
use domain::runtime_plan::ConfigSource;
use domain::{DomainError, RuntimePlan, RuntimeState};
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
use crate::helper_client::{build_helper_link, HelperConfig, HelperLink, TunLease};
use crate::journal::{self, JournalEntry};
use crate::tun_lease;

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
        Self {
            pipe_name: std::env::var("V2RAYN_R_PIPE")
                .unwrap_or_else(|_| NET_HOST_PIPE_NAME.to_string()),
            run_root,
            disconnect_grace: env_ms("V2RAYN_R_DISCONNECT_GRACE_MS", 6_000),
            heartbeat_interval: env_ms("V2RAYN_R_HEARTBEAT_MS", 2_000),
            readiness_timeout: env_ms("V2RAYN_R_READY_TIMEOUT_MS", 20_000),
            readiness_interval: env_ms("V2RAYN_R_READY_INTERVAL_MS", 500),
            helper: HelperConfig::from_env(),
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
}

/// One additional managed process in the plan's [`ProcessGraph`] (RR-06):
/// today the pre-SOCKS / LegacyProtect sidecar, distinguished so its logs are
/// labeled and it can be stopped in reverse start order.
pub struct SidecarSession {
    pub child: tokio::process::Child,
    /// Owns the sidecar's Job Object; dropping it (on stop or session drop)
    /// guarantees the sidecar tree dies even if net-host is hard-killed.
    pub _job: JobGuard,
}

impl SidecarSession {
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
}

/// Factory for helper links. Production builds a pipe (or dry-run) link from
/// the host config; tests inject an in-memory fake. Never touches the OS.
pub type HelperLinkFactory =
    std::sync::Arc<dyn Fn(&HelperConfig) -> Box<dyn HelperLink> + Send + Sync>;

pub struct HostState {
    pub inner: Mutex<Inner>,
    pub bus: EventBus,
    pub config: HostConfig,
    pub shutdown: Arc<Notify>,
    helper_factory: std::sync::Mutex<Option<HelperLinkFactory>>,
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_operation_id() -> String {
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
    format!("op-{}-{}", journal::now_ms(), seq)
}

/// Stub-core tests opt out of the real `test_args` config check.
fn skip_config_check() -> bool {
    std::env::var_os("V2RAYN_R_SKIP_CONFIG_CHECK").is_some()
}

/// Structured error for a core that could not be bound into the ownership job.
/// Non-retryable: the caller must not keep an unowned core alive.
fn job_assign_failed(operation_id: &str, detail: impl Into<String>) -> DomainError {
    DomainError::new(domain::codes::JOB_ASSIGN_FAILED, "error.job_assign_failed")
        .with_operation(operation_id)
        .with_detail(detail)
}

impl HostState {
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
            }),
            bus: EventBus::new(),
            config,
            shutdown: Arc::new(Notify::new()),
            helper_factory: std::sync::Mutex::new(None),
        };
        // Reconcile TUN leases from a previous process. Only session dirs that
        // still carry a lease journal are touched; with no stale leases no
        // helper connection is even attempted.
        state.reconcile_tun_leases();
        state
    }

    /// Test seam: replace helper-link construction (in-memory fake, no pipe).
    #[cfg(test)]
    pub fn set_helper_factory(&self, factory: HelperLinkFactory) {
        if let Ok(mut slot) = self.helper_factory.lock() {
            *slot = Some(factory);
        }
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
    pub fn reconcile_tun_leases(&self) {
        let mut link = self.make_helper_link();
        let report = tun_lease::reconcile_stale_tun(&self.config.run_root, &mut *link);
        if report.scanned > 0 {
            eprintln!(
                "[net_host] tun recovery: scanned={} cleaned={} pending={}",
                report.scanned, report.cleaned, report.pending
            );
        }
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

    /// Snapshot of runtime facts for an IPC `GetSnapshot`.
    pub async fn ipc_snapshot(&self) -> RuntimeSnapshot {
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
        let inner = self.inner.lock().await;
        let payload = serde_json::to_value(&inner.detail).unwrap_or(json!({}));
        ServerFrame::Event(
            self.bus
                .make(EventKind::Other(RUNTIME_DETAIL_EVENT.to_string()), payload),
        )
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
    /// the journal), idempotently. Best-effort: a cleanup failure is logged,
    /// never surfaced over the caller's root-cause error.
    async fn release_tun_lease(&self) {
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
            return;
        };
        let run_root = self.config.run_root.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            if let Some(mut link) = link {
                let _ = tun_lease::cleanup_tun_lease(&run_root, &session_id, &lease, &mut *link);
            } else {
                // Link lost without cleanup (should not happen): the helper
                // owns disconnect cleanup for its own session, so drop the
                // journal rather than report a phantom lease forever.
                tun_lease::remove_tun_journal(&run_root, &session_id);
                eprintln!("[net_host] tun link lost for {session_id}; journal dropped");
            }
        })
        .await;
        if let Err(join) = outcome {
            eprintln!("[net_host] tun release task failed: {join}");
        }
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
        self.release_tun_lease().await;
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
        let tun_spec = tun_spec_from_plan(plan)?;

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
            if id == core_id || id == runtime::TUN_PROCESS_ID {
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
            prepared.push(PreparedSidecar {
                id: node.id.clone(),
                adapter,
                exe: node_exe,
                body: node_body,
                port: node_port,
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
        let result = tokio::task::spawn_blocking(move || {
            let mut command = std::process::Command::new(&exe);
            command
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            command.output()
        })
        .await;
        let _ = std::fs::remove_file(&path);
        match result {
            Ok(Ok(output)) if output.status.success() => Ok(()),
            Ok(Ok(output)) => {
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
            Ok(Err(e)) => Err(DomainError::new(
                domain::codes::UNAVAILABLE,
                "error.core_spawn_failed",
            )
            .with_detail(format!("config check spawn failed: {e}"))),
            Err(join) => Err(DomainError::new(
                domain::codes::INTERNAL,
                "error.config_check_failed",
            )
            .with_detail(format!("config check task failed: {join}"))),
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

        // Stop any existing managed session before switching.
        let _ = self.stop_managed(None).await;
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

        // --- Nested process graph (RR-06): start the pre-SOCKS sidecar(s)
        // before the main core, wait for the shared proxy port to accept a
        // SOCKS greeting, then start the main core. Reverse order on any
        // failure; a sidecar that never becomes ready rolls the session back
        // before the main core is spawned.
        let mut sidecar_sessions = Vec::new();
        for sidecar in sidecars {
            let sidecar_dir = journal::session_dir(&self.config.run_root, &session_id)
                .join("processes")
                .join(&sidecar.id);
            if let Err(e) = std::fs::create_dir_all(&sidecar_dir) {
                let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                    .with_operation(&operation_id)
                    .with_detail(format!("create sidecar dir failed: {e}"));
                self.fail_operation(&operation_id, &error).await;
                self.stop_sidecars(&mut sidecar_sessions).await;
                return Err(error);
            }
            let sidecar_config = sidecar_dir.join("config.json");
            if let Err(e) = std::fs::write(&sidecar_config, sidecar.body.as_bytes()) {
                let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                    .with_operation(&operation_id)
                    .with_detail(format!("write sidecar config failed: {e}"));
                self.fail_operation(&operation_id, &error).await;
                self.stop_sidecars(&mut sidecar_sessions).await;
                return Err(error);
            }
            let sidecar_log = sidecar_dir.join("core.log");
            let mut command = Command::new(&sidecar.exe);
            command
                .args(sidecar.adapter.run_args(&sidecar_config))
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                command.creation_flags(CREATE_NO_WINDOW);
            }
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(e) => {
                    let error =
                        DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                            .with_operation(&operation_id)
                            .with_detail(format!("{}: {e}", sidecar.exe.display()));
                    self.fail_operation(&operation_id, &error).await;
                    self.stop_sidecars(&mut sidecar_sessions).await;
                    return Err(error);
                }
            };
            let sidecar_job = match JobGuard::create_kill_on_close() {
                Ok(sidecar_job) => sidecar_job,
                Err(e) => {
                    let _ = child.start_kill();
                    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                    let error =
                        DomainError::new(domain::codes::UNAVAILABLE, "error.job_create_failed")
                            .with_operation(&operation_id)
                            .with_detail(e.to_string());
                    self.fail_operation(&operation_id, &error).await;
                    self.stop_sidecars(&mut sidecar_sessions).await;
                    return Err(error);
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
                    let error = job_assign_failed(&operation_id, e.to_string());
                    self.fail_operation(&operation_id, &error).await;
                    self.stop_sidecars(&mut sidecar_sessions).await;
                    return Err(error);
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
            sidecar_sessions.push(SidecarSession {
                child,
                _job: sidecar_job,
            });
            // Wait for the sidecar to accept the same SOCKS greeting the core
            // will: `WaitForProxyPort` upstream.
            if sidecar.port != 0 {
                let ready = wait_socks_port(
                    sidecar.port,
                    self.config.readiness_timeout,
                    self.config.readiness_interval,
                )
                .await;
                if !ready {
                    let error = DomainError::new(domain::codes::TIMEOUT, "error.readiness_timeout")
                        .with_operation(&operation_id)
                        .with_field("port")
                        .with_detail(format!(
                            "sidecar {} port {} not ready within {:?}",
                            sidecar.id, sidecar.port, self.config.readiness_timeout
                        ));
                    self.fail_operation(&operation_id, &error).await;
                    self.stop_sidecars(&mut sidecar_sessions).await;
                    return Err(error);
                }
            }
        }

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
                self.release_tun_lease().await;
                self.finalize_journal(&journal_entry).await;
                self.fail_operation(&operation_id, &error).await;
                self.stop_sidecars(&mut sidecar_sessions).await;
                return Err(error);
            }
        };

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

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                let error = DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                    .with_operation(&operation_id)
                    .with_detail(format!("{}: {e}", exe.display()));
                self.release_tun_lease().await;
                self.finalize_journal(&journal_entry).await;
                self.fail_operation(&operation_id, &error).await;
                self.stop_sidecars(&mut sidecar_sessions).await;
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
                self.stop_sidecars(&mut sidecar_sessions).await;
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
        let mut sidecar_sessions: Vec<SidecarSession> = std::mem::take(&mut sidecar_sessions);

        let outcome = wait_ready(
            &mut child,
            port,
            self.config.readiness_timeout,
            self.config.readiness_interval,
        )
        .await;

        match outcome {
            ReadyOutcome::Ready => {
                let session = Session {
                    session_id: session_id.clone(),
                    plan_id: plan.plan_id.clone(),
                    desired_revision: plan.desired_revision,
                    port,
                    config_sha256: actual_hash.clone(),
                    exe,
                    child,
                    identity,
                    sidecars: std::mem::take(&mut sidecar_sessions),
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
                self.stop_sidecars(&mut sidecar_sessions).await;
                self.rollback(&operation_id, &mut child, &session_id, error)
                    .await
            }
            ReadyOutcome::Timeout => {
                let error = DomainError::new(domain::codes::TIMEOUT, "error.readiness_timeout")
                    .with_operation(&operation_id)
                    .with_field("port")
                    .with_detail(format!(
                        "port {port} not ready within {:?}",
                        self.config.readiness_timeout
                    ));
                self.stop_sidecars(&mut sidecar_sessions).await;
                self.rollback(&operation_id, &mut child, &session_id, error)
                    .await
            }
        }
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
        self.release_tun_lease().await;
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
    pub async fn stop_managed(&self, _operation_id: Option<String>) -> Option<String> {
        let mut inner = self.inner.lock().await;
        let Some(mut session) = inner.session.take() else {
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
            // between helper-apply and core-spawn); always release.
            self.release_tun_lease().await;
            return None;
        };
        let session_id = session.session_id.clone();
        let pid = session.identity.pid;
        inner.detail.state = RuntimeState::RollingBack;
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
        eprintln!("[net_host] session {session_id} STOPPED pid={pid}");
        drop(inner);
        // Reverse cleanup after the core tree is gone.
        self.release_tun_lease().await;
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
    fn rr06_apply_starts_and_stops_the_sidecar_before_the_core() {
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
        // The sidecar must start before the main core.
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
}
