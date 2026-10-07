//! Wave B (local-only): FLD-CFG-084 convert -> parse -> replace E2E.
//!
//! A synthetic loopback converter stands in for the configured
//! `ConstItem.SubConvertUrl` service. The suite drives the real pipeline —
//! request construction (`{0}` encoding + `target`/`config`), download through
//! the converter, shared parsing, transactional group replace and reopen —
//! and proves error/cancel/partial-failure keep the old group.
//!
//! Synthetic fixtures only. Every port is pre-probed free and `>= 11808`
//! (never 10808). No real external network; no OS side effects.

use std::sync::{Arc, Mutex};

use application::runtime_client::NullRuntimeClient;
use application::subs::{refresh_subscriptions_with_convert, SubUpdateRequest};
use application::{AppEngine, SubItem, SubUpdateOutcome};
use domain::{CancellationToken, DesiredRevision};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const GOOD_VLESS: &str =
    "vless://11111111-1111-1111-1111-111111111111@converted.example:443?encryption=none#conv-node";
const BAD_GARBAGE: &str = "this is not a node {{{ no profile here";

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

fn http_ok(body: &str) -> Vec<u8> {
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(body.as_bytes());
    bytes
}

fn http_500() -> Vec<u8> {
    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\nConnection: close\r\n\r\nboom"
        .as_bytes()
        .to_vec()
}

/// Synthetic converter: captures every request line; the reply is picked by
/// routing on the (URL-encoded) source inside the request target.
struct Converter {
    base: String,
    captured: Arc<Mutex<Vec<String>>>,
    handle: tokio::task::JoinHandle<()>,
}

impl Converter {
    async fn spawn() -> Self {
        let listener = bind().await;
        let port = listener.local_addr().unwrap().port();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let seen = captured.clone();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 8192];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                seen.lock().unwrap().push(request.clone());
                // The encoded source survives as a plain substring because
                // the markers are alphanumeric.
                let reply = if request.contains("origin-bad-http") {
                    http_500()
                } else if request.contains("origin-bad-parse") {
                    http_ok(BAD_GARBAGE)
                } else {
                    http_ok(GOOD_VLESS)
                };
                let _ = socket.write_all(&reply).await;
                let _ = socket.shutdown().await;
            }
        });
        Self {
            base: format!("http://127.0.0.1:{port}"),
            captured,
            handle,
        }
    }

    fn template(&self) -> String {
        format!("{}/sub?url={{0}}", self.base)
    }

    fn requests(&self) -> Vec<String> {
        self.captured.lock().unwrap().clone()
    }

    fn abort(self) {
        self.handle.abort();
    }
}

fn set_convert_url(engine: &AppEngine, template: String) {
    let loaded = engine.load_settings().expect("load settings");
    let mut settings = loaded.settings;
    settings.const_item.sub_convert_url = Some(template);
    engine
        .save_settings(settings, loaded.revision)
        .expect("save settings");
}

fn add_sub(engine: &AppEngine, remarks: &str, url: String, target: &str) -> SubItem {
    engine
        .save_sub_item(SubItem {
            remarks: remarks.into(),
            url,
            convert_target: Some(target.into()),
            ..SubItem::default()
        })
        .expect("save sub")
}

/// Seed one subscription-sourced node so replace/preserve is observable.
fn seed_old_node(engine: &AppEngine, sub_id: &str, index_id: &str, address: &str) {
    let mut old = subscriptions::resolve_uri(
        "vless://99999999-9999-9999-9999-999999999999@old.example:443?encryption=none#keep",
    )
    .expect("fixture parses");
    old.index_id = index_id.into();
    old.subid = sub_id.into();
    old.address = address.into();
    old.is_sub = true;
    engine
        .save_profile(old, DesiredRevision::new(engine.desired_revision()))
        .expect("seed old node");
}

fn request_for(sub_id: &str) -> SubUpdateRequest {
    SubUpdateRequest {
        sub_ids: vec![sub_id.to_string()],
        via_proxy: false,
        proxy_url: None,
    }
}

#[tokio::test]
async fn wave_b_084_convert_parse_replace_reopen() {
    let converter = Converter::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_convert_url(&engine, converter.template());
    let saved = add_sub(
        &engine,
        "conv",
        format!("{}/origin-good", converter.base),
        "v2ray",
    );
    seed_old_node(&engine, &saved.id, "old-conv", "old.example");

    let report = refresh_subscriptions_with_convert(
        &engine,
        request_for(&saved.id),
        &CancellationToken::new(),
        100,
    )
    .await;
    assert_eq!(report.success_count(), 1, "{:?}", report.entries);

    // Transactional replace: the converted node lands, the old one is gone.
    let profiles = engine.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1, "{profiles:?}");
    assert_eq!(profiles[0].address, "converted.example");
    assert!(profiles.iter().all(|p| p.is_sub));

    // The converter saw one request carrying the encoded source + params.
    let requests = converter.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert!(
        requests[0].contains("url=http%3A%2F%2F127.0.0.1"),
        "source must be URL-encoded into {{0}}: {}",
        requests[0]
    );
    assert!(requests[0].contains("target=v2ray"), "{}", requests[0]);
    assert!(requests[0].contains("config="), "{}", requests[0]);

    // Reopen: converted group + converter URL both survive.
    drop(engine);
    let reopened = open(dir.path());
    let profiles = reopened.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].address, "converted.example");
    let settings = reopened.load_settings().expect("settings").settings;
    assert_eq!(
        settings.const_item.sub_convert_url.as_deref(),
        Some(converter.template().as_str())
    );
    converter.abort();
}

