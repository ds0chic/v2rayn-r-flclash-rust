//! net-host / privileged-helper IPC contract (T02).
//!
//! The IPC surface between AppEngine and net-host is deliberately small
//! (plan §14): `GetSnapshot`, `ApplyPlan`, `StopRuntime`, `GetOperation`,
//! `SubscribeEvents`, `Shutdown`, plus restricted test-session operations.
//! Proxy/TUN changes are folded into a single `ApplyPlan` so there is never
//! more than one uncoordinated writer.
//!
//! The wire format is JSON (serde). The transport (Windows named pipe / Unix
//! domain socket) is implemented by T03; this crate owns only the message
//! types, the protocol version, the size limit and the validation functions.

use serde::{Deserialize, Serialize};

use domain::event::EventEpoch;
use domain::job::{CancelOutcome, JobId, JobState};
use domain::runtime_plan::RuntimePlan;
use domain::{DomainError, RuntimeState};

pub mod helper;
pub mod stable;

pub use helper::*;
pub use stable::*;

/// IPC protocol version. Bump on any incompatible message change.
pub const IPC_PROTOCOL_VERSION: u32 = 1;

/// Hard cap on a single IPC frame (control messages only; configs are staged
/// by reference, never inlined past this limit).
pub const IPC_MAX_MESSAGE_BYTES: usize = 8 * 1024 * 1024;

/// Default request timeout for synchronous IPC calls.
pub const IPC_REQUEST_TIMEOUT_MS: u64 = 5_000;

/// Longer timeout for `ApplyPlan`, which may start/stop processes.
pub const IPC_APPLY_TIMEOUT_MS: u64 = 60_000;

/// Heartbeat interval used for liveness (plan §5: ~2s, 3 misses).
pub const IPC_HEARTBEAT_INTERVAL_MS: u64 = 2_000;

/// Consecutive missed heartbeats before reconciling process identity.
pub const IPC_HEARTBEAT_MISS_THRESHOLD: u8 = 3;

/// Session identity a peer must present. The token is never logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    /// Protocol version the peer speaks.
    pub protocol_version: u32,
    /// Per-boot/per-session nonce issued by net-host.
    pub session_token: String,
    /// OS process id of the AppEngine peer.
    pub peer_pid: u32,
    /// Peer process creation time (millis) to defeat PID reuse.
    pub peer_created_at_ms: i64,
}

/// A request envelope carrying the session identity and a correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub session: SessionIdentity,
    /// Correlation id echoed on the response.
    pub request_id: String,
    pub operation: IpcOperation,
}

/// A response envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub request_id: String,
    pub result: IpcResult,
}

/// The operation set. Unknown operations cannot be represented; the enum is
/// closed on purpose so no arbitrary command can be smuggled through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum IpcOperation {
    /// Read the current runtime snapshot and startup-recovery status.
    GetSnapshot,
    /// Apply an immutable runtime plan.
    ApplyPlan { plan: Box<RuntimePlan> },
    /// Stop the current managed runtime (idempotent).
    StopRuntime { operation_id: Option<String> },
    /// Query the state of a prior operation/job.
    GetOperation { operation_id: String },
    /// Subscribe to the event stream from a given epoch/seq.
    SubscribeEvents { epoch: EventEpoch, from_seq: u64 },
    /// Graceful shutdown of net-host after cleanup.
    Shutdown,
    /// Restricted test-session operations (temporary test cores only).
    TestSession(TestSessionOperation),
}

/// Restricted, enumerable test-session operations. No arbitrary shell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "test_op", rename_all = "snake_case")]
pub enum TestSessionOperation {
    /// Open a bounded test session: a dedicated temporary core, isolated from
    /// the managed runtime, on the plan's ports (all `>= 11808`).
    Open {
        /// Already-generated config for the temporary core.
        plan: Box<RuntimePlan>,
        /// Absolute wall-clock cap for the session.
        max_duration_ms: u64,
    },
    /// Close a test session by id.
    Close { session_id: String },
    /// Query a test session.
    Status { session_id: String },
}

