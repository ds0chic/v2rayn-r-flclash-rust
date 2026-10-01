//! Session lifecycle: the state machine that owns exactly one managed core.

use std::collections::HashMap;
use std::path::PathBuf;
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

use runtime::{
    adapter_for, matches_identity, process_creation_time_ms, sha256_hex, CoreLocator, JobGuard,
    ProcessIdentity, RuntimeDetail, ServerFrame, NET_HOST_PIPE_NAME, RUNTIME_DETAIL_EVENT,
};

use crate::events::EventBus;
use crate::journal::{self, JournalEntry};

/// Runtime configuration, overridable from the environment for tests.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub pipe_name: String,
    pub run_root: PathBuf,
    pub disconnect_grace: Duration,
    pub heartbeat_interval: Duration,
    pub readiness_timeout: Duration,
    pub readiness_interval: Duration,
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
    pub child: tokio::process::Child,
    pub identity: ProcessIdentity,
    pub job: JobGuard,
}

impl Session {
    /// Terminate and reap the core; closing the job guarantees the tree dies.
    pub async fn terminate_and_wait(&mut self, grace: Duration) {
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
    pub detail: RuntimeDetail,
    pub active_operation: Option<String>,
    pub operations: HashMap<String, OperationStatus>,
    pub active_connections: usize,
    pub no_client_since: Option<Instant>,
    pub recovery: Option<RecoveryStatus>,
}

pub struct HostState {
    pub inner: Mutex<Inner>,
    pub bus: EventBus,
    pub config: HostConfig,
    pub shutdown: Arc<Notify>,
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_operation_id() -> String {
    let seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
    format!("op-{}-{}", journal::now_ms(), seq)
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
        Self {
            inner: Mutex::new(Inner {
                session: None,
                detail: RuntimeDetail::default(),
                active_operation: None,
                operations: HashMap::new(),
                active_connections: 0,
                no_client_since: None,
                recovery: recovery_status,
            }),
            bus: EventBus::new(),
            config,
            shutdown: Arc::new(Notify::new()),
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

    /// Apply an immutable plan: validate, stage, spawn, probe, commit.
    pub async fn apply_plan(&self, plan: RuntimePlan) -> Result<String, DomainError> {
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
        let core = plan.target.core_type;
        let adapter = adapter_for(core).ok_or_else(|| {
            DomainError::new(domain::codes::NOT_FOUND, "error.core_not_supported")
                .with_detail(format!("no adapter for {}", core.as_str()))
        })?;
        let body = match &plan.target.config {
            ConfigSource::Inline { body } => body.clone(),
            ConfigSource::ControlledFile { .. } => {
                let error = DomainError::new(domain::codes::INVALID_PLAN, "error.plan_unsupported")
                    .with_detail("controlled-file configs are not supported in T03");
                self.fail_operation(&operation_id, &error).await;
                return Err(error);
            }
        };
        let actual_hash = sha256_hex(body.as_bytes());
        let declared = plan.target.config_sha256.as_str();
        if !declared.is_empty() && declared != actual_hash {
            let error = DomainError::new(domain::codes::INVALID_PLAN, "error.config_hash_mismatch")
                .with_field("config_sha256")
                .with_detail(format!("declared {declared}, computed {actual_hash}"));
            self.fail_operation(&operation_id, &error).await;
            return Err(error);
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

        let session_seq = SESSION_COUNTER.fetch_add(1, Ordering::AcqRel) + 1;
        let session_id = format!("s-{}-{}", journal::now_ms(), session_seq);
        let dir = journal::session_dir(&self.config.run_root, &session_id);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("create session dir failed: {e}"));
            self.fail_operation(&operation_id, &error).await;
            return Err(error);
        }
        let config_path = dir.join("config.json");
        if let Err(e) = std::fs::write(&config_path, body.as_bytes()) {
            let error = DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(format!("write config failed: {e}"));
            self.fail_operation(&operation_id, &error).await;
            return Err(error);
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

        let locator = CoreLocator::from_env();
        let exe = match locator.resolve(core, plan.target.version.as_deref()) {
            Ok(exe) => exe,
            Err(error) => {
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
        };

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
                self.finalize_journal(&journal_entry).await;
                self.fail_operation(&operation_id, &error).await;
                return Err(error);
            }
        };

        let pid = child.id().unwrap_or(0);
        let created_at_ms = process_creation_time_ms(pid).unwrap_or(0);
        let identity = ProcessIdentity::new(pid, created_at_ms);
        #[cfg(windows)]
        if let Some(handle) = child.raw_handle() {
            if let Err(e) = job.assign(handle) {
                eprintln!("[net_host] job assign failed: {e}");
            }
        }

        // Stream stdout/stderr to the session log, with bounded memory.
        if let Some(stdout) = child.stdout.take() {
            spawn_log_reader(stdout, log_path.clone(), "stdout");
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_reader(stderr, log_path.clone(), "stderr");
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
                let session = Session {
                    session_id: session_id.clone(),
                    plan_id: plan.plan_id.clone(),
                    desired_revision: plan.desired_revision,
                    port,
                    config_sha256: actual_hash.clone(),
                    child,
                    identity,
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
                self.rollback(&operation_id, &mut child, &session_id, error)
                    .await
            }
        }
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
            inner.detail.state = RuntimeState::Stopped;
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
        inner.detail.state = RuntimeState::Stopped;
        inner.detail.pid = None;
        inner.detail.created_at_ms = None;
        inner.detail.error = None;
        inner.active_operation = None;
        eprintln!("[net_host] session {session_id} STOPPED pid={pid}");
        drop(inner);
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

fn spawn_log_reader<S>(stream: S, path: PathBuf, label: &'static str)
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
}
