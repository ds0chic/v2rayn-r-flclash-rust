//! UX-TEST-02: deterministic HTTPS speedtest probe.
//!
//! A synthetic CA (`tests/fixtures/tls/`) signs a `localhost`/`127.0.0.1`
//! server certificate. A local rustls TLS server plays the target, a minimal
//! SOCKS5 server plays the temporary test session, and `http_get_via_socks`
//! speaks real TLS through it. Every test injects its own root store, so the
//! suite is hermetic (no OS trust, no real network, no 10808).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use application::speedtest::{
    http_get_via_socks, release_test_port, reserve_free_test_port, DownloadOutcome, ProbeError,
    ProbeFailureKind, SpeedTestRunner, SpeedTestSession, SpeedTestSettings, TestNode, TestSession,
    TlsTrust,
};
use domain::{CancellationToken, DomainError, SpeedTestAction};
use rustls::pki_types::CertificateDer;
use rustls::{RootCertStore, ServerConfig, ServerConnection, StreamOwned};

const CA_PEM: &[u8] = include_bytes!("fixtures/tls/ca.pem");
const SERVER_PEM: &[u8] = include_bytes!("fixtures/tls/server.pem");
const SERVER_KEY_PEM: &[u8] = include_bytes!("fixtures/tls/server.key.pem");

fn ca_store() -> RootCertStore {
    let mut store = RootCertStore::empty();
    let mut cursor = std::io::Cursor::new(CA_PEM);
    for cert in rustls_pemfile::certs(&mut cursor) {
        store.add(cert.expect("ca cert")).expect("add ca");
    }
    store
}

fn server_config() -> Arc<ServerConfig> {
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut std::io::Cursor::new(SERVER_PEM))
            .collect::<Result<_, _>>()
            .expect("server certs");
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(SERVER_KEY_PEM))
        .expect("key parse")
        .expect("key present");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("server config");
    Arc::new(config)
}

/// A local TLS server with `/204`, `/big`, `/slow` and `/404` routes.
struct TlsServer {
    port: u16,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl TlsServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("tls bind");
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let config = server_config();
        let join = std::thread::spawn(move || {
            listener.set_nonblocking(false).ok();
            while !stop_flag.load(Ordering::Acquire) {
                let Ok((stream, _)) = listener.accept() else {
                    continue;
                };
                let config = config.clone();
                std::thread::spawn(move || serve_one(stream, config));
            }
        });
        Self {
            port,
            stop,
            join: Some(join),
        }
    }
}

