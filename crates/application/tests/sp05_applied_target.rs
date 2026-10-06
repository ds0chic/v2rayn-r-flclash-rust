//! SP-05: frozen applied target and job terminal states (application half).
//!
//! Correct expectations (stable-port plan §3.1/§3.2, CP-03 / RUN-04 / RUN-05):
//!
//! - A is submitted (target/plan/revision frozen) and, while the apply is in
//!   flight, the user changes the desired default to B. The running core and
//!   the applied target must still report A; the desired default B is a
//!   separate identity and never relabels the accepted plan.
//! - a successful apply closes its job as Done while the session stays
//!   Running: no dangling active `apply_runtime` job, and the returned
//!   `"<operation_id>:<job_id>"` correlation resolves through the read-only
//!   `operation_status` path for both the raw and the compound form;
//! - a stop withdraws the live session but never rewrites the frozen history
//!   to B, and advances the actual generation even though desired is
//!   unchanged;
//! - reopen (independent process, same data dir) reports the same
//!   applied-vs-actual truth: history A is retained, live facts come only
//!   from the backend, and counters stay monotonic;
//! - `operation_id`/`intent_seq` order by admission; a stale submit is
//!   rejected with `REVISION_STALE` before touching the runtime.
//!
//! All fixtures are synthetic (TEST-NET addresses, no user secrets). The
//! gated fake binds no socket and spawns no core/helper/pipe; plans carry
//! probed ports >= 11808 (never 10808) like every other plan test.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

use application::runtime_client::{
    ApplyOutcome, OperationStatusView, RuntimeClient, RuntimeSnapshot,
};
use application::{job_state_for_operation, operation_state_name, AppEngine};
use domain::{
    AppliedRevision, ConfigType, DesiredRevision, DomainError, JobId, Profile, RuntimePlan,
    RuntimeState,
};
use ipc_contract::stable::OperationState as ReceiptState;

/// Gated fake runtime: the first apply blocks on a test-owned gate (the
/// in-flight A window), the backend facts are scripted afterwards.
struct GatedFake {
    calls: Mutex<Vec<String>>,
    apply_gate: Mutex<Option<mpsc::Receiver<()>>>,
    ops: Mutex<HashMap<String, OperationStatusView>>,
    next_op: AtomicUsize,
    state: Mutex<RuntimeState>,
    ports: Mutex<Vec<u16>>,
    session_id: Mutex<Option<String>>,
    applied_revision: Mutex<AppliedRevision>,
}

impl GatedFake {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            apply_gate: Mutex::new(None),
            ops: Mutex::new(HashMap::new()),
            next_op: AtomicUsize::new(0),
            state: Mutex::new(RuntimeState::Stopped),
            ports: Mutex::new(Vec::new()),
            session_id: Mutex::new(None),
            applied_revision: Mutex::new(AppliedRevision::ZERO),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn apply_count(&self) -> usize {
        self.next_op.load(Ordering::SeqCst)
    }

    /// Script the backend as a running core (test helper, no socket).
    fn mark_running(&self, session_id: &str, ports: Vec<u16>, revision: AppliedRevision) {
        *self.state.lock().unwrap() = RuntimeState::Running;
        *self.ports.lock().unwrap() = ports;
        *self.session_id.lock().unwrap() = Some(session_id.to_string());
        *self.applied_revision.lock().unwrap() = revision;
    }
}

impl RuntimeClient for GatedFake {
    fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
        Ok(RuntimeSnapshot {
            state: *self.state.lock().unwrap(),
            applied_revision: *self.applied_revision.lock().unwrap(),
            host_alive: true,
            pid: None,
            created_at_ms: None,
            ports: self.ports.lock().unwrap().clone(),
            session_id: self.session_id.lock().unwrap().clone(),
            config_sha256: None,
            operation_id: None,
            error: None,
            tun: None,
            actual_generation: 0,
            core_version: None,
            last_exit: None,
            last_exit_sidecar: None,
        })
    }

    fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
        self.ops
            .lock()
            .unwrap()
            .get(operation_id)
            .cloned()
            .ok_or_else(|| DomainError::not_found("operation", operation_id))
    }

    fn apply(&self, _plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError> {
        self.calls.lock().unwrap().push("apply".to_string());
        if let Some(rx) = self.apply_gate.lock().unwrap().take() {
            let _ = rx.recv_timeout(Duration::from_secs(15));
        }
        let op = format!("op-{}", self.next_op.fetch_add(1, Ordering::SeqCst) + 1);
        self.ops.lock().unwrap().insert(
            op.clone(),
            OperationStatusView {
                operation_id: op.clone(),
                job_id: None,
                state: domain::JobState::Running,
                cancel: None,
                error: None,
            },
        );
        Ok(ApplyOutcome::Accepted { operation_id: op })
    }

    fn stop(&self) -> Result<(), DomainError> {
        self.calls.lock().unwrap().push("stop".to_string());
        *self.state.lock().unwrap() = RuntimeState::Stopped;
        *self.ports.lock().unwrap() = Vec::new();
        *self.session_id.lock().unwrap() = None;
        Ok(())
    }

    fn cancel(&self, _job_id: &JobId) -> Result<domain::CancelOutcome, DomainError> {
        Ok(domain::CancelOutcome::NotCancellable)
    }
}

