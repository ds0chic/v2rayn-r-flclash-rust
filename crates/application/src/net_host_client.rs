//! Real `RuntimeClient` backed by net-host over a Windows named pipe (T03).
//!
//! I/O model (important on Windows): a synchronous pipe handle is serialized
//! per *FileObject*, and duplicated handles share the same FileObject, so a
//! blocking reader and writer on cloned handles deadlock. This client never
//! does that:
//!
//! - **control requests** run on their own short-lived connection, writing the
//!   request and reading the reply on the *same* handle in one thread; the
//!   caller bounds the wait with `recv_timeout`;
//! - **events** use one long-lived connection whose single thread subscribes,
//!   then only reads. Keeping it open also keeps the runtime lease alive;
//! - no thread ever waits without a deadline on the caller side.
//!
//! Client liveness is the connection itself (a killed GUI closes its handle),
//! so no application-level heartbeat is required.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use domain::event::{EventEnvelope, EventEpoch};
use domain::{AppliedRevision, CancelOutcome, DomainError, JobId, RuntimePlan};
use ipc_contract::{
    IpcOperation, IpcResult, RequestEnvelope, RuntimeSnapshot as IpcSnapshot, SessionIdentity,
    IPC_APPLY_TIMEOUT_MS, IPC_PROTOCOL_VERSION, IPC_REQUEST_TIMEOUT_MS,
};
use runtime::{
    current_identity, decode_payload, encode_frame, frame_len, frame_len_ok, RuntimeDetail,
    ServerFrame, LEN_PREFIX_BYTES, NET_HOST_PIPE_NAME, RUNTIME_DETAIL_EVENT,
};

use crate::runtime_client::{
    ApplyOutcome, EventSink, ExitFact, OperationStatusView, RuntimeClient, RuntimeSnapshot,
    TunStatus,
};

/// How long to wait for a freshly launched net-host to accept a connection.
const LAUNCH_WAIT: Duration = Duration::from_secs(10);
/// Poll interval while waiting for the pipe.
const LAUNCH_POLL: Duration = Duration::from_millis(100);
/// Reconnect backoff for the event connection.
const EVENT_RECONNECT: Duration = Duration::from_millis(500);

/// Transport signal asking a lagged subscriber to resynchronize. Mirrors
/// `net_host::events::RESYNC_REQUIRED_EVENT`; the payload carries the
/// authoritative `reason`/`epoch`/`last_seq`. Never a lifecycle outcome.
const RESYNC_REQUIRED_EVENT: &str = "resync_required";

/// Authoritative resync request derived from the event stream (SP-07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResyncAsk {
    pub reason: String,
    pub epoch: u64,
    pub last_seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CursorOutcome {
    pub resync: Option<ResyncAsk>,
    /// Whether the event may advance lifecycle/final-outcome state.
    /// Telemetry, duplicates and transport signals never do.
    pub lifecycle: bool,
}

/// Subscription cursor: expected `(epoch, next_seq)` plus the newest
/// attributed fact generation. A gap, epoch change or explicit resync notice
/// yields a `ResyncAsk` (fetch authoritative snapshot, then `adopt_origin`);
/// the cursor holds position until that snapshot arrives, so a stale event
/// can never rewind generation or overwrite a final outcome.
#[derive(Debug)]
pub(crate) struct EventCursor {
    epoch: u64,
    next_seq: u64,
    generation: Option<u64>,
    resyncs: u64,
}

impl EventCursor {
    pub(crate) fn new(epoch: u64, next_seq: u64) -> Self {
        Self {
            epoch,
            next_seq,
            generation: None,
            resyncs: 0,
        }
    }

    pub(crate) fn next_seq(&self) -> u64 {
        self.next_seq
    }

    pub(crate) fn generation(&self) -> Option<u64> {
        self.generation
    }

    /// Advance the cursor to an authoritative snapshot origin.
    pub(crate) fn adopt_origin(&mut self, epoch: u64, next_seq: u64) {
        self.epoch = epoch;
        self.next_seq = next_seq;
    }

    pub(crate) fn observe(&mut self, event: &EventEnvelope) -> CursorOutcome {
        if event.kind.as_str() == RESYNC_REQUIRED_EVENT {
            self.resyncs += 1;
            let reason = event
                .payload
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("lagged");
            let epoch = event
                .payload
                .get("epoch")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| event.epoch.get());
            let last_seq = event
                .payload
                .get("last_seq")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_else(|| event.seq.get());
            return CursorOutcome {
                resync: Some(ResyncAsk {
                    reason: reason.to_string(),
                    epoch,
                    last_seq,
                }),
                lifecycle: false,
            };
        }
        if event.epoch.get() != self.epoch {
            self.resyncs += 1;
            return CursorOutcome {
                resync: Some(ResyncAsk {
                    reason: "epoch_mismatch".to_string(),
                    epoch: event.epoch.get(),
                    last_seq: event.seq.get().saturating_sub(1),
                }),
                lifecycle: false,
            };
        }
        let seq = event.seq.get();
        if seq < self.next_seq {
            return CursorOutcome {
                resync: None,
                lifecycle: false,
            };
        }
        if seq > self.next_seq {
            self.resyncs += 1;
            return CursorOutcome {
                resync: Some(ResyncAsk {
                    reason: "gap".to_string(),
                    epoch: self.epoch,
                    last_seq: seq.saturating_sub(1),
                }),
                lifecycle: false,
            };
        }
        self.next_seq = seq.saturating_add(1);
        let (_, generation) = parse_event_identity(event);
        if generation.is_some() {
            self.generation = generation;
        }
        CursorOutcome {
            resync: None,
            lifecycle: event.kind.is_control(),
        }
    }
}

