//! net-host side privileged-helper client and TUN lease (T14).
//!
//! net-host owns the session; the helper owns the elevated platform work. This
//! module is the *only* place net-host talks to the helper. It:
//!
//! - builds the [`HelperLink`] transport (named pipe on Windows, a dry-run
//!   recorder, or an explicit "unavailable" stub elsewhere);
//! - applies a validated [`TunSpec`] as `AddRoutes` + `SetTunAdapterAddress`
//!   over one persistent helper session, and rolls back its own partial work;
//! - cleans the lease idempotently (only its own recorded routes + adapter);
//! - never silently degrades: an unreachable or denying helper becomes a
//!   structured `E_TUN_HELPER_UNAVAILABLE` failure.
//!
//! The helper itself enforces `LeasePolicy::CleanOwned` on disconnect, so a
//! net-host crash still releases the resources this module recorded.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

#[cfg(windows)]
use std::io::{Read, Write};

use domain::{codes, DomainError};
use ipc_contract::{
    CoreExitObservation, ElevatedCoreSpec, HelperError, HelperOp, HelperRequest, HelperResponse,
    HelperResult, LeaseStatus, SessionIdentity, HELPER_PROTOCOL_VERSION,
};
#[cfg(test)]
use ipc_contract::{RouteEntry, TunAddressConfig};
use runtime::tun::{route_digest, route_summary, TunSpec};
use serde::{Deserialize, Serialize};

/// Stable code for "the TUN helper is unreachable or refused the request".
/// The UI must return to the real (unmodified) network state on this error and
/// must not fall back to a direct/downgraded TUN path.
pub const E_TUN_HELPER_UNAVAILABLE: &str = "E_TUN_HELPER_UNAVAILABLE";

/// Where the helper lives and how net-host may start it.
#[derive(Debug, Clone)]
pub struct HelperConfig {
    /// Named pipe the helper listens on.
    pub pipe_name: String,
    /// Shared token presented on every request; never logged.
    pub token: String,
    /// Helper executable to launch on demand, when configured.
    pub bin: Option<PathBuf>,
    /// Whether net-host may launch the helper.
    pub auto_launch: bool,
    /// Controlled roots an elevated core may run under.
    pub allowed_run_roots: Vec<String>,
    /// When true, never contact the helper or touch the OS.
    pub dry_run: bool,
}

impl Default for HelperConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

impl HelperConfig {
    pub fn from_env() -> Self {
        let dry_run = runtime::tun::dry_run_from_env(&std::env::args().collect::<Vec<_>>());
        // Packaged installs place `privileged_helper.exe` next to
        // `net_host.exe`; discovering it here lets a plain RC auto-launch the
        // helper (and generate a token) with no user-set environment.
        let bin = std::env::var_os("V2RAYN_R_HELPER_BIN")
            .map(PathBuf::from)
            .or_else(default_helper_bin);
        Self {
            pipe_name: std::env::var("V2RAYN_R_HELPER_PIPE")
                .unwrap_or_else(|_| ipc_contract::HELPER_PIPE_NAME.to_string()),
            token: std::env::var("V2RAYN_R_HELPER_TOKEN").unwrap_or_default(),
            bin,
            auto_launch: std::env::var_os("V2RAYN_R_HELPER_NO_AUTOLAUNCH").is_none(),
            allowed_run_roots: std::env::var("V2RAYN_R_HELPER_RUN_ROOTS")
                .ok()
                .map(|value| {
                    value
                        .split(';')
                        .map(|part| part.trim().to_string())
                        .filter(|part| !part.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            dry_run,
        }
    }

    /// The token net-host presents and (when it launches the helper) passes on
    /// the helper command line. When the operator did not configure one and the
    /// helper may be auto-launched, a per-process token is generated so a plain
    /// RC does not require `V2RAYN_R_HELPER_TOKEN` on the user's environment. A
    /// pre-existing helper with a different token is rejected by the handshake
    /// rather than silently trusted.
    pub fn effective_token(&self) -> Option<String> {
        if !self.token.trim().is_empty() {
            return Some(self.token.clone());
        }
        if self.auto_launch && self.bin.is_some() {
            return Some(generated_helper_token().to_string());
        }
        None
    }
}

/// The packaged helper next to this executable, when it exists. Windows-only
/// install shape (`net_host.exe` + `privileged_helper.exe` in one directory).
fn default_helper_bin() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let exe = std::env::current_exe().ok()?;
        let candidate = exe.parent()?.join("privileged_helper.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Process-stable generated helper token (never logged). Derived from the
/// net-host pid plus a monotonic clock reading; uniqueness is sufficient for a
/// local, same-user named-pipe handshake.
fn generated_helper_token() -> &'static str {
    static TOKEN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    TOKEN.get_or_init(|| {
        let identity = runtime::current_identity();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        format!("nh-{:x}-{:x}", identity.pid, nanos)
    })
}

/// Quote a helper launch argument for `ShellExecuteW` when it contains spaces.
#[cfg(windows)]
fn quote_arg(value: &str) -> String {
    if value.is_empty() {
        "\"\"".to_string()
    } else if value.contains(' ') {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

/// The recorded TUN resource lease. It is the net-host-side counterpart of the
/// helper's `ConnectionLease`: only the resources named here are ever cleaned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunLease {
    /// Helper-side session that owns these resources.
    pub helper_session_id: String,
    pub adapter_name: String,
    pub interface_index: u32,
    pub route_count: u32,
    /// Order-insensitive digest of the applied routes.
    pub route_digest: String,
    /// Full descriptor, kept so recovery can re-derive the exact cleanup.
    pub spec: TunSpec,
    /// Whether this lease was a dry-run (no helper / no OS touched).
    pub dry_run: bool,
}

impl TunLease {
    pub fn new(helper_session_id: impl Into<String>, spec: TunSpec, dry_run: bool) -> Self {
        let entries = spec.to_route_entries().unwrap_or_default();
        let route_count = entries.len() as u32;
        let route_digest = route_digest(&entries);
        Self {
            helper_session_id: helper_session_id.into(),
            adapter_name: spec.adapter_name.trim().to_string(),
            interface_index: spec.interface_index,
            route_count,
            route_digest,
            spec,
            dry_run,
        }
    }

    /// Redacted one-line summary for logs and journals (no next-hop addresses).
    pub fn summary(&self) -> String {
        format!(
            "adapter={} if={} {} digest={} dry_run={}",
            self.adapter_name,
            self.interface_index,
            route_summary(&self.spec.to_route_entries().unwrap_or_default()),
            &self.route_digest[..self.route_digest.len().min(12)],
            self.dry_run
        )
    }
}

/// Transport/executor the net-host uses for elevated TUN work.
pub trait HelperLink: Send {
    /// Helper session identifier (empty for the unavailable stub).
    fn session_id(&self) -> String;

    /// Whether this link is a dry-run (never touches helper/OS).
    fn dry_run(&self) -> bool;

    /// Confirm the helper is reachable and will accept a session.
    fn available(&mut self) -> Result<(), DomainError>;

    /// Apply the descriptor and return the owned lease.
    fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError>;

    /// Clean an owned lease. Must be safe to call repeatedly.
    fn cleanup(&mut self, lease: &TunLease) -> Result<(), DomainError>;

    /// Start a core elevated through the helper (`RunElevatedCore`). Required
    /// for a Wintun-creating TUN core: a normal-privilege spawn cannot create
    /// the adapter. Returns `(helper handle, pid)`.
    fn run_core(&mut self, spec: &ElevatedCoreSpec) -> Result<(u64, u32), DomainError> {
        let _ = spec;
        Err(tun_helper_unavailable(
            "this link cannot run elevated cores",
        ))
    }

    /// Stop a core previously started through this link. Idempotent.
    fn stop_core(&mut self, handle: u64) -> Result<(), DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable(
            "this link cannot stop elevated cores",
        ))
    }

    /// Renew one owned elevated-core lease (SP-09 `RenewLease`). Returns the
    /// helper-reported expiry (Unix ms). Unknown handles are a structured
    /// not-found, never a silent success. Lease ops use the contract
    /// per-request timeout class ([`ipc_contract::helper_timeout_for`]).
    fn renew_lease(&mut self, handle: u64) -> Result<i64, DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable(
            "this link cannot renew helper leases",
        ))
    }

