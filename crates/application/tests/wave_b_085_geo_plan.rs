//! Wave B (local-only): FLD-CFG-085 GeoSource download -> formal plan E2E.
//!
//! The stored `ConstItem.GeoSourceUrl` template must drive the canonical
//! download set, the landed `geoip.dat`/`geosite.dat` bytes must reach the
//! formal plan (`RuntimePlan.resources`, by hash) and the sing-box emission
//! must prefer the downloaded snapshot over a remote fetch. A failed or
//! cancelled pass preserves the previous files and the previous plan hashes.
//!
//! Synthetic fixtures only. Every port is pre-probed free and `>= 11808`
//! (never 10808). No real external network; no OS side effects.

use std::sync::Arc;

use application::engine::{build_resource_requests, DEFAULT_SRS_GEOSITE};
use application::runtime_client::NullRuntimeClient;
use application::AppEngine;
use domain::{
    CancellationToken, ConfigType, CoreType, DesiredRevision, Profile, RoutingProfile, RoutingRule,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const GEOIP_V1: &[u8] = b"wave-b-geoip-v1";
const GEOSITE_V1: &[u8] = b"wave-b-geosite-v1";
const SRS_BODY: &[u8] = b"wave-b-synthetic-srs";

fn open(dir: &std::path::Path) -> AppEngine {
    AppEngine::open_with_runtime(dir, Arc::new(NullRuntimeClient::new())).expect("open engine")
}

async fn bind() -> TcpListener {
    for port in 11808..11950u16 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)).await {
            return listener;
        }
    }
    panic!("no free loopback port in 11808..11950");
}

/// A surely-closed loopback port (bound then released, never 10808).
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    assert_ne!(
        port, 10808,
        "ephemeral port must never be the live proxy port"
    );
    drop(listener);
    port
}

fn free_base_port() -> u16 {
    for port in 11808..11950u16 {
        if port == 10808 {
            continue;
        }
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("no free loopback port in 11808..11950");
}

fn http_bytes(body: &[u8]) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// Synthetic resource origin: `/geoip.dat`, `/geosite.dat` and any
/// `/rule-set/*.srs` path, each with fixed bytes.
struct Origin {
    base: String,
    handle: tokio::task::JoinHandle<()>,
}

impl Origin {
    async fn spawn() -> Self {
        let listener = bind().await;
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = head
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .to_string();
                let body: &[u8] = if path == "/geoip.dat" {
                    GEOIP_V1
                } else if path == "/geosite.dat" {
                    GEOSITE_V1
                } else {
                    SRS_BODY
                };
                let _ = socket.write_all(&http_bytes(body)).await;
                let _ = socket.shutdown().await;
            }
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            handle,
        }
    }

    fn geo_template(&self) -> String {
        format!("{}/{{0}}.dat", self.base)
    }

    fn srs_template(&self) -> String {
        format!("{}/rule-set/{{1}}.srs", self.base)
    }

    fn abort(self) {
        self.handle.abort();
    }
}

fn set_sources(engine: &AppEngine, interval: i32, geo: Option<String>, srs: Option<String>) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.gui_item.auto_update_interval = interval;
    settings.const_item.geo_source_url = geo;
    settings.const_item.srs_source_url = srs;
    if let Some(first) = settings.inbound.first_mut() {
        first.local_port = i32::from(free_base_port());
    }
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn vless_leaf(id: &str) -> Profile {
    let mut profile = Profile {
        index_id: id.into(),
        config_type: ConfigType::Vless,
        core_type: Some(CoreType::SingBox),
        remarks: id.into(),
        address: "192.0.2.10".into(),
        port: 443,
        password: "11111111-2222-3333-4444-555555555555".into(),
        ..Default::default()
    };
    profile.proto_extra.vless_encryption = Some("none".into());
    profile
}

fn seed(engine: &AppEngine) {
    let revision = engine.desired_revision();
    engine
        .save_profile(vless_leaf("n1"), DesiredRevision::new(revision))
        .expect("save node");
    let mut scheme = RoutingProfile {
        remarks: "wave-b-085".into(),
        ..Default::default()
    };
    scheme
        .set_rules(&[RoutingRule {
            id: "r1".into(),
            outbound_tag: Some("proxy".into()),
            domain: Some(vec!["geosite:google".into()]),
            enabled: true,
            remarks: Some("r1".into()),
            rule_type: Some(domain::RuleType::Routing),
            ..Default::default()
        }])
        .expect("set rules");
    let saved = engine.save_routing(scheme).expect("save routing");
    engine
        .set_default_routing(&saved.id)
        .expect("set default routing");
}

fn bin_dir_of(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join("bin")
}