/// Lifecycle latch: only control events may advance the retained outcome.
/// High-frequency telemetry increments a counter and can never evict the
/// final result; transport signals are not outcomes either.
#[derive(Debug, Default)]
pub(crate) struct LifecycleLatch {
    last_control: Option<EventEnvelope>,
    telemetry_seen: u64,
}

impl LifecycleLatch {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn note(&mut self, event: &EventEnvelope) {
        if event.kind.as_str() == RESYNC_REQUIRED_EVENT {
            return;
        }
        if event.kind.is_control() {
            self.last_control = Some(event.clone());
        } else {
            self.telemetry_seen = self.telemetry_seen.saturating_add(1);
        }
    }

    pub(crate) fn last_control(&self) -> Option<EventEnvelope> {
        self.last_control.clone()
    }

    pub(crate) fn telemetry_seen(&self) -> u64 {
        self.telemetry_seen
    }
}

/// Read the `(operation_id, generation)` control identity from an envelope
/// payload. Mirrors `net_host::events::parse_event_identity`.
pub(crate) fn parse_event_identity(event: &EventEnvelope) -> (Option<String>, Option<u64>) {
    let operation_id = event
        .payload
        .get("operation_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let generation = event
        .payload
        .get("generation")
        .and_then(serde_json::Value::as_u64);
    (operation_id, generation)
}

fn trace(message: impl AsRef<str>) {
    if std::env::var_os("V2RAYN_R_TRACE").is_some() {
        eprintln!("[net-host-client] {}", message.as_ref());
    }
}

/// Serializes control requests and bounds them to one outstanding worker.
///
/// The previous design held a plain `Mutex` only on the caller's stack: on
/// timeout the caller released it while its blocking reader thread stayed
/// alive, so repeated timeouts accumulated workers (D07). This gate keeps the
/// slot owned until the worker actually reports back, and acquisition itself is
/// bounded by the request deadline, so queue time counts toward the total.
struct RequestGate {
    busy: Mutex<bool>,
    cv: Condvar,
    /// Workers currently inside `run_request` (observable, never > 1).
    active_workers: AtomicUsize,
    /// Cumulative workers spawned (observable; a leak shows as growth).
    spawned_workers: AtomicUsize,
}

impl RequestGate {
    fn new() -> Self {
        Self {
            busy: Mutex::new(false),
            cv: Condvar::new(),
            active_workers: AtomicUsize::new(0),
            spawned_workers: AtomicUsize::new(0),
        }
    }

    /// Take the single request slot or fail once `deadline` passes. A previous
    /// timed-out worker that has not finished still owns the slot, so no second
    /// worker is started behind it.
    fn acquire(&self, deadline: Instant) -> Result<(), DomainError> {
        let mut busy = self
            .busy
            .lock()
            .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
        while *busy {
            let now = Instant::now();
            if now >= deadline {
                return Err(timeout_error("queued request"));
            }
            let (guard, _) = self
                .cv
                .wait_timeout(busy, deadline - now)
                .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
            busy = guard;
        }
        *busy = true;
        Ok(())
    }

    fn record_spawn(&self) {
        self.spawned_workers.fetch_add(1, Ordering::AcqRel);
        self.active_workers.fetch_add(1, Ordering::AcqRel);
    }

    /// Release the slot after the worker finished (or never started).
    fn release(&self) {
        if let Ok(mut busy) = self.busy.lock() {
            *busy = false;
            self.cv.notify_one();
        }
    }

    fn record_finish(&self) {
        self.active_workers.fetch_sub(1, Ordering::AcqRel);
        self.release();
    }

    fn active_workers(&self) -> usize {
        self.active_workers.load(Ordering::Acquire)
    }

    fn spawned_workers(&self) -> usize {
        self.spawned_workers.load(Ordering::Acquire)
    }
}

struct Shared {
    pipe_name: String,
    auto_launch: bool,
    /// Managed cores root forwarded to a launched net-host as
    /// `V2RAYN_R_CORES_ROOT`, so install (`UpdateService`) and run
    /// (`CoreLocator`) share one root with no user-set environment.
    cores_root: Mutex<Option<PathBuf>>,
    /// Serializes control requests and owns the single in-flight worker.
    gate: RequestGate,
    /// Serializes net-host launches across the event and control paths.
    launch_mutex: Mutex<()>,
    sink: Arc<Mutex<Option<EventSink>>>,
    events_started: AtomicBool,
    events_stop: AtomicBool,
    session: SessionIdentity,
    request_seq: AtomicU64,
}

/// Production runtime client. Construct once per process and share it.
pub struct NetHostClient {
    shared: Arc<Shared>,
}

impl Default for NetHostClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NetHostClient {
    pub fn new() -> Self {
        Self::with_pipe(
            std::env::var("V2RAYN_R_PIPE").unwrap_or_else(|_| NET_HOST_PIPE_NAME.to_string()),
        )
    }

    pub fn with_pipe(pipe_name: impl Into<String>) -> Self {
        let id = current_identity();
        let session = SessionIdentity {
            protocol_version: IPC_PROTOCOL_VERSION,
            session_token: format!("v2rayn-{}", id.pid),
            peer_pid: id.pid,
            peer_created_at_ms: id.created_at_ms,
        };
        Self {
            shared: Arc::new(Shared {
                pipe_name: pipe_name.into(),
                auto_launch: std::env::var_os("V2RAYN_R_NO_AUTOLAUNCH").is_none(),
                cores_root: Mutex::new(None),
                gate: RequestGate::new(),
                launch_mutex: Mutex::new(()),
                sink: Arc::new(Mutex::new(None)),
                events_started: AtomicBool::new(false),
                events_stop: AtomicBool::new(false),
                session,
                request_seq: AtomicU64::new(0),
            }),
        }
    }

    /// Build a client that forwards `cores_root` to every net-host it launches.
    pub fn with_cores_root(pipe_name: impl Into<String>, cores_root: impl Into<PathBuf>) -> Self {
        let client = Self::with_pipe(pipe_name);
        client.set_cores_root(cores_root);
        client
    }

    /// Set the managed cores root forwarded as `V2RAYN_R_CORES_ROOT` on launch.
    /// Call before the first runtime request; later calls affect only later
    /// launches.
    pub fn set_cores_root(&self, cores_root: impl Into<PathBuf>) {
        if let Ok(mut slot) = self.shared.cores_root.lock() {
            *slot = Some(cores_root.into());
        }
    }

    /// The configured pipe name (diagnostics/tests).
    pub fn pipe_name(&self) -> &str {
        &self.shared.pipe_name
    }

    /// The managed cores root forwarded to a launched net-host, if configured.
    pub fn cores_root(&self) -> Option<PathBuf> {
        self.shared
            .cores_root
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
    }

    /// `(active, spawned)` control-request workers. `active` never exceeds one
    /// by construction; `spawned` is cumulative so a timeout leak is visible.
    pub fn request_worker_counts(&self) -> (usize, usize) {
        (
            self.shared.gate.active_workers(),
            self.shared.gate.spawned_workers(),
        )
    }

    fn request(
        &self,
        operation: IpcOperation,
        timeout: Duration,
    ) -> Result<(IpcResult, RuntimeDetail), DomainError> {
        do_request(&self.shared, operation, timeout)
    }

    /// Open a restricted temporary test core, isolated from the managed
    /// runtime. Returns the net-host test-session id.
    pub fn open_test_session(
        &self,
        plan: &RuntimePlan,
        max_duration_ms: u64,
    ) -> Result<String, DomainError> {
        let (result, _) = self.request(
            IpcOperation::TestSession(ipc_contract::TestSessionOperation::Open {
                plan: Box::new(plan.clone()),
                max_duration_ms,
            }),
            Duration::from_millis(IPC_APPLY_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::TestSession { session_id, .. } => Ok(session_id),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("test_session_open", &other)),
        }
    }

    /// Close a restricted test session (idempotent).
    pub fn close_test_session(&self, session_id: &str) -> Result<(), DomainError> {
        let (result, _) = self.request(
            IpcOperation::TestSession(ipc_contract::TestSessionOperation::Close {
                session_id: session_id.to_string(),
            }),
            Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::TestSession { .. } => Ok(()),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("test_session_close", &other)),
        }
    }
}