    /// Read one owned handle's lease state without renewing it (SP-09
    /// `GetLeaseStatus`). Non-draining: a later `poll_core_exits` still
    /// observes the same exit.
    fn get_lease_status(&mut self, handle: u64) -> Result<LeaseStatus, DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable(
            "this link cannot read helper lease status",
        ))
    }

    /// Poll handle-scoped elevated-core exits (SP-10 `PollCoreExits`).
    /// Drain-once per handle: an exit is reported at most once; the helper
    /// remembers it for `get_lease_status` afterwards. Observations for
    /// handles outside `handles` must be ignored by the caller, never
    /// fabricated into session state.
    fn poll_core_exits(
        &mut self,
        handles: &[u64],
    ) -> Result<Vec<CoreExitObservation>, DomainError> {
        let _ = handles;
        Err(tun_helper_unavailable(
            "this link cannot poll helper core exits",
        ))
    }

    /// In-memory audit of intended/actual operations (redacted).
    fn audit(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Structured "helper unavailable" error. `denied` distinguishes a refusal
/// (UAC/authorization) from an unreachable helper, but both use the same
/// stable code so the UI treats them identically.
pub fn tun_helper_unavailable(detail: impl Into<String>) -> DomainError {
    DomainError::new(E_TUN_HELPER_UNAVAILABLE, "error.tun_helper_unavailable").with_detail(detail)
}

/// Helper refused the session (authorization / allow-list).
pub fn tun_helper_denied(detail: impl Into<String>) -> DomainError {
    DomainError::new(E_TUN_HELPER_UNAVAILABLE, "error.tun_helper_denied").with_detail(detail)
}

/// The helper accepted the session but a backend operation failed.
pub fn tun_apply_failed(detail: impl Into<String>) -> DomainError {
    DomainError::new(codes::UNAVAILABLE, "error.tun_apply_failed").with_detail(detail)
}

/// Map a helper protocol error onto the net-host error model.
pub fn map_helper_error(error: HelperError) -> DomainError {
    match error {
        HelperError::Unauthorized { detail }
        | HelperError::NotAllowlisted { detail }
        | HelperError::PathOutOfBounds { detail } => tun_helper_denied(detail),
        HelperError::Timeout { timeout_ms } => {
            tun_helper_unavailable(format!("helper timed out after {timeout_ms} ms")).retryable()
        }
        HelperError::Backend { detail } | HelperError::JobAssignFailed { detail } => {
            tun_apply_failed(detail)
        }
        other => other.to_domain(),
    }
}

/// Fault injection for the in-memory helper used by tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub enum FakeHelperFault {
    /// Reachable but refuses the session.
    Deny,
    /// Reachable but never answers.
    Timeout,
    /// The pipe drops during apply.
    Disconnect,
    /// Routes are added, then the adapter set fails.
    PartialTun,
    /// SP-08: apply succeeds, but every cleanup fails with a backend error.
    /// Resources are left intact so the retry path can be exercised.
    CleanupFails,
    /// SP-08: apply succeeds, but every cleanup reports the helper
    /// unreachable (retryable). Resources are left intact.
    CleanupUnavailable,
}

#[derive(Debug, Default)]
#[cfg(test)]
struct FakeLease {
    pid: u32,
    expires_at_ms: i64,
    released: bool,
}

#[derive(Debug, Default)]
#[cfg(test)]
struct FakeHelperState {
    connected: bool,
    routes: Vec<RouteEntry>,
    tun: Option<TunAddressConfig>,
    ops: Vec<String>,
    apply_count: u32,
    cleanup_count: u32,
    cleanup_attempts: u32,
    cleaned: bool,
    elevated_cores: Vec<ElevatedCoreSpec>,
    next_core_handle: u64,
    /// Owned elevated-core leases, mirroring the helper server's
    /// `ConnectionLease` (per-handle expiry, pid, released history).
    leases: std::collections::BTreeMap<u64, FakeLease>,
    /// Exits staged for the next poll (drain-once source).
    pending_exits: std::collections::BTreeMap<u64, CoreExitObservation>,
    /// Drained exits, remembered so status reports `Exited` afterwards.
    observed_exits: std::collections::BTreeMap<u64, CoreExitObservation>,
}

/// In-memory helper used by unit tests. It records every operation and can be
/// told to deny, time out, disconnect or partially fail.
#[derive(Debug)]
#[cfg(test)]
pub struct FakeHelperLink {
    session_id: String,
    fault: Option<FakeHelperFault>,
    state: Mutex<FakeHelperState>,
}

#[cfg(test)]
impl Default for FakeHelperLink {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl FakeHelperLink {
    pub fn new() -> Self {
        Self {
            session_id: "helper-session-1".into(),
            fault: None,
            state: Mutex::new(FakeHelperState {
                connected: true,
                ..FakeHelperState::default()
            }),
        }
    }

