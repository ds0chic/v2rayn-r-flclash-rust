//! Flat, FRB-translatable DTOs for the T02 API surface.
//!
//! These are deliberately decoupled from the rich domain types (which contain
//! `serde_json::Map` unknown-extension bags that FRB cannot translate). The
//! DTOs carry only the fields the UI needs; the domain remains the source of
//! truth on the Rust side.

use domain::enums::{ConfigType, CoreType};
use domain::error::DomainError;
use domain::event::RuntimeState;
use domain::job::{CancelOutcome, JobState};
use domain::revision::RevisionState;

/// Revision pair reported by the snapshot.
#[derive(Clone)]
pub struct RevisionDto {
    pub desired: u64,
    pub applied: u64,
    pub state: RevisionState,
}

/// Active job summary.
#[derive(Clone)]
pub struct JobDto {
    pub job_id: String,
    pub kind: String,
    pub state: JobState,
    /// `None` when the stage is unknown (never fake a percent).
    pub percent: Option<u8>,
    pub stage_key: Option<String>,
    pub error_code: Option<String>,
    pub error_message_key: Option<String>,
}

/// Core capability entry.
#[derive(Clone)]
pub struct CapabilityDto {
    pub core: CoreType,
    pub config_types: Vec<ConfigType>,
    pub structured_generation: bool,
    pub update_supported: bool,
}

/// Startup recovery summary.
#[derive(Clone)]
pub struct RecoveryDto {
    pub recovery_needed: bool,
    pub stage: Option<String>,
    pub restored: u32,
    pub pending: u32,
}

/// The `get_snapshot` result.
///
/// Runtime facts (`runtime_pid`, `runtime_ports`, ...) are the live values
/// reported by net-host; they are `None`/empty when the runtime is not running
/// and are never synthesized from UI state.
#[derive(Clone)]
pub struct SnapshotDto {
    pub desired_revision: u64,
    pub applied_revision: u64,
    pub revision_state: RevisionState,
    pub runtime_state: RuntimeState,
    pub host_alive: bool,
    pub runtime_pid: Option<u32>,
    pub runtime_created_at_ms: Option<i64>,
    pub runtime_ports: Vec<u16>,
    pub runtime_session_id: Option<String>,
    pub runtime_config_sha256: Option<String>,
    pub runtime_operation_id: Option<String>,
    pub runtime_error: Option<ErrorDto>,
    pub active_jobs: Vec<JobDto>,
    pub capabilities: Vec<CapabilityDto>,
    pub recovery: RecoveryDto,
    pub profile_count: u64,
}

/// Error DTO carrying the stable contract fields.
#[derive(Clone)]
pub struct ErrorDto {
    pub code: String,
    pub message_key: String,
    pub field_path: Option<String>,
    pub retryable: bool,
    pub operation_id: Option<String>,
    pub detail: Option<String>,
}

impl From<DomainError> for ErrorDto {
    fn from(e: DomainError) -> Self {
        Self {
            code: e.code,
            message_key: e.message_key,
            field_path: e.field_path,
            retryable: e.retryable,
            operation_id: e.operation_id,
            detail: e.detail,
        }
    }
}

/// Query filter for `query_profiles`.
#[derive(Clone)]
pub struct ProfileFilterDto {
    pub text: Option<String>,
    pub config_types: Vec<ConfigType>,
    pub subid: Option<String>,
}

/// Sort key for `query_profiles`.
#[derive(Clone)]
pub enum ProfileSortDto {
    Remarks,
    Address,
    Delay,
    IndexId,
}

/// A profile row. `config_version` and `core_type` are optional because the
/// summary row need not carry them.
#[derive(Clone)]
pub struct ProfileDto {
    pub index_id: String,
    pub config_type: ConfigType,
    pub core_type: Option<CoreType>,
    pub remarks: String,
    pub address: String,
    pub port: i32,
    pub network: String,
    pub stream_security: Option<String>,
    pub subid: String,
    pub username: String,
    pub mux_enabled: Option<bool>,
    /// Opaque unknown-extension JSON, preserved for round-trip.
    pub extra_json: String,
}

/// One page of profiles with a stable cursor.
#[derive(Clone)]
pub struct ProfilePageDto {
    pub items: Vec<ProfileDto>,
    pub total: u64,
    pub next_cursor: Option<u64>,
}

/// Result of `save_profile`: either the saved profile or a field error.
#[derive(Clone)]
pub struct SaveProfileResult {
    pub ok: bool,
    pub profile: Option<ProfileDto>,
    pub new_revision: Option<u64>,
    pub error: Option<ErrorDto>,
}

/// Result of `apply_runtime`.
#[derive(Clone)]
pub struct ApplyRuntimeResult {
    pub ok: bool,
    pub operation_id: Option<String>,
    pub error: Option<ErrorDto>,
}

/// Result of `stop_runtime`.
#[derive(Clone)]
pub struct StopRuntimeResult {
    pub ok: bool,
    pub error: Option<ErrorDto>,
}

/// Result of `cancel_job`.
#[derive(Clone)]
pub struct CancelResult {
    pub outcome: CancelOutcome,
    pub error: Option<ErrorDto>,
}

/// Event envelope delivered on the FRB stream.
#[derive(Clone)]
pub struct EventEnvelopeDto {
    pub epoch: u64,
    pub seq: u64,
    pub kind: String,
    /// `true` for control events, `false` for telemetry.
    pub control: bool,
    /// Raw JSON payload.
    pub payload_json: String,
}

/// Request to apply a runtime plan by target id (T02: a stub plan builder).
#[derive(Clone)]
pub struct ApplyRuntimeRequest {
    pub target_id: String,
    pub expected_revision: u64,
}
