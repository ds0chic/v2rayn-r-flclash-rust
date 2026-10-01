//! Loopback mock HTTP servers for the T16 updater tests.
//!
//! Every server binds `127.0.0.1` on a free port from `11808..13000`; the
//! user's live `10808` is never touched. Servers stop on drop.

#![allow(dead_code)]

pub mod archives;

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tiny_http::{Header, Response, Server, StatusCode};

pub const PORT_BASE: u16 = 11808;
pub const PORT_LIMIT: u16 = 13000;

static NEXT_PORT: AtomicU16 = AtomicU16::new(0);

/// Pick a currently free loopback port, always `>= 11808`.
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

/// A free port no mock ever binds (`61000..62000`).
pub fn pick_spare_port() -> u16 {
    for port in 61000u16..62000 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("no free spare port in 61000..62000");
}

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
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    /// When true, send `body.len()` as Content-Length then stop early.
    pub truncate: bool,
    /// Delay before responding.
    pub delay: Option<Duration>,
}

impl MockResponse {
    pub fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into().into_bytes(),
            content_type: Some("application/json".to_string()),
            truncate: false,
            delay: None,
        }
    }

    pub fn bytes(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            content_type: Some("application/octet-stream".to_string()),
            truncate: false,
            delay: None,
        }
    }

    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    pub fn truncate(mut self) -> Self {
        self.truncate = true;
        self
    }

    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

/// Blocking HTTP mock backed by `tiny_http`.
pub struct MockHttp {
    pub port: u16,
    running: Arc<AtomicBool>,
    requests: Arc<AtomicUsize>,
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
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let handler = Arc::new(handler);
        let handle = thread::spawn(move || {
            while thread_flag.load(Ordering::SeqCst) {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(request)) => {
                        counter.fetch_add(1, Ordering::Relaxed);
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
                        if let Some(delay) = response.delay {
                            thread::sleep(delay);
                        }
                        let mut out = if response.truncate {
                            let mut r = Response::from_data(response.body)
                                .with_status_code(StatusCode(response.status));
                            r.add_header(
                                Header::from_bytes(&b"Content-Length"[..], b"999999").unwrap(),
                            );
                            r
                        } else {
                            Response::from_data(response.body)
                                .with_status_code(StatusCode(response.status))
                        };
                        if let Some(content_type) = response.content_type {
                            if let Ok(header) =
                                Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                            {
                                out.add_header(header);
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
            requests,
            handle: Some(handle),
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn request_count(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }
}

impl Drop for MockHttp {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        // Do not join: a handler may be blocked writing a body to a socket the
        // client already closed (Windows loopback can stall that write). The
        // thread observes `running` between requests and exits; `pick_port`
        // skips any still-bound port so a lingering thread cannot cause reuse.
        if let Some(handle) = self.handle.take() {
            drop(handle);
        }
    }
}
