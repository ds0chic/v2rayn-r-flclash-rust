//! Atomic replacement and rollback within a caller-owned root.
//!
//! `apply_atomic` performs a same-volume directory swap using two renames,
//! keeping the previous version until the new one is in place. Any failure
//! restores the original layout. The external-upgrade coordinator only returns
//! a spec — it never spawns a process (the plan forbids touching live files in
//! this task; the spec exists so a future Windows `AmazTool` equivalent can be
//! verified).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::UpdateError;

/// One installed file recorded in the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledEntry {
    /// Path relative to the version directory.
    pub relative_path: String,
    /// Lowercase hex SHA-256.
    pub sha256: String,
    pub size: u64,
}

/// Version/hash bookkeeping written next to an install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub version: String,
    /// Directory name of the previous version kept for rollback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_version: Option<String>,
    #[serde(default)]
    pub files: Vec<InstalledEntry>,
}

impl InstallManifest {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            previous_version: None,
            files: Vec::new(),
        }
    }

    /// Convenience: fill `files` from a directory tree (regular files only).
    pub fn scan_directory(version: impl Into<String>, dir: &Path) -> Result<Self, UpdateError> {
        let mut files = Vec::new();
        collect_files(dir, dir, &mut files)?;
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(Self {
            version: version.into(),
            previous_version: None,
            files,
        })
    }
}

fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<InstalledEntry>,
) -> Result<(), UpdateError> {
    for entry in std::fs::read_dir(dir).map_err(|e| UpdateError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| UpdateError::Io(e.to_string()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| UpdateError::InstallConflict(path.display().to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path).map_err(|e| UpdateError::Io(e.to_string()))?;
            use sha2::{Digest, Sha256};
            out.push(InstalledEntry {
                relative_path: relative,
                sha256: hex::encode(Sha256::digest(&bytes)),
                size: bytes.len() as u64,
            });
        }
    }
    Ok(())
}

/// A plan describing a single atomic replacement.
#[derive(Debug, Clone)]
pub struct InstallPlan {
    /// Managed root; every path in the plan must live inside it.
    pub root: PathBuf,
    /// Directory currently holding the active version.
    pub current_dir: PathBuf,
    /// Directory holding the new version (already unpacked).
    pub staged_dir: PathBuf,
    /// Directory name to keep the previous version under (`<root>/<keep_name>`).
    pub keep_name: String,
    pub version: String,
    /// When true, the current version is deleted after a successful swap.
    pub discard_previous: bool,
}

impl InstallPlan {
    pub fn new(
        root: impl Into<PathBuf>,
        current_dir: impl Into<PathBuf>,
        staged_dir: impl Into<PathBuf>,
        keep_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            root: root.into(),
            current_dir: current_dir.into(),
            staged_dir: staged_dir.into(),
            keep_name: keep_name.into(),
            version: version.into(),
            discard_previous: false,
        }
    }

    fn validate(&self) -> Result<(), UpdateError> {
        for path in [&self.current_dir, &self.staged_dir] {
            if !path.starts_with(&self.root) {
                return Err(UpdateError::UnsafeTarget(path.display().to_string()));
            }
        }
        if self.keep_name.contains('/') || self.keep_name.contains('\\') || self.keep_name == ".." {
            return Err(UpdateError::UnsafeTarget(self.keep_name.clone()));
        }
        if !self.staged_dir.is_dir() {
            return Err(UpdateError::InstallConflict(format!(
                "staged dir missing: {}",
                self.staged_dir.display()
            )));
        }
        Ok(())
    }
}

/// Outcome of a successful swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub version: String,
    /// Where the previous version was kept (`None` if there was none).
    pub kept_previous: Option<PathBuf>,
    pub manifest: InstallManifest,
}

/// Injection points used by crash-recovery tests. Production callers use
/// [`apply_atomic`], which never injects.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailPoint {
    None,
    /// Fail the `current -> keep` rename.
    StageRename,
    /// Fail the `staged -> current` rename (after the stage rename succeeded).
    CommitRename,
}

/// Injection-aware variant of [`apply_atomic`]. Not part of the stable API.
#[doc(hidden)]
pub fn apply_atomic_inject(
    plan: &InstallPlan,
    fail_at: FailPoint,
) -> Result<ApplyOutcome, UpdateError> {
    apply_atomic_inner(plan, fail_at)
}

/// Swap `staged_dir` into `current_dir` atomically, keeping the old version.
///
/// Sequence within one filesystem:
/// 1. rename `current_dir` → `<root>/<keep_name>`
/// 2. rename `staged_dir` → `current_dir`
///
/// If step 2 fails, step 1 is rolled back. The old version is never deleted
/// before the new one is active, so a crash leaves either the old or the new
/// layout intact (worst case with the new version under `keep_name`).
pub fn apply_atomic(plan: &InstallPlan) -> Result<ApplyOutcome, UpdateError> {
    apply_atomic_inner(plan, FailPoint::None)
}

