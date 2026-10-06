//! Backend abstraction for the helper.
//!
//! [`HelperBackend`] is implemented by the real [`crate::windows::WindowsBackend`]
//! (compiled, not executed here) and by [`FakeBackend`], which records every
//! call in memory and returns configurable results for tests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use ipc_contract::{ElevatedCoreSpec, HelperError, RouteEntry, TunAddressConfig};

/// Handle plus pid returned when an elevated core starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedCore {
    pub handle: u64,
    pub pid: u32,
}

/// One observed elevated-core exit (SP-06). The observation is
/// handle-scoped: the helper owns the process handle, so a recycled PID can
/// never cause a miskill and polling never kills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreExit {
    pub handle: u64,
    pub pid: u32,
    pub exit_code: Option<i32>,
}

/// Outcome of a route-removal backend call (SP-08).
///
/// `AlreadyGone` means the OS confirmed the entries were already absent
/// (`ERROR_NOT_FOUND` on Windows); the desired end state holds, so the
/// caller releases the journal record instead of retaining a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteRemovalOutcome {
    Removed(u32),
    AlreadyGone,
}

/// Outcome of a TUN-address reset backend call (SP-08). Same contract as
/// [`RouteRemovalOutcome`]: `AlreadyGone` is success, not failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunResetOutcome {
    Reset,
    AlreadyGone,
}

/// The finite privileged operation set the server may invoke.
pub trait HelperBackend: Send + Sync {
    /// Whether the current process token is elevated.
    fn is_elevated(&self) -> bool;
    /// Add route entries; returns the number applied.
    fn add_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError>;
    /// Remove route entries. `AlreadyGone` reports the OS confirmed the
    /// entries were already absent (idempotent success).
    fn remove_routes(&self, entries: &[RouteEntry]) -> Result<RouteRemovalOutcome, HelperError>;
    /// Assign addresses (and optional MTU) to a TUN adapter.
    fn set_tun_address(&self, config: &TunAddressConfig) -> Result<(), HelperError>;
    /// Remove addresses previously assigned to an interface index.
    /// `AlreadyGone` reports the OS confirmed the addresses were already
    /// absent (idempotent success).
    fn reset_tun_address(&self, interface_index: u32) -> Result<TunResetOutcome, HelperError>;
    /// Start an allow-listed core elevated.
    fn run_elevated_core(&self, spec: &ElevatedCoreSpec) -> Result<StartedCore, HelperError>;
    /// Stop a previously started core; idempotent for known handles.
    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError>;
    /// Non-blocking poll of owned elevated cores that exited since the last
    /// poll. Never kills; removal stays with stop/disconnect cleanup.
    fn poll_core_exits(&self) -> Vec<CoreExit>;
    /// Release helper resources.
    fn shutdown(&self) -> Result<(), HelperError>;
}

/// Operation identities used to inject fake failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FakeOp {
    AddRoutes,
    RemoveRoutes,
    SetTunAddress,
    ResetTunAddress,
    RunElevatedCore,
    StopElevatedCore,
    Shutdown,
}

/// A recorded backend call.
#[derive(Debug, Clone, PartialEq)]
pub enum FakeCall {
    AddRoutes(Vec<RouteEntry>),
    RemoveRoutes(Vec<RouteEntry>),
    SetTunAddress(TunAddressConfig),
    ResetTunAddress(u32),
    RunElevatedCore(ElevatedCoreSpec),
    StopElevatedCore(u64),
    Shutdown,
}

impl FakeCall {
    /// Operation identity of this call, for assertions.
    pub fn op(&self) -> FakeOp {
        match self {
            FakeCall::AddRoutes(_) => FakeOp::AddRoutes,
            FakeCall::RemoveRoutes(_) => FakeOp::RemoveRoutes,
            FakeCall::SetTunAddress(_) => FakeOp::SetTunAddress,
            FakeCall::ResetTunAddress(_) => FakeOp::ResetTunAddress,
            FakeCall::RunElevatedCore(_) => FakeOp::RunElevatedCore,
            FakeCall::StopElevatedCore(_) => FakeOp::StopElevatedCore,
            FakeCall::Shutdown => FakeOp::Shutdown,
        }
    }
}

#[derive(Debug, Default)]
struct FakeState {
    calls: Vec<FakeCall>,
    running: BTreeSet<u64>,
    known: BTreeSet<u64>,
    /// Fault-injected exits: handle -> wait code, drained by the next poll.
    exited: BTreeMap<u64, Option<i32>>,
    /// Owned identity per handle, so a poll keeps the spawn-time pid.
    pids: BTreeMap<u64, u32>,
    tun: BTreeSet<u32>,
    stopped: Vec<u64>,
    next_handle: u64,
    next_pid: u32,
    fail_op: Option<FakeOp>,
    fail_error: Option<HelperError>,
    /// SP-08: operations that report `AlreadyGone` instead of success, so
    /// dispatch/cleanup idempotency is exercised without the OS.
    already_gone: BTreeSet<FakeOp>,
    /// Attempts per operation, including fault-injected failures (SP-08):
    /// proves a retry really re-attempted the retained resource.
    attempts: BTreeMap<FakeOp, u32>,
    elevated: bool,
    shutdown_count: u32,
}

