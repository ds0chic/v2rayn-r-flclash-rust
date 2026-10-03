//! FRB functions: snapshot, profile query/save, runtime apply/stop, cancel and
//! the event stream.
//!
//! T03 wires the runtime boundary to the real net-host client in production
//! builds; unit tests keep the in-memory engine so no process or pipe is
//! touched. The API surface is unchanged apart from the added `stop_runtime`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use application::{AppEngine, EventSink, PageRequest, ProfileFilter, ProfileSort};
use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
use domain::job::JobId;
use domain::revision::DesiredRevision;
#[cfg(test)]
use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, ProcessGraph,
    ProcessNode, RequiredPrivilege, RuntimePlan, RuntimeTarget,
};
#[cfg(test)]
use domain::CoreType;
use domain::{DomainError, MultipleLoad, Profile, ProtocolExtra, SecurityParams, TransportExtra};
use serde_json::Value;

use crate::api::contract::{
    ApplyRuntimeResult, CancelResult, CapabilityDto, CopyProfilesResult, DeleteProfilesResult,
    ErrorDto, EventEnvelopeDto, JobDto, ProfileDto, ProfileFilterDto, ProfilePageDto,
    ProfileSortDto, ProtocolExtraDto, RecoveryDto, SaveProfileResult, SecurityDto, SimpleResult,
    SnapshotDto, StopRuntimeResult, TransportExtraDto,
};

use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;

/// Reserved test-only smoke-session port. Never 10808 (the user's live
/// proxy). The production `apply_runtime` path does not use this; it derives
/// the inbound port from the persisted settings.
pub const TEST_SMOKE_PORT: u16 = 11808;

/// Global engine for the process. Tests use the in-memory engine; production
/// builds open real SQLite storage under the resolved data directory.
static ENGINE: OnceLock<AppEngine> = OnceLock::new();

/// Optional data-directory override set by [`init_engine`] before first use.
static ENGINE_DIR: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();

/// Serializes tests that touch the process-global [`ENGINE`].
///
/// Production keeps a single shared engine; tests must not run those cases
/// concurrently or the desired-revision counter races. Holding this lock is
/// the equivalent of running with `--test-threads=1` for the
/// engine-touching subset, without forcing the whole workspace
/// single-threaded.
#[cfg(test)]
pub(crate) fn engine_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static ENGINE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENGINE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn engine_dir() -> &'static Mutex<Option<std::path::PathBuf>> {
    ENGINE_DIR.get_or_init(|| Mutex::new(None))
}

pub(crate) fn engine() -> &'static AppEngine {
    ENGINE.get_or_init(|| {
        if cfg!(test) {
            return AppEngine::in_memory();
        }
        let dir = engine_dir()
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_else(AppEngine::default_data_dir);
        match AppEngine::open(&dir) {
            Ok(engine) => engine,
            Err(error) => {
                eprintln!("v2rayn-r: failed to open storage at {dir:?}: {error}");
                AppEngine::in_memory()
            }
        }
    })
}

/// Open the application engine against an explicit data directory (tests and
/// portable installs). Must be called before any other API for it to take
/// effect; later calls are a no-op once the engine is live.
#[frb(sync)]
pub fn init_engine(data_dir: Option<String>) -> SimpleResult {
    if ENGINE.get().is_some() {
        return SimpleResult {
            ok: true,
            error: None,
        };
    }
    if let Some(dir) = data_dir {
        if let Ok(mut guard) = engine_dir().lock() {
            *guard = Some(std::path::PathBuf::from(dir));
        }
    }
    engine();
    SimpleResult {
        ok: true,
        error: None,
    }
}

/// The resolved data directory currently in use (diagnostics/tests).
#[frb(sync)]
pub fn data_dir() -> String {
    engine_dir()
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| AppEngine::default_data_dir().to_string_lossy().into_owned())
}

/// Monotonic epoch for the in-process event stream.
static EVENT_EPOCH: OnceLock<AtomicU64> = OnceLock::new();
/// Sequence counter for the in-process event stream.
static EVENT_SEQ: OnceLock<AtomicU64> = OnceLock::new();

fn epoch_counter() -> &'static AtomicU64 {
    EVENT_EPOCH.get_or_init(|| AtomicU64::new(1))
}

fn seq_counter() -> &'static AtomicU64 {
    EVENT_SEQ.get_or_init(|| AtomicU64::new(0))
}

pub(crate) fn error_dto(e: DomainError) -> ErrorDto {
    ErrorDto::from(e)
}

fn json_map(raw: &str) -> domain::ExtraMap {
    serde_json::from_str(raw).unwrap_or_default()
}

