//! Stable-port shared contracts (SP-00).
//!
//! These are the versioned wire/contract types the stable-port plan (§3)
//! freezes before parallel wiring: runtime intents/receipts/actual facts,
//! settings save receipts with per-phase apply state, native window envelopes
//! and paged/import request metadata. They are compile-available contracts;
//! providers and callers are wired by the later SP cards. Field and enum
//! names follow the plan so no two DTOs describe the same fact differently.

use serde::{Deserialize, Serialize};

/// Version of this contract family. Bump on any incompatible change.
pub const STABLE_CONTRACT_VERSION: u32 = 1;

/// Whole-dataset generation, created when a restore/backup candidate replaces
/// the active data. Ordinary settings, text imports and subscription deltas
/// keep it. Requests, receipts and queries must carry it so a stale request
/// cannot take effect after a restore (plan §3.1).
pub type DatasetEpoch = u64;

/// Structured contract error (stable fields, no secret material).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractError {
    pub code: String,
    pub message_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default)]
    pub retryable: bool,
}

/// One user command intent for the runtime plane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeIntent {
    pub client_command_id: String,
    pub client_instance_id: String,
    pub intent_seq: u64,
    pub operation_id: String,
    pub dataset_epoch: DatasetEpoch,
    pub action: RuntimeIntentAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_desired_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explicit_target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen_plan_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_generation: Option<u64>,
}

/// Runtime intent actions. `Start`/`Reload` map to the host `Apply`; there is
/// exactly one action enum (no parallel unmapped variant set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIntentAction {
    Start,
    Reload,
    Stop,
}

/// Operation lifecycle state, versioned separately from the historical
/// `JobState` so the existing job facility keeps its enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Accepted,
    Executing,
    Succeeded,
    Failed,
    Cancelled,
    Superseded,
}

/// Receipt for one accepted intent. Transport ACK is not this receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReceipt {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    pub accepted_intent_seq: u64,
    pub host_instance_id: String,
    pub host_admission_seq: u64,
    pub status: OperationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ContractError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_snapshot: Option<RuntimeActualDescriptor>,
}

/// A typed ready endpoint published by the actual runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadyEndpoint {
    pub scheme: String,
    pub owner: String,
    pub core: String,
    pub api_kind: String,
    pub auth_required: bool,
    pub port: u16,
}

/// One managed process fact (main core or sidecar).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessStateFact {
    pub id: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

/// Last exit observed for a managed process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreExitFact {
    pub pid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub at_ms: i64,
}

impl CoreExitFact {
    /// Record one handle-authoritative exit observation (SP-06). Numbers only;
    /// never credentials or config material.
    pub fn observed(pid: u32, exit_code: Option<i32>, at_ms: i64) -> Self {
        Self {
            pid,
            exit_code,
            at_ms,
        }
    }
}

/// Live TUN lease facts (mirrors the runtime snapshot facts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunLeaseFacts {
    pub adapter_name: String,
    pub interface_index: u32,
    pub route_count: u32,
    pub dry_run: bool,
}

/// The frozen plan that is actually running: identity, generation and facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeActualDescriptor {
    pub session_id: String,
    pub actual_generation: u64,
    pub operation_id: String,
    pub intent_seq: u64,
    pub target_profile_id: String,
    pub target_core: String,
    pub core_version: String,
    pub plan_hash: String,
    pub applied_runtime_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_pid: Option<u32>,
    #[serde(default)]
    pub sidecar_pids: Vec<u32>,
    #[serde(default)]
    pub ready_endpoints: Vec<ReadyEndpoint>,
    pub main_state: String,
    #[serde(default)]
    pub sidecar_states: Vec<ProcessStateFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tun_lease_facts: Option<TunLeaseFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_exit: Option<CoreExitFact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<ContractError>,
}

impl RuntimeActualDescriptor {
    /// Apply a main-core exit (SP-06): the fact generation advances even
    /// though desired did not change, the live PID/endpoints are withdrawn,
    /// and the frozen plan history (target/applied revision/plan hash) is
    /// preserved. Never invents Running.
    pub fn mark_main_exited(&mut self, exit: CoreExitFact, error: ContractError) {
        self.actual_generation = self.actual_generation.saturating_add(1);
        self.main_state = "exited".to_string();
        self.main_pid = None;
        self.ready_endpoints.clear();
        self.last_exit = Some(exit);
        self.last_error = Some(error);
    }