    pub fn with_fault(fault: FakeHelperFault) -> Self {
        let mut link = Self::new();
        link.fault = Some(fault);
        if fault == FakeHelperFault::Disconnect {
            link.lock().connected = false;
        }
        link
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeHelperState> {
        self.state.lock().expect("fake helper poisoned")
    }

    /// Switch the injected fault mid-test (e.g. fail cleanups, then recover).
    pub fn set_fault(&mut self, fault: Option<FakeHelperFault>) {
        self.fault = fault;
    }

    pub fn ops(&self) -> Vec<String> {
        self.lock().ops.clone()
    }

    pub fn route_count(&self) -> usize {
        self.lock().routes.len()
    }

    pub fn has_tun(&self) -> bool {
        self.lock().tun.is_some()
    }

    pub fn apply_count(&self) -> u32 {
        self.lock().apply_count
    }

    pub fn cleanup_count(&self) -> u32 {
        self.lock().cleanup_count
    }

    /// Every cleanup call, including failed ones (unlike `cleanup_count`,
    /// which only counts confirmed releases).
    pub fn cleanup_attempts(&self) -> u32 {
        self.lock().cleanup_attempts
    }

    /// Keep-alive term granted to a freshly started fake core, mirroring the
    /// SP-09 provisional lease term the host negotiates with the real helper.
    pub fn lease_term_ms() -> i64 {
        crate::lifecycle::HELPER_LEASE_TERM_MS as i64
    }

    fn fake_now_ms() -> i64 {
        crate::journal::now_ms()
    }

    /// Stage a helper-observed exit for `handle`. The next
    /// `poll_core_exits` listing `handle` drains it exactly once.
    pub fn inject_core_exit(&self, handle: u64, pid: u32, exit_code: Option<i32>) {
        let mut state = self.lock();
        let at_ms = Self::fake_now_ms();
        state.pending_exits.insert(
            handle,
            CoreExitObservation {
                handle,
                pid,
                exit_code,
                at_ms,
            },
        );
    }

    /// Test seam: record a previously established lease without a launch.
    /// Models the lease granted before the helper became unreachable: the
    /// renew/retry path can run against it even while every link call fails.
    pub fn seed_lease(&self, handle: u64, pid: u32, expires_at_ms: i64) {
        self.lock().leases.insert(
            handle,
            FakeLease {
                pid,
                expires_at_ms,
                released: false,
            },
        );
    }

    pub fn lease_expires_at(&self, handle: u64) -> Option<i64> {
        self.lock()
            .leases
            .get(&handle)
            .map(|lease| lease.expires_at_ms)
    }

    /// Mirror of the contract `validate_poll_handles`: the fake rejects the
    /// same malformed lists the real helper would, so host tests cannot pass
    /// with a request the wire would refuse.
    fn check_poll_handles(handles: &[u64]) -> Result<(), DomainError> {
        if handles.is_empty() {
            return Err(map_helper_error(HelperError::malformed(
                "poll handle list is empty",
            )));
        }
        if handles.len() > ipc_contract::HELPER_MAX_POLL_HANDLES {
            return Err(map_helper_error(HelperError::malformed(format!(
                "{} poll handles exceed the limit of {}",
                handles.len(),
                ipc_contract::HELPER_MAX_POLL_HANDLES
            ))));
        }
        let mut seen = std::collections::BTreeSet::new();
        for (index, handle) in handles.iter().enumerate() {
            if *handle == 0 {
                return Err(map_helper_error(HelperError::malformed(format!(
                    "handles[{index}] must be non-zero"
                ))));
            }
            if !seen.insert(*handle) {
                return Err(map_helper_error(HelperError::malformed(format!(
                    "handles[{index}] duplicates handle {handle}"
                ))));
            }
        }
        Ok(())
    }

    fn check_link_usable(&self) -> Result<(), DomainError> {
        match self.fault {
            Some(FakeHelperFault::Deny) => Err(tun_helper_denied("fake helper denied")),
            Some(FakeHelperFault::Timeout) => {
                Err(tun_helper_unavailable("fake helper timed out").retryable())
            }
            Some(FakeHelperFault::Disconnect) => {
                Err(tun_helper_unavailable("fake helper disconnected"))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
impl HelperLink for FakeHelperLink {
    fn session_id(&self) -> String {
        self.session_id.clone()
    }

    fn dry_run(&self) -> bool {
        false
    }

    fn available(&mut self) -> Result<(), DomainError> {
        match self.fault {
            Some(FakeHelperFault::Deny) => Err(tun_helper_denied("fake helper denied")),
            Some(FakeHelperFault::Timeout) => {
                Err(tun_helper_unavailable("fake helper timed out").retryable())
            }
            Some(FakeHelperFault::Disconnect) => {
                Err(tun_helper_unavailable("fake helper disconnected"))
            }
            _ => Ok(()),
        }
    }

    fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
        match self.fault {
            Some(FakeHelperFault::Deny) => return Err(tun_helper_denied("fake helper denied")),
            Some(FakeHelperFault::Timeout) => {
                return Err(tun_helper_unavailable("fake helper timed out").retryable())
            }
            Some(FakeHelperFault::Disconnect) => {
                let mut state = self.lock();
                state.connected = false;
                return Err(tun_helper_unavailable(
                    "fake helper disconnected during apply",
                ));
            }
            _ => {}
        }
        let entries = spec.to_route_entries()?;
        let mut state = self.lock();
        if !entries.is_empty() {
            state
                .ops
                .push(format!("add_routes count={}", entries.len()));
            state.routes = entries;
        }
        if self.fault == Some(FakeHelperFault::PartialTun) {
            // The helper accepted the routes but the adapter step failed; it
            // must not leave the routes behind.
            state.routes.clear();
            state.ops.push("set_tun_address failed".into());
            return Err(tun_apply_failed("fake set_tun_adapter_address failed"));
        }
        state.tun = Some(spec.to_tun_address_config());
        state.ops.push("set_tun_address".into());
        state.apply_count += 1;
        state.cleaned = false;
        drop(state);
        Ok(TunLease::new(self.session_id.clone(), spec.clone(), false))
    }

    fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
        let mut state = self.lock();
        state.cleanup_attempts += 1;
        if self.fault == Some(FakeHelperFault::CleanupFails) {
            state.ops.push("cleanup failed".into());
            return Err(tun_apply_failed("fake cleanup backend failure"));
        }
        if self.fault == Some(FakeHelperFault::CleanupUnavailable) {
            state.ops.push("cleanup unavailable".into());
            return Err(
                tun_helper_unavailable("fake helper unreachable during cleanup").retryable(),
            );
        }
        if state.cleaned {
            return Ok(());
        }
        if !state.connected {
            // The pipe is gone: the helper cleaned its own lease on disconnect.
            state.routes.clear();
            state.tun = None;
            state.cleaned = true;
            state.ops.push("disconnect_cleanup".into());
            return Ok(());
        }
        let removed = state.routes.len();
        state.ops.push(format!("remove_routes count={removed}"));
        state.routes.clear();
        state.ops.push("reset_tun_address".into());
        state.tun = None;
        state.cleanup_count += 1;
        state.cleaned = true;
        Ok(())
    }

    fn audit(&self) -> Vec<String> {
        self.lock().ops.clone()
    }

    fn run_core(&mut self, spec: &ElevatedCoreSpec) -> Result<(u64, u32), DomainError> {
        match self.fault {
            Some(FakeHelperFault::Deny) => return Err(tun_helper_denied("fake helper denied")),
            Some(FakeHelperFault::Timeout) => {
                return Err(tun_helper_unavailable("fake helper timed out").retryable())
            }
            Some(FakeHelperFault::Disconnect) => {
                return Err(tun_helper_unavailable("fake helper disconnected"))
            }
            _ => {}
        }
        let mut state = self.lock();
        state
            .ops
            .push(format!("run_elevated_core core={}", spec.core));
        state.next_core_handle += 1;
        let handle = state.next_core_handle;
        state.elevated_cores.push(spec.clone());
        state.leases.insert(
            handle,
            FakeLease {
                pid: 4242,
                expires_at_ms: Self::fake_now_ms() + Self::lease_term_ms(),
                released: false,
            },
        );
        Ok((handle, 4242))
    }

    fn stop_core(&mut self, handle: u64) -> Result<(), DomainError> {
        let mut state = self.lock();
        state
            .ops
            .push(format!("stop_elevated_core handle={handle}"));
        if let Some(lease) = state.leases.get_mut(&handle) {
            lease.released = true;
            lease.expires_at_ms = 0;
        }
        Ok(())
    }

    fn renew_lease(&mut self, handle: u64) -> Result<i64, DomainError> {
        self.check_link_usable()?;
        if handle == 0 {
            return Err(map_helper_error(HelperError::malformed(
                "handle must be non-zero",
            )));
        }
        let mut state = self.lock();
        let Some(lease) = state.leases.get_mut(&handle) else {
            return Err(map_helper_error(HelperError::UnknownHandle { handle }));
        };
        if lease.released {
            return Err(map_helper_error(HelperError::UnknownHandle { handle }));
        }
        let expires_at_ms = Self::fake_now_ms() + Self::lease_term_ms();
        lease.expires_at_ms = expires_at_ms;
        state.ops.push(format!("renew_lease handle={handle}"));
        Ok(expires_at_ms)
    }

    fn get_lease_status(&mut self, handle: u64) -> Result<LeaseStatus, DomainError> {
        use ipc_contract::LeaseState;

        self.check_link_usable()?;
        if handle == 0 {
            return Err(map_helper_error(HelperError::malformed(
                "handle must be non-zero",
            )));
        }
        let state = self.lock();
        let Some(lease) = state.leases.get(&handle) else {
            return Err(map_helper_error(HelperError::UnknownHandle { handle }));
        };
        if lease.released {
            return Ok(LeaseStatus {
                handle,
                state: LeaseState::Released,
                pid: Some(lease.pid),
                expires_at_ms: 0,
            });
        }
        let observed = state.observed_exits.contains_key(&handle);
        Ok(LeaseStatus {
            handle,
            state: if observed {
                LeaseState::Exited
            } else {
                LeaseState::Active
            },
            pid: Some(lease.pid),
            expires_at_ms: lease.expires_at_ms,
        })
    }

    fn poll_core_exits(
        &mut self,
        handles: &[u64],
    ) -> Result<Vec<CoreExitObservation>, DomainError> {
        self.check_link_usable()?;
        Self::check_poll_handles(handles)?;
        let mut state = self.lock();
        let mut exits = Vec::new();
        for handle in handles {
            if let Some(exit) = state.pending_exits.remove(handle) {
                state.observed_exits.insert(*handle, exit);
                exits.push(exit);
            }
        }
        state
            .ops
            .push(format!("poll_core_exits requested={}", handles.len()));
        Ok(exits)
    }
}

/// Dry-run link: records what *would* happen and never touches a helper or the
/// OS. Used by `--dry-run-tun` / `V2RAYN_R_DRY_RUN_TUN`.
#[derive(Debug, Default)]
pub struct DryRunHelperLink {
    ops: Mutex<Vec<String>>,
}

impl DryRunHelperLink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ops(&self) -> Vec<String> {
        self.ops.lock().map(|ops| ops.clone()).unwrap_or_default()
    }
}

impl HelperLink for DryRunHelperLink {
    fn session_id(&self) -> String {
        "dry-run".into()
    }

    fn dry_run(&self) -> bool {
        true
    }

    fn available(&mut self) -> Result<(), DomainError> {
        Ok(())
    }

    fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
        let lease = TunLease::new("dry-run", spec.clone(), true);
        let mut ops = self
            .ops
            .lock()
            .map_err(|_| DomainError::new(codes::INTERNAL, "error.lock_poisoned"))?;
        if !spec.routes.is_empty() {
            ops.push(format!("dry_run:add_routes count={}", spec.routes.len()));
        }
        ops.push(format!(
            "dry_run:set_tun_address adapter={} if={}",
            spec.adapter_name.trim(),
            spec.interface_index
        ));
        drop(ops);
        Ok(lease)
    }

    fn cleanup(&mut self, lease: &TunLease) -> Result<(), DomainError> {
        if let Ok(mut ops) = self.ops.lock() {
            ops.push(format!(
                "dry_run:remove_routes count={} then reset_tun_address if={}",
                lease.route_count, lease.interface_index
            ));
        }
        Ok(())
    }

    fn run_core(&mut self, spec: &ElevatedCoreSpec) -> Result<(u64, u32), DomainError> {
        if let Ok(mut ops) = self.ops.lock() {
            ops.push(format!("dry_run:run_elevated_core core={}", spec.core));
        }
        Ok((0, 0))
    }

    fn stop_core(&mut self, handle: u64) -> Result<(), DomainError> {
        if let Ok(mut ops) = self.ops.lock() {
            ops.push(format!("dry_run:stop_elevated_core handle={handle}"));
        }
        Ok(())
    }

    fn renew_lease(&mut self, handle: u64) -> Result<i64, DomainError> {
        let _ = handle;
        // Dry-run owns no helper lease: there is nothing to renew. The host
        // gates real renewals on a non-zero elevated handle, so this is only
        // reached by a programming error, never by a simulated session.
        Err(tun_helper_unavailable("dry-run helper link owns no leases"))
    }

    fn get_lease_status(&mut self, handle: u64) -> Result<LeaseStatus, DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable("dry-run helper link owns no leases"))
    }