fn security_to_dto(s: &SecurityParams) -> SecurityDto {
    SecurityDto {
        stream_security: s.stream_security.clone(),
        allow_insecure: s.allow_insecure.clone(),
        sni: s.sni.clone(),
        alpn: s.alpn.clone(),
        fingerprint: s.fingerprint.clone(),
        public_key: s.public_key.clone(),
        short_id: s.short_id.clone(),
        spider_x: s.spider_x.clone(),
        mldsa65_verify: s.mldsa65_verify.clone(),
        cert: s.cert.clone(),
        cert_sha: s.cert_sha.clone(),
        ech_config_list: s.ech_config_list.clone(),
        verify_peer_cert_by_name: s.verify_peer_cert_by_name.clone(),
    }
}

fn security_from_dto(d: SecurityDto) -> SecurityParams {
    SecurityParams {
        stream_security: d.stream_security,
        allow_insecure: d.allow_insecure,
        sni: d.sni,
        alpn: d.alpn,
        fingerprint: d.fingerprint,
        public_key: d.public_key,
        short_id: d.short_id,
        spider_x: d.spider_x,
        mldsa65_verify: d.mldsa65_verify,
        cert: d.cert,
        cert_sha: d.cert_sha,
        ech_config_list: d.ech_config_list,
        verify_peer_cert_by_name: d.verify_peer_cert_by_name,
    }
}

fn multiple_load_value(v: MultipleLoad) -> i32 {
    match v {
        MultipleLoad::LeastPing => 0,
        MultipleLoad::Fallback => 1,
        MultipleLoad::Random => 2,
        MultipleLoad::RoundRobin => 3,
        MultipleLoad::LeastLoad => 4,
    }
}

fn multiple_load_from_value(v: i32) -> Option<MultipleLoad> {
    Some(match v {
        0 => MultipleLoad::LeastPing,
        1 => MultipleLoad::Fallback,
        2 => MultipleLoad::Random,
        3 => MultipleLoad::RoundRobin,
        4 => MultipleLoad::LeastLoad,
        _ => return None,
    })
}