/// Result payload of an IPC call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcResult {
    Snapshot(Box<RuntimeSnapshot>),
    Accepted { operation_id: String },
    Operation(Box<OperationStatus>),
    EventStreamOpened { epoch: EventEpoch, from_seq: u64 },
    Stopped,
    Shutdown,
    TestSession { session_id: String, state: JobState },
    Error(DomainError),
}

/// Runtime snapshot reported by net-host. This is the runtime half of the
/// AppEngine snapshot; net-host never writes the business database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    /// Runtime state machine value.
    pub state: RuntimeState,
    /// Applied revision currently running.
    pub applied_revision: u64,
    /// Current event epoch.
    pub epoch: EventEpoch,
    /// Last emitted sequence within the epoch.
    pub last_seq: u64,
    /// Currently running operation, if any.
    pub active_operation: Option<String>,
    /// Startup-recovery summary, when a recovery is pending or was performed.
    pub recovery: Option<RecoveryStatus>,
    /// Whether net-host itself is alive and holding the runtime lease.
    pub host_alive: bool,
}

/// Startup / crash recovery status (plan §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryStatus {
    /// Whether a recovery was detected on this boot.
    pub recovery_needed: bool,
    /// Stage reached by the last recorded recovery journal.
    pub last_stage: RecoveryStage,
    /// Number of resources successfully restored.
    pub restored: u32,
    /// Number of resources still owned/pending.
    pub pending: u32,
}

/// Recovery journal stages. The journal is a hint, not system truth: state is
/// always re-read from the OS before acting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStage {
    Prepared,
    Applying,
    Applied,
    Finalized,
}

/// Status of a previously issued operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationStatus {
    pub operation_id: String,
    pub job_id: Option<JobId>,
    pub state: JobState,
    pub cancel: Option<CancelOutcome>,
    pub error: Option<DomainError>,
}

impl OperationStatus {
    /// Read-only: whether the operation reached a terminal state, so a caller
    /// reconciling after a timeout knows it can stop polling.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

/// Errors from IPC framing/validation, kept separate from domain errors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IpcError {
    VersionMismatch { peer: u32, host: u32 },
    MessageTooLarge { size: usize, limit: usize },
    Malformed { detail: String },
    Unauthorized { detail: String },
}

impl IpcError {
    pub fn to_domain(&self) -> DomainError {
        match self {
            IpcError::VersionMismatch { peer, host } => DomainError::new(
                domain::codes::IPC_VERSION_MISMATCH,
                "error.ipc_version_mismatch",
            )
            .with_detail(format!("peer speaks v{peer}, host speaks v{host}")),
            IpcError::MessageTooLarge { size, limit } => DomainError::new(
                domain::codes::IPC_MESSAGE_TOO_LARGE,
                "error.ipc_message_too_large",
            )
            .with_detail(format!("{size} bytes exceeds limit {limit}")),
            IpcError::Malformed { detail } => {
                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.ipc_malformed")
                    .with_detail(detail.clone())
            }
            IpcError::Unauthorized { detail } => {
                DomainError::new(domain::codes::PERMISSION_DENIED, "error.ipc_unauthorized")
                    .with_detail(detail.clone())
            }
        }
    }
}

/// Validate the byte length of an encoded frame against the size cap.
pub fn check_frame_size(len: usize) -> Result<(), IpcError> {
    if len > IPC_MAX_MESSAGE_BYTES {
        Err(IpcError::MessageTooLarge {
            size: len,
            limit: IPC_MAX_MESSAGE_BYTES,
        })
    } else {
        Ok(())
    }
}

/// Validate a peer session against the host protocol version.
pub fn check_session(session: &SessionIdentity) -> Result<(), IpcError> {
    if session.protocol_version != IPC_PROTOCOL_VERSION {
        return Err(IpcError::VersionMismatch {
            peer: session.protocol_version,
            host: IPC_PROTOCOL_VERSION,
        });
    }
    if session.session_token.is_empty() {
        return Err(IpcError::Unauthorized {
            detail: "empty session token".to_string(),
        });
    }
    Ok(())
}

