//! Serialized job manager.
//!
//! Runtime-affecting operations are serialized (plan §8: "运行相关动作串行收敛").
//! A new operation supersedes a not-yet-started one; the manager exposes a
//! `CancellationToken` per job and implements idempotent `cancel`.
//!
//! This is an in-memory skeleton: it does not spawn threads. Callers drive the
//! job body and report transitions back through [`JobManager::finish`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use domain::{CancelOutcome, CancellationToken, DomainError, JobId, JobState};
use serde::{Deserialize, Serialize};

/// Public view of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobView {
    pub job_id: JobId,
    pub kind: String,
    pub state: JobState,
    /// 0..=100 when known; `None` when the stage is unknown.
    pub percent: Option<u8>,
    pub stage_key: Option<String>,
    pub error: Option<DomainError>,
}

struct JobEntry {
    kind: String,
    state: JobState,
    percent: Option<u8>,
    stage_key: Option<String>,
    error: Option<DomainError>,
    token: CancellationToken,
    /// Operations already past the cancellation safe point.
    past_safe_point: bool,
}

/// Serialized job manager with idempotent cancellation.
#[derive(Clone)]
pub struct JobManager {
    inner: Arc<JobManagerInner>,
}

struct JobManagerInner {
    seq: AtomicU64,
    jobs: Mutex<HashMap<String, JobEntry>>,
    order: Mutex<Vec<String>>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(JobManagerInner {
                seq: AtomicU64::new(0),
                jobs: Mutex::new(HashMap::new()),
                order: Mutex::new(Vec::new()),
            }),
        }
    }

    fn next_id(&self) -> JobId {
        let n = self.inner.seq.fetch_add(1, Ordering::AcqRel) + 1;
        JobId::new(format!("job-{n:08}"))
    }

    /// Start a new job in the `Running` state.
    pub fn start(&self, kind: impl Into<String>) -> JobView {
        let job_id = self.next_id();
        let entry = JobEntry {
            kind: kind.into(),
            state: JobState::Running,
            percent: None,
            stage_key: None,
            error: None,
            token: CancellationToken::new(),
            past_safe_point: false,
        };
        let kind = entry.kind.clone();
        self.inner
            .jobs
            .lock()
            .expect("job mutex poisoned")
            .insert(job_id.0.clone(), entry);
        self.inner
            .order
            .lock()
            .expect("order mutex poisoned")
            .push(job_id.0.clone());
        JobView {
            job_id,
            kind,
            state: JobState::Running,
            percent: None,
            stage_key: None,
            error: None,
        }
    }

    pub fn token(&self, job_id: &JobId) -> Option<CancellationToken> {
        self.inner
            .jobs
            .lock()
            .ok()
            .and_then(|jobs| jobs.get(job_id.as_str()).map(|e| e.token.clone()))
    }

    /// Idempotent cancel. Returns the outcome describing what will happen.
    pub fn cancel(&self, job_id: &JobId) -> CancelOutcome {
        let mut jobs = match self.inner.jobs.lock() {
            Ok(j) => j,
            Err(_) => {
                return CancelOutcome::AlreadyFinished;
            }
        };
        let Some(entry) = jobs.get_mut(job_id.as_str()) else {
            return CancelOutcome::AlreadyFinished;
        };
        if entry.state.is_terminal() {
            return CancelOutcome::AlreadyFinished;
        }
        if entry.past_safe_point {
            // Cannot cancel; mark the state but don't touch the token so the
            // body keeps running to completion.
            return CancelOutcome::NotCancellable;
        }
        let first = entry.token.cancel();
        if entry.state == JobState::Cancelling {
            // already requested
            return CancelOutcome::Compensating;
        }
        entry.state = JobState::Cancelling;
        if first {
            CancelOutcome::Requested
        } else {
            CancelOutcome::Compensating
        }
    }

    /// Mark a job as having passed its cancellation safe point.
    pub fn mark_past_safe_point(&self, job_id: &JobId) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if let Some(entry) = jobs.get_mut(job_id.as_str()) {
                entry.past_safe_point = true;
            }
        }
    }

    /// Update progress stage / percent (percent is never faked).
    pub fn progress(
        &self,
        job_id: &JobId,
        percent: Option<u8>,
        stage_key: Option<String>,
    ) -> Option<JobView> {
        let mut jobs = self.inner.jobs.lock().ok()?;
        let entry = jobs.get_mut(job_id.as_str())?;
        if entry.state.is_terminal() {
            return None;
        }
        entry.percent = percent.map(|p| p.min(100));
        entry.stage_key = stage_key;
        Some(JobView {
            job_id: job_id.clone(),
            kind: entry.kind.clone(),
            state: entry.state,
            percent: entry.percent,
            stage_key: entry.stage_key.clone(),
            error: entry.error.clone(),
        })
    }

    /// Transition a job to a terminal state.
    pub fn finish(&self, job_id: &JobId, state: JobState, error: Option<DomainError>) {
        if let Ok(mut jobs) = self.inner.jobs.lock() {
            if let Some(entry) = jobs.get_mut(job_id.as_str()) {
                entry.state = state;
                entry.error = error;
            }
        }
    }

    pub fn get(&self, job_id: &JobId) -> Option<JobView> {
        let jobs = self.inner.jobs.lock().ok()?;
        let entry = jobs.get(job_id.as_str())?;
        Some(JobView {
            job_id: job_id.clone(),
            kind: entry.kind.clone(),
            state: entry.state,
            percent: entry.percent,
            stage_key: entry.stage_key.clone(),
            error: entry.error.clone(),
        })
    }

    /// All non-terminal jobs, in start order.
    pub fn active(&self) -> Vec<JobView> {
        let jobs = match self.inner.jobs.lock() {
            Ok(j) => j,
            Err(_) => return Vec::new(),
        };
        let order = match self.inner.order.lock() {
            Ok(o) => o,
            Err(_) => return Vec::new(),
        };
        order
            .iter()
            .filter_map(|id| jobs.get(id).map(|e| (id, e)))
            .filter(|(_, e)| !e.state.is_terminal())
            .map(|(id, e)| JobView {
                job_id: JobId::new(id.clone()),
                kind: e.kind.clone(),
                state: e.state,
                percent: e.percent,
                stage_key: e.stage_key.clone(),
                error: e.error.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_idempotent_from_manager() {
        let mgr = JobManager::new();
        let job = mgr.start("apply_runtime");
        assert_eq!(mgr.cancel(&job.job_id), CancelOutcome::Requested);
        assert_eq!(mgr.cancel(&job.job_id), CancelOutcome::Compensating);
        assert_eq!(mgr.cancel(&job.job_id), CancelOutcome::Compensating);
    }

    #[test]
    fn finished_job_reports_already_finished() {
        let mgr = JobManager::new();
        let job = mgr.start("test");
        mgr.finish(&job.job_id, JobState::Done, None);
        assert_eq!(mgr.cancel(&job.job_id), CancelOutcome::AlreadyFinished);
    }

    #[test]
    fn past_safe_point_is_not_cancellable() {
        let mgr = JobManager::new();
        let job = mgr.start("apply_runtime");
        mgr.mark_past_safe_point(&job.job_id);
        assert_eq!(mgr.cancel(&job.job_id), CancelOutcome::NotCancellable);
    }

    #[test]
    fn token_is_shared_with_body() {
        let mgr = JobManager::new();
        let job = mgr.start("apply_runtime");
        let token = mgr.token(&job.job_id).unwrap();
        assert!(!token.is_cancelled());
        mgr.cancel(&job.job_id);
        assert!(token.is_cancelled());
        assert!(token.check().is_err());
    }

    #[test]
    fn active_excludes_terminal_jobs() {
        let mgr = JobManager::new();
        let a = mgr.start("a");
        let b = mgr.start("b");
        mgr.finish(&a.job_id, JobState::Done, None);
        let active = mgr.active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].kind, "b");
        let _ = b;
    }
}
