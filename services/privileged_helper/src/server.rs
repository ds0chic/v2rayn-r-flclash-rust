//! Transport-agnostic helper server: request dispatch, audit and lease cleanup.
//!
//! The framing scheme mirrors the net-host convention: every frame is
//! `u32 little-endian length || JSON payload`, capped at the contract limit.
//! The real Windows named pipe wraps [`serve_connection`]; tests drive it over
//! `tokio::io::duplex`.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ipc_contract::{
    check_helper_session, validate_elevated_core, validate_handle, validate_poll_handles,
    validate_route_entries, validate_tun_address, CoreExitObservation, ElevationStatus,
    HelperError, HelperOp, HelperRequest, HelperResponse, HelperResult, LeaseState, LeaseStatus,
    RouteEntry, HELPER_MAX_MESSAGE_BYTES, HELPER_PROTOCOL_VERSION,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::audit::{now_ms, AuditLog, AuditOutcome};
use crate::backend::{HelperBackend, RouteRemovalOutcome, StartedCore, TunResetOutcome};
use crate::journal::{
    core_label, route_label, tun_label, JournalEntry, JournalKind, ResourceJournal,
};

/// Frame length prefix size in bytes (net-host convention).
pub const LEN_PREFIX_BYTES: usize = 4;

/// What the helper does with resources owned by a disconnected session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeasePolicy {
    /// Remove owned routes, reset owned TUN addresses, stop owned cores.
    CleanOwned,
    /// Stop owned cores but leave routes/TUN untouched.
    StopCoresOnly,
    /// Do nothing; resources outlive the session (explicit opt-in only).
    LeaveRunning,
}

/// Helper server configuration.
#[derive(Debug, Clone)]
pub struct HelperServerConfig {
    /// Shared secret a client must present (never logged).
    pub session_token: String,
    /// Expected client SID; compared against the OS-reported SID when required.
    pub expected_sid: Option<String>,
    /// Whether the connection SID must match `expected_sid`.
    pub require_sid_match: bool,
    /// Controlled roots under which elevated cores may run.
    pub allowed_run_roots: Vec<String>,
    /// Per-request read timeout.
    pub request_timeout: Duration,
    /// Idle keep-alive bound for an open session. A connected client (e.g.
    /// net-host owning an elevated TUN core) may hold the session without
    /// requests for minutes; only a dead pipe, `Shutdown` or this bound ends
    /// it. Must be far larger than [`Self::request_timeout`] so an idle
    /// session never reclaims its active lease (TUN-A01).
    pub idle_timeout: Duration,
    /// Disconnected-session cleanup policy.
    pub lease_policy: LeasePolicy,
}

impl Default for HelperServerConfig {
    fn default() -> Self {
        Self {
            session_token: String::new(),
            expected_sid: None,
            require_sid_match: false,
            allowed_run_roots: Vec::new(),
            request_timeout: Duration::from_millis(ipc_contract::IPC_REQUEST_TIMEOUT_MS),
            idle_timeout: Duration::from_secs(24 * 60 * 60),
            lease_policy: LeasePolicy::CleanOwned,
        }
    }
}

/// Resources owned by one client connection. Cleanup only ever touches this
/// list, so sessions cannot clean up each other's resources.
///
/// SP-08: every owned resource is mirrored in [`ResourceJournal`]. A failed
/// release keeps the resource in the owned list *and* in the journal, and the
/// lease stays open so a retry re-attempts exactly the unconfirmed items.
/// `closed` therefore means "no further cleanup will be attempted", never
/// "failures were forgotten".
#[derive(Debug, Clone, Default)]
pub struct ConnectionLease {
    session_id: String,
    routes: Vec<RouteEntry>,
    tun_interfaces: Vec<u32>,
    cores: Vec<u64>,
    closed: bool,
    journal: ResourceJournal,
    cleanup_attempts: u32,
    /// SP-09: per-handle keep-alive expiry (Unix ms), set at start and
    /// extended by `RenewLease`.
    lease_expires_at_ms: std::collections::BTreeMap<u64, i64>,
    /// Spawn-time pid per owned handle, for status/exit attribution.
    core_pids: std::collections::BTreeMap<u64, u32>,
    /// Handles already released (stop/cleanup confirmed): status history.
    released_cores: std::collections::BTreeSet<u64>,
    /// Handle-scoped exits observed by `PollCoreExits`, kept so
    /// `GetLeaseStatus` can report `Exited` without draining again.
    observed_exits: std::collections::BTreeMap<u64, CoreExitObservation>,
}

