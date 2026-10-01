//! Update channels, supported update targets and version-cap rules.
//!
//! Derived from upstream `ServiceLib/Manager/CoreInfoManager.cs` and
//! `ServiceLib/Global.cs` (`CoreUrls`) at commit `7d6a967`.

use serde::{Deserialize, Serialize};

use crate::error::UpdateError;
use crate::semver::Semver;

/// The release channel a check runs against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    Stable,
    Prerelease,
}

impl UpdateChannel {
    pub fn is_prerelease(self) -> bool {
        matches!(self, UpdateChannel::Prerelease)
    }
}

/// Upstream's `LockedMaxVersion = new SemanticVersion(1, 14, int.MaxValue)` for
/// sing-box.
pub const LOCKED_MAX_SING_BOX: (u64, u64, u64) = (1, 14, u32::MAX as u64);

/// `Global.CoreUrls` reduced to repository slugs.
pub const CORE_URLS: &[(&str, &str)] = &[
    ("v2fly", "v2fly/v2ray-core"),
    ("v2fly_v5", "v2fly/v2ray-core"),
    ("xray", "XTLS/Xray-core"),
    ("sing_box", "SagerNet/sing-box"),
    ("mihomo", "MetaCubeX/mihomo"),
    ("hysteria", "apernet/hysteria"),
    ("hysteria2", "apernet/hysteria"),
    ("naiveproxy", "klzgrad/naiveproxy"),
    ("tuic", "EAimTY/tuic"),
    ("juicity", "juicity/juicity"),
    ("brook", "txthinking/brook"),
    ("overtls", "ShadowsocksR-Live/overtls"),
    ("shadowquic", "spongebob888/shadowquic"),
    ("mieru", "enfein/mieru"),
    ("v2rayN", "2dust/v2rayN"),
];

/// Static release-target description for one core type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSpec {
    /// Upstream `ECoreType` name (`xray`, `sing_box`, ...).
    pub core: &'static str,
    /// Repository slug (`XTLS/Xray-core`).
    pub repo: &'static str,
    /// Optional locked maximum version.
    pub locked_max_version: Option<(u64, u64, u64)>,
    /// Whether a prerelease check is even possible for this target.
    pub prerelease_capable: bool,
}

/// Repository slug for a core type.
pub fn core_url_slug(core: &str) -> Option<&'static str> {
    CORE_URLS
        .iter()
        .find(|(key, _)| *key == core)
        .map(|(_, slug)| *slug)
}

