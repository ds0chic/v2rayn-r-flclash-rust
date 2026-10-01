//! T15b: real end-to-end measurement through a **real Xray core**.
//!
//! The temporary test session is a genuine Xray process with a `socks` inbound
//! and a `freedom` outbound, listening on a free port `>= 11808` (never 10808).
//! A local `tiny_http` server plays the role of the ping/download target. The
//! test proves the real `TCP connect -> SOCKS5 -> HTTP` measurement path, the
//! per-node session lifecycle (open/close) and prompt cancellation, without
//! touching the user's live proxy or the managed runtime.
//!
//! Every wait is bounded; only our own child is killed.

// `DomainError` is the shared error contract and is deliberately large.
#![allow(clippy::result_large_err)]

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use application::speedtest::{
    http_get_via_socks, DownloadOutcome, SpeedTestRunner, SpeedTestSession, SpeedTestSettings,
    TestNode, TestSession,
};
use domain::{CancellationToken, DomainError, SpeedTestAction};

const XRAY_REL: &str = "tools/cores/xray/v26.3.27/xray.exe";
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// Tests in this file each spawn a real core on the shared `>= 11808` port
/// band; serialize them so the `free_port` probe cannot race.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn xray_exe() -> PathBuf {
    repo_root().join(XRAY_REL)
}

/// Pick a free TCP port at/above `base`; never 10808.
fn free_port(mut base: u16) -> u16 {
    loop {
        assert!(base < 60_000, "port scan exhausted");
        if base == 10_808 {
            base += 1;
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", base)).is_ok() {
            return base;
        }
        base += 1;
    }
}

fn wait_port(port: u16, deadline: Instant, child: &mut Child) -> Result<(), DomainError> {
    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(
                DomainError::new(domain::codes::UNAVAILABLE, "error.core_exited")
                    .with_detail(format!("exit {status:?}")),
            );
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(DomainError::new(
        domain::codes::TIMEOUT,
        "error.readiness_timeout",
    ))
}

/// A real Xray `socks -> freedom` session on an isolated port.
struct XraySocksSession {
    exe: PathBuf,
    _server_port: u16,
    children: Mutex<Vec<(String, Child, u16)>>,
}

impl XraySocksSession {
    fn new(server_port: u16) -> Self {
        Self {
            exe: xray_exe(),
            _server_port: server_port,
            children: Mutex::new(Vec::new()),
        }
    }

