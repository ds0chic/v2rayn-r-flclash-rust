//! Local PAC HTTP server.
//!
//! Serves `GET /pac` with the PAC text, bound to a loopback address. The port
//! is configurable and defaults to the first free port at or above
//! [`DEFAULT_PAC_PORT_BASE`] (11808). Port 10808 is deliberately out of scope:
//! it is the user's live proxy inbound port.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{PlatformError, Result};

/// Lowest port the auto-selector will try. Never 10808.
pub const DEFAULT_PAC_PORT_BASE: u16 = 11808;
/// Default request path.
pub const DEFAULT_PAC_PATH: &str = "/pac";
/// PAC placeholder replaced with the proxy rule at serve time.
pub const PAC_PROXY_PLACEHOLDER: &str = "__PROXY__";

const ACCEPT_POLL: Duration = Duration::from_millis(10);
const IO_TIMEOUT: Duration = Duration::from_millis(500);

/// Where the PAC text comes from.
#[derive(Debug, Clone)]
pub enum PacSource {
    /// Literal PAC script.
    Inline(String),
    /// A custom PAC file (the `CustomSystemProxyPacPath` field). Validated for
    /// existence when read.
    File(PathBuf),
}

impl PacSource {
    /// Read the raw PAC text.
    pub fn read(&self) -> Result<String> {
        match self {
            PacSource::Inline(text) => Ok(text.clone()),
            PacSource::File(path) => {
                if !path.is_file() {
                    return Err(PlatformError::NotFound(path.display().to_string()));
                }
                Ok(std::fs::read_to_string(path)?)
            }
        }
    }
}

/// Replace the `__PROXY__` placeholder, mirroring upstream `PacManager`.
pub fn render_pac(template: &str, proxy_rule: &str) -> String {
    template.replace(PAC_PROXY_PLACEHOLDER, proxy_rule)
}

/// PAC server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacConfig {
    /// Loopback host. Non-loopback hosts are rejected by [`PacServer::new`].
    pub host: String,
    /// Port; `0` selects the first free port >= [`DEFAULT_PAC_PORT_BASE`].
    pub port: u16,
    /// Request path, default `/pac`.
    pub path: String,
    /// Optional proxy rule substituted for [`PAC_PROXY_PLACEHOLDER`].
    pub proxy_rule: Option<String>,
}

impl Default for PacConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 0,
            path: DEFAULT_PAC_PATH.to_string(),
            proxy_rule: None,
        }
    }
}

impl PacConfig {
    /// Config bound to an explicit port.
    pub fn with_port(port: u16) -> Self {
        Self {
            port,
            ..Self::default()
        }
    }
}

/// A running (or stopped) PAC HTTP server.
pub struct PacServer {
    config: PacConfig,
    bound: Option<SocketAddr>,
    content: Arc<RwLock<Vec<u8>>>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl PacServer {
    /// Create a stopped server. Fails if the host is not loopback.
    pub fn new(config: PacConfig) -> Result<Self> {
        if !is_loopback_host(&config.host) {
            return Err(PlatformError::Invalid(format!(
                "PAC server must bind a loopback address, got {}",
                config.host
            )));
        }
        if config.path.is_empty() || !config.path.starts_with('/') {
            return Err(PlatformError::Invalid(
                "PAC path must start with '/'".to_string(),
            ));
        }
        Ok(Self {
            config,
            bound: None,
            content: Arc::new(RwLock::new(Vec::new())),
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        })
    }

    /// Whether the accept loop is active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Actual bound address once started.
    pub fn bound_addr(&self) -> Option<SocketAddr> {
        self.bound
    }

    /// Actual port once started.
    pub fn port(&self) -> Option<u16> {
        self.bound.map(|addr| addr.port())
    }

    /// PAC URL once started.
    pub fn url(&self) -> Option<String> {
        self.bound
            .map(|addr| format!("http://{}:{}{}", addr.ip(), addr.port(), self.config.path))
    }

    /// Start serving `source`. Idempotent: a second call while running only
    /// refreshes the content and keeps the existing listener/port.
    pub fn start(&mut self, source: PacSource) -> Result<u16> {
        let text = source.read()?;
        let body = match &self.config.proxy_rule {
            Some(rule) => render_pac(&text, rule).into_bytes(),
            None => text.into_bytes(),
        };

        if self.is_running() {
            self.store_content(body);
            return Ok(self.port().unwrap_or(0));
        }

        let listener = self.bind()?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;

        self.store_content(body);
        self.bound = Some(addr);
        self.running.store(true, Ordering::SeqCst);

        let running = Arc::clone(&self.running);
        let content = Arc::clone(&self.content);
        let path = self.config.path.clone();
        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _peer)) => {
                        let _ = serve(stream, &path, &content);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(ACCEPT_POLL);
                    }
                    Err(_) => thread::sleep(ACCEPT_POLL),
                }
            }
        });
        self.handle = Some(handle);
        Ok(addr.port())
    }

    /// Stop and join the accept loop. Safe to call when already stopped.
    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    fn store_content(&self, body: Vec<u8>) {
        if let Ok(mut guard) = self.content.write() {
            *guard = body;
        }
    }

    fn bind(&self) -> Result<TcpListener> {
        if self.config.port == 0 {
            let mut last: Option<std::io::Error> = None;
            for port in DEFAULT_PAC_PORT_BASE..=u16::MAX {
                match TcpListener::bind((self.config.host.as_str(), port)) {
                    Ok(listener) => return Ok(listener),
                    Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => last = Some(e),
                    Err(e) => last = Some(e),
                }
            }
            Err(PlatformError::Backend(match last {
                Some(e) => format!("no free PAC port at or above {DEFAULT_PAC_PORT_BASE}: {e}"),
                None => "no free PAC port available".to_string(),
            }))
        } else {
            let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
                .parse()
                .map_err(|_| PlatformError::Invalid("bad PAC bind address".to_string()))?;
            TcpListener::bind(addr).map_err(|e| {
                if e.kind() == std::io::ErrorKind::AddrInUse {
                    PlatformError::PortInUse(addr)
                } else {
                    PlatformError::Io(e)
                }
            })
        }
    }
}

impl Drop for PacServer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn serve(mut stream: TcpStream, path: &str, content: &RwLock<Vec<u8>>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let target = request_line.split_whitespace().nth(1).unwrap_or("");
    let path_only = target.split('?').next().unwrap_or("");

    if path_only == path {
        let body = match content.read() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        };
        let header = format!(
            "HTTP/1.0 200 OK\r\nContent-Type: application/x-ns-proxy-autoconfig\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(&body)?;
    } else {
        let body = b"404 Not Found".to_vec();
        let header = format!(
            "HTTP/1.0 404 Not Found\r\nContent-Type: text/plain\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(&body)?;
    }
    stream.flush()
}
