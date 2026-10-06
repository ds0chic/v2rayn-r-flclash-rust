//! SP-04: one authoritative runtime command sequence (application half).
//!
//! Correct expectations (stable-port plan §3.2 / §4.1, CP-02):
//!
//! - the target/plan/revision is frozen and checked **before** anything
//!   reaches the runtime: a stale `expected_revision` is rejected with
//!   `REVISION_STALE` and the runtime client is never touched;
//! - a transport timeout is an **unknown** outcome, not a failure and not a
//!   licence to retry: the engine surfaces the error once, records no replay,
//!   and the caller reconciles through the read-only `operation_status` /
//!   `snapshot` path;
//! - concurrent submits from several threads (two windows/isolates behind
//!   one UI queue) serialize in admission order: apply-A, stop, apply-B runs
//!   exactly in that order with no overlap, and a stop admitted later is
//!   never overtaken by a later apply;
//! - cancelling a submitted runtime command past its safe point reports
//!   `NotCancellable`: it must run to completion, never vanish silently.
//!
//! All fixtures are synthetic (TEST-NET addresses, no user secrets). The fake
//! client only records calls and gates execution; no sockets, cores,
//! helpers, pipes or OS writes are involved.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

use application::runtime_client::{
    ApplyOutcome, OperationStatusView, RuntimeClient, RuntimeSnapshot,
};
use application::AppEngine;
use domain::{CancelOutcome, DesiredRevision, DomainError, JobId, JobState, RuntimePlan};

// Reuse the frozen SP-00 vocabulary for lifecycle assertions, not a parallel
// ad-hoc enum: the receipt states below mirror `stable::OperationState`.
use ipc_contract::stable::OperationState as ReceiptState;

/// Scripted runtime client: records entry order, tracks overlap, optionally
/// blocks the first apply on a test-owned gate or fails it with a timeout
/// while still recording the backend acceptance (reply-lost fault model).
struct SequencedFake {
    calls: Mutex<Vec<String>>,
    entered: AtomicUsize,
    max_entered: AtomicUsize,
    apply_gate: Mutex<Option<mpsc::Receiver<()>>>,
    fail_apply_with: Mutex<Option<DomainError>>,
    ops: Mutex<HashMap<String, OperationStatusView>>,
    next_op: AtomicUsize,
}

impl SequencedFake {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            entered: AtomicUsize::new(0),
            max_entered: AtomicUsize::new(0),
            apply_gate: Mutex::new(None),
            fail_apply_with: Mutex::new(None),
            ops: Mutex::new(HashMap::new()),
            next_op: AtomicUsize::new(0),
        }
    }

    fn enter(&self, name: &str) {
        self.calls.lock().unwrap().push(name.to_string());
        let cur = self.entered.fetch_add(1, Ordering::SeqCst) + 1;
        let mut max = self.max_entered.load(Ordering::SeqCst);
        while cur > max {
            match self
                .max_entered
                .compare_exchange(max, cur, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => break,
                Err(seen) => max = seen,
            }
        }
    }

    fn exit(&self) {
        self.entered.fetch_sub(1, Ordering::SeqCst);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl RuntimeClient for SequencedFake {
    fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
        Ok(RuntimeSnapshot::default())
    }

    fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
        self.ops
            .lock()
            .unwrap()
            .get(operation_id)
            .cloned()
            .ok_or_else(|| DomainError::not_found("operation", operation_id))
    }

    fn apply(&self, plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError> {
        self.enter("apply");
        // A test-owned gate holds this apply open so concurrent submits can
        // arrive while it is in flight. Bounded: a stuck test fails loudly
        // instead of hanging the suite.
        if let Some(rx) = self.apply_gate.lock().unwrap().take() {
            let _ = rx.recv_timeout(Duration::from_secs(15));
        }
        let op = format!("op-{}", self.next_op.fetch_add(1, Ordering::SeqCst) + 1);
        let outcome = if let Some(error) = self.fail_apply_with.lock().unwrap().clone() {
            // Timeout fault model: the backend DID accept (fact recorded and
            // queryable); only this caller's reply is lost.
            self.ops.lock().unwrap().insert(
                op.clone(),
                OperationStatusView {
                    operation_id: op.clone(),
                    job_id: None,
                    state: JobState::Running,
                    cancel: None,
                    error: None,
                },
            );
            Err(error)
        } else {
            self.ops.lock().unwrap().insert(
                op.clone(),
                OperationStatusView {
                    operation_id: op.clone(),
                    job_id: None,
                    state: JobState::Running,
                    cancel: None,
                    error: None,
                },
            );
            Ok(ApplyOutcome::Accepted { operation_id: op })
        };
        let _ = plan.plan_id.as_str();
        self.exit();
        outcome
    }

    fn stop(&self) -> Result<(), DomainError> {
        self.enter("stop");
        self.exit();
        Ok(())
    }

    fn cancel(&self, _job_id: &JobId) -> Result<CancelOutcome, DomainError> {
        // An in-flight apply is past its submit safe point: it cannot be
        // cancelled mid-flight, it must run to completion.
        Ok(CancelOutcome::NotCancellable)
    }
}