fn seed_nodes(engine: &AppEngine, ids: &[&str], active: &str) {
    let profiles: Vec<Profile> = ids
        .iter()
        .map(|id| Profile {
            index_id: id.to_string(),
            config_type: ConfigType::Vless,
            remarks: format!("synthetic-{id}"),
            address: "192.0.2.10".into(),
            port: 443,
            password: "11111111-2222-3333-4444-555555555555".into(),
            ..Default::default()
        })
        .collect();
    engine.seed(profiles);
    engine.set_active(Some(active.to_string())).unwrap();
}

/// Probe a free loopback port (>= 11808, never 10808) before use. The fake
/// binds nothing, but the plan generator refuses the reserved live proxy
/// port, so the fixture carries a probed synthetic port.
fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60000, "port scan exhausted");
        if base == 10808 {
            base += 1;
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

fn set_base_port(engine: &AppEngine, port: u16) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = port as i32;
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn frozen_plan(engine: &AppEngine, target: &str) -> (RuntimePlan, DesiredRevision) {
    let revision = engine.desired_revision();
    let plan = engine
        .build_runtime_plan(target, revision)
        .expect("synthetic plan must build");
    (plan, DesiredRevision::new(revision))
}

fn split_compound(correlation: &str) -> (String, String) {
    let (op, job) = correlation
        .split_once(':')
        .expect("apply returns <operation_id>:<job_id>");
    (op.to_string(), job.to_string())
}

#[test]
fn sp05_apply_a_then_default_b_still_reports_a() {
    // Baseline-compatible surface only (apply_runtime / applied_session):
    // on the pre-SP-05 engine this fails with applied B instead of A,
    // matching the audit `applied-target-race-contract.log` red contract.
    let fake = Arc::new(GatedFake::new());
    let engine = Arc::new(AppEngine::with_runtime(fake.clone()));
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    let port = free_port(11951);
    set_base_port(&engine, port);
    let (plan_a, revision) = frozen_plan(&engine, "node-a");

    // Hold A's apply open inside the runtime: the in-flight window during
    // which the user edits the desired default.
    let (release_tx, release_rx) = mpsc::channel::<()>();
    *fake.apply_gate.lock().unwrap() = Some(release_rx);
    let engine_clone = engine.clone();
    let apply_a = std::thread::spawn(move || {
        engine_clone
            .apply_runtime(plan_a, revision)
            .expect("apply A")
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fake.calls(), vec!["apply".to_string()]);

    // The user switches the desired default to B while A is in flight. This
    // only edits desired state; it must not relabel the accepted plan.
    engine.set_active(Some("node-b".to_string())).unwrap();

    release_tx.send(()).unwrap();
    let correlation = apply_a.join().expect("apply thread");
    assert!(correlation.starts_with("op-"));

    // The backend runs A's session now (frozen at submit, not re-read).
    fake.mark_running("s-sp05-a", vec![port], AppliedRevision::new(0));
    engine.snapshot().unwrap();

    let applied = engine
        .applied_session()
        .expect("a running session must publish its applied target");
    assert_eq!(
        applied.active_index_id.as_deref(),
        Some("node-a"),
        "a newer desired selection is not the accepted plan's applied target"
    );
    assert_eq!(applied.proxy_port, Some(port));

    // SP-17: the same snapshot also carries the actual descriptor: frozen
    // target + live facts, never the newly desired B.
    let snap = engine.snapshot().unwrap();
    let actual = snap.actual.expect("a running session has actual facts");
    assert_eq!(actual.target_profile_id.as_deref(), Some("node-a"));
    assert_eq!(actual.ready_endpoints, vec![port]);
    assert_eq!(actual.applied_runtime_revision, 0);
    assert_eq!(actual.actual_generation, 1, "first Running publish");
    let frozen = engine.applied_target().expect("frozen history");
    assert_eq!(
        actual.plan_hash.as_deref(),
        Some(frozen.config_sha256.as_str())
    );
    assert_eq!(actual.target_core.as_deref(), Some(frozen.core.as_str()));
}

#[test]
fn sp17_actual_view_is_absent_without_facts() {
    // A fresh engine with a stopped runtime has no actual fact at all: the
    // descriptor must be absent instead of an empty "nothing ran" record.
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::with_runtime(fake);
    let snap = engine.snapshot().unwrap();
    assert!(snap.actual.is_none());
}

#[test]
fn sp05_successful_apply_job_is_done_while_session_runs() {
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    let port = free_port(11953);
    set_base_port(&engine, port);
    let (plan_a, revision) = frozen_plan(&engine, "node-a");

    let correlation = engine
        .apply_runtime_for_target(plan_a, "node-a", revision)
        .expect("apply A");
    let (raw_op, job_id) = split_compound(&correlation);

    // The frozen history names A with this operation and the first intent.
    // The generation is still zero: the submit alone creates no facts.
    let frozen = engine.applied_target().expect("submit freezes a target");
    assert_eq!(frozen.target_profile_id, "node-a");
    assert_eq!(frozen.operation_id, raw_op);
    assert_eq!(frozen.intent_seq, 1);
    assert_eq!(engine.last_intent_seq(), 1);
    assert_eq!(engine.actual_generation(), 0);

    // Before Running there is no live session, only frozen history. The
    // idle backend reports Stopped with no error, so the open submit stays
    // pending (never failed, never cancelled by a mere read).
    engine.snapshot().unwrap();
    assert!(
        engine.applied_session().is_none(),
        "desired intent alone publishes no endpoint"
    );
    assert_eq!(
        engine.operation_status(&raw_op).unwrap().state,
        domain::JobState::Running,
        "a pending submit stays open until the backend proves an end"
    );

    fake.mark_running("s-sp05-a", vec![port], AppliedRevision::new(0));
    engine.snapshot().unwrap();
    engine.snapshot().unwrap();
    let applied = engine.applied_session().expect("running session published");
    assert_eq!(applied.active_index_id.as_deref(), Some("node-a"));

    // RUN-05: the accepted apply reached Running, so its job is Done while
    // the session stays Running — no dangling active apply job.
    let view = engine
        .operation_status(&correlation)
        .expect("compound correlation must resolve");
    assert_eq!(view.operation_id, raw_op);
    assert_eq!(view.job_id.as_deref(), Some(job_id.as_str()));
    assert_eq!(view.state, domain::JobState::Done);
    let raw_view = engine
        .operation_status(&raw_op)
        .expect("raw operation id must resolve");
    assert_eq!(raw_view.state, domain::JobState::Done);
    assert_eq!(raw_view.job_id.as_deref(), Some(job_id.as_str()));
    assert!(
        engine
            .jobs()
            .active()
            .iter()
            .all(|job| job.kind != "apply_runtime"),
        "a successful apply leaves no active apply job"
    );
    // One mapped vocabulary: the job Done reads as stable `succeeded`.
    assert_eq!(operation_state_name(view.state), "succeeded");
    assert_eq!(operation_state_name(domain::JobState::Running), "executing");
}

#[test]
fn sp05_stop_keeps_history_and_advances_generation() {
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    let port = free_port(11955);
    set_base_port(&engine, port);
    let (plan_a, revision) = frozen_plan(&engine, "node-a");
    let correlation = engine
        .apply_runtime_for_target(plan_a, "node-a", revision)
        .expect("apply A");
    let (raw_op, _) = split_compound(&correlation);
    fake.mark_running("s-sp05-a", vec![port], AppliedRevision::new(0));
    engine.snapshot().unwrap();
    assert_eq!(
        engine
            .applied_session()
            .expect("running")
            .active_index_id
            .as_deref(),
        Some("node-a")
    );
    let generation_running = engine.actual_generation();

    // Desired moves to B, then the user stops the core.
    engine.set_active(Some("node-b".to_string())).unwrap();
    engine.stop_runtime().expect("stop runs");

    // Live facts are withdrawn, history still names A, and the generation
    // advanced even though no new desired revision was applied.
    engine.snapshot().unwrap();
    assert!(
        engine.applied_session().is_none(),
        "stop withdraws the live endpoint"
    );
    let frozen = engine.applied_target().expect("history survives stop");
    assert_eq!(
        frozen.target_profile_id, "node-a",
        "stop must not rewrite history to the newer default B"
    );
    assert_eq!(frozen.operation_id, raw_op);
    assert!(
        engine.actual_generation() > generation_running,
        "exit/withdraw is an actual-fact transition"
    );
    // The completed apply job keeps its terminal state (never rewritten).
    assert_eq!(
        engine.operation_status(&raw_op).unwrap().state,
        domain::JobState::Done
    );
    // A fresh submit now starts a new intent; the old correlation is intact.
    assert_eq!(fake.calls().iter().filter(|c| *c == "stop").count(), 1);
}

#[test]
fn sp05_reopen_reports_same_applied_truth() {
    let dir = tempfile::tempdir().expect("temp data dir");
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::open_with_runtime(dir.path(), fake.clone()).expect("open engine");
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    let port = free_port(11957);
    set_base_port(&engine, port);
    let (plan_a, revision) = frozen_plan(&engine, "node-a");
    let correlation = engine
        .apply_runtime_for_target(plan_a, "node-a", revision)
        .expect("apply A");
    let (raw_op, _) = split_compound(&correlation);
    fake.mark_running("s-sp05-a", vec![port], AppliedRevision::new(0));
    engine.snapshot().unwrap();
    let generation_before = engine.actual_generation();
    let intent_before = engine.last_intent_seq();
    // Desired moves to B before the restart; applied history must stay A.
    engine.set_active(Some("node-b".to_string())).unwrap();
    drop(engine);

    // Independent reopen against the same data dir and a still-running
    // backend: history A reloads, live facts come from the backend only.
    let reopened = AppEngine::open_with_runtime(dir.path(), fake.clone()).expect("reopen engine");
    assert_eq!(
        reopened.active_profile().as_deref(),
        Some("node-b"),
        "desired default B persists independently"
    );
    let frozen = reopened.applied_target().expect("history reloads");
    assert_eq!(frozen.target_profile_id, "node-a");
    assert_eq!(frozen.operation_id, raw_op);
    assert_eq!(reopened.last_intent_seq(), intent_before);
    assert!(reopened.actual_generation() >= generation_before);
    reopened.snapshot().unwrap();
    assert_eq!(
        reopened
            .applied_session()
            .expect("backend still runs A's session")
            .active_index_id
            .as_deref(),
        Some("node-a"),
        "reopen applied and actual agree on A, not on desired B"
    );
}

#[test]
fn sp05_operation_ids_correlate_with_intent_order() {
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    set_base_port(&engine, free_port(11959));
    let (plan_a, revision) = frozen_plan(&engine, "node-a");
    let first = engine
        .apply_runtime_for_target(plan_a, "node-a", revision)
        .expect("apply A");
    engine.set_active(Some("node-b".to_string())).unwrap();
    let (plan_b, revision_b) = frozen_plan(&engine, "node-b");
    let second = engine
        .apply_runtime_for_target(plan_b, "node-b", revision_b)
        .expect("apply B");
    assert_ne!(first, second);

    let (op_a, _) = split_compound(&first);
    let (op_b, _) = split_compound(&second);
    let frozen = engine.applied_target().expect("latest submit wins history");
    assert_eq!(frozen.target_profile_id, "node-b");
    assert_eq!(frozen.operation_id, op_b);
    assert_eq!(frozen.intent_seq, 2);
    assert_eq!(engine.last_intent_seq(), 2);
    // Both correlations resolve; the engine never sends the compound form
    // to the runtime verbatim (each raw op is queryable on its own).
    assert!(engine.operation_status(&first).is_ok());
    assert!(engine.operation_status(&op_a).is_ok());
    assert!(engine.operation_status(&op_b).is_ok());
    assert!(engine.operation_status("op-9999").is_err());
    // The correlated job identity round-trips through the map.
    let job_b = engine.operation_job(&op_b).expect("correlated job");
    assert_eq!(
        engine.operation_status(&second).unwrap().job_id.as_deref(),
        Some(job_b.as_str())
    );
}

#[test]
fn sp05_stale_submit_never_switches() {
    let fake = Arc::new(GatedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_nodes(&engine, &["node-a", "node-b"], "node-a");
    set_base_port(&engine, free_port(11961));
    let (plan_a, _) = frozen_plan(&engine, "node-a");

    let error = engine
        .apply_runtime_for_target(plan_a, "node-a", DesiredRevision::new(999))
        .expect_err("stale revision must be rejected");
    assert_eq!(error.code, domain::codes::REVISION_STALE);
    assert!(
        fake.calls().is_empty(),
        "a stale submit must never touch the runtime"
    );
    assert_eq!(fake.apply_count(), 0);
    assert!(
        engine.applied_target().is_none(),
        "a rejected submit freezes nothing"
    );
    assert_eq!(engine.last_intent_seq(), 0);
    assert_eq!(engine.actual_generation(), 0);
}

#[test]
fn sp05_operation_vocabulary_matches_stable_wire() {
    // The single mapped vocabulary agrees with the frozen SP-00 wire names:
    // serialization of `stable::OperationState` is the same word the engine
    // helper returns for the corresponding job state.
    let wire = serde_json::to_string(&ReceiptState::Succeeded).unwrap();
    assert_eq!(wire, "\"succeeded\"");
    assert_eq!(operation_state_name(domain::JobState::Done), "succeeded");
    assert_eq!(
        job_state_for_operation("superseded"),
        Some(domain::JobState::Cancelled)
    );
    assert_eq!(
        job_state_for_operation("executing"),
        Some(domain::JobState::Running)
    );
    assert_eq!(job_state_for_operation("bogus"), None);
}
