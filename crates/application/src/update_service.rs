//! T16 update use cases over the `updater` pipeline.
//!
//! Channels, version caps, metadata selection, download+verify, safe unpack
//! and atomic replacement all live in the `updater` crate; this module binds
//! them to the application layout (`cores/<name>/v<version>/`), the running
//! session's explicit proxy endpoint and the built-in target list. The app's
//! own update is coordinated as an [`updater::ExternalUpgradeSpec`]; no process
//! is ever spawned here.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use domain::{codes, CancellationToken, DomainError};
use updater::arch::{binary_matches, detect_target, HostTarget};
use updater::channel::{self, CoreSpec};
use updater::download::{DownloadRequest, DownloaderOptions, FileDownloader};
use updater::fetch::CoreReleaseApi;
use updater::install::{
    apply_atomic, ExternalUpgradeSpec, InstallManifest, InstallPlan, UpgradeCoordinator,
};
use updater::metadata::ReleasesClient;
use updater::semver::Semver;
use updater::unpack::{safe_unpack_targz, safe_unpack_zip, UnpackLimits};
use updater::UpdateError;

/// Maximum wall-clock for one metadata fetch or artifact download.
pub const UPDATE_TIMEOUT: Duration = Duration::from_secs(30);
/// Whole-stream download timeout (artifacts can be tens of MiB).
pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(180);
/// Hard cap for a single download (matches `DownloaderOptions::default`).
pub const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;
/// Version bookkeeping written next to a managed install.
pub const INSTALL_MANIFEST_NAME: &str = "install-manifest.json";

/// Production releases API base (GitHub).
pub const GITHUB_API_BASE: &str = "https://api.github.com/repos";

/// Test-only environment variable that overrides [`GITHUB_API_BASE`]. Only
/// honoured in debug builds; see [`test_api_base_override`].
pub const API_BASE_ENV: &str = "V2RAYN_R_UPDATE_API_BASE";

/// Test-only API base override for the releases endpoint.
///
/// This is compiled out of release builds (`debug_assertions` off) so a shipped
/// binary can never be redirected by an environment variable. Only `http`/
/// `https` URLs are accepted and the production GitHub base remains the
/// default. Digest/signature checks are unaffected.
pub fn test_api_base_override() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(API_BASE_ENV)
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| value.starts_with("http://") || value.starts_with("https://"))
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

/// Built-in update targets, in upstream order (`GetCheckUpdateCoreTypes`).
pub const BUILTIN_TARGETS: &[&str] = &["v2rayN", "xray", "mihomo", "sing_box"];

/// Every core the UI lists, including ones this build does not update. The
/// unsupported entries are shown disabled with a note (`is_check_update_supported`).
pub const UI_TARGETS: &[&str] = &[
    "v2rayN",
    "xray",
    "mihomo",
    "sing_box",
    "v2fly",
    "hysteria2",
    "tuic",
    "naiveproxy",
    "juicity",
    "brook",
    "overtls",
    "shadowquic",
    "mieru",
];

/// Map an updater failure onto the shared domain error contract.
pub fn update_error(err: UpdateError) -> DomainError {
    let code = match &err {
        UpdateError::Timeout => codes::TIMEOUT,
        UpdateError::Cancelled => codes::CANCELLED,
        UpdateError::VersionOutOfRange(_) | UpdateError::NotNewer { .. } => codes::FIELD_RANGE,
        UpdateError::NoRelease | UpdateError::NoMatchingAsset(_) => codes::NOT_FOUND,
        UpdateError::DigestMismatch { .. } => codes::CONFLICT,
        UpdateError::ArchMismatch { .. } => codes::FIELD_FORMAT,
        UpdateError::UnsafeArchivePath(_) | UpdateError::UnsafeArchive(_) => {
            codes::PERMISSION_DENIED
        }
        UpdateError::UnsafeTarget(_) | UpdateError::InstallConflict(_) => codes::CONFLICT,
        UpdateError::SignatureInvalid(_) => codes::PERMISSION_DENIED,
        UpdateError::SignatureUnsupported(_) => codes::UNAVAILABLE,
        _ => codes::UNAVAILABLE,
    };
    DomainError::new(code, "error.update_failed").with_detail(err.to_string())
}