/// In-memory backend that records every call and returns configurable results.
#[derive(Debug)]
pub struct FakeBackend {
    inner: Mutex<FakeState>,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FakeState {
                next_handle: 1000,
                next_pid: 2000,
                ..FakeState::default()
            }),
        }
    }

    /// Mark the fake as elevated (or not) for `is_elevated`.
    pub fn elevated(mut self, elevated: bool) -> Self {
        self.inner.get_mut().expect("fresh mutex").elevated = elevated;
        self
    }

    /// Inject a failure for a specific operation.
    pub fn fail_on(mut self, op: FakeOp, error: HelperError) -> Self {
        let state = self.inner.get_mut().expect("fresh mutex");
        state.fail_op = Some(op);
        state.fail_error = Some(error);
        self
    }

    /// Clear an injected failure so a retained resource can be retried
    /// to success (SP-08).
    pub fn clear_failure(&self) {
        let mut state = self.lock();
        state.fail_op = None;
        state.fail_error = None;
    }

    /// Report `AlreadyGone` for one operation (SP-08 idempotency simulation).
    /// Builder form, mirroring [`Self::fail_on`].
    pub fn already_gone_on(mut self, op: FakeOp) -> Self {
        self.inner
            .get_mut()
            .expect("fresh mutex")
            .already_gone
            .insert(op);
        self
    }

    /// Switch `AlreadyGone` simulation mid-test.
    pub fn set_already_gone(&self, op: FakeOp, gone: bool) {
        let mut state = self.lock();
        if gone {
            state.already_gone.insert(op);
        } else {
            state.already_gone.remove(&op);
        }
    }

    /// Clear every `AlreadyGone` simulation.
    pub fn clear_already_gone(&self) {
        self.lock().already_gone.clear();
    }

    /// Attempts for one operation, including failed ones.
    pub fn attempt_count(&self, op: FakeOp) -> u32 {
        self.lock().attempts.get(&op).copied().unwrap_or(0)
    }

    /// All recorded calls, in order.
    pub fn calls(&self) -> Vec<FakeCall> {
        self.lock().calls.clone()
    }

    /// Number of recorded calls.
    pub fn call_count(&self) -> usize {
        self.lock().calls.len()
    }

    pub fn clear_calls(&self) {
        self.lock().calls.clear();
    }

    /// Whether a core handle is currently running.
    pub fn is_running(&self, handle: u64) -> bool {
        self.lock().running.contains(&handle)
    }

    /// Fault injection (SP-06): pretend the owned elevated core exited with
    /// `exit_code`. Unknown handles are ignored, never recorded.
    pub fn inject_exit(&self, handle: u64, exit_code: Option<i32>) {
        let mut state = self.lock();
        if state.known.contains(&handle) && state.running.contains(&handle) {
            state.exited.insert(handle, exit_code);
        }
    }

    /// Handles for which stop was called, in order.
    pub fn stopped_handles(&self) -> Vec<u64> {
        self.lock().stopped.clone()
    }

    /// Interface indices currently holding a fake TUN address.
    pub fn tun_interfaces(&self) -> Vec<u32> {
        self.lock().tun.iter().copied().collect()
    }

    pub fn shutdown_count(&self) -> u32 {
        self.lock().shutdown_count
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
        self.inner.lock().expect("fake backend poisoned")
    }

    fn check_fail(state: &mut FakeState, op: FakeOp) -> Result<(), HelperError> {
        *state.attempts.entry(op).or_insert(0) += 1;
        if state.fail_op == Some(op) {
            return Err(state
                .fail_error
                .clone()
                .unwrap_or_else(|| HelperError::Backend {
                    detail: "injected failure".to_string(),
                }));
        }
        Ok(())
    }
}

impl HelperBackend for FakeBackend {
    fn is_elevated(&self) -> bool {
        self.lock().elevated
    }

