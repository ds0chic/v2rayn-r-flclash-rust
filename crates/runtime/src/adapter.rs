//! Core adapters: executable location, command line, readiness and stop
//! strategy (plan §5, §13).
//!
//! A `CoreAdapter` knows how to turn a staged config file into a concrete
//! command. It never spawns anything itself; net-host owns process creation so
//! all lifecycle and Job-Object rules stay in one place.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use domain::{CoreType, DomainError};

use crate::install_layout::CoreInstallLayout;

/// Where a managed core executable was found.
pub struct CoreLocator {
    roots: Vec<PathBuf>,
    /// Explicit override (e.g. a dev build), checked first.
    xray_override: Option<PathBuf>,
}

impl CoreLocator {
    /// Build a locator from the environment.
    ///
    /// Search order: `V2RAYN_R_XRAY_BIN` (explicit per-exe override),
    /// `V2RAYN_R_CORES_ROOT` (the managed root the app's NetHostClient always
    /// forwards), otherwise the managed default `<data>/cores`
    /// (`V2RAYN_R_DATA_DIR` or `%LOCALAPPDATA%\v2rayn-r\data\cores`, matching
    /// `AppEngine::default_data_dir`). The repository `tools/cores` tree is
    /// appended *only* as a development fallback; when an explicit
    /// `V2RAYN_R_CORES_ROOT` is present it is the sole root so a development
    /// tree can never mask a production install.
    pub fn from_env() -> Self {
        let explicit = std::env::var_os("V2RAYN_R_CORES_ROOT")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let mut roots = Vec::new();
        match &explicit {
            Some(root) => roots.push(root.clone()),
            None => roots.push(default_managed_cores_root()),
        }
        // Development fallbacks (the repository `tools/cores` tree and the
        // `V2RAYN_R_XRAY_BIN` override) are honored only in an explicit
        // development mode. A packaged run must resolve the managed install
        // root and never silently pick up a checkout or a stray env binary
        // (plan §3.7).
        let dev = dev_mode_enabled();
        if explicit.is_none() && dev {
            roots.extend(ancestor_core_roots());
        }
        Self {
            roots,
            xray_override: dev
                .then(|| std::env::var_os("V2RAYN_R_XRAY_BIN"))
                .flatten()
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
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
                    return validate_core_exe(path).map(|()| path.clone());
                }
            }
        }

        let exe = adapter.exe_name();
        let pinned = version.map(|v| v.trim()).filter(|v| !v.is_empty());
        let mut searched = Vec::new();
        let mut invalid: Vec<String> = Vec::new();
        for root in &self.roots {
            let layout = CoreInstallLayout::new(root);
            let base = layout.core_dir(core);
            if !base.is_dir() {
                continue;
            }
            let mut candidates: Vec<PathBuf> = Vec::new();
            if let Some(version) = pinned {
                let candidate = layout.version_dir(core, version).join(exe);
                if candidate.is_file() {
                    candidates.push(candidate);
                } else {
                    searched.push(candidate.display().to_string());
                }
            } else if let Some(candidate) = layout.resolve_exe(core, None) {
                candidates.push(candidate);
            } else {
                searched.push(base.display().to_string());
            }
            // Candidate fallback (`CoreInfo.CoreExes`): try every alternate
            // name inside the pinned version dir or any core subdirectory.
            if let Some(candidate) = find_candidate_exe(&layout, core, pinned, &*adapter) {
                if !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
            }
            // A located file that cannot possibly run (empty / corrupt) is not
            // a usable core: keep searching the other roots, but if nothing
            // valid turns up report the readable `error.core_invalid` so the UI
            // routes the user to the install/update entry.
            for candidate in candidates {
                match validate_core_exe(&candidate) {
                    Ok(()) => return Ok(candidate),
                    Err(error) => invalid.push(
                        error
                            .detail
                            .clone()
                            .unwrap_or_else(|| candidate.display().to_string()),
                    ),
                }
            }
        }

        if !invalid.is_empty() {
            return Err(
                DomainError::new(domain::codes::NOT_FOUND, "error.core_invalid")
                    .with_field("core")
                    .with_detail(format!(
                        "{} executable is present but not runnable: {}",
                        core.as_str(),
                        invalid.join(", ")
                    )),
            );
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

/// Directory name of a core under a cores root.
///
/// Delegates to [`CoreInstallLayout`] so the updater and the locator always
/// agree (`sing_box` is `singbox`, never `sing-box`).
pub fn core_dir(core: CoreType) -> &'static str {
    CoreInstallLayout::dir_name(core)
}

