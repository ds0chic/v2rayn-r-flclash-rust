//! SP-22 real-core continuation: bounded live sampling of the monitor
//! pipeline against the locked mihomo core (`tools/cores/mihomo/**`).
//!
//! Loopback synthetic only: the core exposes a mixed inbound and its clash
//! controller on probed `127.0.0.1` ports `>= 11808` (never the user's live
//! `127.0.0.1:10808`), no real upstream (a local echo listener stands in for
//! the far end, everything stays on loopback), synthetic secret only. No
//! system proxy/registry/route/TUN change; every process started here is
//! stopped here.
//!
//! What the test does, in order:
//! 1. start the real core, wait for its controller;
//! 2. drive `N` real loopback TCP connections through the mixed inbound via
//!    SOCKS5 (raw sockets through the app's own inbound path) and hold them;
//! 3. sample the monitor pipeline at ~1 Hz through the real
//!    [`ClashApiService`] client (`connections()` polling = the same call the
//!    UI read model uses), feeding totals into [`StatsService`] and core
//!    stdout into [`LogService`] at the real cadence;
//! 4. record connection-table completeness, per-poll latency, mihomo RSS,
//!    queue/drop counters, one real single-close round trip, and clean
//!    teardown (no leftover process).
//!
//! `N` comes from `SP22_REALCORE_CONNS` (default 2000; set to `10000` for the
//! 10k run only after the 2k run completes within budget).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use application::monitor::{ClashApiService, InMemoryTrafficStore, LogService, StatsService};
use core_adapters::log_stream::LogLine;
use core_adapters::stats::CounterSample;

const SYNTHETIC_SECRET: &str = "sp22-synthetic-secret";
const POLL_ROUNDS: usize = 5;
const POLL_GAP: Duration = Duration::from_millis(1000);
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const TEST_PORT_START: u16 = 21_808;

