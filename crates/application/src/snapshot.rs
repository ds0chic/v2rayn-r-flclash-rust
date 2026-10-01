//! Snapshot assembly (plan §14 `get_snapshot`).
//!
//! The snapshot is the single read model the UI synchronizes against. It
//! reports settings version, runtime snapshot, active jobs, capability table
//! and startup-recovery status. It never infers runtime state from a button
//! click; the runtime fields come from the [`RuntimeClient`].

use serde::{Deserialize, Serialize};

use domain::{
    AppliedRevision, DesiredRevision, DomainError, RevisionPair, RevisionState, RuntimeState,
};

use crate::jobs::JobView;
use crate::runtime_client::RuntimeSnapshot;

/// Capability entry: which protocols a core supports (plan §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEntry {
    pub core: domain::CoreType,
    pub config_types: Vec<domain::ConfigType>,
    /// Whether structured config generation is available (vs. custom file).
    pub structured_generation: bool,
    /// Whether this app manages updates for the core.
    pub update_supported: bool,
}

/// Startup recovery status surfaced to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupRecovery {
    pub recovery_needed: bool,
    /// Human-readable stage token (journal hint only, not system truth).
    pub stage: Option<String>,
    pub restored: u32,
    pub pending: u32,
}

/// The assembled snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Desired/applied revision pair.
    pub revisions: RevisionPair,
    /// Derived revision state for the UI.
    pub revision_state: RevisionState,
    /// Runtime state as reported by net-host.
    pub runtime_state: RuntimeState,
    /// Applied revision as reported by net-host.
    pub applied_revision: AppliedRevision,
    /// Whether net-host itself is alive and holding the runtime lease.
    pub host_alive: bool,
    /// OS pid of the managed core, when running (never fabricated).
    pub runtime_pid: Option<u32>,
    /// Creation time of the managed core (Windows 100ns -> Unix ms).
    pub runtime_created_at_ms: Option<i64>,
    /// Ports the managed core is listening on.
    pub runtime_ports: Vec<u16>,
    /// Current net-host session id.
    pub runtime_session_id: Option<String>,
    /// staged config SHA-256 as verified by net-host.
    pub runtime_config_sha256: Option<String>,
    /// Currently running operation id, when any.
    pub runtime_operation_id: Option<String>,
    /// Last structured runtime error, when the runtime is not healthy.
    pub runtime_error: Option<DomainError>,
    /// Active (non-terminal) jobs.
    pub active_jobs: Vec<JobView>,
    /// Core capability table.
    pub capabilities: Vec<CapabilityEntry>,
    /// Startup recovery status.
    pub recovery: StartupRecovery,
    /// Number of profiles in the store.
    pub profile_count: u64,
}

/// Build a snapshot from its parts.
pub fn assemble(
    desired: DesiredRevision,
    runtime: &RuntimeSnapshot,
    active_jobs: Vec<JobView>,
    capabilities: Vec<CapabilityEntry>,
    recovery: StartupRecovery,
    profile_count: u64,
) -> Snapshot {
    let revisions = RevisionPair::new(desired, runtime.applied_revision);
    Snapshot {
        revision_state: revisions.state(),
        revisions,
        runtime_state: runtime.state,
        applied_revision: runtime.applied_revision,
        host_alive: runtime.host_alive,
        runtime_pid: runtime.pid,
        runtime_created_at_ms: runtime.created_at_ms,
        runtime_ports: runtime.ports.clone(),
        runtime_session_id: runtime.session_id.clone(),
        runtime_config_sha256: runtime.config_sha256.clone(),
        runtime_operation_id: runtime.operation_id.clone(),
        runtime_error: runtime.error.clone(),
        active_jobs,
        capabilities,
        recovery,
        profile_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reports_pending_when_desired_ahead() {
        let runtime = RuntimeSnapshot {
            state: RuntimeState::Stopped,
            applied_revision: AppliedRevision::new(3),
            host_alive: true,
            ..RuntimeSnapshot::default()
        };
        let snap = assemble(
            DesiredRevision::new(5),
            &runtime,
            vec![],
            vec![],
            StartupRecovery {
                recovery_needed: false,
                stage: None,
                restored: 0,
                pending: 0,
            },
            10,
        );
        assert_eq!(snap.revision_state, RevisionState::Pending);
        assert_eq!(snap.runtime_state, RuntimeState::Stopped);
        assert_eq!(snap.profile_count, 10);
    }

    #[test]
    fn snapshot_in_sync_when_equal() {
        let runtime = RuntimeSnapshot {
            state: RuntimeState::Running,
            applied_revision: AppliedRevision::new(4),
            host_alive: true,
            ..RuntimeSnapshot::default()
        };
        let snap = assemble(
            DesiredRevision::new(4),
            &runtime,
            vec![],
            vec![],
            StartupRecovery {
                recovery_needed: false,
                stage: None,
                restored: 0,
                pending: 0,
            },
            0,
        );
        assert_eq!(snap.revision_state, RevisionState::InSync);
    }
}