fn apply_atomic_inner(plan: &InstallPlan, fail_at: FailPoint) -> Result<ApplyOutcome, UpdateError> {
    plan.validate()?;
    let keep_dir = plan.root.join(&plan.keep_name);

    if keep_dir.exists() {
        return Err(UpdateError::InstallConflict(format!(
            "keep dir already exists: {}",
            keep_dir.display()
        )));
    }
    let had_current = plan.current_dir.exists();

    if had_current {
        if fail_at == FailPoint::StageRename {
            return Err(UpdateError::Io("injected: stage rename".into()));
        }
        std::fs::rename(&plan.current_dir, &keep_dir)
            .map_err(|e| UpdateError::Io(format!("stage rename: {e}")))?;
    }
    if fail_at == FailPoint::CommitRename {
        if had_current {
            let _ = std::fs::rename(&keep_dir, &plan.current_dir);
        }
        return Err(UpdateError::Io("injected: commit rename".into()));
    }
    if let Err(error) = std::fs::rename(&plan.staged_dir, &plan.current_dir) {
        // Roll back the first rename so the caller keeps a usable install.
        if had_current {
            let _ = std::fs::rename(&keep_dir, &plan.current_dir);
        }
        return Err(UpdateError::Io(format!("commit rename: {error}")));
    }

    let manifest = InstallManifest::scan_directory(&plan.version, &plan.current_dir)?;
    let mut manifest = manifest;
    manifest.previous_version = had_current.then(|| plan.keep_name.clone());

    if plan.discard_previous && had_current {
        std::fs::remove_dir_all(&keep_dir).map_err(|e| UpdateError::Io(e.to_string()))?;
        manifest.previous_version = None;
        return Ok(ApplyOutcome {
            version: plan.version.clone(),
            kept_previous: None,
            manifest,
        });
    }

    Ok(ApplyOutcome {
        version: plan.version.clone(),
        kept_previous: had_current.then_some(keep_dir),
        manifest,
    })
}

/// Verify a manifest against the on-disk tree it describes.
pub fn verify_manifest(dir: &Path, manifest: &InstallManifest) -> Result<(), UpdateError> {
    use sha2::{Digest, Sha256};
    for entry in &manifest.files {
        let path = dir.join(
            entry
                .relative_path
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let bytes = std::fs::read(&path).map_err(|e| UpdateError::Io(e.to_string()))?;
        let actual = hex::encode(Sha256::digest(&bytes));
        if actual != entry.sha256 {
            return Err(UpdateError::DigestMismatch {
                expected: entry.sha256.clone(),
                actual,
            });
        }
    }
    Ok(())
}

/// A spec for an external updater process (Windows cannot replace a running
/// executable). The coordinator returns this; it never spawns anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalUpgradeSpec {
    /// Executable that will perform the swap.
    pub helper_exe: PathBuf,
    /// Archive or directory the helper should install from.
    pub source: PathBuf,
    /// Installation root the helper owns.
    pub install_root: PathBuf,
    /// PID of the process that must exit first (the running app).
    pub wait_for_pid: u32,
    /// Extra CLI arguments for the helper (upstream uses a file path).
    pub args: Vec<String>,
}

/// Coordinates an upgrade that cannot run in-process.
#[derive(Debug, Clone)]
pub struct UpgradeCoordinator {
    pub helper_exe: PathBuf,
    pub install_root: PathBuf,
}

impl UpgradeCoordinator {
    pub fn new(helper_exe: impl Into<PathBuf>, install_root: impl Into<PathBuf>) -> Self {
        Self {
            helper_exe: helper_exe.into(),
            install_root: install_root.into(),
        }
    }

    /// Build the spec for the external helper. No process is started.
    pub fn external_upgrade_spec(
        &self,
        source: impl Into<PathBuf>,
        wait_for_pid: u32,
    ) -> Result<ExternalUpgradeSpec, UpdateError> {
        let source = source.into();
        if !source.starts_with(&self.install_root) && !source.is_absolute() {
            return Err(UpdateError::UnsafeTarget(source.display().to_string()));
        }
        Ok(ExternalUpgradeSpec {
            helper_exe: self.helper_exe.clone(),
            args: vec![source.to_string_lossy().into_owned()],
            source,
            install_root: self.install_root.clone(),
            wait_for_pid,
        })
    }
}

/// Free function form used by the public API.
pub fn external_upgrade_spec(
    coordinator: &UpgradeCoordinator,
    source: impl Into<PathBuf>,
    wait_for_pid: u32,
) -> Result<ExternalUpgradeSpec, UpdateError> {
    coordinator.external_upgrade_spec(source, wait_for_pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_rejects_paths_outside_root() {
        let plan = InstallPlan::new(
            "/root",
            "/elsewhere/current",
            "/root/staged",
            "previous",
            "1.0.0",
        );
        assert!(matches!(plan.validate(), Err(UpdateError::UnsafeTarget(_))));
    }

    #[test]
    fn plan_rejects_keep_name_with_separator() {
        let plan = InstallPlan::new("/root", "/root/current", "/root/staged", "../x", "1.0.0");
        assert!(matches!(plan.validate(), Err(UpdateError::UnsafeTarget(_))));
    }

    #[test]
    fn external_spec_does_not_start_process() {
        let coordinator = UpgradeCoordinator::new("/root/helper.exe", "/root");
        let spec = coordinator
            .external_upgrade_spec("/root/stage/new.zip", 4242)
            .unwrap();
        assert_eq!(spec.wait_for_pid, 4242);
        assert_eq!(spec.args, vec!["/root/stage/new.zip".to_string()]);
        assert!(!spec.source.is_dir());
    }
}