impl ConnectionLease {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            ..Self::default()
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn owned_route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn owned_tun_count(&self) -> usize {
        self.tun_interfaces.len()
    }

    pub fn owned_core_count(&self) -> usize {
        self.cores.len()
    }

    pub fn owned_cores(&self) -> Vec<u64> {
        self.cores.clone()
    }

    /// SP-09: track a freshly started owned handle with its initial expiry.
    pub fn track_core_lease(&mut self, handle: u64, pid: u32, expires_at_ms: i64) {
        self.lease_expires_at_ms.insert(handle, expires_at_ms);
        self.core_pids.insert(handle, pid);
        self.observed_exits.remove(&handle);
        self.released_cores.remove(&handle);
    }

    /// SP-09: extend one owned handle's keep-alive. Unknown (never started or
    /// already released) handles are rejected, never silently renewed.
    pub fn renew_core_lease(&mut self, handle: u64, idle: Duration) -> Result<i64, HelperError> {
        if !self.cores.contains(&handle) {
            return Err(HelperError::UnknownHandle { handle });
        }
        let expires = now_ms().saturating_add(idle.as_millis() as i64);
        self.lease_expires_at_ms.insert(handle, expires);
        Ok(expires)
    }

    /// SP-09: mark a handle released (stop/cleanup confirmed).
    pub fn mark_core_released(&mut self, handle: u64) {
        self.cores.retain(|owned| *owned != handle);
        self.released_cores.insert(handle);
        self.lease_expires_at_ms.remove(&handle);
    }

    /// SP-10: remember one observed exit for status queries.
    pub fn note_observed_exit(&mut self, exit: CoreExitObservation) {
        self.observed_exits.insert(exit.handle, exit);
    }

    /// SP-09: lease facts for one handle, or `None` when the handle is not
    /// (and was never) owned by this session.
    pub fn core_lease_status(&self, handle: u64) -> Option<LeaseStatus> {
        let pid = self.core_pids.get(&handle).copied();
        if self.released_cores.contains(&handle) {
            return Some(LeaseStatus {
                handle,
                state: LeaseState::Released,
                pid,
                expires_at_ms: 0,
            });
        }
        if self.cores.contains(&handle) {
            let state = if self.observed_exits.contains_key(&handle) {
                LeaseState::Exited
            } else {
                LeaseState::Active
            };
            return Some(LeaseStatus {
                handle,
                state,
                pid,
                expires_at_ms: self.lease_expires_at_ms.get(&handle).copied().unwrap_or(0),
            });
        }
        None
    }

    /// Journal entries for resources whose release failed and is awaiting
    /// retry. This is what the owning plane (net-host) surfaces to the user.
    pub fn pending_cleanup(&self) -> Vec<JournalEntry> {
        self.journal.pending()
    }

    pub fn pending_cleanup_count(&self) -> usize {
        self.journal.pending_count()
    }

    /// Full owned-resource journal snapshot, including released records.
    pub fn journal_snapshot(&self) -> Vec<JournalEntry> {
        self.journal.snapshot()
    }

    /// How many cleanup passes ran over this lease.
    pub fn cleanup_attempts(&self) -> u32 {
        self.cleanup_attempts
    }
}

/// The helper server. Safe to share across connection tasks.
pub struct HelperServer<B: HelperBackend> {
    backend: Arc<B>,
    config: HelperServerConfig,
    audit: AuditLog,
    session_count: AtomicU32,
    shutdown: AtomicBool,
}

