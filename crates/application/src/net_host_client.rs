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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::event::EventEpoch;
use domain::{AppliedRevision, CancelOutcome, DomainError, JobId, RuntimePlan};
use ipc_contract::{
    IpcOperation, IpcResult, RequestEnvelope, RuntimeSnapshot as IpcSnapshot, SessionIdentity,
    IPC_APPLY_TIMEOUT_MS, IPC_PROTOCOL_VERSION, IPC_REQUEST_TIMEOUT_MS,
};
use runtime::{
    current_identity, decode_payload, encode_frame, frame_len, frame_len_ok, RuntimeDetail,
    ServerFrame, LEN_PREFIX_BYTES, NET_HOST_PIPE_NAME, RUNTIME_DETAIL_EVENT,
};

use crate::runtime_client::{ApplyOutcome, EventSink, RuntimeClient, RuntimeSnapshot};

/// How long to wait for a freshly launched net-host to accept a connection.
const LAUNCH_WAIT: Duration = Duration::from_secs(10);
/// Poll interval while waiting for the pipe.
const LAUNCH_POLL: Duration = Duration::from_millis(100);
/// Reconnect backoff for the event connection.
const EVENT_RECONNECT: Duration = Duration::from_millis(500);

fn trace(message: impl AsRef<str>) {
    if std::env::var_os("V2RAYN_R_TRACE").is_some() {
        eprintln!("[net-host-client] {}", message.as_ref());
    }
}

struct Shared {
    pipe_name: String,
    auto_launch: bool,
    /// Serializes control requests so at most one request connection is live.
    req_mutex: Mutex<()>,
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
                req_mutex: Mutex::new(()),
                launch_mutex: Mutex::new(()),
                sink: Arc::new(Mutex::new(None)),
                events_started: AtomicBool::new(false),
                events_stop: AtomicBool::new(false),
                session,
                request_seq: AtomicU64::new(0),
            }),
        }
    }

    /// The configured pipe name (diagnostics/tests).
    pub fn pipe_name(&self) -> &str {
        &self.shared.pipe_name
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
    }
}

fn do_request(
    shared: &Arc<Shared>,
    operation: IpcOperation,
    timeout: Duration,
) -> Result<(IpcResult, RuntimeDetail), DomainError> {
    let _guard = shared
        .req_mutex
        .lock()
        .map_err(|_| DomainError::new(domain::codes::INTERNAL, "error.lock_poisoned"))?;
    trace("do_request: dispatching");
    let (tx, rx) = mpsc::channel();
    let worker = Arc::clone(shared);
    std::thread::Builder::new()
        .name("net-host-request".into())
        .spawn(move || {
            let result = run_request(&worker, operation);
            let _ = tx.send(result);
        })
        .map_err(|e| unavailable(format!("spawn request thread failed: {e}")))?;

    match rx.recv_timeout(timeout) {
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
    while !shared.events_stop.load(Ordering::Acquire) {
        match connect(shared) {
            Ok(mut file) => {
                trace("event_loop: connected");
                if subscribe(shared, &mut file).is_err() {
                    std::thread::sleep(EVENT_RECONNECT);
                    continue;
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
                        let callback = shared.sink.lock().ok().and_then(|slot| slot.clone());
                        if let Some(callback) = callback {
                            callback(event);
                        }
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

fn subscribe(shared: &Arc<Shared>, file: &mut File) -> Result<(), DomainError> {
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
    // Read the EventStreamOpened acknowledgment on the same handle.
    let payload =
        read_payload(file).map_err(|e| unavailable(format!("subscribe read failed: {e}")))?;
    decode_payload::<ServerFrame>(&payload)
        .map_err(|e| unavailable(format!("subscribe decode failed: {e}")))?;
    Ok(())
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
    launch_net_host()?;
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

fn launch_net_host() -> Result<(), DomainError> {
    let exe = locate_net_host().ok_or_else(|| {
        unavailable("net_host.exe not found (set V2RAYN_R_NET_HOST or build the workspace)")
    })?;
    let mut command = std::process::Command::new(&exe);
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