/// The download asset URL template (`{0}` = tag, `{1}` = bare version) for a
/// core and OS/arch. Derived from `CoreInfoManager.cs` `DownloadUrl*` fields;
/// callers substitute the version.
pub fn download_template(core: &str, target: crate::arch::HostTarget) -> Option<&'static str> {
    use crate::arch::{Os, PlatformArch};
    let (os, arch) = (target.os, target.arch);
    let (base, tpl): (&str, &'static str) = match core {
        "xray" => (
            "https://github.com/XTLS/Xray-core/releases",
            match (os, arch) {
                (Os::Windows, PlatformArch::X64) => "/download/{0}/Xray-windows-64.zip",
                (Os::Windows, PlatformArch::Arm64) => "/download/{0}/Xray-windows-arm64-v8a.zip",
                (Os::Linux, PlatformArch::X64) => "/download/{0}/Xray-linux-64.zip",
                (Os::Linux, PlatformArch::Arm64) => "/download/{0}/Xray-linux-arm64-v8a.zip",
                (Os::Macos, PlatformArch::X64) => "/download/{0}/Xray-macos-64.zip",
                (Os::Macos, PlatformArch::Arm64) => "/download/{0}/Xray-macos-arm64-v8a.zip",
                _ => return None,
            },
        ),
        "sing_box" => (
            "https://github.com/SagerNet/sing-box/releases",
            match (os, arch) {
                (Os::Windows, PlatformArch::X64) => "/download/{0}/sing-box-{1}-windows-amd64.zip",
                (Os::Windows, PlatformArch::Arm64) => {
                    "/download/{0}/sing-box-{1}-windows-arm64.zip"
                }
                (Os::Linux, PlatformArch::X64) => "/download/{0}/sing-box-{1}-linux-amd64.tar.gz",
                (Os::Linux, PlatformArch::Arm64) => "/download/{0}/sing-box-{1}-linux-arm64.tar.gz",
                (Os::Macos, PlatformArch::X64) => "/download/{0}/sing-box-{1}-darwin-amd64.tar.gz",
                (Os::Macos, PlatformArch::Arm64) => {
                    "/download/{0}/sing-box-{1}-darwin-arm64.tar.gz"
                }
                _ => return None,
            },
        ),
        "mihomo" => (
            "https://github.com/MetaCubeX/mihomo/releases",
            match (os, arch) {
                (Os::Windows, PlatformArch::X64) => "/download/{0}/mihomo-windows-amd64-v1-{0}.zip",
                (Os::Windows, PlatformArch::Arm64) => "/download/{0}/mihomo-windows-arm64-{0}.zip",
                (Os::Linux, PlatformArch::X64) => "/download/{0}/mihomo-linux-amd64-v1-{0}.gz",
                (Os::Linux, PlatformArch::Arm64) => "/download/{0}/mihomo-linux-arm64-{0}.gz",
                (Os::Macos, PlatformArch::X64) => "/download/{0}/mihomo-darwin-amd64-v1-{0}.gz",
                (Os::Macos, PlatformArch::Arm64) => "/download/{0}/mihomo-darwin-arm64-{0}.gz",
                _ => return None,
            },
        ),
        "v2rayN" => (
            "https://github.com/2dust/v2rayN/releases",
            match (os, arch) {
                (Os::Windows, PlatformArch::X64) => "/download/{0}/v2rayN-windows-64.zip",
                (Os::Windows, PlatformArch::Arm64) => "/download/{0}/v2rayN-windows-arm64.zip",
                (Os::Linux, PlatformArch::X64) => "/download/{0}/v2rayN-linux-64.zip",
                (Os::Linux, PlatformArch::Arm64) => "/download/{0}/v2rayN-linux-arm64.zip",
                (Os::Macos, PlatformArch::X64) => "/download/{0}/v2rayN-macos-64.zip",
                (Os::Macos, PlatformArch::Arm64) => "/download/{0}/v2rayN-macos-arm64.zip",
                _ => return None,
            },
        ),
        _ => return None,
    };
    let _ = base;
    Some(tpl)
}

/// Static spec for a core, or `None` if it is not an updatable target.
pub fn core_spec(core: &str) -> Option<CoreSpec> {
    let repo = core_url_slug(core)?;
    let (locked_max, prerelease_capable) = match core {
        "sing_box" => (Some(LOCKED_MAX_SING_BOX), false),
        "v2rayN" | "xray" => (None, true),
        "mihomo" => (None, false),
        _ => (None, false),
    };
    Some(CoreSpec {
        core: match core {
            "sing_box" => "sing_box",
            "v2rayN" => "v2rayN",
            "xray" => "xray",
            "mihomo" => "mihomo",
            _ => return None,
        },
        repo,
        locked_max_version: locked_max,
        prerelease_capable,
    })
}

/// Only these built-in targets can be updated, and only when not installed as
/// a packaged app for `v2rayN`. Mirrors `IsCheckUpdateSupported` /
/// `GetCheckUpdateCoreTypes` (the x86 exclusion is applied by the caller with
/// the detected target).
pub fn is_check_update_supported(core: &str, packaged_install: bool) -> bool {
    match core {
        "v2rayN" => !packaged_install,
        "xray" | "mihomo" | "sing_box" => true,
        _ => false,
    }
}

/// Whether a prerelease check is meaningful for this core. Mirrors
/// `GetCheckPreRelease`: only `v2rayN` and `Xray` follow the user's prerelease
/// preference; the rest are always stable.
pub fn check_pre_release(core: &str, requested: bool) -> bool {
    match core {
        "v2rayN" | "xray" => requested,
        _ => false,
    }
}

/// The locked maximum version for a core, if any.
pub fn max_allowed_version(core: &str) -> Option<Semver> {
    core_spec(core)
        .and_then(|spec| spec.locked_max_version)
        .map(|(major, minor, patch)| Semver::new(major, minor, patch))
}

/// Reject a version that exceeds a core's locked maximum.
pub fn spec_version_in_range(core: &str, version: &Semver) -> Result<(), UpdateError> {
    match max_allowed_version(core) {
        Some(max) if version > &max => Err(UpdateError::VersionOutOfRange(version.to_string())),
        _ => Ok(()),
    }
}

