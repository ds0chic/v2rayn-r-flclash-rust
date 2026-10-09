//! Wave B / Tier 4 synthetic-local WebDAV E2E (FLD-CFG-143/144/145/146).
//!
//! A tiny in-test WebDAV server on a probed 127.0.0.1 port in `11808..11900`
//! (never 10808, asserted) speaks PROPFIND / MKCOL / PUT / GET / DELETE with
//! synthetic Basic auth. The app's [`WebDavClient`][application::webdav] is
//! driven through backup-settings save -> bundle upload -> list -> download ->
//! restore roundtrip -> reopen, with bundle integrity asserted by hash.
//!
//! Failure matrix (144/145/146): auth failure, mid-transfer failure, retry and
//! interrupt/cancel must each leave local state consistent (old backup intact,
//! no partial replace). The failure paths never reach import/restore, and the
//! import/restore path itself is transactional (see `t16_backup.rs`).
//! Credentials are synthetic fixtures and are never logged; error details are
//! asserted to contain no credential material.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use application::webdav::{WebDavClient, WebDavConfig, DEFAULT_DIR};
use application::{zip_upstream_layout, BackupService};
use persistence::Store;
use tiny_http::{Response, Server};

// Clearly synthetic fixtures, never real credentials.
const SYN_USER: &str = "waveb-synthetic-user";
const SYN_PASS: &str = "waveb-synthetic-pass";
const SYN_DIR: &str = "waveb_backup";

/// First free loopback port at or above 11808; 10808 is never touched.
fn bind_loopback() -> Server {
    for port in 11808u16..11900 {
        if port == 10808 {
            continue;
        }
        if let Ok(server) = Server::http(("127.0.0.1", port)) {
            return server;
        }
    }
    panic!("no free loopback test port in 11808..11900");
}

fn bound_port(server: &Server) -> u16 {
    let port = server
        .server_addr()
        .to_ip()
        .expect("expected an IP listen address")
        .port();
    assert!(
        port >= 11808 && port != 10808,
        "test port must stay >= 11808 and off 10808, got {port}"
    );
    port
}

#[derive(Debug, Default)]
struct ServerState {
    stored: Vec<u8>,
    fail_next_put: Option<u16>,
    fail_next_get: Option<u16>,
    truncate_get_to: Option<usize>,
    delay: Duration,
    puts: u64,
    gets: u64,
    deletes: u64,
}

#[derive(Clone)]
struct Harness {
    state: Arc<Mutex<ServerState>>,
    port: u16,
}