/// Request/response timeout for an operation.
pub fn timeout_for(operation: &IpcOperation) -> u64 {
    match operation {
        IpcOperation::ApplyPlan { .. } | IpcOperation::StopRuntime { .. } => IPC_APPLY_TIMEOUT_MS,
        IpcOperation::Shutdown => IPC_APPLY_TIMEOUT_MS,
        _ => IPC_REQUEST_TIMEOUT_MS,
    }
}

/// Enforce the test-port floor so the reserved user proxy port is never used.
pub fn check_test_ports(ports: &[u16]) -> Result<(), IpcError> {
    const TEST_PORT_FLOOR: u16 = 11_808;
    if let Some(bad) = ports.iter().find(|p| **p < TEST_PORT_FLOOR) {
        return Err(IpcError::Malformed {
            detail: format!("test port {bad} is below the required floor {TEST_PORT_FLOOR}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::runtime_plan::{
        ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, ProcessGraph, RuntimeTarget,
    };

    fn session(version: u32) -> SessionIdentity {
        SessionIdentity {
            protocol_version: version,
            session_token: "tok".into(),
            peer_pid: 42,
            peer_created_at_ms: 1,
        }
    }

    #[test]
    fn frame_size_is_enforced() {
        assert!(check_frame_size(IPC_MAX_MESSAGE_BYTES).is_ok());
        let err = check_frame_size(IPC_MAX_MESSAGE_BYTES + 1).unwrap_err();
        assert!(matches!(err, IpcError::MessageTooLarge { .. }));
        assert_eq!(err.to_domain().code, domain::codes::IPC_MESSAGE_TOO_LARGE);
    }

    #[test]
    fn version_mismatch_is_rejected() {
        let err = check_session(&session(IPC_PROTOCOL_VERSION + 1)).unwrap_err();
        assert!(matches!(err, IpcError::VersionMismatch { .. }));
    }

    #[test]
    fn empty_token_is_rejected() {
        let mut s = session(IPC_PROTOCOL_VERSION);
        s.session_token.clear();
        assert!(matches!(
            check_session(&s).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
    }

    #[test]
    fn test_ports_floor_is_enforced() {
        assert!(check_test_ports(&[11_808, 12_000]).is_ok());
        assert!(check_test_ports(&[10_808]).is_err());
    }

    #[test]
    fn apply_plan_uses_longer_timeout() {
        let plan = RuntimePlan {
            plan_id: "p1".into(),
            desired_revision: 1,
            target: RuntimeTarget {
                core_type: domain::CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("ab"),
            },
            process_graph: ProcessGraph::default(),
            outbound_graph: OutboundGraph::default(),
            ports: vec![],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        };
        let op = IpcOperation::ApplyPlan {
            plan: Box::new(plan),
        };
        assert_eq!(timeout_for(&op), IPC_APPLY_TIMEOUT_MS);
        assert_eq!(
            timeout_for(&IpcOperation::GetSnapshot),
            IPC_REQUEST_TIMEOUT_MS
        );
    }

    #[test]
    fn request_roundtrips_through_json() {
        let req = RequestEnvelope {
            session: session(IPC_PROTOCOL_VERSION),
            request_id: "r1".into(),
            operation: IpcOperation::SubscribeEvents {
                epoch: EventEpoch(2),
                from_seq: 17,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: RequestEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn operation_status_carries_cancel_outcome() {
        let status = OperationStatus {
            operation_id: "op1".into(),
            job_id: Some(JobId::new("j1")),
            state: JobState::Cancelling,
            cancel: Some(CancelOutcome::Compensating),
            error: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        let back: OperationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, back);
        assert!(!status.is_terminal());
    }

    #[test]
    fn operation_status_terminal_is_read_only() {
        let done = OperationStatus {
            operation_id: "op2".into(),
            job_id: None,
            state: JobState::Done,
            cancel: None,
            error: None,
        };
        assert!(done.is_terminal());
        let failed = OperationStatus {
            state: JobState::Failed,
            ..done.clone()
        };
        assert!(failed.is_terminal());
        let running = OperationStatus {
            state: JobState::Running,
            ..done
        };
        assert!(!running.is_terminal());
    }
}