    /// Apply a sidecar exit (SP-06): the sidecar fact goes `exited`, a live
    /// main core degrades instead of reporting a clean Running, and the most
    /// recent exit is recorded. History is preserved.
    pub fn mark_sidecar_exited(
        &mut self,
        sidecar_id: &str,
        exit: CoreExitFact,
        error: ContractError,
    ) {
        self.actual_generation = self.actual_generation.saturating_add(1);
        let mut found = false;
        for state in &mut self.sidecar_states {
            if state.id == sidecar_id {
                state.state = "exited".to_string();
                state.pid = None;
                found = true;
            }
        }
        if !found {
            self.sidecar_states.push(ProcessStateFact {
                id: sidecar_id.to_string(),
                state: "exited".to_string(),
                pid: None,
            });
        }
        if self.main_state == "running" {
            self.main_state = "degraded".to_string();
        }
        self.last_exit = Some(exit);
        self.last_error = Some(error);
    }
}

/// Settings persistence phase outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSaveState {
    NotStarted,
    Rejected,
    Committed,
    CommitUnknown,
    RecoveryRequired,
}

/// One apply phase (core / platform) outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseApplyState {
    NotRequired,
    Pending,
    Succeeded,
    Failed,
    Unknown,
}

/// Error attached to one phase of a save/apply operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseError {
    pub phase: String,
    pub error: ContractError,
}

/// Settings save + apply receipt. Save and apply facts are separate: a failed
/// core/platform apply never erases a committed save, and desired is never
/// reported as applied (plan §3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsSaveReceipt {
    pub mutation_id: String,
    pub dataset_epoch: DatasetEpoch,
    pub commit_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_epoch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_document_token: Option<String>,
    pub save: SettingsSaveState,
    pub core_apply: PhaseApplyState,
    pub platform_apply: PhaseApplyState,
    #[serde(default)]
    pub phase_errors: Vec<PhaseError>,
    #[serde(default)]
    pub applied_content_hashes: Vec<String>,
    #[serde(default)]
    pub operation_ids: Vec<String>,
}

/// Native-window request envelope. The MethodChannel transport ACK only means
/// the request arrived; the real outcome comes back as a
/// [`WindowSaveOutcome`] keyed by generation/request id (plan §3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowRequestEnvelope {
    pub window_id: String,
    pub window_generation: u64,
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_id: Option<String>,
    pub dataset_epoch: DatasetEpoch,
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    pub payload_json: String,
}

/// Native-window save outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowOutcomeStatus {
    Ok,
    Failed,
    /// The bounded wait expired; the caller must query the mutation instead of
    /// replaying a non-idempotent save.
    PendingConfirmation,
}

/// Structured outcome for one window request (an exception must still produce
/// an outcome, never a silent hang).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSaveOutcome {
    pub window_id: String,
    pub window_generation: u64,
    pub request_id: String,
    pub dataset_epoch: DatasetEpoch,
    pub status: WindowOutcomeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save_receipt: Option<SettingsSaveReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ContractError>,
}

/// Async paged-query metadata (plan §3.5). The bridge layer keeps its existing
/// filter/sort DTOs; this is the shared cursor/epoch contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageQueryMeta {
    pub cursor: u64,
    pub dataset_revision: u64,
    pub request_generation: u64,
    pub limit: u32,
}

/// Result metadata for one async page. A stale `dataset_revision` invalidates
/// the cursor instead of silently skipping/duplicating rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageResultMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<u64>,
    pub dataset_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ContractError>,
}

/// Commit of a previously previewed import batch. `preview_token` binds the
/// exact previewed content; `mutation_id` + `dataset_epoch` make the commit
/// deduplicable and epoch-scoped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitImportRequest {
    pub preview_token: String,
    pub expected_revision: u64,
    pub mutation_id: String,
    pub dataset_epoch: DatasetEpoch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_group: Option<String>,
}

