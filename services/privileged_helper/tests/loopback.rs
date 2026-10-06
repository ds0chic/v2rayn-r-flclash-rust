//! Loopback transport tests over `tokio::io::duplex` (T14).
//!
//! These exercise the framing, size cap, timeout, SID rejection and shutdown
//! paths without any named pipe, privilege or network change.

use std::sync::Arc;
use std::time::Duration;

use ipc_contract::{
    HelperError, HelperOp, HelperRequest, HelperResponse, HelperResult, SessionIdentity,
    HELPER_MAX_MESSAGE_BYTES, HELPER_PROTOCOL_VERSION,
};
use privileged_helper::backend::FakeBackend;
use privileged_helper::server::{
    serve_connection, ConnectionLease, HelperServer, HelperServerConfig, LeasePolicy,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

fn session() -> SessionIdentity {
    SessionIdentity {
        protocol_version: HELPER_PROTOCOL_VERSION,
        session_token: "tok".to_string(),
        peer_pid: 1,
        peer_created_at_ms: 1,
    }
}

fn request(operation: HelperOp) -> HelperRequest {
    HelperRequest {
        session: session(),
        request_id: "r1".to_string(),
        operation,
    }
}

fn config() -> HelperServerConfig {
    HelperServerConfig {
        session_token: "tok".to_string(),
        allowed_run_roots: vec![r"C:\v2rayn-r".to_string()],
        request_timeout: Duration::from_millis(150),
        lease_policy: LeasePolicy::CleanOwned,
        ..HelperServerConfig::default()
    }
}

fn encode(request: &HelperRequest) -> Vec<u8> {
    let payload = serde_json::to_vec(request).unwrap();
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&payload);
    frame
}

async fn read_response<R>(reader: &mut R) -> HelperResponse
where
    R: AsyncRead + Unpin,
{
    let mut prefix = [0u8; 4];
    reader.read_exact(&mut prefix).await.unwrap();
    let len = u32::from_le_bytes(prefix) as usize;
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload).await.unwrap();
    serde_json::from_slice(&payload).unwrap()
}

#[tokio::test]
async fn ping_over_loopback() {
    let fake = Arc::new(FakeBackend::new().elevated(true));
    let server = Arc::new(HelperServer::new(fake, config()));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    client
        .write_all(&encode(&request(HelperOp::Ping)))
        .await
        .unwrap();
    client.flush().await.unwrap();
    let response = read_response(&mut client).await;
    match response.result {
        HelperResult::Pong { elevation } => assert!(elevation.elevated),
        other => panic!("expected pong, got {other:?}"),
    }

    drop(client);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn malformed_json_returns_malformed() {
    let fake = Arc::new(FakeBackend::new());
    let server = Arc::new(HelperServer::new(fake, config()));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    let garbage = b"not-json";
    let mut frame = Vec::new();
    frame.extend_from_slice(&(garbage.len() as u32).to_le_bytes());
    frame.extend_from_slice(garbage);
    client.write_all(&frame).await.unwrap();
    client.flush().await.unwrap();

    let response = read_response(&mut client).await;
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Malformed { .. }
        }
    ));
    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}

#[tokio::test]
async fn oversized_frame_is_rejected() {
    let fake = Arc::new(FakeBackend::new());
    let server = Arc::new(HelperServer::new(fake, config()));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    let declared = (HELPER_MAX_MESSAGE_BYTES + 1) as u32;
    client.write_all(&declared.to_le_bytes()).await.unwrap();
    client.flush().await.unwrap();

    let response = read_response(&mut client).await;
    match response.result {
        HelperResult::Error {
            error: HelperError::MessageTooLarge { size, limit },
        } => {
            assert_eq!(size, HELPER_MAX_MESSAGE_BYTES + 1);
            assert_eq!(limit, HELPER_MAX_MESSAGE_BYTES);
        }
        other => panic!("expected message too large, got {other:?}"),
    }
    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}

#[tokio::test]
async fn idle_connection_survives_request_timeout() {
    // TUN-A01: the per-request timeout (150ms here) must not reclaim an idle
    // session; an open session keeps its lease until EOF or the idle bound.
    let fake = Arc::new(FakeBackend::new());
    let server = Arc::new(HelperServer::new(fake, config()));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    let early = tokio::time::timeout(Duration::from_millis(500), read_response(&mut client)).await;
    assert!(
        early.is_err(),
        "an idle session must not time out at the per-request bound"
    );
    assert!(!task.is_finished(), "the session must still be open");
    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}

#[tokio::test]
async fn idle_connection_times_out_at_the_idle_bound() {
    // The long keep-alive bound still ends a session that stays open with no
    // traffic; configured short here to keep the test fast.
    let fake = Arc::new(FakeBackend::new());
    let mut server_config = config();
    server_config.idle_timeout = Duration::from_millis(200);
    let server = Arc::new(HelperServer::new(fake, server_config));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    let response = tokio::time::timeout(Duration::from_secs(2), read_response(&mut client))
        .await
        .expect("timeout response should arrive at the idle bound");
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Timeout { .. }
        }
    ));
    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}

#[tokio::test]
async fn shutdown_closes_session() {
    let fake = Arc::new(FakeBackend::new());
    let server = Arc::new(HelperServer::new(fake.clone(), config()));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(server_side, server, ConnectionLease::new("s1"), None)
                .await
                .unwrap();
        }
    });

    client
        .write_all(&encode(&request(HelperOp::Shutdown)))
        .await
        .unwrap();
    client.flush().await.unwrap();
    let response = read_response(&mut client).await;
    assert!(matches!(response.result, HelperResult::Shutdown));

    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert!(server.is_shutdown());
    assert_eq!(fake.shutdown_count(), 1);
}

#[tokio::test]
async fn sid_mismatch_rejected_before_request() {
    let fake = Arc::new(FakeBackend::new());
    let mut cfg = config();
    cfg.require_sid_match = true;
    cfg.expected_sid = Some("S-1-5-21-1-2-3".to_string());
    let server = Arc::new(HelperServer::new(fake, cfg));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(
                server_side,
                server,
                ConnectionLease::new("s1"),
                Some("S-1-5-21-9-9-9".to_string()),
            )
            .await
            .unwrap();
        }
    });

    let response = read_response(&mut client).await;
    assert!(matches!(
        response.result,
        HelperResult::Error {
            error: HelperError::Unauthorized { .. }
        }
    ));
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}

#[tokio::test]
async fn sid_match_allows_request() {
    let fake = Arc::new(FakeBackend::new().elevated(true));
    let mut cfg = config();
    cfg.require_sid_match = true;
    cfg.expected_sid = Some("S-1-5-21-1-2-3".to_string());
    let server = Arc::new(HelperServer::new(fake, cfg));
    let (mut client, server_side) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn({
        let server = server.clone();
        async move {
            serve_connection(
                server_side,
                server,
                ConnectionLease::new("s1"),
                Some("s-1-5-21-1-2-3".to_string()),
            )
            .await
            .unwrap();
        }
    });

    client
        .write_all(&encode(&request(HelperOp::Ping)))
        .await
        .unwrap();
    client.flush().await.unwrap();
    let response = read_response(&mut client).await;
    assert!(matches!(response.result, HelperResult::Pong { .. }));
    drop(client);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;
}