/// The update-pipeline key for a core type.
///
/// The updater identifies cores with the lowercase `Global.CoreUrls` keys
/// (`updater::channel::CORE_URLS`); `CoreType::as_str()` preserves upstream's
/// capitalized `Xray`. The ordinary install/update entry matrix maps through
/// here so it can never omit or mis-name a core the runtime has an adapter for
/// (R4-21).
pub const fn update_core_key(core: CoreType) -> &'static str {
    match core {
        CoreType::V2fly => "v2fly",
        CoreType::Xray => "xray",
        CoreType::V2flyV5 => "v2fly_v5",
        CoreType::Mihomo => "mihomo",
        CoreType::Hysteria => "hysteria",
        CoreType::NaiveProxy => "naiveproxy",
        CoreType::Tuic => "tuic",
        CoreType::SingBox => "sing_box",
        CoreType::Juicity => "juicity",
        CoreType::Hysteria2 => "hysteria2",
        CoreType::Brook => "brook",
        CoreType::OverTls => "overtls",
        CoreType::ShadowQuic => "shadowquic",
        CoreType::Mieru => "mieru",
        CoreType::App => "v2rayN",
    }
}

/// Reverse of [`update_core_key`] for the proxy cores.
pub fn core_type_for_update_key(key: &str) -> Option<CoreType> {
    CoreType::PROXY_CORES
        .iter()
        .copied()
        .find(|core| update_core_key(*core) == key)
}

/// The managed cores root for the current user, mirroring
/// `application::AppEngine::default_data_dir()` (`<data>/cores`) so the runtime
/// and the update pipeline resolve the same directory even before the app has
/// forwarded an explicit `V2RAYN_R_CORES_ROOT`.
pub fn default_managed_cores_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("V2RAYN_R_DATA_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("cores");
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local)
            .join("v2rayn-r")
            .join("data")
            .join("cores");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("v2rayn-r")
            .join("data")
            .join("cores");
    }
    PathBuf::from("v2rayn-r-data").join("cores")
}

/// Explicit development mode. Enables the repository `tools/cores` fallback and
/// the `V2RAYN_R_XRAY_BIN` override; the packaged application never sets it.
pub fn dev_mode_enabled() -> bool {
    std::env::var_os("V2RAYN_R_DEV_MODE").is_some_and(|value| !value.is_empty())
}

/// Reject a located executable that could never run. Upstream only checks
/// `File.Exists`; a zero-byte (failed/partial) download would otherwise be
/// treated as a located core and fail later with a confusing spawn error. The
/// readable `error.core_invalid` instead sends the user to the install/update
/// entry. Other corruption is caught by the core's real `test_args` check.
fn validate_core_exe(path: &Path) -> Result<(), DomainError> {
    let meta = std::fs::metadata(path).map_err(|e| {
        DomainError::new(domain::codes::NOT_FOUND, "error.core_invalid")
            .with_field("core")
            .with_detail(format!("{}: {e}", path.display()))
    })?;
    if meta.len() == 0 {
        return Err(
            DomainError::new(domain::codes::NOT_FOUND, "error.core_invalid")
                .with_field("core")
                .with_detail(format!("{} is empty (0 bytes)", path.display())),
        );
    }
    // A non-empty-but-corrupt core is caught by the core's own `test_args`
    // config check before the old session is stopped; that error is readable
    // and retryable (`error.config_check_failed`). The locator only rejects the
    // unambiguous empty file so a failed/partial download cannot be treated as
    // a located core.
    Ok(())
}