    fn poll_core_exits(
        &mut self,
        handles: &[u64],
    ) -> Result<Vec<CoreExitObservation>, DomainError> {
        let _ = handles;
        // Simulated cores never exit through the helper.
        Ok(Vec::new())
    }

    fn audit(&self) -> Vec<String> {
        self.ops()
    }
}

/// Explicit "helper not usable on this platform/configuration" link. Any TUN
/// apply fails loudly with [`E_TUN_HELPER_UNAVAILABLE`]; nothing is downgraded.
#[derive(Debug, Default)]
#[cfg(any(test, not(windows)))]
pub struct UnavailableHelperLink;

#[cfg(any(test, not(windows)))]
impl UnavailableHelperLink {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(any(test, not(windows)))]
impl HelperLink for UnavailableHelperLink {
    fn session_id(&self) -> String {
        String::new()
    }

    fn dry_run(&self) -> bool {
        false
    }

    fn available(&mut self) -> Result<(), DomainError> {
        Err(tun_helper_unavailable(
            "privileged helper is not available on this host",
        ))
    }

    fn apply(&mut self, _spec: &TunSpec) -> Result<TunLease, DomainError> {
        Err(tun_helper_unavailable(
            "privileged helper is not available on this host",
        ))
    }

    fn cleanup(&mut self, _lease: &TunLease) -> Result<(), DomainError> {
        Err(tun_helper_unavailable(
            "privileged helper is not available for cleanup",
        ))
    }

