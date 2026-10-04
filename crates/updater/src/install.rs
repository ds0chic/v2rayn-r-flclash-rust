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

/// How an [`InstallPlan`] places the new version.
///
/// `DirSwap` (cores) renames the whole current directory aside and swaps in the
/// staged directory. `FlatOverlay` (the application, whose release ZIP and the
/// Inno installer both lay files out flat in `<install_root>`) copies the
/// staged files over the install root in place, backing up every overwritten
/// file under `<root>/<keep_name>` for rollback. This matches upstream
/// `AmazTool.UpgradeApp`, which extracts the package over `StartupPath()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InstallMode {
    #[default]
    DirSwap,
    FlatOverlay,
}

/// A plan describing a single atomic replacement.
///
/// Serialised as JSON so an out-of-process helper (the `upgrade_runner`
/// service) can load exactly the same plan the coordinator produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    /// Managed root; every path in the plan must live inside it.
    pub root: PathBuf,
    /// Directory currently holding the active version. For `FlatOverlay` this
    /// is the install root itself.
    pub current_dir: PathBuf,
    /// Directory holding the new version (already unpacked).
    pub staged_dir: PathBuf,
    /// Directory name to keep the previous version under (`<root>/<keep_name>`).
    pub keep_name: String,
    pub version: String,
    /// When true, the current version is deleted after a successful swap.
    pub discard_previous: bool,
    /// Placement strategy; defaults to the historical directory swap.
    #[serde(default)]
    pub mode: InstallMode,
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
            mode: InstallMode::DirSwap,
        }
    }

    /// Plan that overlays a flat staged payload over the install root.
    pub fn flat_overlay(
        root: impl Into<PathBuf>,
        staged_dir: impl Into<PathBuf>,
        keep_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        let root = root.into();
        Self {
            current_dir: root.clone(),
            root,
            staged_dir: staged_dir.into(),
            keep_name: keep_name.into(),
            version: version.into(),
            discard_previous: false,
            mode: InstallMode::FlatOverlay,
        }
    }

    fn validate(&self) -> Result<(), UpdateError> {
        for path in [&self.current_dir, &self.staged_dir] {
            if !is_within_root(&self.root, path) {
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
        if self.mode == InstallMode::FlatOverlay
            && normalize_lexically(&self.current_dir) != normalize_lexically(&self.root)
        {
            return Err(UpdateError::UnsafeTarget(format!(
                "flat overlay current dir must equal the install root: {}",
                self.current_dir.display()
            )));
        }
        if self.mode == InstallMode::FlatOverlay
            && normalize_lexically(&self.staged_dir)
                .starts_with(normalize_lexically(std::path::Path::new(&self.keep_name)))
        {
            return Err(UpdateError::UnsafeTarget(
                self.staged_dir.display().to_string(),
            ));
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
    if plan.mode == InstallMode::FlatOverlay {
        return apply_flat_overlay(plan, fail_at);
    }
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

/// Flat-overlay placement: copy every staged file over the install root,
/// moving any overwritten file into the keep directory first so a failed copy
/// can be rolled back. Files the user added that are not part of the package
/// are never touched.
fn apply_flat_overlay(plan: &InstallPlan, fail_at: FailPoint) -> Result<ApplyOutcome, UpdateError> {
    let root = plan.root.clone();
    let keep_dir = root.join(&plan.keep_name);
    if fail_at == FailPoint::StageRename {
        return Err(UpdateError::Io("injected: overlay stage".into()));
    }
    if keep_dir.exists() {
        std::fs::remove_dir_all(&keep_dir).map_err(|e| UpdateError::Io(e.to_string()))?;
    }

    let mut files = Vec::new();
    collect_relative_files(&plan.staged_dir, &plan.staged_dir, &mut files)?;
    files.sort();

    // Phase 1: back up files that the package overwrites.
    let mut replaced: Vec<PathBuf> = Vec::new();
    for rel in &files {
        let dest = root.join(rel);
        if !dest.is_file() {
            continue;
        }
        let backup = keep_dir.join(rel);
        if let Some(parent) = backup.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                restore_overlay_backups(&root, &keep_dir, &replaced);
                return Err(UpdateError::Io(format!("overlay backup dir: {e}")));
            }
        }
        if let Err(e) = std::fs::rename(&dest, &backup) {
            restore_overlay_backups(&root, &keep_dir, &replaced);
            return Err(UpdateError::Io(format!("overlay backup: {e}")));
        }
        replaced.push(rel.clone());
    }
    if fail_at == FailPoint::CommitRename {
        restore_overlay_backups(&root, &keep_dir, &replaced);
        let _ = std::fs::remove_dir_all(&keep_dir);
        return Err(UpdateError::Io("injected: overlay commit".into()));
    }

    // Phase 2: copy the new files into place.
    let mut copied: Vec<PathBuf> = Vec::new();
    for rel in &files {
        let src = plan.staged_dir.join(rel);
        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                remove_overlay_copies(&root, &copied);
                restore_overlay_backups(&root, &keep_dir, &replaced);
                return Err(UpdateError::Io(format!("overlay target dir: {e}")));
            }
        }
        match std::fs::copy(&src, &dest) {
            Ok(_) => copied.push(rel.clone()),
            Err(e) => {
                remove_overlay_copies(&root, &copied);
                restore_overlay_backups(&root, &keep_dir, &replaced);
                return Err(UpdateError::Io(format!("overlay copy: {e}")));
            }
        }
    }

    let kept = !replaced.is_empty();
    if !kept && keep_dir.is_dir() {
        let _ = std::fs::remove_dir_all(&keep_dir);
    }
    let mut manifest = InstallManifest::scan_directory(&plan.version, &plan.staged_dir)?;
    manifest.previous_version = kept.then(|| plan.keep_name.clone());
    if plan.discard_previous && kept {
        let _ = std::fs::remove_dir_all(&keep_dir);
        manifest.previous_version = None;
    }
    let kept_previous = (kept && keep_dir.is_dir()).then_some(keep_dir);
    Ok(ApplyOutcome {
        version: plan.version.clone(),
        kept_previous,
        manifest,
    })
}