#[tokio::test]
async fn wave_b_084_convert_http_error_keeps_old_group() {
    let converter = Converter::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_convert_url(&engine, converter.template());
    let saved = add_sub(
        &engine,
        "conv",
        format!("{}/origin-bad-http", converter.base),
        "clash",
    );
    seed_old_node(&engine, &saved.id, "old-conv-http", "old.example");

    let report = refresh_subscriptions_with_convert(
        &engine,
        request_for(&saved.id),
        &CancellationToken::new(),
        100,
    )
    .await;
    assert_eq!(report.success_count(), 0);
    assert!(
        matches!(
            report.entries[0].outcome,
            SubUpdateOutcome::PreservedError { .. } | SubUpdateOutcome::Failed { .. }
        ),
        "converter failure must be structured: {:?}",
        report.entries[0].outcome
    );
    let profiles = engine.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].index_id, "old-conv-http");

    // Reopen: the old group is still the persisted state.
    drop(engine);
    let reopened = open(dir.path());
    let profiles = reopened.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].index_id, "old-conv-http");
    converter.abort();
}

#[tokio::test]
async fn wave_b_084_convert_unparsable_body_keeps_old_group() {
    let converter = Converter::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_convert_url(&engine, converter.template());
    let saved = add_sub(
        &engine,
        "conv",
        format!("{}/origin-bad-parse", converter.base),
        "clash",
    );
    seed_old_node(&engine, &saved.id, "old-conv-parse", "old.example");

    let report = refresh_subscriptions_with_convert(
        &engine,
        request_for(&saved.id),
        &CancellationToken::new(),
        100,
    )
    .await;
    assert_eq!(report.success_count(), 0);
    assert!(
        matches!(
            report.entries[0].outcome,
            SubUpdateOutcome::PreservedError { .. }
        ),
        "a 200 with zero parsable nodes must preserve: {:?}",
        report.entries[0].outcome
    );
    let profiles = engine.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].index_id, "old-conv-parse");
    converter.abort();
}

#[tokio::test]
async fn wave_b_084_convert_cancel_keeps_old_group_without_request() {
    let converter = Converter::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_convert_url(&engine, converter.template());
    let saved = add_sub(
        &engine,
        "conv",
        format!("{}/origin-good", converter.base),
        "v2ray",
    );
    seed_old_node(&engine, &saved.id, "old-conv-cancel", "old.example");

    let token = CancellationToken::new();
    token.cancel();
    let report =
        refresh_subscriptions_with_convert(&engine, request_for(&saved.id), &token, 100).await;
    assert!(
        report
            .entries
            .iter()
            .any(|e| e.outcome == SubUpdateOutcome::Cancelled),
        "cancelled run must report Cancelled: {:?}",
        report.entries
    );
    assert!(
        converter.requests().is_empty(),
        "a cancelled run must not hit the converter"
    );
    let profiles = engine.profiles_by_subid(&saved.id).expect("profiles");
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].index_id, "old-conv-cancel");
    converter.abort();
}

#[tokio::test]
async fn wave_b_084_convert_partial_failure_preserves_failing_group() {
    let converter = Converter::spawn().await;
    let dir = tempfile::tempdir().expect("tempdir");
    let engine = open(dir.path());
    set_convert_url(&engine, converter.template());
    let good = add_sub(
        &engine,
        "good",
        format!("{}/origin-good", converter.base),
        "v2ray",
    );
    let bad = add_sub(
        &engine,
        "bad",
        format!("{}/origin-bad-http", converter.base),
        "clash",
    );
    seed_old_node(&engine, &good.id, "old-good", "old.example");
    seed_old_node(&engine, &bad.id, "old-bad", "old.example");

    let report = refresh_subscriptions_with_convert(
        &engine,
        SubUpdateRequest {
            sub_ids: vec![],
            via_proxy: false,
            proxy_url: None,
        },
        &CancellationToken::new(),
        100,
    )
    .await;
    assert_eq!(report.success_count(), 1, "{:?}", report.entries);

    // The converted group is replaced; the failing group is untouched.
    let good_profiles = engine.profiles_by_subid(&good.id).expect("good profiles");
    assert_eq!(good_profiles.len(), 1);
    assert_eq!(good_profiles[0].address, "converted.example");
    let bad_profiles = engine.profiles_by_subid(&bad.id).expect("bad profiles");
    assert_eq!(bad_profiles.len(), 1);
    assert_eq!(bad_profiles[0].index_id, "old-bad");
    converter.abort();
}