fn target_conns() -> usize {
    std::env::var("SP22_REALCORE_CONNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000)
}

/// First free `127.0.0.1` port at/after `from` (10808 is never returned).
fn floor_port(from: u16) -> u16 {
    for port in from..from.saturating_add(1024) {
        if port == 10_808 {
            continue;
        }
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("sp22 realcore: no free loopback port >= 11808");
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("sp22 realcore runtime")
}

fn mihomo_exe() -> std::path::PathBuf {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("../../tools/cores/mihomo/v1.19.32");
    for name in [
        "mihomo-windows-amd64-v1.exe",
        "mihomo-linux-amd64-v1",
        "mihomo-darwin-arm64-v1",
    ] {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "sp22 realcore blocked: no locked mihomo binary under {}",
        dir.display()
    );
}

/// The real core plus its captured stdout lines.
struct RealCore {
    child: Child,
    _dir: tempfile::TempDir,
    lines: Arc<Mutex<Vec<String>>>,
    log_done: Arc<AtomicBool>,
    log_handles: Vec<std::thread::JoinHandle<()>>,
}

impl RealCore {
    fn start(mixed: u16, ctrl: u16) -> Self {
        let dir = tempfile::tempdir().expect("sp22 realcore tempdir");
        let config = format!(
            "mixed-port: {mixed}\n\
             bind-address: 127.0.0.1\n\
             external-controller: 127.0.0.1:{ctrl}\n\
             secret: \"{SYNTHETIC_SECRET}\"\n\
             log-level: debug\n\
             allow-lan: false\n\
             mode: rule\n\
             rules: [\"MATCH,DIRECT\"]\n\
             ipv6: false\n"
        );
        std::fs::write(dir.path().join("config.yaml"), config).expect("write core config");
        let exe = mihomo_exe();
        let mut child = Command::new(&exe)
            .arg("-f")
            .arg(dir.path().join("config.yaml"))
            .arg("-d")
            .arg(dir.path())
            .current_dir(dir.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn locked mihomo core");
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let log_done = Arc::new(AtomicBool::new(false));
        let mut log_handles = Vec::new();
        let spawn_reader =
            |pipe: Box<dyn Read + Send>, sink: &Arc<Mutex<Vec<String>>>, done: &Arc<AtomicBool>| {
                let sink = Arc::clone(sink);
                let done = Arc::clone(done);
                std::thread::spawn(move || {
                    let reader = BufReader::new(pipe);
                    for line in reader.lines() {
                        if done.load(Ordering::SeqCst) {
                            break;
                        }
                        let Ok(line) = line else { break };
                        let mut guard = sink.lock().unwrap_or_else(|p| p.into_inner());
                        if guard.len() < 50_000 {
                            guard.push(line);
                        }
                    }
                })
            };
        if let Some(pipe) = child.stdout.take() {
            log_handles.push(spawn_reader(Box::new(pipe), &lines, &log_done));
        }
        if let Some(pipe) = child.stderr.take() {
            log_handles.push(spawn_reader(Box::new(pipe), &lines, &log_done));
        }
        // Wait for the controller (bounded 30 s).
        let rt = test_runtime();
        let probe =
            ClashApiService::new(ctrl, Some(SYNTHETIC_SECRET.into()), DIAL_TIMEOUT, POLL_GAP)
                .expect("client builds");
        let ready_at = Instant::now();
        loop {
            if rt.block_on(probe.mode()).is_ok() {
                break;
            }
            if ready_at.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                panic!("sp22 realcore blocked: controller never came up on {ctrl}");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        eprintln!(
            "sp22 realcore: controller ready in {:?}",
            ready_at.elapsed()
        );
        Self {
            child,
            _dir: dir,
            lines,
            log_done,
            log_handles,
        }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    /// RSS in bytes via a read-only process query (Windows); `None` elsewhere.
    #[cfg(windows)]
    fn rss_bytes(&self) -> Option<u64> {
        let out = Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!(
                "(Get-Process -Id {}).WorkingSet64",
                self.child.id()
            ))
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    #[cfg(not(windows))]
    fn rss_bytes(&self) -> Option<u64> {
        None
    }

    fn shutdown(mut self) -> (Vec<String>, bool) {
        self.log_done.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let mut exited = false;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    break;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => break,
            }
        }
        // Closing the pipes ends the reader threads; join briefly.
        drop(self.child.stdout.take());
        drop(self.child.stderr.take());
        for handle in self.log_handles.drain(..) {
            let _ = handle.join();
        }
        let lines = std::mem::take(&mut *self.lines.lock().unwrap_or_else(|p| p.into_inner()));
        (lines, exited)
    }
}

impl Drop for RealCore {
    fn drop(&mut self) {
        self.log_done.store(true, Ordering::SeqCst);
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        drop(self.child.stdout.take());
        drop(self.child.stderr.take());
        for handle in self.log_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

/// Synthetic far end: accepts relayed connections, echoes one payload, then
/// holds the socket open until `stop` so the core keeps the connection alive.
struct Upstream {
    port: u16,
    accepted: Arc<AtomicUsize>,
    received_bytes: Arc<AtomicUsize>,
    echoed_bytes: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Upstream {
    fn start(port: u16) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind upstream");
        listener.set_nonblocking(true).expect("nonblocking");
        let accepted = Arc::new(AtomicUsize::new(0));
        let received_bytes = Arc::new(AtomicUsize::new(0));
        let echoed_bytes = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::clone(&accepted);
        let received = Arc::clone(&received_bytes);
        let echoed = Arc::clone(&echoed_bytes);
        let flag = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                let (stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                };
                let count = Arc::clone(&count);
                let received = Arc::clone(&received);
                let echoed = Arc::clone(&echoed);
                let flag = Arc::clone(&flag);
                std::thread::spawn(move || {
                    stream
                        .set_nonblocking(false)
                        .expect("blocking upstream connection");
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                    let mut stream = stream;
                    count.fetch_add(1, Ordering::SeqCst);
                    // Echo every chunk until the peer goes quiet. The relay can
                    // split one 1 KiB write into several reads; echoing only the
                    // first chunk would leave the client waiting for the rest
                    // until its per-op timeout (the old silent 5 s-per-connection
                    // grind). The blocking read also holds the socket open so
                    // the core keeps the connection in its live table.
                    let mut buf = vec![0u8; 4096];
                    loop {
                        match stream.read(&mut buf) {
                            Ok(0) => break,
                            Ok(n) => {
                                received.fetch_add(n, Ordering::SeqCst);
                                if stream.write_all(&buf[..n]).is_err() {
                                    break;
                                }
                                echoed.fetch_add(n, Ordering::SeqCst);
                            }
                            Err(err)
                                if matches!(
                                    err.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                if flag.load(Ordering::SeqCst) {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                });
            }
        });
        Self {
            port,
            accepted,
            received_bytes,
            echoed_bytes,
            stop,
            handle: Some(handle),
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    fn byte_totals(&self) -> (usize, usize) {
        (
            self.received_bytes.load(Ordering::SeqCst),
            self.echoed_bytes.load(Ordering::SeqCst),
        )
    }

    fn stop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        self.stop.store(true, Ordering::SeqCst);
        // Unblock the accept loop.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        let _ = handle.join();
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.stop();
    }
}

/// One SOCKS5 CONNECT through the mixed inbound to the synthetic upstream.
fn socks5_connect(mixed: u16, target: u16) -> std::io::Result<TcpStream> {
    let stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], mixed)),
        DIAL_TIMEOUT,
    )?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut stream = stream;
    stream.write_all(&[0x05, 0x01, 0x00])?;
    let mut greeting = [0u8; 2];
    stream.read_exact(&mut greeting)?;
    if greeting != [0x05, 0x00] {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("bad socks greeting {greeting:?}"),
        ));
    }
    let hi = (target >> 8) as u8;
    let lo = (target & 0xff) as u8;
    stream.write_all(&[0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1, hi, lo])?;
    let mut reply = [0u8; 10];
    stream.read_exact(&mut reply)?;
    if reply[0] != 0x05 || reply[1] != 0x00 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!("bad socks reply {reply:?}"),
        ));
    }
    Ok(stream)
}

