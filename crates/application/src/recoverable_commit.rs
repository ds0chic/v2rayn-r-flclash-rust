//! Recoverable settings commit helpers (SP-02).
//!
//! The commit protocol coordinates one SQLite transaction with one
//! `guiNConfig.json` publish through the [`persistence::commit`] journal:
//!
//! ```text
//! validate -> begin (stage + journal + staged file) -> db commit
//!   -> mark_db_committed -> publish staged file -> finish -> install memory
//! ```
//!
//! Receipts reuse the frozen SP-00 contract
//! ([`ipc_contract::stable`]): a rejected save carries no `new_revision`; a
//! DB-committed but unpublished save is `CommitUnknown` and blocks new writes
//! with `RecoveryRequired` until recovery rolls it forward or back. The DB
//! transaction and the file publish are *not* claimed to be one atomic
//! transaction; the journal makes every stage explicit and recoverable.

use std::collections::HashMap;

use domain::DomainError;
use ipc_contract::stable::{
    ContractError, DatasetEpoch, PhaseApplyState, PhaseError, SettingsSaveReceipt,
    SettingsSaveState,
};
use serde::{Deserialize, Serialize};

pub use persistence::commit::CommitFault as CommitTestFault;
pub use persistence::commit::CommitStage;

/// In-memory stand-in for a journal + done receipt, used only by engines
/// without a data directory (tests/bootstrap). File-backed engines use the
/// real journal on disk.
#[derive(Debug, Clone)]
pub struct MemCommit {
    pub receipt_json: String,
    pub content_hash: String,
}

/// In-memory stand-in for an unresolved journal (no data directory).
#[derive(Debug, Clone)]
pub struct MemPending {
    pub mutation_id: String,
    pub commit_id: String,
    pub dataset_epoch: DatasetEpoch,
    pub expected_revision: u64,
    pub content_hash: String,
    pub stage: CommitStage,
    pub doc: serde_json::Value,
    pub new_revision: u64,
    pub new_desired: u64,
}

/// Outcome of one settings commit attempt before the receipt is built.
/// `settings_hash` is the idempotency key (canonical normalised settings);
/// `doc_hash` is the publish integrity key (exact staged payload bytes).
#[derive(Debug, Clone)]
pub struct ValidatedSave {
    pub doc: serde_json::Value,
    pub settings_hash: String,
    pub doc_hash: String,
    pub staged_text: String,
    pub commit_id: String,
    pub new_revision: u64,
    pub new_desired: u64,
    pub new_groups: HashMap<String, u64>,
}

/// Canonical hash of normalised settings: the idempotency key.
pub fn settings_hash_of(settings: &domain::AppSettings) -> String {
    let canonical = serde_json::to_string(settings).unwrap_or_default();
    persistence::hash::sha256_hex(canonical.as_bytes())
}

pub fn contract_error(
    code: &str,
    message_key: &str,
    detail: Option<String>,
    retryable: bool,
) -> ContractError {
    ContractError {
        code: code.to_string(),
        message_key: message_key.to_string(),
        detail,
        retryable,
    }
}

pub fn domain_to_contract(error: &DomainError) -> ContractError {
    contract_error(
        &error.code,
        &error.message_key,
        error.detail.clone(),
        error.retryable,
    )
}

/// Map a persistence failure onto the shared contract, preserving the stable
/// code and message key. The detail carries only the classified failure text,
/// never document content or secrets.
pub fn persist_to_contract(error: &persistence::PersistenceError) -> ContractError {
    let retryable = matches!(
        error,
        persistence::PersistenceError::Io(_)
            | persistence::PersistenceError::Sqlite(_)
            | persistence::PersistenceError::Validation(_)
    );
    contract_error(
        error.code(),
        error.message_key(),
        Some(error.to_string()),
        retryable,
    )
}

/// Point-in-time settings state a commit is built from.
#[derive(Debug, Clone)]
pub struct SettingsCommitSnapshot {
    pub settings: domain::AppSettings,
    pub revision: u64,
    pub group_revisions: std::collections::BTreeMap<String, u64>,
    pub desired: u64,
    pub active: Option<String>,
    pub sub_index_id: Option<String>,
    pub rule_mode: String,
    pub templates: Vec<domain::FullConfigTemplate>,
}