impl<B: HelperBackend> HelperServer<B> {
    pub fn new(backend: Arc<B>, config: HelperServerConfig) -> Self {
        Self {
            backend,
            config,
            audit: AuditLog::default(),
            session_count: AtomicU32::new(0),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn config(&self) -> &HelperServerConfig {
        &self.config
    }

    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    pub fn backend(&self) -> &Arc<B> {
        &self.backend
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    pub fn session_count(&self) -> u32 {
        self.session_count.load(Ordering::SeqCst)
    }

    pub fn connection_opened(&self) {
        self.session_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn connection_closed(&self) {
        self.session_count.fetch_sub(1, Ordering::SeqCst);
    }

    fn elevation(&self) -> ElevationStatus {
        ElevationStatus {
            elevated: self.backend.is_elevated(),
            session_count: self.session_count(),
            protocol_version: HELPER_PROTOCOL_VERSION,
        }
    }

    /// Validate the session, dispatch the operation and audit the outcome.
    pub fn handle(&self, lease: &mut ConnectionLease, request: &HelperRequest) -> HelperResponse {
        let request_id = request.request_id.clone();
        let operation = op_name(&request.operation);
        if let Err(error) = check_helper_session(&request.session) {
            self.audit.record(
                &lease.session_id,
                operation,
                "session rejected",
                AuditOutcome::Rejected,
            );
            return HelperResponse {
                request_id,
                result: HelperResult::Error { error },
            };
        }
        match self.dispatch(lease, &request.operation) {
            Ok((result, summary)) => {
                self.audit
                    .record(&lease.session_id, operation, &summary, AuditOutcome::Ok);
                HelperResponse { request_id, result }
            }
            Err(error) => {
                self.audit.record(
                    &lease.session_id,
                    operation,
                    &format!("error: {}", error_label(&error)),
                    classify(&error),
                );
                HelperResponse {
                    request_id,
                    result: HelperResult::Error { error },
                }
            }
        }
    }

    fn dispatch(
        &self,
        lease: &mut ConnectionLease,
        operation: &HelperOp,
    ) -> Result<(HelperResult, String), HelperError> {
        match operation {
            HelperOp::Ping => Ok((
                HelperResult::Pong {
                    elevation: self.elevation(),
                },
                "ping".to_string(),
            )),
            HelperOp::GetElevationStatus => Ok((
                HelperResult::ElevationStatus {
                    status: self.elevation(),
                },
                "elevation enquiry".to_string(),
            )),
            HelperOp::AddRoutes { entries } => {
                validate_route_entries(entries)?;
                let count = self.backend.add_routes(entries)?;
                lease.routes.extend(entries.iter().cloned());
                for entry in entries {
                    lease.journal.record_owned(
                        JournalKind::Route,
                        route_label(&entry.destination, entry.interface_index),
                    );
                }
                Ok((
                    HelperResult::RoutesAdded { count },
                    format!("add_routes count={}", entries.len()),
                ))
            }
            HelperOp::RemoveRoutes { entries } => {
                validate_route_entries(entries)?;
                let outcome = self.backend.remove_routes(entries)?;
                lease.routes.retain(|owned| !entries.contains(owned));
                for entry in entries {
                    lease.journal.mark_released(
                        JournalKind::Route,
                        &route_label(&entry.destination, entry.interface_index),
                    );
                }
                match outcome {
                    RouteRemovalOutcome::Removed(count) => Ok((
                        HelperResult::RoutesRemoved { count },
                        format!("remove_routes count={}", entries.len()),
                    )),
                    RouteRemovalOutcome::AlreadyGone => {
                        let resource = gone_resource_for_routes(entries);
                        Ok((
                            HelperResult::AlreadyGone {
                                resource: resource.clone(),
                            },
                            format!("remove_routes already_gone resource={resource}"),
                        ))
                    }
                }
            }
            HelperOp::SetTunAdapterAddress { config } => {
                validate_tun_address(config)?;
                self.backend.set_tun_address(config)?;
                if !lease.tun_interfaces.contains(&config.interface_index) {
                    lease.tun_interfaces.push(config.interface_index);
                }
                lease
                    .journal
                    .record_owned(JournalKind::TunAddress, tun_label(config.interface_index));
                Ok((
                    HelperResult::TunAddressSet {
                        interface_index: config.interface_index,
                    },
                    format!(
                        "set_tun_address addresses={} mtu_set={}",
                        config.addresses.len(),
                        config.mtu.is_some()
                    ),
                ))
            }
            HelperOp::RunElevatedCore { spec } => {
                validate_elevated_core(spec, &self.config.allowed_run_roots)?;
                let started: StartedCore = self.backend.run_elevated_core(spec)?;
                lease.cores.push(started.handle);
                lease.track_core_lease(
                    started.handle,
                    started.pid,
                    now_ms().saturating_add(self.config.idle_timeout.as_millis() as i64),
                );
                lease
                    .journal
                    .record_owned(JournalKind::Core, core_label(started.handle));
                Ok((
                    HelperResult::CoreStarted {
                        handle: started.handle,
                        pid: started.pid,
                    },
                    format!(
                        "run_elevated_core core={} args={}",
                        spec.core,
                        spec.args.len()
                    ),
                ))
            }
            HelperOp::StopElevatedCore { handle } => {
                self.backend.stop_elevated_core(*handle)?;
                lease.mark_core_released(*handle);
                lease
                    .journal
                    .mark_released(JournalKind::Core, &core_label(*handle));
                Ok((
                    HelperResult::CoreStopped { handle: *handle },
                    format!("stop_elevated_core handle={handle}"),
                ))
            }
            HelperOp::RenewLease { handle } => {
                validate_handle(*handle)?;
                let expires_at_ms = lease.renew_core_lease(*handle, self.config.idle_timeout)?;
                Ok((
                    HelperResult::LeaseRenewed {
                        handle: *handle,
                        expires_at_ms,
                    },
                    format!("renew_lease handle={handle}"),
                ))
            }
            HelperOp::GetLeaseStatus { handle } => {
                validate_handle(*handle)?;
                let status = lease
                    .core_lease_status(*handle)
                    .ok_or(HelperError::UnknownHandle { handle: *handle })?;
                Ok((
                    HelperResult::LeaseStatus { status },
                    format!("get_lease_status handle={handle}"),
                ))
            }
            HelperOp::PollCoreExits { handles } => {
                validate_poll_handles(handles)?;
                let observed = self.backend.poll_core_exits();
                let at_ms = now_ms();
                let mut exits = Vec::new();
                for exit in observed {
                    let observation = CoreExitObservation {
                        handle: exit.handle,
                        pid: exit.pid,
                        exit_code: exit.exit_code,
                        at_ms,
                    };
                    // Every drained exit is remembered for status queries,
                    // even when the caller did not list that handle this
                    // round (drain-once would otherwise lose the fact).
                    lease.note_observed_exit(observation);
                    if handles.contains(&exit.handle) {
                        exits.push(observation);
                    }
                }
                Ok((
                    HelperResult::CoreExits { exits },
                    format!("poll_core_exits requested={}", handles.len()),
                ))
            }
            HelperOp::Shutdown => {
                self.backend.shutdown()?;
                self.shutdown.store(true, Ordering::SeqCst);
                Ok((HelperResult::Shutdown, "shutdown".to_string()))
            }
        }
    }

    /// Clean up the resources owned by a disconnected session.
    ///
    /// SP-08: each resource is released individually and confirmed. A failed
    /// item stays in the owned list and in the journal, the lease stays open,
    /// and the failure is returned (never swallowed). A retry re-attempts
    /// exactly the unconfirmed items; only a pass with zero failures closes
    /// the lease. Calling again after a fully confirmed cleanup (or after an
    /// explicit `LeaveRunning`) performs no backend work.
    pub fn on_disconnect(&self, lease: &mut ConnectionLease) -> Vec<HelperError> {
        self.run_cleanup(lease, "lease_cleanup")
    }

    /// Retry a previously failed cleanup. Same semantics as
    /// [`Self::on_disconnect`] without requiring a new connection; exposed
    /// so the owning plane can surface "pending cleanup" and retry it.
    pub fn retry_cleanup(&self, lease: &mut ConnectionLease) -> Vec<HelperError> {
        self.run_cleanup(lease, "lease_cleanup_retry")
    }

    fn run_cleanup(&self, lease: &mut ConnectionLease, op: &str) -> Vec<HelperError> {
        if lease.closed {
            // Closed means fully released or explicitly left running: there
            // is nothing unconfirmed to retry, so no backend work happens.
            return Vec::new();
        }
        lease.cleanup_attempts += 1;
        let mut failures: Vec<HelperError> = Vec::new();
        match self.config.lease_policy {
            LeasePolicy::LeaveRunning => {
                lease.closed = true;
                self.audit.record(
                    &lease.session_id,
                    op,
                    &format!(
                        "policy=LeaveRunning left routes={} tun={} cores={}",
                        lease.routes.len(),
                        lease.tun_interfaces.len(),
                        lease.cores.len()
                    ),
                    AuditOutcome::Ok,
                );
                return Vec::new();
            }
            LeasePolicy::StopCoresOnly => {
                self.release_cores(lease, &mut failures);
            }
            LeasePolicy::CleanOwned => {
                // Reverse-dependency order: stop owned cores before
                // tearing down the addresses and routes they used.
                self.release_cores(lease, &mut failures);
                self.release_tun_addresses(lease, &mut failures);
                self.release_routes(lease, &mut failures);
            }
        }
        // A lease with unconfirmed cleanups is never marked closed: a later
        // retry must still see and re-attempt the retained resources.
        lease.closed = failures.is_empty();
        let (outcome, summary) = if failures.is_empty() {
            (
                AuditOutcome::Ok,
                format!("policy={:?} released", self.config.lease_policy),
            )
        } else {
            let labels: Vec<&str> = failures.iter().map(error_label).collect();
            (
                AuditOutcome::Error,
                format!(
                    "policy={:?} cleanup_failures={} [{}] pending={} (needs owner retry)",
                    self.config.lease_policy,
                    failures.len(),
                    labels.join(","),
                    lease.journal.pending_count(),
                ),
            )
        };
        self.audit.record(&lease.session_id, op, &summary, outcome);
        failures
    }

    /// Release owned cores one handle at a time; failed handles stay owned.
    fn release_cores(&self, lease: &mut ConnectionLease, failures: &mut Vec<HelperError>) {
        let handles = std::mem::take(&mut lease.cores);
        for handle in handles {
            match self.backend.stop_elevated_core(handle) {
                Ok(()) => {
                    lease.mark_core_released(handle);
                    lease
                        .journal
                        .mark_released(JournalKind::Core, &core_label(handle));
                }
                Err(error) => {
                    lease.journal.mark_failed(
                        JournalKind::Core,
                        core_label(handle),
                        error_label(&error),
                    );
                    lease.cores.push(handle);
                    failures.push(error);
                }
            }
        }
    }

    /// Reset owned TUN addresses one interface at a time. An `AlreadyGone`
    /// backend confirmation releases the journal record like a success: the
    /// desired end state holds, so no failure is retained and the lease may
    /// close.
    fn release_tun_addresses(&self, lease: &mut ConnectionLease, failures: &mut Vec<HelperError>) {
        let interfaces = std::mem::take(&mut lease.tun_interfaces);
        for interface_index in interfaces {
            match self.backend.reset_tun_address(interface_index) {
                Ok(TunResetOutcome::Reset) | Ok(TunResetOutcome::AlreadyGone) => {
                    lease
                        .journal
                        .mark_released(JournalKind::TunAddress, &tun_label(interface_index));
                }
                Err(error) => {
                    lease.journal.mark_failed(
                        JournalKind::TunAddress,
                        tun_label(interface_index),
                        error_label(&error),
                    );
                    lease.tun_interfaces.push(interface_index);
                    failures.push(error);
                }
            }
        }
    }

    /// Remove owned routes one entry at a time so a single failure retains
    /// exactly its own entry while confirmed entries release. An `AlreadyGone`
    /// confirmation releases the entry: absent is the desired end state.
    fn release_routes(&self, lease: &mut ConnectionLease, failures: &mut Vec<HelperError>) {
        let routes = std::mem::take(&mut lease.routes);
        for entry in routes {
            let label = route_label(&entry.destination, entry.interface_index);
            match self.backend.remove_routes(std::slice::from_ref(&entry)) {
                Ok(RouteRemovalOutcome::Removed(_)) | Ok(RouteRemovalOutcome::AlreadyGone) => {
                    lease.journal.mark_released(JournalKind::Route, &label);
                }
                Err(error) => {
                    lease
                        .journal
                        .mark_failed(JournalKind::Route, label, error_label(&error));
                    lease.routes.push(entry);
                    failures.push(error);
                }
            }
        }
    }
}

/// Compare an OS-reported client SID against the expected one.
pub fn sid_matches(expected: &str, actual: &str) -> bool {
    !expected.is_empty() && expected.eq_ignore_ascii_case(actual)
}

fn op_name(operation: &HelperOp) -> &'static str {
    match operation {
        HelperOp::Ping => "ping",
        HelperOp::GetElevationStatus => "get_elevation_status",
        HelperOp::AddRoutes { .. } => "add_routes",
        HelperOp::RemoveRoutes { .. } => "remove_routes",
        HelperOp::SetTunAdapterAddress { .. } => "set_tun_adapter_address",
        HelperOp::RunElevatedCore { .. } => "run_elevated_core",
        HelperOp::StopElevatedCore { .. } => "stop_elevated_core",
        HelperOp::RenewLease { .. } => "renew_lease",
        HelperOp::GetLeaseStatus { .. } => "get_lease_status",
        HelperOp::PollCoreExits { .. } => "poll_core_exits",
        HelperOp::Shutdown => "shutdown",
    }
}

/// Bounded resource label for an `AlreadyGone` batch response. Single-entry
/// batches carry the exact route label; multi-entry batches carry the count
/// plus the first label so the audit trail stays bounded and redacted.
fn gone_resource_for_routes(entries: &[RouteEntry]) -> String {
    match entries {
        [only] => route_label(&only.destination, only.interface_index),
        [first, ..] => format!(
            "routes count={} first={}",
            entries.len(),
            route_label(&first.destination, first.interface_index)
        ),
        [] => "routes count=0".to_string(),
    }
}

fn error_label(error: &HelperError) -> &'static str {
    match error {
        HelperError::VersionMismatch { .. } => "version_mismatch",
        HelperError::MessageTooLarge { .. } => "message_too_large",
        HelperError::Malformed { .. } => "malformed",
        HelperError::Unauthorized { .. } => "unauthorized",
        HelperError::NotAllowlisted { .. } => "not_allowlisted",
        HelperError::PathOutOfBounds { .. } => "path_out_of_bounds",
        HelperError::UnknownHandle { .. } => "unknown_handle",
        HelperError::Backend { .. } => "backend",
        HelperError::JobAssignFailed { .. } => "job_assign_failed",
        HelperError::Timeout { .. } => "timeout",
    }
}

fn classify(error: &HelperError) -> AuditOutcome {
    match error {
        HelperError::VersionMismatch { .. }
        | HelperError::MessageTooLarge { .. }
        | HelperError::Malformed { .. }
        | HelperError::Unauthorized { .. }
        | HelperError::NotAllowlisted { .. }
        | HelperError::PathOutOfBounds { .. }
        | HelperError::UnknownHandle { .. } => AuditOutcome::Rejected,
        HelperError::Backend { .. }
        | HelperError::JobAssignFailed { .. }
        | HelperError::Timeout { .. } => AuditOutcome::Error,
    }
}

enum FrameError {
    Io(std::io::Error),
    TooLarge(usize),
}

async fn read_frame<R>(reader: &mut R) -> Result<Vec<u8>, FrameError>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0u8; LEN_PREFIX_BYTES];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(FrameError::Io)?;
    let len = u32::from_le_bytes(prefix) as usize;
    if len > HELPER_MAX_MESSAGE_BYTES {
        return Err(FrameError::TooLarge(len));
    }
    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(FrameError::Io)?;
    Ok(payload)
}