/// Static description of one update target for the UI list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTargetInfo {
    pub core: String,
    pub repo: String,
    pub supported: bool,
    pub prerelease_capable: bool,
    pub max_version: Option<String>,
    pub note: Option<String>,
}

/// Every target the UI lists, plus an explicit note when it is disabled
/// (packaged installation or a core this build does not update).
pub fn builtin_targets(packaged: bool) -> Vec<UpdateTargetInfo> {
    UI_TARGETS
        .iter()
        .map(|core| target_info(core, channel::core_spec(core).as_ref(), packaged))
        .collect()
}

fn target_info(core: &str, spec: Option<&CoreSpec>, packaged: bool) -> UpdateTargetInfo {
    let supported = channel::is_check_update_supported(core, packaged);
    let note = if !supported {
        Some("error.update_unsupported".to_string())
    } else {
        None
    };
    UpdateTargetInfo {
        core: core.to_string(),
        repo: spec.map(|spec| spec.repo.to_string()).unwrap_or_default(),
        supported,
        prerelease_capable: spec.map(|spec| spec.prerelease_capable).unwrap_or(false),
        max_version: channel::max_allowed_version(core).map(|v| v.to_standard_string(None)),
        note,
    }
}

/// One target's check result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreUpdateCheck {
    pub core: String,
    pub supported: bool,
    pub note: Option<String>,
    pub installed_version: Option<String>,
    pub remote_version: Option<String>,
    pub has_update: bool,
    pub asset_name: Option<String>,
    pub download_url: Option<String>,
    pub expected_sha256: Option<String>,
    pub dgst_url: Option<String>,
}

/// Request to download+verify+install one core artifact.
#[derive(Debug, Clone)]
pub struct CoreApplyRequest {
    pub core: String,
    pub version: String,
    pub asset_name: String,
    pub download_url: String,
    pub expected_sha256: Option<String>,
    pub dgst_url: Option<String>,
    /// Explicit proxy endpoint (the running session's local port).
    pub proxy: Option<String>,
}

/// A completed atomic core replacement.
#[derive(Debug, Clone)]
pub struct CoreApplyOutcome {
    pub core: String,
    pub version: String,
    pub installed_dir: PathBuf,
    pub kept_previous: Option<PathBuf>,
}

/// An installed core directory discovered under `cores_root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCore {
    pub core: String,
    pub dir: String,
    pub version: String,
    pub executable: Option<String>,
}

/// Manual cleanup outcome.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanupReport {
    pub deleted: u32,
    pub bytes: u64,
    pub skipped: u32,
}

/// Application update service rooted at a cores directory.
#[derive(Debug, Clone)]
pub struct UpdateService {
    /// GitHub API base; overridden by loopback mocks in tests.
    pub api_base: String,
    pub target: HostTarget,
    pub cores_root: PathBuf,
    /// Root the external-upgrade coordinator is allowed to touch.
    pub install_root: PathBuf,
    pub packaged: bool,
    pub timeout: Duration,
}

