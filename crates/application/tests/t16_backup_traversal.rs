//! S6-01: restoring a crafted bundle must not write outside the data dir.
//!
//! Every test builds a synthetic bundle in a temp tree (never the live data
//! dir), tampers `manifest.json` with one malicious resource entry, and calls
//! [`application::BackupService::restore`]. Pre-fix the malicious entries pass
//! verification and land outside the data directory; post-fix the whole bundle
//! is rejected with `error.backup_invalid` and nothing is written.

use std::path::{Path, PathBuf};

use application::BackupService;
use persistence::Store;

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}

struct Layout {
    _base: tempfile::TempDir,
    data: PathBuf,
    bundle: PathBuf,
    outside: PathBuf,
    work: PathBuf,
    before_files: Vec<PathBuf>,
}

fn seed_data_dir(dir: &Path) {
    std::fs::create_dir_all(dir).expect("data dir");
    let store = Store::create(dir.join("guiNDB.db")).expect("create db");
    store
        .connection()
        .execute(
            "INSERT INTO SubItem (Id, Remarks) VALUES ('s1', 'source')",
            [],
        )
        .expect("insert row");
    drop(store);
    std::fs::write(dir.join("guiNConfig.json"), r#"{"IndexId":"n1"}"#).expect("config");
    std::fs::write(dir.join("notes.txt"), b"original notes").expect("resource");
}

fn list_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if entry.file_type().expect("type").is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn setup() -> Layout {
    let base = tempfile::tempdir().expect("base");
    let data = base.path().join("data");
    seed_data_dir(&data);
    let bundle = base.path().join("bundles").join("b0");
    BackupService::new(&data)
        .create_local(&bundle, 42)
        .expect("bundle");
    let outside = base.path().join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    let work = base.path().join("work");
    std::fs::create_dir_all(&work).expect("work");
    let before_files = list_files(&data);
    Layout {
        _base: base,
        data,
        bundle,
        outside,
        work,
        before_files,
    }
}

/// Append a resource entry to the bundle manifest and place `content` at
/// `bundle_root.join(relative)` (which may resolve outside the bundle root).
fn tamper(layout: &Layout, relative: &str, content: &[u8]) {
    let target = layout.bundle.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).expect("evil parent");
    }
    std::fs::write(&target, content).expect("evil bytes");
    let manifest_path = layout.bundle.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["referenced_resources"]
        .as_array_mut()
        .expect("resources")
        .push(serde_json::json!({
            "relative_path": relative,
            "sha256": sha256_hex(content),
            "bytes": content.len(),
        }));
    std::fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");
}

/// Append a resource entry for bytes that already sit at the resolved source
/// path (used when the setup owns the file placement).
fn tamper_manifest_entry(layout: &Layout, relative: &str, content: &[u8]) {
    let manifest_path = layout.bundle.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["referenced_resources"]
        .as_array_mut()
        .expect("resources")
        .push(serde_json::json!({
            "relative_path": relative,
            "sha256": sha256_hex(content),
            "bytes": content.len(),
        }));
    std::fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");
}

/// Reference a resource that cannot exist as a bundle file (reserved names,
/// ADS syntax, unreachable UNC): only the manifest is edited.
fn tamper_manifest_only(layout: &Layout, relative: &str) {
    tamper_manifest_entry(layout, relative, b"unreachable");
}

#[allow(clippy::result_large_err)]
fn restore(layout: &Layout) -> Result<persistence::backup::RestoreReport, domain::DomainError> {
    BackupService::new(&layout.data).restore(&layout.bundle, &layout.work)
}

fn assert_rejected(
    result: &Result<persistence::backup::RestoreReport, domain::DomainError>,
    layout: &Layout,
) {
    let error = result
        .as_ref()
        .expect_err("crafted bundle must be rejected");
    assert_eq!(error.message_key, "error.backup_invalid");
    assert!(
        error
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("rejected resource path"),
        "validation marker missing: {error:?}"
    );
    assert_eq!(
        list_files(&layout.data),
        layout.before_files,
        "rejected bundle must leave zero writes in the data dir"
    );
}

#[test]
fn restore_rejects_parent_escape() {
    let layout = setup();
    // `bundles/b0/../escape.txt` resolves to `bundles/escape.txt`: place the
    // attacker bytes there so hash verification passes pre-fix, while the
    // restore destination resolves outside the data dir.
    tamper(&layout, "../escape.txt", b"pwned");
    let result = restore(&layout);
    // Exploit proof (fails pre-fix): the restore accepted the bundle, so the
    // file outside the data dir now carries attacker bytes.
    if result.is_ok() {
        let escaped = layout.data.join("../escape.txt");
        panic!(
            "EXPLOIT: dotdot bundle accepted; outside file = {:?}",
            std::fs::read(&escaped)
        );
    }
    assert_rejected(&result, &layout);
    assert!(
        !layout.data.join("../escape.txt").exists(),
        "no file may be written outside the data dir"
    );
}

