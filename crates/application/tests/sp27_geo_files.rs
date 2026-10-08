//! SP28-L1-001: the update flow's `GeoFiles` row (synthetic loopback only).
//!
//! Upstream (`UpdateService.GetGeoFilesRequest` + `CheckUpdateViewModel`) lists
//! a trailing `GeoFiles` row that refreshes `geoip.dat`/`geosite.dat` from the
//! settings `GeoSourceUrl` template (upstream `Global.GeoUrl` fallback). Geo
//! files carry no release version: the check row never reports one, a failed
//! pass leaves the previous files intact, and an unreachable source is an
//! honest error. Every port is pre-probed free and `>= 11808` (never 10808);
//! no real external network; no OS side effects.

use std::time::Duration;

use application::{UpdateService, GEO_FILES_TARGET};
use sha2::{Digest, Sha256};
use tiny_http::{Response, Server};

const GEOIP_BODY: &[u8] = b"sp27-synthetic-geoip-v1";
const GEOSITE_BODY: &[u8] = b"sp27-synthetic-geosite-v1";

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

/// Bind a loopback mock on a pre-probed free port `>= 11808` (never 10808).
/// Serves the synthetic `.dat` bytes; every other path is a 404.
fn bind_mock() -> (u16, String) {
    for port in 11808..11960u16 {
        if port == 10808 {
            continue;
        }
        let Ok(server) = Server::http(format!("127.0.0.1:{port}")) else {
            continue;
        };
        spawn_handler(server);
        let base = format!("http://127.0.0.1:{port}");
        return (port, format!("{base}/{{0}}.dat"));
    }
    panic!("no free loopback port in 11808..11960");
}

fn spawn_handler(server: Server) {
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let body: Option<&[u8]> = if url == "/geoip.dat" {
                Some(GEOIP_BODY)
            } else if url == "/geosite.dat" {
                Some(GEOSITE_BODY)
            } else {
                None
            };
            match body {
                Some(bytes) => {
                    let _ = request.respond(
                        Response::from_data(bytes.to_vec())
                            .with_header(header("Content-Type", "application/octet-stream"))
                            .with_status_code(200),
                    );
                }
                None => {
                    let _ = request.respond(Response::empty(404));
                }
            }
        }
    });
}

/// A surely-closed loopback port (bound then released, never 10808).
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().unwrap().port();
    assert_ne!(
        port, 10808,
        "ephemeral port must never be the live proxy port"
    );
    drop(listener);
    port
}

fn service_with_template(cores_root: &std::path::Path, template: &str) -> UpdateService {
    let mut service = UpdateService::new(cores_root);
    service.timeout = Duration::from_secs(5);
    service.with_geo_source(template.to_string())
}

/// Isolated `<data>/cores` root so the derived `<data>/bin` destination (and
/// `<cores>/.staging`) never escapes the temporary directory.
fn isolated_cores_root(data: &tempfile::TempDir) -> std::path::PathBuf {
    data.path().join("cores")
}

fn no_staging_residue(cores_root: &std::path::Path) {
    assert!(
        !cores_root.join(".staging").join("geo-pending").exists(),
        "staging residue under {}",
        cores_root.display()
    );
}

#[test]
fn check_reports_geo_row_without_network_or_version() {
    // The default service (upstream built-in template) reports the row with no
    // I/O at all; the row never fabricates a remote version.
    let data = tempfile::tempdir().expect("data");
    let service = UpdateService::new(isolated_cores_root(&data));
    let check = runtime()
        .block_on(service.check_core(GEO_FILES_TARGET, false, None))
        .expect("geo check");
    assert_eq!(check.core, GEO_FILES_TARGET);
    assert!(check.supported);
    assert!(check.note.is_none());
    assert!(check.has_update);
    assert!(check.remote_version.is_none(), "never fabricate a version");
    assert!(check.installed_version.is_none());
    assert!(check.asset_name.is_none());
    assert!(check.download_url.is_none());
}

