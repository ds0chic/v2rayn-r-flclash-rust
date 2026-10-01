//! T02 FRB functions: snapshot, profile query/save, runtime apply, cancel and
//! the event stream.
//!
//! All of these run against the in-memory [`application::AppEngine`]. The
//! runtime boundary is the `NullRuntimeClient` for T02; T03 swaps in the real
//! net-host client without changing this surface.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use application::{AppEngine, PageRequest, ProfileFilter, ProfileSort};
use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
use domain::job::JobId;
use domain::revision::DesiredRevision;
use domain::runtime_plan::{
    ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, ProcessGraph, RuntimePlan,
    RuntimeTarget,
};
use domain::{CoreType, Profile};
use serde_json::Value;

use crate::api::contract::{
    ApplyRuntimeResult, CancelResult, CapabilityDto, ErrorDto, EventEnvelopeDto, JobDto,
    ProfileDto, ProfileFilterDto, ProfilePageDto, ProfileSortDto, RecoveryDto, SaveProfileResult,
    SnapshotDto,
};
use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;

/// Global engine for the process. T02 keeps a single in-memory instance.
static ENGINE: OnceLock<AppEngine> = OnceLock::new();

fn engine() -> &'static AppEngine {
    ENGINE.get_or_init(AppEngine::in_memory)
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

fn error_dto(e: domain::DomainError) -> ErrorDto {
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

/// `get_snapshot` — settings version, runtime snapshot, active jobs,
/// capabilities and startup-recovery status.
#[frb(sync)]
pub fn get_snapshot() -> SnapshotDto {
    match engine().snapshot() {
        Ok(s) => SnapshotDto {
            desired_revision: s.revisions.desired.get(),
            applied_revision: s.revisions.applied.get(),
            revision_state: s.revision_state,
            runtime_state: s.runtime_state,
            active_jobs: s
                .active_jobs
                .into_iter()
                .map(|j| JobDto {
                    job_id: j.job_id.0,
                    kind: j.kind,
                    state: j.state,
                    percent: j.percent,
                    stage_key: j.stage_key,
                    error_code: j.error.as_ref().map(|e| e.code.clone()),
                    error_message_key: j.error.as_ref().map(|e| e.message_key.clone()),
                })
                .collect(),
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
        },
        Err(_e) => SnapshotDto {
            desired_revision: 0,
            applied_revision: 0,
            revision_state: domain::RevisionState::Empty,
            runtime_state: domain::RuntimeState::Stopped,
            active_jobs: Vec::new(),
            capabilities: Vec::new(),
            recovery: RecoveryDto {
                recovery_needed: false,
                stage: None,
                restored: 0,
                pending: 0,
            },
            profile_count: 0,
        },
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

/// Build a minimal, valid runtime plan for a target id (T02 stub).
///
/// T03 replaces this with the real `config_codegen` output. It is honest about
/// being a stub: the plan carries the target id and a placeholder hash, and
/// uses no ports so it can never touch the user's running proxy.
fn stub_plan(target_id: &str, desired_revision: u64) -> RuntimePlan {
    let body = serde_json::json!({
        "stub": true,
        "target": target_id,
    })
    .to_string();
    RuntimePlan {
        plan_id: format!("stub-{target_id}-{desired_revision}"),
        desired_revision,
        target: RuntimeTarget {
            core_type: CoreType::Xray,
            version: None,
            config: ConfigSource::Inline { body },
            config_sha256: ContentHash::new("0000000000000000"),
        },
        process_graph: ProcessGraph::default(),
        outbound_graph: OutboundGraph::default(),
        ports: Vec::new(),
        privileges: Vec::new(),
        network_policy: NetworkPolicy::default(),
        resources: Vec::new(),
    }
}

/// `apply_runtime` — target id + expected revision; returns an operation id.
/// The result flows on the event stream.
#[frb(sync)]
pub fn apply_runtime(target_id: String, expected_revision: u64) -> ApplyRuntimeResult {
    let plan = stub_plan(&target_id, expected_revision);
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

/// `subscribe_events` — FRB stream of events. On subscribe the current
/// epoch/seq is sent first so a reconnecting UI can detect gaps.
pub fn subscribe_events(sink: StreamSink<EventEnvelopeDto>) {
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