impl Drop for NetHostClient {
    fn drop(&mut self) {
        self.shared.events_stop.store(true, Ordering::Release);
    }
}

impl RuntimeClient for NetHostClient {
    fn snapshot(&self) -> Result<RuntimeSnapshot, DomainError> {
        let (result, detail) = self.request(
            IpcOperation::GetSnapshot,
            Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::Snapshot(snapshot) => Ok(map_snapshot(&snapshot, &detail)),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("snapshot", &other)),
        }
    }

    fn operation_status(&self, operation_id: &str) -> Result<OperationStatusView, DomainError> {
        let (result, _detail) = self.request(
            IpcOperation::GetOperation {
                operation_id: operation_id.to_string(),
            },
            Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::Operation(status) => Ok(OperationStatusView {
                operation_id: status.operation_id,
                job_id: status.job_id.map(|id| id.0),
                state: status.state,
                cancel: status.cancel,
                error: status.error,
            }),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("operation_status", &other)),
        }
    }

    fn apply(&self, plan: &RuntimePlan) -> Result<ApplyOutcome, DomainError> {
        let (result, _detail) = self.request(
            IpcOperation::ApplyPlan {
                plan: Box::new(plan.clone()),
            },
            Duration::from_millis(IPC_APPLY_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::Accepted { operation_id } => Ok(ApplyOutcome::Accepted { operation_id }),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("apply", &other)),
        }
    }

    fn stop(&self) -> Result<(), DomainError> {
        let (result, _detail) = self.request(
            IpcOperation::StopRuntime { operation_id: None },
            Duration::from_millis(IPC_APPLY_TIMEOUT_MS),
        )?;
        match result {
            IpcResult::Stopped => Ok(()),
            IpcResult::Error(error) => Err(error),
            other => Err(unexpected("stop", &other)),
        }
    }

    fn cancel(&self, _job_id: &JobId) -> Result<CancelOutcome, DomainError> {
        // net-host has no arbitrary cancel operation; an in-flight apply is
        // bounded by the readiness timeout and cannot be stopped mid-flight.
        Ok(CancelOutcome::NotCancellable)
    }

    fn subscribe_events(&self, sink: EventSink) {
        if let Ok(mut slot) = self.shared.sink.lock() {
            *slot = Some(sink);
        }
        start_event_thread(&self.shared);
    }
}

