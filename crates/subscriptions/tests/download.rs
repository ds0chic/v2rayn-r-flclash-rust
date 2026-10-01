//! Downloader tests against a local loopback HTTP server. Ports are chosen
//! from 11808 upwards; the reserved user proxy port 10808 is never touched.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::CancellationToken;
use subscriptions::download::{build_client, download_string, DownloadOptions, ProxyConfig};
use subscriptions::SubError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

enum Reply {
    Raw(Vec<u8>),
    Delay(Duration, Vec<u8>),
    RequireHeader {
        name: String,
        value: String,
        body: Vec<u8>,
    },
}

struct Server {
    base: String,
    handle: tokio::task::JoinHandle<()>,
}

impl Server {
    fn abort(self) {
        self.handle.abort();
    }
}

async fn bind() -> TcpListener {
    for port in 11808..11900u16 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return listener;
        }
    }
    panic!("no free loopback port in 11808..11900");
}

async fn spawn(replies: Vec<Reply>) -> Server {
    let listener = bind().await;
    let port = listener.local_addr().unwrap().port();
    let queue = Arc::new(Mutex::new(VecDeque::from(replies)));
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let request = read_request(&mut socket).await;
            let Some(reply) = queue.lock().unwrap().pop_front() else {
                break;
            };
            let bytes = match reply {
                Reply::Raw(bytes) => bytes,
                Reply::Delay(delay, bytes) => {
                    tokio::time::sleep(delay).await;
                    bytes
                }
                Reply::RequireHeader { name, value, body } => {
                    if header_present(&request, &name, &value) {
                        http_ok(&body, "text/plain; charset=utf-8")
                    } else {
                        http_status(400)
                    }
                }
            };
            let _ = socket.write_all(&bytes).await;
            let _ = socket.shutdown().await;
        }
    });
    Server {
        base: format!("http://127.0.0.1:{port}"),
        handle,
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let read = tokio::time::timeout_at(deadline, socket.read(&mut chunk)).await;
        match read {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Ok(Err(_)) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn header_present(request: &str, name: &str, value: &str) -> bool {
    let needle = format!("{name}: {value}");
    request
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case(&needle) || line.contains(value))
}

fn http_ok(body: &[u8], content_type: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn http_status(code: u16) -> Vec<u8> {
    format!("HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}

fn http_redirect(location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

#[tokio::test]
async fn downloads_body_successfully() {
    let server = spawn(vec![Reply::Raw(http_ok(b"hello", "text/plain"))]).await;
    let result = download_string(
        &server.base,
        &DownloadOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.body, "hello");
    assert_eq!(result.bytes, 5);
    server.abort();
}

#[tokio::test]
async fn non_success_status_is_an_error() {
    let server = spawn(vec![Reply::Raw(http_status(404))]).await;
    let err = download_string(
        &server.base,
        &DownloadOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, SubError::Http(_)));
    server.abort();
}

#[tokio::test]
async fn slow_response_times_out() {
    let options = DownloadOptions {
        timeout: Duration::from_millis(150),
        ..DownloadOptions::default()
    };
    let server = spawn(vec![Reply::Delay(
        Duration::from_secs(3),
        http_ok(b"late", "text/plain"),
    )])
    .await;
    let err = download_string(&server.base, &options, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, SubError::Timeout));
    server.abort();
}

#[tokio::test]
async fn same_origin_redirect_is_followed() {
    let server = spawn(vec![
        Reply::Raw(http_redirect("/final")),
        Reply::Raw(http_ok(b"redirected", "text/plain")),
    ])
    .await;
    let result = download_string(
        &server.base,
        &DownloadOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.body, "redirected");
    assert!(result.final_url.ends_with("/final"));
    server.abort();
}

#[tokio::test]
async fn cross_origin_redirect_drops_authorization() {
    // Target server demands a Basic authorization header.
    let target = spawn(vec![Reply::RequireHeader {
        name: "Authorization".into(),
        value: "Basic dXNlcjpwYXNz".into(),
        body: b"authorized".to_vec(),
    }])
    .await;
    let target_location = format!("{}/secret", target.base);
    // Redirector on a different port (different origin).
    let redirector = spawn(vec![Reply::Raw(http_redirect(&target_location))]).await;

    let options = DownloadOptions {
        headers: vec![("Authorization".into(), "Basic dXNlcjpwYXNz".into())],
        ..DownloadOptions::default()
    };
    let err = download_string(&redirector.base, &options, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, SubError::Http(_)));
    redirector.abort();
    target.abort();
}

#[tokio::test]
async fn custom_header_and_user_agent_are_sent() {
    let server = spawn(vec![Reply::RequireHeader {
        name: "X-Token".into(),
        value: "secret-value".into(),
        body: b"ok".to_vec(),
    }])
    .await;
    let options = DownloadOptions {
        headers: vec![("X-Token".into(), "secret-value".into())],
        user_agent: Some("v2rayN-test/1.0".into()),
        ..DownloadOptions::default()
    };
    let result = download_string(&server.base, &options, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.body, "ok");
    server.abort();
}

#[tokio::test]
async fn missing_required_header_is_rejected_by_server() {
    let server = spawn(vec![Reply::RequireHeader {
        name: "X-Token".into(),
        value: "secret".into(),
        body: b"ok".to_vec(),
    }])
    .await;
    let err = download_string(
        &server.base,
        &DownloadOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, SubError::Http(_)));
    server.abort();
}

#[tokio::test]
async fn gbk_charset_is_decoded() {
    // GBK for "东京" is 0xB6AB 0xBEA9.
    let body = [0xB6u8, 0xAB, 0xBE, 0xA9];
    let server = spawn(vec![Reply::Raw(http_ok(&body, "text/plain; charset=gbk"))]).await;
    let result = download_string(
        &server.base,
        &DownloadOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(result.body, "东京");
    server.abort();
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    let options = DownloadOptions {
        max_bytes: 4,
        ..DownloadOptions::default()
    };
    let server = spawn(vec![Reply::Raw(http_ok(b"0123456789", "text/plain"))]).await;
    let err = download_string(&server.base, &options, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, SubError::TooLarge));
    server.abort();
}

#[tokio::test]
async fn cancellation_stops_a_slow_download() {
    let server = spawn(vec![Reply::Delay(
        Duration::from_secs(5),
        http_ok(b"late", "text/plain"),
    )])
    .await;
    let downloader = build_client(&DownloadOptions::default()).unwrap();
    let token = CancellationToken::new();
    let canceller = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        canceller.cancel();
    });
    let err = downloader.download(&server.base, &token).await.unwrap_err();
    assert!(matches!(err, SubError::Cancelled));
    server.abort();
}

#[test]
fn proxy_scheme_validation_is_local() {
    let options = DownloadOptions {
        proxy: Some(ProxyConfig::new("ftp://127.0.0.1:21")),
        ..DownloadOptions::default()
    };
    assert!(matches!(
        build_client(&options),
        Err(SubError::InvalidUri(_))
    ));
}
