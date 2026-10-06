//! SP-22 real-scale behavior: 10k nodes + 10k connections-scale overlays,
//! massive log flood memory trimming/throughput, connection generation
//! switch / cancel recovery, dropped-line accounting.
//!
//! Synthetic data only (`*.example.invalid`, `flood N`, `syn-final`). The one
//! stub controller binds a probed loopback port `>= 11808` (never the user's
//! live `127.0.0.1:10808`); no system proxy/registry/route/TUN change.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use application::monitor::{
    close_response_is_current, coalesce_log_batch, freeze_close_request, merge_delay_map,
    ClashApiService, LogService, MAX_LOG_BATCH_LINES,
};
use core_adapters::log_stream::LogLine;
use domain::event::{EventEnvelope, EventEpoch, EventKind, EventSeq};
use tiny_http::{Response, Server};

/// First free loopback port `>= 11808` with a bound tiny_http server.
fn floor_server() -> Server {
    for port in 11808u16..13000 {
        if port == 10_808 {
            continue;
        }
        if let Ok(server) = Server::http(("127.0.0.1", port)) {
            let bound = server.server_addr().to_ip().expect("ip addr").port();
            assert!(bound >= 11808 && bound != 10_808);
            return server;
        }
    }
    panic!("no free loopback port >= 11808");
}

fn synthetic_line(i: usize) -> LogLine {
    LogLine::new(format!("flood {i} host=node-{i}.example.invalid"))
}

fn control_marker() -> EventEnvelope {
    EventEnvelope::new(
        EventEpoch(1),
        EventSeq(1),
        EventKind::ErrorRaised,
        serde_json::json!({"detail": "synthetic stream marker"}),
    )
}

#[test]
fn sp22_scale_10k_node_delay_overlay_is_complete_and_fast() {
    // 10k probed nodes overlay by id: nothing lost, single pass, bounded time.
    let items: Vec<(String, i32)> = (0..10_000)
        .map(|i| (format!("node-{i:05}.example.invalid"), i % 900))
        .collect();
    let mut merged: HashMap<String, i32> = HashMap::new();
    let started = Instant::now();
    merge_delay_map(&mut merged, items);
    let elapsed = started.elapsed();
    eprintln!(
        "sp22 10k delay overlay: {elapsed:?} ({} items)",
        merged.len()
    );
    assert_eq!(merged.len(), 10_000);
    assert_eq!(merged.get("node-00000.example.invalid"), Some(&0));
    assert_eq!(merged.get("node-09999.example.invalid"), Some(&99));
    assert!(
        elapsed < Duration::from_secs(5),
        "10k overlay must not block, took {elapsed:?}"
    );

    // A second sweep (re-probe) overwrites by id without growing the map.
    let started = Instant::now();
    merge_delay_map(
        &mut merged,
        vec![("node-00042.example.invalid".to_string(), 7)],
    );
    assert_eq!(merged.len(), 10_000);
    assert_eq!(merged.get("node-00042.example.invalid"), Some(&7));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn sp22_scale_log_flood_memory_bounded_and_accounted() {
    // 10 bursts x 5000 lines = 50k offered against the default ring
    // (10k lines / 10 MiB). Per-burst shed heads count as rejected
    // (visible overflow); the ring evicts the rest with its own counters.
    let mut service = LogService::with_defaults();
    service.ingest_envelope(&control_marker());
    let started = Instant::now();
    for burst in 0..10 {
        let lines: Vec<LogLine> = (0..5000)
            .map(|i| synthetic_line(burst * 5000 + i))
            .collect();
        service.ingest_lines(lines);
    }
    let elapsed = started.elapsed();
    let page = service.snapshot(0, usize::MAX);
    eprintln!(
        "sp22 50k-line flood: {elapsed:?} total={} dropped_lines={} dropped_bytes={} rejected={} accepted={}",
        page.total,
        page.dropped_lines,
        page.dropped_bytes,
        service.rejected(),
        service.accepted(),
    );

    // Real memory trimming: the ring never holds more than its cap.
    assert_eq!(page.total, 10_000);
    assert!(page.dropped_bytes <= 10 * 1024 * 1024);
    // Accounting closes: 50k offered = 30k burst-shed (rejected) + 20k run
    // through, of which the ring kept 10k and evicted 10k (dropped_lines).
    assert_eq!(service.rejected(), 30_000);
    assert_eq!(service.accepted(), 20_000);
    assert_eq!(page.dropped_lines, 10_000);
    assert_eq!(service.accepted(), page.total as u64 + page.dropped_lines);
    // Control queue survives the flood; throughput stays interactive.
    assert_eq!(page.control_lines, 1);
    assert!(elapsed < Duration::from_secs(30), "took {elapsed:?}");

    // Final result marker lands after the flood and is never squeezed out.
    service.ingest_text("SPEEDTEST DONE id=syn-final delay=42ms");
    let page = service.snapshot(0, usize::MAX);
    assert!(
        page.entries.iter().any(|e| e.text.contains("syn-final")),
        "final result marker must survive the flood"
    );
    assert_eq!(page.total, 10_000);
}

#[test]
fn sp22_scale_byte_budget_trims_memory() {
    // Tight byte budget: 10k x ~200 B offered into 64 KiB.
    let mut service = LogService::new(1_000_000, 64 * 1024);
    for chunk in 0..10 {
        let lines: Vec<LogLine> = (0..1000)
            .map(|i| LogLine::new(format!("chunk{chunk} {i:0>180}")))
            .collect();
        service.ingest_lines(lines);
    }
    let page = service.snapshot(0, usize::MAX);
    eprintln!(
        "sp22 byte-budget: total={} dropped_lines={} dropped_bytes={}",
        page.total, page.dropped_lines, page.dropped_bytes,
    );
    assert!(page.dropped_bytes > 0, "overflow must be counted");
    assert_eq!(service.accepted(), page.total as u64 + page.dropped_lines);
    // ~200 B/line in 64 KiB => only a few hundred lines fit.
    assert!(page.total < 1000, "byte cap must trim, kept {}", page.total);
}

#[test]
fn sp22_scale_single_10k_burst_keeps_tail_and_reports_shed() {
    // One 10k burst through a single call: only the tail enters the ring,
    // the shed head is reported (never silently kept, never unbounded).
    let lines: Vec<LogLine> = (0..10_000).map(synthetic_line).collect();
    let (kept, dropped) = coalesce_log_batch(lines, MAX_LOG_BATCH_LINES);
    assert_eq!(kept.len(), MAX_LOG_BATCH_LINES);
    assert_eq!(dropped, 10_000 - MAX_LOG_BATCH_LINES);
    assert_eq!(
        kept.first().expect("head").text,
        "flood 8000 host=node-8000.example.invalid"
    );
    assert!(kept.last().expect("tail").text.contains("flood 9999"));
}

#[test]
fn sp22_scale_generation_switch_freezes_and_stales_close() {
    // Freeze at request time; a session switch in between stales the reply.
    let request = freeze_close_request("conn-7", 7).expect("frozen");
    assert!(close_response_is_current(request.generation, 7));
    assert!(!close_response_is_current(request.generation, 8));
    assert!(freeze_close_request("", 7).is_none());
}

// Slow delay stub: `/proxies` lists group `g` with 4 children; every
// `/delay` sleeps 300 ms so serial execution would take ~1200 ms.
fn start_slow_delay_stub() -> (u16, Arc<AtomicBool>) {
    let server = floor_server();
    let port = server.server_addr().to_ip().expect("ip addr").port();
    assert!(port >= 11808 && port != 10_808);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            let Ok(Some(request)) = server.recv_timeout(Duration::from_millis(100)) else {
                continue;
            };
            // One thread per request: concurrent probes must overlap instead
            // of queueing behind the 300 ms delay sleep (SP-22 concurrency).
            std::thread::spawn(move || serve_delay(request));
        }
    });
    (port, stop)
}