/// Scan a core directory (pinned version dir or the core dir plus its
/// immediate version subdirectories) for any candidate executable name.
fn find_candidate_exe(
    layout: &CoreInstallLayout,
    core: CoreType,
    pinned: Option<&str>,
    adapter: &dyn CoreAdapter,
) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    match pinned {
        Some(version) => dirs.push(layout.version_dir(core, version)),
        None => {
            let base = layout.core_dir(core);
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        dirs.push(path);
                    }
                }
            }
            dirs.push(base);
        }
    }
    for dir in dirs {
        for name in adapter.exe_names() {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
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
    /// Canonical executable file name including extension.
    fn exe_name(&self) -> &'static str;
    /// Candidate executable names, canonical first. Mirrors upstream
    /// `CoreInfo.CoreExes`; the locator scans a version directory for each in
    /// order so e.g. `sing-box-client`/`sing-box` and the platform-suffixed
    /// hysteria/brook binaries resolve the same way as the frozen 7.25.4 table.
    fn exe_names(&self) -> Vec<&'static str> {
        vec![self.exe_name()]
    }
    /// Arguments to run with a config file.
    fn run_args(&self, config: &Path) -> Vec<OsString>;
    /// Arguments for a config check that exits without binding anything.
    fn test_args(&self, config: &Path) -> Vec<OsString>;
    /// Arguments for a version probe.
    fn version_args(&self) -> Vec<OsString>;
    /// Environment variables the core needs to find its config. Empty for
    /// cores that take the config path as a command-line argument. Mirrors
    /// upstream `CoreInfo.Environment` (R3-03).
    fn env_vars(&self, _config: &Path) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Working directory for the process. `None` keeps the caller's default;
    /// cores like mihomo pass their directory as an argument instead.
    fn working_dir(&self, _config: &Path) -> Option<std::path::PathBuf> {
        None
    }
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

    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["sing-box-client.exe", "sing-box.exe"]
        } else {
            vec!["sing-box-client", "sing-box"]
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

/// v2fly (v4) adapter: `v2ray -config {0}`, `v2ray -test -config {0}`,
/// `v2ray -version`.
///
/// The real 4.45.2 binary ignores a bare positional config path (upstream
/// `Arguments = "{0}"`) and falls back to `<exe dir>/config.json`, so the
/// explicit `-config` flag is required for the staged config to load. `-test`
/// performs a real non-binding config validation.
pub struct V2flyAdapter;

impl CoreAdapter for V2flyAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::V2fly
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "v2ray.exe"
        } else {
            "v2ray"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["v2ray.exe"]
        } else {
            vec!["v2ray"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["-config".into(), config.as_os_str().to_os_string()]
    }
    fn test_args(&self, config: &Path) -> Vec<OsString> {
        vec![
            "-test".into(),
            "-config".into(),
            config.as_os_str().to_os_string(),
        ]
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["-version".into()]
    }
}

/// v2fly v5 adapter: `v2ray run -c {0}`, `v2ray test -c {0}`,
/// `v2ray version`.
///
/// The real 5.53.0 binary rejects upstream's `-format jsonv5` flag ("unknown
/// field `type`" once the v5 native loader is selected); `run`/`test` auto-
/// detect the v4-compatible schema the shared config generator emits, so the
/// format flag is not passed.
pub struct V2flyV5Adapter;

impl CoreAdapter for V2flyV5Adapter {
    fn core_type(&self) -> CoreType {
        CoreType::V2flyV5
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "v2ray.exe"
        } else {
            "v2ray"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["v2ray.exe"]
        } else {
            vec!["v2ray"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["run".into(), "-c".into(), config.as_os_str().to_os_string()]
    }
    fn test_args(&self, config: &Path) -> Vec<OsString> {
        vec![
            "test".into(),
            "-c".into(),
            config.as_os_str().to_os_string(),
        ]
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["version".into()]
    }
}

/// mihomo adapter: `-f {0}`, `VersionArg = "-v"`.
pub struct MihomoAdapter;

impl CoreAdapter for MihomoAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Mihomo
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "mihomo-windows-amd64-v1.exe"
        } else {
            "mihomo-linux-amd64-v1"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        // Mirrors `CoreInfoManager.GetMihomoCoreExes()` (+ `.exe` on Windows).
        if cfg!(windows) {
            vec![
                "mihomo-windows-amd64-v1.exe",
                "mihomo-windows-amd64-compatible.exe",
                "mihomo-windows-amd64.exe",
                "mihomo-windows-arm64.exe",
                "clash.exe",
                "mihomo.exe",
            ]
        } else {
            vec![
                "mihomo-linux-amd64-v1",
                "mihomo-linux-amd64",
                "mihomo-linux-arm64",
                "mihomo-linux-riscv64",
                "mihomo-linux-loong64-abi2",
                "clash",
                "mihomo",
            ]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        // Upstream `Arguments = "-f {0}" + PortableMode() -d <bin dir>`.
        vec![
            "-f".into(),
            config.as_os_str().to_os_string(),
            "-d".into(),
            config
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .as_os_str()
                .to_os_string(),
        ]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["-v".into()]
    }
}

