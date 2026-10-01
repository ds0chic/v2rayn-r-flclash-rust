//! Loopback mock servers for the T15 adapter tests.
//!
//! Every server binds `127.0.0.1` on a free port picked from `11808..13000`;
//! the user's live `10808` is never touched. HTTP mocks stop on drop, WS mocks
//! stop when the test runtime is torn down.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Response, Server, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::WebSocketStream;

/// First port tried by every mock, per the project's `>= 11808` rule.
pub const PORT_BASE: u16 = 11808;
pub const PORT_LIMIT: u16 = 13000;

static NEXT_PORT: AtomicU16 = AtomicU16::new(0);

/// Pick a currently free loopback port, always `>= 11808`. Concurrent tests
/// start their scan at rotating offsets to avoid racing on the same probe.
pub fn pick_port() -> u16 {
    let span = PORT_LIMIT - PORT_BASE;
    let start = NEXT_PORT.fetch_add(1, Ordering::Relaxed) % span;
    for step in 0..span {
        let port = PORT_BASE + ((start + step) % span);
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("no free loopback port in {PORT_BASE}..{PORT_LIMIT}");
}

/// Pick a free port from a range no mock ever binds (`61000..62000`) so a
/// "connection refused" test cannot race with a parallel mock server.
pub fn pick_spare_port() -> u16 {
    for port in 61000u16..62000 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("no free spare port in 61000..62000");
}

/// A received request reduced to the fields the tests assert on.
#[derive(Debug, Clone)]
pub struct MockRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl MockRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Response returned by a mock handler.
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub status: u16,
    pub body: String,
    pub content_type: Option<String>,
}

impl MockResponse {
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            content_type: Some("application/json".to_string()),
        }
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
}

/// Blocking HTTP mock backed by `tiny_http`.
pub struct MockHttp {
    pub port: u16,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl MockHttp {
    pub fn spawn<F>(handler: F) -> Self
    where
        F: Fn(&MockRequest) -> MockResponse + Send + Sync + 'static,
    {
        let port = pick_port();
        let server = Server::http(("127.0.0.1", port)).expect("bind mock http");
        let running = Arc::new(AtomicBool::new(true));
        let thread_flag = Arc::clone(&running);
        let handler = Arc::new(handler);
        let handle = thread::spawn(move || {
            while thread_flag.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(request)) => {
                        let info = MockRequest {
                            method: request.method().as_str().to_string(),
                            url: request.url().to_string(),
                            headers: request
                                .headers()
                                .iter()
                                .map(|header| {
                                    (
                                        header.field.as_str().to_string(),
                                        header.value.as_str().to_string(),
                                    )
                                })
                                .collect(),
                        };
                        let response = handler(&info);
                        let mut out = Response::from_string(response.body)
                            .with_status_code(StatusCode(response.status));
                        if let Some(content_type) = response.content_type {
                            if let Ok(header) =
                                Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                            {
                                out = out.with_header(header);
                            }
                        }
                        let _ = request.respond(out);
                    }
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Self {
            port,
            running,
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for MockHttp {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Bind a TCP listener for a mock WebSocket server on a free port.
pub async fn ws_listener() -> (u16, TcpListener) {
    for _ in 0..(PORT_LIMIT - PORT_BASE) {
        let port = pick_port();
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return (port, listener);
        }
    }
    panic!("no free loopback port for ws in {PORT_BASE}..{PORT_LIMIT}");
}

/// Complete the server side of a WebSocket handshake.
pub async fn accept_ws(listener: &TcpListener) -> WebSocketStream<tokio::net::TcpStream> {
    let (stream, _addr) = listener.accept().await.expect("accept");
    tokio_tungstenite::accept_async(stream)
        .await
        .expect("ws handshake")
}

/// `Message::Text` helper for tungstenite 0.26 (`Utf8Bytes`).
pub fn text_message(body: &str) -> tokio_tungstenite::tungstenite::Message {
    tokio_tungstenite::tungstenite::Message::Text(body.to_string().into())
}
