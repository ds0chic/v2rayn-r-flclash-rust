//! Named-pipe server, request dispatch and the disconnect watchdog.

use std::sync::Arc;
use std::time::Duration;

use domain::event::{EventEnvelope, EventEpoch};
use domain::DomainError;
use ipc_contract::{
    check_session, check_test_ports, check_test_session_duration, constant_time_eq,
    expected_session_token, IpcError, IpcOperation, IpcResult, RequestEnvelope, ResponseEnvelope,
    SessionIdentity,
};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
use tokio::sync::{broadcast, mpsc};

use runtime::{decode_payload, encode_frame, frame_len, ServerFrame};

use crate::dacl::PipeSecurity;
use crate::session::HostState;

type Tx = mpsc::Sender<ServerFrame>;

/// Run the accept loop until `Shutdown` or a fatal pipe error.
pub async fn run_server(state: Arc<HostState>) -> std::io::Result<()> {
    let pipe_name = state.config.pipe_name.clone();
    let mut first = true;
    loop {
        let server = match create_pipe(&pipe_name, first) {
            Ok(server) => server,
            Err(e) if first => {
                eprintln!("[net_host] cannot create pipe {pipe_name}: {e}");
                return Err(e);
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        first = false;

        tokio::select! {
            _ = state.shutdown.notified() => break,
            connected = server.connect() => {
                if let Err(e) = connected {
                    eprintln!("[net_host] connect error: {e}");
                    continue;
                }
            }
        }
        let state = state.clone();
        tokio::spawn(async move {
            handle_connection(server, state).await;
        });
    }
    Ok(())
}

fn create_pipe(name: &str, first: bool) -> std::io::Result<NamedPipeServer> {
    let security = PipeSecurity::current_user_only()?;
    let mut options = ServerOptions::new();
    options.pipe_mode(PipeMode::Byte);
    options.reject_remote_clients(true);
    options.max_instances(16);
    options.first_pipe_instance(first);
    unsafe { options.create_with_security_attributes_raw(name, security.as_mut_ptr()) }
}

/// Whether a subscribe request can be served live, or must resynchronize
/// first. The bus keeps no replay buffer, so an epoch mismatch or a
/// `from_seq` beyond the authoritative tip cannot be filled: the subscriber
/// takes an authoritative snapshot and re-subscribes from the returned origin.
pub(crate) fn resync_needed(
    requested_epoch: u64,
    requested_from_seq: u64,
    current_epoch: u64,
    current_last_seq: u64,
) -> Option<&'static str> {
    if requested_epoch != current_epoch {
        return Some("epoch_mismatch");
    }
    if requested_from_seq > current_last_seq.saturating_add(1) {
        return Some("seq_ahead");
    }
    None
}

/// One broadcast receive outcome on a subscription forwarder.
pub(crate) enum ForwardAction {
    Forward(EventEnvelope),
    /// Recoverable lag: the forwarder emits a `resync_required` notice and
    /// ends the subscription so the client reliably reconnects.
    Resync(&'static str),
    /// Bus closed: the stream ended.
    Closed,
}

/// Classify a broadcast receive: `Lagged` is a recoverable resync request,
/// never a silent EOF.
pub(crate) fn classify_recv(
    result: Result<EventEnvelope, broadcast::error::RecvError>,
) -> ForwardAction {
    match result {
        Ok(event) => ForwardAction::Forward(event),
        Err(broadcast::error::RecvError::Lagged(_)) => ForwardAction::Resync("lagged"),
        Err(broadcast::error::RecvError::Closed) => ForwardAction::Closed,
    }
}

/// Transport-authenticated peer of one pipe connection: the OS-reported
/// client pid plus its creation time, both queried once at connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PipePeer {
    pub pid: u32,
    pub created_at_ms: Option<i64>,
}

/// Resolve the connecting client's pid from the connected pipe instance.
/// Fail-closed: when the OS will not name the peer, the connection serves
/// nothing.
fn pipe_peer(pipe: &NamedPipeServer) -> Option<PipePeer> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;

    let handle = HANDLE(pipe.as_raw_handle() as *mut core::ffi::c_void);
    let mut pid = 0u32;
    // SAFETY: `handle` is the live, connected pipe instance owned by this
    // task; the call only reads the peer pid into `pid`.
    unsafe { GetNamedPipeClientProcessId(handle, &mut pid) }.ok()?;
    if pid == 0 {
        return None;
    }
    Some(PipePeer {
        pid,
        created_at_ms: runtime::process_creation_time_ms(pid),
    })
}