fn collect_relative_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), UpdateError> {
    for entry in std::fs::read_dir(dir).map_err(|e| UpdateError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| UpdateError::Io(e.to_string()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| UpdateError::Io(e.to_string()))?;
        if file_type.is_dir() {
            collect_relative_files(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| UpdateError::InstallConflict(path.display().to_string()))?;
            out.push(rel.to_path_buf());
        }
    }
    Ok(())
}

fn restore_overlay_backups(root: &Path, keep_dir: &Path, replaced: &[PathBuf]) {
    for rel in replaced {
        let backup = keep_dir.join(rel);
        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::rename(&backup, &dest);
    }
}

fn remove_overlay_copies(root: &Path, copied: &[PathBuf]) {
    for rel in copied {
        let _ = std::fs::remove_file(root.join(rel));
    }
}

/// directory is missing but the kept previous version is present, rename it
/// back into place. Returns whether a restore happened.
///
/// This is the external runner's safety net: [`apply_atomic`] already rolls
/// back its own rename failures, but a helper that crashes between steps can
/// still leave only the kept version, which this recovers.
pub fn restore_previous(plan: &InstallPlan) -> Result<bool, UpdateError> {
    if plan.mode == InstallMode::FlatOverlay {
        // The install root always exists; `apply_atomic` already rolls back its
        // own overlay failures in-process, so there is nothing to rename.
        return Ok(false);
    }
    if plan.current_dir.exists() {
        return Ok(false);
    }
    let keep_dir = plan.root.join(&plan.keep_name);
    if !keep_dir.is_dir() {
        return Ok(false);
    }
    std::fs::rename(&keep_dir, &plan.current_dir)
        .map_err(|e| UpdateError::Io(format!("restore previous: {e}")))?;
    Ok(true)
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
///
/// `args` is the exact command line for the `upgrade_runner` helper:
/// `--plan <json> --result <json> --pid <n> [--restart-exe <path>
/// --restart-cwd <dir>]`.
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
    /// CLI arguments for the helper (runner flags, not just a bare path).
    pub args: Vec<String>,
    /// Plan JSON the runner loads (mirrors `--plan`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PathBuf>,
    /// Result JSON the runner writes (mirrors `--result`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<PathBuf>,
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

    /// Build the spec for the external helper, emitting the real
    /// `upgrade_runner` command line. No process is started.
    ///
    /// `restart_exe`/`restart_cwd` are the relaunch target the runner spawns
    /// after a successful swap; pass `None` to leave restart to the caller.
    pub fn runner_spec(
        &self,
        source: impl Into<PathBuf>,
        plan_path: impl Into<PathBuf>,
        result_path: impl Into<PathBuf>,
        restart_exe: Option<&Path>,
        restart_cwd: Option<&Path>,
        wait_for_pid: u32,
    ) -> Result<ExternalUpgradeSpec, UpdateError> {
        let source = source.into();
        if !is_within_root(&self.install_root, &source) {
            return Err(UpdateError::UnsafeTarget(source.display().to_string()));
        }
        let plan_path = plan_path.into();
        let result_path = result_path.into();
        let mut args = vec![
            "--plan".to_string(),
            plan_path.to_string_lossy().into_owned(),
            "--result".to_string(),
            result_path.to_string_lossy().into_owned(),
            "--pid".to_string(),
            wait_for_pid.to_string(),
        ];
        if let Some(exe) = restart_exe {
            args.push("--restart-exe".to_string());
            args.push(exe.to_string_lossy().into_owned());
        }
        if let Some(cwd) = restart_cwd {
            args.push("--restart-cwd".to_string());
            args.push(cwd.to_string_lossy().into_owned());
        }
        Ok(ExternalUpgradeSpec {
            helper_exe: self.helper_exe.clone(),
            source,
            install_root: self.install_root.clone(),
            wait_for_pid,
            args,
            plan: Some(plan_path),
            result: Some(result_path),
        })
    }
}