/// hysteria adapter: upstream `Arguments = ""`.
///
/// The v1 core takes no config path argument; it reads `./config.json` from
/// its working directory, so the staged config's parent is the working
/// directory (verified against 1.3.5).
pub struct HysteriaAdapter;

impl CoreAdapter for HysteriaAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Hysteria
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "hysteria.exe"
        } else {
            "hysteria"
        }
    }
    fn run_args(&self, _config: &Path) -> Vec<OsString> {
        Vec::new()
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["-v".into()]
    }
    fn working_dir(&self, config: &Path) -> Option<std::path::PathBuf> {
        config.parent().map(Path::to_path_buf)
    }
}

/// naiveproxy adapter: candidate `naive`/`naiveproxy`, `Arguments = "{0}"`.
pub struct NaiveProxyAdapter;

impl CoreAdapter for NaiveProxyAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::NaiveProxy
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "naive.exe"
        } else {
            "naive"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["naive.exe", "naiveproxy.exe"]
        } else {
            vec!["naive", "naiveproxy"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec![config.as_os_str().to_os_string()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// tuic adapter: candidates `tuic-client`/`tuic`, `-c {0}`.
pub struct TuicAdapter;

impl CoreAdapter for TuicAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Tuic
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "tuic-client.exe"
        } else {
            "tuic-client"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["tuic-client.exe", "tuic.exe"]
        } else {
            vec!["tuic-client", "tuic"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["-c".into(), config.as_os_str().to_os_string()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// juicity adapter: candidates `juicity-client`/`juicity`, `run -c {0}`.
pub struct JuicityAdapter;

impl CoreAdapter for JuicityAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Juicity
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "juicity-client.exe"
        } else {
            "juicity-client"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["juicity-client.exe", "juicity.exe"]
        } else {
            vec!["juicity-client", "juicity"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["run".into(), "-c".into(), config.as_os_str().to_os_string()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// hysteria2 adapter: platform-suffixed binaries, upstream `Arguments = ""`.
///
/// Like hysteria v1 the core consumes `config.json` resolved from its working
/// directory (viper "config" search), so the config's parent is the working
/// directory. Verified against 2.12.3.
pub struct Hysteria2Adapter;

impl CoreAdapter for Hysteria2Adapter {
    fn core_type(&self) -> CoreType {
        CoreType::Hysteria2
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "hysteria-windows-amd64.exe"
        } else {
            "hysteria-linux-amd64"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["hysteria-windows-amd64.exe", "hysteria.exe"]
        } else {
            vec!["hysteria-linux-amd64", "hysteria"]
        }
    }
    fn run_args(&self, _config: &Path) -> Vec<OsString> {
        Vec::new()
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["version".into()]
    }
    fn working_dir(&self, config: &Path) -> Option<std::path::PathBuf> {
        config.parent().map(Path::to_path_buf)
    }
}

/// brook adapter: platform-suffixed binaries, upstream `Arguments = " {0}"`.
pub struct BrookAdapter;

impl CoreAdapter for BrookAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Brook
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "brook_windows_amd64.exe"
        } else {
            "brook_linux_amd64"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["brook_windows_amd64.exe", "brook.exe"]
        } else {
            vec!["brook_linux_amd64", "brook"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec![config.as_os_str().to_os_string()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// overtls adapter: candidates `overtls-bin`/`overtls`, `-r client -c {0}`.
pub struct OverTlsAdapter;

impl CoreAdapter for OverTlsAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::OverTls
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "overtls-bin.exe"
        } else {
            "overtls-bin"
        }
    }
    fn exe_names(&self) -> Vec<&'static str> {
        if cfg!(windows) {
            vec!["overtls-bin.exe", "overtls.exe"]
        } else {
            vec!["overtls-bin", "overtls"]
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec![
            "-r".into(),
            "client".into(),
            "-c".into(),
            config.as_os_str().to_os_string(),
        ]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// shadowquic adapter: `-c {0}`.
pub struct ShadowQuicAdapter;

impl CoreAdapter for ShadowQuicAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::ShadowQuic
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "shadowquic.exe"
        } else {
            "shadowquic"
        }
    }
    fn run_args(&self, config: &Path) -> Vec<OsString> {
        vec!["-c".into(), config.as_os_str().to_os_string()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["--version".into()]
    }
}

/// mieru adapter: `run`, config delivered via `MIERU_CONFIG_JSON_FILE`.
pub struct MieruAdapter;

impl CoreAdapter for MieruAdapter {
    fn core_type(&self) -> CoreType {
        CoreType::Mieru
    }
    fn exe_name(&self) -> &'static str {
        if cfg!(windows) {
            "mieru.exe"
        } else {
            "mieru"
        }
    }
    fn run_args(&self, _config: &Path) -> Vec<OsString> {
        vec!["run".into()]
    }
    fn test_args(&self, _config: &Path) -> Vec<OsString> {
        self.version_args()
    }
    fn version_args(&self) -> Vec<OsString> {
        vec!["version".into()]
    }
    /// Upstream `CoreInfoManager` injects `MIERU_CONFIG_JSON_FILE={0}` for
    /// mieru; `run` reads the config from that environment variable (R3-03).
    fn env_vars(&self, config: &Path) -> Vec<(String, String)> {
        vec![(
            "MIERU_CONFIG_JSON_FILE".to_string(),
            config.to_string_lossy().into_owned(),
        )]
    }
}

/// Resolve the adapter for a core type. `App` (the v2rayN self-update
/// identity) has no proxy adapter; every `CoreType::PROXY_CORES` entry does.
pub fn adapter_for(core: CoreType) -> Option<Box<dyn CoreAdapter>> {
    match core {
        CoreType::Xray => Some(Box::new(XrayAdapter)),
        CoreType::SingBox => Some(Box::new(SingBoxAdapter)),
        CoreType::V2fly => Some(Box::new(V2flyAdapter)),
        CoreType::V2flyV5 => Some(Box::new(V2flyV5Adapter)),
        CoreType::Mihomo => Some(Box::new(MihomoAdapter)),
        CoreType::Hysteria => Some(Box::new(HysteriaAdapter)),
        CoreType::NaiveProxy => Some(Box::new(NaiveProxyAdapter)),
        CoreType::Tuic => Some(Box::new(TuicAdapter)),
        CoreType::Juicity => Some(Box::new(JuicityAdapter)),
        CoreType::Hysteria2 => Some(Box::new(Hysteria2Adapter)),
        CoreType::Brook => Some(Box::new(BrookAdapter)),
        CoreType::OverTls => Some(Box::new(OverTlsAdapter)),
        CoreType::ShadowQuic => Some(Box::new(ShadowQuicAdapter)),
        CoreType::Mieru => Some(Box::new(MieruAdapter)),
        CoreType::App => None,
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
        assert_eq!(core_dir(CoreType::SingBox), "singbox");
    }

    #[test]
    fn app_identity_has_no_adapter_but_every_proxy_core_does() {
        assert!(adapter_for(CoreType::App).is_none());
        for core in CoreType::PROXY_CORES {
            assert!(
                adapter_for(core).is_some(),
                "missing adapter for {}",
                core.as_str()
            );
        }
    }

    #[test]
    fn update_core_key_round_trips_every_proxy_core() {
        assert_eq!(update_core_key(CoreType::Xray), "xray");
        assert_eq!(update_core_key(CoreType::SingBox), "sing_box");
        assert_eq!(update_core_key(CoreType::App), "v2rayN");
        for core in CoreType::PROXY_CORES {
            let key = update_core_key(core);
            assert_eq!(
                core_type_for_update_key(key),
                Some(core),
                "update key {key} does not map back"
            );
        }
        assert!(core_type_for_update_key("v2rayN").is_none());
    }

    #[test]
    fn candidate_exe_names_mirror_frozen_core_info() {
        let singbox = adapter_for(CoreType::SingBox).unwrap();
        assert!(singbox
            .exe_names()
            .iter()
            .any(|n| n.starts_with("sing-box")));
        let naive = adapter_for(CoreType::NaiveProxy).unwrap();
        assert!(naive.exe_names().iter().any(|n| n.starts_with("naive")));
        let hysteria2 = adapter_for(CoreType::Hysteria2).unwrap();
        assert!(hysteria2.exe_names().iter().any(|n| n.contains("hysteria")));
    }

    #[test]
    fn candidate_fallback_finds_alternate_exe_name() {
        let root = temp_root("candidates");
        let dir = root
            .join(CoreInstallLayout::dir_name(CoreType::NaiveProxy))
            .join("v1.0.0");
        std::fs::create_dir_all(&dir).unwrap();
        // Canonical name is absent; the alternate upstream name resolves.
        let alternate = if cfg!(windows) {
            "naiveproxy.exe"
        } else {
            "naiveproxy"
        };
        std::fs::write(dir.join(alternate), core_stub()).unwrap();
        let locator = CoreLocator::with_roots(vec![root.clone()], None);
        let picked = locator
            .resolve(CoreType::NaiveProxy, Some("v1.0.0"))
            .unwrap();
        assert!(picked.ends_with(alternate));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mihomo_run_args_use_config_flag() {
        let adapter = MihomoAdapter;
        let args = adapter.run_args(Path::new("C:/run/config.json"));
        assert_eq!(args[0], "-f");
        assert_eq!(args[1], "C:/run/config.json");
    }

    #[test]
    fn mieru_config_is_delivered_via_environment() {
        let adapter = MieruAdapter;
        let config = Path::new("C:/run/config.json");
        assert_eq!(adapter.run_args(config), vec![OsString::from("run")]);
        let env = adapter.env_vars(config);
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, "MIERU_CONFIG_JSON_FILE");
        assert_eq!(env[0].1, "C:/run/config.json");
    }

    #[test]
    fn cores_without_env_contract_report_none() {
        for adapter in [
            Box::new(XrayAdapter) as Box<dyn CoreAdapter>,
            Box::new(SingBoxAdapter),
            Box::new(MihomoAdapter),
            Box::new(NaiveProxyAdapter),
            Box::new(TuicAdapter),
        ] {
            let config = Path::new("C:/run/config");
            assert!(
                adapter.env_vars(config).is_empty(),
                "{} must not inject environment",
                adapter.core_type().as_str()
            );
        }
    }

    #[test]
    fn v2fly_uses_the_explicit_config_flag() {
        let adapter = V2flyAdapter;
        let config = Path::new("C:/run/config.json");
        assert_eq!(
            adapter.run_args(config),
            vec![
                OsString::from("-config"),
                OsString::from("C:/run/config.json")
            ]
        );
        assert_eq!(
            adapter.test_args(config),
            vec![
                OsString::from("-test"),
                OsString::from("-config"),
                OsString::from("C:/run/config.json")
            ]
        );
    }

    #[test]
    fn v2fly_v5_uses_run_and_test_without_jsonv5_format() {
        let adapter = V2flyV5Adapter;
        let config = Path::new("C:/run/config.json");
        let run = adapter.run_args(config);
        assert_eq!(
            run,
            vec![
                OsString::from("run"),
                OsString::from("-c"),
                OsString::from("C:/run/config.json")
            ]
        );
        assert!(!run.iter().any(|a| a == "jsonv5" || a == "-format"));
        assert_eq!(
            adapter.test_args(config),
            vec![
                OsString::from("test"),
                OsString::from("-c"),
                OsString::from("C:/run/config.json")
            ]
        );
    }

    #[test]
    fn hysteria_cores_run_from_the_config_directory() {
        let adapters: Vec<Box<dyn CoreAdapter>> =
            vec![Box::new(HysteriaAdapter), Box::new(Hysteria2Adapter)];
        for adapter in adapters {
            let config = Path::new("C:/run/session/config.json");
            assert!(adapter.run_args(config).is_empty());
            assert_eq!(
                adapter.working_dir(config),
                Some(PathBuf::from("C:/run/session"))
            );
        }
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

    /// A tiny stand-in executable. On Windows it carries the `MZ` header so the
    /// locator's corrupt-core check accepts it.
    fn core_stub() -> &'static [u8] {
        if cfg!(windows) {
            b"MZstub"
        } else {
            b"stub"
        }
    }

    fn write_core(root: &Path, version: &str, bytes: &[u8]) -> PathBuf {
        let exe = if cfg!(windows) { "xray.exe" } else { "xray" };
        let dir = root.join("xray").join(version);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(exe);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn zero_byte_core_is_reported_invalid() {
        let root = temp_root("zero-byte");
        write_core(&root, "v1.0.0", b"");
        let locator = CoreLocator::with_roots(vec![root.clone()], None);
        let err = locator.resolve(CoreType::Xray, None).unwrap_err();
        assert_eq!(err.message_key, "error.core_invalid");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_handles_a_managed_root_with_spaces() {
        let root = temp_root("managed cores root");
        write_core(&root, "v1.0.0", core_stub());
        let locator = CoreLocator::with_roots(vec![root.clone()], None);
        let picked = locator.resolve(CoreType::Xray, None).unwrap();
        assert!(picked.to_string_lossy().contains("managed cores root"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dev_fallback_and_xray_override_require_dev_mode() {
        let _guard = ENV_GUARD.lock().unwrap();
        let root = temp_root("dev-gate");
        let override_exe = root.join(if cfg!(windows) { "xray.exe" } else { "xray" });
        std::fs::write(&override_exe, core_stub()).unwrap();
        let override_str = override_exe.to_string_lossy().into_owned();
        let _env = EnvScope::set(&[
            ("V2RAYN_R_XRAY_BIN", Some(&override_str)),
            ("V2RAYN_R_DEV_MODE", None),
            ("V2RAYN_R_CORES_ROOT", Some("C:/managed/cores")),
        ]);
        let locator = CoreLocator::from_env();
        assert!(
            locator.xray_override.is_none(),
            "the dev binary override must be ignored outside dev mode"
        );
        std::env::set_var("V2RAYN_R_DEV_MODE", "1");
        let dev = CoreLocator::from_env();
        assert!(dev.xray_override.is_some());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_prefers_highest_version_then_pinned() {
        let root = temp_root("versions");
        let exe = if cfg!(windows) { "xray.exe" } else { "xray" };
        for version in ["v1.0.0", "v2.0.0"] {
            let dir = root.join("xray").join(version);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(exe), core_stub()).unwrap();
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
        std::fs::write(&override_exe, core_stub()).unwrap();
        let locator = CoreLocator::with_roots(vec![root.join("empty")], Some(override_exe.clone()));
        assert_eq!(locator.resolve(CoreType::Xray, None).unwrap(), override_exe);

        let missing = CoreLocator::with_roots(vec![root.join("empty")], None);
        let err = missing.resolve(CoreType::Xray, None).unwrap_err();
        assert_eq!(err.code, domain::codes::NOT_FOUND);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Serializes the two env-mutating tests in this module; no other test in
    /// the crate reads `V2RAYN_R_CORES_ROOT` / `V2RAYN_R_DATA_DIR`.
    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvScope(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvScope {
        fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let saved = pairs
                .iter()
                .map(|(k, _)| (*k, std::env::var_os(k)))
                .collect::<Vec<_>>();
            for (k, v) in pairs {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
            Self(saved)
        }
    }

    impl Drop for EnvScope {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn explicit_cores_root_disables_dev_fallback() {
        let _guard = ENV_GUARD.lock().unwrap();
        let root = temp_root("explicit-root");
        let _env = EnvScope::set(&[("V2RAYN_R_CORES_ROOT", Some("C:/managed/cores"))]);
        let locator = CoreLocator::from_env();
        assert_eq!(locator.roots.len(), 1);
        assert_eq!(locator.roots[0], PathBuf::from("C:/managed/cores"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn default_root_matches_engine_data_cores() {
        let _guard = ENV_GUARD.lock().unwrap();
        let root = temp_root("default-root");
        let data = root.join("data");
        let data_str = data.to_string_lossy().into_owned();
        let _env = EnvScope::set(&[
            ("V2RAYN_R_CORES_ROOT", None),
            ("V2RAYN_R_DATA_DIR", Some(&data_str)),
        ]);
        assert_eq!(default_managed_cores_root(), data.join("cores"));
        // Without an explicit root the dev fallback is allowed to participate.
        let locator = CoreLocator::from_env();
        assert_eq!(locator.roots[0], data.join("cores"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
