//! GitHub releases metadata: the minimal model plus asset selection.
//!
//! Mirrors the subset of `ServiceLib/Models/Dto/GitHubRelease.cs` the updater
//! actually reads: `tag_name`, `name`, `prerelease`, `published_at` and the
//! assets' `name`, `size`, `browser_download_url` (plus the newer `digest`
//! field GitHub now emits, e.g. `sha256:<hex>`). Everything else is ignored.

use serde::{Deserialize, Serialize};

use crate::arch::HostTarget;
use crate::error::UpdateError;
use crate::semver::Semver;

/// One release asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub browser_download_url: String,
    /// GitHub's optional `sha256:<hex>` integrity digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl ReleaseAsset {
    /// The SHA-256 hex from `digest`, when it is a `sha256:` value.
    pub fn sha256(&self) -> Option<&str> {
        self.digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|hex| !hex.is_empty())
    }
}

/// One GitHub release, reduced to the fields the pipeline uses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    #[serde(default)]
    pub tag_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

impl ReleaseInfo {
    /// Parsed `tag_name` (never fails; falls back to `0.0.0`).
    pub fn version(&self) -> Semver {
        Semver::parse(&self.tag_name)
    }

    pub fn parsed_version(&self) -> Option<Semver> {
        Semver::try_parse_strict(&self.tag_name)
    }

    pub fn asset(&self, name: &str) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// Select the first asset whose name contains every `needle` fragment
    /// (case-insensitive), matching case-sensitively on the original name too.
    pub fn select_asset(&self, needles: &[&str]) -> Result<&ReleaseAsset, UpdateError> {
        let matched = self.assets.iter().find(|asset| {
            needles.iter().all(|needle| {
                asset
                    .name
                    .to_ascii_lowercase()
                    .contains(&needle.to_ascii_lowercase())
            })
        });
        matched.ok_or_else(|| UpdateError::NoMatchingAsset(needles.join("+")))
    }

    /// Select an asset for a specific `os`/`arch` using upstream's asset names.
    ///
    /// `core` is the [`crate::channel::CoreSpec`] slug (e.g. `xray`); the
    /// returned needles follow `CoreInfoManager.cs` templates.
    pub fn select_for_target(
        &self,
        core: &str,
        target: HostTarget,
    ) -> Result<&ReleaseAsset, UpdateError> {
        let needles = asset_needles(core, target);
        if needles.is_empty() {
            return Err(UpdateError::NoMatchingAsset(format!(
                "{core}:{}",
                target.key()
            )));
        }
        self.select_asset(&needles)
    }
}

/// Asset-name fragments for a core/target pair, derived from upstream
/// `CoreInfoManager.cs` `DownloadUrl*` templates (commit `7d6a967`).
pub fn asset_needles(core: &str, target: HostTarget) -> Vec<&'static str> {
    use crate::arch::{Os, PlatformArch};
    let (os, arch) = (target.os, target.arch);
    match core {
        "xray" => match os {
            Os::Windows => match arch {
                PlatformArch::X64 => vec!["Xray-windows-64.zip"],
                PlatformArch::Arm64 => vec!["Xray-windows-arm64-v8a.zip"],
                PlatformArch::X86 => vec!["Xray-windows-32.zip"],
            },
            Os::Linux => match arch {
                PlatformArch::X64 => vec!["Xray-linux-64.zip"],
                PlatformArch::Arm64 => vec!["Xray-linux-arm64-v8a.zip"],
                PlatformArch::X86 => vec!["Xray-linux-32.zip"],
            },
            Os::Macos => match arch {
                PlatformArch::X64 => vec!["Xray-macos-64.zip"],
                PlatformArch::Arm64 => vec!["Xray-macos-arm64-v8a.zip"],
                PlatformArch::X86 => vec!["Xray-macos-32.zip"],
            },
        },
        "sing-box" | "sing_box" => match os {
            Os::Windows => match arch {
                PlatformArch::X64 => vec!["windows-amd64.zip"],
                PlatformArch::Arm64 => vec!["windows-arm64.zip"],
                PlatformArch::X86 => vec!["windows-386.zip"],
            },
            Os::Linux => match arch {
                PlatformArch::X64 => vec!["linux-amd64.tar.gz"],
                PlatformArch::Arm64 => vec!["linux-arm64.tar.gz"],
                PlatformArch::X86 => vec!["linux-386.tar.gz"],
            },
            Os::Macos => match arch {
                PlatformArch::X64 => vec!["darwin-amd64.tar.gz"],
                PlatformArch::Arm64 => vec!["darwin-arm64.tar.gz"],
                PlatformArch::X86 => vec!["darwin-386.tar.gz"],
            },
        },
        "mihomo" => match os {
            Os::Windows => match arch {
                PlatformArch::X64 => vec!["mihomo-windows-amd64-v1", ".zip"],
                PlatformArch::Arm64 => vec!["mihomo-windows-arm64", ".zip"],
                PlatformArch::X86 => vec!["mihomo-windows-386", ".zip"],
            },
            Os::Linux => match arch {
                PlatformArch::X64 => vec!["mihomo-linux-amd64-v1", ".gz"],
                PlatformArch::Arm64 => vec!["mihomo-linux-arm64", ".gz"],
                PlatformArch::X86 => vec!["mihomo-linux-386", ".gz"],
            },
            Os::Macos => match arch {
                PlatformArch::X64 => vec!["mihomo-darwin-amd64-v1", ".gz"],
                PlatformArch::Arm64 => vec!["mihomo-darwin-arm64", ".gz"],
                PlatformArch::X86 => vec!["mihomo-darwin-amd64", ".gz"],
            },
        },
        "v2rayN" => match os {
            Os::Windows => match arch {
                PlatformArch::X64 => vec!["v2rayN-windows-64.zip"],
                PlatformArch::Arm64 => vec!["v2rayN-windows-arm64.zip"],
                PlatformArch::X86 => vec!["v2rayN-windows-32.zip"],
            },
            Os::Linux => match arch {
                PlatformArch::X64 => vec!["v2rayN-linux-64.zip"],
                PlatformArch::Arm64 => vec!["v2rayN-linux-arm64.zip"],
                PlatformArch::X86 => vec!["v2rayN-linux-32.zip"],
            },
            Os::Macos => match arch {
                PlatformArch::X64 => vec!["v2rayN-macos-64.zip"],
                PlatformArch::Arm64 => vec!["v2rayN-macos-arm64.zip"],
                PlatformArch::X86 => vec!["v2rayN-macos-32.zip"],
            },
        },
        _ => Vec::new(),
    }
}

