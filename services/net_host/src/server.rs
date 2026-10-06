//! Named-pipe server, request dispatch and the disconnect watchdog.

use std::sync::Arc;
use std::time::Duration;

use domain::event::{EventEnvelope, EventEpoch};
use domain::DomainError;
use ipc_contract::{
    check_session, check_test_ports, IpcError, IpcOperation, IpcResult, RequestEnvelope,
    ResponseEnvelope,
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

async fn handle_connection(pipe: NamedPipeServer, state: Arc<HostState>) {
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
        if let Err(ipc_error) = check_session(&request.session) {
            let _ = send_result(
                &tx,
                &request.request_id,
                IpcResult::Error(ipc_error.to_domain()),
            )
            .await;
            continue;
        }
        let request_id = request.request_id.clone();
        match request.operation {
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
            IpcOperation::TestSession(test_op) => {
                let result = match test_op {
                    ipc_contract::TestSessionOperation::Open {
                        plan,
                        max_duration_ms,
                    } => {
                        let ports: Vec<u16> = plan.ports.iter().map(|p| p.port).collect();
                        match check_test_ports(&ports) {
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

    for forwarder in forwarders {
        forwarder.abort();
    }
    state.connection_closed().await;
    eprintln!("[net_host] client disconnected");
    drop(tx);
    let _ = writer_task.await;
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
}
