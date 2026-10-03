//! Application self-update: stage -> atomic replace (keep `.previous`) ->
//! restart command (FIX-12B).
//!
//! Modelled on the frozen upstream `AmazTool/UpgradeApp.cs` and
//! `AmazTool/Utils.StartV2RayN`, driven by `CheckUpdateViewModel.UpgradeN`:
//! the app stages the downloaded package, launches an external helper
//! (`AmazTool.exe` upstream, `v2rayN-upgrade.exe` here), exits, and the helper
//! extracts over the install directory and relaunches the application.
//!
//! This module freezes the same shape for the Flutter rebuild:
//!
//! - the active application payload lives under `<install_root>/app`;
//! - [`apply_app_upgrade`] swaps it atomically via [`crate::install::apply_atomic`],
//!   keeping `<install_root>/app.previous` for rollback;
//! - [`AppInstallLayout::restart_command`] *constructs* the relaunch command
//!   (never spawns it); production hands both the staged source and the helper
//!   to the external runner through [`crate::install::UpgradeCoordinator`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::UpdateError;
use crate::install::{apply_atomic, InstallManifest, InstallPlan, UpgradeCoordinator};

/// Default external runner file name (`AmazTool` equivalent).
pub const DEFAULT_RUNNER_NAME: &str = "v2rayN-upgrade.exe";

/// Default application executable name when the running process cannot be
/// resolved.
pub const DEFAULT_APP_EXE: &str = if cfg!(windows) {
    "v2rayN.exe"
} else {
    "v2rayN"
};

/// Sub-directory of the install root holding the active application payload.
pub const PAYLOAD_DIR: &str = "app";

/// Sibling directory keeping the previous payload for rollback.
pub const PREVIOUS_DIR: &str = "app.previous";

/// Directory (under the install root) used to stage a new payload.
pub const STAGING_DIR: &str = ".staging";

/// Frozen application install layout: install root, executable name and the
/// external runner. The install root is always supplied explicitly (FIX-12 gap:
/// the application root is not `cores_root.parent()`).
#[derive(Debug, Clone)]
pub struct AppInstallLayout {
    install_root: PathBuf,
    app_exe_name: String,
    runner_name: String,
}

impl AppInstallLayout {
    pub fn new(install_root: impl Into<PathBuf>, app_exe_name: impl Into<String>) -> Self {
        Self {
            install_root: install_root.into(),
            app_exe_name: app_exe_name.into(),
            runner_name: DEFAULT_RUNNER_NAME.to_string(),
        }
    }

    /// Layout for a root with the running executable's name, so the restart
    /// command targets the same binary.
    pub fn from_current_exe(install_root: impl Into<PathBuf>) -> Self {
        let name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| DEFAULT_APP_EXE.to_string());
        Self::new(install_root, name)
    }

    pub fn with_runner_name(mut self, runner_name: impl Into<String>) -> Self {
        self.runner_name = runner_name.into();
        self
    }

    pub fn install_root(&self) -> &Path {
        &self.install_root
    }

    pub fn app_exe_name(&self) -> &str {
        &self.app_exe_name
    }

    pub fn runner_name(&self) -> &str {
        &self.runner_name
    }

    /// `<install_root>/app` — the active payload swapped on upgrade.
    pub fn payload_dir(&self) -> PathBuf {
        self.install_root.join(PAYLOAD_DIR)
    }

    /// `<install_root>/app.previous` — rollback copy.
    pub fn previous_dir(&self) -> PathBuf {
        self.install_root.join(PREVIOUS_DIR)
    }

    /// `<install_root>/<runner_name>` — external helper executable.
    pub fn runner_exe(&self) -> PathBuf {
        self.install_root.join(&self.runner_name)
    }

    /// The running application executable inside the active payload.
    pub fn app_exe(&self) -> PathBuf {
        self.payload_dir().join(&self.app_exe_name)
    }

    /// `<install_root>/.staging/app-<version>`.
    pub fn staging_dir(&self, version: &str) -> PathBuf {
        self.install_root
            .join(STAGING_DIR)
            .join(format!("app-{version}"))
    }

    /// Coordinator that hands the staged payload to the external runner.
    pub fn coordinator(&self) -> UpgradeCoordinator {
        UpgradeCoordinator::new(self.runner_exe(), self.install_root.clone())
    }

    /// Atomic replacement plan for an already-staged payload directory.
    pub fn replacement_plan(&self, version: &str, staged_payload: &Path) -> InstallPlan {
        InstallPlan::new(
            &self.install_root,
            self.payload_dir(),
            staged_payload.to_path_buf(),
            PREVIOUS_DIR,
            version.to_string(),
        )
    }

    /// The command the runner executes after the swap. Constructed only; the
    /// caller (an external process) is responsible for spawning it.
    pub fn restart_command(&self) -> AppRestartCommand {
        AppRestartCommand {
            program: self.app_exe(),
            args: Vec::new(),
            working_dir: self.payload_dir(),
        }
    }
}