fn map_snapshot(snapshot: &IpcSnapshot, detail: &RuntimeDetail) -> RuntimeSnapshot {
    RuntimeSnapshot {
        state: snapshot.state,
        applied_revision: AppliedRevision::new(snapshot.applied_revision),
        host_alive: snapshot.host_alive,
        pid: detail.pid,
        created_at_ms: detail.created_at_ms,
        ports: detail.ports.clone(),
        session_id: detail.session_id.clone(),
        config_sha256: detail.config_sha256.clone(),
        operation_id: snapshot
            .active_operation
            .clone()
            .or_else(|| detail.operation_id.clone()),
        error: detail.error.clone(),
        // Redacted TUN lease facts flow through untouched: `None` (no lease)
        // must stay `None` so the UI renders "not enabled", never active.
        tun: detail.tun.as_ref().map(|tun| TunStatus {
            adapter_name: tun.adapter_name.clone(),
            interface_index: tun.interface_index,
            route_count: tun.route_count,
            dry_run: tun.dry_run,
        }),
        // SP-17: actual facts flow through untouched; a missing exit stays
        // `None` instead of being synthesized from the desired plan.
        actual_generation: detail.actual_generation,
        core_version: detail.core_version.clone(),
        last_exit: detail.last_exit.as_ref().map(|exit| ExitFact {
            pid: exit.pid,
            exit_code: exit.exit_code,
            at_ms: exit.at_ms,
        }),
        last_exit_sidecar: detail.last_exit_sidecar.clone(),
    }
}

fn do_request(
    shared: &Arc<Shared>,
    operation: IpcOperation,
    timeout: Duration,
) -> Result<(IpcResult, RuntimeDetail), DomainError> {
    // The queue wait and the response wait share one deadline (D07): a request
    // stuck behind another command cannot spend `timeout` twice.
    let deadline = Instant::now() + timeout;
    shared.gate.acquire(deadline)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        shared.gate.release();
        return Err(timeout_error("request"));
    }
    trace("do_request: dispatching");
    let (tx, rx) = mpsc::channel();
    let worker = Arc::clone(shared);
    shared.gate.record_spawn();
    let spawn = std::thread::Builder::new()
        .name("net-host-request".into())
        .spawn(move || {
            let result = run_request(&worker, operation);
            let _ = tx.send(result);
            // Release the slot only when the (possibly blocking) worker is
            // truly done, so a timed-out caller never lets a second worker in.
            worker.gate.record_finish();
        });
    if let Err(e) = spawn {
        shared.gate.record_finish();
        return Err(unavailable(format!("spawn request thread failed: {e}")));
    }

    match rx.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(timeout_error("request")),
        Err(RecvTimeoutError::Disconnected) => Err(unavailable("net-host request thread died")),
    }
}

fn run_request(
    shared: &Arc<Shared>,
    operation: IpcOperation,
) -> Result<(IpcResult, RuntimeDetail), DomainError> {
    let mut file = connect(shared)?;
    let request = RequestEnvelope {
        session: shared.session.clone(),
        request_id: format!(
            "app-{}-{}",
            shared.session.peer_pid,
            shared.request_seq.fetch_add(1, Ordering::AcqRel) + 1
        ),
        operation,
    };
    let bytes =
        encode_frame(&request).map_err(|e| unavailable(format!("encode request failed: {e}")))?;
    file.write_all(&bytes)
        .and_then(|_| file.flush())
        .map_err(|e| unavailable(format!("pipe write failed: {e}")))?;
    trace("run_request: request written");

    let mut detail = RuntimeDetail::default();
    loop {
        let payload =
            read_payload(&mut file).map_err(|e| unavailable(format!("pipe read failed: {e}")))?;
        match decode_payload::<ServerFrame>(&payload)
            .map_err(|e| unavailable(format!("decode frame failed: {e}")))?
        {
            ServerFrame::Response(envelope) => return Ok((envelope.result, detail)),
            ServerFrame::Event(event) => {
                if event.kind.as_str() == RUNTIME_DETAIL_EVENT {
                    if let Ok(parsed) = serde_json::from_value::<RuntimeDetail>(event.payload) {
                        detail = parsed;
                    }
                }
            }
        }
    }
}

