//! T21 external self-update integration test.
//!
//! Drives the real pipeline against a loopback GitHub mock:
//!   old install (A) -> release JSON (new 1.0.1) -> download + verify ->
//!   InstallPlan -> spawn `upgrade_runner` (waits for a fake "app" PID, swaps
//!   atomically, restarts) -> assert new files, rollback copy and result JSON.
//!
//! A second case serves an asset whose SHA-256 does not match the metadata and
//! proves the replacement is aborted with the old install left byte-for-byte
//! intact.
//!
//! Ports are OS-assigned (ephemeral, never 10808/11808) and every spawned
//! process is a real, short-lived child killed only by PID.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use application::{CancellationToken, CoreApplyRequest, UpdateService};
use sha2::{Digest, Sha256};
use tiny_http::{Header, Response, Server};
use updater::arch::{HostTarget, Os, PlatformArch};

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        writer.start_file(name.to_string(), options).expect("start");
        writer.write_all(bytes).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

/// The `upgrade_runner` binary under test (`cargo test` builds it first).
fn runner_exe() -> PathBuf {
    let mut path = std::env::current_exe().expect("current exe");
    path.pop(); // test binary name
    path.pop(); // deps/
    path.push(if cfg!(windows) {
        "upgrade_runner.exe"
    } else {
        "upgrade_runner"
    });
    assert!(
        path.is_file(),
        "upgrade_runner binary not built at {}",
        path.display()
    );
    path
}

fn installed_dir(root: &Path) -> PathBuf {
    root.join("v2rayN-R")
}

fn write_old_install(root: &Path) {
    let dir = installed_dir(root);
    std::fs::create_dir_all(&dir).expect("old dir");
    std::fs::write(dir.join("v2rayn_desktop.exe"), b"OLD-APP-BINARY").expect("old exe");
    std::fs::write(dir.join("version.txt"), b"1.0.0").expect("old version");
    std::fs::write(dir.join("bridge_api.dll"), b"OLD-DLL").expect("old dll");
}

/// Release metadata + asset bytes for a "new" package. When `corrupt_asset` is
/// set the served bytes differ from the declared digest.
struct Mock {
    port: u16,
}