/// Free function form used by the public API.
pub fn external_upgrade_spec(
    coordinator: &UpgradeCoordinator,
    source: impl Into<PathBuf>,
    wait_for_pid: u32,
) -> Result<ExternalUpgradeSpec, UpdateError> {
    // Legacy bare-path entry point kept for callers that only need the helper
    // and staging source; production app upgrades use
    // [`UpgradeCoordinator::runner_spec`] so the arguments match the runner.
    let source = source.into();
    if !is_within_root(&coordinator.install_root, &source) {
        return Err(UpdateError::UnsafeTarget(source.display().to_string()));
    }
    Ok(ExternalUpgradeSpec {
        helper_exe: coordinator.helper_exe.clone(),
        args: vec![source.to_string_lossy().into_owned()],
        source,
        install_root: coordinator.install_root.clone(),
        wait_for_pid,
        plan: None,
        result: None,
    })
}

/// Lexically normalize a path (resolve `.`/`..` without touching the fs).
fn normalize_lexically(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    // Keep leading `..` for relative paths.
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// A Windows drive-absolute path (`C:\…` / `C:/…`), even when parsed on a
/// non-Windows host where `Path::is_absolute` would be false.
fn is_absolute_any(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let s = path.as_os_str().to_string_lossy();
    let mut chars = s.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(d), Some(':')) if d.is_ascii_alphabetic()
    )
}