    fn add_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::AddRoutes)?;
        state.calls.push(FakeCall::AddRoutes(entries.to_vec()));
        Ok(entries.len() as u32)
    }

    fn remove_routes(&self, entries: &[RouteEntry]) -> Result<RouteRemovalOutcome, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::RemoveRoutes)?;
        state.calls.push(FakeCall::RemoveRoutes(entries.to_vec()));
        if state.already_gone.contains(&FakeOp::RemoveRoutes) {
            return Ok(RouteRemovalOutcome::AlreadyGone);
        }
        Ok(RouteRemovalOutcome::Removed(entries.len() as u32))
    }

    fn set_tun_address(&self, config: &TunAddressConfig) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::SetTunAddress)?;
        state.tun.insert(config.interface_index);
        state.calls.push(FakeCall::SetTunAddress(config.clone()));
        Ok(())
    }

    fn reset_tun_address(&self, interface_index: u32) -> Result<TunResetOutcome, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::ResetTunAddress)?;
        state.tun.remove(&interface_index);
        state.calls.push(FakeCall::ResetTunAddress(interface_index));
        if state.already_gone.contains(&FakeOp::ResetTunAddress) {
            return Ok(TunResetOutcome::AlreadyGone);
        }
        Ok(TunResetOutcome::Reset)
    }

    fn run_elevated_core(&self, spec: &ElevatedCoreSpec) -> Result<StartedCore, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::RunElevatedCore)?;
        let handle = state.next_handle;
        let pid = state.next_pid;
        state.next_handle += 1;
        state.next_pid += 1;
        state.known.insert(handle);
        state.running.insert(handle);
        state.pids.insert(handle, pid);
        state.calls.push(FakeCall::RunElevatedCore(spec.clone()));
        Ok(StartedCore { handle, pid })
    }

    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::StopElevatedCore)?;
        if !state.known.contains(&handle) {
            return Err(HelperError::UnknownHandle { handle });
        }
        state.running.remove(&handle);
        state.stopped.push(handle);
        state.calls.push(FakeCall::StopElevatedCore(handle));
        Ok(())
    }

    fn poll_core_exits(&self) -> Vec<CoreExit> {
        let mut state = self.lock();
        // Drain exactly once: an observed exit is never reported twice, and
        // polling never touches unknown handles.
        let handles: Vec<u64> = state.exited.keys().copied().collect();
        let mut exits = Vec::new();
        for handle in handles {
            let code = state
                .exited
                .remove(&handle)
                .expect("polled handle recorded");
            if !state.running.remove(&handle) {
                continue;
            }
            exits.push(CoreExit {
                handle,
                pid: state.pids.get(&handle).copied().unwrap_or(0),
                exit_code: code,
            });
        }
        exits
    }

    fn shutdown(&self) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&mut state, FakeOp::Shutdown)?;
        state.shutdown_count += 1;
        state.calls.push(FakeCall::Shutdown);
        Ok(())
    }
}

/// Monotonic handle allocator shared by real and fake backends.
#[derive(Debug, Default)]
pub struct HandleAllocator {
    next: AtomicU64,
}

impl HandleAllocator {
    pub fn new(start: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
        }
    }

    pub fn allocate(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst)
    }
}

/// Small helper used by the real backend to track applied TUN configs.
pub type TunRegistry = BTreeMap<u32, TunAddressConfig>;

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ElevatedCoreSpec {
        ElevatedCoreSpec {
            core: "sing-box".into(),
            exe_path: r"C:\run\s1\processes\pre-socks\sing-box.exe".into(),
            args: vec!["run".into(), "-c".into(), "config.json".into()],
            run_dir: r"C:\run\s1\processes\pre-socks".into(),
        }
    }

    #[test]
    fn sp06_poll_reports_an_injected_exit_exactly_once() {
        let backend = FakeBackend::new();
        let started = backend.run_elevated_core(&spec()).expect("starts");
        assert!(backend.poll_core_exits().is_empty(), "live core is quiet");
        backend.inject_exit(started.handle, Some(1));
        let exits = backend.poll_core_exits();
        assert_eq!(exits.len(), 1, "one owned exit observed");
        assert_eq!(exits[0].handle, started.handle);
        assert_eq!(exits[0].pid, started.pid, "exit keeps the owned identity");
        assert_eq!(exits[0].exit_code, Some(1));
        assert!(
            backend.poll_core_exits().is_empty(),
            "an observed exit is never reported twice"
        );
    }

    #[test]
    fn sp06_stop_after_poll_stays_idempotent_without_miskill() {
        let backend = FakeBackend::new();
        let started = backend.run_elevated_core(&spec()).expect("starts");
        backend.inject_exit(started.handle, Some(1));
        assert_eq!(backend.poll_core_exits().len(), 1);
        backend
            .stop_elevated_core(started.handle)
            .expect("stop of a known exited core succeeds");
        assert!(!backend.is_running(started.handle));
    }

    #[test]
    fn sp06_unknown_handles_are_never_reported() {
        let backend = FakeBackend::new();
        backend.inject_exit(999_999, Some(1));
        assert!(
            backend.poll_core_exits().is_empty(),
            "foreign identities must not surface as owned exits"
        );
        assert!(backend.stop_elevated_core(999_999).is_err());
    }
}