fn build_mock(corrupt_asset: bool) -> Mock {
    // A "new package" that still looks like an x64 PE so arch verification runs.
    let new_zip = make_zip(&[
        ("v2rayn_desktop.exe", b"NEW-APP-BINARY".as_slice()),
        ("version.txt", b"1.0.1".as_slice()),
        ("NEW-VERSION.txt", b"present".as_slice()),
        ("bridge_api.dll", b"NEW-DLL".as_slice()),
    ]);
    let declared_sha = if corrupt_asset {
        // Declare the digest of a *valid but different* zip so the size/format
        // pass and only the SHA-256 check can fail.
        sha256_hex(&make_zip(&[("whatever.txt", b"no".as_slice())]))
    } else {
        sha256_hex(&new_zip)
    };

    let server = Server::http("127.0.0.1:0").expect("bind mock");
    let port = server.server_addr().to_ip().expect("ip addr").port();
    let base = format!("http://127.0.0.1:{port}");
    let releases = format!(
        r#"[{{"tag_name":"v1.0.1","name":"v2rayN-R 1.0.1","prerelease":false,
            "assets":[
              {{"name":"v2rayN-windows-64.zip","size":{},"browser_download_url":"{base}/assets/new.zip","digest":"sha256:{declared_sha}"}}
            ]}}]"#,
        new_zip.len()
    );
    let served = if corrupt_asset {
        // A valid zip whose bytes simply do not match the declared digest.
        make_zip(&[("decoy.txt", b"decoy".as_slice())])
    } else {
        new_zip.clone()
    };
    let mut assets = HashMap::new();
    assets.insert("/assets/new.zip".to_string(), served);
    let releases_owned = releases.clone();
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            if url == "/repos/2dust/v2rayN/releases" {
                let _ = request.respond(
                    Response::from_string(releases_owned.clone())
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

fn service(root: &Path, port: u16) -> UpdateService {
    let mut svc = UpdateService::new(root.join("cores"));
    svc.api_base = format!("http://127.0.0.1:{port}/repos");
    svc.target = HostTarget::new(Os::Windows, PlatformArch::X64);
    svc.timeout = Duration::from_secs(10);
    svc
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

/// Spawn a trivial process that stays alive so the runner has something to wait
/// for, then kill it once the runner is going. Returns its PID.
fn spawn_short_lived(secs: u64) -> Child {
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", &format!("timeout /T {secs} /NOBREAK >NUL")])
            .spawn()
            .expect("spawn timeout")
    }
    #[cfg(not(windows))]
    {
        Command::new("sleep")
            .arg(secs.to_string())
            .spawn()
            .expect("spawn sleep")
    }
}

fn wait_file(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn read_result(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("result json");
    serde_json::from_str(&text).expect("parse result json")
}

#[test]
fn self_update_replaces_and_backs_up_then_restarts() {
    let work = tempfile::tempdir().expect("work");
    // The app stages inside its install root; the runner plans within that root.
    let root = work.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    write_old_install(&root);
    let mock = build_mock(false);
    let svc = service(&root, mock.port);

    let plan_path = work.path().join("plan.json");
    let result_path = work.path().join("result.json");
    let restarted_marker = work.path().join("restarted.txt");
    // A real restart target: writes a marker then exits, so the test can prove
    // the runner restarted *something* without launching the GUI.
    let restart_exe = write_restart_stub(work.path(), &restarted_marker);

    let rt = runtime();
    rt.block_on(async {
        let check = svc.check_core("v2rayN", false, None).await.expect("check");
        assert!(check.has_update);
        assert_eq!(check.remote_version.as_deref(), Some("1.0.1"));
        let token = CancellationToken::new();
        let spec = svc
            .app_update_spec(&request_from(&check), restart_exe.clone(), 0, &token)
            .await
            .expect("spec");

        let plan = updater::install::InstallPlan::new(
            &root,
            installed_dir(&root),
            spec.source.clone(),
            "v2rayN-R.previous",
            "1.0.1",
        );
        std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).expect("write plan");
    });

    // Start a stand-in for the running app, then let the runner wait for it.
    let mut app = spawn_short_lived(3);
    let child = Command::new(runner_exe())
        .arg("--plan")
        .arg(&plan_path)
        .arg("--result")
        .arg(&result_path)
        .arg("--pid")
        .arg(app.id().to_string())
        .arg("--wait-timeout-secs")
        .arg("20")
        .arg("--restart-exe")
        .arg(&restart_exe)
        .arg("--restart-cwd")
        .arg(installed_dir(&root))
        .spawn()
        .expect("spawn runner");
    // The runner must not touch files while the app is still up.
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        installed_dir(&root).join("version.txt").is_file(),
        "runner acted before the target exited"
    );

    // Let the fake app die naturally; the runner then proceeds.
    let _ = app.wait();
    assert!(
        wait_file(&result_path, Duration::from_secs(60)),
        "runner result not written"
    );
    let status = child.wait_with_output().expect("runner output");
    let result = read_result(&result_path);

    assert!(result["ok"].as_bool().unwrap_or(false), "result: {result}");
    assert_eq!(result["stage"], "done");
    assert_eq!(result["version"], "1.0.1");

    let installed = installed_dir(&root);
    assert!(
        installed.join("NEW-VERSION.txt").is_file(),
        "new file present"
    );
    assert_eq!(
        std::fs::read_to_string(installed.join("version.txt")).unwrap(),
        "1.0.1"
    );
    let backup = root.join("v2rayN-R.previous");
    assert!(backup.join("version.txt").is_file(), "backup present");
    assert_eq!(
        std::fs::read_to_string(backup.join("version.txt")).unwrap(),
        "1.0.0"
    );
    assert!(backup.join("v2rayn_desktop.exe").is_file());
    assert!(installed.join("install-manifest.json").is_file());
    assert!(result["restarted"].as_bool().unwrap_or(false));
    assert!(
        wait_file(&restarted_marker, Duration::from_secs(20)),
        "restart marker"
    );
    assert!(status.status.success() || status.status.code() == Some(0));
}

#[test]
fn digest_mismatch_aborts_without_touching_install() {
    let work = tempfile::tempdir().expect("work");
    let root = work.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    write_old_install(&root);
    let installed = installed_dir(&root);
    let before: HashMap<String, Vec<u8>> = ["v2rayn_desktop.exe", "version.txt", "bridge_api.dll"]
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                std::fs::read(installed.join(name)).unwrap(),
            )
        })
        .collect();

    let mock = build_mock(true);
    let svc = service(work.path(), mock.port);
    let rt = runtime();
    rt.block_on(async {
        let check = svc.check_core("v2rayN", false, None).await.expect("check");
        // The metadata declares sha256:tampered-bytes; the served bytes hash
        // differently, so verification must fail before any plan is produced.
        let token = CancellationToken::new();
        let error = svc
            .app_update_spec(&request_from(&check), runner_exe(), 0, &token)
            .await
            .expect_err("digest must fail");
        assert_eq!(error.code, "E_CONFLICT", "unexpected error: {error:?}");
    });

    // The old install is byte-for-byte intact; no staging leaked into it.
    for (name, bytes) in &before {
        assert_eq!(
            &std::fs::read(installed.join(name)).unwrap(),
            bytes,
            "{name} changed"
        );
    }
    assert!(!installed.join("NEW-VERSION.txt").exists());
    assert!(!root.join("v2rayN-R.previous").exists());
}