/// Whether `path` is located inside `root` after lexical normalization.
/// When both paths exist on disk, canonicalized prefixes are compared as
/// well so symlinks cannot escape the managed root.
fn is_within_root(root: &Path, path: &Path) -> bool {
    // Windows drive-absolute sources never live inside the managed root
    // unless they lexically start with it (checked below on Windows).
    if is_absolute_any(path) && cfg!(not(windows)) {
        let norm_root = normalize_lexically(root);
        let norm_path = normalize_lexically(path);
        // On non-Windows hosts a `C:\…` path cannot be inside a POSIX root.
        if !norm_path.starts_with(&norm_root) {
            return false;
        }
    }
    let norm_root = normalize_lexically(root);
    // Relative sources are resolved against the root (staging layout).
    let joined = if path.is_absolute() || is_absolute_any(path) {
        normalize_lexically(path)
    } else {
        normalize_lexically(&norm_root.join(path))
    };
    if !joined.starts_with(&norm_root) {
        return false;
    }
    // When the filesystem entries exist, re-verify via canonical paths to
    // defeat symlink / 8.3 / mount-point escapes.
    match (std::fs::canonicalize(root), std::fs::canonicalize(&joined)) {
        (Ok(canon_root), Ok(canon_path)) => canon_path.starts_with(&canon_root),
        _ => true,
    }
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
        let spec = external_upgrade_spec(&coordinator, "/root/stage/new.zip", 4242).unwrap();
        assert_eq!(spec.wait_for_pid, 4242);
        assert_eq!(spec.args, vec!["/root/stage/new.zip".to_string()]);
        assert!(!spec.source.is_dir());
    }

    #[test]
    fn external_spec_rejects_absolute_path_outside_root() {
        let coordinator = UpgradeCoordinator::new("/root/helper.exe", "/root");
        assert!(matches!(
            external_upgrade_spec(&coordinator, "/elsewhere/evil.zip", 1),
            Err(UpdateError::UnsafeTarget(_))
        ));
        assert!(matches!(
            external_upgrade_spec(&coordinator, "C:/Windows/evil.zip", 1),
            Err(UpdateError::UnsafeTarget(_))
        ));
        assert!(matches!(
            external_upgrade_spec(&coordinator, r"C:\Windows\evil.zip", 1),
            Err(UpdateError::UnsafeTarget(_))
        ));
        // Traversal via `..` must not escape either.
        assert!(matches!(
            external_upgrade_spec(&coordinator, "/root/../evil.zip", 1),
            Err(UpdateError::UnsafeTarget(_))
        ));
        // Relative paths resolve against the root staging layout.
        assert!(external_upgrade_spec(&coordinator, "stage/new.zip", 1).is_ok());
    }

    fn tmp(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("v2rayn-install-{tag}-{nanos}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn flat_overlay_replaces_managed_files_and_keeps_user_files() {
        let root = tmp("overlay");
        std::fs::write(root.join("v2rayn_desktop.exe"), b"old-exe").unwrap();
        std::fs::write(root.join("user.txt"), b"user-data").unwrap();
        let staged = root.join(".staging").join("app-9.9.9");
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(staged.join("v2rayn_desktop.exe"), b"new-exe").unwrap();
        std::fs::write(staged.join("net_host.exe"), b"new-net").unwrap();

        let plan = InstallPlan::flat_overlay(&root, &staged, "app.previous", "9.9.9");
        let outcome = apply_atomic(&plan).unwrap();
        assert_eq!(
            std::fs::read(root.join("v2rayn_desktop.exe")).unwrap(),
            b"new-exe"
        );
        assert!(root.join("net_host.exe").is_file());
        // Untouched user file survives the overlay.
        assert_eq!(std::fs::read(root.join("user.txt")).unwrap(), b"user-data");
        // The overwritten file is kept for rollback.
        assert_eq!(
            std::fs::read(root.join("app.previous").join("v2rayn_desktop.exe")).unwrap(),
            b"old-exe"
        );
        assert_eq!(outcome.kept_previous, Some(root.join("app.previous")));
    }

    #[test]
    fn flat_overlay_commit_failure_leaves_original_intact() {
        let root = tmp("overlay-fail");
        std::fs::write(root.join("v2rayn_desktop.exe"), b"old-exe").unwrap();
        let staged = root.join(".staging").join("app-9.9.9");
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(staged.join("v2rayn_desktop.exe"), b"new-exe").unwrap();

        let plan = InstallPlan::flat_overlay(&root, &staged, "app.previous", "9.9.9");
        let error = apply_atomic_inject(&plan, FailPoint::CommitRename).unwrap_err();
        assert!(matches!(error, UpdateError::Io(_)));
        assert_eq!(
            std::fs::read(root.join("v2rayn_desktop.exe")).unwrap(),
            b"old-exe"
        );
    }

    #[test]
    fn runner_spec_args_match_runner_cli() {
        let root = tmp("runner-cli");
        let coordinator = UpgradeCoordinator::new(root.join("v2rayN-upgrade.exe"), &root);
        let spec = coordinator
            .runner_spec(
                root.join(".staging").join("app-1.2.3"),
                root.join(".staging").join("plan.json"),
                root.join(".staging").join("result.json"),
                Some(&root.join("v2rayn_desktop.exe")),
                Some(&root),
                4242,
            )
            .unwrap();
        let expected = vec![
            "--plan".to_string(),
            root.join(".staging")
                .join("plan.json")
                .to_string_lossy()
                .into_owned(),
            "--result".to_string(),
            root.join(".staging")
                .join("result.json")
                .to_string_lossy()
                .into_owned(),
            "--pid".to_string(),
            "4242".to_string(),
            "--restart-exe".to_string(),
            root.join("v2rayn_desktop.exe")
                .to_string_lossy()
                .into_owned(),
            "--restart-cwd".to_string(),
            root.to_string_lossy().into_owned(),
        ];
        assert_eq!(spec.args, expected);
        assert!(spec.plan.is_some());
        assert!(spec.result.is_some());
    }
}
