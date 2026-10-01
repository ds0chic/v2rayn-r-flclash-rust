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

/// TLS/Reality fields carried directly on `ProfileItem`.
#[derive(Clone, Default)]
pub struct SecurityDto {
    pub stream_security: Option<String>,
    pub allow_insecure: Option<String>,
    pub sni: Option<String>,
    pub alpn: Option<String>,
    pub fingerprint: Option<String>,
    pub public_key: Option<String>,
    pub short_id: Option<String>,
    pub spider_x: Option<String>,
    pub mldsa65_verify: Option<String>,
    pub cert: Option<String>,
    pub cert_sha: Option<String>,
    pub ech_config_list: Option<String>,
    pub verify_peer_cert_by_name: Option<String>,
}

/// `ProtocolExtraItem` (30 properties) exposed to the editor.
#[derive(Clone, Default)]
pub struct ProtocolExtraDto {
    pub uot: Option<bool>,
    pub congestion_control: Option<String>,
    pub http_headers: Option<String>,
    pub alter_id: Option<String>,
    pub vmess_security: Option<String>,
    pub flow: Option<String>,
    pub vless_encryption: Option<String>,
    pub ss_method: Option<String>,
    pub wg_public_key: Option<String>,
    pub wg_preshared_key: Option<String>,
    pub wg_interface_address: Option<String>,
    pub wg_reserved: Option<String>,
    pub wg_mtu: Option<i32>,
    pub wg_dns: Option<String>,
    pub salamander_pass: Option<String>,
    pub up_mbps: Option<i32>,
    pub down_mbps: Option<i32>,
    pub ports: Option<String>,
    pub hop_interval: Option<String>,
    pub hy2_realm_url: Option<String>,
    pub gecko_min_packet_size: Option<String>,
    pub gecko_max_packet_size: Option<String>,
    pub insecure_concurrency: Option<i32>,
    pub naive_quic: Option<bool>,
    pub group_type: Option<String>,
    pub child_items: Option<String>,
    pub sub_child_items: Option<String>,
    pub filter: Option<String>,
    /// Numeric `EMultipleLoad` value.
    pub multiple_load: Option<i32>,
    pub is_singbox_endpoint: Option<bool>,
    /// Opaque unknown-extension JSON, preserved for round-trip.
    pub extra_json: String,
}

/// `TransportExtraItem` (11 properties) exposed to the editor.
#[derive(Clone, Default)]
pub struct TransportExtraDto {
    pub raw_header_type: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
    pub xhttp_mode: Option<String>,
    pub xhttp_extra: Option<String>,
    pub grpc_authority: Option<String>,
    pub grpc_service_name: Option<String>,
    pub grpc_mode: Option<String>,
    pub kcp_header_type: Option<String>,
    pub kcp_seed: Option<String>,
    pub kcp_mtu: Option<i32>,
    /// Opaque unknown-extension JSON, preserved for round-trip.
    pub extra_json: String,
}

/// A profile row carrying every editable field.
#[derive(Clone)]
pub struct ProfileDto {
    pub index_id: String,
    pub config_type: ConfigType,
    pub core_type: Option<CoreType>,
    pub config_version: i32,
    pub subid: String,
    pub is_sub: bool,
    pub pre_socks_port: Option<i32>,
    pub display_log: bool,
    pub remarks: String,
    pub address: String,
    pub port: i32,
    pub password: String,
    pub username: String,
    pub network: String,
    pub mux_enabled: Option<bool>,
    pub finalmask: Option<String>,
    pub security: SecurityDto,
    pub proto_extra: ProtocolExtraDto,
    pub transport_extra: TransportExtraDto,
    /// Opaque unknown-extension JSON, preserved for round-trip.
    pub extra_json: String,
}

/// Result of a batch delete.
#[derive(Clone)]
pub struct DeleteProfilesResult {
    pub ok: bool,
    pub removed: u64,
    pub error: Option<ErrorDto>,
}

/// Result of a batch copy.
#[derive(Clone)]
pub struct CopyProfilesResult {
    pub ok: bool,
    pub copies: Vec<ProfileDto>,
    pub error: Option<ErrorDto>,
}

/// Result of a simple boolean mutation.
#[derive(Clone)]
pub struct SimpleResult {
    pub ok: bool,
    pub error: Option<ErrorDto>,
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

// -- T09 subscription + import/export DTOs ---------------------------------

/// The 17-field subscription model (`SubItem`).
#[derive(Clone)]
pub struct SubItemDto {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub more_url: String,
    pub enabled: bool,
    pub user_agent: String,
    pub request_headers: Option<String>,
    pub sort: i32,
    pub filter: Option<String>,
    pub auto_update_interval: i32,
    pub update_time: i64,
    pub convert_target: Option<String>,
    pub prev_profile: Option<String>,
    pub next_profile: Option<String>,
    pub pre_socks_port: Option<i32>,
    pub memo: Option<String>,
    pub custom_core_type: Option<i32>,
}

/// Result of a subscription mutation.
#[derive(Clone)]
pub struct SubItemDtoResult {
    pub ok: bool,
    pub item: Option<SubItemDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `list_sub_items`.
#[derive(Clone)]
pub struct SubsPageDto {
    pub items: Vec<SubItemDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `delete_sub_items`.
#[derive(Clone)]
pub struct DeleteSubsResult {
    pub ok: bool,
    pub removed: u64,
    pub error: Option<ErrorDto>,
}

/// One entry of a subscription update report.
#[derive(Clone)]
pub struct SubUpdateEntryDto {
    pub sub_id: String,
    pub remarks: String,
    /// `updated` / `preserved_empty` / `preserved_error` / `skipped` /
    /// `cancelled` / `failed`.
    pub status: String,
    pub added: Option<u32>,
    pub existing: Option<u32>,
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Result of `update_subscriptions`.
#[derive(Clone)]
pub struct SubUpdateResult {
    pub ok: bool,
    pub success: u32,
    pub cancelled: bool,
    pub entries: Vec<SubUpdateEntryDto>,
    pub job_id: Option<String>,
    pub error: Option<ErrorDto>,
}

/// A located per-item parse failure.
#[derive(Clone)]
pub struct ParseIssueDto {
    pub code: String,
    pub message: String,
    pub item_index: Option<u32>,
    pub byte_offset: Option<u64>,
}

/// Result of importing from clipboard text.
#[derive(Clone)]
pub struct ImportResult {
    pub ok: bool,
    pub imported: u32,
    pub profiles: Vec<ProfileDto>,
    pub errors: Vec<ParseIssueDto>,
    pub error: Option<ErrorDto>,
}

/// Result of parsing a single share URI.
#[derive(Clone)]
pub struct UriParseResult {
    pub ok: bool,
    pub profile: Option<ProfileDto>,
    pub error: Option<ErrorDto>,
}

/// Result of exporting profiles as share/base64/inner text.
#[derive(Clone)]
pub struct ShareExportResult {
    pub ok: bool,
    pub text: String,
    pub count: u32,
    pub error: Option<ErrorDto>,
}