    fn renew_lease(&mut self, handle: u64) -> Result<i64, DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable(
            "privileged helper is not available for lease renewal",
        ))
    }

    fn get_lease_status(&mut self, handle: u64) -> Result<LeaseStatus, DomainError> {
        let _ = handle;
        Err(tun_helper_unavailable(
            "privileged helper is not available for lease status",
        ))
    }

    fn poll_core_exits(
        &mut self,
        handles: &[u64],
    ) -> Result<Vec<CoreExitObservation>, DomainError> {
        let _ = handles;
        Err(tun_helper_unavailable(
            "privileged helper is not available for exit polling",
        ))
    }
}

/// Build the transport from configuration. Dry-run wins over everything.
pub fn build_helper_link(config: &HelperConfig) -> Box<dyn HelperLink> {
    if config.dry_run {
        return Box::new(DryRunHelperLink::new());
    }
    #[cfg(windows)]
    {
        Box::new(PipeHelperLink::new(config.clone()))
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Box::new(UnavailableHelperLink::new())
    }
}

#[cfg(windows)]
struct PipeHelperConn {
    file: std::fs::File,
    session_id: String,
    seq: u64,
}

/// Windows named-pipe transport. One persistent connection per lease, so the
/// helper's disconnect cleanup covers exactly one TUN session.
#[cfg(windows)]
pub struct PipeHelperLink {
    config: HelperConfig,
    conn: Option<PipeHelperConn>,
}

#[cfg(windows)]
impl PipeHelperLink {
    pub fn new(config: HelperConfig) -> Self {
        Self { config, conn: None }
    }

    fn identity(&self) -> SessionIdentity {
        let identity = runtime::current_identity();
        SessionIdentity {
            protocol_version: HELPER_PROTOCOL_VERSION,
            session_token: self.config.effective_token().unwrap_or_default(),
            peer_pid: identity.pid,
            peer_created_at_ms: identity.created_at_ms,
        }
    }

