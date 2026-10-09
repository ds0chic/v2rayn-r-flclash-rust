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
use runtime::CoreInstallLayout;
use updater::app_upgrade::{
    apply_app_upgrade, rollback_app_upgrade, AppInstallLayout, AppRestartCommand, AppUpgradeOutcome,
};
use updater::arch::{binary_matches, detect_target, HostTarget};
use updater::channel;
use updater::download::{DownloadRequest, DownloaderOptions, FileDownloader};
use updater::fetch::CoreReleaseApi;
use updater::geo_file_requests;

pub use updater::geo::GEO_FILES_TARGET;
use updater::install::{
    apply_atomic, ExternalUpgradeSpec, InstallManifest, InstallPlan, UpgradeCoordinator,
};
use updater::metadata::ReleasesClient;
use updater::semver::Semver;
use updater::tls::HttpsTrust;
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

/// Error detail used when a caller tries to install the application itself as a
/// core (`v2rayN` is not a runnable core and must never land under the cores
/// root; its own update is staged through the external-upgrade path).
pub const APP_NOT_CORE: &str = "v2rayN is an application target, not a core";

/// Production releases API base (GitHub).
pub const GITHUB_API_BASE: &str = "https://api.github.com/repos";

/// Test-only environment variable that overrides [`GITHUB_API_BASE`]. Only
/// honoured in debug builds; see [`test_api_base_override`].
pub const API_BASE_ENV: &str = "V2RAYN_R_UPDATE_API_BASE";

/// Test-only environment variable that configures the application's own release
/// repository slug (`owner/name`). Unset by default: the Flutter rebuild has no
/// real published release source yet, so a real application update reports
/// `error.update_app_source_unconfigured` instead of pretending to work. Only
/// honoured in debug builds; see [`test_app_repo_override`].
pub const APP_REPO_ENV: &str = "V2RAYN_R_APP_REPO";

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

/// Test-only application release source override (`owner/name`).
///
/// Compiled out of release builds so a shipped binary cannot be redirected by
/// an environment variable. Returns `None` when unset, which is the shipped
/// default (no fabricated endpoint).
pub fn test_app_repo_override() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var(APP_REPO_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

fn default_app_exe_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "v2rayN.exe".to_string()
            } else {
                "v2rayN".to_string()
            }
        })
}

/// Built-in auto-update targets: the upstream `CoreInfoManager`
/// check-update core types plus the `GeoFiles` row the check-update view
/// appends after the cores (`CheckUpdateViewModel.GetGeoFileCheckUpdateModel`).
/// These are the rows the check pipeline reports; every other proxy core is
/// manual (R4-21).
pub const BUILTIN_TARGETS: &[&str] = &["v2rayN", "xray", "mihomo", "sing_box", GEO_FILES_TARGET];

/// The 14 frozen proxy cores (R3-CORE-MATRIX) in the update pipeline's key
/// spelling, derived from the runtime adapter list so the UI matrix can never
/// silently omit one.
pub fn proxy_update_cores() -> Vec<&'static str> {
    domain::CoreType::PROXY_CORES
        .iter()
        .map(|core| runtime::adapter::update_core_key(*core))
        .collect()
}

/// Every row the update window lists: the application identity, all 14 proxy
/// cores, and the trailing `GeoFiles` row (upstream appends it after the
/// cores), regardless of whether they can be auto-updated.
pub fn ui_targets() -> Vec<&'static str> {
    let mut targets = vec!["v2rayN"];
    targets.extend(proxy_update_cores());
    targets.push(GEO_FILES_TARGET);
    targets
}

/// How a row's install/update path is exposed by the ordinary UI entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreEntryKind {
    /// In-app download+verify+install through the check-update pipeline.
    Auto,
    /// Upstream `CoreInfoManager` defines no `DownloadUrl*` for it, so there is
    /// no in-app download; the user places the binary under the managed cores
    /// directory by hand. The selection/runtime path still works once present.
    Manual,
    /// The ordinary entry is unavailable for a concrete reason (see the note).
    Blocked,
}

/// Classify one core row. `Auto` mirrors [`updater::channel::is_check_update_supported`];
/// any other proxy core with a frozen spec is `Manual`; anything else (an
/// unknown key, or a packaged-application self-update) is `Blocked`.
pub fn core_entry_kind(core: &str, packaged: bool) -> CoreEntryKind {
    if channel::is_check_update_supported(core, packaged) {
        CoreEntryKind::Auto
    } else if core != "v2rayN" && runtime::adapter::core_type_for_update_key(core).is_some() {
        CoreEntryKind::Manual
    } else {
        CoreEntryKind::Blocked
    }
}