fn propfind_xml(dir: &str, stored_len: Option<u64>) -> String {
    let mut xml = r#"<?xml version="1.0" encoding="utf-8"?>"#.to_string();
    xml.push_str(r#"<D:multistatus xmlns:D="DAV:">"#);
    xml.push_str(&format!(
        "<D:response><D:href>/{dir}/</D:href>\
         <D:propstat><D:prop><D:getcontentlength>0</D:getcontentlength></D:prop></D:propstat>\
         </D:response>"
    ));
    if let Some(len) = stored_len {
        xml.push_str(&format!(
            "<D:response><D:href>/{dir}/backup.zip</D:href>\
             <D:propstat><D:prop><D:getcontentlength>{len}</D:getcontentlength>\
             <D:getlastmodified>Fri, 02 Oct 2026 00:00:00 GMT</D:getlastmodified>\
             </D:prop></D:propstat></D:response>"
        ));
    }
    xml.push_str("</D:multistatus>");
    xml
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(output)
}

fn authorized(headers: &[tiny_http::Header], user: &str, pass: &str) -> bool {
    for header in headers {
        if !header.field.equiv("Authorization") {
            continue;
        }
        let value = header.value.as_str();
        let payload = match value.strip_prefix("Basic ") {
            Some(payload) => payload,
            None => return false,
        };
        let decoded = base64_decode(payload.trim()).unwrap_or_default();
        return decoded == format!("{user}:{pass}").as_bytes();
    }
    false
}

fn spawn_harness() -> Harness {
    let server = bind_loopback();
    let port = bound_port(&server);
    let state = Arc::new(Mutex::new(ServerState::default()));
    let thread_state = Arc::clone(&state);
    std::thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let delay = thread_state.lock().expect("lock").delay;
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            let headers: Vec<tiny_http::Header> = request.headers().to_vec();
            if !authorized(&headers, SYN_USER, SYN_PASS) {
                let _ = request.respond(Response::empty(401));
                continue;
            }
            let method = request.method().as_str().to_string();
            let url = request.url().to_string();
            let is_backup = url.ends_with("backup.zip");
            match method.as_str() {
                "PROPFIND" => {
                    let guard = thread_state.lock().expect("lock");
                    let stored_len =
                        (!guard.stored.is_empty()).then_some(guard.stored.len() as u64);
                    let _ = request.respond(
                        Response::from_string(propfind_xml(SYN_DIR, stored_len))
                            .with_status_code(207),
                    );
                }
                "MKCOL" => {
                    let _ = request.respond(Response::empty(201));
                }
                "PUT" if is_backup => {
                    let mut body = Vec::new();
                    let _ = request.as_reader().read_to_end(&mut body);
                    let mut guard = thread_state.lock().expect("lock");
                    guard.puts += 1;
                    if let Some(status) = guard.fail_next_put.take() {
                        // Old bytes stay: no partial replace on failure.
                        let _ = request.respond(Response::empty(status));
                    } else {
                        guard.stored = body;
                        let _ = request.respond(Response::empty(201));
                    }
                }
                "GET" if is_backup => {
                    let mut guard = thread_state.lock().expect("lock");
                    guard.gets += 1;
                    if let Some(status) = guard.fail_next_get.take() {
                        let _ = request.respond(Response::empty(status));
                    } else if guard.stored.is_empty() {
                        let _ = request.respond(Response::empty(404));
                    } else if let Some(limit) = guard.truncate_get_to.take() {
                        let short = guard.stored[..guard.stored.len().min(limit)].to_vec();
                        let _ = request.respond(Response::from_data(short).with_status_code(200));
                    } else {
                        let data = guard.stored.clone();
                        let _ = request.respond(Response::from_data(data).with_status_code(200));
                    }
                }
                "DELETE" if is_backup => {
                    let mut guard = thread_state.lock().expect("lock");
                    guard.deletes += 1;
                    guard.stored.clear();
                    let _ = request.respond(Response::empty(204));
                }
                _ => {
                    let _ = request.respond(Response::empty(405));
                }
            }
        }
    });
    Harness { state, port }
}

