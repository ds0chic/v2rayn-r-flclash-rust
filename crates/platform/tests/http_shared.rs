//! Shared HTTP facility tests (Wave B G-05 / FLD-CFG-087 infra layer).
//!
//! Synthetic loopback servers only: ports are probed from 11808 upwards and
//! `10808` is never touched. No external network, no proxy, no TUN.

use std::time::Duration;

use domain::CancellationToken;
use platform::http::{fetch_bytes, FetchOptions, HttpPolicy, RedirectPolicy, SharedHttpClient};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

enum Reply {
    Raw(Vec<u8>),
    Delay(Duration, Vec<u8>),
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
    for port in 11808..12100u16 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return listener;
        }
    }
    panic!("no free loopback port in 11808..12100");
}

async fn spawn(replies: Vec<Reply>) -> Server {
    let listener = bind().await;
    let port = listener.local_addr().unwrap().port();
    assert!(port >= 11808, "test port must be >= 11808, got {port}");
    assert_ne!(port, 10808, "must never bind 10808");
    let handle = tokio::spawn(async move {
        let mut replies = replies;
        replies.reverse();
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let _ = read_request(&mut socket).await;
            let Some(reply) = replies.pop() else {
                break;
            };
            let bytes = match reply {
                Reply::Raw(bytes) => bytes,
                Reply::Delay(delay, bytes) => {
                    tokio::time::sleep(delay).await;
                    bytes
                }
            };
            let _ = socket.write_all(&bytes).await;
            let _ = socket.shutdown().await;
            if replies.is_empty() {
                break;
            }
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

fn client_with_timeout(timeout: Duration) -> SharedHttpClient {
    SharedHttpClient::new(HttpPolicy {
        timeout,
        connect_timeout: Duration::from_secs(5),
        redirect: RedirectPolicy::None,
        ..HttpPolicy::test_direct()
    })
    .unwrap()
}

#[tokio::test]
async fn fetch_success_returns_body() {
    let server = spawn(vec![Reply::Raw(http_ok(b"hello", "text/plain"))]).await;
    let client = client_with_timeout(Duration::from_secs(10));
    let fetched = fetch_bytes(
        &client,
        &server.base,
        &FetchOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(fetched.body, b"hello");
    assert_eq!(fetched.text().unwrap(), "hello");
    server.abort();
}

#[tokio::test]
async fn fetch_error_status_is_reported() {
    let server = spawn(vec![Reply::Raw(http_status(404))]).await;
    let client = client_with_timeout(Duration::from_secs(10));
    let err = fetch_bytes(
        &client,
        &server.base,
        &FetchOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, platform::http::HttpError::Http(_)));
    assert!(err.to_string().contains("404"));
    server.abort();
}

#[tokio::test]
async fn fetch_timeout_is_classified() {
    let server = spawn(vec![Reply::Delay(
        Duration::from_secs(3),
        http_ok(b"late", "text/plain"),
    )])
    .await;
    let client = client_with_timeout(Duration::from_millis(150));
    let err = fetch_bytes(
        &client,
        &server.base,
        &FetchOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert_eq!(err, platform::http::HttpError::Timeout);
    server.abort();
}

#[tokio::test]
async fn fetch_redirect_is_followed_within_bound() {
    let server = spawn(vec![
        Reply::Raw(http_redirect("/final")),
        Reply::Raw(http_ok(b"redirected", "text/plain")),
    ])
    .await;
    let client = client_with_timeout(Duration::from_secs(10));
    let fetched = fetch_bytes(
        &client,
        &server.base,
        &FetchOptions::default(),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(fetched.body, b"redirected");
    assert!(fetched.final_url.ends_with("/final"));
    server.abort();
}

#[tokio::test]
async fn fetch_redirect_over_bound_is_an_error() {
    let server = spawn(vec![Reply::Raw(http_redirect("/loop"))]).await;
    let client = client_with_timeout(Duration::from_secs(10));
    let options = FetchOptions {
        max_redirects: 0,
        ..FetchOptions::default()
    };
    let err = fetch_bytes(&client, &server.base, &options, &CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, platform::http::HttpError::Redirect(_)));
    server.abort();
}

#[tokio::test]
async fn fetch_cancellation_is_observed() {
    let server = spawn(vec![Reply::Delay(
        Duration::from_secs(5),
        http_ok(b"late", "text/plain"),
    )])
    .await;
    let client = client_with_timeout(Duration::from_secs(10));
    let token = CancellationToken::new();
    let canceller = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        canceller.cancel();
    });
    let err = fetch_bytes(&client, &server.base, &FetchOptions::default(), &token)
        .await
        .unwrap_err();
    assert_eq!(err, platform::http::HttpError::Cancelled);
    server.abort();
}

#[test]
fn equal_policies_reuse_one_construction() {
    // Fetch tests build clients in parallel in the same binary, so the global
    // construction counter cannot be asserted absolutely here. Reuse is proven
    // by handle identity (`same_inner`); the exact single-construction seam is
    // asserted in the lib unit test where no parallel builders exist.
    let marker = HttpPolicy {
        timeout: Duration::from_millis(77_001),
        ..HttpPolicy::test_direct()
    };
    let first = SharedHttpClient::shared(marker.clone()).unwrap();
    let second = SharedHttpClient::shared(marker.clone()).unwrap();
    assert!(first.same_inner(&second));
    let other = SharedHttpClient::shared(HttpPolicy {
        timeout: Duration::from_millis(77_002),
        ..HttpPolicy::test_direct()
    })
    .unwrap();
    assert!(!first.same_inner(&other));
}
