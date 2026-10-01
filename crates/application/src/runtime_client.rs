//! The runtime client boundary.
//!
//! T02 defines only the trait; T03 implements it with the real IPC client to
//! net-host. Keeping the trait here lets the in-memory `AppEngine` be tested
//! without any process or socket.

use domain::{AppliedRevision, CancelOutcome, DomainError, JobId, RuntimePlan, RuntimeState};

/// A runtime snapshot as reported by net-host, decoupled from the ipc crate so
/// `application` does not depend on the wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub state: RuntimeState,
    pub applied_revision: AppliedRevision,
    pub host_alive: bool,
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            state: RuntimeState::Stopped,
            applied_revision: AppliedRevision::ZERO,
            host_alive: false,
        }
    }
}

/// Outcome of an `apply` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// Plan accepted; progress arrives via the event stream.
    Accepted { operation_id: String },
    /// The runtime is not available (net-host down).
    Unavailable,
}

/// net-host client contract implemented by T03.
pub trait RuntimeClient: Send + Sync {
    /// Read the current runtime snapshot.
    fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError>;

    /// Submit an immutable plan. Results are delivered via the event stream.
    fn apply(&self, plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError>;

    /// Stop the managed runtime. Idempotent.
    fn stop(&self) -> Result<(), DomainError>;

    /// Request cancellation of a running operation. Idempotent.
    fn cancel(&self, job_id: &JobId) -> Result<CancelOutcome, DomainError>;
}

/// A no-op runtime client used by the T02 in-memory engine and tests.
///
/// It never touches a real process; `apply` records the last plan so tests can
/// assert what was submitted.
#[derive(Debug, Default)]
pub struct NullRuntimeClient {
    last_plan: std::sync::Mutex<Option<RuntimePlan>>,
    state: std::sync::Mutex<RuntimeState>,
    applied: std::sync::Mutex<AppliedRevision>,
}

impl NullRuntimeClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last_plan(&self) -> Option<RuntimePlan> {
        self.last_plan.lock().ok().and_then(|p| p.clone())
    }

    /// Simulate a successful application (test helper).
    pub fn mark_running(&self, revision: AppliedRevision) {
        if let Ok(mut s) = self.state.lock() {
            *s = RuntimeState::Running;
        }
        if let Ok(mut a) = self.applied.lock() {
            *a = revision;
        }
    }
}

impl RuntimeClient for NullRuntimeClient {
    fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
        Ok(RuntimeSnapshot {
            state: self
                .state
                .lock()
                .map(|s| *s)
                .unwrap_or(RuntimeState::Stopped),
            applied_revision: self
                .applied
                .lock()
                .map(|a| *a)
                .unwrap_or(AppliedRevision::ZERO),
            host_alive: true,
        })
    }

    fn apply(&self, plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError> {
        if let Ok(mut p) = self.last_plan.lock() {
            *p = Some(plan.clone());
        }
        Ok(ApplyOutcome::Accepted {
            operation_id: format!("op-{}", plan.plan_id),
        })
    }

    fn stop(&self) -> Result<(), DomainError> {
        if let Ok(mut s) = self.state.lock() {
            *s = RuntimeState::Stopped;
        }
        Ok(())
    }

    fn cancel(&self, _job_id: &JobId) -> Result<CancelOutcome, DomainError> {
        Ok(CancelOutcome::Requested)
    }
}
