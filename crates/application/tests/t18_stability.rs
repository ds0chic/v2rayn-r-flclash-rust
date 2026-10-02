//! T18 performance/stability integration tests.
//!
//! These run against the real in-process components (ring buffer, SQLite
//! engine). They print one machine-readable `T18_*` line per measurement so the
//! `benchmarks/T18/*.ps1` runner can capture raw numbers without guessing.
//! No network, no live installation, no 10808.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use application::monitor::{DEFAULT_MAX_LOG_BYTES, DEFAULT_MAX_LOG_LINES};
use application::runtime_client::NullRuntimeClient;
use application::{AppEngine, LogService};
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn open(dir: &Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[test]
fn log_ring_throughput_100k_is_bounded() {
    let mut logs = LogService::with_defaults();
    let lines: Vec<String> = (0..100_000)
        .map(|i| format!("2026-01-01 00:00:00 [info] synthetic log line {i}"))
        .collect();

    let start = Instant::now();
    for line in &lines {
        logs.ingest_text(line.clone());
    }
    let elapsed = start.elapsed();

    let page = logs.snapshot(0, usize::MAX);
    let retained = page.total;
    let dropped = logs.dropped_lines();
    let per_line_us = elapsed.as_secs_f64() * 1_000_000.0 / lines.len() as f64;

    assert!(
        retained <= DEFAULT_MAX_LOG_LINES,
        "ring retained {retained} > cap {DEFAULT_MAX_LOG_LINES}"
    );
    assert_eq!(
        retained as u64 + dropped,
        100_000,
        "ingested + dropped must equal injected"
    );
    assert!(per_line_us < 50.0, "ingest too slow: {per_line_us} us/line");

    println!(
        "T18_LOG_BENCH {{\"injected\":100000,\"retained\":{retained},\
\"dropped\":{dropped},\"max_lines\":{DEFAULT_MAX_LOG_LINES},\
\"max_bytes\":{DEFAULT_MAX_LOG_BYTES},\"elapsed_ms\":{:.3},\"per_line_us\":{:.3}}}",
        elapsed.as_secs_f64() * 1000.0,
        per_line_us
    );
}

#[test]
fn log_ring_bounds_bytes_for_large_lines() {
    let mut logs = LogService::with_defaults();
    let big = "x".repeat(64 * 1024);
    for i in 0..2000 {
        logs.ingest_text(format!("{big} {i}"));
    }
    let page = logs.snapshot(0, usize::MAX);
    let dropped_bytes = page.dropped_bytes;
    assert!(
        dropped_bytes > 0,
        "64MiB injected against a 10MiB cap must drop bytes"
    );
    println!(
        "T18_LOG_BYTES {{\"retained\":{},\"dropped_bytes\":{dropped_bytes},\"max_bytes\":{DEFAULT_MAX_LOG_BYTES}}}",
        page.total
    );
}

#[test]
fn engine_open_failure_on_bad_path_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not_a_dir");
    std::fs::write(&blocker, b"x").unwrap();
    // The data dir is a child of a *file*: create_dir_all must fail and the
    // engine must surface a structured error rather than panic.
    let result =
        AppEngine::open_with_runtime(blocker.join("data"), Arc::new(NullRuntimeClient::new()));
    let error = match result {
        Ok(_) => panic!("opening under a file must fail"),
        Err(error) => error,
    };
    println!("T18_IO_ERROR {{\"code\":\"{}\"}}", error.code);
}

#[test]
fn settings_write_failure_is_reported_and_old_value_kept() {
    let dir = tempfile::tempdir().unwrap();
    let engine = open(dir.path());
    let loaded = engine.load_settings().unwrap();
    let mut settings = loaded.settings;
    settings.core_basic_item.loglevel = Some("debug".into());
    engine
        .save_settings(settings.clone(), loaded.revision)
        .expect("first save");
    let revision = engine.settings_revision();

    // Simulate a write failure: the config path becomes a directory so the
    // atomic persist cannot replace it.
    let config_path = dir.path().join("guiNConfig.json");
    std::fs::remove_file(&config_path).unwrap();
    std::fs::create_dir(&config_path).unwrap();
    let mut next = engine.load_settings().unwrap().settings;
    next.core_basic_item.loglevel = Some("trace".into());
    let error = engine
        .save_settings(next, revision)
        .expect_err("persist must fail");
    // The in-memory tree keeps the previous value (no partial commit).
    let after = engine.load_settings().unwrap();
    assert_eq!(
        after.settings.core_basic_item.loglevel.as_deref(),
        Some("debug")
    );
    println!(
        "T18_IO_ERROR {{\"phase\":\"settings_save\",\"code\":\"{}\",\"old_value_kept\":true}}",
        error.code
    );
}

#[test]
fn app_reopen_ten_times_has_no_drift() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("guiNConfig.json");

    // Seed one profile and one non-default setting.
    let seed_hash;
    let seed_revision;
    {
        let engine = open(dir.path());
        let mut loaded = engine.load_settings().unwrap();
        loaded.settings.core_basic_item.loglevel = Some("debug".into());
        if let Some(first) = loaded.settings.inbound.first_mut() {
            first.local_port = 11808;
        }
        let outcome = engine
            .save_settings(loaded.settings, loaded.revision)
            .expect("save settings");
        seed_revision = outcome.new_revision;
        let profile = Profile {
            index_id: "t18-seed".into(),
            config_type: ConfigType::Vless,
            core_type: Some(CoreType::Xray),
            remarks: "T18 seed".into(),
            address: "192.0.2.10".into(),
            port: 443,
            ..Default::default()
        };
        engine
            .save_profile(profile, DesiredRevision::ZERO)
            .expect("save profile");
        drop(engine);
        seed_hash = sha256_hex(&std::fs::read(&config_path).expect("config exists"));
        assert!(seed_hash.len() == 64);
    }

    let mut observed = Vec::new();
    for iteration in 0..10 {
        let engine = open(dir.path());
        let loaded = engine.load_settings().unwrap();
        assert_eq!(loaded.revision, seed_revision, "settings revision drift");
        assert_eq!(
            loaded.settings.core_basic_item.loglevel.as_deref(),
            Some("debug"),
            "loglevel drift at iteration {iteration}"
        );
        assert_eq!(engine.profile_count(), 1, "profile count drift");
        let profile = engine
            .profile_by_id("t18-seed")
            .unwrap()
            .expect("seed profile present");
        assert_eq!(profile.address, "192.0.2.10");
        drop(engine);
        let hash = sha256_hex(&std::fs::read(&config_path).expect("config exists"));
        assert_eq!(
            hash, seed_hash,
            "config hash drift at iteration {iteration}"
        );
        observed.push(hash);
    }

    println!(
        "T18_REOPEN {{\"iterations\":10,\"config_sha256\":\"{seed_hash}\",\
\"settings_revision\":{seed_revision},\"profiles\":1,\"drift\":0}}"
    );
}