fn serve_one(stream: TcpStream, config: Arc<ServerConfig>) {
    let conn = match ServerConnection::new(config) {
        Ok(conn) => conn,
        Err(_) => return,
    };
    let mut tls = StreamOwned::new(conn, stream);
    let mut request = Vec::new();
    let mut buf = [0u8; 1024];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match tls.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => request.extend_from_slice(&buf[..n]),
            Err(_) => return,
        }
        if request.len() > 16 * 1024 {
            return;
        }
    }
    let path = request
        .split(|b| *b == b' ')
        .nth(1)
        .map(|p| String::from_utf8_lossy(p).to_string())
        .unwrap_or_default();
    let (status, body): (&str, Vec<u8>) = if path.starts_with("/slow") {
        std::thread::sleep(Duration::from_secs(3));
        ("204 No Content", Vec::new())
    } else if path.starts_with("/big") {
        ("200 OK", vec![0x41u8; 4 * 1024 * 1024])
    } else if path.starts_with("/404") {
        ("404 Not Found", b"nope".to_vec())
    } else {
        ("204 No Content", Vec::new())
    };
    let header = format!(
        "HTTP/1.0 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = tls.write_all(header.as_bytes());
    let _ = tls.write_all(&body);
    let _ = tls.flush();
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// A minimal SOCKS5 no-auth CONNECT relay used as the temporary test session.
struct FakeSocks {
    port: u16,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl FakeSocks {
    fn start() -> Self {
        let port = reserve_free_test_port().expect("reserve socks test port");
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("socks bind");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let join = std::thread::spawn(move || {
            while !stop_flag.load(Ordering::Acquire) {
                let Ok((stream, _)) = listener.accept() else {
                    continue;
                };
                std::thread::spawn(move || handle_socks(stream));
            }
        });
        Self {
            port,
            stop,
            join: Some(join),
        }
    }
}

fn handle_socks(mut client: TcpStream) {
    let _ = client.set_read_timeout(Some(Duration::from_secs(5)));
    let mut greeting = [0u8; 2];
    if client.read_exact(&mut greeting).is_err() || greeting[0] != 0x05 {
        return;
    }
    let mut methods = vec![0u8; greeting[1] as usize];
    if client.read_exact(&mut methods).is_err() {
        return;
    }
    let _ = client.write_all(&[0x05, 0x00]);
    let mut head = [0u8; 4];
    if client.read_exact(&mut head).is_err() || head[1] != 0x01 {
        return;
    }
    let host = match head[3] {
        0x01 => {
            let mut a = [0u8; 4];
            if client.read_exact(&mut a).is_err() {
                return;
            }
            std::net::Ipv4Addr::from(a).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            if client.read_exact(&mut len).is_err() {
                return;
            }
            let mut name = vec![0u8; len[0] as usize];
            if client.read_exact(&mut name).is_err() {
                return;
            }
            String::from_utf8_lossy(&name).to_string()
        }
        0x04 => {
            let mut a = [0u8; 16];
            if client.read_exact(&mut a).is_err() {
                return;
            }
            std::net::Ipv6Addr::from(a).to_string()
        }
        _ => return,
    };
    let mut port_bytes = [0u8; 2];
    if client.read_exact(&mut port_bytes).is_err() {
        return;
    }
    let port = u16::from_be_bytes(port_bytes);
    // The `/404-closed` route targets a port the caller never opened.
    let target = if port == 1 {
        None
    } else {
        (host.as_str(), port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
    };
    let Some(addr) = target else {
        let _ = client.write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
        return;
    };
    let Ok(mut upstream) = TcpStream::connect(addr) else {
        let _ = client.write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
        return;
    };
    let _ = client.write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]);
    let _ = client.set_read_timeout(None);
    let mut upstream_clone = upstream.try_clone().ok();
    let mut client_clone = client.try_clone().ok();
    let relay = std::thread::spawn(move || {
        if let (Some(mut u), Some(mut c)) = (upstream_clone.take(), client_clone.take()) {
            let _ = std::io::copy(&mut c, &mut u);
        }
    });
    let _ = std::io::copy(&mut upstream, &mut client);
    let _ = relay.join();
}

impl Drop for FakeSocks {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        release_test_port(self.port);
    }
}

/// Session that talks through the fake SOCKS server using the production probe.
struct LoopbackSession {
    socks_port: u16,
}

impl SpeedTestSession for LoopbackSession {
    fn open(&self, node: &TestNode) -> Result<TestSession, DomainError> {
        Ok(TestSession {
            node: node.clone(),
            port: self.socks_port,
            handle_id: "loopback".to_string(),
        })
    }

    fn real_ping(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        trust: &TlsTrust,
        ct: &CancellationToken,
    ) -> Result<i32, ProbeError> {
        let probe = http_get_via_socks(session.port, url, trust, timeout, 0, ct)?;
        Ok((probe.header_ms as i32).max(1))
    }

    fn download(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        max_bytes: u64,
        trust: &TlsTrust,
        ct: &CancellationToken,
    ) -> Result<DownloadOutcome, ProbeError> {
        let probe = http_get_via_socks(session.port, url, trust, timeout, max_bytes, ct)?;
        let secs = probe.total.as_secs_f64().max(0.001);
        Ok(DownloadOutcome {
            mb_s: probe.body_bytes as f64 / 1_000_000.0 / secs,
            bytes: probe.body_bytes,
            elapsed: probe.total,
        })
    }

    fn close(&self, _session: TestSession) {}
}

fn node() -> TestNode {
    TestNode {
        index_id: "n1".to_string(),
        address: "127.0.0.1".to_string(),
        port: 443,
        config_type: 5,
        core_type: 1,
    }
}

fn https_url(server: &TlsServer, path: &str) -> String {
    format!("https://127.0.0.1:{}{}", server.port, path)
}

fn trust() -> TlsTrust {
    TlsTrust::custom(ca_store())
}

// ---------------------------------------------------------------------------
// Probe-level cases
// ---------------------------------------------------------------------------

#[test]
fn https_real_ping_succeeds_with_injected_root() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let probe = http_get_via_socks(
        socks.port,
        &https_url(&server, "/204"),
        &trust(),
        Duration::from_secs(10),
        0,
        &CancellationToken::new(),
    )
    .expect("https 204 must succeed");
    assert!(probe.success);
    assert_eq!(probe.status, Some(204));
    // A loopback TLS round trip can complete in under a millisecond; the
    // runner normalizes that to >=1 ms (asserted at the runner level).
}