#[tokio::test]
async fn wave_b_085_geo_download_reaches_plan_and_emission_across_reopen() {
    let origin = Origin::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_sources(
        &engine,
        1,
        Some(origin.geo_template()),
        Some(origin.srs_template()),
    );
    seed(&engine);
    let bin = bin_dir_of(dir.path());
    std::fs::create_dir_all(&bin).expect("bin dir");

    // The canonical download set consumes the stored templates.
    let loaded = engine.load_settings().expect("settings").settings;
    let requests = build_resource_requests(&loaded.const_item, &bin);
    assert_eq!(requests.len(), 2 + DEFAULT_SRS_GEOSITE.len());
    assert!(
        requests
            .iter()
            .all(|r| r.url.starts_with(origin.base.as_str())),
        "every request must use the stored loopback source: {:?}",
        requests.iter().map(|r| &r.url).collect::<Vec<_>>()
    );
    assert_eq!(requests[0].target, bin.join("geoip.dat"));
    assert_eq!(requests[1].target, bin.join("geosite.dat"));

    // The resource pass lands the exact bytes.
    let report = engine
        .run_resource_pass(&bin, 1, &CancellationToken::new())
        .await
        .expect("resource pass");
    assert!(report.due);
    assert!(report.failed.is_empty(), "failures: {:?}", report.failed);
    assert_eq!(
        std::fs::read(bin.join("geoip.dat")).unwrap().as_slice(),
        GEOIP_V1
    );
    assert_eq!(
        std::fs::read(bin.join("geosite.dat")).unwrap().as_slice(),
        GEOSITE_V1
    );

    // The formal plan carries the landed assets by hash.
    let expected = application::dns::geo_asset_hashes(&bin);
    assert_eq!(expected.len(), 2);
    assert_eq!(
        expected[0],
        domain::ContentHash::new(runtime::sha256_hex(GEOIP_V1))
    );
    let plan = engine
        .build_runtime_plan("n1", engine.desired_revision())
        .expect("runtime plan");
    assert_eq!(plan.resources, expected, "plan must consume the download");

    // The emission prefers the downloaded snapshot: no remote rule_set for
    // the downloaded tag, and the stored SRS template reaches generation.
    let opts = engine.runtime_codegen_options();
    assert_eq!(
        opts.bin_directory,
        bin.to_string_lossy().into_owned(),
        "plan context must resolve to the download directory"
    );
    assert!(opts.local_srs_files.contains("geosite-google"));
    let input = engine
        .build_codegen_input("n1", CoreType::SingBox, &opts)
        .expect("codegen input");
    assert_eq!(
        input.settings.ruleset_url.as_deref(),
        Some(origin.srs_template().as_str())
    );
    let generated = application::codegen::generate(CoreType::SingBox, &input).expect("generate");
    let rule_sets = generated
        .main
        .pointer("/route/rule_set")
        .and_then(serde_json::Value::as_array)
        .expect("route.rule_set array");
    let entry = rule_sets
        .iter()
        .find(|e| e.get("tag").and_then(serde_json::Value::as_str) == Some("geosite-google"))
        .expect("geosite-google rule_set entry");
    assert_eq!(
        entry.get("type").and_then(serde_json::Value::as_str),
        Some("local"),
        "downloaded snapshot must not fetch remote: {entry}"
    );

    // Reopen: sources persist, assets persist, plan hashes are identical.
    drop(engine);
    let reopened = open(dir.path());
    let settings = reopened.load_settings().expect("settings").settings;
    assert_eq!(
        settings.const_item.geo_source_url.as_deref(),
        Some(origin.geo_template().as_str())
    );
    let reopened_plan = reopened
        .build_runtime_plan("n1", reopened.desired_revision())
        .expect("reopen plan");
    assert_eq!(reopened_plan.resources, expected);
    origin.abort();
}

#[tokio::test]
async fn wave_b_085_failed_pass_preserves_old_files_and_plan_hashes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = bin_dir_of(dir.path());
    std::fs::create_dir_all(&bin).expect("bin dir");
    std::fs::write(bin.join("geoip.dat"), b"OLD-GEOIP").expect("old geoip");
    std::fs::write(bin.join("geosite.dat"), b"OLD-GEOSITE").expect("old geosite");

    let engine = open(dir.path());
    let dead = closed_port();
    set_sources(
        &engine,
        1,
        Some(format!("http://127.0.0.1:{dead}/{{0}}.dat")),
        Some(format!("http://127.0.0.1:{dead}/rule-set/{{1}}.srs")),
    );
    seed(&engine);
    let before = application::dns::geo_asset_hashes(&bin);
    assert_eq!(before.len(), 2);

    let report = engine
        .run_resource_pass(&bin, 1, &CancellationToken::new())
        .await
        .expect("resource pass");
    assert!(report.due);
    assert!(!report.ok(), "an unreachable origin must fail the pass");
    assert!(report.downloaded.is_empty());
    assert_eq!(
        std::fs::read(bin.join("geoip.dat")).unwrap().as_slice(),
        b"OLD-GEOIP".as_slice()
    );
    assert_eq!(
        std::fs::read(bin.join("geosite.dat")).unwrap().as_slice(),
        b"OLD-GEOSITE".as_slice()
    );
    assert!(
        !bin.join("geoip.part").exists(),
        "staging must be cleaned up"
    );

    // The plan still references the preserved files, byte for byte.
    let plan = engine
        .build_runtime_plan("n1", engine.desired_revision())
        .expect("runtime plan");
    assert_eq!(plan.resources, before);
}

#[tokio::test]
async fn wave_b_085_cancelled_pass_writes_nothing() {
    let origin = Origin::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = bin_dir_of(dir.path());
    let engine = open(dir.path());
    set_sources(
        &engine,
        1,
        Some(origin.geo_template()),
        Some(origin.srs_template()),
    );

    let token = CancellationToken::new();
    token.cancel();
    let report = engine
        .run_resource_pass(&bin, 1, &token)
        .await
        .expect("resource pass");
    assert!(report.downloaded.is_empty());
    assert!(
        !bin.join("geoip.dat").exists() && !bin.join("geosite.dat").exists(),
        "a cancelled pass must land no files"
    );
    origin.abort();
}