/// Read the expected SHA-256 for an asset from a `cores.lock.json` document
/// (`cores[].asset == asset_name`).
pub fn read_lock_expected_sha256(lock_json: &str, asset_name: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(lock_json).ok()?;
    value
        .get("cores")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("asset").and_then(|a| a.as_str()) == Some(asset_name))
        .and_then(|entry| entry.get("sha256"))
        .and_then(|sha| sha.as_str())
        .map(|sha| sha.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{HostTarget, Os, PlatformArch};

    #[test]
    fn supported_targets_and_packaging() {
        assert!(is_check_update_supported("xray", false));
        assert!(is_check_update_supported("sing_box", false));
        assert!(is_check_update_supported("mihomo", false));
        assert!(is_check_update_supported("v2rayN", false));
        assert!(!is_check_update_supported("v2rayN", true));
        assert!(!is_check_update_supported("tuic", false));
        assert!(!is_check_update_supported("v2fly", false));
    }

    #[test]
    fn prerelease_only_for_v2rayn_and_xray() {
        assert!(check_pre_release("v2rayN", true));
        assert!(check_pre_release("xray", true));
        assert!(!check_pre_release("sing_box", true));
        assert!(!check_pre_release("mihomo", true));
        assert!(!check_pre_release("v2rayN", false));
    }

    #[test]
    fn singbox_cap_is_one_fourteen() {
        let max = max_allowed_version("sing_box").unwrap();
        assert_eq!(max.core_string(), "1.14.4294967295");
        assert!(spec_version_in_range("sing_box", &Semver::parse("1.14.2")).is_ok());
        assert!(spec_version_in_range("sing_box", &Semver::parse("1.14.99")).is_ok());
        assert!(spec_version_in_range("sing_box", &Semver::parse("1.15.0")).is_err());
        assert!(spec_version_in_range("sing_box", &Semver::parse("2.0.0")).is_err());
    }

    #[test]
    fn cores_without_cap_accept_anything() {
        assert!(max_allowed_version("xray").is_none());
        assert!(spec_version_in_range("xray", &Semver::parse("99.0.0")).is_ok());
    }

    #[test]
    fn url_slugs_match_upstream_coreurls() {
        assert_eq!(core_url_slug("xray"), Some("XTLS/Xray-core"));
        assert_eq!(core_url_slug("sing_box"), Some("SagerNet/sing-box"));
        assert_eq!(core_url_slug("mihomo"), Some("MetaCubeX/mihomo"));
        assert_eq!(core_url_slug("v2rayN"), Some("2dust/v2rayN"));
        assert_eq!(core_url_slug("nope"), None);
    }

    #[test]
    fn download_templates_use_expected_assets() {
        let win = HostTarget::new(Os::Windows, PlatformArch::X64);
        assert_eq!(
            download_template("xray", win),
            Some("/download/{0}/Xray-windows-64.zip")
        );
        assert_eq!(
            download_template("sing_box", win),
            Some("/download/{0}/sing-box-{1}-windows-amd64.zip")
        );
        let linux_arm = HostTarget::new(Os::Linux, PlatformArch::Arm64);
        assert_eq!(
            download_template("sing_box", linux_arm),
            Some("/download/{0}/sing-box-{1}-linux-arm64.tar.gz")
        );
        // Conflicting needles: mihomo windows template uses {0} twice.
        assert_eq!(
            download_template("mihomo", win)
                .unwrap()
                .matches("{0}")
                .count(),
            2
        );
    }

    #[test]
    fn reads_expected_sha256_from_lock() {
        let lock =
            r#"{"cores":[{"core":"xray","asset":"Xray-windows-64.zip","sha256":"D004C392"}]}"#;
        assert_eq!(
            read_lock_expected_sha256(lock, "Xray-windows-64.zip"),
            Some("d004c392".to_string())
        );
        assert_eq!(read_lock_expected_sha256(lock, "missing.zip"), None);
        assert_eq!(read_lock_expected_sha256("not json", "x"), None);
    }

    #[test]
    fn core_specs_carry_locked_max() {
        assert_eq!(
            core_spec("sing_box").unwrap().locked_max_version,
            Some(LOCKED_MAX_SING_BOX)
        );
        assert!(core_spec("xray").unwrap().locked_max_version.is_none());
        assert!(core_spec("v2rayN").unwrap().prerelease_capable);
        assert!(!core_spec("sing_box").unwrap().prerelease_capable);
    }
}