/// Result of one import commit (whole batch or nothing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitImportResult {
    pub ok: bool,
    pub imported: u32,
    pub skipped: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_revision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ContractError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_version_is_one() {
        assert_eq!(STABLE_CONTRACT_VERSION, 1);
    }

    #[test]
    fn runtime_intent_round_trips_with_epoch() {
        let intent = RuntimeIntent {
            client_command_id: "cmd-1".into(),
            client_instance_id: "win-main".into(),
            intent_seq: 7,
            operation_id: "op-1".into(),
            dataset_epoch: 3,
            action: RuntimeIntentAction::Start,
            expected_desired_revision: Some(11),
            explicit_target_id: Some("p-1".into()),
            frozen_plan_hash: None,
            accepted_generation: Some(2),
        };
        let json = serde_json::to_string(&intent).unwrap();
        let back: RuntimeIntent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, intent);
    }

    #[test]
    fn save_receipt_separates_save_and_apply_facts() {
        let receipt = SettingsSaveReceipt {
            mutation_id: "m-1".into(),
            dataset_epoch: 0,
            commit_id: "c-1".into(),
            commit_epoch: Some(1),
            new_revision: Some(12),
            saved_document_token: Some("tok".into()),
            save: SettingsSaveState::Committed,
            core_apply: PhaseApplyState::Failed,
            platform_apply: PhaseApplyState::Pending,
            phase_errors: vec![PhaseError {
                phase: "core".into(),
                error: ContractError {
                    code: "E_UNAVAILABLE".into(),
                    message_key: "error.core_apply_failed".into(),
                    detail: None,
                    retryable: true,
                },
            }],
            applied_content_hashes: vec!["abc".into()],
            operation_ids: vec!["op-9".into()],
        };
        let json = serde_json::to_string(&receipt).unwrap();
        let back: SettingsSaveReceipt = serde_json::from_str(&json).unwrap();
        assert_eq!(back.save, SettingsSaveState::Committed);
        assert_eq!(back.core_apply, PhaseApplyState::Failed);
        assert_eq!(back.new_revision, Some(12));
    }

    #[test]
    fn window_outcome_round_trips() {
        let outcome = WindowSaveOutcome {
            window_id: "settings".into(),
            window_generation: 4,
            request_id: "req-2".into(),
            dataset_epoch: 1,
            status: WindowOutcomeStatus::PendingConfirmation,
            save_receipt: None,
            new_revision: None,
            error: None,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: WindowSaveOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(back, outcome);
    }

    #[test]
    fn commit_import_request_binds_preview_and_epoch() {
        let request = CommitImportRequest {
            preview_token: "pv-1".into(),
            expected_revision: 5,
            mutation_id: "m-2".into(),
            dataset_epoch: 2,
            target_group: Some("g-1".into()),
        };
        let json = serde_json::to_string(&request).unwrap();
        let back: CommitImportRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back, request);
    }

    fn running_descriptor() -> RuntimeActualDescriptor {
        RuntimeActualDescriptor {
            session_id: "s-1".into(),
            actual_generation: 4,
            operation_id: "op-1".into(),
            intent_seq: 2,
            target_profile_id: "p-a".into(),
            target_core: "xray".into(),
            core_version: "26.3.27".into(),
            plan_hash: "hash-plan".into(),
            applied_runtime_revision: 11,
            main_pid: Some(42756),
            sidecar_pids: vec![42757],
            ready_endpoints: vec![ReadyEndpoint {
                scheme: "socks".into(),
                owner: "inbound".into(),
                core: "xray".into(),
                api_kind: "socks5".into(),
                auth_required: false,
                port: 11977,
            }],
            main_state: "running".into(),
            sidecar_states: vec![ProcessStateFact {
                id: "pre-socks".into(),
                state: "running".into(),
                pid: Some(42757),
            }],
            tun_lease_facts: None,
            last_exit: None,
            last_error: None,
        }
    }

    fn exit_error() -> ContractError {
        ContractError {
            code: "E_INTERNAL".into(),
            message_key: "error.core_exited".into(),
            detail: Some("pid=42756 code=1".into()),
            retryable: false,
        }
    }

    #[test]
    fn main_exit_withdraws_endpoints_and_advances_generation() {
        let mut descriptor = running_descriptor();
        let exit = CoreExitFact::observed(42756, Some(1), 1_700_000_000_001);
        descriptor.mark_main_exited(exit.clone(), exit_error());
        assert_eq!(descriptor.main_state, "exited");
        assert!(descriptor.main_pid.is_none(), "no ghost PID");
        assert!(descriptor.ready_endpoints.is_empty(), "no stale endpoint");
        assert_eq!(descriptor.last_exit, Some(exit));
        assert_eq!(descriptor.last_error, Some(exit_error()));
        assert_eq!(descriptor.actual_generation, 5);
        assert_eq!(descriptor.target_profile_id, "p-a");
        assert_eq!(descriptor.applied_runtime_revision, 11);
        assert_eq!(descriptor.plan_hash, "hash-plan");
    }

    #[test]
    fn sidecar_exit_degrades_without_rewriting_history() {
        let mut descriptor = running_descriptor();
        let exit = CoreExitFact::observed(42757, Some(3), 1_700_000_000_002);
        descriptor.mark_sidecar_exited("pre-socks", exit.clone(), exit_error());
        assert_eq!(descriptor.main_state, "degraded");
        assert_eq!(
            descriptor.sidecar_states,
            vec![ProcessStateFact {
                id: "pre-socks".into(),
                state: "exited".into(),
                pid: None,
            }]
        );
        assert_eq!(descriptor.main_pid, Some(42756));
        assert_eq!(descriptor.ready_endpoints.len(), 1);
        assert_eq!(descriptor.last_exit, Some(exit));
        assert_eq!(descriptor.actual_generation, 5);
        assert_eq!(descriptor.applied_runtime_revision, 11);
    }
}