/// The human-facing message key for a row's entry state (R4-21).
pub fn core_entry_note(kind: CoreEntryKind) -> Option<&'static str> {
    match kind {
        CoreEntryKind::Auto => None,
        CoreEntryKind::Manual => Some("error.update_manual"),
        CoreEntryKind::Blocked => Some("error.update_unsupported"),
    }
}

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
/// (manual install or packaged installation).
pub fn builtin_targets(packaged: bool) -> Vec<UpdateTargetInfo> {
    ui_targets()
        .into_iter()
        .map(|core| target_info(core, packaged))
        .collect()
}

fn target_info(core: &str, packaged: bool) -> UpdateTargetInfo {
    // The GeoFiles row has no release channel: it is always refreshable from
    // its URL template and never version-compared.
    if core == GEO_FILES_TARGET {
        return UpdateTargetInfo {
            core: GEO_FILES_TARGET.to_string(),
            repo: String::new(),
            supported: true,
            prerelease_capable: false,
            max_version: None,
            note: None,
        };
    }
    let kind = core_entry_kind(core, packaged);
    let supported = kind == CoreEntryKind::Auto;
    let note = core_entry_note(kind).map(str::to_string);
    let spec = channel::core_spec(core);
    UpdateTargetInfo {
        core: core.to_string(),
        // `core_spec` only describes the four auto targets, so the repository
        // slug comes from the full `Global.CoreUrls` table; a manual row still
        // points the user at the right release page.
        repo: channel::core_url_slug(core).unwrap_or_default().to_string(),
        supported,
        prerelease_capable: spec.map(|s| s.prerelease_capable).unwrap_or(false),
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
    /// Detached `.sig` asset URL (application targets only; cores ship
    /// sha256/`.dgst` instead).
    pub sig_url: Option<String>,
}

impl CoreUpdateCheck {
    /// A check without any remote release, version or asset.
    fn unsupported(core: &str, note: &str, installed_version: Option<String>) -> Self {
        Self {
            core: core.to_string(),
            supported: false,
            note: Some(note.to_string()),
            installed_version,
            remote_version: None,
            has_update: false,
            asset_name: None,
            download_url: None,
            expected_sha256: None,
            dgst_url: None,
            sig_url: None,
        }
    }
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

/// One landed Geo `.dat` file with its recorded content hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoFileOutcome {
    pub name: String,
    pub sha256: String,
}

/// Outcome of refreshing the managed Geo `.dat` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoApplyOutcome {
    pub target: String,
    pub bin_dir: PathBuf,
    pub files: Vec<GeoFileOutcome>,
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
    /// Explicit application install root the external-upgrade coordinator is
    /// allowed to touch (not `cores_root.parent()`; set by the bridge).
    pub install_root: PathBuf,
    /// Application release repository slug (`owner/name`); `None` means the
    /// Flutter rebuild has no published release source (real updates blocked).
    pub app_repo: Option<String>,
    /// File name of the running application executable.
    pub app_exe_name: String,
    /// File name of the external upgrade runner.
    pub runner_name: String,
    pub packaged: bool,
    pub timeout: Duration,
    /// HTTPS trust roots for metadata fetch and artifact downloads (SP-25
    /// `RootCertProvider` consumer). `System` is the shipped default; the
    /// settings layer switches this with [`Self::with_tls_trust`].
    pub tls_trust: HttpsTrust,
    /// Explicit Geo `.dat` source template (`{0}` = bare asset name); `None`
    /// means the upstream built-in. The settings layer sets this from
    /// [`crate::dns::effective_geo_source`] (the stored `GeoSourceUrl` with
    /// the upstream built-in fallback); tests point it at a loopback origin.
    pub geo_source: Option<String>,
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
            app_repo: test_app_repo_override(),
            app_exe_name: default_app_exe_name(),
            runner_name: updater::DEFAULT_RUNNER_NAME.to_string(),
            packaged: false,
            timeout: UPDATE_TIMEOUT,
            tls_trust: HttpsTrust::System,
            geo_source: None,
        }
    }

    /// Trust `trust` for release-metadata fetch and artifact/signature
    /// downloads (the `RootCertProvider` selection, frozen per service).
    pub fn with_tls_trust(mut self, trust: HttpsTrust) -> Self {
        self.tls_trust = trust;
        self
    }

    /// Explicit releases API base override (loopback mocks in tests).
    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// Explicit application install root (the directory holding the running
    /// executable); the bridge passes the current process directory.
    pub fn with_app_install_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.install_root = root.into();
        self
    }

    /// Override the expected application executable name.
    pub fn with_app_exe_name(mut self, name: impl Into<String>) -> Self {
        self.app_exe_name = name.into();
        self
    }

    /// Configure the application release repository (`owner/name`).
    pub fn with_app_repo(mut self, repo: impl Into<String>) -> Self {
        self.app_repo = Some(repo.into());
        self
    }

    /// Override the Geo `.dat` source template (`{0}` = bare asset name).
    pub fn with_geo_source(mut self, template: impl Into<String>) -> Self {
        self.geo_source = Some(template.into());
        self
    }

    /// Effective Geo `.dat` source template: the override when set and
    /// non-blank, else the upstream built-in (`UpdateService.GetGeoFilesRequest`
    /// falls back to `Global.GeoUrl` the same way).
    pub fn effective_geo_template(&self) -> String {
        self.geo_source
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| crate::dns::BUILTIN_GEO_URL.to_string())
    }

    /// Managed Geo `.dat` destination directory (`<data>/bin`, next to the
    /// managed cores root; upstream `Utils.GetBinPath`).
    pub fn geo_bin_dir(&self) -> PathBuf {
        self.cores_root
            .parent()
            .map(|parent| parent.join("bin"))
            .unwrap_or_else(|| PathBuf::from("bin"))
    }

    /// The `GeoFiles` update row. Geo files carry no release version, so the
    /// row never reports one: `remote_version` is always `None`, `has_update`
    /// means a refresh is available from the configured source, and apply
    /// recomputes the per-file URLs from the same template (no stale URLs
    /// cross the check/apply boundary).
    pub fn check_geo_files(&self) -> CoreUpdateCheck {
        CoreUpdateCheck {
            core: GEO_FILES_TARGET.to_string(),
            supported: true,
            note: None,
            installed_version: None,
            remote_version: None,
            has_update: true,
            asset_name: None,
            download_url: None,
            expected_sha256: None,
            dgst_url: None,
            sig_url: None,
        }
    }

    /// Refresh the managed Geo `.dat` files (`geoip.dat`, `geosite.dat`) from
    /// the effective source template into [`Self::geo_bin_dir`].
    ///
    /// Every file is staged under `<cores_root>/.staging` first and only
    /// renamed into place after all downloads succeed, so a failed pass leaves
    /// the previous files intact (upstream `DownloadGeoFiles` copies temp files
    /// over only on success). The returned hashes record the landed content
    /// (see also [`crate::dns::geo_asset_hashes`]).
    pub async fn apply_geo_files(
        &self,
        proxy: Option<&str>,
        cancellation: &CancellationToken,
    ) -> Result<GeoApplyOutcome, DomainError> {
        let template = self.effective_geo_template();
        let bin_dir = self.geo_bin_dir();
        let requests = geo_file_requests(&template, &bin_dir).map_err(update_error)?;
        std::fs::create_dir_all(&bin_dir).map_err(|e| io_error("error.update_install", e))?;
        let staging = self.cores_root.join(".staging").join("geo-pending");
        reset_dir(&staging)?;
        let options = DownloaderOptions {
            proxy: proxy.map(str::to_string),
            timeout: DOWNLOAD_TIMEOUT,
            max_bytes: MAX_DOWNLOAD_BYTES,
            ..DownloaderOptions::default()
        };
        let downloader = FileDownloader::new_with_trust(options, self.tls_trust.clone())
            .map_err(update_error)?;
        let mut staged = Vec::new();
        for request in &requests {
            let download =
                DownloadRequest::new(request.url.clone(), staging.join(&request.file_name));
            match downloader.download(&download, cancellation).await {
                Ok(done) => staged.push((request.clone(), done)),
                Err(error) => {
                    let _ = std::fs::remove_dir_all(&staging);
                    return Err(update_error(error));
                }
            }
        }
        for (request, done) in &staged {
            if std::fs::rename(&done.path, &request.target).is_err()
                && std::fs::copy(&done.path, &request.target).is_err()
            {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(DomainError::new(codes::INTERNAL, "error.update_install")
                    .with_detail(request.target.display().to_string()));
            }
        }
        let mut files = Vec::new();
        for request in &requests {
            let bytes =
                std::fs::read(&request.target).map_err(|e| io_error("error.update_install", e))?;
            files.push(GeoFileOutcome {
                name: request.file_name.clone(),
                sha256: updater::sha256_of(&bytes),
            });
        }
        let _ = std::fs::remove_dir_all(&staging);
        Ok(GeoApplyOutcome {
            target: GEO_FILES_TARGET.to_string(),
            bin_dir,
            files,
        })
    }

    /// The directory name a core is stored under (`cores/<dir>/<version>/`).
    ///
    /// Delegates to [`CoreInstallLayout`] so the updater and the runtime
    /// locator share one frozen mapping (`sing_box` is `singbox`).
    pub fn core_dir_name(core: &str) -> &str {
        CoreInstallLayout::dir_name_str(core)
    }

    /// Highest installed version for a core, if any.
    ///
    /// A managed install records its version in `install-manifest.json`; the
    /// `v*` sub-directory scan is a fallback for the dev `tools/cores/` layout.
    pub fn installed_version(&self, core: &str) -> Option<String> {
        let layout = CoreInstallLayout::new(&self.cores_root);
        let dir = layout.core_dir_str(core);
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
        let layout = CoreInstallLayout::new(&self.cores_root);
        let mut out = Vec::new();
        for core in proxy_update_cores() {
            let dir_name = Self::core_dir_name(core);
            let root = layout.core_dir_str(core);
            if let Ok(text) = std::fs::read_to_string(root.join(INSTALL_MANIFEST_NAME)) {
                if let Ok(manifest) = serde_json::from_str::<InstallManifest>(&text) {
                    out.push(InstalledCore {
                        core: core.to_string(),
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
                    core: core.to_string(),
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
        self.check_core_inner(core, None, prerelease_requested, proxy)
            .await
    }

    /// Check the application's own update against its configured release
    /// source.
    ///
    /// The Flutter rebuild has no real published release source yet; when none
    /// is configured this fails closed with `error.update_app_source_unconfigured`
    /// rather than reaching a fabricated endpoint.
    pub async fn check_app_update(
        &self,
        prerelease_requested: bool,
        proxy: Option<&str>,
    ) -> Result<CoreUpdateCheck, DomainError> {
        let repo = self.app_repo.clone().ok_or_else(|| {
            DomainError::new(codes::UNAVAILABLE, "error.update_app_source_unconfigured")
                .with_detail("no application release source is configured for this build")
        })?;
        self.check_core_inner("v2rayN", Some(&repo), prerelease_requested, proxy)
            .await
    }

    /// The releases repository a target is checked against. The application
    /// (`v2rayN`) uses the configured own release source; every core uses its
    /// frozen repository. `None` means no source is configured for that target
    /// (the shipped default for the application), so the caller must report a
    /// blocked check instead of falling back to the upstream v2rayN repo.
    pub fn release_repo_for(&self, core: &str) -> Option<String> {
        if core == "v2rayN" {
            return self.app_repo.clone();
        }
        channel::core_spec(core).map(|spec| spec.repo.to_string())
    }

    /// The blocked application check used when no own release source is
    /// configured (shipped default). It carries the locally installed version
    /// but never a fabricated remote release/asset, so the UI cannot present
    /// the upstream v2rayN release as an available self-update.
    pub fn app_source_unconfigured_check(&self) -> CoreUpdateCheck {
        CoreUpdateCheck::unsupported(
            "v2rayN",
            "error.update_app_source_unconfigured",
            self.installed_version("v2rayN"),
        )
    }

    async fn check_core_inner(
        &self,
        core: &str,
        repo_override: Option<&str>,
        prerelease_requested: bool,
        proxy: Option<&str>,
    ) -> Result<CoreUpdateCheck, DomainError> {
        // The GeoFiles row has no release channel; it is reported without any
        // network access and never carries a fabricated remote version.
        if core == GEO_FILES_TARGET {
            return Ok(self.check_geo_files());
        }
        let installed = self.installed_version(core);
        let spec = match channel::core_spec(core) {
            Some(spec) if channel::is_check_update_supported(core, self.packaged) => spec,
            // A runnable core with no update-channel spec (the 11 manual cores)
            // or no in-app download asset (R4-21) is reported as manual, never
            // as a fake update; a packaged self-update or an unknown key stays
            // explicitly unsupported.
            _ => {
                let note = core_entry_note(core_entry_kind(core, self.packaged))
                    .unwrap_or("error.update_unsupported");
                return Ok(CoreUpdateCheck::unsupported(core, note, installed));
            }
        };
        let prerelease = channel::check_pre_release(core, prerelease_requested);
        // The releases repository is resolved by the shared provider: an
        // explicit override wins, otherwise the application uses its own
        // configured source and every core its frozen repository. An
        // unconfigured application source fails closed instead of silently
        // querying the upstream v2rayN repo (R3-08).
        let repo = match repo_override {
            Some(repo) => repo.to_string(),
            None => self.release_repo_for(core).ok_or_else(|| {
                DomainError::new(codes::UNAVAILABLE, "error.update_app_source_unconfigured")
                    .with_detail("no application release source is configured for this build")
            })?,
        };
        let client = ReleasesClient::new(repo).with_api_base(self.api_base.clone());
        let api = CoreReleaseApi::new_with_tls(self.timeout, proxy, self.tls_trust.clone())
            .map_err(update_error)?;
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
        let asset_url = |suffix: &str| {
            release
                .assets
                .iter()
                .find(|a| a.name == format!("{}{}", asset.name, suffix))
                .map(|a| a.browser_download_url.clone())
                .filter(|url| !url.is_empty())
        };
        let dgst_url = asset_url(".dgst");
        let sig_url = asset_url(".sig");
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
            sig_url,
        })
    }

    /// Download, verify, safely unpack and atomically install one core.
    pub async fn apply_core(
        &self,
        request: &CoreApplyRequest,
        cancellation: &CancellationToken,
    ) -> Result<CoreApplyOutcome, DomainError> {
        if request.core == "v2rayN" {
            return Err(
                DomainError::new(codes::FIELD_FORMAT, "error.update_app_not_core")
                    .with_detail(APP_NOT_CORE),
            );
        }
        let staging = self.cores_root.join(".staging").join(format!(
            "{}-{}",
            Self::core_dir_name(&request.core),
            request.version
        ));
        reset_dir(&staging)?;
        let staged = self.stage_artifact(request, &staging, cancellation).await?;
        self.verify_arch(&staged)?;
        self.install_core_from_dir(&request.core, &request.version, &staged)
    }

    /// Install an already-unpacked core directory under
    /// `<cores_root>/<dir>/<version>/`, keeping the previous core directory for
    /// rollback.
    ///
    /// Shared by [`Self::apply_core`] and tests so the layout the update
    /// pipeline produces is exactly the one [`runtime::CoreLocator`] resolves.
    pub fn install_core_from_dir(
        &self,
        core: &str,
        version: &str,
        staged: &Path,
    ) -> Result<CoreApplyOutcome, DomainError> {
        if !staged.is_dir() {
            return Err(DomainError::new(codes::NOT_FOUND, "error.update_staging")
                .with_detail(staged.display().to_string()));
        }
        let dir_name = Self::core_dir_name(core);
        let core_dir = CoreInstallLayout::new(&self.cores_root).core_dir_str(core);
        std::fs::create_dir_all(&self.cores_root)
            .map_err(|e| io_error("error.update_install", e))?;

        // Wrap the unpacked tree in a version directory so several versions can
        // coexist and the runtime resolves `<dir>/<version>/<exe>`.
        let versioned_parent = self
            .cores_root
            .join(".staging")
            .join(format!(".install-{dir_name}-{version}"));
        reset_dir(&versioned_parent)?;
        let versioned = versioned_parent.join("versioned");
        copy_tree(staged, &versioned.join(version))?;

        let keep_name = format!("{dir_name}.previous");
        let keep_dir = self.cores_root.join(&keep_name);
        if keep_dir.exists() {
            std::fs::remove_dir_all(&keep_dir).map_err(|e| io_error("error.update_install", e))?;
        }
        let plan = InstallPlan::new(
            &self.cores_root,
            &core_dir,
            &versioned,
            keep_name,
            version.to_string(),
        );
        let outcome = match apply_atomic(&plan) {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&versioned_parent);
                return Err(update_error(error));
            }
        };
        let _ = std::fs::remove_dir_all(&versioned_parent);

        let manifest_path = core_dir.join(INSTALL_MANIFEST_NAME);
        let manifest_bytes = serde_json::to_vec_pretty(&outcome.manifest)
            .map_err(|e| io_error("error.update_manifest", std::io::Error::other(e)))?;
        std::fs::write(&manifest_path, manifest_bytes)
            .map_err(|e| io_error("error.update_manifest", e))?;
        Ok(CoreApplyOutcome {
            core: core.to_string(),
            version: outcome.version,
            installed_dir: core_dir,
            kept_previous: outcome.kept_previous,
        })
    }

    /// Build the external-upgrade spec for the application itself. The artifact
    /// is staged and digest-checked, but no process is started.
    pub async fn app_update_spec(
        &self,
        request: &CoreApplyRequest,
        helper_exe: impl Into<PathBuf>,
        wait_for_pid: u32,
        cancellation: &CancellationToken,
    ) -> Result<ExternalUpgradeSpec, DomainError> {
        self.app_update_spec_inner(request, None, None, helper_exe, wait_for_pid, cancellation)
            .await
    }

    /// Application self-update staging with the detached OpenPGP trust root
    /// enforced (RT-15). `signature_url` is the `.sig` asset published next to
    /// the app artifact: a missing signature fails closed, a wrong one reports
    /// `SignatureInvalid`. No helper process is spawned here.
    pub async fn app_update_spec_verified(
        &self,
        request: &CoreApplyRequest,
        signature_url: Option<&str>,
        verifier: &dyn updater::signature::SignatureVerifier,
        helper_exe: impl Into<PathBuf>,
        wait_for_pid: u32,
        cancellation: &CancellationToken,
    ) -> Result<ExternalUpgradeSpec, DomainError> {
        self.app_update_spec_inner(
            request,
            signature_url,
            Some(verifier),
            helper_exe,
            wait_for_pid,
            cancellation,
        )
        .await
    }

    async fn app_update_spec_inner(
        &self,
        request: &CoreApplyRequest,
        signature_url: Option<&str>,
        verifier: Option<&dyn updater::signature::SignatureVerifier>,
        helper_exe: impl Into<PathBuf>,
        wait_for_pid: u32,
        cancellation: &CancellationToken,
    ) -> Result<ExternalUpgradeSpec, DomainError> {
        // The staged payload must live inside the application install root so
        // the external runner's atomic swap can happen on the same volume.
        let staging = self.app_layout().staging_dir(&request.version);
        reset_dir(&staging)?;
        let staged = self.stage_artifact(request, &staging, cancellation).await?;

        if let Some(verifier) = verifier {
            let artifact = std::fs::read(staging.join(&request.asset_name))
                .map_err(|e| io_error("error.update_signature", e))?;
            let signature = match signature_url.filter(|url| !url.is_empty()) {
                Some(url) => Some(
                    download_signature(
                        url,
                        &staging,
                        &self.tls_trust,
                        request.proxy.as_deref(),
                        cancellation,
                    )
                    .await?,
                ),
                None => None,
            };
            enforce_detached_signature(verifier, &artifact, signature.as_deref())?;
        }

        let coordinator = UpgradeCoordinator::new(helper_exe.into(), self.install_root.clone());
        // RR-04: emit the real `upgrade_runner` command line. The plan is the
        // flat-overlay contract (payload laid over the install root exactly like
        // the RC ZIP / Inno install); writing it next to the staged payload lets
        // the external runner apply the same swap in place.
        let layout = self.app_layout();
        let plan = layout.replacement_plan(&request.version, &staged);
        let plan_dir = self.install_root.join(".staging");
        std::fs::create_dir_all(&plan_dir).map_err(|e| io_error("error.update_install", e))?;
        let plan_path = plan_dir.join(format!("upgrade-plan-{}.json", request.version));
        let result_path = plan_dir.join(format!("upgrade-result-{}.json", request.version));
        let plan_bytes = serde_json::to_vec_pretty(&plan)
            .map_err(|e| io_error("error.update_manifest", std::io::Error::other(e)))?;
        std::fs::write(&plan_path, plan_bytes).map_err(|e| io_error("error.update_manifest", e))?;
        let restart = layout.restart_command();
        coordinator
            .runner_spec(
                &staged,
                &plan_path,
                &result_path,
                Some(&restart.program),
                Some(&restart.working_dir),
                wait_for_pid,
            )
            .map_err(update_error)
    }

    /// Frozen application install layout (explicit install root + runner).
    pub fn app_layout(&self) -> AppInstallLayout {
        AppInstallLayout::new(&self.install_root, &self.app_exe_name)
            .with_runner_name(&self.runner_name)
    }

    /// Atomically install a staged application payload into the isolated
    /// install root, keeping `app.previous`. This is what the external runner
    /// (`v2rayN-upgrade.exe`) performs after the application exits; no process
    /// is spawned here.
    pub fn apply_app_upgrade(
        &self,
        spec: &ExternalUpgradeSpec,
        version: &str,
    ) -> Result<AppUpgradeOutcome, DomainError> {
        if spec.install_root != self.install_root {
            return Err(
                DomainError::new(codes::CONFLICT, "error.update_app_install_root").with_detail(
                    "external spec install root does not match the application layout",
                ),
            );
        }
        apply_app_upgrade(&self.app_layout(), version, &spec.source).map_err(update_error)
    }

    /// Restore the `app.previous` payload kept by the last replacement.
    pub fn rollback_app_upgrade(&self) -> Result<PathBuf, DomainError> {
        rollback_app_upgrade(&self.app_layout()).map_err(update_error)
    }

    /// The relaunch command the external runner builds after the swap.
    pub fn app_restart_command(&self) -> AppRestartCommand {
        self.app_layout().restart_command()
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
        let downloader = FileDownloader::new_with_trust(options, self.tls_trust.clone())
            .map_err(update_error)?;
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

/// The built-in OpenPGP verifier for application (`v2rayN`) release assets.
///
/// Fails closed (`UNAVAILABLE`) when neither the pure-Rust backend nor GnuPG
/// can enforce the bundled upstream key, so callers never stage an app update
/// on an unverifiable trust root.
pub fn app_signature_verifier(
) -> Result<Box<dyn updater::signature::SignatureVerifier>, DomainError> {
    updater::signature::v2rayn_app_verifier().map_err(update_error)
}

/// Enforce a detached signature over an already-downloaded artifact.
///
/// A missing signature fails closed; a wrong signature surfaces as
/// `SignatureInvalid`. Exposed so the download pipeline's enforcement can be
/// exercised without a network peer.
pub fn enforce_detached_signature(
    verifier: &dyn updater::signature::SignatureVerifier,
    artifact: &[u8],
    signature: Option<&[u8]>,
) -> Result<(), DomainError> {
    match signature {
        Some(signature) => verifier.verify(artifact, signature).map_err(update_error),
        None => Err(
            DomainError::new(codes::UNAVAILABLE, "error.update_signature_missing")
                .with_detail("no detached signature asset was published"),
        ),
    }
}

async fn download_signature(
    url: &str,
    staging: &Path,
    trust: &HttpsTrust,
    proxy: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, DomainError> {
    let options = DownloaderOptions {
        proxy: proxy.map(str::to_string),
        timeout: DOWNLOAD_TIMEOUT,
        max_bytes: 4 * 1024 * 1024,
        ..DownloaderOptions::default()
    };
    let downloader =
        FileDownloader::new_with_trust(options, trust.clone()).map_err(update_error)?;
    let target = staging.join("artifact.sig");
    let request = DownloadRequest::new(url.to_string(), target.clone());
    downloader
        .download(&request, cancellation)
        .await
        .map_err(update_error)?;
    std::fs::read(&target).map_err(|e| io_error("error.update_signature", e))
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), DomainError> {
    std::fs::create_dir_all(to).map_err(|e| io_error("error.update_install", e))?;
    for entry in std::fs::read_dir(from).map_err(|e| io_error("error.update_install", e))? {
        let entry = entry.map_err(|e| io_error("error.update_install", e))?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| io_error("error.update_install", e))?;
        if file_type.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| io_error("error.update_install", e))?;
        }
    }
    Ok(())
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

    fn write_stub(path: &std::path::Path, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn one_shot_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn install_layout_is_resolved_by_runtime_locator() {
        let root = std::env::temp_dir().join(format!("v2rayn-fix12-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cores = root.join("cores");
        let staged = root.join("staged");
        let exe = if cfg!(windows) {
            "sing-box.exe"
        } else {
            "sing-box"
        };
        write_stub(&staged.join(exe), b"stub");
        let service = UpdateService::new(&cores);
        let outcome = service
            .install_core_from_dir("sing_box", "1.14.2", &staged)
            .unwrap();
        assert!(outcome.installed_dir.ends_with("singbox"));

        // runtime resolves exactly the directory the updater just installed.
        let locator = runtime::CoreLocator::with_roots(vec![cores.clone()], None);
        let resolved = locator
            .resolve(domain::CoreType::SingBox, Some("1.14.2"))
            .unwrap();
        assert!(resolved.starts_with(&outcome.installed_dir), "{resolved:?}");
        assert!(resolved.ends_with(std::path::Path::new("1.14.2").join(exe)));

        // A newly installed version becomes the unpinned pick.
        std::fs::remove_dir_all(&staged).unwrap();
        write_stub(&staged.join(exe), b"stub2");
        service
            .install_core_from_dir("sing_box", "1.15.0", &staged)
            .unwrap();
        let newest = locator.resolve(domain::CoreType::SingBox, None).unwrap();
        assert!(newest.ends_with(std::path::Path::new("1.15.0").join(exe)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn apply_core_refuses_application_target() {
        let service = UpdateService::new(std::env::temp_dir().join("v2rayn-fix12-app"));
        let request = CoreApplyRequest {
            core: "v2rayN".to_string(),
            version: "1.0.0".to_string(),
            asset_name: "v2rayN-windows-64.zip".to_string(),
            download_url: "http://127.0.0.1:11808/v2rayN-windows-64.zip".to_string(),
            expected_sha256: None,
            dgst_url: None,
            proxy: None,
        };
        let token = CancellationToken::new();
        let error = one_shot_runtime()
            .block_on(service.apply_core(&request, &token))
            .unwrap_err();
        assert_eq!(error.message_key, "error.update_app_not_core");
    }

    #[test]
    fn detached_signature_enforcement_rejects_wrong_and_missing() {
        let artifact = b"v2rayN app payload";
        let digest = updater::sha256_of(artifact);
        let prefix = digest[..16].to_string();
        let verifier = updater::signature::PrefixHashVerifier {
            expected_prefix: prefix.clone(),
        };
        enforce_detached_signature(&verifier, artifact, Some(prefix.as_bytes())).unwrap();
        let wrong = enforce_detached_signature(&verifier, artifact, Some(b"deadbeef")).unwrap_err();
        assert_eq!(wrong.code, codes::PERMISSION_DENIED);
        let missing = enforce_detached_signature(&verifier, artifact, None).unwrap_err();
        assert_eq!(missing.message_key, "error.update_signature_missing");
    }

    #[test]
    fn app_and_core_checks_use_separate_repositories() {
        // The application is dispatched to its own configured release source;
        // every core keeps its frozen repository. This is the routing assertion
        // the normal "check updates" list must honour (R3-08); it needs no
        // network because it records the resolved repo per target.
        let service = UpdateService::new(std::env::temp_dir()).with_app_repo("example/v2rayn-r");
        assert_eq!(
            service.release_repo_for("v2rayN").as_deref(),
            Some("example/v2rayn-r")
        );
        assert_eq!(
            service.release_repo_for("xray").as_deref(),
            Some("XTLS/Xray-core")
        );
        assert_eq!(
            service.release_repo_for("sing_box").as_deref(),
            Some("SagerNet/sing-box")
        );

        // A build without a published self-release source must not fall back to
        // the upstream `2dust/v2rayN` repo for the application target.
        let unconfigured = UpdateService::new(std::env::temp_dir());
        assert_eq!(unconfigured.release_repo_for("v2rayN"), None);
        assert_eq!(
            unconfigured.release_repo_for("xray").as_deref(),
            Some("XTLS/Xray-core")
        );
    }

    #[test]
    fn app_source_unconfigured_check_is_blocked_without_fake_release() {
        let service = UpdateService::new(std::env::temp_dir());
        let check = service.app_source_unconfigured_check();
        assert_eq!(check.core, "v2rayN");
        assert!(!check.supported);
        assert_eq!(
            check.note.as_deref(),
            Some("error.update_app_source_unconfigured")
        );
        // No upstream release info may be presented as an available update.
        assert!(check.remote_version.is_none());
        assert!(check.asset_name.is_none());
        assert!(check.download_url.is_none());
        assert!(!check.has_update);
    }

    #[test]
    fn entry_matrix_lists_every_frozen_proxy_core() {
        let targets = builtin_targets(false);
        let cores: Vec<&str> = targets.iter().map(|t| t.core.as_str()).collect();
        // no silent omission: all 14 proxy cores plus the application row plus
        // the trailing GeoFiles row (upstream appends it after the cores).
        assert_eq!(cores.len(), 16, "{cores:?}");
        for expected in [
            "v2fly",
            "v2fly_v5",
            "xray",
            "sing_box",
            "mihomo",
            "hysteria",
            "naiveproxy",
            "tuic",
            "juicity",
            "hysteria2",
            "brook",
            "overtls",
            "shadowquic",
            "mieru",
        ] {
            assert!(cores.contains(&expected), "matrix omitted {expected}");
        }
        assert!(cores.contains(&"v2rayN"));
        // The GeoFiles row trails the cores and is always refreshable.
        assert_eq!(cores.last(), Some(&GEO_FILES_TARGET));
        let geo = targets.iter().find(|t| t.core == GEO_FILES_TARGET).unwrap();
        assert!(geo.supported);
        assert!(geo.note.is_none());

        let auto: Vec<&str> = targets
            .iter()
            .filter(|t| t.supported)
            .map(|t| t.core.as_str())
            .collect();
        assert_eq!(
            auto,
            vec!["v2rayN", "xray", "mihomo", "sing_box", GEO_FILES_TARGET]
        );
        // Every manual row carries the manual note, never "up to date".
        for core in ["v2fly_v5", "hysteria", "tuic", "mieru"] {
            let row = targets.iter().find(|t| t.core == core).unwrap();
            assert!(!row.supported, "{core} unexpectedly auto");
            assert_eq!(row.note.as_deref(), Some("error.update_manual"));
        }
    }

    #[test]
    fn every_manual_core_has_a_runtime_adapter() {
        for core in proxy_update_cores() {
            let kind = core_entry_kind(core, false);
            assert_ne!(kind, CoreEntryKind::Blocked, "{core} is blocked");
            let core_type = runtime::adapter::core_type_for_update_key(core)
                .unwrap_or_else(|| panic!("no adapter authority for {core}"));
            assert!(
                runtime::adapter::adapter_for(core_type).is_some(),
                "no adapter for {core}"
            );
        }
    }

    #[test]
    fn check_core_labels_a_manual_core_without_network() {
        let service = UpdateService::new(std::env::temp_dir());
        let check = one_shot_runtime()
            .block_on(service.check_core("hysteria", false, None))
            .unwrap();
        assert_eq!(check.core, "hysteria");
        assert!(!check.supported);
        assert_eq!(check.note.as_deref(), Some("error.update_manual"));
        assert!(check.download_url.is_none());
        assert!(!check.has_update);
    }
}
