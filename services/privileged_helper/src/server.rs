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
    check_helper_session, validate_elevated_core, validate_route_entries, validate_tun_address,
    ElevationStatus, HelperError, HelperOp, HelperRequest, HelperResponse, HelperResult,
    RouteEntry, HELPER_MAX_MESSAGE_BYTES, HELPER_PROTOCOL_VERSION,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::audit::{AuditLog, AuditOutcome};
use crate::backend::{HelperBackend, StartedCore};

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
            lease_policy: LeasePolicy::CleanOwned,
        }
    }
}

/// Resources owned by one client connection. Cleanup only ever touches this
/// list, so sessions cannot clean up each other's resources.
#[derive(Debug, Clone, Default)]
pub struct ConnectionLease {
    session_id: String,
    routes: Vec<RouteEntry>,
    tun_interfaces: Vec<u32>,
    cores: Vec<u64>,
    closed: bool,
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
                Ok((
                    HelperResult::RoutesAdded { count },
                    format!("add_routes count={}", entries.len()),
                ))
            }
            HelperOp::RemoveRoutes { entries } => {
                validate_route_entries(entries)?;
                let count = self.backend.remove_routes(entries)?;
                lease.routes.retain(|owned| !entries.contains(owned));
                Ok((
                    HelperResult::RoutesRemoved { count },
                    format!("remove_routes count={}", entries.len()),
                ))
            }
            HelperOp::SetTunAdapterAddress { config } => {
                validate_tun_address(config)?;
                self.backend.set_tun_address(config)?;
                if !lease.tun_interfaces.contains(&config.interface_index) {
                    lease.tun_interfaces.push(config.interface_index);
                }
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
                lease.cores.retain(|owned| owned != handle);
                Ok((
                    HelperResult::CoreStopped { handle: *handle },
                    format!("stop_elevated_core handle={handle}"),
                ))
            }
            HelperOp::Shutdown => {
                self.backend.shutdown()?;
                self.shutdown.store(true, Ordering::SeqCst);
                Ok((HelperResult::Shutdown, "shutdown".to_string()))
            }
        }
    }

    /// Clean up the resources owned by a disconnected session. Idempotent:
    /// a second call on an already-closed lease performs no backend work.
    pub fn on_disconnect(&self, lease: &mut ConnectionLease) {
        if lease.closed {
            return;
        }
        lease.closed = true;
        let policy = self.config.lease_policy;
        match policy {
            LeasePolicy::LeaveRunning => {}
            LeasePolicy::StopCoresOnly => {
                for handle in std::mem::take(&mut lease.cores) {
                    let _ = self.backend.stop_elevated_core(handle);
                }
            }
            LeasePolicy::CleanOwned => {
                if !lease.routes.is_empty() {
                    let routes = std::mem::take(&mut lease.routes);
                    let _ = self.backend.remove_routes(&routes);
                }
                for interface_index in std::mem::take(&mut lease.tun_interfaces) {
                    let _ = self.backend.reset_tun_address(interface_index);
                }
                for handle in std::mem::take(&mut lease.cores) {
                    let _ = self.backend.stop_elevated_core(handle);
                }
            }
        }
        lease.routes.clear();
        lease.tun_interfaces.clear();
        lease.cores.clear();
        self.audit.record(
            &lease.session_id,
            "lease_cleanup",
            &format!("policy={policy:?}"),
            AuditOutcome::Ok,
        );
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
        HelperOp::Shutdown => "shutdown",
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
        HelperError::Backend { .. } | HelperError::Timeout { .. } => AuditOutcome::Error,
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
            server.on_disconnect(&mut lease);
            server.connection_closed();
            return Ok(());
        }
    }

    let timeout_duration = server.config.request_timeout;
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

    server.on_disconnect(&mut lease);
    server.connection_closed();
    Ok(())
}