async fn write_response<W>(writer: &mut W, response: &HelperResponse) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let payload = serde_json::to_vec(response).map_err(std::io::Error::other)?;
    if payload.len() > HELPER_MAX_MESSAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "helper response exceeds the frame cap",
        ));
    }
    let mut out = Vec::with_capacity(LEN_PREFIX_BYTES + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    writer.write_all(&out).await?;
    writer.flush().await
}

fn error_response(error: HelperError) -> HelperResponse {
    HelperResponse {
        request_id: String::new(),
        result: HelperResult::Error { error },
    }
}

/// Serve one connection until `Shutdown`, timeout, EOF or SID rejection.
///
/// `peer_sid` is the SID reported by the OS for the connected peer; when the
/// server requires a SID match and it does not match, the connection is
/// rejected before any operation is processed.
pub async fn serve_connection<S, B>(
    stream: S,
    server: Arc<HelperServer<B>>,
    mut lease: ConnectionLease,
    peer_sid: Option<String>,
) -> std::io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
    B: HelperBackend,
{
    server.connection_opened();
    let (mut reader, mut writer) = tokio::io::split(stream);

    if server.config.require_sid_match {
        let expected = server.config.expected_sid.as_deref().unwrap_or("");
        let actual = peer_sid.as_deref().unwrap_or("");
        if !sid_matches(expected, actual) {
            let response = error_response(HelperError::Unauthorized {
                detail: "client SID mismatch".to_string(),
            });
            let _ = write_response(&mut writer, &response).await;
            let failures = server.on_disconnect(&mut lease);
            if !failures.is_empty() {
                eprintln!("[helper] disconnect cleanup failures: {}", failures.len());
            }
            server.connection_closed();
            return Ok(());
        }
    }

    // A connected session stays alive while the client is silent: the bound
    // here is the long idle keep-alive, not the per-request timeout. Using the
    // request timeout would tear down an idle session after ~5s and stop its
    // owned elevated core (TUN-A01).
    let timeout_duration = server.config.idle_timeout;
    loop {
        match tokio::time::timeout(timeout_duration, read_frame(&mut reader)).await {
            Err(_) => {
                let response = error_response(HelperError::Timeout {
                    timeout_ms: timeout_duration.as_millis() as u64,
                });
                let _ = write_response(&mut writer, &response).await;
                break;
            }
            Ok(Err(FrameError::TooLarge(size))) => {
                let response = error_response(HelperError::MessageTooLarge {
                    size,
                    limit: HELPER_MAX_MESSAGE_BYTES,
                });
                let _ = write_response(&mut writer, &response).await;
                break;
            }
            Ok(Err(FrameError::Io(error))) => {
                eprintln!("[helper] read error: {error}");
                break;
            }
            Ok(Ok(payload)) => {
                let request = match serde_json::from_slice::<HelperRequest>(&payload) {
                    Ok(request) => request,
                    Err(error) => {
                        let response = error_response(HelperError::Malformed {
                            detail: format!("decode failed: {error}"),
                        });
                        let _ = write_response(&mut writer, &response).await;
                        continue;
                    }
                };
                let is_shutdown = matches!(request.operation, HelperOp::Shutdown);
                let response = server.handle(&mut lease, &request);
                if write_response(&mut writer, &response).await.is_err() {
                    break;
                }
                if is_shutdown {
                    break;
                }
            }
        }
    }

    let failures = server.on_disconnect(&mut lease);
    if !failures.is_empty() {
        eprintln!("[helper] disconnect cleanup failures: {}", failures.len());
    }
    server.connection_closed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{FakeBackend, FakeOp};

    fn valid_session() -> ipc_contract::SessionIdentity {
        ipc_contract::SessionIdentity {
            protocol_version: HELPER_PROTOCOL_VERSION,
            session_token: "tok".into(),
            peer_pid: 7,
            peer_created_at_ms: 1,
        }
    }

    fn valid_entry() -> RouteEntry {
        RouteEntry {
            destination: "0.0.0.0/0".into(),
            next_hop: "10.0.0.1".into(),
            interface_index: 3,
            metric: 1,
            family: ipc_contract::AddressFamily::V4,
        }
    }

    fn add_route_request() -> HelperRequest {
        HelperRequest {
            session: valid_session(),
            request_id: "r1".into(),
            operation: HelperOp::AddRoutes {
                entries: vec![valid_entry()],
            },
        }
    }

    #[tokio::test]
    async fn idle_session_survives_past_the_request_timeout() {
        // TUN-A01: an idle but connected session must keep its lease. The old
        // code reused `request_timeout` as the read bound and tore the session
        // (and its elevated cores) down after ~5s of silence.
        let backend = Arc::new(FakeBackend::new());
        let server = Arc::new(HelperServer::new(
            backend,
            HelperServerConfig {
                lease_policy: LeasePolicy::CleanOwned,
                request_timeout: Duration::from_millis(50),
                idle_timeout: Duration::from_secs(60),
                ..HelperServerConfig::default()
            },
        ));
        let (client, server_stream) = tokio::io::duplex(1024);
        let lease = ConnectionLease::new("s-idle");
        let task = tokio::spawn(async move {
            let _ = serve_connection(server_stream, server, lease, None).await;
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !task.is_finished(),
            "an idle session must stay open past the per-request timeout"
        );
        drop(client);
        let _ = tokio::time::timeout(Duration::from_secs(5), task).await;
    }

    #[test]
    fn disconnect_cleanup_failure_is_surfaced_and_audited() {
        let backend = Arc::new(FakeBackend::new().fail_on(
            FakeOp::RemoveRoutes,
            HelperError::Backend {
                detail: "boom".into(),
            },
        ));
        let server = HelperServer::new(
            backend.clone(),
            HelperServerConfig {
                lease_policy: LeasePolicy::CleanOwned,
                ..HelperServerConfig::default()
            },
        );
        let mut lease = ConnectionLease::new("s-cleanup-fail");
        let response = server.handle(&mut lease, &add_route_request());
        assert!(matches!(
            response.result,
            HelperResult::RoutesAdded { count: 1 }
        ));
        let failures = server.on_disconnect(&mut lease);
        assert_eq!(failures.len(), 1);
        // SP-08 (CP-04): the failed route stays owned, the lease stays open,
        // and the journal exposes it for retry. A retry must re-attempt the
        // retained resource, never report quiet success.
        assert_eq!(lease.owned_route_count(), 1);
        assert!(!lease.is_closed());
        assert_eq!(lease.pending_cleanup_count(), 1);
        let records = server.audit().records();
        let last = records.last().expect("lease_cleanup audit record");
        assert_eq!(last.operation, "lease_cleanup");
        assert_eq!(last.outcome, AuditOutcome::Error);
        assert!(last.summary.contains("cleanup_failures=1"));
        assert!(server.on_disconnect(&mut lease).len() == 1);
        assert_eq!(backend.attempt_count(FakeOp::RemoveRoutes), 2);
        // Once the fault clears, retry confirms the release and closes.
        backend.clear_failure();
        assert!(server.retry_cleanup(&mut lease).is_empty());
        assert_eq!(lease.owned_route_count(), 0);
        assert_eq!(lease.pending_cleanup_count(), 0);
        assert!(lease.is_closed());
        // A fully confirmed cleanup stays idempotent with no backend work.
        let attempts = backend.attempt_count(FakeOp::RemoveRoutes);
        assert!(server.on_disconnect(&mut lease).is_empty());
        assert_eq!(backend.attempt_count(FakeOp::RemoveRoutes), attempts);
    }

    #[test]
    fn disconnect_cleanup_success_audits_ok() {
        let backend = Arc::new(FakeBackend::new());
        let server = HelperServer::new(
            backend,
            HelperServerConfig {
                lease_policy: LeasePolicy::CleanOwned,
                ..HelperServerConfig::default()
            },
        );
        let mut lease = ConnectionLease::new("s-cleanup-ok");
        let response = server.handle(&mut lease, &add_route_request());
        assert!(matches!(
            response.result,
            HelperResult::RoutesAdded { count: 1 }
        ));
        assert!(server.on_disconnect(&mut lease).is_empty());
        let records = server.audit().records();
        let last = records.last().expect("lease_cleanup audit record");
        assert_eq!(last.outcome, AuditOutcome::Ok);
    }
}
