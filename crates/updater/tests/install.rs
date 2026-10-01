//! Atomic replace, rollback and crash injection.
//!
//! All operations run in temp directories; no live installation is touched.

use std::fs;
use std::path::Path;

use updater::install::apply_atomic_inject;
use updater::{
    apply_atomic, verify_manifest, FailPoint, InstallManifest, InstallPlan, UpdateError,
    UpgradeCoordinator,
};

fn write(dir: &Path, name: &str, data: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, data).unwrap();
}

/// Build a root with an active `current/` and a `staged/` holding new content.
fn setup(root: &Path) {
    write(&root.join("current"), "xray.exe", "old-binary");
    write(&root.join("current"), "geoip.dat", "old-geo");
    write(&root.join("staged"), "xray.exe", "new-binary");
    write(&root.join("staged"), "geodata/geoip.dat", "new-geo");
}

#[test]
fn atomic_swap_keeps_previous_version() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let outcome = apply_atomic(&plan).unwrap();

    assert_eq!(outcome.version, "2.0.0");
    assert_eq!(outcome.kept_previous, Some(root.join("previous")));
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "new-binary"
    );
    assert_eq!(
        fs::read_to_string(root.join("previous/xray.exe")).unwrap(),
        "old-binary"
    );
    assert!(!root.join("staged").exists());
}

#[test]
fn atomic_swap_records_manifest_hashes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let outcome = apply_atomic(&plan).unwrap();

    assert_eq!(
        outcome.manifest.previous_version.as_deref(),
        Some("previous")
    );
    assert!(outcome
        .manifest
        .files
        .iter()
        .any(|f| f.relative_path == "xray.exe"));
    // Manifest verifies against the committed tree.
    verify_manifest(&root.join("current"), &outcome.manifest).unwrap();
}

#[test]
fn atomic_swap_without_previous_installs_cleanly() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("staged"), "sing-box", "fresh");
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "1.0.0",
    );
    let outcome = apply_atomic(&plan).unwrap();
    assert!(outcome.kept_previous.is_none());
    assert_eq!(
        fs::read_to_string(root.join("current/sing-box")).unwrap(),
        "fresh"
    );
}

#[test]
fn discard_previous_removes_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    let mut plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    plan.discard_previous = true;
    let outcome = apply_atomic(&plan).unwrap();
    assert!(outcome.kept_previous.is_none());
    assert!(!root.join("previous").exists());
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "new-binary"
    );
}

#[test]
fn missing_staged_dir_is_conflict_without_touching_current() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("current"), "xray.exe", "old-binary");
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let err = apply_atomic(&plan).unwrap_err();
    assert!(matches!(err, UpdateError::InstallConflict(_)));
    // Current is untouched and no keep dir appears.
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "old-binary"
    );
    assert!(!root.join("previous").exists());
}

#[test]
fn existing_keep_dir_is_rejected_before_swap() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    write(&root.join("previous"), "leftover", "stale");
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let err = apply_atomic(&plan).unwrap_err();
    assert!(matches!(err, UpdateError::InstallConflict(_)));
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "old-binary"
    );
}

/// A staged *file* (not a directory) is rejected during validation, so the
/// current install is never disturbed.
#[test]
fn staged_file_is_rejected_without_touching_current() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    // Create a file at current/ so the second rename (`staged` -> `current`)
    // fails because a directory cannot overwrite a file... but we first move
    // current away, so instead obstruct by making staged a *file*.
    fs::remove_dir_all(root.join("staged")).unwrap();
    fs::write(root.join("staged"), b"not-a-dir").unwrap();

    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    // A file passes validate()? `staged_dir.is_dir()` is false -> InstallConflict.
    let err = apply_atomic(&plan).unwrap_err();
    assert!(matches!(err, UpdateError::InstallConflict(_)));
    // Original still intact.
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "old-binary"
    );
    assert!(!root.join("previous").exists());
}

