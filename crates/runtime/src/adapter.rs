//! Core adapters: executable location, command line, readiness and stop
//! strategy (plan §5, §13).
//!
//! A `CoreAdapter` knows how to turn a staged config file into a concrete
//! command. It never spawns anything itself; net-host owns process creation so
//! all lifecycle and Job-Object rules stay in one place.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use domain::{CoreType, DomainError};

/// Where a managed core executable was found.
pub struct CoreLocator {
    roots: Vec<PathBuf>,
    /// Explicit override (e.g. a dev build), checked first.
    xray_override: Option<PathBuf>,
}

impl CoreLocator {
    /// Build a locator from the environment.
    ///
    /// Search order: `V2RAYN_R_XRAY_BIN` (explicit), `V2RAYN_R_CORES_ROOT`,
    /// `%LOCALAPPDATA%\v2rayn-r\cores`, then the `tools/cores` directory of
    /// this repository (discovered by walking up from the current exe/cwd).
    pub fn from_env() -> Self {
        let mut roots = Vec::new();
        if let Some(root) = std::env::var_os("V2RAYN_R_CORES_ROOT") {
            roots.push(PathBuf::from(root));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(local).join("v2rayn-r").join("cores"));
        }
        roots.extend(ancestor_core_roots());
        Self {
            roots,
            xray_override: std::env::var_os("V2RAYN_R_XRAY_BIN").map(PathBuf::from),
        }
    }

    /// Build a locator with explicit roots (deterministic tests).
    pub fn with_roots(roots: Vec<PathBuf>, xray_override: Option<PathBuf>) -> Self {
        Self {
            roots,
            xray_override,
        }
    }

    /// Locate the executable for `core`, optionally pinned to `version`.
    pub fn resolve(&self, core: CoreType, version: Option<&str>) -> Result<PathBuf, DomainError> {
        let adapter = adapter_for(core).ok_or_else(|| {
            DomainError::new(domain::codes::NOT_FOUND, "error.core_not_supported")
                .with_detail(format!("no adapter for core {}", core.as_str()))
        })?;

        if core == CoreType::Xray {
            if let Some(path) = &self.xray_override {
                if path.is_file() {
                    return Ok(path.clone());
                }
            }
        }

        let dir = core_dir(core);
        let exe = adapter.exe_name();
        let pinned = version.map(|v| v.trim()).filter(|v| !v.is_empty());
        let mut searched = Vec::new();
        for root in &self.roots {
            let base = root.join(dir);
            if !base.is_dir() {
                continue;
            }
            if let Some(version) = pinned {
                let candidate = base.join(version).join(exe);
                if candidate.is_file() {
                    return Ok(candidate);
                }
                searched.push(candidate.display().to_string());
            } else {
                let mut versions: Vec<PathBuf> = std::fs::read_dir(&base)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                versions.sort();
                versions.reverse();
                for version_dir in versions {
                    let candidate = version_dir.join(exe);
                    if candidate.is_file() {
                        return Ok(candidate);
                    }
                }
                searched.push(base.display().to_string());
            }
        }

        Err(
            DomainError::new(domain::codes::NOT_FOUND, "error.core_not_found")
                .with_field("core")
                .with_detail(format!(
                    "{} {} not found; searched: {}",
                    core.as_str(),
                    pinned.unwrap_or("<any>"),
                    if searched.is_empty() {
                        "<no candidate roots>".to_string()
                    } else {
                        searched.join(", ")
                    }
                )),
        )
    }
}

/// Directory name of a core under a cores root (lower-case, hyphenated).
pub fn core_dir(core: CoreType) -> &'static str {
    match core {
        CoreType::Xray => "xray",
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
        CoreType::V2fly => "v2fly",
        CoreType::V2flyV5 => "v2fly",
        CoreType::Hysteria2 | CoreType::Hysteria => "hysteria",
        _ => "cores",
    }
}

fn ancestor_core_roots() -> Vec<PathBuf> {
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    let mut found = Vec::new();
    for start in starts {
        let mut dir: Option<&Path> = Some(start.as_path());
        let mut depth = 0;
        while let Some(current) = dir {
            let candidate = current.join("tools").join("cores");
            if candidate.is_dir() && !found.contains(&candidate) {
                found.push(candidate);
            }
            dir = current.parent();
            depth += 1;
            if depth > 8 {
                break;
            }
        }
    }
    found
}