/// Parse a JSON array of releases (GitHub `/releases` response).
pub fn parse_releases(json: &str) -> Result<Vec<ReleaseInfo>, UpdateError> {
    serde_json::from_str(json).map_err(|e| UpdateError::InvalidMetadata(e.to_string()))
}

/// A client bound to one repository's releases endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasesClient {
    pub repo: String,
    pub api_base: String,
}

impl ReleasesClient {
    /// Build from a `owner/name` slug and an API base (defaults to GitHub's).
    pub fn new(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            api_base: "https://api.github.com/repos".to_string(),
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    /// `GET {api_base}/{repo}/releases`.
    pub fn releases_url(&self) -> String {
        format!(
            "{}/{}/releases",
            self.api_base.trim_end_matches('/'),
            self.repo
        )
    }

    /// The release for a channel. Mirrors `UpdateService.GetRemoteVersion`:
    /// with `prerelease = true` GitHub's newest release is taken as-is (it may
    /// itself be a prerelease); otherwise the first non-prerelease entry wins.
    pub fn pick<'a>(
        &self,
        releases: &'a [ReleaseInfo],
        prerelease: bool,
    ) -> Result<&'a ReleaseInfo, UpdateError> {
        if prerelease {
            return releases.first().ok_or(UpdateError::NoRelease);
        }
        releases
            .iter()
            .find(|r| !r.prerelease)
            .ok_or(UpdateError::NoRelease)
    }

    /// Pick a release under a locked maximum version: prefer the channel head,
    /// then fall back to the highest release at or below `locked_max`.
    /// Mirrors the `LockedMaxVersion` branch in `GetRemoteVersion`.
    pub fn pick_with_limit<'a>(
        &self,
        releases: &'a [ReleaseInfo],
        prerelease: bool,
        locked_max: &Semver,
    ) -> Result<&'a ReleaseInfo, UpdateError> {
        let candidate = self.pick(releases, prerelease)?;
        let version = candidate
            .parsed_version()
            .ok_or_else(|| UpdateError::InvalidMetadata("bad tag".into()))?;
        if &version <= locked_max {
            return Ok(candidate);
        }
        releases
            .iter()
            .filter(|r| prerelease || !r.prerelease)
            .filter_map(|r| r.parsed_version().map(|v| (r, v)))
            .filter(|(_, v)| v <= locked_max)
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .map(|(r, _)| r)
            .ok_or(UpdateError::NoRelease)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::{Os, PlatformArch};

    const SAMPLE: &str = r#"[
      {"tag_name":"v26.3.27","name":"Xray 26.3.27","prerelease":false,"published_at":"2026-03-27T00:00:00Z",
       "assets":[
         {"name":"Xray-windows-64.zip","size":20913304,"browser_download_url":"https://example/x.zip","digest":"sha256:abc"},
         {"name":"Xray-windows-arm64-v8a.zip","size":1,"browser_download_url":"https://example/a.zip"},
         {"name":"Xray-linux-64.zip","size":2,"browser_download_url":"https://example/l.zip"}
       ]},
      {"tag_name":"v26.4.0-beta.2","name":"Xray beta","prerelease":true,"published_at":"2026-04-02T00:00:00Z",
       "assets":[{"name":"Xray-windows-64.zip","size":3,"browser_download_url":"https://example/beta.zip"}]}
    ]"#;

    #[test]
    fn parses_release_model() {
        let releases = parse_releases(SAMPLE).unwrap();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].tag_name, "v26.3.27");
        assert!(!releases[0].prerelease);
        assert_eq!(releases[0].assets.len(), 3);
        assert_eq!(
            releases[0].asset("Xray-windows-64.zip").unwrap().size,
            20913304
        );
        assert_eq!(releases[0].assets[0].sha256(), Some("abc"));
        assert_eq!(releases[0].assets[1].sha256(), None);
    }

    #[test]
    fn picks_stable_vs_prerelease() {
        let releases = parse_releases(SAMPLE).unwrap();
        let client = ReleasesClient::new("XTLS/Xray-core");
        // Stable channel skips prereleases and picks the first stable entry.
        assert_eq!(client.pick(&releases, false).unwrap().tag_name, "v26.3.27");
        // Prerelease channel takes the first entry as GitHub returns it
        // (newest first); this fixture lists stable first, so it is returned.
        assert_eq!(client.pick(&releases, true).unwrap().tag_name, "v26.3.27");
    }

    #[test]
    fn prerelease_channel_takes_newest_first() {
        // GitHub orders releases newest-first, so the beta leads.
        let releases: Vec<ReleaseInfo> = parse_releases(
            r#"[{"tag_name":"v26.4.0-beta.2","prerelease":true,"assets":[]},
                {"tag_name":"v26.3.27","prerelease":false,"assets":[]}]"#,
        )
        .unwrap();
        let client = ReleasesClient::new("XTLS/Xray-core");
        assert_eq!(
            client.pick(&releases, true).unwrap().tag_name,
            "v26.4.0-beta.2"
        );
        assert_eq!(client.pick(&releases, false).unwrap().tag_name, "v26.3.27");
    }

    #[test]
    fn missing_stable_only_prerelease_errors() {
        let releases = vec![ReleaseInfo {
            tag_name: "1.0.0-beta".into(),
            name: None,
            prerelease: true,
            published_at: None,
            assets: vec![],
        }];
        let client = ReleasesClient::new("a/b");
        assert_eq!(client.pick(&releases, false), Err(UpdateError::NoRelease));
    }

    #[test]
    fn selects_assets_per_target() {
        let releases = parse_releases(SAMPLE).unwrap();
        let stable = &releases[0];
        let win64 = stable
            .select_for_target("xray", HostTarget::new(Os::Windows, PlatformArch::X64))
            .unwrap();
        assert_eq!(win64.name, "Xray-windows-64.zip");
        let linux = stable
            .select_for_target("xray", HostTarget::new(Os::Linux, PlatformArch::X64))
            .unwrap();
        assert_eq!(linux.name, "Xray-linux-64.zip");
    }

    #[test]
    fn missing_asset_reports_fragments() {
        let releases = parse_releases(SAMPLE).unwrap();
        let err = releases[0]
            .select_for_target("xray", HostTarget::new(Os::Macos, PlatformArch::Arm64))
            .unwrap_err();
        assert!(matches!(err, UpdateError::NoMatchingAsset(_)));
    }

    #[test]
    fn locked_max_falls_back_to_highest_allowed() {
        let releases = parse_releases(SAMPLE).unwrap();
        let client = ReleasesClient::new("XTLS/Xray-core");
        let max = Semver::parse("26.3.99");
        let picked = client.pick_with_limit(&releases, false, &max).unwrap();
        assert_eq!(picked.tag_name, "v26.3.27");
    }

    #[test]
    fn locked_max_with_no_allowed_release_errors() {
        let releases = parse_releases(SAMPLE).unwrap();
        let client = ReleasesClient::new("XTLS/Xray-core");
        let max = Semver::parse("26.0.0");
        assert_eq!(
            client.pick_with_limit(&releases, false, &max),
            Err(UpdateError::NoRelease)
        );
    }

    #[test]
    fn releases_url_joins_base() {
        let client =
            ReleasesClient::new("SagerNet/sing-box").with_api_base("http://127.0.0.1:11808/repos/");
        assert_eq!(
            client.releases_url(),
            "http://127.0.0.1:11808/repos/SagerNet/sing-box/releases"
        );
    }
}