/// Bind one request's session to the transport peer (S1-02, net-host side).
///
/// `check_session` only validates shape; this completes it: the token must
/// equal [`expected_session_token`] for the OS-authenticated pipe pid
/// (constant-time compare), the claimed `peer_pid` must be that pid, and the
/// claimed creation time must match the live process (defeats PID reuse).
/// Anything else is a typed `Unauthorized` rejection. Token values never
/// enter logs; the caller logs only the rejection reason.
pub(crate) fn verify_peer_session(
    session: &SessionIdentity,
    peer: &PipePeer,
) -> Result<(), IpcError> {
    check_session(session)?;
    let expected = expected_session_token(peer.pid);
    if !constant_time_eq(&session.session_token, &expected) {
        return Err(IpcError::Unauthorized {
            detail: "session token mismatch".to_string(),
        });
    }
    if session.peer_pid != peer.pid {
        return Err(IpcError::Unauthorized {
            detail: "peer pid mismatch".to_string(),
        });
    }
    match (peer.created_at_ms, session.peer_created_at_ms) {
        (Some(actual), claimed) if actual == claimed => Ok(()),
        _ => Err(IpcError::Unauthorized {
            detail: "peer identity mismatch".to_string(),
        }),
    }
}

/// Log-safe rejection reason (never a token value).
fn rejection_reason(error: &IpcError) -> String {
    match error {
        IpcError::Unauthorized { detail } => format!("unauthorized: {detail}"),
        IpcError::VersionMismatch { peer, host } => {
            format!("version mismatch: peer v{peer}, host v{host}")
        }
        IpcError::MessageTooLarge { size, limit } => {
            format!("frame too large: {size} > {limit}")
        }
        IpcError::Malformed { detail } => format!("malformed: {detail}"),
    }
}

async fn handle_connection(pipe: NamedPipeServer, state: Arc<HostState>) {
    // Transport peer first: an unidentifiable client is dropped before it
    // counts as a connection, so it can neither command nor hold the lease.
    let peer = match pipe_peer(&pipe) {
        Some(peer) => peer,
        None => {
            eprintln!("[net_host] client identity unavailable; connection dropped");
            return;
        }
    };
    state.connection_opened().await;
    eprintln!(
        "[net_host] client connected (active={})",
        state.inner.lock().await.active_connections
    );
    let (mut reader, mut writer) = tokio::io::split(pipe);
    let (tx, mut rx) = mpsc::channel::<ServerFrame>(256);
    let writer_task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if write_frame(&mut writer, &frame).await.is_err() {
                break;
            }
        }
    });

    let mut forwarders = Vec::new();
    loop {
        let payload = match read_frame(&mut reader).await {
            Ok(payload) => payload,
            Err(_) => break,
        };
        let request: RequestEnvelope = match decode_payload::<RequestEnvelope>(&payload) {
            Ok(request) => request,
            Err(e) => {
                let _ = send_result(
                    &tx,
                    "",
                    IpcResult::Error(
                        DomainError::new(domain::codes::INVALID_ARGUMENT, "error.ipc_malformed")
                            .with_detail(format!("malformed request: {e}")),
                    ),
                )
                .await;
                continue;
            }
        };
        if let Err(ipc_error) = verify_peer_session(&request.session, &peer) {
            eprintln!(
                "[net_host] ipc auth rejected request_id={} peer_pid={} reason={}",
                request.request_id,
                request.session.peer_pid,
                rejection_reason(&ipc_error),
            );
            let _ = send_result(
                &tx,
                &request.request_id,
                IpcResult::Error(ipc_error.to_domain()),
            )
            .await;
            continue;
        }
        dispatch_operation(&state, &request.request_id, request.operation, &tx, &mut forwarders)
            .await;
    }

    for forwarder in forwarders {
        forwarder.abort();
    }
    state.connection_closed().await;
    eprintln!("[net_host] client disconnected");
    drop(tx);
    let _ = writer_task.await;
}