/// With `--fail-inject commit`, the swap's final rename fails after the old
/// version was set aside. `apply_atomic` already rolls that rename back, so the
/// runner reports `ok=false` and the old install is left fully usable.
#[test]
fn commit_rename_failure_leaves_install_usable() {
    let work = tempfile::tempdir().expect("work");
    let root = work.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    write_old_install(&root);
    let installed = installed_dir(&root);

    // A valid staged directory inside the managed root.
    let staged = root.join(".staging").join("app-1.0.1").join("unpacked");
    std::fs::create_dir_all(&staged).expect("staged dir");
    std::fs::write(staged.join("version.txt"), b"1.0.1").expect("staged version");

    let plan_path = work.path().join("plan.json");
    let result_path = work.path().join("result.json");
    let plan = updater::install::InstallPlan::new(
        &root,
        installed_dir(&root),
        staged,
        "v2rayN-R.previous",
        "1.0.1",
    );
    std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan).unwrap()).expect("plan");

    let out = Command::new(runner_exe())
        .arg("--plan")
        .arg(&plan_path)
        .arg("--result")
        .arg(&result_path)
        .arg("--fail-inject")
        .arg("commit")
        .arg("--no-restart")
        .output()
        .expect("run runner");
    assert!(!out.status.success(), "injected failure must exit non-zero");

    let result = read_result(&result_path);
    assert_eq!(result["ok"], false);
    assert_eq!(result["stage"], "apply");
    assert_eq!(result["restarted"], false);

    // The old version is back in place, byte-for-byte, and no keep dir remains.
    assert_eq!(
        std::fs::read_to_string(installed.join("version.txt")).unwrap(),
        "1.0.0"
    );
    assert!(installed.join("v2rayn_desktop.exe").is_file());
    assert!(!root.join("v2rayN-R.previous").exists());
    assert!(!installed.join("NEW-VERSION.txt").exists());
}

/// A helper that crashed between the two renames leaves the new version under
/// `previous` and the active dir missing; the runner's `restore_previous` must
/// put the old version back.
#[test]
fn crash_between_renames_is_recovered() {
    let work = tempfile::tempdir().expect("work");
    let root = work.path().join("install");
    std::fs::create_dir_all(&root).expect("root");
    write_old_install(&root);
    let installed = installed_dir(&root);

    // Simulate the half-done swap: old version moved to keep_name, current gone,
    // and a staged dir still waiting.
    std::fs::rename(&installed, root.join("v2rayN-R.previous")).expect("stage rename");
    let staged = root.join("staged-1.0.1");
    std::fs::create_dir_all(&staged).expect("staged");
    std::fs::write(staged.join("version.txt"), b"1.0.1").expect("staged version");

    // restore_previous is the exact recovery the runner performs on failure.
    let plan = updater::install::InstallPlan::new(
        &root,
        installed_dir(&root),
        staged,
        "v2rayN-R.previous",
        "1.0.1",
    );
    let restored = updater::install::restore_previous(&plan).expect("restore");
    assert!(restored, "previous must be restored");
    assert_eq!(
        std::fs::read_to_string(installed.join("version.txt")).unwrap(),
        "1.0.0"
    );
    assert!(installed.join("v2rayn_desktop.exe").is_file());
}

/// A tiny exe stand-in for the restarted application: it is the `cmd` builtin
/// running `echo` into a marker file, wrapped so the runner can spawn it.
fn write_restart_stub(dir: &Path, marker: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let script = dir.join("restart_stub.cmd");
        std::fs::write(
            &script,
            format!("@echo restarted>\"{}\"\r\n", marker.display()),
        )
        .expect("stub script");
        script
    }
    #[cfg(not(windows))]
    {
        let script = dir.join("restart_stub.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\necho restarted > '{}'\n", marker.display()),
        )
        .expect("stub script");
        script
    }
}
