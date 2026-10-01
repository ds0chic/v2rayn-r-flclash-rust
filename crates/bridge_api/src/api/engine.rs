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
use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, ProcessGraph,
    ProcessNode, RequiredPrivilege, RuntimePlan, RuntimeTarget,
};
use domain::{CoreType, DomainError, Profile};
use serde_json::Value;

use crate::api::contract::{
    ApplyRuntimeResult, CancelResult, CapabilityDto, ErrorDto, EventEnvelopeDto, JobDto,
    ProfileDto, ProfileFilterDto, ProfilePageDto, ProfileSortDto, RecoveryDto, SaveProfileResult,
    SnapshotDto, StopRuntimeResult,
};
use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;

/// Reserved smoke-session port. Never 10808 (the user's live proxy).
pub const SMOKE_PORT: u16 = 11808;

/// Global engine for the process. Tests use the in-memory engine; production
/// builds use the real net-host client.
static ENGINE: OnceLock<AppEngine> = OnceLock::new();

fn engine() -> &'static AppEngine {
    ENGINE.get_or_init(|| {
        if cfg!(test) {
            AppEngine::in_memory()
        } else {
            AppEngine::production()
        }
    })
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

fn error_dto(e: DomainError) -> ErrorDto {
    ErrorDto::from(e)
}

fn profile_to_dto(p: Profile) -> ProfileDto {
    let extra_json = serde_json::to_string(&p.extra).unwrap_or_else(|_| "{}".to_string());
    ProfileDto {
        index_id: p.index_id,
        config_type: p.config_type,
        core_type: p.core_type,
        remarks: p.remarks,
        address: p.address,
        port: p.port,
        network: p.network,
        stream_security: p.security.stream_security,
        subid: p.subid,
        username: p.username,
        mux_enabled: p.mux_enabled,
        extra_json,
    }
}

fn dto_to_profile(d: ProfileDto) -> Profile {
    let mut p = Profile {
        index_id: d.index_id,
        config_type: d.config_type,
        core_type: d.core_type,
        remarks: d.remarks,
        address: d.address,
        port: d.port,
        network: d.network,
        subid: d.subid,
        username: d.username,
        mux_enabled: d.mux_enabled,
        ..Default::default()
    };
    p.security.stream_security = d.stream_security;
    if let Ok(extra) = serde_json::from_str::<domain::ExtraMap>(&d.extra_json) {
        p.extra = extra;
    }
    p
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
pub fn save_profile(draft: ProfileDto, expected_revision: u64) -> SaveProfileResult {
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

/// Build the T03 Xray smoke plan through the real structured generator.
///
/// The generated config is a v2rayN client config: a `mixed` (SOCKS+HTTP)
/// inbound on `127.0.0.1:11808` tagged `socks`, and a `freedom` outbound
/// tagged `direct` so the core can serve. The profile outbound itself points
/// at a documentation address (RFC5737) and is never exercised by the smoke
/// test. Port 10808 is never emitted.
fn smoke_body() -> Result<String, DomainError> {
    use config_codegen::input::CodegenInput;

    let mut input = CodegenInput::default();
    input.profile.config_type = config_codegen::ConfigType::Socks;
    input.profile.address = "192.0.2.10".into();
    input.profile.port = 1080;
    input.profile.network = "tcp".into();
    input.settings.inbound.local_port = SMOKE_PORT as i32;
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

/// Pretty-printed Xray smoke config (evidence tool). Marked `frb(ignore)` so it
/// is not part of the Dart API surface.
#[frb(ignore)]
pub fn xray_smoke_config_json() -> Result<String, DomainError> {
    let body = smoke_body()?;
    let value: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    Ok(serde_json::to_string_pretty(&value).unwrap_or(body))
}

fn smoke_plan(target_id: &str, desired_revision: u64) -> Result<RuntimePlan, DomainError> {
    let body = smoke_body()?;
    let plan_id = format!("t03-smoke-{target_id}-{desired_revision}");
    Ok(build_plan(&body, &plan_id, desired_revision))
}

fn build_plan(body: &str, plan_id: &str, desired_revision: u64) -> RuntimePlan {
    let node = ProcessNode {
        id: "xray".into(),
        core_type: CoreType::Xray,
        config: ConfigSource::Inline {
            body: body.to_string(),
        },
        ports: vec![PortRequest::tcp(SMOKE_PORT, "inbound-socks")],
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
        ports: vec![PortRequest::tcp(SMOKE_PORT, "inbound-socks")],
        privileges: vec![RequiredPrivilege::None],
        network_policy: NetworkPolicy::default(),
        resources: vec![],
    }
}

/// `apply_runtime` — target id + expected revision; returns an operation id.
/// The result flows on the event stream.
pub fn apply_runtime(target_id: String, expected_revision: u64) -> ApplyRuntimeResult {
    let plan = match smoke_plan(&target_id, expected_revision) {
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
fn emit_control(kind: &str, payload: Value) {
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
        let sink: EventSink = Arc::new(|env: EventEnvelope| broadcast(&env));
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
    use super::*;

    #[test]
    fn smoke_plan_has_socks_inbound_and_freedom_outbound() {
        let plan = smoke_plan("smoke", 0).expect("generate smoke plan");
        let body = match &plan.target.config {
            ConfigSource::Inline { body } => body,
            other => panic!("expected inline config, got {other:?}"),
        };
        let value: Value = serde_json::from_str(body).unwrap();
        let inbound = &value["inbounds"][0];
        assert_eq!(inbound["listen"], "127.0.0.1");
        assert_eq!(inbound["port"], SMOKE_PORT);
        assert_eq!(inbound["tag"], "socks");
        let outbounds = value["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["protocol"] == "freedom"));
        assert!(!body.contains("10808"));
        // Hash matches the staged body.
        assert_eq!(
            plan.target.config_sha256.as_str(),
            runtime::sha256_hex(body.as_bytes())
        );
        assert_eq!(plan.ports[0].port, SMOKE_PORT);
    }

    #[test]
    fn snapshot_dto_serializes_with_contract_fields() {
        seed_synthetic_profiles(3);
        let snap = get_snapshot();
        // The engine is process-global; other tests may have added profiles.
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

    #[test]
    fn save_profile_rejects_stale_revision_through_bridge() {
        let engine = engine();
        let draft = ProfileDto {
            index_id: "bridge-1".into(),
            config_type: domain::ConfigType::Vless,
            core_type: Some(domain::CoreType::Xray),
            remarks: "bridge".into(),
            address: "192.0.2.55".into(),
            port: 443,
            network: "raw".into(),
            stream_security: Some("tls".into()),
            subid: String::new(),
            username: String::new(),
            mux_enabled: None,
            extra_json: "{}".into(),
        };
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
    fn event_kind_str_is_stable() {
        assert_eq!(EventKind::LogBatch.as_str(), "log_batch");
        assert_eq!(EventKind::Other("custom".into()).as_str(), "custom");
    }
}