fn base_receipt(mutation_id: &str, dataset_epoch: DatasetEpoch) -> SettingsSaveReceipt {
    SettingsSaveReceipt {
        mutation_id: mutation_id.to_string(),
        dataset_epoch,
        commit_id: String::new(),
        commit_epoch: None,
        new_revision: None,
        saved_document_token: None,
        save: SettingsSaveState::NotStarted,
        core_apply: PhaseApplyState::NotRequired,
        platform_apply: PhaseApplyState::NotRequired,
        phase_errors: Vec::new(),
        applied_content_hashes: Vec::new(),
        operation_ids: Vec::new(),
    }
}

/// A save that committed nothing carries no `new_revision`.
pub fn rejected_receipt(
    mutation_id: &str,
    dataset_epoch: DatasetEpoch,
    phase: &str,
    error: ContractError,
) -> SettingsSaveReceipt {
    let mut receipt = base_receipt(mutation_id, dataset_epoch);
    receipt.save = SettingsSaveState::Rejected;
    receipt.phase_errors.push(PhaseError {
        phase: phase.to_string(),
        error,
    });
    receipt
}

/// A query for a mutation this engine never saw.
pub fn not_started_receipt(mutation_id: &str, dataset_epoch: DatasetEpoch) -> SettingsSaveReceipt {
    let mut receipt = base_receipt(mutation_id, dataset_epoch);
    receipt.phase_errors.push(PhaseError {
        phase: "commit".to_string(),
        error: contract_error(
            "E_UNKNOWN_MUTATION",
            "error.unknown_mutation",
            Some(format!("mutation `{mutation_id}` was never submitted")),
            false,
        ),
    });
    receipt
}

/// The DB half committed but the outcome is not confirmed. Never claims
/// "nothing changed" and never pins a revision.
pub fn unknown_receipt(
    mutation_id: &str,
    commit_id: &str,
    dataset_epoch: DatasetEpoch,
    phase: &str,
    error: ContractError,
) -> SettingsSaveReceipt {
    let mut receipt = base_receipt(mutation_id, dataset_epoch);
    receipt.save = SettingsSaveState::CommitUnknown;
    receipt.commit_id = commit_id.to_string();
    receipt.commit_epoch = Some(1);
    receipt.phase_errors.push(PhaseError {
        phase: phase.to_string(),
        error,
    });
    receipt
}

/// New writes stay blocked while recovery is pending.
pub fn blocked_receipt(mutation_id: &str, dataset_epoch: DatasetEpoch) -> SettingsSaveReceipt {
    let mut receipt = base_receipt(mutation_id, dataset_epoch);
    receipt.save = SettingsSaveState::RecoveryRequired;
    receipt.phase_errors.push(PhaseError {
        phase: "recovery".to_string(),
        error: contract_error(
            "E_RECOVERY_REQUIRED",
            "error.recovery_required",
            Some("a previous commit is unresolved; run recovery before writing".to_string()),
            true,
        ),
    });
    receipt
}

pub fn committed_receipt(
    mutation_id: &str,
    commit_id: &str,
    dataset_epoch: DatasetEpoch,
    new_revision: u64,
    token: &str,
    content_hash: &str,
) -> SettingsSaveReceipt {
    let mut receipt = base_receipt(mutation_id, dataset_epoch);
    receipt.save = SettingsSaveState::Committed;
    receipt.commit_id = commit_id.to_string();
    receipt.commit_epoch = Some(1);
    receipt.new_revision = Some(new_revision);
    receipt.saved_document_token = Some(token.to_string());
    receipt.applied_content_hashes = vec![content_hash.to_string()];
    receipt
}

/// Deterministic commit identity for a mutation, so a replayed mutation maps
/// to the same commit instead of a duplicate.
pub fn commit_id_for(mutation_id: &str) -> String {
    persistence::hash::derived_id("commit", mutation_id)
}

/// Token binding a committed document to its commit + revision + content.
pub fn document_token(commit_id: &str, new_revision: u64, content_hash: &str) -> String {
    let short = content_hash.chars().take(16).collect::<String>();
    format!("{commit_id}:{new_revision}:{short}")
}

/// Stored completion: receipt JSON plus the content hash it committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCompletion {
    pub receipt_json: String,
    pub content_hash: String,
}

pub fn stored_completion(receipt: &SettingsSaveReceipt, content_hash: &str) -> StoredCompletion {
    StoredCompletion {
        receipt_json: serde_json::to_string(receipt).unwrap_or_else(|_| "{}".to_string()),
        content_hash: content_hash.to_string(),
    }
}

pub fn parse_receipt(text: &str) -> Option<SettingsSaveReceipt> {
    serde_json::from_str(text).ok()
}