/// Dispatch one authenticated operation. Extracted from the connection loop
/// so IPC-level behavior (including the SP-08 retry entry) is directly
/// testable without a pipe.
async fn dispatch_operation(
    state: &Arc<HostState>,
    request_id: &str,
    operation: IpcOperation,
    tx: &Tx,
    forwarders: &mut Vec<tokio::task::JoinHandle<()>>,
) {
        match operation {
            IpcOperation::GetSnapshot => {
                let _ = tx.send(state.detail_frame().await).await;
                let snapshot = state.ipc_snapshot().await;
                let _ =
                    send_result(&tx, &request_id, IpcResult::Snapshot(Box::new(snapshot))).await;
            }
            IpcOperation::ApplyPlan { plan } => {
                let result = match state.apply_plan(*plan).await {
                    Ok(operation_id) => IpcResult::Accepted { operation_id },
                    Err(error) => IpcResult::Error(error),
                };
                let _ = tx.send(state.detail_frame().await).await;
                let _ = send_result(&tx, &request_id, result).await;
            }
            IpcOperation::StopRuntime { operation_id } => {
                let _ = state.stop_managed(operation_id).await;
                let _ = tx.send(state.detail_frame().await).await;
                let _ = send_result(&tx, &request_id, IpcResult::Stopped).await;
            }
            IpcOperation::GetOperation { operation_id } => {
                let status = state.operation_status(&operation_id).await;
                let result = match status {
                    Some(status) => IpcResult::Operation(Box::new(status)),
                    None => IpcResult::Error(DomainError::not_found("operation", &operation_id)),
                };
                let _ = send_result(&tx, &request_id, result).await;
            }
            IpcOperation::SubscribeEvents { epoch, from_seq } => {
                let current_epoch = state.bus.epoch().get();
                let current_last_seq = state.bus.last_seq();
                let mut receiver = state.bus.subscribe();
                let forward_tx = tx.clone();
                let bus = state.bus.clone();
                forwarders.push(tokio::spawn(async move {
                    let mut last_operation_id: Option<String> = None;
                    let mut last_generation: Option<u64> = None;
                    loop {
                        match classify_recv(receiver.recv().await) {
                            ForwardAction::Forward(event) => {
                                let (operation_id, generation) =
                                    crate::events::parse_event_identity(&event);
                                if operation_id.is_some() {
                                    last_operation_id = operation_id;
                                }
                                if generation.is_some() {
                                    last_generation = generation;
                                }
                                if forward_tx.send(ServerFrame::Event(event)).await.is_err() {
                                    break;
                                }
                            }
                            ForwardAction::Resync(reason) => {
                                let notice = bus.resync_notice_attributed(
                                    reason,
                                    bus.epoch().get(),
                                    bus.last_seq(),
                                    last_operation_id.as_deref(),
                                    last_generation,
                                );
                                let _ = forward_tx.send(ServerFrame::Event(notice)).await;
                                break;
                            }
                            ForwardAction::Closed => break,
                        }
                    }
                }));
                // Authoritative origin, never an echo of stale parameters: the
                // client advances its subscription cursor to these values.
                if let Some(reason) =
                    resync_needed(epoch.get(), from_seq, current_epoch, current_last_seq)
                {
                    let notice = state
                        .bus
                        .resync_notice(reason, current_epoch, current_last_seq);
                    let _ = tx.send(ServerFrame::Event(notice)).await;
                }
                let _ = send_result(
                    &tx,
                    &request_id,
                    IpcResult::EventStreamOpened {
                        epoch: EventEpoch(current_epoch),
                        from_seq: current_last_seq.saturating_add(1),
                    },
                )
                .await;
            }
            IpcOperation::Shutdown => {
                let _ = state.stop_managed(None).await;
                state.stop_all_test_sessions().await;
                let _ = send_result(&tx, &request_id, IpcResult::Shutdown).await;
                state.shutdown.notify_one();
            }
            // SP-08 explicit retry entry (S7a-05): re-attempt every
            // unconfirmed TUN cleanup with the live link (or a fresh link
            // for journals left by a previous process). A confirmed retry
            // with nothing left outstanding restores the closed state.
            IpcOperation::RetryTunCleanup => {
                let result = match state.retry_tun_cleanup().await {
                    Ok(()) => IpcResult::TunCleanupRetried,
                    Err(error) => IpcResult::Error(error),
                };
                let _ = send_result(&tx, request_id, result).await;
            }
            IpcOperation::TestSession(test_op) => {
                let result = match test_op {
                    ipc_contract::TestSessionOperation::Open {
                        plan,
                        max_duration_ms,
                    } => {
                        let ports: Vec<u16> = plan.ports.iter().map(|p| p.port).collect();
                        match check_test_ports(&ports)
                            .and(check_test_session_duration(max_duration_ms))
                        {
                            Err(IpcError::Malformed { detail }) => IpcResult::Error(
                                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.invalid")
                                    .with_detail(detail),
                            ),
                            Err(other) => IpcResult::Error(other.to_domain()),
                            Ok(()) => {
                                match HostState::open_test_session(&state, *plan, max_duration_ms)
                                    .await
                                {
                                    Ok(session_id) => IpcResult::TestSession {
                                        session_id,
                                        state: domain::JobState::Running,
                                    },
                                    Err(error) => IpcResult::Error(error),
                                }
                            }
                        }
                    }
                    ipc_contract::TestSessionOperation::Close { session_id } => {
                        match state.close_test_session(&session_id).await {
                            Ok(()) => IpcResult::TestSession {
                                session_id,
                                state: domain::JobState::Done,
                            },
                            Err(error) => IpcResult::Error(error),
                        }
                    }
                    ipc_contract::TestSessionOperation::Status { session_id } => {
                        match state.test_session_status(&session_id).await {
                            Some(state) => IpcResult::TestSession { session_id, state },
                            None => IpcResult::Error(DomainError::not_found(
                                "test_session",
                                &session_id,
                            )),
                        }
                    }
                };
                let _ = send_result(&tx, &request_id, result).await;
            }
        }
}