impl UpdateService {
    /// Default target/API; `cores_root` is the managed cores directory.
    pub fn new(cores_root: impl Into<PathBuf>) -> Self {
        let cores_root = cores_root.into();
        let install_root = cores_root
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cores_root.clone());
        Self {
            api_base: test_api_base_override().unwrap_or_else(|| GITHUB_API_BASE.to_string()),
            target: detect_target().unwrap_or_else(default_target),
            cores_root,
            install_root,
            packaged: false,
            timeout: UPDATE_TIMEOUT,
        }
    }

    /// Explicit releases API base override (loopback mocks in tests).
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// The directory name a core is stored under (`tools/cores/<dir>/`).
    pub fn core_dir_name(core: &str) -> &str {
        match core {
            "sing_box" | "sing-box" => "singbox",
            other => other,
        }
    }

    /// Highest installed version for a core, if any.
    ///
    /// A managed install records its version in `install-manifest.json`; the
    /// `v*` sub-directory scan is a fallback for the dev `tools/cores/` layout.
    pub fn installed_version(&self, core: &str) -> Option<String> {
        let dir = self.cores_root.join(Self::core_dir_name(core));
        if let Ok(text) = std::fs::read_to_string(dir.join(INSTALL_MANIFEST_NAME)) {
            if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&text) {
                if let Some(version) = Semver::try_parse_strict(&manifest.version) {
                    return Some(version.to_standard_string(None));
                }
            }
        }
        let entries = std::fs::read_dir(&dir).ok()?;
        let mut best: Option<Semver> = None;
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(version) = Semver::try_parse_strict(&name) {
                best = Some(match best {
                    Some(current) if current >= version => current,
                    _ => version,
                });
            }
        }
        best.map(|v| v.to_standard_string(None))
    }

    /// Every installed core directory (read-only probe for the UI).
    pub fn installed_cores(&self) -> Vec<InstalledCore> {
        let mut out = Vec::new();
        for core in BUILTIN_TARGETS {
            if *core == "v2rayN" {
                continue;
            }
            let dir_name = Self::core_dir_name(core);
            let root = self.cores_root.join(dir_name);
            if let Ok(text) = std::fs::read_to_string(root.join(INSTALL_MANIFEST_NAME)) {
                if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&text) {
                    out.push(InstalledCore {
                        core: (*core).to_string(),
                        dir: dir_name.to_string(),
                        version: manifest.version.clone(),
                        executable: find_executable(&root)
                            .map(|p| p.to_string_lossy().into_owned()),
                    });
                }
            }
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(version) = Semver::try_parse_strict(&name) else {
                    continue;
                };
                out.push(InstalledCore {
                    core: (*core).to_string(),
                    dir: dir_name.to_string(),
                    version: version.to_standard_string(None),
                    executable: find_executable(&entry.path())
                        .map(|p| p.to_string_lossy().into_owned()),
                });
            }
        }
        out
    }

    /// Check one core against its release channel.
    pub async fn check_core(
        &self,
        core: &str,
        prerelease_requested: bool,
        proxy: Option<&str>,
    ) -> Result<CoreUpdateCheck, DomainError> {
        let installed = self.installed_version(core);
        let spec = match channel::core_spec(core) {
            Some(spec) => spec,
            None => {
                return Ok(CoreUpdateCheck {
                    core: core.to_string(),
                    supported: false,
                    note: Some("error.update_unsupported".to_string()),
                    installed_version: installed,
                    remote_version: None,
                    has_update: false,
                    asset_name: None,
                    download_url: None,
                    expected_sha256: None,
                    dgst_url: None,
                })
            }
        };
        if !channel::is_check_update_supported(core, self.packaged) {
            return Ok(CoreUpdateCheck {
                core: core.to_string(),
                supported: false,
                note: Some("error.update_unsupported".to_string()),
                installed_version: installed,
                remote_version: None,
                has_update: false,
                asset_name: None,
                download_url: None,
                expected_sha256: None,
                dgst_url: None,
            });
        }
        let prerelease = channel::check_pre_release(core, prerelease_requested);
        let client = ReleasesClient::new(spec.repo).with_api_base(self.api_base.clone());
        let api = CoreReleaseApi::new_with_proxy(self.timeout, proxy).map_err(update_error)?;
        let releases = api.fetch(&client).await.map_err(update_error)?;
        let release = match spec.locked_max_version {
            Some((major, minor, patch)) => {
                client.pick_with_limit(&releases, prerelease, &Semver::new(major, minor, patch))
            }
            None => client.pick(&releases, prerelease),
        }
        .map_err(update_error)?;
        let asset = release
            .select_for_target(core, self.target)
            .map_err(update_error)?;
        let dgst_url = release
            .assets
            .iter()
            .find(|a| a.name == format!("{}.dgst", asset.name))
            .map(|a| a.browser_download_url.clone())
            .filter(|url| !url.is_empty());
        let remote_semver = release.parsed_version();
        let installed_semver = installed.as_deref().and_then(Semver::try_parse_strict);
        let has_update = match (installed_semver, &remote_semver) {
            (Some(inst), Some(rem)) => *rem > inst,
            (None, Some(_)) => true,
            _ => false,
        };
        Ok(CoreUpdateCheck {
            core: core.to_string(),
            supported: true,
            note: None,
            installed_version: installed,
            remote_version: remote_semver.map(|v| v.to_standard_string(None)),
            has_update,
            asset_name: Some(asset.name.clone()),
            download_url: Some(asset.browser_download_url.clone()),
            expected_sha256: asset.sha256().map(|s| s.to_ascii_lowercase()),
            dgst_url,
        })
    }

    /// Download, verify, safely unpack and atomically install one core.
    pub async fn apply_core(
        &self,
        request: &CoreApplyRequest,
        cancellation: &CancellationToken,
    ) -> Result<CoreApplyOutcome, DomainError> {
        let staging = self.cores_root.join(".staging").join(format!(
            "{}-{}",
            Self::core_dir_name(&request.core),
            request.version
        ));
        reset_dir(&staging)?;
        let staged = self.stage_artifact(request, &staging, cancellation).await?;
        self.verify_arch(&staged)?;

        let core_dir = self.cores_root.join(Self::core_dir_name(&request.core));
        std::fs::create_dir_all(&self.cores_root)
            .map_err(|e| io_error("error.update_install", e))?;
        let keep_name = format!("{}.previous", Self::core_dir_name(&request.core));
        let plan = InstallPlan::new(
            &self.cores_root,
            &core_dir,
            &staged,
            keep_name,
            request.version.clone(),
        );
        let outcome = apply_atomic(&plan).map_err(update_error)?;
        let manifest_path = core_dir.join(INSTALL_MANIFEST_NAME);
        let manifest_bytes = serde_json::to_vec_pretty(&outcome.manifest)
            .map_err(|e| io_error("error.update_manifest", std::io::Error::other(e)))?;
        std::fs::write(&manifest_path, manifest_bytes)
            .map_err(|e| io_error("error.update_manifest", e))?;
        Ok(CoreApplyOutcome {
            core: request.core.clone(),
            version: outcome.version,
            installed_dir: core_dir,
            kept_previous: outcome.kept_previous,
        })
    }

    /// Build the external-upgrade spec for the application itself. The artifact
    /// is staged and verified, but no process is started.
    pub async fn app_update_spec(
        &self,
        request: &CoreApplyRequest,
        helper_exe: impl Into<PathBuf>,
        wait_for_pid: u32,
        cancellation: &CancellationToken,
    ) -> Result<ExternalUpgradeSpec, DomainError> {
        let staging = self
            .cores_root
            .join(".staging")
            .join(format!("app-{}", request.version));
        reset_dir(&staging)?;
        let staged = self.stage_artifact(request, &staging, cancellation).await?;
        let coordinator = UpgradeCoordinator::new(helper_exe.into(), self.install_root.clone());
        coordinator
            .external_upgrade_spec(staged, wait_for_pid)
            .map_err(update_error)
    }

    /// Restore the `<core>.previous` version kept by a prior install.
    pub fn rollback_core(&self, core: &str) -> Result<(), DomainError> {
        let dir_name = Self::core_dir_name(core);
        let current = self.cores_root.join(dir_name);
        let previous = self.cores_root.join(format!("{dir_name}.previous"));
        if !previous.is_dir() {
            return Err(DomainError::new(
                codes::NOT_FOUND,
                "error.update_no_rollback",
            ));
        }
        let trash = self.cores_root.join(format!(".{dir_name}.old"));
        if trash.exists() {
            std::fs::remove_dir_all(&trash).map_err(|e| io_error("error.update_rollback", e))?;
        }
        if current.exists() {
            std::fs::rename(&current, &trash).map_err(|e| io_error("error.update_rollback", e))?;
        }
        std::fs::rename(&previous, &current).map_err(|e| io_error("error.update_rollback", e))?;
        if trash.exists() {
            let _ = std::fs::remove_dir_all(&trash);
        }
        Ok(())
    }

    async fn stage_artifact(
        &self,
        request: &CoreApplyRequest,
        staging: &Path,
        cancellation: &CancellationToken,
    ) -> Result<PathBuf, DomainError> {
        let asset_path = staging.join(&request.asset_name);
        let options = DownloaderOptions {
            proxy: request.proxy.clone(),
            timeout: DOWNLOAD_TIMEOUT,
            max_bytes: MAX_DOWNLOAD_BYTES,
            ..DownloaderOptions::default()
        };
        let downloader = FileDownloader::new(options).map_err(update_error)?;
        let download_request =
            DownloadRequest::new(request.download_url.clone(), asset_path.clone());
        let downloaded = downloader
            .download(&download_request, cancellation)
            .await
            .map_err(update_error)?;

        let mut expected = request
            .expected_sha256
            .as_ref()
            .map(|s| s.to_ascii_lowercase());
        if let Some(dgst_url) = request.dgst_url.as_ref().filter(|u| !u.is_empty()) {
            if let Some(parsed) =
                download_dgst_sha256(&downloader, dgst_url, staging, cancellation).await
            {
                expected = Some(parsed);
            }
        }
        if let Some(expected) = expected {
            let actual = downloaded.sha256.to_ascii_lowercase();
            if actual != expected {
                return Err(update_error(UpdateError::DigestMismatch {
                    expected,
                    actual,
                }));
            }
        }

        let unpacked = staging.join("unpacked");
        let limits = UnpackLimits::default();
        let name = request.asset_name.to_ascii_lowercase();
        if name.ends_with(".zip") {
            safe_unpack_zip(&downloaded.path, &unpacked, limits).map_err(update_error)?;
        } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            safe_unpack_targz(&downloaded.path, &unpacked, limits).map_err(update_error)?;
        } else {
            return Err(update_error(UpdateError::UnsupportedArchive(
                request.asset_name.clone(),
            )));
        }
        let staged = flatten_single_dir(&unpacked);
        if !staged.is_dir() {
            return Err(update_error(UpdateError::UnsafeArchive(
                "archive produced no directory".into(),
            )));
        }
        Ok(staged)
    }

    fn verify_arch(&self, staged: &Path) -> Result<(), DomainError> {
        let Some(executable) = find_executable(staged) else {
            return Ok(());
        };
        let bytes = std::fs::read(&executable).map_err(|e| io_error("error.update_arch", e))?;
        match binary_matches(&bytes, self.target) {
            Ok(true) => Ok(()),
            Ok(false) => Err(update_error(UpdateError::ArchMismatch {
                expected: self.target.key(),
                found: "mismatch".to_string(),
            })),
            Err(_) => Ok(()),
        }
    }
}

