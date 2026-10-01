//! Shared helpers for the T04 integration tests.
#![allow(dead_code)]

use std::path::PathBuf;

/// Root of the synthetic fixtures.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/synthetic")
        .canonicalize()
        .expect("fixtures/synthetic directory")
}

pub fn upstream_v2() -> PathBuf {
    fixtures_dir().join("upstream-v2")
}

pub fn upstream_v3() -> PathBuf {
    fixtures_dir().join("upstream-v3")
}

/// A fresh temp work directory plus target path, kept alive by the caller.
pub fn temp_workspace() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let work = dir.path().join("work");
    let target = dir.path().join("app.db");
    std::fs::create_dir_all(&work).unwrap();
    (dir, work, target)
}