/// How a core is invoked and observed.
pub trait CoreAdapter: Send + Sync {
    fn core_type(&self) -> CoreType;
    /// Executable file name including extension.
    fn exe_name(&self) -> &'static str;
    /// Arguments to run with a config file.
    fn run_args(&self, config: &Path) -> Vec<OsString>;
    /// Arguments for a config check that exits without binding anything.
    fn test_args(&self, config: &Path) -> Vec<OsString>;
    /// Arguments for a version probe.
    fn version_args(&self) -> Vec<OsString>;
}

/// Xray-core adapter (`xray run -c`, `xray run -test -c`, `xray version`).
pub struct XrayAdapter;

impl CoreAdapter for XrayAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Xray
    }

    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "xray.exe"
        } else {
            "xray"
        }
    }

    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["run".into(), "-c".into(), config.as_os_str().to_os_string()]
    }

    fn test_args(&self, config: &Path) -> Vec<OsString> {
        vec![
            "run".into(),
            "-test".into(),
            "-c".into(),
            config.as_os_str().to_os_string(),
        ]
    }

    fn version_args(&self) -> Vec<OsString> {
        vec!["version".into()]
    }
}

/// sing-box adapter.
pub struct SingBoxAdapter;

impl CoreAdapter for SingBoxAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::SingBox
    }

    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "sing-box.exe"
        } else {
            "sing-box"
        }
    }

    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["run".into(), "-c".into(), config.as_os_str().to_os_string()]
    }

    fn test_args(&self, config: &Path) -> Vec<OsString> {
        vec![
            "check".into(),
            "-c".into(),
            config.as_os_str().to_os_string(),
        ]
    }

    fn version_args(&self) -> Vec<OsString> {
        vec!["version".into()]
    }
}

/// Resolve the adapter for a core type.
pub fn adapter_for(core: CoreType) -> Option<Box<dyn CoreAdapter>> {
    match core {
        CoreType::Xray => Some(Box::new(XrayAdapter)),
        CoreType::SingBox => Some(Box::new(SingBoxAdapter)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xray_command_shape() {
        let adapter = XrayAdapter;
        let config = Path::new("C:/run/config.json");
        let run = adapter.run_args(config);
        assert_eq!(run[0], "run");
        assert_eq!(run[1], "-c");
        assert_eq!(run[2], "C:/run/config.json");
        let test = adapter.test_args(config);
        assert!(test.iter().any(|a| a == "-test"));
        assert_eq!(adapter.version_args(), vec![OsString::from("version")]);
    }

    #[test]
    fn core_dir_is_lowercase() {
        assert_eq!(core_dir(CoreType::Xray), "xray");
        assert_eq!(core_dir(CoreType::SingBox), "sing-box");
    }

    #[test]
    fn unknown_core_has_no_adapter() {
        assert!(adapter_for(CoreType::Mihomo).is_none());
    }

    fn temp_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("v2rayn-t03-adapter-{tag}-{nanos}"));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn resolve_prefers_highest_version_then_pinned() {
        let root = temp_root("versions");
        let exe = if cfg!(windows) { "xray.exe" } else { "xray" };
        for version in ["v1.0.0", "v2.0.0"] {
            let dir = root.join("xray").join(version);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(exe), b"stub").unwrap();
        }
        let locator = CoreLocator::with_roots(vec![root.clone()], None);
        // Unpinned: highest sorted version wins.
        let picked = locator.resolve(CoreType::Xray, None).unwrap();
        assert!(picked.ends_with(Path::new("v2.0.0").join(exe)));
        // Pinned: exact version wins.
        let pinned = locator.resolve(CoreType::Xray, Some("v1.0.0")).unwrap();
        assert!(pinned.ends_with(Path::new("v1.0.0").join(exe)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_override_wins_and_missing_is_not_found() {
        let root = temp_root("override");
        let override_exe = root.join("custom-xray.exe");
        std::fs::write(&override_exe, b"stub").unwrap();
        let locator = CoreLocator::with_roots(vec![root.join("empty")], Some(override_exe.clone()));
        assert_eq!(locator.resolve(CoreType::Xray, None).unwrap(), override_exe);

        let missing = CoreLocator::with_roots(vec![root.join("empty")], None);
        let err = missing.resolve(CoreType::Xray, None).unwrap_err();
        assert_eq!(err.code, domain::codes::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&root);
    }
}
