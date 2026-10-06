//! The runtime client boundary.
//!
//! T02 defines only the trait; T03 implements it with the real IPC client to
//! net-host ([`crate::net_host_client::NetHostClient`]). Keeping the trait here
//! lets the in-memory `AppEngine` be tested without any process or socket.

use std::sync::Arc;

use domain::{
    AppliedRevision, CancelOutcome, DomainError, EventEnvelope, JobId, JobState, RuntimePlan,
    RuntimeState,
};
use serde::{Deserialize, Serialize};

/// Callback invoked for every unsolicited runtime event (control + telemetry).
pub type EventSink = Arc<dyn Fn(EventEnvelope) + Send + Sync>;

/// Redacted TUN lease facts reported by net-host (T14).
///
/// Adapter label, interface index, route count and the dry-run flag only:
/// never addresses, next hops or tokens. `None` means no TUN lease is active
/// (TUN not requested, backend unavailable, or runtime stopped) — the UI must
/// render that as "not enabled", never as an active TUN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunStatus {
    pub adapter_name: String,
    pub interface_index: u32,
    pub route_count: u32,
    pub dry_run: bool,
}

/// A runtime snapshot as reported by net-host, decoupled from the ipc crate so
/// `application` does not depend on the wire format.
///
/// `state`/`applied_revision`/`host_alive` are the authoritative state; the
/// optional fields carry the live process facts the UI displays. A missing
/// `pid` is never turned into a fake "running" state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub state: RuntimeState,
    pub applied_revision: AppliedRevision,
    pub host_alive: bool,
    pub pid: Option<u32>,
    pub created_at_ms: Option<i64>,
    pub ports: Vec<u16>,
    pub session_id: Option<String>,
    pub config_sha256: Option<String>,
    pub operation_id: Option<String>,
    pub error: Option<DomainError>,
    /// Active TUN lease facts, when the running plan requested TUN.
    pub tun: Option<TunStatus>,
    /// SP-17: fact generation reported by net-host (advances on actual
    /// transitions even when desired is unchanged).
    pub actual_generation: u64,
    /// SP-17: core version of the applied plan.
    pub core_version: Option<String>,
    /// SP-17: most recent unsolicited process exit (numbers only).
    pub last_exit: Option<ExitFact>,
    /// Sidecar id for `last_exit`, when the exit was a sidecar.
    pub last_exit_sidecar: Option<String>,
}

/// One process-exit fact as reported by net-host (numbers only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitFact {
    pub pid: u32,
    pub exit_code: Option<i32>,
    pub at_ms: i64,
}

impl Default for RuntimeSnapshot {
    fn default() -> Self {
        Self {
            state: RuntimeState::Stopped,
            applied_revision: AppliedRevision::ZERO,
            host_alive: false,
            pid: None,
            created_at_ms: None,
            ports: Vec::new(),
            session_id: None,
            config_sha256: None,
            operation_id: None,
            error: None,
            tun: None,
            actual_generation: 0,
            core_version: None,
            last_exit: None,
            last_exit_sidecar: None,
        }
    }
}

/// Minimal applied-session fact (FIX-07).
///
/// Published only while net-host reports a `Running` session, so the UI and the
/// via-proxy download path never treat a *desired* active node as an *applied*
/// endpoint. `proxy_port` is the first bound inbound port net-host reports; a
/// distinct API/telemetry port is not modeled yet (registered interface gap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedSession {
    /// net-host session id of the running core.
    pub session_id: Option<String>,
    /// The active node id the running plan was built for at apply time.
    pub active_index_id: Option<String>,
    /// Actual bound local proxy port, when net-host reported one.
    pub proxy_port: Option<u16>,
    /// Applied revision reported by net-host for this session.
    pub applied_revision: AppliedRevision,
}

/// Structured, read-only status of a prior runtime operation (R4-04
/// reconcile). Decoupled from the IPC wire type so `application` stays free of
/// the ipc crate; the UI uses it to reconcile after a queue timeout instead of
/// assuming a command was lost.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationStatusView {
    pub operation_id: String,
    pub job_id: Option<String>,
    pub state: JobState,
    pub cancel: Option<CancelOutcome>,
    pub error: Option<DomainError>,
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

    /// Read the structured status of a prior operation/job (R4-04 reconcile).
    ///
    /// The default is a structured not-found error so an in-memory client can
    /// never fabricate a tracked operation.
    fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
        Err(DomainError::not_found("operation", operation_id))
    }

    /// Submit an immutable plan. Results are delivered via the event stream.
    fn apply(&self, plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError>;

    /// Stop the managed runtime. Idempotent.
    fn stop(&self) -> Result<(), DomainError>;

    /// Request cancellation of a running operation. Idempotent.
    fn cancel(&self, job_id: &JobId) -> Result<CancelOutcome, DomainError>;

    /// Register the sink for unsolicited events. The in-memory client has no
    /// asynchronous producer, so the default is a no-op.
    fn subscribe_events(&self, _sink: EventSink) {}
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
    ports: std::sync::Mutex<Vec<u16>>,
    session_id: std::sync::Mutex<Option<String>>,
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

    /// Simulate a running core with a concrete session and bound ports
    /// (test helper for the FIX-07 applied-session publication).
    pub fn mark_running_with(&self, session_id: &str, ports: Vec<u16>, revision: AppliedRevision) {
        self.mark_running(revision);
        if let Ok(mut p) = self.ports.lock() {
            *p = ports;
        }
        if let Ok(mut s) = self.session_id.lock() {
            *s = Some(session_id.to_string());
        }
    }

    /// Force a non-`Running` state (e.g. `Degraded`) for failure-path tests.
    pub fn set_state(&self, state: RuntimeState) {
        if let Ok(mut s) = self.state.lock() {
            *s = state;
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
            ports: self.ports.lock().map(|p| p.clone()).unwrap_or_default(),
            session_id: self.session_id.lock().ok().and_then(|s| s.clone()),
            ..RuntimeSnapshot::default()
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
        if let Ok(mut p) = self.ports.lock() {
            p.clear();
        }
        if let Ok(mut sid) = self.session_id.lock() {
            *sid = None;
        }
        Ok(())
    }

    fn cancel(&self, _job_id: &JobId) -> Result<CancelOutcome, DomainError> {
        Ok(CancelOutcome::Requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_client_records_plan_and_marks_running() {
        let client = NullRuntimeClient::new();
        assert_eq!(client.snapshot().unwrap().state, RuntimeState::Stopped);
        client.mark_running(AppliedRevision::new(2));
        let snap = client.snapshot().unwrap();
        assert_eq!(snap.state, RuntimeState::Running);
        assert_eq!(snap.applied_revision, AppliedRevision::new(2));
        assert!(snap.host_alive);
        assert!(snap.pid.is_none());
    }

    #[test]
    fn snapshot_carries_no_tun_lease_by_default() {
        let snap = RuntimeSnapshot::default();
        assert!(snap.tun.is_none());
    }

    #[test]
    fn tun_status_roundtrips_and_compares() {
        let status = TunStatus {
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            route_count: 2,
            dry_run: true,
        };
        let back: TunStatus =
            serde_json::from_slice(&serde_json::to_vec(&status).unwrap()).unwrap();
        assert_eq!(status, back);
        let snap = RuntimeSnapshot {
            tun: Some(status),
            ..RuntimeSnapshot::default()
        };
        assert!(snap.tun.as_ref().unwrap().dry_run);
    }
}