    fn ensure_connection(&mut self) -> Result<(), DomainError> {
        if self.conn.is_some() {
            return Ok(());
        }
        let token = self
            .config
            .effective_token()
            .ok_or_else(|| tun_helper_denied("helper token is not configured"))?;
        if let Ok(file) = open_pipe(&self.config.pipe_name) {
            self.conn = Some(PipeHelperConn {
                file,
                session_id: format!("nh-{}", runtime::current_identity().pid),
                seq: 0,
            });
            return Ok(());
        }
        if !self.config.auto_launch {
            return Err(tun_helper_unavailable("helper is not running"));
        }
        let bin = self
            .config
            .bin
            .clone()
            .ok_or_else(|| tun_helper_unavailable("helper executable is not configured"))?;
        launch_helper(&bin, &self.config, &token)?;
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(file) = open_pipe(&self.config.pipe_name) {
                self.conn = Some(PipeHelperConn {
                    file,
                    session_id: format!("nh-{}", runtime::current_identity().pid),
                    seq: 0,
                });
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(tun_helper_unavailable(
                    "helper did not accept a connection within 10s",
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn call(&mut self, operation: HelperOp) -> Result<HelperResult, DomainError> {
        self.ensure_connection()?;
        let identity = self.identity();
        let conn = self.conn.as_mut().expect("connection established");
        conn.seq += 1;
        let request = HelperRequest {
            session: identity,
            request_id: format!("{}-{}", conn.session_id, conn.seq),
            operation,
        };
        let bytes = runtime::encode_frame(&request)
            .map_err(|e| tun_helper_unavailable(format!("encode failed: {e}")))?;
        if let Err(e) = conn.file.write_all(&bytes).and_then(|_| conn.file.flush()) {
            self.conn = None;
            return Err(tun_helper_unavailable(format!("helper write failed: {e}")));
        }
        let mut prefix = [0u8; runtime::LEN_PREFIX_BYTES];
        if let Err(e) = conn.file.read_exact(&mut prefix) {
            self.conn = None;
            return Err(tun_helper_unavailable(format!("helper read failed: {e}")));
        }
        let len = runtime::frame_len(prefix);
        if !runtime::frame_len_ok(len) {
            self.conn = None;
            return Err(tun_helper_unavailable(format!(
                "helper frame too large: {len}"
            )));
        }
        let mut payload = vec![0u8; len as usize];
        if let Err(e) = conn.file.read_exact(&mut payload) {
            self.conn = None;
            return Err(tun_helper_unavailable(format!("helper read failed: {e}")));
        }
        let response: HelperResponse = runtime::decode_payload(&payload)
            .map_err(|e| tun_helper_unavailable(format!("helper decode failed: {e}")))?;
        match response.result {
            HelperResult::Error { error } => Err(map_helper_error(error)),
            result => Ok(result),
        }
    }
}

#[cfg(windows)]
impl HelperLink for PipeHelperLink {
    fn session_id(&self) -> String {
        self.conn
            .as_ref()
            .map(|conn| conn.session_id.clone())
            .unwrap_or_default()
    }

    fn dry_run(&self) -> bool {
        false
    }

    fn available(&mut self) -> Result<(), DomainError> {
        self.call(HelperOp::Ping).map(|_| ())
    }

    fn apply(&mut self, spec: &TunSpec) -> Result<TunLease, DomainError> {
        let entries = spec.to_route_entries()?;
        if !entries.is_empty() {
            if let Err(error) = self.call(HelperOp::AddRoutes {
                entries: entries.clone(),
            }) {
                // Roll back anything already applied by *this* attempt.
                let _ = self.call(HelperOp::RemoveRoutes { entries });
                return Err(error);
            }
        }
        if let Err(error) = self.call(HelperOp::SetTunAdapterAddress {
            config: spec.to_tun_address_config(),
        }) {
            let _ = self.call(HelperOp::RemoveRoutes {
                entries: spec.to_route_entries()?,
            });
            return Err(error);
        }
        let session = self.session_id();
        Ok(TunLease::new(session, spec.clone(), false))
    }

    fn cleanup(&mut self, lease: &TunLease) -> Result<(), DomainError> {
        let entries = lease.spec.to_route_entries()?;
        if !entries.is_empty() {
            // SP-08: a removal failure is returned, never swallowed. The
            // journal stays so the cleanup is retried; dropping the local
            // connection still lets the helper's disconnect path run, but the
            // caller only reports success once a cleanup is confirmed (or the
            // resources are proven already-absent). Per-resource confirmation
            // needs the A02 `ReleaseOwnedResources` report; until then any
            // `Err` keeps the lease pending.
            if let Err(error) = self.call(HelperOp::RemoveRoutes { entries }) {
                self.conn = None;
                return Err(error);
            }
        }
        // Closing the session lets the helper reset the owned TUN adapter via
        // its `LeasePolicy::CleanOwned` disconnect path (there is no explicit
        // reset op in the frozen helper protocol).
        self.conn = None;
        Ok(())
    }

    fn run_core(&mut self, spec: &ElevatedCoreSpec) -> Result<(u64, u32), DomainError> {
        match self.call(HelperOp::RunElevatedCore { spec: spec.clone() })? {
            HelperResult::CoreStarted { handle, pid } => Ok((handle, pid)),
            _ => Err(tun_apply_failed(
                "helper returned an unexpected result for RunElevatedCore",
            )),
        }
    }

    fn stop_core(&mut self, handle: u64) -> Result<(), DomainError> {
        match self.call(HelperOp::StopElevatedCore { handle })? {
            HelperResult::CoreStopped { .. } => Ok(()),
            _ => Err(tun_apply_failed(
                "helper returned an unexpected result for StopElevatedCore",
            )),
        }
    }

    fn renew_lease(&mut self, handle: u64) -> Result<i64, DomainError> {
        match self.call(HelperOp::RenewLease { handle })? {
            HelperResult::LeaseRenewed {
                handle: renewed,
                expires_at_ms,
            } if renewed == handle => Ok(expires_at_ms),
            HelperResult::LeaseRenewed {
                handle: renewed, ..
            } => Err(tun_apply_failed(format!(
                "helper renewed handle {renewed} for requested handle {handle}"
            ))),
            _ => Err(tun_apply_failed(
                "helper returned an unexpected result for RenewLease",
            )),
        }
    }

    fn get_lease_status(&mut self, handle: u64) -> Result<LeaseStatus, DomainError> {
        match self.call(HelperOp::GetLeaseStatus { handle })? {
            HelperResult::LeaseStatus { status } if status.handle == handle => Ok(status),
            HelperResult::LeaseStatus { status } => Err(tun_apply_failed(format!(
                "helper reported status for handle {} for requested handle {handle}",
                status.handle
            ))),
            _ => Err(tun_apply_failed(
                "helper returned an unexpected result for GetLeaseStatus",
            )),
        }
    }

    fn poll_core_exits(
        &mut self,
        handles: &[u64],
    ) -> Result<Vec<CoreExitObservation>, DomainError> {
        match self.call(HelperOp::PollCoreExits {
            handles: handles.to_vec(),
        })? {
            HelperResult::CoreExits { exits } => Ok(exits),
            _ => Err(tun_apply_failed(
                "helper returned an unexpected result for PollCoreExits",
            )),
        }
    }
}

#[cfg(windows)]
fn open_pipe(pipe_name: &str) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
}

/// Launch the helper elevated through UAC (`runas`). The token and allowed run
/// roots travel as command-line arguments (the helper also still honors the
/// legacy environment variables), so a plain RC no longer needs the operator to
/// export them. A user cancel (Windows error 1223) is a structured denial and
/// leaves no half-started helper behind; any other `ShellExecuteW` failure is an
/// unavailable-helper error. The caller never falls back to an unelevated path.
#[cfg(windows)]
fn launch_helper(
    bin: &std::path::Path,
    config: &HelperConfig,
    token: &str,
) -> Result<(), DomainError> {
    let params = format!(
        "--serve --token {} --run-roots {}",
        quote_arg(token),
        quote_arg(&config.allowed_run_roots.join(";")),
    );
    match elevation::shell_execute_runas(bin.as_os_str(), std::ffi::OsStr::new(&params)) {
        Ok(()) => Ok(()),
        Err(code) if elevation::is_user_cancelled(code) => Err(tun_helper_denied(
            "TUN helper elevation was cancelled by the user",
        )),
        Err(code) => Err(tun_helper_unavailable(format!(
            "failed to elevate TUN helper (ShellExecuteW code {code})"
        ))),
    }
}

/// Direct `ShellExecuteW` binding (shell32). Avoids adding a UI-shell feature
/// to the `windows` crate just for the `runas` verb.
#[cfg(windows)]
mod elevation {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    /// `ShellExecuteW` returns an `HINSTANCE`; values `<= 32` are error codes.
    pub const ERROR_CANCELLED: isize = 1223;
    const SW_HIDE: i32 = 0;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> *mut core::ffi::c_void;
    }

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Whether a `ShellExecuteW` result is a user-cancelled UAC prompt.
    pub fn is_user_cancelled(code: isize) -> bool {
        code == ERROR_CANCELLED
    }

    /// Launch `file` elevated with the `runas` verb. Returns the raw
    /// `ShellExecuteW` error code on failure (UAC cancel = 1223).
    pub fn shell_execute_runas(file: &OsStr, parameters: &OsStr) -> Result<(), isize> {
        let operation = wide(OsStr::new("runas"));
        let file = wide(file);
        let parameters = wide(parameters);
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                parameters.as_ptr(),
                std::ptr::null(),
                SW_HIDE,
            )
        } as isize;
        if result <= 32 {
            Err(result)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipc_contract::AddressFamily;
    use runtime::tun::{TunAddress, TunRoute, TUN_CONFIG_KIND};

    #[test]
    fn fake_helper_records_elevated_core_lifecycle() {
        let mut link = FakeHelperLink::new();
        let spec = ElevatedCoreSpec {
            core: "sing-box".into(),
            exe_path: r"C:\run\s1\processes\pre-socks\sing-box.exe".into(),
            args: vec!["run".into(), "-c".into(), "config.json".into()],
            run_dir: r"C:\run\s1\processes\pre-socks".into(),
        };
        let (handle, pid) = link.run_core(&spec).expect("run_core");
        assert_eq!(handle, 1);
        assert_eq!(pid, 4242);
        link.stop_core(handle).expect("stop_core");
        let ops = link.ops();
        assert!(ops.iter().any(|op| op == "run_elevated_core core=sing-box"));
        assert!(ops.iter().any(|op| op == "stop_elevated_core handle=1"));
    }

    #[test]
    fn fake_helper_denies_elevated_core_on_fault() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Deny);
        let spec = ElevatedCoreSpec {
            core: "sing-box".into(),
            exe_path: r"C:\run\s1\sing-box.exe".into(),
            args: vec![],
            run_dir: r"C:\run\s1".into(),
        };
        let error = link.run_core(&spec).expect_err("denied");
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
    }

