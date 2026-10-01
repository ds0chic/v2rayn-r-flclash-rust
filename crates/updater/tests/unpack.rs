//! Safe archive extraction: traversal, symlink, bomb guards, valid extraction.

mod common;

use common::archives::{write_tar, write_tar_typed, write_zip, ZipEntry};
use updater::{safe_join, safe_unpack_targz, safe_unpack_zip, UnpackLimits, UpdateError};

fn limits() -> UnpackLimits {
    UnpackLimits::default()
}

#[test]
fn safe_join_allows_normal_and_normalizes_separators() {
    let root = std::path::Path::new("/tmp/root");
    assert_eq!(
        safe_join(root, "bin/xray.exe").unwrap(),
        std::path::Path::new("/tmp/root/bin/xray.exe")
    );
    assert_eq!(
        safe_join(root, "bin\\xray.exe").unwrap(),
        std::path::Path::new("/tmp/root/bin/xray.exe")
    );
    assert_eq!(
        safe_join(root, "./a/b").unwrap(),
        std::path::Path::new("/tmp/root/a/b")
    );
}

#[test]
fn safe_join_rejects_traversal_and_absolute() {
    let root = std::path::Path::new("/tmp/root");
    for bad in [
        "../evil",
        "a/../../evil",
        "/etc/passwd",
        "C:/windows",
        "..\\evil",
        "",
    ] {
        assert!(
            matches!(safe_join(root, bad), Err(UpdateError::UnsafeArchivePath(_))),
            "should reject {bad}"
        );
    }
}

#[test]
fn zip_extracts_valid_archive() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("core.zip");
    write_zip(
        &archive,
        &[
            ZipEntry::file("xray/xray.exe", b"binary"),
            ZipEntry::file("xray/geoip.dat", b"geo"),
        ],
    );
    let out = dir.path().join("out");
    let summary = safe_unpack_zip(&archive, &out, limits()).unwrap();
    assert_eq!(summary.entries, 2);
    assert_eq!(summary.bytes, 9);
    assert!(out.join("xray/xray.exe").exists());
    assert_eq!(std::fs::read(out.join("xray/geoip.dat")).unwrap(), b"geo");
}

#[test]
fn zip_traversal_entry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    write_zip(&archive, &[ZipEntry::file("../evil.txt", b"boom")]);
    let out = dir.path().join("out");
    let err = safe_unpack_zip(&archive, &out, limits()).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchivePath(_)));
    assert!(!dir.path().join("evil.txt").exists());
}

#[test]
fn zip_absolute_path_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("abs.zip");
    write_zip(&archive, &[ZipEntry::file("/tmp/evil.txt", b"boom")]);
    let out = dir.path().join("out");
    let err = safe_unpack_zip(&archive, &out, limits()).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchivePath(_)));
}

#[test]
fn zip_symlink_entry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("link.zip");
    write_zip(&archive, &[ZipEntry::symlink("xray/link", "/etc/passwd")]);
    let out = dir.path().join("out");
    let err = safe_unpack_zip(&archive, &out, limits()).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}

#[test]
fn zip_single_entry_bomb_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("bomb.zip");
    let big = vec![0u8; 4096];
    write_zip(&archive, &[ZipEntry::file("big.bin", &big)]);
    let out = dir.path().join("out");
    let tight = UnpackLimits {
        max_entry_bytes: 1024,
        ..limits()
    };
    let err = safe_unpack_zip(&archive, &out, tight).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}

#[test]
fn zip_total_bomb_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("total.zip");
    let one = vec![1u8; 800];
    write_zip(
        &archive,
        &[ZipEntry::file("a.bin", &one), ZipEntry::file("b.bin", &one)],
    );
    let out = dir.path().join("out");
    let tight = UnpackLimits {
        max_total_bytes: 1000,
        max_entry_bytes: 900,
        ..limits()
    };
    let err = safe_unpack_zip(&archive, &out, tight).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}

#[test]
fn zip_too_many_entries_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("many.zip");
    let entries: Vec<ZipEntry> = (0..10)
        .map(|i| ZipEntry::file(Box::leak(format!("f{i}.txt").into_boxed_str()), b"x"))
        .collect();
    write_zip(&archive, &entries);
    let out = dir.path().join("out");
    let tight = UnpackLimits {
        max_entries: 3,
        ..limits()
    };
    let err = safe_unpack_zip(&archive, &out, tight).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}

#[test]
fn targz_extracts_valid_archive() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("core.tar.gz");
    write_tar(
        &archive,
        &[("sing-box", b"binary-data"), ("LICENSE", b"text")],
        true,
    );
    let out = dir.path().join("out");
    let summary = safe_unpack_targz(&archive, &out, limits()).unwrap();
    assert_eq!(summary.entries, 2);
    assert_eq!(std::fs::read(out.join("sing-box")).unwrap(), b"binary-data");
    assert_eq!(std::fs::read(out.join("LICENSE")).unwrap(), b"text");
}

#[test]
fn targz_traversal_entry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.tar.gz");
    write_tar(&archive, &[("../evil", b"boom")], true);
    let out = dir.path().join("out");
    let err = safe_unpack_targz(&archive, &out, limits()).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchivePath(_)));
    assert!(!dir.path().join("evil").exists());
}

#[test]
fn targz_symlink_entry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("link.tar.gz");
    write_tar_typed(&archive, &[("link", b"/etc/passwd", b'2')], true);
    let out = dir.path().join("out");
    let err = safe_unpack_targz(&archive, &out, limits()).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}

#[test]
fn targz_entry_bomb_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("bomb.tar.gz");
    let big = vec![0u8; 4096];
    write_tar(&archive, &[("big.bin", &big)], true);
    let out = dir.path().join("out");
    let tight = UnpackLimits {
        max_entry_bytes: 512,
        ..limits()
    };
    let err = safe_unpack_targz(&archive, &out, tight).unwrap_err();
    assert!(matches!(err, UpdateError::UnsafeArchive(_)));
}
