//! Update pipeline for v2rayN → Flutter + Rust (T16).
//!
//! The crate is modelled on the frozen upstream (commit `7d6a967`) sources:
//! `ServiceLib/Services/UpdateService.cs`, `ServiceLib/Manager/CoreInfoManager.cs`,
//! `ServiceLib/Global.cs` (`CoreUrls`) and `AmazTool/UpgradeApp.cs`.
//!
//! Pipeline: check channel → fetch metadata → stage download → verify
//! (digest / signature hook / platform arch) → safe unpack → atomic replace or
//! controlled external updater → keep a rollback version. Nothing here ever
//! rewrites a live installation: [`install::apply_atomic`] only renames
//! within a caller-owned root, and the external-upgrade coordinator returns a
//! spec instead of spawning a process.
//!
//! Evidence: `docs/evidence/T16-updater.md`; divergences:
//! `docs/decisions/T16-updater.md`.

#![forbid(unsafe_code)]

pub mod app_upgrade;
pub mod arch;
pub mod channel;
pub mod dgst;
pub mod download;
pub mod error;
pub mod fetch;
pub mod install;
pub mod metadata;
pub mod semver;
pub mod signature;
pub mod unpack;

pub use app_upgrade::{
    apply_app_upgrade, rollback_app_upgrade, verify_payload, AppInstallLayout, AppRestartCommand,
    AppUpgradeOutcome, DEFAULT_APP_EXE, DEFAULT_RUNNER_NAME, PAYLOAD_DIR, PREVIOUS_DIR,
    STAGING_DIR,
};
pub use arch::{
    binary_matches, detect_target, parse_binary_arch, parse_triple, BinaryArch, BinaryFormat,
    HostTarget, Os, PlatformArch,
};
pub use channel::{
    check_pre_release, core_spec, core_url_slug, download_template, is_check_update_supported,
    max_allowed_version, read_lock_expected_sha256, spec_version_in_range, CoreSpec, CORE_URLS,
    LOCKED_MAX_SING_BOX,
};
pub use dgst::{parse_dgst_sha256, sha256_file_sync, verify_dgst_file};
pub use download::{
    sha256_file, sha256_of, DownloadRequest, DownloadedFile, DownloaderOptions, FileDownloader,
};
pub use error::UpdateError;
pub use fetch::{CoreReleaseApi, ReleaseSource};
pub use install::{
    apply_atomic, external_upgrade_spec, restore_previous, verify_manifest, ApplyOutcome,
    FailPoint, InstallManifest, InstallPlan, InstalledEntry, UpgradeCoordinator,
};
pub use metadata::{parse_releases, ReleaseAsset, ReleaseInfo, ReleasesClient};
pub use semver::Semver;
pub use signature::{
    v2rayn_app_verifier, verify_app_release_asset, GpgCliVerifier, PgpDetachedVerifier,
    PrefixHashVerifier, SignatureVerifier, UnsupportedSignatureVerifier, V2RAYN_PUBLIC_KEY_ASC,
};
pub use unpack::{safe_join, safe_unpack_targz, safe_unpack_zip, UnpackLimits, Unpacked};

/// Re-exported for callers and tests that drive cancellable downloads.
pub use domain::CancellationToken;
