//! Frozen core install layout shared by the update pipeline and the runtime
//! locator (RT-04).
//!
//! Upstream resolves a core executable through
//! `Utils.GetBinPath(Utils.GetExeName(name), coreType.ToString())` and
//! `CoreInfoManager.GetCoreExecFile`, i.e. `<startup>/bin/<coreLower>/<exe>`.
//! This project keeps a versioned `<cores_root>/<dir>/<version>/<exe>` layout
//! so several versions can coexist and the runtime always starts the version
//! the updater most recently installed.
//!
//! Both `application::UpdateService` (which drives `updater`) and
//! `runtime::CoreLocator` build every path through this type so `sing-box` and
//! `singbox` can never point at different directories again.

use std::path::{Path, PathBuf};

use domain::CoreType;

/// The single source of truth for core directory / version / executable names.
#[derive(Debug, Clone)]
pub struct CoreInstallLayout {
    root: PathBuf,
}

impl CoreInstallLayout {
    /// `root` is the managed cores directory (`<data>/cores`).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Directory name of a core under the cores root (`xray`, `singbox`, ...).
    ///
    /// `sing_box` is stored as `singbox` (the project's fixture layout) so the
    /// updater and the runtime cannot diverge on hyphenation.
    pub fn dir_name(core: CoreType) -> &'static str {
        match core {
            CoreType::Xray => "xray",
            CoreType::SingBox => "singbox",
            CoreType::Mihomo => "mihomo",
            CoreType::V2fly | CoreType::V2flyV5 => "v2fly",
            CoreType::Hysteria2 | CoreType::Hysteria => "hysteria",
            _ => "cores",
        }
    }

    /// String form used by the update targets (`sing_box` → `singbox`).
    pub fn dir_name_str(core: &str) -> &str {
        match core {
            "sing_box" | "sing-box" => "singbox",
            "v2fly_v5" => "v2fly",
            "hysteria2" => "hysteria",
            other => other,
        }
    }

    /// Executable file name (platform aware) for a supported core.
    pub fn exe_name(core: CoreType) -> Option<&'static str> {
        match core {
            CoreType::Xray => Some(if cfg!(windows) { "xray.exe" } else { "xray" }),
            CoreType::SingBox => Some(if cfg!(windows) {
                "sing-box.exe"
            } else {
                "sing-box"
            }),
            _ => None,
        }
    }

    /// String form of [`Self::exe_name`].
    pub fn exe_name_str(core: &str) -> Option<&'static str> {
        match core {
            "xray" => Some(if cfg!(windows) { "xray.exe" } else { "xray" }),
            "sing_box" | "sing-box" => Some(if cfg!(windows) {
                "sing-box.exe"
            } else {
                "sing-box"
            }),
            _ => None,
        }
    }

    /// `<root>/<dir>`.
    pub fn core_dir(&self, core: CoreType) -> PathBuf {
        self.root.join(Self::dir_name(core))
    }

    /// `<root>/<dir>` for the string core id used by the update targets.
    pub fn core_dir_str(&self, core: &str) -> PathBuf {
        self.root.join(Self::dir_name_str(core))
    }

    /// `<root>/<dir>/<version>`.
    pub fn version_dir(&self, core: CoreType, version: &str) -> PathBuf {
        self.core_dir(core).join(version)
    }

    /// `<root>/<dir>/<version>` for the string core id.
    pub fn version_dir_str(&self, core: &str, version: &str) -> PathBuf {
        self.core_dir_str(core).join(version)
    }

    /// Name of the version manifest written inside a core directory.
    pub const MANIFEST_NAME: &'static str = "install-manifest.json";

    /// Highest installed version directory (semver-ordered), if any.
    pub fn latest_version(&self, core: CoreType) -> Option<String> {
        latest_version_in(&self.core_dir(core))
    }

    /// Highest installed version for the string core id.
    pub fn latest_version_str(&self, core: &str) -> Option<String> {
        latest_version_in(&self.core_dir_str(core))
    }

    /// Resolve `<root>/<dir>/<version>/<exe>` when it exists.
    pub fn resolve_exe(&self, core: CoreType, version: Option<&str>) -> Option<PathBuf> {
        let exe = Self::exe_name(core)?;
        let base = self.core_dir(core);
        let candidate = match version.map(str::trim).filter(|v| !v.is_empty()) {
            Some(version) => base.join(version).join(exe),
            None => base.join(self.latest_version(core)?).join(exe),
        };
        candidate.is_file().then_some(candidate)
    }

    /// Resolve the highest installed version dir for the string core id.
    pub fn resolve_exe_str(&self, core: &str, version: Option<&str>) -> Option<PathBuf> {
        let exe = Self::exe_name_str(core)?;
        let base = self.core_dir_str(core);
        let candidate = match version.map(str::trim).filter(|v| !v.is_empty()) {
            Some(version) => base.join(version).join(exe),
            None => base.join(self.latest_version_str(core)?).join(exe),
        };
        candidate.is_file().then_some(candidate)
    }
}