/// Watchdog implementing the disconnect/heartbeat policy (plan §5).
pub async fn watchdog(state: Arc<HostState>) {
    let interval = state.config.heartbeat_interval;
    loop {
        tokio::time::sleep(interval).await;
        if state.should_reclaim().await {
            eprintln!("[net_host] lease reclaim triggered; stopping managed session");
            let _ = state.stop_managed(None).await;
            state.stop_all_test_sessions().await;
            state
                .bus
                .emit_named("lease_reclaimed", json!({"reason": "client_lost"}));
        } else {
            // Server->client liveness: subscribers learn net-host is alive.
            state
                .bus
                .emit_named("heartbeat", json!({"host": "net_host"}));
        }
        // R4-05: once every client is gone and no core/lease/test session
        // remains, terminate so a real exit leaves no residual service. A
        // reopened GUI within the idle window still reuses this process.
        if state.should_exit_idle().await {
            eprintln!("[net_host] idle with no client/session; shutting down");
            state.shutdown.notify_one();
            break;
        }
    }
}

async fn send_result(tx: &Tx, request_id: &str, result: IpcResult) -> Result<(), ()> {
    tx.send(ServerFrame::Response(ResponseEnvelope {
        request_id: request_id.to_string(),
        result,
    }))
    .await
    .map_err(|_| ())
}

async fn read_frame<R>(reader: &mut R) -> std::io::Result<Vec<u8>>
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix).await?;
    let len = frame_len(prefix);
    if !runtime::frame_len_ok(len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len}"),
        ));
    }
    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload).await?;
    Ok(payload)
}