fn start_event_thread(shared: &Arc<Shared>) {
    if shared.events_started.swap(true, Ordering::AcqRel) {
        return;
    }
    let shared = Arc::clone(shared);
    let _ = std::thread::Builder::new()
        .name("net-host-events".into())
        .spawn(move || event_loop(&shared));
}

fn event_loop(shared: &Arc<Shared>) {
    let mut cursor: Option<EventCursor> = None;
    let mut latch = LifecycleLatch::new();
    while !shared.events_stop.load(Ordering::Acquire) {
        match connect(shared) {
            Ok(mut file) => {
                trace("event_loop: connected");
                let (epoch, from_seq) = match subscribe(shared, &mut file) {
                    Ok(origin) => origin,
                    Err(_) => {
                        std::thread::sleep(EVENT_RECONNECT);
                        continue;
                    }
                };
                // Re-anchor on every (re)subscribe: a disconnect never
                // resumes from a stale cursor without a snapshot round-trip.
                match cursor.as_mut() {
                    Some(cursor) => cursor.adopt_origin(epoch, from_seq),
                    None => cursor = Some(EventCursor::new(epoch, from_seq)),
                }
                while !shared.events_stop.load(Ordering::Acquire) {
                    let payload = match read_payload(&mut file) {
                        Ok(payload) => payload,
                        Err(e) => {
                            trace(format!("event_loop: read ended: {e}"));
                            break;
                        }
                    };
                    let frame = match decode_payload::<ServerFrame>(&payload) {
                        Ok(frame) => frame,
                        Err(_) => break,
                    };
                    if let ServerFrame::Event(event) = frame {
                        deliver(shared, &event);
                        if let Some(cursor) = cursor.as_mut() {
                            latch.note(&event);
                            let outcome = cursor.observe(&event);
                            if let Some(ask) = outcome.resync {
                                log_resync(cursor, &latch, &ask);
                                if let Some((epoch, next)) = reconcile_after_resync(shared, &ask) {
                                    cursor.adopt_origin(epoch, next);
                                }
                            }
                        }
                    }
                }
                // The read loop ended (disconnect or a server-closed resync
                // subscription): re-anchor via snapshot before resubscribing,
                // so no silent half-open resumes from a stale cursor.
                if let Some(cursor) = cursor.as_mut() {
                    let ask = ResyncAsk {
                        reason: "reconnect".to_string(),
                        epoch,
                        last_seq: from_seq.saturating_sub(1),
                    };
                    if let Some((epoch, next)) = reconcile_after_resync(shared, &ask) {
                        cursor.adopt_origin(epoch, next);
                    }
                }
            }
            Err(e) => trace(format!("event_loop: connect failed: {e}")),
        }
        if shared.events_stop.load(Ordering::Acquire) {
            break;
        }
        std::thread::sleep(EVENT_RECONNECT);
    }
}

fn deliver(shared: &Arc<Shared>, event: &EventEnvelope) {
    let callback = shared.sink.lock().ok().and_then(|slot| slot.clone());
    if let Some(callback) = callback {
        callback(event.clone());
    }
}

fn log_resync(cursor: &EventCursor, latch: &LifecycleLatch, ask: &ResyncAsk) {
    let (kept_op, kept_gen) = latch
        .last_control()
        .as_ref()
        .map(parse_event_identity)
        .unwrap_or((None, None));
    trace(format!(
        "resync {}: want epoch={} last_seq={}; cursor next={} gen={:?}; kept op={:?} gen={:?} telemetry={}",
        ask.reason,
        ask.epoch,
        ask.last_seq,
        cursor.next_seq(),
        cursor.generation(),
        kept_op,
        kept_gen,
        latch.telemetry_seen(),
    ));
}

/// Fetch the authoritative snapshot after a lag/disconnect resync ask.
/// Returns the origin the subscription must continue from.
fn reconcile_after_resync(shared: &Arc<Shared>, ask: &ResyncAsk) -> Option<(u64, u64)> {
    match do_request(
        shared,
        IpcOperation::GetSnapshot,
        Duration::from_millis(IPC_REQUEST_TIMEOUT_MS),
    ) {
        Ok((IpcResult::Snapshot(snapshot), _)) => {
            trace(format!(
                "resync {} answered: epoch={} last_seq={}",
                ask.reason,
                snapshot.epoch.get(),
                snapshot.last_seq
            ));
            Some((snapshot.epoch.get(), snapshot.last_seq.saturating_add(1)))
        }
        _ => None,
    }
}