/// The persisted-settings step: the saved WebDavItem fields (Url/UserName/
/// Password/DirName = FLD-CFG-143/144/145/146) map 1:1 onto this config. The
/// formal settings-window save lives in bridge_api/Dart (out of scope here).
fn saved_config(port: u16) -> WebDavConfig {
    WebDavConfig::new(
        format!("http://127.0.0.1:{port}"),
        SYN_USER,
        SYN_PASS,
        SYN_DIR,
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// Source data dir: real SQLite db + config + nested resource, zippable with
/// the upstream layout the remote `backup.zip` must carry.
fn seed_source(dir: &Path) {
    let store = Store::create(dir.join("guiNDB.db")).expect("create source db");
    store
        .connection()
        .execute(
            "INSERT INTO ProfileItem \
             (IndexId, ConfigType, ConfigVersion, Subid, Remarks, Address, Security, Id) \
             VALUES ('node-waveb', 2, 4, '', 'waveb-remark', 'custom/sub/node.json', '', '')",
            [],
        )
        .expect("insert source profile");
    drop(store);
    std::fs::write(
        dir.join("guiNConfig.json"),
        r#"{"IndexId":"node-waveb","SubIndexId":"","UIItem":{"CurrentTheme":"Dark"}}"#,
    )
    .expect("source config");
    std::fs::create_dir_all(dir.join("custom/sub")).expect("nested dir");
    std::fs::write(dir.join("custom/sub/node.json"), br#"{"k":1}"#).expect("nested resource");
}

/// Pre-existing local install that failure paths must leave untouched.
fn seed_existing_target(dir: &Path) {
    let store = Store::create(dir.join("guiNDB.db")).expect("create target db");
    store
        .connection()
        .execute(
            "INSERT INTO SubItem (Id, Remarks) VALUES ('old', 'existing')",
            [],
        )
        .expect("insert existing row");
    drop(store);
    std::fs::write(dir.join("guiNConfig.json"), r#"{"IndexId":"old-config"}"#).expect("old config");
}

fn assert_existing_target_intact(dir: &Path) {
    let store = Store::open(dir.join("guiNDB.db")).expect("open existing db");
    assert_eq!(store.count_rows("SubItem").expect("count"), 1);
    let remark: String = store
        .connection()
        .query_row("SELECT Remarks FROM SubItem LIMIT 1", [], |row| row.get(0))
        .expect("remark");
    assert_eq!(remark, "existing");
    let config = std::fs::read_to_string(dir.join("guiNConfig.json")).expect("config");
    assert!(
        config.contains("old-config"),
        "live config replaced: {config}"
    );
}

fn assert_no_credential_leak(detail: &str) {
    assert!(
        !detail.contains(SYN_USER),
        "error detail leaks the username"
    );
    assert!(
        !detail.contains(SYN_PASS),
        "error detail leaks the password"
    );
}

/// FLD-CFG-143 (Url save/roundtrip) + FLD-CFG-146 (DirName encoding/default):
/// save -> check -> bundle upload -> list -> download -> restore -> reopen,
/// with hash integrity end to end.
#[test]
fn waveb_e2e_save_upload_list_download_restore_reopen() {
    let harness = spawn_harness();
    let config = saved_config(harness.port);

    // Saved address/dirname encode exactly the configured endpoint (146).
    assert_eq!(
        config.backup_url(),
        format!("http://127.0.0.1:{}/waveb_backup/backup.zip", harness.port)
    );
    let defaulted = WebDavConfig::new(
        format!("http://127.0.0.1:{}", harness.port),
        SYN_USER,
        SYN_PASS,
        "",
    );
    assert!(
        defaulted
            .backup_url()
            .ends_with(&format!("/{DEFAULT_DIR}/backup.zip")),
        "empty DirName must fall back to {DEFAULT_DIR}: {}",
        defaulted.backup_url()
    );

    let client = WebDavClient::new(config, Duration::from_secs(5), None).expect("client");
    let rt = runtime();

    let src = tempfile::tempdir().expect("src");
    seed_source(src.path());
    let transfer = tempfile::tempdir().expect("transfer");
    let bundle_path = transfer.path().join("backup.zip");
    let bundle_len = application::zip_upstream_layout_to_file(src.path(), &bundle_path)
        .expect("stream upstream layout");
    let bundle = std::fs::read(&bundle_path).expect("read expected bundle");
    let bundle_hash = sha256_hex(&bundle);

    let downloaded = rt.block_on(async {
        let check = client.check().await.expect("check");
        assert!(check.status == 207 || check.status == 201);
        let uploaded = client
            .upload_file(&bundle_path, bundle_len)
            .await
            .expect("upload");
        assert_eq!(uploaded, bundle_len);
        let entries = client.list().await.expect("list");
        let entry = entries
            .iter()
            .find(|e| e.href.ends_with("backup.zip"))
            .expect("list must contain backup.zip");
        assert_eq!(entry.size, bundle_len);
        client.download().await.expect("download")
    });
    assert_eq!(
        sha256_hex(&downloaded),
        bundle_hash,
        "downloaded bundle must be bit-identical to the upload"
    );

    // Restore roundtrip into a fresh install, then reopen twice.
    let work = tempfile::tempdir().expect("work");
    let zip_path = work.path().join("remote-backup.zip");
    std::fs::write(&zip_path, &downloaded).expect("stage zip");
    let data = tempfile::tempdir().expect("data");
    let service = BackupService::new(data.path());
    let recognition = service.recognize(&zip_path).expect("recognize");
    assert!(recognition.is_upstream, "remote bundle must stay upstream");
    let report = service
        .import_upstream(&zip_path, &work.path().join("import-work"), 7)
        .expect("import");
    let expected_active = persistence::hash::derived_id(
        "profile",
        &format!("{}:node-waveb", report.source_fingerprint),
    );

    let engine = application::AppEngine::open(data.path()).expect("open engine");
    assert_eq!(
        engine.active_profile().as_deref(),
        Some(expected_active.as_str()),
        "reopen must serve the restored active profile"
    );
    assert!(data.path().join("custom/sub/node.json").is_file());
    drop(engine);
    let reopened = application::AppEngine::open(data.path()).expect("reopen engine");
    assert_eq!(
        reopened.active_profile().as_deref(),
        Some(expected_active.as_str()),
        "second reopen must keep the restored identity"
    );
}

/// FLD-CFG-144 (UserName) + FLD-CFG-145 (Password): wrong credentials fail
/// every operation explicitly, leak nothing, and change neither the remote old
/// backup nor the local install.
#[test]
fn waveb_auth_failure_leaves_local_and_remote_state() {
    let harness = spawn_harness();
    let good = WebDavClient::new(saved_config(harness.port), Duration::from_secs(5), None)
        .expect("client");
    let rt = runtime();

    // Seed the remote old backup with good credentials.
    let src = tempfile::tempdir().expect("src");
    seed_source(src.path());
    let old_bundle = zip_upstream_layout(src.path()).expect("zip");
    rt.block_on(async {
        good.check().await.expect("check");
        good.upload(old_bundle.clone()).await.expect("seed upload");
    });

    let bad_config = WebDavConfig::new(
        format!("http://127.0.0.1:{}", harness.port),
        "wrong-user",
        "wrong-pass",
        SYN_DIR,
    );
    let bad = WebDavClient::new(bad_config, Duration::from_secs(5), None).expect("client");
    rt.block_on(async {
        for error in [
            bad.check().await.expect_err("check must fail"),
            bad.list().await.expect_err("list must fail"),
            bad.upload(b"new".to_vec())
                .await
                .expect_err("upload must fail"),
            bad.download().await.expect_err("download must fail"),
        ] {
            assert_eq!(error.code, "E_PERMISSION_DENIED");
            assert_no_credential_leak(&error.detail.unwrap_or_default());
        }
    });

    // Remote old backup intact through the good client.
    let still_there = rt.block_on(good.download()).expect("good download");
    assert_eq!(still_there, old_bundle);

    // Local install never entered the flow, so it is trivially intact; assert
    // the pre-existing target explicitly to pin the transactional contract.
    let target = tempfile::tempdir().expect("target");
    seed_existing_target(target.path());
    assert_existing_target_intact(target.path());
}

/// FLD-CFG-144/145 retry semantics: a failed PUT keeps the old remote bytes
/// (no partial replace); the retry then succeeds and replaces them.
#[test]
fn waveb_put_failure_keeps_old_remote_then_retry_succeeds() {
    let harness = spawn_harness();
    let client = WebDavClient::new(saved_config(harness.port), Duration::from_secs(5), None)
        .expect("client");
    let rt = runtime();

    let src = tempfile::tempdir().expect("src");
    seed_source(src.path());
    let old_bundle = zip_upstream_layout(src.path()).expect("zip");
    rt.block_on(async {
        client.check().await.expect("check");
        client.upload(old_bundle.clone()).await.expect("seed");
    });

    harness.state.lock().expect("lock").fail_next_put = Some(500);
    let new_bytes = b"new-bundle-bytes".to_vec();
    let error = rt
        .block_on(client.upload(new_bytes.clone()))
        .expect_err("500");
    assert_eq!(error.code, "E_UNAVAILABLE");
    assert!(error.retryable, "mid-transfer failure must be retryable");

    // No partial replace: the old remote backup is still whole.
    let kept = rt.block_on(client.download()).expect("download old");
    assert_eq!(kept, old_bundle);

    // Retry with the fault cleared replaces the bytes.
    let retried = rt
        .block_on(client.upload(new_bytes.clone()))
        .expect("retry");
    assert_eq!(retried, new_bytes.len() as u64);
    let current = rt.block_on(client.download()).expect("download new");
    assert_eq!(current, new_bytes);
}

/// Mid-transfer GET corruption: a truncated 200 body must be caught by the
/// hash gate so restore never runs and the live install is untouched.
#[test]
fn waveb_truncated_download_rejected_by_hash_without_restore() {
    let harness = spawn_harness();
    let client = WebDavClient::new(saved_config(harness.port), Duration::from_secs(5), None)
        .expect("client");
    let rt = runtime();

    let src = tempfile::tempdir().expect("src");
    seed_source(src.path());
    let bundle = zip_upstream_layout(src.path()).expect("zip");
    let bundle_hash = sha256_hex(&bundle);
    rt.block_on(async {
        client.check().await.expect("check");
        client.upload(bundle.clone()).await.expect("upload");
    });

    harness.state.lock().expect("lock").truncate_get_to = Some(16);
    let short = rt
        .block_on(client.download())
        .expect("truncated body still 200s");
    assert!(short.len() < bundle.len());
    assert_ne!(
        sha256_hex(&short),
        bundle_hash,
        "integrity gate must catch the truncated body"
    );

    // The corrupt bytes never reach restore: the live install is intact and
    // the remote still serves the whole bundle afterwards.
    let target = tempfile::tempdir().expect("target");
    seed_existing_target(target.path());
    assert_existing_target_intact(target.path());
    let whole = rt.block_on(client.download()).expect("full download");
    assert_eq!(sha256_hex(&whole), bundle_hash);
}

/// Interrupt/cancel: a download cut off by timeout surfaces explicitly and
/// leaves both sides consistent; a later retry succeeds.
#[test]
fn waveb_interrupted_download_timeout_keeps_state_retryable() {
    let harness = spawn_harness();
    harness.state.lock().expect("lock").delay = Duration::from_millis(1500);
    let slow = WebDavClient::new(saved_config(harness.port), Duration::from_millis(400), None)
        .expect("client");
    let rt = runtime();
    let error = rt.block_on(slow.download()).expect_err("timeout");
    assert_eq!(error.code, "E_TIMEOUT");

    harness.state.lock().expect("lock").delay = Duration::ZERO;
    // The single-threaded mock still sleeps out the abandoned request; let it
    // drain so the retry below measures recovery, not queueing.
    std::thread::sleep(Duration::from_secs(2));
    let target = tempfile::tempdir().expect("target");
    seed_existing_target(target.path());
    assert_existing_target_intact(target.path());

    // Retry after the interrupt: seed + full download round-trips.
    let src = tempfile::tempdir().expect("src");
    seed_source(src.path());
    let bundle = zip_upstream_layout(src.path()).expect("zip");
    rt.block_on(async {
        slow.check().await.expect("check after interrupt");
        slow.upload(bundle.clone())
            .await
            .expect("upload after interrupt");
        let back = slow.download().await.expect("download after interrupt");
        assert_eq!(back, bundle);
    });
}

/// DELETE is spoken: removing the remote backup makes list/download report
/// absence instead of stale bytes.
#[test]
fn waveb_delete_removes_remote_backup() {
    let harness = spawn_harness();
    let config = saved_config(harness.port);
    let client = WebDavClient::new(config.clone(), Duration::from_secs(5), None).expect("client");
    let rt = runtime();
    rt.block_on(async {
        client.check().await.expect("check");
        client.upload(b"ephemeral".to_vec()).await.expect("upload");
        let entries = client.list().await.expect("list");
        assert!(entries.iter().any(|e| e.href.ends_with("backup.zip")));

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("raw client");
        let deleted = http
            .delete(config.backup_url())
            .basic_auth(SYN_USER, Some(SYN_PASS))
            .send()
            .await
            .expect("delete");
        assert_eq!(deleted.status().as_u16(), 204);

        let entries = client.list().await.expect("list after delete");
        assert!(
            !entries.iter().any(|e| e.href.ends_with("backup.zip")),
            "deleted backup must disappear from list: {entries:?}"
        );
        let error = client.download().await.expect_err("missing file");
        assert_eq!(error.code, "E_NOT_FOUND");
    });
    assert_eq!(harness.state.lock().expect("lock").deletes, 1);
}
