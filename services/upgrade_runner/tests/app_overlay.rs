//! RR-04: drive the real `upgrade_runner` binary over a synthetic flat
//! application install. Verifies stage -> overlay -> rollback and that the
//! restart step is exercised without relaunching the host application.
//!
//! No host proxy / registry / port is touched; everything lives under a
//! `tempfile` directory and no real GUI process is started.

use std::path::{Path, PathBuf};
use std::process::Command;

use updater::install::InstallPlan;

fn runner() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_upgrade_runner"))
}

fn write_plan(dir: &Path, plan: &InstallPlan) -> PathBuf {
    let path = dir.join("plan.json");
    std::fs::write(&path, serde_json::to_vec_pretty(plan).unwrap()).unwrap();
    path
}

fn read_result(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn seed(tmp: &Path) -> (PathBuf, PathBuf) {
    let root = tmp.join("install");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("v2rayn_desktop.exe"), b"old-exe").unwrap();
    std::fs::write(root.join("user.txt"), b"keep").unwrap();
    let staged = root.join(".staging").join("app-9.9.9");
    std::fs::create_dir_all(&staged).unwrap();
    std::fs::write(staged.join("v2rayn_desktop.exe"), b"new-exe").unwrap();
    std::fs::write(staged.join("net_host.exe"), b"new-net").unwrap();
    (root, staged)
}

#[test]
fn flat_overlay_runner_applies_keeps_previous_and_user_files() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, staged) = seed(tmp.path());
    let plan = InstallPlan::flat_overlay(&root, &staged, "app.previous", "9.9.9");
    let plan_path = write_plan(tmp.path(), &plan);
    let result_path = tmp.path().join("result.json");

    let status = Command::new(runner())
        .args([
            "--plan",
            plan_path.to_str().unwrap(),
            "--result",
            result_path.to_str().unwrap(),
            "--no-restart",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    assert_eq!(
        std::fs::read(root.join("v2rayn_desktop.exe")).unwrap(),
        b"new-exe"
    );
    assert!(root.join("net_host.exe").is_file());
    assert_eq!(std::fs::read(root.join("user.txt")).unwrap(), b"keep");
    assert_eq!(
        std::fs::read(root.join("app.previous").join("v2rayn_desktop.exe")).unwrap(),
        b"old-exe"
    );
    let result = read_result(&result_path);
    assert_eq!(result["ok"], true);
    assert_eq!(result["version"], "9.9.9");
    assert_eq!(result["stage"], "done");
}

#[test]
fn flat_overlay_runner_rolls_back_on_injected_commit_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, staged) = seed(tmp.path());
    let plan = InstallPlan::flat_overlay(&root, &staged, "app.previous", "9.9.9");
    let plan_path = write_plan(tmp.path(), &plan);
    let result_path = tmp.path().join("result.json");

    let status = Command::new(runner())
        .args([
            "--plan",
            plan_path.to_str().unwrap(),
            "--result",
            result_path.to_str().unwrap(),
            "--fail-inject",
            "commit",
            "--no-restart",
        ])
        .status()
        .unwrap();
    assert!(!status.success());

    // The injected failure must leave the original install intact.
    assert_eq!(
        std::fs::read(root.join("v2rayn_desktop.exe")).unwrap(),
        b"old-exe"
    );
    assert!(!root.join("net_host.exe").exists());
    let result = read_result(&result_path);
    assert_eq!(result["ok"], false);
    assert_eq!(result["stage"], "apply");
}

#[test]
fn runner_reports_bad_restart_target_without_starting_the_host() {
    let tmp = tempfile::tempdir().unwrap();
    let (root, staged) = seed(tmp.path());
    let plan = InstallPlan::flat_overlay(&root, &staged, "app.previous", "9.9.9");
    let plan_path = write_plan(tmp.path(), &plan);
    let result_path = tmp.path().join("result.json");
    let missing = root.join("does-not-exist.exe");

    let status = Command::new(runner())
        .args([
            "--plan",
            plan_path.to_str().unwrap(),
            "--result",
            result_path.to_str().unwrap(),
            "--restart-exe",
            missing.to_str().unwrap(),
            "--restart-cwd",
            root.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!status.success());

    let result = read_result(&result_path);
    assert_eq!(result["ok"], false);
    assert_eq!(result["stage"], "restart");
    assert!(
        result["error"]
            .as_str()
            .unwrap_or_default()
            .contains("restart"),
        "error should name the restart step: {result}"
    );
}