#[test]
fn https_download_measures_and_respects_max_bytes() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let probe = http_get_via_socks(
        socks.port,
        &https_url(&server, "/big"),
        &trust(),
        Duration::from_secs(20),
        256 * 1024,
        &CancellationToken::new(),
    )
    .expect("https download must succeed");
    assert_eq!(probe.status, Some(200));
    assert!(
        probe.body_bytes >= 256 * 1024,
        "body read: {}",
        probe.body_bytes
    );
    // Bounded by max_bytes, so it must not have drained all 4 MB.
    assert!(probe.body_bytes < 4 * 1024 * 1024);
}

#[test]
fn https_untrusted_root_is_tls_failure() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let empty = TlsTrust::custom(RootCertStore::empty());
    let error = http_get_via_socks(
        socks.port,
        &https_url(&server, "/204"),
        &empty,
        Duration::from_secs(5),
        0,
        &CancellationToken::new(),
    )
    .expect_err("untrusted certificate must fail");
    assert_eq!(error.kind, ProbeFailureKind::Tls);
    assert_eq!(error.message_key(), "speedtest.tls_failed");
}

#[test]
fn https_connection_refused_is_connect_failure() {
    let socks = FakeSocks::start();
    // Port 1 makes the fake SOCKS reply "connection refused".
    let error = http_get_via_socks(
        socks.port,
        "https://127.0.0.1:1/204",
        &trust(),
        Duration::from_secs(5),
        0,
        &CancellationToken::new(),
    )
    .expect_err("refused connection must fail");
    assert_eq!(error.kind, ProbeFailureKind::Connect);
    assert_eq!(error.message_key(), "speedtest.connect_failed");
}

#[test]
fn https_slow_server_times_out() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let error = http_get_via_socks(
        socks.port,
        &https_url(&server, "/slow"),
        &trust(),
        Duration::from_millis(400),
        0,
        &CancellationToken::new(),
    )
    .expect_err("slow server must time out");
    assert_eq!(error.kind, ProbeFailureKind::Timeout);
    assert_eq!(error.message_key(), "speedtest.timeout");
}

#[test]
fn https_404_is_http_status_failure() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let error = http_get_via_socks(
        socks.port,
        &https_url(&server, "/404"),
        &trust(),
        Duration::from_secs(10),
        0,
        &CancellationToken::new(),
    )
    .expect_err("404 must be a classified status failure");
    assert_eq!(error.kind, ProbeFailureKind::HttpStatus(404));
    assert_eq!(error.message_key(), "speedtest.http_status");
}

#[test]
fn https_cancellation_is_prompt() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let ct = CancellationToken::new();
    let ct2 = ct.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        ct2.cancel();
    });
    let start = Instant::now();
    let error = http_get_via_socks(
        socks.port,
        &https_url(&server, "/slow"),
        &trust(),
        Duration::from_secs(30),
        0,
        &ct,
    )
    .expect_err("cancelled probe must fail");
    assert_eq!(error.kind, ProbeFailureKind::Cancelled);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "cancel must be prompt"
    );
}

#[test]
fn http_plain_still_works_through_socks() {
    // A tiny plain HTTP server proves the non-TLS path is unchanged.
    let listener = TcpListener::bind("127.0.0.1:0").expect("http bind");
    let port = listener.local_addr().unwrap().port();
    let join = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 512];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.0 204 No Content\r\nContent-Length: 0\r\n\r\n");
        }
    });
    let socks = FakeSocks::start();
    let probe = http_get_via_socks(
        socks.port,
        &format!("http://127.0.0.1:{port}/204"),
        &TlsTrust::Native,
        Duration::from_secs(5),
        0,
        &CancellationToken::new(),
    )
    .expect("plain http must still work");
    assert_eq!(probe.status, Some(204));
    join.join().unwrap();
}

// ---------------------------------------------------------------------------
// Runner-level cases (with injected roots)
// ---------------------------------------------------------------------------

#[test]
fn runner_realping_with_injected_root_reports_delay() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let session = Arc::new(LoopbackSession {
        socks_port: socks.port,
    });
    let settings = SpeedTestSettings {
        speed_ping_test_url: https_url(&server, "/204"),
        timeout: Duration::from_secs(10),
        ..SpeedTestSettings::default()
    };
    let runner = SpeedTestRunner::with_roots(settings, session, ca_store());
    let outcome = runner.run(
        SpeedTestAction::Realping,
        &[node()],
        &CancellationToken::new(),
        |_| {},
    );
    assert!(
        outcome.results[0].delay.unwrap_or(-1) > 0,
        "https real ping must succeed: {:?}",
        outcome.results[0]
    );
}