fn proto_to_dto(p: &ProtocolExtra) -> ProtocolExtraDto {
    ProtocolExtraDto {
        uot: p.uot,
        congestion_control: p.congestion_control.clone(),
        http_headers: p.http_headers.clone(),
        alter_id: p.alter_id.clone(),
        vmess_security: p.vmess_security.clone(),
        flow: p.flow.clone(),
        vless_encryption: p.vless_encryption.clone(),
        ss_method: p.ss_method.clone(),
        wg_public_key: p.wg_public_key.clone(),
        wg_preshared_key: p.wg_preshared_key.clone(),
        wg_interface_address: p.wg_interface_address.clone(),
        wg_reserved: p.wg_reserved.clone(),
        wg_mtu: p.wg_mtu,
        wg_dns: p.wg_dns.clone(),
        salamander_pass: p.salamander_pass.clone(),
        up_mbps: p.up_mbps,
        down_mbps: p.down_mbps,
        ports: p.ports.clone(),
        hop_interval: p.hop_interval.clone(),
        hy2_realm_url: p.hy2_realm_url.clone(),
        gecko_min_packet_size: p.gecko_min_packet_size.clone(),
        gecko_max_packet_size: p.gecko_max_packet_size.clone(),
        insecure_concurrency: p.insecure_concurrency,
        naive_quic: p.naive_quic,
        group_type: p.group_type.clone(),
        child_items: p.child_items.clone(),
        sub_child_items: p.sub_child_items.clone(),
        filter: p.filter.clone(),
        multiple_load: p.multiple_load.map(multiple_load_value),
        is_singbox_endpoint: p.is_singbox_endpoint,
        extra_json: serde_json::to_string(&p.extra).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn proto_from_dto(d: ProtocolExtraDto) -> ProtocolExtra {
    ProtocolExtra {
        uot: d.uot,
        congestion_control: d.congestion_control,
        http_headers: d.http_headers,
        alter_id: d.alter_id,
        vmess_security: d.vmess_security,
        flow: d.flow,
        vless_encryption: d.vless_encryption,
        ss_method: d.ss_method,
        wg_public_key: d.wg_public_key,
        wg_preshared_key: d.wg_preshared_key,
        wg_interface_address: d.wg_interface_address,
        wg_reserved: d.wg_reserved,
        wg_mtu: d.wg_mtu,
        wg_dns: d.wg_dns,
        salamander_pass: d.salamander_pass,
        up_mbps: d.up_mbps,
        down_mbps: d.down_mbps,
        ports: d.ports,
        hop_interval: d.hop_interval,
        hy2_realm_url: d.hy2_realm_url,
        gecko_min_packet_size: d.gecko_min_packet_size,
        gecko_max_packet_size: d.gecko_max_packet_size,
        insecure_concurrency: d.insecure_concurrency,
        naive_quic: d.naive_quic,
        group_type: d.group_type,
        child_items: d.child_items,
        sub_child_items: d.sub_child_items,
        filter: d.filter,
        multiple_load: d.multiple_load.and_then(multiple_load_from_value),
        is_singbox_endpoint: d.is_singbox_endpoint,
        extra: json_map(&d.extra_json),
    }
}

fn transport_to_dto(t: &TransportExtra) -> TransportExtraDto {
    TransportExtraDto {
        raw_header_type: t.raw_header_type.clone(),
        host: t.host.clone(),
        path: t.path.clone(),
        xhttp_mode: t.xhttp_mode.clone(),
        xhttp_extra: t.xhttp_extra.clone(),
        grpc_authority: t.grpc_authority.clone(),
        grpc_service_name: t.grpc_service_name.clone(),
        grpc_mode: t.grpc_mode.clone(),
        kcp_header_type: t.kcp_header_type.clone(),
        kcp_seed: t.kcp_seed.clone(),
        kcp_mtu: t.kcp_mtu,
        extra_json: serde_json::to_string(&t.extra).unwrap_or_else(|_| "{}".to_string()),
    }
}

fn transport_from_dto(d: TransportExtraDto) -> TransportExtra {
    TransportExtra {
        raw_header_type: d.raw_header_type,
        host: d.host,
        path: d.path,
        xhttp_mode: d.xhttp_mode,
        xhttp_extra: d.xhttp_extra,
        grpc_authority: d.grpc_authority,
        grpc_service_name: d.grpc_service_name,
        grpc_mode: d.grpc_mode,
        kcp_header_type: d.kcp_header_type,
        kcp_seed: d.kcp_seed,
        kcp_mtu: d.kcp_mtu,
        extra: json_map(&d.extra_json),
    }
}

fn profile_to_dto(p: Profile) -> ProfileDto {
    let extra_json = serde_json::to_string(&p.extra).unwrap_or_else(|_| "{}".to_string());
    ProfileDto {
        index_id: p.index_id,
        config_type: p.config_type,
        core_type: p.core_type,
        config_version: p.config_version,
        subid: p.subid,
        is_sub: p.is_sub,
        pre_socks_port: p.pre_socks_port,
        display_log: p.display_log,
        remarks: p.remarks,
        address: p.address,
        port: p.port,
        password: p.password,
        username: p.username,
        network: p.network,
        mux_enabled: p.mux_enabled,
        finalmask: p.finalmask,
        security: security_to_dto(&p.security),
        proto_extra: proto_to_dto(&p.proto_extra),
        transport_extra: transport_to_dto(&p.transport_extra),
        extra_json,
    }
}

fn dto_to_profile(d: ProfileDto) -> Profile {
    Profile {
        index_id: d.index_id,
        config_type: d.config_type,
        core_type: d.core_type,
        config_version: if d.config_version == 0 {
            4
        } else {
            d.config_version
        },
        subid: d.subid,
        is_sub: d.is_sub,
        pre_socks_port: d.pre_socks_port,
        display_log: d.display_log,
        remarks: d.remarks,
        address: d.address,
        port: d.port,
        password: d.password,
        username: d.username,
        network: d.network,
        mux_enabled: d.mux_enabled,
        finalmask: d.finalmask,
        security: security_from_dto(d.security),
        proto_extra: proto_from_dto(d.proto_extra),
        transport_extra: transport_from_dto(d.transport_extra),
        extra: json_map(&d.extra_json),
        ..Default::default()
    }
}

fn job_dto(j: application::JobView) -> JobDto {
    JobDto {
        job_id: j.job_id.0,
        kind: j.kind,
        state: j.state,
        percent: j.percent,
        stage_key: j.stage_key,
        error_code: j.error.as_ref().map(|e| e.code.clone()),
        error_message_key: j.error.as_ref().map(|e| e.message_key.clone()),
    }
}

fn snapshot_to_dto(s: application::Snapshot) -> SnapshotDto {
    SnapshotDto {
        desired_revision: s.revisions.desired.get(),
        applied_revision: s.revisions.applied.get(),
        revision_state: s.revision_state,
        runtime_state: s.runtime_state,
        host_alive: s.host_alive,
        runtime_pid: s.runtime_pid,
        runtime_created_at_ms: s.runtime_created_at_ms,
        runtime_ports: s.runtime_ports,
        runtime_session_id: s.runtime_session_id,
        runtime_config_sha256: s.runtime_config_sha256,
        runtime_operation_id: s.runtime_operation_id,
        runtime_error: s.runtime_error.map(ErrorDto::from),
        active_jobs: s.active_jobs.into_iter().map(job_dto).collect(),
        capabilities: s
            .capabilities
            .into_iter()
            .map(|c| CapabilityDto {
                core: c.core,
                config_types: c.config_types,
                structured_generation: c.structured_generation,
                update_supported: c.update_supported,
            })
            .collect(),
        recovery: RecoveryDto {
            recovery_needed: s.recovery.recovery_needed,
            stage: s.recovery.stage,
            restored: s.recovery.restored,
            pending: s.recovery.pending,
        },
        profile_count: s.profile_count,
    }
}

fn empty_snapshot_dto() -> SnapshotDto {
    SnapshotDto {
        desired_revision: 0,
        applied_revision: 0,
        revision_state: domain::RevisionState::Empty,
        runtime_state: domain::RuntimeState::Stopped,
        host_alive: false,
        runtime_pid: None,
        runtime_created_at_ms: None,
        runtime_ports: Vec::new(),
        runtime_session_id: None,
        runtime_config_sha256: None,
        runtime_operation_id: None,
        runtime_error: None,
        active_jobs: Vec::new(),
        capabilities: Vec::new(),
        recovery: RecoveryDto {
            recovery_needed: false,
            stage: None,
            restored: 0,
            pending: 0,
        },
        profile_count: 0,
    }
}

/// `get_snapshot` — settings version, runtime snapshot, active jobs,
/// capabilities and startup-recovery status.
///
/// The runtime facts come from net-host (or the in-memory client under test);
/// the fallback is an honest "host down / stopped" snapshot, never a fake
/// running state.
pub fn get_snapshot() -> SnapshotDto {
    match engine().snapshot() {
        Ok(s) => snapshot_to_dto(s),
        Err(_) => empty_snapshot_dto(),
    }
}

/// `query_profiles` — filter/sort/cursor/page_size with stable ids.
#[frb(sync)]
pub fn query_profiles(
    filter: ProfileFilterDto,
    sort: ProfileSortDto,
    cursor: u64,
    page_size: u32,
) -> ProfilePageDto {
    let filter = ProfileFilter {
        text: filter.text,
        config_types: filter.config_types,
        subid: filter.subid,
    };
    let sort = match sort {
        ProfileSortDto::Remarks => ProfileSort::Remarks,
        ProfileSortDto::Address => ProfileSort::Address,
        ProfileSortDto::Delay => ProfileSort::Delay,
        ProfileSortDto::IndexId => ProfileSort::IndexId,
    };
    let page = PageRequest {
        cursor: cursor as usize,
        page_size,
    };
    match engine().query_profiles(filter, sort, page) {
        Ok(p) => ProfilePageDto {
            items: p.items.into_iter().map(profile_to_dto).collect(),
            total: p.total as u64,
            next_cursor: p.next_cursor.map(|c| c as u64),
        },
        Err(_) => ProfilePageDto {
            items: Vec::new(),
            total: 0,
            next_cursor: None,
        },
    }
}

/// `save_profile` — draft + expected revision; returns the saved entity and
/// new revision or a field error.
#[frb(sync)]
pub fn save_profile(mut draft: ProfileDto, expected_revision: u64) -> SaveProfileResult {
    // New nodes arrive without an id; assign the stable identity here.
    if draft.index_id.trim().is_empty() {
        draft.index_id = application::new_index_id();
    }
    let profile = dto_to_profile(draft);
    match engine().save_profile(profile, DesiredRevision::new(expected_revision)) {
        Ok((saved, new_revision)) => SaveProfileResult {
            ok: true,
            profile: Some(profile_to_dto(saved)),
            new_revision: Some(new_revision.get()),
            error: None,
        },
        Err(e) => SaveProfileResult {
            ok: false,
            profile: None,
            new_revision: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `save_imported_profile` — import-pipeline persistence (FIX-04): accepts the
/// empty remarks/address/port that share URIs and inner `v2rayn://` payloads
/// legitimately carry, unlike the editor draft contract of `save_profile`.
#[frb(sync)]
pub fn save_imported_profile(mut draft: ProfileDto, expected_revision: u64) -> SaveProfileResult {
    if draft.index_id.trim().is_empty() {
        draft.index_id = application::new_index_id();
    }
    let profile = dto_to_profile(draft);
    match engine().save_imported_profile(profile, DesiredRevision::new(expected_revision)) {
        Ok((saved, new_revision)) => SaveProfileResult {
            ok: true,
            profile: Some(profile_to_dto(saved)),
            new_revision: Some(new_revision.get()),
            error: None,
        },
        Err(e) => SaveProfileResult {
            ok: false,
            profile: None,
            new_revision: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `delete_profiles` — delete a selection by stable id set.
#[frb(sync)]
pub fn delete_profiles(ids: Vec<String>) -> DeleteProfilesResult {
    match engine().delete_profiles(&ids) {
        Ok(removed) => DeleteProfilesResult {
            ok: true,
            removed,
            error: None,
        },
        Err(e) => DeleteProfilesResult {
            ok: false,
            removed: 0,
            error: Some(error_dto(e)),
        },
    }
}

/// `copy_profiles` — clone a selection with fresh ids and a "(副本)" suffix.
#[frb(sync)]
pub fn copy_profiles(ids: Vec<String>) -> CopyProfilesResult {
    match engine().copy_profiles(&ids) {
        Ok(copies) => CopyProfilesResult {
            ok: true,
            copies: copies.into_iter().map(profile_to_dto).collect(),
            error: None,
        },
        Err(e) => CopyProfilesResult {
            ok: false,
            copies: Vec::new(),
            error: Some(error_dto(e)),
        },
    }
}

/// `set_profile_remarks` — rename one profile in place.
#[frb(sync)]
pub fn set_profile_remarks(index_id: String, remarks: String) -> SaveProfileResult {
    match engine().set_remarks(&index_id, remarks) {
        Ok(saved) => SaveProfileResult {
            ok: true,
            profile: Some(profile_to_dto(saved)),
            new_revision: None,
            error: None,
        },
        Err(e) => SaveProfileResult {
            ok: false,
            profile: None,
            new_revision: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `set_active_profile` — persist the active node id (`None` clears it).
#[frb(sync)]
pub fn set_active_profile(index_id: Option<String>) -> SimpleResult {
    match engine().set_active(index_id) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(error_dto(e)),
        },
    }
}

/// `get_active_profile` — the persisted active node id, if any.
#[frb(sync)]
pub fn get_active_profile() -> Option<String> {
    engine().active_profile()
}

/// `get_profile` — full editable profile by stable id.
#[frb(sync)]
pub fn get_profile(index_id: String) -> Option<ProfileDto> {
    engine()
        .profile_by_id(&index_id)
        .ok()
        .flatten()
        .map(profile_to_dto)
}

/// `profile_revision` — current desired revision for optimistic saves.
#[frb(sync)]
pub fn profile_revision() -> u64 {
    engine().desired_revision()
}

/// Build the test-only T03 Xray smoke plan through the real structured
/// generator.
///
/// This is NOT the product path (the product path is `AppEngine::
/// build_runtime_plan`). It exists only so the T03 fault harness and the
/// generator smoke tests can exercise the structured generator with a fixed
/// `127.0.0.1:11808` inbound. Port 10808 is never emitted.
fn test_only_smoke_body() -> Result<String, DomainError> {
    use config_codegen::input::CodegenInput;

    let mut input = CodegenInput::default();
    input.profile.config_type = config_codegen::ConfigType::Socks;
    input.profile.address = "192.0.2.10".into();
    input.profile.port = 1080;
    input.profile.network = "tcp".into();
    input.settings.inbound.local_port = TEST_SMOKE_PORT as i32;
    input.settings.inbound.udp_enabled = true;
    input.settings.inbound.sniffing_enabled = true;

    let generated = config_codegen::generate_xray(&input).map_err(|e| {
        DomainError::new(domain::codes::INVALID_PLAN, "error.codegen_failed")
            .with_detail(e.to_string())
    })?;
    serde_json::to_string(&generated.main).map_err(|e| {
        DomainError::new(domain::codes::INTERNAL, "error.config_serialize_failed")
            .with_detail(e.to_string())
    })
}

/// Pretty-printed Xray smoke config (test-only evidence tool). Marked
/// `frb(ignore)` so it is not part of the Dart API surface.
#[frb(ignore)]
pub fn t18b_test_only_smoke_config_json() -> Result<String, DomainError> {
    let body = test_only_smoke_body()?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    Ok(serde_json::to_string_pretty(&value).unwrap_or(body))
}

/// Test-only plan builder around [`test_only_smoke_body`]. Not the product
/// path.
#[cfg(test)]
fn test_only_smoke_plan(
    target_id: &str,
    desired_revision: u64,
) -> Result<RuntimePlan, DomainError> {
    let body = test_only_smoke_body()?;
    let plan_id = format!("t03-smoke-{target_id}-{desired_revision}");
    Ok(build_test_plan(&body, &plan_id, desired_revision))
}

#[cfg(test)]
fn build_test_plan(body: &str, plan_id: &str, desired_revision: u64) -> RuntimePlan {
    let node = ProcessNode {
        id: "xray".into(),
        core_type: CoreType::Xray,
        config: ConfigSource::Inline {
            body: body.to_string(),
        },
        ports: vec![PortRequest::tcp(TEST_SMOKE_PORT, "inbound-socks")],
        privileges: vec![RequiredPrivilege::None],
    };
    let mut graph = ProcessGraph::default();
    graph.add_process(node);
    RuntimePlan {
        plan_id: plan_id.to_string(),
        desired_revision,
        target: RuntimeTarget {
            core_type: CoreType::Xray,
            version: None,
            config: ConfigSource::Inline {
                body: body.to_string(),
            },
            config_sha256: ContentHash::new(runtime::sha256_hex(body.as_bytes())),
        },
        process_graph: graph,
        outbound_graph: OutboundGraph::default(),
        ports: vec![PortRequest::tcp(TEST_SMOKE_PORT, "inbound-socks")],
        privileges: vec![RequiredPrivilege::None],
        network_policy: NetworkPolicy::default(),
        resources: vec![],
    }
}

/// `apply_runtime` — target id + expected revision; returns an operation id.
///
/// Builds the real plan from persisted state (active node / expanded policy
/// group + settings + routing + DNS + rule mode) through
/// `AppEngine::build_runtime_plan`. An empty `target_id` resolves to the
/// persisted active node; a missing target or a generator failure returns a
/// structured error instead of falling back to a hardcoded config.
pub fn apply_runtime(target_id: String, expected_revision: u64) -> ApplyRuntimeResult {
    let resolved = if target_id.trim().is_empty() {
        engine().active_profile()
    } else {
        Some(target_id)
    };
    let Some(target) = resolved else {
        return ApplyRuntimeResult {
            ok: false,
            operation_id: None,
            error: Some(error_dto(
                DomainError::new(domain::codes::FIELD_REQUIRED, "error.no_active_profile")
                    .with_field("target_id"),
            )),
        };
    };
    let plan = match engine().build_runtime_plan(&target, expected_revision) {
        Ok(plan) => plan,
        Err(error) => {
            return ApplyRuntimeResult {
                ok: false,
                operation_id: None,
                error: Some(error_dto(error)),
            };
        }
    };
    match engine().apply_runtime(plan, DesiredRevision::new(expected_revision)) {
        Ok(operation_id) => {
            emit_control(
                "job_progress",
                serde_json::json!({ "operation_id": operation_id }),
            );
            ApplyRuntimeResult {
                ok: true,
                operation_id: Some(operation_id),
                error: None,
            }
        }
        Err(e) => ApplyRuntimeResult {
            ok: false,
            operation_id: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `stop_runtime` — stop the managed core; idempotent.
pub fn stop_runtime() -> StopRuntimeResult {
    match engine().stop_runtime() {
        Ok(()) => StopRuntimeResult {
            ok: true,
            error: None,
        },
        Err(e) => StopRuntimeResult {
            ok: false,
            error: Some(error_dto(e)),
        },
    }
}

/// `cancel_job` — idempotent cancellation.
#[frb(sync)]
pub fn cancel_job(job_id: String) -> CancelResult {
    let outcome = engine().cancel_job(&JobId::new(job_id));
    CancelResult {
        outcome,
        error: None,
    }
}

/// Public alias of the profile DTO mapper for other API modules.
pub(crate) fn profile_dto(profile: Profile) -> ProfileDto {
    profile_to_dto(profile)
}

/// Public alias of the job DTO mapper for other API modules.
pub(crate) fn job_view_dto(job: application::JobView) -> JobDto {
    job_dto(job)
}

/// Build an event envelope with a fresh sequence.
fn next_event(kind: EventKind, payload: Value) -> EventEnvelope {
    let seq = seq_counter().fetch_add(1, Ordering::AcqRel) + 1;
    EventEnvelope::new(
        EventEpoch(epoch_counter().load(Ordering::Acquire)),
        EventSeq(seq),
        kind,
        payload,
    )
}

/// Emit a control event to any subscribers.
pub(crate) fn emit_control(kind: &str, payload: Value) {
    let env = next_event(EventKind::Other(kind.to_string()), payload);
    broadcast(&env);
}

// --- subscriber registry (bounded fan-out, control events never dropped) ---

type Sink = StreamSink<EventEnvelopeDto>;

static SUBSCRIBERS: OnceLock<Mutex<Vec<Sink>>> = OnceLock::new();
/// Guards one-time registration of the net-host event forwarder.
static RUNTIME_SUBSCRIPTION: OnceLock<()> = OnceLock::new();

fn subscribers() -> &'static Mutex<Vec<Sink>> {
    SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn envelope_to_dto(env: &EventEnvelope) -> EventEnvelopeDto {
    EventEnvelopeDto {
        epoch: env.epoch.get(),
        seq: env.seq.get(),
        kind: env.kind.as_str().to_string(),
        control: env.kind.is_control(),
        payload_json: env.payload.to_string(),
    }
}

/// Deliver an envelope to all live subscribers, dropping closed sinks.
fn broadcast(env: &EventEnvelope) {
    let dto = envelope_to_dto(env);
    let mut subs = match subscribers().lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    subs.retain(|sink| sink.add(dto.clone()).is_ok());
}

/// Start forwarding net-host events into the FRB stream exactly once.
fn ensure_runtime_subscription() {
    RUNTIME_SUBSCRIPTION.get_or_init(|| {
        let sink: EventSink = Arc::new(|env: EventEnvelope| {
            // Feed the T15a log pipeline before the generic event fan-out so
            // net-host `log_line` / `log_batch` events reach the log stream.
            crate::api::monitor::ingest_runtime_event(&env);
            broadcast(&env);
        });
        engine().subscribe_runtime_events(sink);
    });
}

/// `subscribe_events` — FRB stream of events. On subscribe the current
/// epoch/seq is sent first so a reconnecting UI can detect gaps.
pub fn subscribe_events(sink: StreamSink<EventEnvelopeDto>) {
    ensure_runtime_subscription();
    let header = EventEnvelope::new(
        EventEpoch(epoch_counter().load(Ordering::Acquire)),
        EventSeq(seq_counter().load(Ordering::Acquire)),
        EventKind::Other("stream_opened".to_string()),
        serde_json::json!({ "replay_from_seq": 0 }),
    );
    if sink.add(envelope_to_dto(&header)).is_err() {
        return;
    }
    if let Ok(mut subs) = subscribers().lock() {
        subs.push(sink);
    }
}

/// Test helper: emit a synthetic control event to the stream.
#[frb(sync)]
pub fn emit_test_event(kind: String, payload_json: String) {
    let payload: Value = serde_json::from_str(&payload_json).unwrap_or(Value::String(payload_json));
    emit_control(&kind, payload);
}

/// Number of active subscribers (diagnostics / tests).
#[frb(sync)]
pub fn subscriber_count() -> u32 {
    subscribers().lock().map(|s| s.len() as u32).unwrap_or(0)
}

/// Mark a job as running its compensating path (test helper for cancel flow).
#[frb(sync)]
pub fn mark_job_past_safe_point(job_id: String) {
    engine().jobs().mark_past_safe_point(&JobId::new(job_id));
}

/// Seed the in-memory engine with `count` synthetic profiles (bootstrap +
/// tests). Addresses are RFC5737 / example domains only.
#[frb(sync)]
pub fn seed_synthetic_profiles(count: u32) {
    let profiles: Vec<Profile> = (0..count)
        .map(application::synthetic::synthetic_full_profile)
        .collect();
    engine().seed(profiles);
}

/// Number of profiles currently held by the in-memory engine.
#[frb(sync)]
pub fn profile_count() -> u64 {
    engine().profile_count()
}

#[cfg(test)]
mod tests {
    use super::engine_test_lock;
    use super::*;

    #[test]
    fn test_only_smoke_plan_has_socks_inbound_and_freedom_outbound() {
        let plan = test_only_smoke_plan("smoke", 0).expect("generate smoke plan");
        let body = match &plan.target.config {
            ConfigSource::Inline { body } => body,
            other => panic!("expected inline config, got {other:?}"),
        };
        let value: Value = serde_json::from_str(body).unwrap();
        let inbound = &value["inbounds"][0];
        assert_eq!(inbound["listen"], "127.0.0.1");
        assert_eq!(inbound["port"], TEST_SMOKE_PORT);
        assert_eq!(inbound["tag"], "socks");
        let outbounds = value["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["protocol"] == "freedom"));
        assert!(!body.contains("10808"));
        // Hash matches the staged body.
        assert_eq!(
            plan.target.config_sha256.as_str(),
            runtime::sha256_hex(body.as_bytes())
        );
        assert_eq!(plan.ports[0].port, TEST_SMOKE_PORT);
    }

    #[test]
    fn snapshot_dto_serializes_with_contract_fields() {
        let _guard = engine_test_lock();
        seed_synthetic_profiles(3);
        let snap = get_snapshot();
        // The engine is process-global and serialized by `engine_test_lock`;
        // other engine-touching tests run before/after, never concurrently.
        assert!(snap.profile_count >= 3);
        assert_eq!(snap.applied_revision, 0);
        // Capabilities must be populated and never empty.
        assert!(!snap.capabilities.is_empty());
        // The snapshot is JSON-serializable with the expected field names.
        let json = snap_json_probe(&snap);
        assert!(json.get("revision_state").is_some());
        assert!(json.get("runtime_state").is_some());
        assert!(json.get("runtime_pid").is_some());
        assert!(json.get("profile_count").is_some());
    }

    #[test]
    fn save_imported_profile_accepts_empty_remarks_and_address() {
        let _guard = engine_test_lock();
        let mut draft = draft_dto();
        draft.index_id = String::new();
        draft.remarks = String::new();
        draft.address = String::new();
        draft.port = 0;
        let result = save_imported_profile(draft, profile_revision());
        assert!(
            result.ok,
            "import save rejected: {:?}",
            result.error.map(|e| e.code)
        );
        let saved = result.profile.expect("saved profile");
        assert!(!saved.index_id.is_empty(), "stable id must be assigned");
        assert_eq!(saved.remarks, "");
        assert!(get_profile(saved.index_id.clone()).is_some());

        // The editor draft contract still requires remarks.
        let mut editor = draft_dto();
        editor.index_id = String::new();
        editor.remarks = String::new();
        let rejected = save_profile(editor, profile_revision());
        assert!(!rejected.ok, "editor save must keep remarks required");
        assert_eq!(
            rejected.error.expect("editor error").code,
            "E_FIELD_REQUIRED"
        );
    }

    /// Mirror of the DTO into a serializable shape for the assertion above.
    /// FRB DTOs are not `Serialize` themselves (they are translated types), so
    /// the test builds the equivalent JSON from the contract fields.
    fn snap_json_probe(s: &SnapshotDto) -> Value {
        serde_json::json!({
            "desired_revision": s.desired_revision,
            "applied_revision": s.applied_revision,
            "revision_state": format!("{:?}", s.revision_state),
            "runtime_state": format!("{:?}", s.runtime_state),
            "runtime_pid": s.runtime_pid,
            "runtime_ports": s.runtime_ports,
            "profile_count": s.profile_count,
            "capabilities": s.capabilities.len(),
            "active_jobs": s.active_jobs.len(),
        })
    }

    fn draft_dto() -> ProfileDto {
        ProfileDto {
            index_id: "bridge-1".into(),
            config_type: domain::ConfigType::Vless,
            core_type: Some(domain::CoreType::Xray),
            config_version: 4,
            subid: String::new(),
            is_sub: true,
            pre_socks_port: None,
            display_log: true,
            remarks: "bridge".into(),
            address: "192.0.2.55".into(),
            port: 443,
            password: String::new(),
            username: String::new(),
            network: "raw".into(),
            mux_enabled: None,
            finalmask: None,
            security: SecurityDto {
                stream_security: Some("tls".into()),
                ..Default::default()
            },
            proto_extra: ProtocolExtraDto::default(),
            transport_extra: TransportExtraDto::default(),
            extra_json: "{}".into(),
        }
    }

    #[test]
    fn save_profile_rejects_stale_revision_through_bridge() {
        let _guard = engine_test_lock();
        let engine = engine();
        // A fresh id per run so reordered/retried runs never collide with a
        // row left behind by an earlier case.
        let mut draft = draft_dto();
        draft.index_id = format!(
            "bridge-stale-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        // Serialized by `engine_test_lock`: no concurrent bump can land
        // between the read and the save, so no retry loop is needed.
        let before = engine
            .snapshot()
            .map(|s| s.revisions.desired.get())
            .unwrap_or(0);
        let ok = save_profile(draft.clone(), before);
        assert!(ok.ok, "{:?}", ok.error.map(|e| e.code));
        // Reuse the now-stale revision.
        let stale = save_profile(draft, before);
        assert!(!stale.ok);
        assert_eq!(stale.error.unwrap().code, domain::codes::REVISION_STALE);
    }

    #[test]
    fn global_engine_tests_are_serialized_not_concurrent() {
        // Documents the F10 contract: any test touching the process-global
        // `ENGINE` must hold `engine_test_lock()`. Trying to lock it from
        // this test while it is free proves the lock exists and is usable;
        // production code paths are unaffected (single shared engine).
        let guard = engine_test_lock();
        let first = engine()
            .snapshot()
            .map(|s| s.revisions.desired.get())
            .unwrap_or(0);
        drop(guard);
        let _guard = engine_test_lock();
        let second = engine()
            .snapshot()
            .map(|s| s.revisions.desired.get())
            .unwrap_or(0);
        assert!(second >= first);
    }

    #[test]
    fn event_kind_str_is_stable() {
        assert_eq!(EventKind::LogBatch.as_str(), "log_batch");
        assert_eq!(EventKind::Other("custom".into()).as_str(), "custom");
    }
}
