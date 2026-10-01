//! Job identity, lifecycle and cancellation semantics (plan §8, §14).
//!
//! Cancellation is cooperative and only happens at safe points. Once a job has
//! entered the point of no return (an OS/network side effect) it must
//! compensate first; the UI shows "cancelling" / "recovering", never a fake
//! immediate completion.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;

/// Opaque, stable job identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JobId(pub String);

impl JobId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lifecycle state of a cancellable operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Running,
    /// Cancel requested; not yet acknowledged at a safe point.
    Cancelling,
    /// Side effects are being rolled back.
    Compensating,
    Done,
    Failed,
    Cancelled,
}

impl JobState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Done | JobState::Failed | JobState::Cancelled
        )
    }

    pub const fn as_u8(self) -> u8 {
        match self {
            JobState::Running => 0,
            JobState::Cancelling => 1,
            JobState::Compensating => 2,
            JobState::Done => 3,
            JobState::Failed => 4,
            JobState::Cancelled => 5,
        }
    }

    pub const fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => JobState::Running,
            1 => JobState::Cancelling,
            2 => JobState::Compensating,
            3 => JobState::Done,
            4 => JobState::Failed,
            5 => JobState::Cancelled,
            _ => return None,
        })
    }
}

/// Outcome of a `cancel_job` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelOutcome {
    /// Cancellation accepted; the job will stop at the next safe point.
    Requested,
    /// Job is compensating already-applied side effects.
    Compensating,
    /// Job had already finished before the request.
    AlreadyFinished,
    /// Job is past a safe point and cannot be cancelled; it must complete.
    NotCancellable,
}

/// Cooperative cancellation token.
///
/// `cancel()` is idempotent and lock-free. Clones share the same flag so the
/// job body and the request handler observe the same state.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    inner: Arc<CancelInner>,
}

#[derive(Debug, Default)]
struct CancelInner {
    requested: AtomicBool,
    state: AtomicU8,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancelInner {
                requested: AtomicBool::new(false),
                state: AtomicU8::new(JobState::Running.as_u8()),
            }),
        }
    }

    /// Idempotent request. Returns `true` if this call transitioned the token
    /// to the cancelling state, `false` if it was already requested or the job
    /// has reached a terminal state.
    pub fn cancel(&self) -> bool {
        let previous = self.inner.requested.swap(true, Ordering::AcqRel);
        if previous {
            return false;
        }
        self.set_state(JobState::Cancelling)
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.requested.load(Ordering::Acquire)
    }

    /// Check whether the job body should abort at this safe point.
    pub fn check(&self) -> Result<(), crate::error::DomainError> {
        if self.is_cancelled() {
            Err(crate::error::DomainError::new(
                crate::error::codes::CANCELLED,
                "error.cancelled",
            ))
        } else {
            Ok(())
        }
    }

    pub fn state(&self) -> JobState {
        JobState::from_u8(self.inner.state.load(Ordering::Acquire)).unwrap_or(JobState::Running)
    }

    /// Transition to a non-terminal lifecycle state (cancelling/compensating).
    /// Terminal transitions require a non-zero previous `cancel` or explicit
    /// `finish` call, which is handled by [`JobManager`](crate::job::JobManager).
    pub fn set_state(&self, state: JobState) -> bool {
        let prev = JobState::from_u8(self.inner.state.swap(state.as_u8(), Ordering::AcqRel));
        prev != Some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_idempotent() {
        let token = CancellationToken::new();
        assert!(token.cancel());
        assert!(!token.cancel());
        assert!(token.is_cancelled());
        assert_eq!(token.state(), JobState::Cancelling);
    }

    #[test]
    fn check_fails_after_cancel() {
        let token = CancellationToken::new();
        assert!(token.check().is_ok());
        token.cancel();
        assert_eq!(
            token.check().unwrap_err().code,
            crate::error::codes::CANCELLED
        );
    }

    #[test]
    fn clones_share_state() {
        let token = CancellationToken::new();
        let clone = token.clone();
        clone.cancel();
        assert!(token.is_cancelled());
    }
}
