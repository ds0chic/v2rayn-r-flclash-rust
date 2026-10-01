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

/// The finite privileged operation set the server may invoke.
pub trait HelperBackend: Send + Sync {
    /// Whether the current process token is elevated.
    fn is_elevated(&self) -> bool;
    /// Add route entries; returns the number applied.
    fn add_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError>;
    /// Remove route entries; returns the number removed.
    fn remove_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError>;
    /// Assign addresses (and optional MTU) to a TUN adapter.
    fn set_tun_address(&self, config: &TunAddressConfig) -> Result<(), HelperError>;
    /// Remove addresses previously assigned to an interface index.
    fn reset_tun_address(&self, interface_index: u32) -> Result<(), HelperError>;
    /// Start an allow-listed core elevated.
    fn run_elevated_core(&self, spec: &ElevatedCoreSpec) -> Result<StartedCore, HelperError>;
    /// Stop a previously started core; idempotent for known handles.
    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError>;
    /// Release helper resources.
    fn shutdown(&self) -> Result<(), HelperError>;
}

/// Operation identities used to inject fake failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    tun: BTreeSet<u32>,
    stopped: Vec<u64>,
    next_handle: u64,
    next_pid: u32,
    fail_op: Option<FakeOp>,
    fail_error: Option<HelperError>,
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

    fn check_fail(state: &FakeState, op: FakeOp) -> Result<(), HelperError> {
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
        Self::check_fail(&state, FakeOp::AddRoutes)?;
        state.calls.push(FakeCall::AddRoutes(entries.to_vec()));
        Ok(entries.len() as u32)
    }

    fn remove_routes(&self, entries: &[RouteEntry]) -> Result<u32, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::RemoveRoutes)?;
        state.calls.push(FakeCall::RemoveRoutes(entries.to_vec()));
        Ok(entries.len() as u32)
    }

    fn set_tun_address(&self, config: &TunAddressConfig) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::SetTunAddress)?;
        state.tun.insert(config.interface_index);
        state.calls.push(FakeCall::SetTunAddress(config.clone()));
        Ok(())
    }

    fn reset_tun_address(&self, interface_index: u32) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::ResetTunAddress)?;
        state.tun.remove(&interface_index);
        state.calls.push(FakeCall::ResetTunAddress(interface_index));
        Ok(())
    }

    fn run_elevated_core(&self, spec: &ElevatedCoreSpec) -> Result<StartedCore, HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::RunElevatedCore)?;
        let handle = state.next_handle;
        let pid = state.next_pid;
        state.next_handle += 1;
        state.next_pid += 1;
        state.known.insert(handle);
        state.running.insert(handle);
        state.calls.push(FakeCall::RunElevatedCore(spec.clone()));
        Ok(StartedCore { handle, pid })
    }

    fn stop_elevated_core(&self, handle: u64) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::StopElevatedCore)?;
        if !state.known.contains(&handle) {
            return Err(HelperError::UnknownHandle { handle });
        }
        state.running.remove(&handle);
        state.stopped.push(handle);
        state.calls.push(FakeCall::StopElevatedCore(handle));
        Ok(())
    }

    fn shutdown(&self) -> Result<(), HelperError> {
        let mut state = self.lock();
        Self::check_fail(&state, FakeOp::Shutdown)?;
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