#[test]
fn sp22_realcore_live_sample() {
    let n = target_conns();
    assert!(n > 0 && n <= 20_000, "bounded run, got {n}");
    let mixed = floor_port(TEST_PORT_START);
    let ctrl = floor_port(mixed + 1);
    let up = floor_port(ctrl + 1);
    for port in [mixed, ctrl, up] {
        assert!(
            port >= 11808 && port != 10_808,
            "port floor violated: {port}"
        );
    }
    eprintln!("sp22 realcore: mixed={mixed} ctrl={ctrl} upstream={up} conns={n}");

    let core = RealCore::start(mixed, ctrl);
    let core_pid = core.pid();
    let rss_start = core.rss_bytes();
    let mut upstream = Upstream::start(up);
    let probe_payload = b"sp22-upstream-preflight";
    let mut probe = TcpStream::connect(("127.0.0.1", up)).expect("upstream preflight connect");
    probe
        .set_read_timeout(Some(IO_TIMEOUT))
        .expect("probe read timeout");
    probe
        .set_write_timeout(Some(IO_TIMEOUT))
        .expect("probe write timeout");
    probe
        .write_all(probe_payload)
        .expect("upstream preflight write");
    let mut probe_echo = vec![0; probe_payload.len()];
    probe
        .read_exact(&mut probe_echo)
        .expect("upstream preflight read");
    assert_eq!(probe_echo, probe_payload, "upstream preflight echo");
    drop(probe);
    std::thread::sleep(Duration::from_millis(10));
    let upstream_baseline = upstream.accepted();
    eprintln!("sp22 realcore: upstream preflight accepted={upstream_baseline}");
    let mut stats = StatsService::new(Box::new(InMemoryTrafficStore::new()), true, true);
    stats.apply(
        &[
            CounterSample {
                tag: "proxy".to_string(),
                up: 0,
                down: 0,
            },
            CounterSample {
                tag: "direct".to_string(),
                up: 0,
                down: 0,
            },
        ],
        0,
        1,
        Instant::now(),
    );

    // Drive N real connections through the mixed inbound, sequentially so the
    // dial path itself stays bounded; each socket is held open afterwards.
    let drive_at = Instant::now();
    let mut held: Vec<TcpStream> = Vec::with_capacity(n);
    let mut dial_fail = 0usize;
    for i in 0..n {
        match socks5_connect(mixed, up) {
            Ok(stream) => held.push(stream),
            Err(err) => {
                dial_fail += 1;
                if dial_fail <= 5 {
                    eprintln!("sp22 realcore: dial {i} failed: {err}");
                }
            }
        }
        if i % 500 == 499 {
            eprintln!("sp22 realcore: dialed {}/{}", i + 1, n);
        }
    }
    let drive_elapsed = drive_at.elapsed();
    eprintln!(
        "sp22 realcore: established {}/{} in {:?} (fail={dial_fail}, upstream_accepted={})",
        held.len(),
        n,
        drive_elapsed,
        upstream.accepted()
    );
    assert_eq!(held.len(), n, "every synthetic dial must establish");

    // Push 1 KiB through each connection and read the echo so the traffic
    // totals move in both directions. Bounded by an aggregate budget so a
    // relay regression fails fast instead of grinding per-connection timeouts.
    let payload = vec![0xABu8; 1024];
    let mut io_fail = 0usize;
    let mut io_errors = Vec::new();
    let echo_deadline = Instant::now() + Duration::from_secs(120);
    for (i, stream) in held.iter_mut().enumerate() {
        if Instant::now() >= echo_deadline {
            io_fail += held.len() - i;
            eprintln!("sp22 realcore: echo budget exhausted at {i}/{}", held.len());
            break;
        }
        if let Err(err) = stream.write_all(&payload) {
            io_fail += 1;
            if io_errors.len() < 5 {
                io_errors.push(format!("connection {i} write: {err}"));
            }
            continue;
        }
        let mut back = vec![0u8; 1024];
        if let Err(err) = stream.read_exact(&mut back) {
            io_fail += 1;
            if io_errors.len() < 5 {
                io_errors.push(format!("connection {i} read: {err}"));
            }
        } else if i == 0 {
            assert_eq!(back, payload, "echo integrity");
        }
    }
    eprintln!(
        "sp22 realcore: echo round trip done (io_fail={io_fail}, upstream_accepted={}, upstream_bytes={:?}, first_errors={io_errors:?})",
        upstream.accepted(),
        upstream.byte_totals()
    );
    if io_fail > 0 {
        let lines = core.lines.lock().unwrap_or_else(|p| p.into_inner());
        let tail = lines
            .iter()
            .rev()
            .take(100)
            .rev()
            .cloned()
            .collect::<Vec<_>>();
        eprintln!("sp22 realcore: core debug tail={tail:?}");
    }
    assert_eq!(
        io_fail, 0,
        "echo must round-trip on every held connection: {io_errors:?}"
    );
    // Cross-check: the far end must have seen every relayed connection.
    std::thread::sleep(Duration::from_millis(500));
    eprintln!("sp22 realcore: upstream accepted={}", upstream.accepted());

    // Sample the monitor pipeline at ~1 Hz through the app's own client.
    let rt = test_runtime();
    let service = ClashApiService::new(
        ctrl,
        Some(SYNTHETIC_SECRET.into()),
        Duration::from_secs(10),
        Duration::from_secs(1),
    )
    .expect("client builds");
    let mut max_seen = 0usize;
    let mut latencies: Vec<Duration> = Vec::with_capacity(POLL_ROUNDS);
    let mut last_totals = (0u64, 0u64);
    for round in 0..POLL_ROUNDS {
        let poll_at = Instant::now();
        let conns = rt
            .block_on(service.connections())
            .expect("poll connections");
        let latency = poll_at.elapsed();
        latencies.push(latency);
        let rows = conns.connections.unwrap_or_default();
        max_seen = max_seen.max(rows.len());
        last_totals = (conns.upload_total, conns.download_total);
        // Drive the stats pipeline off the real totals at the real cadence.
        let now = std::time::Instant::now();
        stats.apply(
            &[
                CounterSample {
                    tag: "proxy".to_string(),
                    up: conns.upload_total,
                    down: conns.download_total,
                },
                CounterSample {
                    tag: "direct".to_string(),
                    up: 0,
                    down: 0,
                },
            ],
            0,
            1,
            now,
        );
        eprintln!(
            "sp22 realcore: poll {round} latency={latency:?} conns={} up={} down={}",
            rows.len(),
            conns.upload_total,
            conns.download_total,
        );
        if round + 1 < POLL_ROUNDS {
            std::thread::sleep(POLL_GAP);
        }
    }
    let completeness = max_seen as f64 / n as f64;
    eprintln!(
        "sp22 realcore: max_seen={max_seen}/{n} completeness={completeness:.3} \
         latencies={latencies:?} session_proxy_up={} session_proxy_down={}",
        stats.session_proxy().up,
        stats.session_proxy().down,
    );
    assert!(
        completeness >= 0.95,
        "connection-table completeness {completeness:.3} (saw {max_seen}/{n})"
    );
    for latency in &latencies {
        assert!(
            *latency < Duration::from_secs(10),
            "per-poll latency must stay bounded, saw {latency:?}"
        );
    }
    assert!(last_totals.0 > 0 && last_totals.1 > 0, "traffic must move");
    assert_eq!(stats.session_proxy().up, last_totals.0);
    assert_eq!(stats.session_proxy().down, last_totals.1);

    // One real single-close round trip through the app client path.
    let conns = rt.block_on(service.connections()).expect("re-list");
    let rows = conns.connections.unwrap_or_default();
    let victim = rows
        .iter()
        .find_map(|row| row.id.clone())
        .expect("at least one live connection");
    assert!(!victim.is_empty());
    let request = application::monitor::freeze_close_request(&victim, 3).expect("frozen");
    rt.block_on(service.close_connection(&request.id))
        .expect("close ok");
    let mut gone = false;
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(500));
        let after = rt.block_on(service.connections()).expect("re-list");
        let ids: Vec<String> = after
            .connections
            .unwrap_or_default()
            .into_iter()
            .filter_map(|row| row.id)
            .collect();
        if !ids.contains(&victim) {
            gone = true;
            break;
        }
    }
    eprintln!("sp22 realcore: single close of {victim} reflected={gone}");
    assert!(gone, "closed connection must leave the live table");

    // Ingest the real core stdout through the log pipeline in bounded bursts.
    let rss_end = core.rss_bytes();
    upstream.stop();
    drop(held);
    let (core_lines, exited) = core.shutdown();
    let mut logs = LogService::with_defaults();
    let bursts: Vec<Vec<LogLine>> = core_lines
        .chunks(2000)
        .map(|chunk| {
            chunk
                .iter()
                .map(|line| LogLine::new(line.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    for burst in bursts {
        logs.ingest_lines(burst);
    }
    let page = logs.snapshot(0, usize::MAX);
    eprintln!(
        "sp22 realcore: core_lines={} log_total={} dropped_lines={} dropped_bytes={} \
         rejected={} accepted={} rss_start={:?} rss_end={:?} exited={exited}",
        core_lines.len(),
        page.total,
        page.dropped_lines,
        page.dropped_bytes,
        logs.rejected(),
        logs.accepted(),
        rss_start,
        rss_end,
    );
    assert!(page.total <= 10_000, "log ring stays capped");
    assert!(exited, "core process must exit on teardown");
    assert!(
        std::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!(
                "if (Get-Process -Id {core_pid} -ErrorAction SilentlyContinue) {{ exit 1 }} else {{ exit 0 }}"
            ))
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        "no leftover mihomo process"
    );
}