    fn spawn(&self, node: &TestNode) -> Result<TestSession, DomainError> {
        let port = free_port(11_808);
        let dir = tempfile::tempdir().map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(e.to_string())
        })?;
        let config = serde_json::json!({
            "log": { "loglevel": "none" },
            "inbounds": [{
                "listen": "127.0.0.1",
                "port": port,
                "protocol": "socks",
                "settings": { "udp": true, "auth": "noauth" }
            }],
            "outbounds": [{ "protocol": "freedom", "settings": {} }]
        });
        let config_path = dir.path().join("config.json");
        std::fs::write(&config_path, serde_json::to_vec(&config).unwrap()).map_err(|e| {
            DomainError::new(domain::codes::INTERNAL, "error.stage_failed")
                .with_detail(e.to_string())
        })?;

        let mut child = Command::new(&self.exe)
            .args(["run", "-c", config_path.to_str().unwrap()])
            .current_dir(self.exe.parent().unwrap())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                DomainError::new(domain::codes::UNAVAILABLE, "error.core_spawn_failed")
                    .with_detail(e.to_string())
            })?;

        let deadline = Instant::now() + READY_TIMEOUT;
        if let Err(error) = wait_port(port, deadline, &mut child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }

        let handle_id = format!("xray-{port}");
        self.children
            .lock()
            .expect("children")
            .push((handle_id.clone(), child, port));
        Ok(TestSession {
            node: node.clone(),
            port,
            handle_id,
        })
    }

    fn close_all(&self) {
        let mut children = self.children.lock().expect("children");
        for (_, mut child, _) in children.drain(..) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl SpeedTestSession for XraySocksSession {
    fn open(&self, node: &TestNode) -> Result<TestSession, DomainError> {
        self.spawn(node)
    }

    fn real_ping(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        ct: &CancellationToken,
    ) -> i32 {
        let mut best: Option<i32> = None;
        for _ in 0..2 {
            if ct.is_cancelled() {
                return -1;
            }
            if let Ok(probe) = http_get_via_socks(session.port, url, timeout, 0, ct) {
                if probe.success {
                    // A loopback RTT can be sub-millisecond; report at least
                    // 1 ms so a positive delay is observable (upstream filters
                    // non-positive samples).
                    let ms = (probe.header_ms as i32).max(1);
                    best = Some(best.map_or(ms, |b| b.min(ms)));
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        best.unwrap_or(-1)
    }

    fn download(
        &self,
        session: &TestSession,
        url: &str,
        timeout: Duration,
        max_bytes: u64,
        ct: &CancellationToken,
    ) -> DownloadOutcome {
        match http_get_via_socks(session.port, url, timeout, max_bytes, ct) {
            Ok(probe) if probe.success => {
                let secs = probe.total.as_secs_f64().max(0.001);
                DownloadOutcome {
                    mb_s: probe.body_bytes as f64 / 1_000_000.0 / secs,
                    bytes: probe.body_bytes,
                    elapsed: probe.total,
                }
            }
            _ => DownloadOutcome {
                mb_s: 0.0,
                bytes: 0,
                elapsed: Duration::ZERO,
            },
        }
    }

    fn close(&self, session: TestSession) {
        let mut children = self.children.lock().expect("children");
        if let Some(pos) = children
            .iter()
            .position(|(id, _, _)| *id == session.handle_id)
        {
            let (_, mut child, _) = children.remove(pos);
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// A tiny HTTP server with `/204`, `/big` and `/slow` endpoints.
struct TestServer {
    port: u16,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> Self {
        let server = tiny_http::Server::http("127.0.0.1:0").expect("http server");
        let port = server.server_addr().to_ip().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        let handle = std::thread::spawn(move || {
            while !stop_flag.load(Ordering::Acquire) {
                let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(100)) else {
                    continue;
                };
                let url = request.url().to_string();
                match url.as_str() {
                    u if u.starts_with("/slow") => {
                        std::thread::sleep(Duration::from_secs(4));
                        let _ = request.respond(tiny_http::Response::empty(204));
                    }
                    u if u.starts_with("/big") => {
                        let body = vec![0u8; 4 * 1024 * 1024];
                        let _ = request.respond(tiny_http::Response::from_data(body));
                    }
                    _ => {
                        let _ = request.respond(tiny_http::Response::empty(204));
                    }
                }
            }
        });
        Self {
            port,
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
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

fn assert_xray_present() {
    assert!(
        xray_exe().is_file(),
        "real Xray core missing at {}",
        xray_exe().display()
    );
}

#[test]
fn real_ping_and_download_through_real_xray() {
    let _serial = serial();
    assert_xray_present();
    let _server = TestServer::start();
    let target = _server.port;
    let session = Arc::new(XraySocksSession::new(target));
    let settings = SpeedTestSettings {
        speed_ping_test_url: format!("http://127.0.0.1:{target}/204"),
        speed_test_url: format!("http://127.0.0.1:{target}/big"),
        timeout: Duration::from_secs(15),
        ..SpeedTestSettings::default()
    };
    let runner = SpeedTestRunner::new(settings, session.clone());

    // RealPing first.
    let ping = runner.run(
        SpeedTestAction::Realping,
        &[node()],
        &CancellationToken::new(),
        |_| {},
    );
    assert_eq!(ping.results.len(), 1, "one result per node");
    assert!(
        ping.results[0].delay.unwrap_or(-1) >= 0,
        "real ping through Xray must succeed: {:?}",
        ping.results[0]
    );

    // Mixed (RealPing + download) through the same real core.
    let mixed = runner.run(
        SpeedTestAction::Mixedtest,
        &[node()],
        &CancellationToken::new(),
        |_| {},
    );
    let r = &mixed.results[0];
    assert!(r.delay.unwrap_or(-1) >= 0, "mixed delay: {r:?}");
    assert!(r.speed.unwrap_or(0.0) > 0.0, "download must measure: {r:?}");

    session.close_all();
}

#[test]
fn cancel_slow_server_is_prompt_and_closes_session() {
    let _serial = serial();
    assert_xray_present();
    let server = TestServer::start();
    let target = server.port;
    let session = Arc::new(XraySocksSession::new(target));
    let settings = SpeedTestSettings {
        speed_ping_test_url: format!("http://127.0.0.1:{target}/slow"),
        speed_test_url: format!("http://127.0.0.1:{target}/big"),
        timeout: Duration::from_secs(10),
        ..SpeedTestSettings::default()
    };
    let runner = SpeedTestRunner::new(settings, session.clone());
    let token = CancellationToken::new();
    let token2 = token.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        token2.cancel();
    });

    let start = Instant::now();
    let outcome = runner.run(SpeedTestAction::Realping, &[node()], &token, |_| {});
    canceller.join().unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "cancel must not wait for the slow server: {:?}",
        start.elapsed()
    );
    assert!(outcome.cancelled() || outcome.results.iter().all(|r| r.delay.unwrap_or(0) <= 0));
    assert!(token.is_cancelled());
    session.close_all();
}

/// Guard: the test port allocator never returns a reserved or low port.
#[test]
fn test_ports_are_isolated() {
    let _serial = serial();
    let port = free_port(11_808);
    assert!(port >= 11_808);
}