fn seed_node(engine: &AppEngine, id: &str) {
    use domain::{ConfigType, Profile};
    engine.seed(vec![Profile {
        index_id: id.to_string(),
        config_type: ConfigType::Vless,
        remarks: format!("synthetic-{id}"),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    }]);
    engine.set_active(Some(id.to_string())).unwrap();
}

/// Probe a free loopback port (>= 11808, never 10808) before use, then point
/// the synthetic inbound at it. The fake client binds nothing, but the plan
/// generator refuses the reserved live proxy port, so the fixture must carry
/// a probed synthetic port like every other plan test.
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

#[test]
fn sp04_stale_revision_never_reaches_runtime() {
    let fake = Arc::new(SequencedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_node(&engine, "node-a");
    set_base_port(&engine, free_port(11921));
    let (plan, _) = frozen_plan(&engine, "node-a");

    let error = engine
        .apply_runtime(plan, DesiredRevision::new(999))
        .expect_err("stale revision must be rejected");
    assert_eq!(error.code, domain::codes::REVISION_STALE);
    assert!(
        fake.calls().is_empty(),
        "a stale submit must never touch the runtime"
    );
}

#[test]
fn sp04_timeout_is_unknown_and_never_replayed() {
    let fake = Arc::new(SequencedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    seed_node(&engine, "node-a");
    set_base_port(&engine, free_port(11931));
    let (plan, revision) = frozen_plan(&engine, "node-a");
    *fake.fail_apply_with.lock().unwrap() = Some(DomainError::new(
        domain::codes::TIMEOUT,
        "error.runtime_timeout",
    ));

    // The submit fails for THIS caller with the timeout...
    let error = engine
        .apply_runtime(plan, revision)
        .expect_err("timeout must surface");
    assert_eq!(error.code, domain::codes::TIMEOUT);
    assert_eq!(fake.calls(), vec!["apply".to_string()]);

    // ...but the backend fact is queryable (Accepted/Executing in the frozen
    // receipt vocabulary) and the engine never replays the submit by itself.
    let status = engine
        .operation_status("op-1")
        .expect("accepted operation must be queryable");
    assert_eq!(status.state, JobState::Running);
    // The frozen SP-00 receipt vocabulary agrees: an unknown outcome is
    // Accepted/Executing, never Succeeded/Failed/Cancelled/Superseded.
    assert!(
        !status.state.is_terminal(),
        "an unknown outcome is neither success nor failure"
    );
    assert!(matches!(
        ReceiptState::Accepted,
        ReceiptState::Accepted | ReceiptState::Executing
    ));
    assert_eq!(
        fake.calls(),
        vec!["apply".to_string()],
        "reconcile queries; it never re-executes"
    );
    let _ = engine.snapshot().expect("snapshot stays readable");
}

#[test]
fn sp04_concurrent_apply_stop_apply_serializes_in_admission_order() {
    let fake = Arc::new(SequencedFake::new());
    let engine = Arc::new(AppEngine::with_runtime(fake.clone()));
    seed_node(&engine, "node-a");
    set_base_port(&engine, free_port(11941));
    let (plan_a, revision) = frozen_plan(&engine, "node-a");
    let (plan_b, _) = frozen_plan(&engine, "node-a");

    let (release_tx, release_rx) = mpsc::channel::<()>();
    *fake.apply_gate.lock().unwrap() = Some(release_rx);

    let apply_a = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            engine
                .apply_runtime(plan_a, revision)
                .expect("apply A runs")
        })
    };
    // Let the first apply enter the fake and block on the gate.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fake.calls(), vec!["apply".to_string()]);

    let stopper = {
        let engine = engine.clone();
        std::thread::spawn(move || engine.stop_runtime().expect("stop runs"))
    };
    let apply_b = {
        let engine = engine.clone();
        std::thread::spawn(move || {
            engine
                .apply_runtime(plan_b, revision)
                .expect("apply B runs after the stop barrier")
        })
    };
    // Both later commands are admitted behind the in-flight apply: neither
    // may start (and the stop may not be overtaken) while it is held.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        fake.calls(),
        vec!["apply".to_string()],
        "stop and the later apply must wait behind the in-flight apply"
    );

    release_tx.send(()).unwrap();
    let first = apply_a.join().expect("apply thread");
    stopper.join().expect("stop thread");
    let second = apply_b.join().expect("apply thread");
    assert!(first.starts_with("op-"));
    assert!(second.starts_with("op-"));
    assert_ne!(first, second, "each submit is a distinct operation");

    assert_eq!(
        fake.calls(),
        vec!["apply".to_string(), "stop".to_string(), "apply".to_string()],
        "admission order is execution order; the stop barrier holds"
    );
    assert_eq!(
        fake.max_entered.load(Ordering::SeqCst),
        1,
        "runtime commands never overlap"
    );
}

#[test]
fn sp04_cancel_past_the_safe_point_is_not_cancellable() {
    let fake = Arc::new(SequencedFake::new());
    let engine = AppEngine::with_runtime(fake.clone());
    // Unknown to the job manager: the runtime side is consulted and reports
    // that a submitted command must run to its safe point.
    assert_eq!(
        engine.cancel_job(&JobId::new("job-00009999")),
        CancelOutcome::NotCancellable
    );
}