/// Numeric version key for a directory name (`v1.14.2` / `1.14.2` → `[1,14,2]`).
///
/// Returns `None` for names that are not version directories (`install.json`,
/// `1.2.3.previous`, `.staging`), so callers can ignore them.
pub fn version_key(value: &str) -> Option<Vec<u64>> {
    let stripped = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value);
    if stripped.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for part in stripped.split('.') {
        if part.is_empty() {
            return None;
        }
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        out.push(digits.parse().ok()?);
    }
    Some(out)
}

fn latest_version_in(base: &Path) -> Option<String> {
    let entries = std::fs::read_dir(base).ok()?;
    let mut best: Option<(Vec<u64>, String)> = None;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(key) = version_key(&name) else {
            continue;
        };
        let replace = match &best {
            Some((best_key, best_name)) => {
                (key.clone(), name.clone()) > (best_key.clone(), best_name.clone())
            }
            None => true,
        };
        if replace {
            best = Some((key, name));
        }
    }
    best.map(|(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("v2rayn-layout-{tag}-{nanos}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn singbox_dir_is_singbox_everywhere() {
        assert_eq!(CoreInstallLayout::dir_name(CoreType::SingBox), "singbox");
        assert_eq!(CoreInstallLayout::dir_name_str("sing_box"), "singbox");
        assert_eq!(CoreInstallLayout::dir_name_str("sing-box"), "singbox");
        assert_eq!(CoreInstallLayout::dir_name(CoreType::Xray), "xray");
    }

    #[test]
    fn version_key_orders_numerically_and_ignores_non_versions() {
        assert!(version_key("v1.14.2") > version_key("1.9.0"));
        assert_eq!(version_key("v1.14.2"), Some(vec![1, 14, 2]));
        assert_eq!(version_key("1.14.2"), Some(vec![1, 14, 2]));
        assert_eq!(version_key("1.14.2.previous"), None);
        assert_eq!(version_key(".staging"), None);
        assert_eq!(version_key("install-manifest.json"), None);
    }

    #[test]
    fn resolve_picks_pinned_and_highest() {
        let root = temp_root("resolve");
        let exe = if cfg!(windows) {
            "sing-box.exe"
        } else {
            "sing-box"
        };
        for version in ["v1.9.0", "v1.10.0", "v1.14.2"] {
            let dir = root.join("singbox").join(version);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(exe), b"stub").unwrap();
        }
        let layout = CoreInstallLayout::new(&root);
        assert_eq!(
            layout.latest_version(CoreType::SingBox).as_deref(),
            Some("v1.14.2")
        );
        assert!(layout
            .resolve_exe(CoreType::SingBox, None)
            .unwrap()
            .ends_with(Path::new("singbox").join("v1.14.2").join(exe)));
        assert!(layout
            .resolve_exe(CoreType::SingBox, Some("v1.9.0"))
            .unwrap()
            .ends_with(Path::new("singbox").join("v1.9.0").join(exe)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn version_dir_uses_shared_names() {
        let layout = CoreInstallLayout::new("/cores");
        assert!(layout
            .version_dir_str("sing_box", "1.14.2")
            .ends_with(Path::new("singbox").join("1.14.2")));
    }
}