#[test]
fn runner_mixed_with_injected_root_measures_speed() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let session = Arc::new(LoopbackSession {
        socks_port: socks.port,
    });
    let settings = SpeedTestSettings {
        speed_ping_test_url: https_url(&server, "/204"),
        speed_test_url: https_url(&server, "/big"),
        timeout: Duration::from_secs(20),
        ..SpeedTestSettings::default()
    };
    let runner = SpeedTestRunner::with_roots(settings, session, ca_store());
    let outcome = runner.run(
        SpeedTestAction::Mixedtest,
        &[node()],
        &CancellationToken::new(),
        |_| {},
    );
    let result = &outcome.results[0];
    assert!(result.delay.unwrap_or(-1) > 0, "mixed delay: {result:?}");
    assert!(
        result.speed.unwrap_or(0.0) > 0.0,
        "https download must measure: {result:?}"
    );
}

#[test]
fn runner_tls_failure_surfaces_message_key() {
    let server = TlsServer::start();
    let socks = FakeSocks::start();
    let session = Arc::new(LoopbackSession {
        socks_port: socks.port,
    });
    let settings = SpeedTestSettings {
        speed_ping_test_url: https_url(&server, "/204"),
        timeout: Duration::from_secs(5),
        ..SpeedTestSettings::default()
    };
    let runner =
        SpeedTestRunner::with_trust(settings, session, TlsTrust::custom(RootCertStore::empty()));
    let outcome = runner.run(
        SpeedTestAction::Realping,
        &[node()],
        &CancellationToken::new(),
        |_| {},
    );
    let result = &outcome.results[0];
    assert_eq!(result.delay, Some(-1));
    assert_eq!(result.message.as_deref(), Some("speedtest.tls_failed"));
    assert!(result.failed);
}

// ---------------------------------------------------------------------------
// Port reservation regression cases
// ---------------------------------------------------------------------------

#[test]
fn reserved_test_ports_are_unique_under_concurrency() {
    let handles: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                let port = reserve_free_test_port().expect("reserve");
                assert!(port >= application::speedtest::TEST_PORT_FLOOR);
                assert_ne!(port, 10_808);
                std::thread::sleep(Duration::from_millis(20));
                release_test_port(port);
                port
            })
        })
        .collect();
    let mut ports: Vec<u16> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(ports.len(), 8, "concurrent reservations must be disjoint");
}

#[test]
fn rapid_reserve_release_cycles_do_not_conflict() {
    let mut seen = Vec::new();
    for _ in 0..25 {
        let port = reserve_free_test_port().expect("reserve");
        assert!(port >= application::speedtest::TEST_PORT_FLOOR);
        assert_ne!(port, 10_808);
        // The port must really be bindable while reserved.
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("reserved port bindable");
        drop(listener);
        release_test_port(port);
        seen.push(port);
    }
    assert_eq!(seen.len(), 25);
}

/// Start/stop lifecycle regression (`error.port_conflict`): 40 reserve ->
/// bind (core up) -> drop + release (core down) cycles. Each reserved port must
/// be immediately bindable, and the rotating cursor must never hand the
/// just-released block straight back (the Windows `TIME_WAIT` hazard).
#[test]
fn session_start_stop_cycles_never_reuse_immediately() {
    let mut seen = Vec::new();
    for _ in 0..40 {
        let port = reserve_free_test_port().expect("reserve");
        assert!(port >= application::speedtest::TEST_PORT_FLOOR);
        assert_ne!(port, 10_808);
        // "core up": the reservation maps to a really bindable listener.
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("reserved port bindable");
        // "core down": the listener is gone before the block is released.
        drop(listener);
        release_test_port(port);
        // A fresh reservation must move on rather than return the same block.
        let next = reserve_free_test_port().expect("re-reserve");
        assert_ne!(next, port, "cursor must skip the just-released block");
        release_test_port(next);
        seen.push(port);
    }
    assert_eq!(seen.len(), 40);
}

/// Two in-flight jobs (e.g. a RealPing job and a mixed job) must hold disjoint
/// test ports even though they reserve concurrently and hold the bind across a
/// short working window.
#[test]
fn concurrent_jobs_hold_disjoint_test_ports() {
    let held: std::sync::Mutex<Vec<u16>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                let port = reserve_free_test_port().expect("reserve");
                let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind while held");
                std::thread::sleep(Duration::from_millis(60));
                drop(listener);
                release_test_port(port);
                held.lock().expect("held").push(port);
            });
        }
    });
    let mut ports = held.into_inner().unwrap_or_default();
    assert_eq!(ports.len(), 2);
    ports.sort_unstable();
    ports.dedup();
    assert_eq!(ports.len(), 2, "two jobs must not share a test port");
}