#[test]
fn apply_downloads_both_files_with_hash_recording() {
    let (port, template) = bind_mock();
    assert_ne!(port, 10808);
    let data = tempfile::tempdir().expect("data");
    let cores_root = isolated_cores_root(&data);
    let service = service_with_template(&cores_root, &template);
    let outcome = runtime()
        .block_on(service.apply_geo_files(None, &application::CancellationToken::new()))
        .expect("apply");
    assert_eq!(outcome.target, GEO_FILES_TARGET);
    assert_eq!(outcome.bin_dir, data.path().join("bin"));
    assert_eq!(outcome.files.len(), 2);
    assert_eq!(outcome.files[0].name, "geoip.dat");
    assert_eq!(outcome.files[0].sha256, sha256_hex(GEOIP_BODY));
    assert_eq!(outcome.files[1].name, "geosite.dat");
    assert_eq!(outcome.files[1].sha256, sha256_hex(GEOSITE_BODY));

    let bin = data.path().join("bin");
    assert_eq!(
        std::fs::read(bin.join("geoip.dat")).expect("geoip"),
        GEOIP_BODY
    );
    assert_eq!(
        std::fs::read(bin.join("geosite.dat")).expect("geosite"),
        GEOSITE_BODY
    );
    // The recorded hashes match what the runtime plan derives from disk.
    let plan_hashes = application::dns::geo_asset_hashes(&bin);
    assert_eq!(plan_hashes.len(), 2);
    assert_eq!(plan_hashes[0].0, sha256_hex(GEOIP_BODY));
    assert_eq!(plan_hashes[1].0, sha256_hex(GEOSITE_BODY));
    // No staging residue leaks into the managed directories.
    no_staging_residue(&cores_root);
}

#[test]
fn failure_leaves_old_files_intact() {
    let (_port, base_template) = bind_mock();
    // Point at paths the mock answers 404 to: the whole pass must fail.
    let template = base_template.replace("{0}", "missing-{0}");
    let data = tempfile::tempdir().expect("data");
    let cores_root = isolated_cores_root(&data);
    let bin = data.path().join("bin");
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::write(bin.join("geoip.dat"), b"OLD-GEOIP").expect("old geoip");
    std::fs::write(bin.join("geosite.dat"), b"OLD-GEOSITE").expect("old geosite");

    let service = service_with_template(&cores_root, &template);
    let error = runtime()
        .block_on(service.apply_geo_files(None, &application::CancellationToken::new()))
        .expect_err("404 pass must fail");
    assert_eq!(error.code, "E_UNAVAILABLE");
    assert_eq!(
        std::fs::read(bin.join("geoip.dat")).expect("geoip"),
        b"OLD-GEOIP"
    );
    assert_eq!(
        std::fs::read(bin.join("geosite.dat")).expect("geosite"),
        b"OLD-GEOSITE"
    );
    no_staging_residue(&cores_root);
}

#[test]
fn offline_source_is_an_honest_error_with_no_fabricated_version() {
    let port = closed_port();
    let template = format!("http://127.0.0.1:{port}/{{0}}.dat");
    let data = tempfile::tempdir().expect("data");
    let cores_root = isolated_cores_root(&data);
    let service = service_with_template(&cores_root, &template);

    // The row still reports no remote version when offline.
    let check = runtime()
        .block_on(service.check_core(GEO_FILES_TARGET, false, None))
        .expect("geo check");
    assert!(check.remote_version.is_none());
    assert!(check.supported);

    let error = runtime()
        .block_on(service.apply_geo_files(None, &application::CancellationToken::new()))
        .expect_err("offline apply must fail");
    assert_eq!(error.code, "E_UNAVAILABLE");
    assert!(!data.path().join("bin").join("geoip.dat").exists());
    assert!(!data.path().join("bin").join("geosite.dat").exists());
    no_staging_residue(&cores_root);
}

#[test]
fn landed_files_survive_a_service_reopen() {
    let (_port, template) = bind_mock();
    let data = tempfile::tempdir().expect("data");
    let cores_root = isolated_cores_root(&data);
    let service = service_with_template(&cores_root, &template);
    let outcome = runtime()
        .block_on(service.apply_geo_files(None, &application::CancellationToken::new()))
        .expect("apply");
    drop(service);

    // Reopen against the same root: the row is still reported and the landed
    // files (with identical hashes) are what the plan will read.
    let reopened = service_with_template(&cores_root, &template);
    let check = runtime()
        .block_on(reopened.check_core(GEO_FILES_TARGET, false, None))
        .expect("geo check after reopen");
    assert_eq!(check.core, GEO_FILES_TARGET);
    assert!(check.supported);
    assert!(check.remote_version.is_none());
    let bin = data.path().join("bin");
    assert_eq!(
        std::fs::read(bin.join("geoip.dat")).expect("geoip"),
        GEOIP_BODY
    );
    let plan_hashes = application::dns::geo_asset_hashes(&bin);
    assert_eq!(plan_hashes.len(), 2);
    assert_eq!(plan_hashes[0].0, outcome.files[0].sha256);
    assert_eq!(plan_hashes[1].0, outcome.files[1].sha256);
}