async fn write_frame<W>(writer: &mut W, frame: &ServerFrame) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let bytes = encode_frame(frame)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    writer.write_all(&bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
    use tokio::sync::broadcast::error::RecvError;

    fn envelope(seq: u64) -> EventEnvelope {
        EventEnvelope::new(
            EventEpoch(3),
            EventSeq(seq),
            EventKind::LogBatch,
            serde_json::json!({}),
        )
    }

    #[test]
    fn stale_epoch_demands_resync_not_echo() {
        assert_eq!(resync_needed(1, 5, 2, 40), Some("epoch_mismatch"));
    }

    #[test]
    fn seq_ahead_of_authority_demands_resync() {
        assert_eq!(resync_needed(3, 100, 3, 40), Some("seq_ahead"));
    }

    #[test]
    fn contiguous_subscription_is_accepted() {
        assert_eq!(resync_needed(3, 41, 3, 40), None);
        assert_eq!(resync_needed(3, 0, 3, 40), None);
    }

    #[test]
    fn lagged_recv_maps_to_resync_not_eof() {
        match classify_recv(Err(RecvError::Lagged(7))) {
            ForwardAction::Resync(reason) => assert_eq!(reason, "lagged"),
            ForwardAction::Forward(_) | ForwardAction::Closed => {
                panic!("Lagged must map to Resync, never EOF-or-forward")
            }
        }
    }

    #[test]
    fn closed_recv_maps_to_eof() {
        assert!(matches!(
            classify_recv(Err(RecvError::Closed)),
            ForwardAction::Closed
        ));
    }

    #[test]
    fn ok_recv_forwards_event() {
        let env = envelope(9);
        match classify_recv(Ok(env.clone())) {
            ForwardAction::Forward(got) => assert_eq!(got.seq, env.seq),
            _ => panic!("Ok must forward"),
        }
    }

    /// Cleanup link that fails until its flag clears (S7a-05 dispatch seam):
    /// models a helper outage with no OS helper involved.
    struct FlagCleanupLink {
        fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl crate::helper_client::HelperLink for FlagCleanupLink {
        fn session_id(&self) -> String {
            "flag-link".into()
        }

        fn dry_run(&self) -> bool {
            false
        }

        fn available(&mut self) -> Result<(), DomainError> {
            Ok(())
        }

        fn apply(
            &mut self,
            spec: &runtime::tun::TunSpec,
        ) -> Result<crate::helper_client::TunLease, DomainError> {
            Ok(crate::helper_client::TunLease::new(
                "flag-link",
                spec.clone(),
                false,
            ))
        }

        fn cleanup(
            &mut self,
            _lease: &crate::helper_client::TunLease,
        ) -> Result<(), DomainError> {
            if self
                .fail
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return Err(crate::helper_client::tun_helper_unavailable(
                    "synthetic helper outage",
                ));
            }
            Ok(())
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn retry_tun_cleanup_dispatch_converges_and_reports() {
        use crate::helper_client::HelperConfig;
        use crate::session::HostConfig;
        use runtime::tun::{TunAddress, TunRoute, TunSpec, TUN_CONFIG_KIND};

        // S7a-05: the SP-08 retry entry is reachable over IPC. A failed
        // cleanup first reports Error through dispatch, then converges to
        // TunCleanupRetried once the helper recovers (pre-fix there was no
        // such operation to dispatch).
        let root = std::env::temp_dir().join(format!(
            "v2rayn-s7a05-dispatch-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = Arc::new(HostState::new(HostConfig {
            pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
            run_root: root.clone(),
            disconnect_grace: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(1000),
            readiness_timeout: Duration::from_millis(100),
            readiness_interval: Duration::from_millis(10),
            helper: HelperConfig {
                pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
                token: String::new(),
                bin: None,
                auto_launch: false,
                allowed_run_roots: Vec::new(),
                dry_run: false,
            },
        }));
        let fail = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let spec = TunSpec {
            kind: TUN_CONFIG_KIND.into(),
            adapter_name: "v2rayn-tun".into(),
            interface_index: 9,
            addresses: vec![TunAddress {
                address: "198.18.0.1".into(),
                prefix_len: 16,
            }],
            mtu: Some(1400),
            routes: vec![TunRoute {
                destination: "0.0.0.0/0".into(),
                next_hop: "198.18.0.1".into(),
                interface_index: 9,
                metric: 1,
            }],
            route_exclude: vec![],
        };
        let lease = crate::helper_client::TunLease::new("flag-link", spec, false);
        crate::tun_lease::write_tun_journal(&root, "sess-retry", &lease).expect("journal");
        {
            let mut inner = state.inner.lock().await;
            inner.tun_session_id = Some("sess-retry".into());
            inner.tun_lease = Some(lease);
            let link: Box<dyn crate::helper_client::HelperLink> =
                Box::new(FlagCleanupLink { fail: fail.clone() });
            inner.tun_link = Some(link);
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<ServerFrame>(16);
        let mut forwarders = Vec::new();
        dispatch_operation(
            &state,
            "r1",
            IpcOperation::RetryTunCleanup,
            &tx,
            &mut forwarders,
        )
        .await;
        let first = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("reply arrives")
            .expect("channel open");
        match first {
            ServerFrame::Response(envelope) => assert!(
                matches!(envelope.result, IpcResult::Error(_)),
                "a still-failing cleanup reports Error, not success"
            ),
            other => panic!("expected a response frame, got {other:?}"),
        }
        assert_eq!(
            state.pending_cleanup_snapshot().await.len(),
            1,
            "the failed cleanup stays retryable"
        );

        fail.store(false, std::sync::atomic::Ordering::SeqCst);
        dispatch_operation(
            &state,
            "r2",
            IpcOperation::RetryTunCleanup,
            &tx,
            &mut forwarders,
        )
        .await;
        let second = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("reply arrives")
            .expect("channel open");
        match second {
            ServerFrame::Response(envelope) => assert!(
                matches!(envelope.result, IpcResult::TunCleanupRetried),
                "a recovered helper converges the retry: {:?}",
                envelope.result
            ),
            other => panic!("expected a response frame, got {other:?}"),
        }
        assert!(
            state.pending_cleanup_snapshot().await.is_empty(),
            "a confirmed retry clears every record"
        );
        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn open_test_session_over_bound_duration_is_rejected_before_spawn() {
        use domain::runtime_plan::{
            ConfigSource, ContentHash, NetworkPolicy, OutboundGraph, PortRequest, ProcessGraph,
            RuntimePlan, RuntimeTarget,
        };

        // S1-18: an unbounded duration is rejected at the gate, before any
        // core is resolved or spawned (no core binary needed here).
        let root = std::env::temp_dir().join(format!(
            "v2rayn-s118-duration-{}-{}",
            std::process::id(),
            crate::journal::now_ms()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = Arc::new(HostState::new(crate::session::HostConfig {
            pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
            run_root: root.clone(),
            disconnect_grace: Duration::from_millis(1000),
            heartbeat_interval: Duration::from_millis(1000),
            readiness_timeout: Duration::from_millis(100),
            readiness_interval: Duration::from_millis(10),
            helper: crate::helper_client::HelperConfig {
                pipe_name: r"\\.\pipe\v2rayn-r-test-nonexistent".into(),
                token: String::new(),
                bin: None,
                auto_launch: false,
                allowed_run_roots: Vec::new(),
                dry_run: false,
            },
        }));
        let plan = Box::new(RuntimePlan {
            plan_id: "p1".into(),
            desired_revision: 1,
            target: RuntimeTarget {
                core_type: domain::CoreType::Xray,
                version: None,
                config: ConfigSource::Inline { body: "{}".into() },
                config_sha256: ContentHash::new("ab"),
            },
            process_graph: ProcessGraph::default(),
            outbound_graph: OutboundGraph::default(),
            ports: vec![PortRequest::tcp(12_300, "inbound")],
            privileges: vec![],
            network_policy: NetworkPolicy::default(),
            resources: vec![],
        });
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ServerFrame>(16);
        let mut forwarders = Vec::new();
        dispatch_operation(
            &state,
            "r1",
            IpcOperation::TestSession(ipc_contract::TestSessionOperation::Open {
                plan,
                max_duration_ms: u64::MAX,
            }),
            &tx,
            &mut forwarders,
        )
        .await;
        let reply = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("reply arrives")
            .expect("channel open");
        match reply {
            ServerFrame::Response(envelope) => assert!(
                matches!(envelope.result, IpcResult::Error(_)),
                "an unbounded test session must be rejected: {:?}",
                envelope.result
            ),
            other => panic!("expected a response frame, got {other:?}"),
        }
        drop(state);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn live_peer() -> PipePeer {
        let pid = std::process::id();
        PipePeer {
            pid,
            created_at_ms: runtime::process_creation_time_ms(pid),
        }
    }

    fn peer_session(peer: &PipePeer) -> SessionIdentity {
        SessionIdentity {
            protocol_version: ipc_contract::IPC_PROTOCOL_VERSION,
            session_token: expected_session_token(peer.pid),
            peer_pid: peer.pid,
            peer_created_at_ms: peer.created_at_ms.unwrap_or(0),
        }
    }

    #[test]
    fn bound_peer_session_is_accepted() {
        let peer = live_peer();
        assert!(peer.created_at_ms.is_some(), "own pid must be queryable");
        assert!(verify_peer_session(&peer_session(&peer), &peer).is_ok());
    }

    #[test]
    fn foreign_token_is_rejected() {
        let peer = live_peer();
        let mut session = peer_session(&peer);
        session.session_token = "v2rayn-1".to_string();
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
        // A well-formed token for a *different* pid is still rejected: the
        // token is bound to the transport peer, not to any live process.
        session.session_token = expected_session_token(peer.pid.wrapping_add(1));
        session.peer_pid = peer.pid.wrapping_add(1);
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
    }

    #[test]
    fn pid_or_creation_lie_is_rejected() {
        let peer = live_peer();
        let mut session = peer_session(&peer);
        session.peer_pid = peer.pid.wrapping_add(1);
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
        let mut session = peer_session(&peer);
        session.peer_created_at_ms = peer.created_at_ms.unwrap_or(0).wrapping_add(1);
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
    }

    #[test]
    fn empty_token_and_version_still_rejected() {
        let peer = live_peer();
        let mut session = peer_session(&peer);
        session.session_token.clear();
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::Unauthorized { .. }
        ));
        let mut session = peer_session(&peer);
        session.protocol_version += 1;
        assert!(matches!(
            verify_peer_session(&session, &peer).unwrap_err(),
            IpcError::VersionMismatch { .. }
        ));
    }
}