fn serve_delay(request: tiny_http::Request) {
    let url = request.url().to_string();
    let method = request.method().to_string();
    let body = if method == "GET" && url == "/proxies" {
        r#"{"proxies":{"g":{"name":"g","type":"Selector","now":"syn-n0","all":["syn-n0","syn-n1","syn-n2","syn-n3"]}}}"#.to_string()
    } else if method == "GET" && url.contains("/delay") {
        std::thread::sleep(Duration::from_millis(300));
        r#"{"delay":42}"#.to_string()
    } else {
        String::new()
    };
    let _ = request.respond(Response::from_string(body));
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("sp22 test runtime")
}

#[test]
fn sp22_scale_group_probe_cancel_recovers() {
    // Cancel: abort the in-flight group probe; nothing partial reaches the
    // caller's overlay. Recovery: the next identical call completes fully.
    let (port, stop) = start_slow_delay_stub();
    let rt = test_runtime();
    let published = Arc::new(Mutex::new(Vec::<(String, i32)>::new()));
    let seen = Arc::clone(&published);
    let probe = rt.spawn(async move {
        let client =
            ClashApiService::new(port, None, Duration::from_secs(5), Duration::from_secs(1))
                .expect("client builds");
        let mut progress: Vec<(String, i32)> = Vec::new();
        let names = client
            .group_delay_with_progress("g", &mut |name, delay| {
                progress.push((name, delay));
            })
            .await
            .expect("probe ok");
        seen.lock().unwrap().extend(progress);
        names
    });
    rt.block_on(async {
        // Every probe sleeps 300 ms; aborting at 100 ms lands mid-flight.
        tokio::time::sleep(Duration::from_millis(100)).await;
        probe.abort();
        let _ = probe.await;
    });
    assert!(
        published.lock().unwrap().is_empty(),
        "aborted probe must publish nothing partial"
    );
    drop(stop);

    // Recovery over a fresh stub: full ordered result, every item reported.
    let (port, stop) = start_slow_delay_stub();
    let service = ClashApiService::new(port, None, Duration::from_secs(5), Duration::from_secs(1))
        .expect("client builds");
    let started = Instant::now();
    let mut progress: Vec<(String, i32)> = Vec::new();
    let names = rt
        .block_on(service.group_delay_with_progress("g", &mut |name, delay| {
            progress.push((name, delay));
        }))
        .expect("recovery probe ok");
    let elapsed = started.elapsed();
    eprintln!("sp22 recovery group_delay: {elapsed:?}");
    assert_eq!(
        names,
        vec![
            "syn-n0".to_string(),
            "syn-n1".to_string(),
            "syn-n2".to_string(),
            "syn-n3".to_string()
        ]
    );
    assert_eq!(progress.len(), 4);
    let mut merged = HashMap::new();
    merge_delay_map(&mut merged, progress);
    assert_eq!(merged.len(), 4);
    assert!(elapsed < Duration::from_millis(900));
    drop(stop);
}
