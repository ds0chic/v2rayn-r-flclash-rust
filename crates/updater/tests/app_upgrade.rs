//! FIX-12B/RR-04: application self-update over a synthetic ZIP in an isolated
//! flat install root — stage -> overlay (keep `app.previous`) -> rollback ->
//! restart command. No process is spawned; the external runner is a stub file.

use std::io::Write;
use std::path::{Path, PathBuf};

use updater::{
    apply_app_upgrade, rollback_app_upgrade, safe_unpack_zip, AppInstallLayout, UnpackLimits,
};

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

fn stage_zip(root: &Path, version: &str, bytes: &[u8]) -> PathBuf {
    let zip_path = root.join(format!("{version}.zip"));
    std::fs::write(&zip_path, bytes).expect("zip");
    let dest = root
        .join(".staging")
        .join(format!("app-{version}"))
        .join("unpacked");
    safe_unpack_zip(&zip_path, &dest, UnpackLimits::default()).expect("unpack");
    dest
}

#[test]
fn synthetic_zip_stage_replace_rollback_restart() {
    let root = tempfile::tempdir().expect("root");
    let layout = AppInstallLayout::new(root.path().to_path_buf(), "v2rayn_desktop.exe");
    std::fs::write(layout.runner_exe(), b"stub").expect("runner stub");

    // Seed a flat current install so the first overlay keeps a rollback copy.
    std::fs::write(layout.app_exe(), b"old-app").expect("old app");
    std::fs::write(root.path().join("old.txt"), b"old").expect("old");

    let zip = make_zip(&[
        ("v2rayn_desktop.exe", b"new-app"),
        ("data/seed.txt", b"seed"),
    ]);
    let staged = stage_zip(root.path(), "7.99.0", &zip);

    let outcome = apply_app_upgrade(&layout, "7.99.0", &staged).expect("apply");
    assert!(outcome.app_exe.is_file());
    assert_eq!(outcome.restart.program, layout.app_exe());
    assert_eq!(outcome.restart.working_dir, root.path());
    assert_eq!(
        outcome.kept_previous.as_deref(),
        Some(layout.previous_dir().as_path())
    );
    // The external runner is never created or executed here.
    assert_eq!(std::fs::read(layout.runner_exe()).expect("stub"), b"stub");

    let restored = rollback_app_upgrade(&layout).expect("rollback");
    assert_eq!(restored, root.path());
    assert_eq!(
        std::fs::read(layout.app_exe()).expect("restored"),
        b"old-app"
    );
    assert!(root.path().join("old.txt").is_file());
}

#[test]
fn synthetic_wpf_zip_is_rejected() {
    let root = tempfile::tempdir().expect("root");
    let layout = AppInstallLayout::new(root.path().to_path_buf(), "v2rayn_desktop.exe");
    let zip = make_zip(&[("v2rayN.exe", b"wpf")]);
    let staged = stage_zip(root.path(), "7.99.0", &zip);
    assert!(apply_app_upgrade(&layout, "7.99.0", &staged).is_err());
    assert!(!layout.app_exe().exists());
}
