//! SP-27 application-package verify -> stage -> install -> rollback.
//!
//! The application update is staged through `app_update_spec_verified` against
//! a loopback release mock (synthetic data only, never the real internet) with
//! a synthetic detached-signature trust root (`PrefixHashVerifier`). The mock
//! binds an explicitly probed loopback port which must satisfy the project rule
//! (>= 11808, never 10808); every install/rollback stays inside temp dirs.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use application::{CoreApplyRequest, UpdateService};
use sha2::{Digest, Sha256};
use tiny_http::{Response, Server};
use updater::arch::{HostTarget, Os, PlatformArch};

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer
            .start_file((*name).to_string(), options)
            .expect("start");
        writer.write_all(bytes).expect("write");
    }
    writer.finish().expect("finish").into_inner()
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

/// Release mock serving the app asset plus its detached `.sig` marker.
fn build_mock(asset: Vec<u8>, sig: Vec<u8>) -> Mock {
    let (server, port) = {
        let mut bound = None;
        for offset in 0..500u16 {
            let candidate = 11980 + offset;
            if candidate == 10808 {
                continue;
            }
            if let Ok(server) = Server::http(("127.0.0.1", candidate)) {
                bound = Some((server, candidate));
                break;
            }
        }
        bound.expect("no free test port >= 11808")
    };
    assert!(
        port >= 11808 && port != 10808,
        "stub port {port} violates the >=11808 / never-10808 rule"
    );
    let base = format!("http://127.0.0.1:{port}");
    let releases = format!(
        r#"[{{"tag_name":"v7.99.0","name":"v2rayN-R","prerelease":false,"assets":[
            {{"name":"v2rayN-windows-64.zip","size":{},"browser_download_url":"{base}/assets/app.zip"}},
            {{"name":"v2rayN-windows-64.zip.sig","size":{},"browser_download_url":"{base}/assets/app.zip.sig"}}
        ]}}]"#,
        asset.len(),
        sig.len()
    );
    let mut assets = HashMap::new();
    assets.insert("/assets/app.zip".to_string(), asset);
    assets.insert("/assets/app.zip.sig".to_string(), sig);
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            if url == "/repos/synthetic/app/releases" {
                let _ = request.respond(
                    Response::from_string(releases.clone())
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

fn service(app_root: &Path, port: u16) -> UpdateService {
    let mut service = UpdateService::new(app_root.join("cores"));
    service.api_base = format!("http://127.0.0.1:{port}/repos");
    service.target = HostTarget::new(Os::Windows, PlatformArch::X64);
    service.timeout = Duration::from_secs(5);
    service.install_root = app_root.to_path_buf();
    service.app_repo = Some("synthetic/app".to_string());
    service.app_exe_name = "v2rayn_desktop.exe".to_string();
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

fn asset_and_marker() -> (Vec<u8>, Vec<u8>) {
    let asset = make_zip(&[
        ("v2rayn_desktop.exe", b"NEW-APP".as_slice()),
        ("data/seed.txt", b"seed".as_slice()),
    ]);
    let marker = sha256_hex(&asset)[..16].to_string().into_bytes();
    (asset, marker)
}

#[test]
fn verified_stage_install_rollback_roundtrip() {
    let (asset, marker) = asset_and_marker();
    let mock = build_mock(asset, marker.clone());
    let app_root = tempfile::tempdir().expect("app root");
    let service = service(app_root.path(), mock.port);
    let layout = service.app_layout();
    std::fs::write(layout.app_exe(), b"old-app").expect("old app");
    std::fs::write(layout.runner_exe(), b"stub").expect("runner stub");
    let verifier = updater::signature::PrefixHashVerifier {
        expected_prefix: String::from_utf8(marker).expect("marker"),
    };

    let rt = runtime();
    rt.block_on(async {
        let check = service.check_app_update(false, None).await.expect("check");
        assert!(check.sig_url.is_some(), "mock must publish a .sig asset");
        let token = application::CancellationToken::new();
        let spec = service
            .app_update_spec_verified(
                &request_from(&check),
                check.sig_url.as_deref(),
                &verifier,
                layout.runner_exe(),
                4242,
                &token,
            )
            .await
            .expect("verified spec");
        assert_eq!(spec.wait_for_pid, 4242);
        assert!(spec.source.is_dir());
        assert_eq!(
            std::fs::read(layout.runner_exe()).expect("stub"),
            b"stub",
            "the helper is never executed or replaced during staging"
        );

        let version = check.remote_version.clone().expect("version");
        let outcome = service.apply_app_upgrade(&spec, &version).expect("apply");
        assert_eq!(
            std::fs::read(layout.app_exe()).expect("new app"),
            b"NEW-APP"
        );
        assert_eq!(outcome.restart.program, layout.app_exe());
        assert_eq!(outcome.restart.working_dir, app_root.path());

        let restored = service.rollback_app_upgrade().expect("rollback");
        assert_eq!(restored, app_root.path());
        assert_eq!(
            std::fs::read(layout.app_exe()).expect("restored"),
            b"old-app"
        );
    });
}

#[test]
fn wrong_signature_is_rejected_before_install() {
    let (asset, _) = asset_and_marker();
    let mock = build_mock(asset, b"deadbeefdeadbeef".to_vec());
    let app_root = tempfile::tempdir().expect("app root");
    let service = service(app_root.path(), mock.port);
    let layout = service.app_layout();
    // A verifier pinned to a different marker: the served signature cannot
    // match, so staging must fail closed as `SignatureInvalid`.
    let verifier = updater::signature::PrefixHashVerifier {
        expected_prefix: "00".repeat(8),
    };

    let rt = runtime();
    rt.block_on(async {
        let check = service.check_app_update(false, None).await.expect("check");
        let token = application::CancellationToken::new();
        let error = service
            .app_update_spec_verified(
                &request_from(&check),
                check.sig_url.as_deref(),
                &verifier,
                layout.runner_exe(),
                4242,
                &token,
            )
            .await
            .expect_err("wrong signature must fail");
        assert_eq!(error.code, "E_PERMISSION_DENIED");
        assert!(!layout.app_exe().exists(), "nothing may be installed");
    });
}

#[test]
fn missing_signature_fails_closed_without_staging_a_spec() {
    let (asset, marker) = asset_and_marker();
    let mock = build_mock(asset, marker.clone());
    let app_root = tempfile::tempdir().expect("app root");
    let service = service(app_root.path(), mock.port);
    let layout = service.app_layout();
    let verifier = updater::signature::PrefixHashVerifier {
        expected_prefix: String::from_utf8(marker).expect("marker"),
    };

    let rt = runtime();
    rt.block_on(async {
        let check = service.check_app_update(false, None).await.expect("check");
        let token = application::CancellationToken::new();
        // No `.sig` asset published: verification is required, never skipped.
        let error = service
            .app_update_spec_verified(
                &request_from(&check),
                None,
                &verifier,
                layout.runner_exe(),
                4242,
                &token,
            )
            .await
            .expect_err("missing signature must fail");
        assert_eq!(error.code, "E_UNAVAILABLE");
        assert_eq!(error.message_key, "error.update_signature_missing");
    });
}
