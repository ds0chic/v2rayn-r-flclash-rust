//! T16 update-pipeline integration tests against a loopback GitHub mock.
//!
//! The mock is reached directly; nothing is downloaded from the real GitHub and
//! every install/rollback happens inside a temporary cores root.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use application::{CoreApplyRequest, UpdateService};
use sha2::{Digest, Sha256};
use tiny_http::{Response, Server};
use updater::arch::{HostTarget, Os, PlatformArch};

/// The OS-assigned port (always in the ephemeral range, never 10808).
fn bound_port(server: &Server) -> u16 {
    server
        .server_addr()
        .to_ip()
        .expect("expected an IP listen address")
        .port()
}

fn make_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer.start_file(name.to_string(), options).expect("start");
        writer.write_all(bytes).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

/// Minimal PE with an x64 COFF machine field.
fn pe_x64() -> Vec<u8> {
    let mut bytes = vec![0u8; 0x80];
    bytes[0] = b'M';
    bytes[1] = b'Z';
    bytes[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes
}

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

struct Mock {
    port: u16,
}

fn build_mock() -> Mock {
    let xray_zip = make_zip(&[("xray.exe", pe_x64())]);
    let xray_zip_sha = sha256_hex(&xray_zip);
    let v2rayn_zip = make_zip(&[("v2rayN.exe", b"app".to_vec())]);

    // The port is needed inside the JSON URLs, so bind the server first.
    let server = Server::http("127.0.0.1:0").expect("bind mock");
    let port = bound_port(&server);
    let base = format!("http://127.0.0.1:{port}");
    let xray_json = format!(
        r#"[{{"tag_name":"v26.4.0","name":"Xray","prerelease":false,"assets":[
            {{"name":"Xray-windows-64.zip","size":{},"browser_download_url":"{base}/assets/xray.zip"}},
            {{"name":"Xray-windows-64.zip.dgst","size":80,"browser_download_url":"{base}/assets/xray.zip.dgst"}}
        ]}}]"#,
        xray_zip.len()
    );
    let v2rayn_json = format!(
        r#"[{{"tag_name":"v7.99.0","name":"v2rayN","prerelease":true,"assets":[
            {{"name":"v2rayN-windows-64.zip","size":{},"browser_download_url":"{base}/assets/v2rayn.zip"}}
        ]}}]"#,
        v2rayn_zip.len()
    );
    let mut assets = HashMap::new();
    assets.insert("/assets/xray.zip".to_string(), xray_zip);
    assets.insert(
        "/assets/xray.zip.dgst".to_string(),
        format!("SHA2-256= {xray_zip_sha}\n").into_bytes(),
    );
    assets.insert("/assets/v2rayn.zip".to_string(), v2rayn_zip);

    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            if url == "/repos/XTLS/Xray-core/releases" {
                let _ = request.respond(
                    Response::from_string(xray_json.clone())
                        .with_header(header("Content-Type", "application/json"))
                        .with_status_code(200),
                );
            } else if url == "/repos/2dust/v2rayN/releases" {
                let _ = request.respond(
                    Response::from_string(v2rayn_json.clone())
                        .with_header(header("Content-Type", "application/json"))
                        .with_status_code(200),
                );
            } else if let Some(bytes) = assets.get(&url) {
                let _ = request.respond(Response::from_data(bytes.clone()).with_status_code(200));
            } else {
                let _ = request.respond(Response::empty(404));
            }
        }
    });
    Mock { port }
}

fn service(cores_root: &Path, port: u16) -> UpdateService {
    let mut service = UpdateService::new(cores_root);
    service.api_base = format!("http://127.0.0.1:{port}/repos");
    service.target = HostTarget::new(Os::Windows, PlatformArch::X64);
    service.timeout = Duration::from_secs(5);
    service
}