fn subscribe(shared: &Arc<Shared>, file: &mut File) -> Result<(u64, u64), DomainError> {
    let request = RequestEnvelope {
        session: shared.session.clone(),
        request_id: format!("sub-{}", shared.session.peer_pid),
        operation: IpcOperation::SubscribeEvents {
            epoch: EventEpoch(0),
            from_seq: 0,
        },
    };
    let bytes =
        encode_frame(&request).map_err(|e| unavailable(format!("encode subscribe failed: {e}")))?;
    file.write_all(&bytes)
        .and_then(|_| file.flush())
        .map_err(|e| unavailable(format!("subscribe write failed: {e}")))?;
    // Read until the EventStreamOpened acknowledgment on the same handle. A
    // resync notice may precede it; deliver it like any other event.
    loop {
        let payload =
            read_payload(file).map_err(|e| unavailable(format!("subscribe read failed: {e}")))?;
        let frame = decode_payload::<ServerFrame>(&payload)
            .map_err(|e| unavailable(format!("subscribe decode failed: {e}")))?;
        match frame {
            ServerFrame::Response(envelope) => match envelope.result {
                IpcResult::EventStreamOpened { epoch, from_seq } => {
                    return Ok((epoch.get(), from_seq));
                }
                IpcResult::Error(error) => return Err(error),
                other => return Err(unexpected("subscribe", &other)),
            },
            ServerFrame::Event(event) => deliver(shared, &event),
        }
    }
}

fn connect(shared: &Arc<Shared>) -> Result<File, DomainError> {
    if let Ok(file) = open_pipe(&shared.pipe_name) {
        return Ok(file);
    }
    if !shared.auto_launch {
        return Err(unavailable("net-host is not running"));
    }
    let _guard = shared
        .launch_mutex
        .lock()
        .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
    // Re-check: another thread may have launched while we waited for the lock.
    if let Ok(file) = open_pipe(&shared.pipe_name) {
        return Ok(file);
    }
    trace("connect: launching net_host");
    launch_net_host(shared)?;
    let deadline = Instant::now() + LAUNCH_WAIT;
    loop {
        if let Ok(file) = open_pipe(&shared.pipe_name) {
            trace("connect: connected after launch");
            return Ok(file);
        }
        if Instant::now() >= deadline {
            return Err(unavailable(
                "net-host did not accept a connection within 10s",
            ));
        }
        std::thread::sleep(LAUNCH_POLL);
    }
}

fn open_pipe(pipe_name: &str) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(true).open(pipe_name)
}

fn read_payload<R: Read>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    let mut prefix = [0u8; LEN_PREFIX_BYTES];
    reader.read_exact(&mut prefix)?;
    let len = frame_len(prefix);
    if !frame_len_ok(len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len}"),
        ));
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

/// Forward the engine's managed cores root to a launched net-host.
///
/// The engine's cores root is the single source of truth: install
/// ([`UpdateService`](crate::UpdateService)) and run (`CoreLocator`) must agree
/// with no user-set environment. An explicit `V2RAYN_R_CORES_ROOT` already in
/// this process wins over the client field.
fn apply_launch_env(command: &mut std::process::Command, shared: &Arc<Shared>) {
    let cores_root = shared
        .cores_root
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
        .or_else(|| std::env::var_os("V2RAYN_R_CORES_ROOT").map(PathBuf::from));
    if let Some(root) = cores_root {
        command.env("V2RAYN_R_CORES_ROOT", root);
    }
}

fn launch_net_host(shared: &Arc<Shared>) -> Result<(), DomainError> {
    let exe = locate_net_host().ok_or_else(|| {
        unavailable("net_host.exe not found (set V2RAYN_R_NET_HOST or build the workspace)")
    })?;
    let mut command = std::process::Command::new(&exe);
    apply_launch_env(&mut command, shared);
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map_err(|e| unavailable(format!("failed to launch {}: {e}", exe.display())))?;
    Ok(())
}

fn locate_net_host() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("V2RAYN_R_NET_HOST") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("net_host.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            starts.push(dir.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for start in starts {
        let mut current: Option<&std::path::Path> = Some(start.as_path());
        let mut depth = 0;
        while let Some(dir) = current {
            for profile in ["debug", "release"] {
                let candidate = dir.join("target").join(profile).join("net_host.exe");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            current = dir.parent();
            depth += 1;
            if depth > 8 {
                break;
            }
        }
    }
    None
}

fn timeout_error(what: &str) -> DomainError {
    DomainError::new(domain::codes::TIMEOUT, "error.runtime_timeout")
        .with_detail(format!("{what} timed out; net-host may be unresponsive"))
}

fn unavailable(detail: impl Into<String>) -> DomainError {
    DomainError::new(domain::codes::UNAVAILABLE, "error.runtime_unavailable")
        .with_detail(detail.into())
}