/// The exit-replace-restart relaunch command (upstream `Utils.StartV2RayN`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRestartCommand {
    /// Application executable to launch.
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Working directory (the active payload, upstream `StartupPath`).
    pub working_dir: PathBuf,
}

/// A completed application replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUpgradeOutcome {
    pub version: String,
    pub app_exe: PathBuf,
    pub installed_dir: PathBuf,
    pub kept_previous: Option<PathBuf>,
    pub restart: AppRestartCommand,
    pub manifest: InstallManifest,
}

/// The staged payload must actually contain the running application
/// executable. The upstream WPF package ships `v2rayN.exe`; when the Flutter
/// build runs a differently named binary the package is rejected here, so a WPF
/// package can never be installed over the Flutter installation.
pub fn verify_payload(layout: &AppInstallLayout, staged_payload: &Path) -> Result<(), UpdateError> {
    if find_named(staged_payload, layout.app_exe_name()) {
        return Ok(());
    }
    Err(UpdateError::InstallConflict(format!(
        "application payload has no {} (refusing a mismatched package)",
        layout.app_exe_name()
    )))
}

fn find_named(dir: &Path, name: &str) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t)
                    if t.is_file()
                        && entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(name) =>
                {
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// Atomically replace `<install_root>/app` with `staged_payload`, keeping the
/// previous payload as `<install_root>/app.previous`. A pre-existing
/// `app.previous` is superseded (the immediate predecessor is the only rollback
/// target), matching [`crate::application`]'s core install semantics.
pub fn apply_app_upgrade(
    layout: &AppInstallLayout,
    version: &str,
    staged_payload: &Path,
) -> Result<AppUpgradeOutcome, UpdateError> {
    if !staged_payload.is_dir() {
        return Err(UpdateError::InstallConflict(format!(
            "staged application payload missing: {}",
            staged_payload.display()
        )));
    }
    verify_payload(layout, staged_payload)?;

    let previous = layout.previous_dir();
    if previous.exists() {
        std::fs::remove_dir_all(&previous).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    let plan = layout.replacement_plan(version, staged_payload);
    let outcome = apply_atomic(&plan)?;
    Ok(AppUpgradeOutcome {
        version: outcome.version,
        app_exe: layout.app_exe(),
        installed_dir: layout.payload_dir(),
        kept_previous: outcome.kept_previous,
        restart: layout.restart_command(),
        manifest: outcome.manifest,
    })
}

/// Restore `<install_root>/app.previous` over the active payload after a bad
/// replacement. Returns the restored active payload directory.
pub fn rollback_app_upgrade(layout: &AppInstallLayout) -> Result<PathBuf, UpdateError> {
    let current = layout.payload_dir();
    let previous = layout.previous_dir();
    if !previous.is_dir() {
        return Err(UpdateError::InstallConflict(
            "no previous application payload".into(),
        ));
    }
    let trash = layout.install_root.join(".app-old");
    if trash.exists() {
        std::fs::remove_dir_all(&trash).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    if current.exists() {
        std::fs::rename(&current, &trash).map_err(|e| UpdateError::Io(e.to_string()))?;
    }
    std::fs::rename(&previous, &current).map_err(|e| UpdateError::Io(e.to_string()))?;
    if trash.exists() {
        let _ = std::fs::remove_dir_all(&trash);
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("v2rayn-app-{tag}-{nanos}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn staged(root: &Path, tag: &str, exe: &str) -> PathBuf {
        let dir = root.join(".staging").join(format!("src-{tag}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(exe), format!("payload-{tag}")).unwrap();
        std::fs::write(dir.join("data.txt"), tag).unwrap();
        dir
    }

    #[test]
    fn replace_keeps_previous_and_restart_targets_payload() {
        let root = temp_root("replace");
        let layout = AppInstallLayout::new(&root, "v2rayn_desktop.exe");
        // Seed an active payload so the first apply keeps a rollback copy.
        std::fs::create_dir_all(layout.payload_dir()).unwrap();
        std::fs::write(layout.payload_dir().join("old.txt"), b"old").unwrap();

        let outcome = apply_app_upgrade(
            &layout,
            "7.99.0",
            &staged(&root, "v2", "v2rayn_desktop.exe"),
        )
        .unwrap();
        assert!(outcome.app_exe.is_file());
        assert!(layout.previous_dir().is_dir());
        assert_eq!(
            outcome.kept_previous.as_deref(),
            Some(layout.previous_dir().as_path())
        );
        assert_eq!(outcome.restart.program, layout.app_exe());
        assert_eq!(outcome.restart.working_dir, layout.payload_dir());

        let restored = rollback_app_upgrade(&layout).unwrap();
        assert_eq!(restored, layout.payload_dir());
        assert!(layout.payload_dir().join("old.txt").is_file());
    }

    #[test]
    fn second_replace_supersedes_previous() {
        let root = temp_root("second");
        let layout = AppInstallLayout::new(&root, "v2rayn_desktop.exe");
        let v1 = staged(&root, "one", "v2rayn_desktop.exe");
        apply_app_upgrade(&layout, "1.0.0", &v1).unwrap();
        let v2 = staged(&root, "two", "v2rayn_desktop.exe");
        let outcome = apply_app_upgrade(&layout, "1.1.0", &v2).unwrap();
        assert_eq!(outcome.version, "1.1.0");
        // The rollback copy is the immediately previous payload only.
        assert_eq!(
            std::fs::read_to_string(layout.previous_dir().join("data.txt")).unwrap(),
            "one"
        );
    }

    #[test]
    fn wpf_package_is_rejected_for_flutter_binary() {
        let root = temp_root("wpf");
        let layout = AppInstallLayout::new(&root, "v2rayn_desktop.exe");
        let wpf = staged(&root, "wpf", "v2rayN.exe");
        let error = apply_app_upgrade(&layout, "7.99.0", &wpf).unwrap_err();
        assert!(matches!(error, UpdateError::InstallConflict(_)));
        assert!(!layout.payload_dir().exists());
    }

    #[test]
    fn runner_spec_uses_install_root_and_helper() {
        let root = temp_root("runner");
        let layout = AppInstallLayout::new(&root, "v2rayn_desktop.exe");
        let source = staged(&root, "src", "v2rayn_desktop.exe");
        let spec = layout
            .coordinator()
            .external_upgrade_spec(&source, 4242)
            .unwrap();
        assert_eq!(spec.helper_exe, layout.runner_exe());
        assert_eq!(spec.install_root, root);
        assert_eq!(spec.wait_for_pid, 4242);
        assert!(!layout.runner_exe().exists(), "runner must not be created");
    }
}