fn request_from(check: &application::CoreUpdateCheck) -> CoreApplyRequest {
    CoreApplyRequest {
        core: check.core.clone(),
        version: check.remote_version.clone().expect("version"),
        asset_name: check.asset_name.clone().expect("asset"),
        download_url: check.download_url.clone().expect("url"),
        expected_sha256: check.expected_sha256.clone(),
        dgst_url: check.dgst_url.clone(),
        proxy: None,
    }
}

#[test]
fn check_apply_and_rollback_in_temp_dir() {
    let mock = build_mock();
    let cores = tempfile::tempdir().expect("cores");
    // Seed an older installed version so there is something to roll back to.
    let old_dir = cores.path().join("xray").join("v26.3.27");
    std::fs::create_dir_all(&old_dir).expect("old dir");
    let old_bytes = pe_x64();
    std::fs::write(old_dir.join("xray.exe"), &old_bytes).expect("old exe");

    let service = service(cores.path(), mock.port);
    let rt = runtime();
    rt.block_on(async {
        let check = service
            .check_core("xray", false, None)
            .await
            .expect("check");
        assert!(
            check.has_update,
            "remote {}",
            check.remote_version.clone().unwrap()
        );
        assert_eq!(check.installed_version.as_deref(), Some("26.3.27"));
        assert_eq!(check.remote_version.as_deref(), Some("26.4.0"));
        assert!(check.dgst_url.is_some());

        let token = application::CancellationToken::new();
        let outcome = service
            .apply_core(&request_from(&check), &token)
            .await
            .expect("apply");
        assert_eq!(outcome.version, "26.4.0");
        assert!(outcome.kept_previous.is_some());
        // The freshly installed version lives in its own version directory and
        // the runtime locator resolves exactly that directory.
        assert!(cores
            .path()
            .join("xray")
            .join("26.4.0")
            .join("xray.exe")
            .is_file());
        let locator = runtime::CoreLocator::with_roots(vec![cores.path().to_path_buf()], None);
        let resolved = locator
            .resolve(domain::CoreType::Xray, None)
            .expect("runtime resolves the just-installed version");
        assert!(
            resolved.ends_with(std::path::Path::new("26.4.0").join("xray.exe")),
            "{resolved:?}"
        );
        assert!(cores.path().join("xray.previous").is_dir());

        service.rollback_core("xray").expect("rollback");
        let restored = std::fs::read(cores.path().join("xray").join("v26.3.27").join("xray.exe"))
            .expect("read");
        assert_eq!(restored, old_bytes);
    });
}

#[test]
fn digest_mismatch_leaves_no_install() {
    let mock = build_mock();
    let cores = tempfile::tempdir().expect("cores");
    let service = service(cores.path(), mock.port);
    let rt = runtime();
    rt.block_on(async {
        let check = service
            .check_core("xray", false, None)
            .await
            .expect("check");
        let mut request = request_from(&check);
        request.expected_sha256 = Some("0".repeat(64));
        request.dgst_url = None;
        let token = application::CancellationToken::new();
        let error = service
            .apply_core(&request, &token)
            .await
            .expect_err("digest");
        assert_eq!(error.code, "E_CONFLICT");
        assert!(!cores.path().join("xray").exists());
    });
}

#[test]
fn app_update_spec_never_spawns_helper() {
    let mock = build_mock();
    let cores = tempfile::tempdir().expect("cores");
    let service = service(cores.path(), mock.port);
    let helper = cores.path().join("v2rayN-upgrade.exe");
    let rt = runtime();
    rt.block_on(async {
        let check = service
            .check_core("v2rayN", true, None)
            .await
            .expect("check");
        assert_eq!(check.remote_version.as_deref(), Some("7.99.0"));
        let token = application::CancellationToken::new();
        let spec = service
            .app_update_spec(&request_from(&check), &helper, 4242, &token)
            .await
            .expect("spec");
        assert_eq!(spec.wait_for_pid, 4242);
        assert!(spec.source.is_dir());
        assert_eq!(spec.helper_exe, helper);
        assert!(!helper.exists(), "helper must not be executed/created");
    });
}