fn unexpected(operation: &str, result: &IpcResult) -> DomainError {
    let kind = match result {
        IpcResult::Snapshot(_) => "snapshot",
        IpcResult::Accepted { .. } => "accepted",
        IpcResult::Operation(_) => "operation",
        IpcResult::EventStreamOpened { .. } => "event_stream_opened",
        IpcResult::Stopped => "stopped",
        IpcResult::Shutdown => "shutdown",
        IpcResult::TestSession { .. } => "test_session",
        IpcResult::Error(_) => "error",
    };
    DomainError::new(domain::codes::INTERNAL, "error.runtime_unexpected_reply")
        .with_detail(format!("{operation} received an unexpected `{kind}` reply"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::event::EventEpoch;
    use domain::RuntimeState;
    use runtime::RuntimeTunDetail;

    fn ipc_snapshot() -> IpcSnapshot {
        IpcSnapshot {
            state: RuntimeState::Running,
            applied_revision: 7,
            epoch: EventEpoch(1),
            last_seq: 2,
            active_operation: None,
            recovery: None,
            host_alive: true,
        }
    }

    #[test]
    fn tun_detail_maps_into_snapshot_status() {
        let detail = RuntimeDetail {
            state: RuntimeState::Running,
            applied_revision: 7,
            pid: Some(4242),
            created_at_ms: Some(1_700_000_000_000),
            ports: vec![11808],
            session_id: Some("s1".into()),
            config_sha256: Some("ab".into()),
            operation_id: Some("op1".into()),
            error: None,
            tun: Some(RuntimeTunDetail {
                adapter_name: "v2rayn-tun".into(),
                interface_index: 9,
                route_count: 2,
                dry_run: true,
            }),
            actual_generation: 3,
            core_version: Some("25.9.1".into()),
            last_exit: None,
            last_exit_sidecar: None,
        };
        let snap = map_snapshot(&ipc_snapshot(), &detail);
        assert_eq!(snap.actual_generation, 3);
        assert_eq!(snap.core_version.as_deref(), Some("25.9.1"));
        assert!(snap.last_exit.is_none());
        let tun = snap.tun.expect("tun lease must flow into the snapshot");
        assert_eq!(tun.adapter_name, "v2rayn-tun");
        assert_eq!(tun.interface_index, 9);
        assert_eq!(tun.route_count, 2);
        assert!(tun.dry_run);
        assert_eq!(snap.pid, Some(4242));
    }

    #[test]
    fn missing_tun_detail_stays_missing() {
        let detail = RuntimeDetail::default();
        let snap = map_snapshot(&ipc_snapshot(), &detail);
        assert!(snap.tun.is_none(), "no lease must never become a fake TUN");
    }

    #[test]
    fn launch_env_forwards_configured_cores_root() {
        let client =
            NetHostClient::with_cores_root("\\\\.\\pipe\\rr01-test", "C:\\tmp\\data\\cores");
        assert_eq!(
            client.cores_root().as_deref(),
            Some(std::path::Path::new("C:\\tmp\\data\\cores"))
        );
        let mut command = std::process::Command::new("net_host.exe");
        apply_launch_env(&mut command, &client.shared);
        let env: Vec<(std::ffi::OsString, Option<std::ffi::OsString>)> = command
            .get_envs()
            .map(|(k, v)| (k.to_os_string(), v.map(|v| v.to_os_string())))
            .collect();
        let found = env
            .iter()
            .find(|(k, _)| k == "V2RAYN_R_CORES_ROOT")
            .expect("cores root must be forwarded");
        assert_eq!(
            found.1.as_deref(),
            Some(std::ffi::OsStr::new("C:\\tmp\\data\\cores"))
        );
    }

    #[test]
    fn launch_env_without_root_forwards_nothing() {
        let client = NetHostClient::with_pipe("\\\\.\\pipe\\rr01-test-empty");
        assert!(client.cores_root().is_none());
        let mut command = std::process::Command::new("net_host.exe");
        apply_launch_env(&mut command, &client.shared);
        let has_root = command.get_envs().any(|(k, _)| k == "V2RAYN_R_CORES_ROOT");
        assert!(!has_root, "no root means no forwarded env var");
    }

    #[test]
    fn request_slot_deadline_includes_queue_wait() {
        let gate = RequestGate::new();
        gate.acquire(Instant::now() + Duration::from_secs(1))
            .expect("first acquisition is free");
        let error = gate
            .acquire(Instant::now() + Duration::from_millis(20))
            .expect_err("a held slot must time out, not wait unbounded");
        assert_eq!(error.code, domain::codes::TIMEOUT);
        gate.release();
        gate.acquire(Instant::now() + Duration::from_secs(1))
            .expect("slot is reusable after release");
    }

    #[test]
    fn request_slot_does_not_accumulate_workers() {
        let gate = RequestGate::new();
        gate.acquire(Instant::now() + Duration::from_secs(1))
            .unwrap();
        gate.record_spawn();
        assert_eq!(gate.active_workers(), 1);
        assert_eq!(gate.spawned_workers(), 1);
        // While the first worker is outstanding the slot stays held: a second
        // request must not spawn behind it.
        assert!(gate
            .acquire(Instant::now() + Duration::from_millis(10))
            .is_err());
        assert_eq!(
            gate.spawned_workers(),
            1,
            "a timed-out request must not spawn a second worker"
        );
        gate.record_finish();
        assert_eq!(gate.active_workers(), 0);
        gate.acquire(Instant::now() + Duration::from_secs(1))
            .unwrap();
    }

    #[test]
    fn net_host_client_exposes_worker_counts() {
        let client = NetHostClient::with_pipe("\\\\.\\pipe\\rr01-test-counts");
        assert_eq!(client.request_worker_counts(), (0, 0));
    }

    fn envelope(
        epoch: u64,
        seq: u64,
        kind: domain::event::EventKind,
        operation_id: Option<&str>,
        generation: Option<u64>,
    ) -> domain::EventEnvelope {
        let mut payload = serde_json::json!({"synthetic": true});
        if let Some(op) = operation_id {
            payload["operation_id"] = serde_json::json!(op);
        }
        if let Some(gen) = generation {
            payload["generation"] = serde_json::json!(gen);
        }
        domain::EventEnvelope::new(
            EventEpoch(epoch),
            domain::event::EventSeq(seq),
            kind,
            payload,
        )
    }

    #[test]
    fn event_identity_parses_operation_and_generation() {
        use domain::event::EventKind;
        let env = envelope(2, 7, EventKind::JobFinished, Some("op-sp07"), Some(4));
        let (op, gen) = parse_event_identity(&env);
        assert_eq!(op.as_deref(), Some("op-sp07"));
        assert_eq!(gen, Some(4));
    }

    #[test]
    fn gap_in_seq_demands_resync_snapshot() {
        use domain::event::EventKind;
        let mut cursor = EventCursor::new(2, 1);
        let first = envelope(2, 1, EventKind::LogBatch, None, None);
        assert!(cursor.observe(&first).resync.is_none());
        let gapped = envelope(2, 5, EventKind::JobFinished, Some("op-9"), Some(1));
        let outcome = cursor.observe(&gapped);
        let ask = outcome.resync.expect("seq gap must demand resync");
        assert_eq!(ask.reason, "gap");
        assert_eq!(ask.epoch, 2);
        // Cursor holds its position until the authoritative snapshot arrives.
        assert_eq!(cursor.next_seq(), 2);
    }

    #[test]
    fn epoch_change_demands_resync_and_never_rewinds_generation() {
        use domain::event::EventKind;
        let mut cursor = EventCursor::new(2, 40);
        cursor.observe(&envelope(
            2,
            40,
            EventKind::JobFinished,
            Some("op-a"),
            Some(6),
        ));
        assert_eq!(cursor.generation(), Some(6));
        let outcome = cursor.observe(&envelope(
            3,
            1,
            EventKind::JobFinished,
            Some("op-b"),
            Some(1),
        ));
        let ask = outcome.resync.expect("epoch change must demand resync");
        assert_eq!(ask.reason, "epoch_mismatch");
        // A stale generation from the old epoch must not overwrite the latch.
        assert_eq!(cursor.generation(), Some(6));
    }

    #[test]
    fn duplicate_seq_is_ignored_without_resync() {
        use domain::event::EventKind;
        let mut cursor = EventCursor::new(2, 2);
        cursor.observe(&envelope(2, 1, EventKind::LogBatch, None, None));
        let outcome = cursor.observe(&envelope(2, 1, EventKind::LogBatch, None, None));
        assert!(outcome.resync.is_none());
        assert_eq!(cursor.next_seq(), 2);
    }

    #[test]
    fn telemetry_flood_never_evicts_lifecycle_outcome() {
        use domain::event::EventKind;
        let mut latch = LifecycleLatch::new();
        for seq in 1..=2049u64 {
            latch.note(&envelope(2, seq, EventKind::LogBatch, None, None));
        }
        let final_outcome = envelope(2, 2050, EventKind::JobFinished, Some("op-final"), Some(3));
        latch.note(&final_outcome);
        // Late telemetry duplicates must not overwrite the final outcome.
        latch.note(&envelope(2, 3, EventKind::LogBatch, None, None));
        let kept = latch
            .last_control()
            .expect("lifecycle outcome must survive telemetry");
        let (op, gen) = parse_event_identity(&kept);
        assert_eq!(op.as_deref(), Some("op-final"));
        assert_eq!(gen, Some(3));
        assert!(latch.telemetry_seen() >= 2049);
    }

    #[test]
    fn resync_event_itself_demands_snapshot_renewal() {
        use domain::event::EventKind;
        let mut cursor = EventCursor::new(2, 30);
        let resync = domain::EventEnvelope::new(
            EventEpoch(2),
            domain::event::EventSeq(31),
            EventKind::Other("resync_required".into()),
            serde_json::json!({"reason": "lagged", "epoch": 2, "last_seq": 120}),
        );
        let outcome = cursor.observe(&resync);
        let ask = outcome
            .resync
            .expect("resync_required must renew via snapshot");
        assert_eq!(ask.reason, "lagged");
    }
}