#[test]
fn restore_rejects_absolute_path() {
    let layout = setup();
    // For an absolute entry the bundle source and the restore destination are
    // the same path by construction: the attacker bytes sit there and the
    // manifest hash matches, so verification passes pre-fix.
    let victim = layout.outside.join("abs.txt");
    std::fs::write(&victim, b"pwned").expect("bundle bytes");
    let absolute = victim
        .to_string_lossy()
        .replace('/', std::path::MAIN_SEPARATOR_STR);
    tamper_manifest_entry(&layout, &absolute, b"pwned");
    let result = restore(&layout);
    if result.is_ok() {
        panic!("EXPLOIT: absolute-path bundle accepted; restore operated on {absolute}");
    }
    assert_rejected(&result, &layout);
    assert_eq!(
        std::fs::read(&victim).expect("victim"),
        b"pwned",
        "rejected restore must not touch the absolute path either"
    );
}

#[test]
fn restore_rejects_unc_path() {
    let layout = setup();
    tamper_manifest_only(&layout, r"\\127.0.0.1\share\evil.txt");
    let result = restore(&layout);
    assert_rejected(&result, &layout);
}

#[test]
fn restore_rejects_nested_parent_escape() {
    let layout = setup();
    // `bundles/b0/a/../../nested.txt` resolves to `bundles/nested.txt` while
    // the restore destination resolves outside the data dir.
    tamper(&layout, "a/../../nested.txt", b"pwned");
    let result = restore(&layout);
    assert_rejected(&result, &layout);
    assert!(
        !layout.data.join("a/../../nested.txt").exists(),
        "no file may be written outside the data dir"
    );
}

#[test]
fn restore_rejects_reserved_name() {
    let layout = setup();
    tamper_manifest_only(&layout, "NUL");
    let result = restore(&layout);
    assert_rejected(&result, &layout);
}

#[test]
fn restore_rejects_ads_colon_name() {
    let layout = setup();
    tamper_manifest_only(&layout, "sub:evil.txt");
    let result = restore(&layout);
    assert_rejected(&result, &layout);
}

#[cfg(windows)]
fn symlink_file(link: &Path, target: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("symlink file");
}

#[cfg(windows)]
fn symlink_dir(link: &Path, target: &Path) {
    std::os::windows::fs::symlink_dir(target, link).expect("symlink dir");
}

#[cfg(unix)]
fn symlink_file(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).expect("symlink file");
}

#[cfg(unix)]
fn symlink_dir(link: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, link).expect("symlink dir");
}

#[test]
fn restore_rejects_symlink_source_entry() {
    let layout = setup();
    let secret = layout.outside.join("secret.txt");
    std::fs::write(&secret, b"top-secret").expect("secret");
    symlink_file(&layout.bundle.join("link.txt"), &secret);
    // Hash over the link target so verification passes pre-fix.
    let manifest_path = layout.bundle.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    manifest["referenced_resources"]
        .as_array_mut()
        .expect("resources")
        .push(serde_json::json!({
            "relative_path": "link.txt",
            "sha256": sha256_hex(b"top-secret"),
            "bytes": 10,
        }));
    std::fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");
    let result = restore(&layout);
    assert_rejected(&result, &layout);
}

#[test]
fn restore_rejects_symlink_parent_escape() {
    let layout = setup();
    let real_dir = layout.outside.join("realdir");
    std::fs::create_dir_all(&real_dir).expect("realdir");
    symlink_dir(&layout.data.join("plug"), &real_dir);
    // Refresh the pre-restore snapshot: the symlink itself is legitimate setup.
    let layout = Layout {
        before_files: list_files(&layout.data),
        _base: layout._base,
        data: layout.data,
        bundle: layout.bundle,
        outside: layout.outside,
        work: layout.work,
    };
    tamper(&layout, "plug/payload.txt", b"pwned");
    let result = restore(&layout);
    if result.is_ok() {
        panic!(
            "EXPLOIT: symlinked-parent bundle accepted; outside dir = {:?}",
            std::fs::read(real_dir.join("payload.txt"))
        );
    }
    assert_rejected(&result, &layout);
    assert!(
        !real_dir.join("payload.txt").exists(),
        "no file may be written through a symlinked parent"
    );
}

#[test]
fn restore_normal_bundle_still_restores() {
    let layout = setup();
    let result = restore(&layout);
    assert!(result.is_ok(), "clean bundle must restore: {result:?}");
    assert_eq!(
        std::fs::read(layout.data.join("notes.txt")).expect("notes"),
        b"original notes"
    );
}