/// Crash injection: the *stage* rename (`current -> keep`) fails. The original
/// layout must be untouched and no backup should linger.
#[test]
fn stage_rename_failure_leaves_current_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let err = apply_atomic_inject(&plan, FailPoint::StageRename).unwrap_err();
    assert!(matches!(err, UpdateError::Io(_)));
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "old-binary"
    );
    assert!(!root.join("previous").exists());
    assert!(root.join("staged").exists());
}

/// Crash injection: the *commit* rename (`staged -> current`) fails after the
/// first rename succeeded. `apply_atomic` must roll `current` back so the
/// directory stays consistent and the old binary is active again.
#[test]
fn commit_rename_failure_rolls_back() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup(root);
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "2.0.0",
    );
    let err = apply_atomic_inject(&plan, FailPoint::CommitRename).unwrap_err();
    assert!(matches!(err, UpdateError::Io(_)));

    // Old version restored as the active `current`.
    assert_eq!(
        fs::read_to_string(root.join("current/xray.exe")).unwrap(),
        "old-binary"
    );
    assert_eq!(
        fs::read_to_string(root.join("current/geoip.dat")).unwrap(),
        "old-geo"
    );
    // No lingering backup and staged content still available for retry.
    assert!(!root.join("previous").exists());
    assert!(root.join("staged").exists());
}

/// Crash injection: the commit rename fails and there was no previous install.
/// Nothing must be half-committed.
#[test]
fn commit_rename_failure_without_previous_is_clean() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("staged"), "sing-box", "fresh");
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "previous",
        "1.0.0",
    );
    let err = apply_atomic_inject(&plan, FailPoint::CommitRename).unwrap_err();
    assert!(matches!(err, UpdateError::Io(_)));
    assert!(!root.join("current").exists());
    assert!(!root.join("previous").exists());
    assert!(root.join("staged").exists());
}

#[test]
fn planner_rejects_outside_root() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(root.join("staged")).unwrap();
    // A current dir that lives outside the managed root.
    let plan = InstallPlan::new(
        &root,
        tmp.path().join("elsewhere"),
        root.join("staged"),
        "prev",
        "1.0.0",
    );
    assert!(matches!(
        apply_atomic(&plan),
        Err(UpdateError::UnsafeTarget(_))
    ));
}

#[test]
fn planner_rejects_keep_name_with_separator() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("staged"), "a", "b");
    let plan = InstallPlan::new(
        root,
        root.join("current"),
        root.join("staged"),
        "../evil",
        "1.0.0",
    );
    assert!(matches!(
        apply_atomic(&plan),
        Err(UpdateError::UnsafeTarget(_))
    ));
}

#[test]
fn manifest_scan_detects_tampering() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(&root.join("v1"), "xray.exe", "content");
    let manifest = InstallManifest::scan_directory("1.0.0", &root.join("v1")).unwrap();
    verify_manifest(&root.join("v1"), &manifest).unwrap();

    fs::write(root.join("v1/xray.exe"), "tampered").unwrap();
    assert!(matches!(
        verify_manifest(&root.join("v1"), &manifest),
        Err(UpdateError::DigestMismatch { .. })
    ));
}

#[test]
fn external_upgrade_spec_is_returned_not_executed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let helper = root.join("AmazTool.exe");
    let source = root.join("stage/update.zip");
    let coordinator = UpgradeCoordinator::new(&helper, root);
    let spec = coordinator.external_upgrade_spec(&source, 4242).unwrap();
    assert_eq!(spec.wait_for_pid, 4242);
    assert_eq!(spec.helper_exe, helper);
    assert_eq!(spec.source, source);
    assert_eq!(spec.install_root, root);
    assert_eq!(spec.args, vec![source.to_string_lossy().into_owned()]);
    // Nothing was spawned: the helper still does not exist as a process.
    assert!(!helper.exists());
}