    #[test]
    fn effective_token_generation_requires_autolaunch() {
        let mut config = HelperConfig {
            pipe_name: "p".into(),
            token: String::new(),
            bin: Some(std::path::PathBuf::from("helper.exe")),
            auto_launch: true,
            allowed_run_roots: vec![],
            dry_run: false,
        };
        assert!(config.effective_token().is_some());
        config.auto_launch = false;
        assert!(config.effective_token().is_none());
        config.token = "explicit".into();
        assert_eq!(config.effective_token().as_deref(), Some("explicit"));
    }

    #[cfg(windows)]
    #[test]
    fn elevation_cancel_maps_to_user_cancelled() {
        assert!(elevation::is_user_cancelled(elevation::ERROR_CANCELLED));
        assert!(!elevation::is_user_cancelled(5));
    }

    #[cfg(windows)]
    #[test]
    fn quote_arg_wraps_spaces() {
        assert_eq!(quote_arg("abc"), "abc");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg(""), "\"\"");
    }

    fn spec() -> TunSpec {
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
        }
    }

    fn spec_without_routes() -> TunSpec {
        let mut value = spec();
        value.routes.clear();
        value
    }

    #[test]
    fn fake_apply_records_routes_and_adapter() {
        let mut link = FakeHelperLink::new();
        link.available().unwrap();
        let lease = link.apply(&spec()).unwrap();
        assert_eq!(lease.route_count, 1);
        assert_eq!(lease.interface_index, 9);
        assert_eq!(link.route_count(), 1);
        assert!(link.has_tun());
        assert_eq!(link.apply_count(), 1);
        assert_eq!(
            link.ops(),
            vec![
                "add_routes count=1".to_string(),
                "set_tun_address".to_string()
            ]
        );
    }

    #[test]
    fn fake_apply_without_routes_skips_add_routes() {
        let mut link = FakeHelperLink::new();
        link.apply(&spec_without_routes()).unwrap();
        assert_eq!(link.ops(), vec!["set_tun_address".to_string()]);
        assert_eq!(link.route_count(), 0);
        assert!(link.has_tun());
    }

    #[test]
    fn fake_denied_maps_to_tun_helper_unavailable() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Deny);
        let error = link.apply(&spec()).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert_eq!(error.message_key, "error.tun_helper_denied");
        assert!(!error.retryable);
        assert_eq!(link.apply_count(), 0);
    }

    #[test]
    fn fake_timeout_maps_to_retryable_unavailable() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Timeout);
        let error = link.available().unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert!(error.retryable);
    }

    #[test]
    fn fake_disconnect_fails_apply_then_cleanup_is_idempotent() {
        let mut link = FakeHelperLink::new();
        let lease = link.apply(&spec()).unwrap();
        link.fault = Some(FakeHelperFault::Disconnect);
        let mut disconnected = FakeHelperLink::with_fault(FakeHelperFault::Disconnect);
        let error = disconnected.apply(&spec()).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        // The original lease still cleans normally.
        link.cleanup(&lease).unwrap();
        assert!(!link.has_tun());
        assert_eq!(link.cleanup_count(), 1);
    }

    #[test]
    fn fake_partial_tun_rolls_back_routes() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::PartialTun);
        let error = link.apply(&spec()).unwrap_err();
        assert_eq!(error.code, codes::UNAVAILABLE);
        assert_eq!(
            link.route_count(),
            0,
            "partial failure must not leak routes"
        );
        assert!(!link.has_tun());
        assert_eq!(link.apply_count(), 0);
    }

    #[test]
    fn fake_cleanup_is_idempotent() {
        let mut link = FakeHelperLink::new();
        let lease = link.apply(&spec()).unwrap();
        link.cleanup(&lease).unwrap();
        let ops_after_first = link.ops();
        link.cleanup(&lease).unwrap();
        assert_eq!(
            link.ops(),
            ops_after_first,
            "second cleanup must be a no-op"
        );
        assert_eq!(link.cleanup_count(), 1);
        assert!(!link.has_tun());
        assert_eq!(link.route_count(), 0);
    }

    #[test]
    fn fake_disconnect_cleanup_is_idempotent() {
        let mut link = FakeHelperLink::new();
        let lease = link.apply(&spec()).unwrap();
        // Simulate the helper dropping the pipe while net-host is gone.
        link.lock().connected = false;
        link.cleanup(&lease).unwrap();
        assert_eq!(link.ops().last().unwrap(), "disconnect_cleanup");
        let ops = link.ops();
        link.cleanup(&lease).unwrap();
        assert_eq!(link.ops(), ops);
    }

    #[test]
    fn dry_run_link_records_but_touches_nothing() {
        let mut link = DryRunHelperLink::new();
        link.available().unwrap();
        let lease = link.apply(&spec()).unwrap();
        assert!(lease.dry_run);
        assert!(link.dry_run());
        link.cleanup(&lease).unwrap();
        let ops = link.ops();
        assert!(ops.iter().any(|op| op.starts_with("dry_run:add_routes")));
        assert!(ops
            .iter()
            .any(|op| op.starts_with("dry_run:set_tun_address")));
        assert!(ops.iter().any(|op| op.starts_with("dry_run:remove_routes")));
    }

    #[test]
    fn unavailable_link_reports_structured_error() {
        let mut link = UnavailableHelperLink::new();
        assert_eq!(link.available().unwrap_err().code, E_TUN_HELPER_UNAVAILABLE);
        assert_eq!(
            link.apply(&spec()).unwrap_err().code,
            E_TUN_HELPER_UNAVAILABLE
        );
    }

    #[test]
    fn lease_summary_is_redacted() {
        let lease = TunLease::new("s1", spec(), false);
        let summary = lease.summary();
        assert!(summary.contains("routes=1 v4=1 v6=0"));
        assert!(!summary.contains("198.18"), "next hops must not be logged");
        assert!(summary.contains("dry_run=false"));
    }

    #[test]
    fn lease_roundtrips_through_json() {
        let lease = TunLease::new("s1", spec(), false);
        let json = serde_json::to_string(&lease).unwrap();
        let back: TunLease = serde_json::from_str(&json).unwrap();
        assert_eq!(lease, back);
    }

    #[test]
    fn helper_error_mapping_is_stable() {
        assert_eq!(
            map_helper_error(HelperError::Unauthorized {
                detail: "no".into()
            })
            .code,
            E_TUN_HELPER_UNAVAILABLE
        );
        assert_eq!(
            map_helper_error(HelperError::NotAllowlisted {
                detail: "no".into()
            })
            .code,
            E_TUN_HELPER_UNAVAILABLE
        );
        assert!(map_helper_error(HelperError::Timeout { timeout_ms: 5 }).retryable);
        assert_eq!(
            map_helper_error(HelperError::Backend {
                detail: "boom".into()
            })
            .code,
            codes::UNAVAILABLE
        );
    }

    #[test]
    fn fake_cleanup_failure_keeps_resources_for_retry() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::CleanupFails);
        let lease = link.apply(&spec()).unwrap();
        let error = link.cleanup(&lease).unwrap_err();
        assert_eq!(error.code, codes::UNAVAILABLE);
        assert_eq!(link.cleanup_attempts(), 1);
        assert_eq!(link.cleanup_count(), 0, "no confirmed release happened");
        assert!(link.has_tun(), "resources stay owned until confirmed");
        assert_eq!(link.route_count(), 1);
        // Recovery: the fault clears and the same lease cleans normally.
        link.set_fault(None);
        link.cleanup(&lease).unwrap();
        assert_eq!(link.cleanup_count(), 1);
        assert!(!link.has_tun());
    }

    #[test]
    fn fake_cleanup_unavailable_is_retryable_and_keeps_resources() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::CleanupUnavailable);
        let lease = link.apply(&spec()).unwrap();
        let error = link.cleanup(&lease).unwrap_err();
        assert_eq!(error.code, E_TUN_HELPER_UNAVAILABLE);
        assert!(error.retryable);
        assert!(link.has_tun());
        assert_eq!(link.cleanup_attempts(), 1);
        assert_eq!(link.cleanup_count(), 0);
    }

    #[test]
    fn route_entries_from_lease_keep_family() {
        let lease = TunLease::new("s1", spec(), false);
        let entries = lease.spec.to_route_entries().unwrap();
        assert_eq!(entries[0].family, AddressFamily::V4);
    }

    // -- SP-09/SP-10 helper protocol v2 lease ops (fake link) ----------------

    fn elevated_spec() -> ElevatedCoreSpec {
        ElevatedCoreSpec {
            core: "sing-box".into(),
            exe_path: r"C:\run\s1\processes\pre-socks\sing-box.exe".into(),
            args: vec!["run".into(), "-c".into(), "config.json".into()],
            run_dir: r"C:\run\s1\processes\pre-socks".into(),
        }
    }

    #[test]
    fn fake_renew_extends_the_lease() {
        use ipc_contract::LeaseState;

        let mut link = FakeHelperLink::new();
        let (handle, _) = link.run_core(&elevated_spec()).expect("run_core");
        let before = link.lease_expires_at(handle).expect("lease tracked");
        let status = link.get_lease_status(handle).expect("status");
        assert_eq!(status.state, LeaseState::Active);
        assert_eq!(status.pid, Some(4242));

        let expires = link.renew_lease(handle).expect("renew");
        assert!(expires >= before, "renew must extend the lease");
        let status = link.get_lease_status(handle).expect("status");
        assert_eq!(status.state, LeaseState::Active);
        assert_eq!(status.expires_at_ms, expires);
        assert!(link.ops().iter().any(|op| op == "renew_lease handle=1"));
    }

    #[test]
    fn fake_renew_rejects_unknown_and_zero_handles() {
        let mut link = FakeHelperLink::new();
        let error = link.renew_lease(99).expect_err("unknown handle");
        assert_eq!(error.code, domain::codes::NOT_FOUND);
        let error = link.renew_lease(0).expect_err("zero handle");
        assert_eq!(error.code, domain::codes::INVALID_ARGUMENT);
        let error = link.get_lease_status(99).expect_err("unknown status");
        assert_eq!(error.code, domain::codes::NOT_FOUND);
    }

    #[test]
    fn fake_released_handle_reads_released_and_refuses_renew() {
        use ipc_contract::LeaseState;

        let mut link = FakeHelperLink::new();
        let (handle, _) = link.run_core(&elevated_spec()).expect("run_core");
        link.stop_core(handle).expect("stop");
        let status = link.get_lease_status(handle).expect("status");
        assert_eq!(status.state, LeaseState::Released);
        let error = link.renew_lease(handle).expect_err("released");
        assert_eq!(error.code, domain::codes::NOT_FOUND);
    }

    #[test]
    fn fake_poll_drains_once_and_status_reflects_it() {
        use ipc_contract::LeaseState;

        let mut link = FakeHelperLink::new();
        let (handle, pid) = link.run_core(&elevated_spec()).expect("run_core");
        link.inject_core_exit(handle, pid, Some(1));

        let exits = link.poll_core_exits(&[handle]).expect("poll");
        assert_eq!(exits.len(), 1);
        assert_eq!(exits[0].handle, handle);
        assert_eq!(exits[0].exit_code, Some(1));

        let again = link.poll_core_exits(&[handle]).expect("second poll");
        assert!(again.is_empty(), "drain-once: no repeat observation");
        let status = link.get_lease_status(handle).expect("status");
        assert_eq!(
            status.state,
            LeaseState::Exited,
            "a drained exit stays visible to status"
        );
    }

    #[test]
    fn fake_poll_ignores_unlisted_handles_and_validates_bounds() {
        let mut link = FakeHelperLink::new();
        let (handle, pid) = link.run_core(&elevated_spec()).expect("run_core");
        // Exit staged for a handle the caller does not own: polling the
        // owned handle must not surface it, and the unknown handle is not
        // an error by itself (the server only answers for listed handles).
        link.inject_core_exit(999, pid, Some(1));
        let exits = link.poll_core_exits(&[handle]).expect("poll");
        assert!(
            exits.is_empty(),
            "an unlisted handle must not fabricate an exit"
        );

        assert!(link.poll_core_exits(&[]).is_err(), "empty list rejected");
        assert!(link.poll_core_exits(&[0]).is_err(), "zero handle rejected");
        assert!(
            link.poll_core_exits(&[handle, handle]).is_err(),
            "duplicates rejected"
        );
        let many = (1..=ipc_contract::HELPER_MAX_POLL_HANDLES as u64 + 1).collect::<Vec<_>>();
        assert!(
            link.poll_core_exits(&many).is_err(),
            "oversized list rejected"
        );
    }

    #[test]
    fn fake_lease_ops_fail_when_the_link_is_down() {
        let mut link = FakeHelperLink::with_fault(FakeHelperFault::Timeout);
        assert!(link.renew_lease(1).expect_err("renew").retryable);
        assert!(link.get_lease_status(1).expect_err("status").retryable);
        assert!(link.poll_core_exits(&[1]).expect_err("poll").retryable);
    }
}