fn default_target() -> HostTarget {
    use updater::arch::{Os, PlatformArch};
    if cfg!(windows) {
        HostTarget::new(Os::Windows, PlatformArch::X64)
    } else if cfg!(target_os = "macos") {
        HostTarget::new(Os::Macos, PlatformArch::X64)
    } else {
        HostTarget::new(Os::Linux, PlatformArch::X64)
    }
}

fn io_error(key: &str, err: std::io::Error) -> DomainError {
    DomainError::new(codes::INTERNAL, key).with_detail(err.to_string())
}

fn reset_dir(dir: &Path) -> Result<(), DomainError> {
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| io_error("error.update_staging", e))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| io_error("error.update_staging", e))
}

fn flatten_single_dir(unpacked: &Path) -> PathBuf {
    let Ok(entries) = std::fs::read_dir(unpacked) else {
        return unpacked.to_path_buf();
    };
    let entries: Vec<_> = entries.flatten().collect();
    if entries.len() == 1 {
        let only = &entries[0];
        if only.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            return only.path();
        }
    }
    unpacked.to_path_buf()
}

fn find_executable(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let file_type = entry.file_type().ok()?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                if name.ends_with(".exe") || (!name.contains('.') && !name.is_empty()) {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Parse `SHA2-256 = <hex>` / `SHA256=<hex>` out of a Xray-style `.dgst` file.
pub fn parse_dgst_sha256(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_uppercase().replace(['-', ' '], "");
        if key == "SHA2256" || key == "SHA256" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_ascii_lowercase());
            }
        }
    }
    None
}

