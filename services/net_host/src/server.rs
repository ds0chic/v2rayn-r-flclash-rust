//! Named-pipe server, request dispatch and the disconnect watchdog.

use std::sync::Arc;
use std::time::Duration;

use domain::DomainError;
use ipc_contract::{
    check_session, check_test_ports, IpcError, IpcOperation, IpcResult, RequestEnvelope,
    ResponseEnvelope,
};
use serde_json::json;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
use tokio::sync::mpsc;

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
                let status = {
                    let inner = state.inner.lock().await;
                    inner.operations.get(&operation_id).cloned()
                };
                let result = match status {
                    Some(status) => IpcResult::Operation(Box::new(status)),
                    None => IpcResult::Error(DomainError::not_found("operation", &operation_id)),
                };
                let _ = send_result(&tx, &request_id, result).await;
            }
            IpcOperation::SubscribeEvents { epoch, from_seq } => {
                let mut receiver = state.bus.subscribe();
                let forward_tx = tx.clone();
                forwarders.push(tokio::spawn(async move {
                    while let Ok(event) = receiver.recv().await {
                        if forward_tx.send(ServerFrame::Event(event)).await.is_err() {
                            break;
                        }
                    }
                }));
                let _ = send_result(
                    &tx,
                    &request_id,
                    IpcResult::EventStreamOpened { epoch, from_seq },
                )
                .await;
            }
            IpcOperation::Shutdown => {
                let _ = state.stop_managed(None).await;
                let _ = send_result(&tx, &request_id, IpcResult::Shutdown).await;
                state.shutdown.notify_one();
            }
            IpcOperation::TestSession(test_op) => {
                let result = match test_op {
                    ipc_contract::TestSessionOperation::Open { ports, .. } => {
                        match check_test_ports(&ports) {
                            Ok(()) => IpcResult::Error(DomainError::new(
                                domain::codes::INVALID_ARGUMENT,
                                "error.test_session_unsupported",
                            )),
                            Err(IpcError::Malformed { detail }) => IpcResult::Error(
                                DomainError::new(domain::codes::INVALID_ARGUMENT, "error.invalid")
                                    .with_detail(detail),
                            ),
                            Err(other) => IpcResult::Error(other.to_domain()),
                        }
                    }
                    _ => IpcResult::Error(DomainError::new(
                        domain::codes::INVALID_ARGUMENT,
                        "error.test_session_unsupported",
                    )),
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
            state
                .bus
                .emit_named("lease_reclaimed", json!({"reason": "client_lost"}));
        } else {
            // Server->client liveness: subscribers learn net-host is alive.
            state
                .bus
                .emit_named("heartbeat", json!({"host": "net_host"}));
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