async fn download_dgst_sha256(
    downloader: &FileDownloader,
    url: &str,
    staging: &Path,
    cancellation: &CancellationToken,
) -> Option<String> {
    let target = staging.join("artifact.dgst");
    let request = DownloadRequest::new(url.to_string(), target.clone());
    downloader.download(&request, cancellation).await.ok()?;
    let text = std::fs::read_to_string(&target).ok()?;
    parse_dgst_sha256(&text)
}

/// Manual trigger of the upstream scheduled cleanup (`TaskManager`):
/// `Test*` files older than `test_age_hours`, logs and temp/`.partial` files
/// older than `log_age_days`. `now` is Unix seconds, injected for tests.
pub fn cleanup_logs_tmp(
    root: &Path,
    now: i64,
    log_age_days: i64,
    test_age_hours: i64,
) -> Result<CleanupReport, DomainError> {
    if !root.is_dir() {
        return Ok(CleanupReport::default());
    }
    let mut report = CleanupReport::default();
    let log_age = log_age_days.max(0) * 86_400;
    let test_age = test_age_hours.max(0) * 3_600;
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() => files.push(path),
                _ => {}
            }
        }
    }
    for path in files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let in_log_dir = path.components().any(|c| {
            matches!(
                c.as_os_str()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .as_str(),
                "log" | "logs"
            )
        });
        let is_test = name.starts_with("test");
        let is_log = name.ends_with(".log") || in_log_dir;
        let is_temp =
            name.ends_with(".tmp") || name.ends_with(".partial") || name.ends_with("tmp-restore");
        if !(is_test || is_log || is_temp) {
            continue;
        }
        let age_seconds = modified_epoch(&path).map(|m| now - m).unwrap_or(0);
        let expired = if is_test {
            age_seconds >= test_age
        } else {
            age_seconds >= log_age
        };
        if !expired {
            report.skipped += 1;
            continue;
        }
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        match std::fs::remove_file(&path) {
            Ok(()) => {
                report.deleted += 1;
                report.bytes += size;
            }
            Err(_) => report.skipped += 1,
        }
    }
    Ok(report)
}

fn modified_epoch(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
        return Some(duration.as_secs() as i64);
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_with_api_base_wins() {
        let service =
            UpdateService::new(std::env::temp_dir()).with_api_base("http://127.0.0.1:11808/repos");
        assert_eq!(service.api_base, "http://127.0.0.1:11808/repos");
    }

    #[test]
    #[cfg(debug_assertions)]
    fn override_is_honoured_and_validated_in_debug() {
        // Held so no other test reads the env var while it is mutated.
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::var(API_BASE_ENV).ok();
        std::env::set_var(API_BASE_ENV, "http://127.0.0.1:19999/repos/");
        assert_eq!(
            test_api_base_override().as_deref(),
            Some("http://127.0.0.1:19999/repos")
        );
        std::env::set_var(API_BASE_ENV, "file:///etc/passwd");
        assert_eq!(test_api_base_override(), None);
        match previous {
            Some(value) => std::env::set_var(API_BASE_ENV, value),
            None => std::env::remove_var(API_BASE_ENV),
        }
    }
}
