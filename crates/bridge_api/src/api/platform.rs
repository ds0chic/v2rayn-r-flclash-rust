//! T13 FRB surface: system proxy (four modes + ownership restore), PAC service,
//! autostart, hotkeys, and the small desktop actions (UWP loopback, admin
//! restart).
//!
//! ## Safety boundary (AGENTS.md / T13 task card)
//!
//! Automated tests, `cargo test`, and the default desktop runtime must never
//! mutate the host system proxy, the Run key, or the routing table. This module
//! therefore wires a [`PlatformService`] over the **in-memory fakes** by
//! default. The real Windows backends are only selected when the caller
//! explicitly opts in via [`init_platform_backend`] with `"windows"`, which the
//! release desktop app does only for a real, user-initiated action — never from
//! an environment variable in debug/test builds.
//!
//! The functions here are thin adapters over `application::platform_service`;
//! all ownership logic lives there and is unit-tested with the fakes.

use std::sync::{Arc, Mutex, OnceLock};

use application::platform_service::{
    derived_local_port, mode_from_domain, mode_value, Ownership, PacHandle, PlatformService,
    ProxyApplyRequest,
};
use domain::SysProxyType;
use flutter_rust_bridge::frb;
use platform::{
    CustomSystemProxySetting, FakeRegistry, FakeSystemProxyBackend, PacSource, SysProxyMode,
    DEFAULT_PAC_PORT_BASE,
};

use crate::api::contract::{ErrorDto, SimpleResult};

/// The four `ESysProxyType` numeric values, for UI labels.
pub const SYSPROXY_FORCED_CLEAR: i32 = 0;
pub const SYSPROXY_FORCED_CHANGE: i32 = 1;
pub const SYSPROXY_UNCHANGED: i32 = 2;
pub const SYSPROXY_PAC: i32 = 3;

/// The default single-instance / autostart Run value prefix (upstream
/// `v2rayNAutoRun_<md5(startup_path)>`).
pub const AUTO_RUN_PREFIX: &str = "v2rayNAutoRun";

// ---------------------------------------------------------------------------
// Flat DTOs
// ---------------------------------------------------------------------------

/// One applied field transition (proof of ownership), readable by the UI.
#[derive(Clone)]
pub struct AppliedChangeDto {
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// One restore action (owned field written back to its origin).
#[derive(Clone)]
pub struct RestoreActionDto {
    pub field: String,
    pub owned_value: Option<String>,
    pub restore_to: Option<String>,
}

/// One externally-modified field kept as-is and reported to the user.
#[derive(Clone)]
pub struct RestoreConflictDto {
    pub field: String,
    pub owned_value: Option<String>,
    pub current_value: Option<String>,
}

/// A per-field ownership classification.
#[derive(Clone)]
pub struct ProxyOwnershipDto {
    pub field: String,
    /// `none` | `ours` | `external`
    pub ownership: String,
}

/// Current per-field proxy state plus the ownership view.
#[derive(Clone)]
pub struct ProxyStateDto {
    pub ok: bool,
    pub enabled: bool,
    pub server: Option<String>,
    pub bypass: Option<String>,
    pub auto_config_url: Option<String>,
    pub auto_detect: bool,
    pub desired_mode: i32,
    pub has_ownership: bool,
    pub pac_running: bool,
    pub pac_url: Option<String>,
    pub pac_port: Option<u16>,
    pub conflicts: Vec<String>,
    pub ownership: Vec<ProxyOwnershipDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `set_system_proxy`.
#[derive(Clone)]
pub struct SysProxyApplyResult {
    pub ok: bool,
    pub mode: i32,
    pub applied: Vec<AppliedChangeDto>,
    pub current: Option<ProxyStateDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `restore_system_proxy` / `restore_system_proxy_on_exit`.
#[derive(Clone)]
pub struct SysProxyRestoreResult {
    pub ok: bool,
    pub clean: bool,
    pub restored: Vec<RestoreActionDto>,
    pub conflicts: Vec<RestoreConflictDto>,
    pub error: Option<ErrorDto>,
}

/// PAC server handle.
#[derive(Clone)]
pub struct PacHandleDto {
    pub ok: bool,
    pub running: bool,
    pub url: Option<String>,
    pub port: Option<u16>,
    pub error: Option<ErrorDto>,
}

/// A resolved PAC script file (path + raw text). `text` still contains
/// `__PROXY__`; substitution happens at serve time.
#[derive(Clone)]
pub struct PacScriptDto {
    pub ok: bool,
    pub path: Option<String>,
    pub text: Option<String>,
    /// True when the file was created from the bundled default template.
    pub seeded_default: bool,
    pub error: Option<ErrorDto>,
}

/// One `KeyEventItem` binding exposed to the hotkey window.
#[derive(Clone)]
pub struct HotkeyDto {
    /// `EGlobalHotkey` numeric value (0..4).
    pub action: i32,
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
    pub key_code: Option<i32>,
}

/// Result of a hotkey registration attempt (`hotkey_register`).
#[derive(Clone)]
pub struct HotkeyRegisterResult {
    pub ok: bool,
    pub registered: Vec<HotkeyDto>,
    /// Human-readable conflict/failure notes per action, if any.
    pub failures: Vec<String>,
    pub error: Option<ErrorDto>,
}

// ---------------------------------------------------------------------------
// Process-global platform service
// ---------------------------------------------------------------------------

static PLATFORM: OnceLock<Mutex<Option<PlatformService>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<PlatformService>> {
    PLATFORM.get_or_init(|| Mutex::new(None))
}

/// Build the default (fake-backed) service. Never touches the host.
fn fake_service() -> PlatformService {
    PlatformService::new(
        Arc::new(FakeSystemProxyBackend::default()),
        Arc::new(FakeRegistry::new()),
    )
}

#[cfg(windows)]
fn windows_service() -> PlatformService {
    use platform::{WindowsRunKeyBackend, WindowsSystemProxyBackend};
    PlatformService::new(
        Arc::new(WindowsSystemProxyBackend::new()),
        Arc::new(WindowsRunKeyBackend::new()),
    )
}

#[cfg(not(windows))]
fn windows_service() -> PlatformService {
    fake_service()
}

/// Explicit opt-in to the real Windows backend for the process.
///
/// `backend` is `"fake"` (default) or `"windows"`. The release desktop app
/// calls this with `"windows"` exactly once at startup; tests and evidence
/// runs keep the fake, so no automated path mutates the host.
#[frb(sync)]
pub fn init_platform_backend(backend: String) {
    let service = match backend.as_str() {
        "windows" => windows_service(),
        _ => fake_service(),
    };
    if let Ok(mut guard) = slot().lock() {
        *guard = Some(service);
    }
}

/// The active service, defaulting to fake backends on first use.
fn service() -> PlatformService {
    let mut guard = slot().lock().unwrap_or_else(|p| p.into_inner());
    guard.get_or_insert_with(fake_service).clone()
}

fn platform_error(e: platform::PlatformError) -> ErrorDto {
    let (code, retryable) = match &e {
        platform::PlatformError::NotFound(_) => ("E_NOT_FOUND", false),
        platform::PlatformError::Invalid(_) => ("E_INVALID_ARGUMENT", false),
        platform::PlatformError::PortInUse(_) => ("E_PORT_IN_USE", true),
        platform::PlatformError::Io(_) => ("E_IO", true),
        platform::PlatformError::Unsupported(_) => ("E_UNSUPPORTED", false),
        platform::PlatformError::Backend(_) => ("E_PLATFORM_BACKEND", true),
        platform::PlatformError::AlreadyRunning(_) => ("E_ALREADY_RUNNING", false),
    };
    ErrorDto {
        code: code.to_string(),
        message_key: format!("error.{code}"),
        field_path: None,
        retryable,
        operation_id: None,
        detail: Some(e.to_string()),
    }
}

fn proxy_state_dto(service: &PlatformService, desired_mode: SysProxyMode) -> ProxyStateDto {
    match service.snapshot() {
        Ok(state) => {
            let view = service.state_view(desired_mode);
            let ownership = service
                .ownership()
                .unwrap_or_default()
                .into_iter()
                .map(|(field, ownership)| ProxyOwnershipDto {
                    field: format!("{field:?}"),
                    ownership: match ownership {
                        Ownership::None => "none",
                        Ownership::Ours => "ours",
                        Ownership::ExternallyModified => "external",
                    }
                    .to_string(),
                })
                .collect();
            ProxyStateDto {
                ok: true,
                enabled: state.enabled,
                server: state.server,
                bypass: state.bypass,
                auto_config_url: state.auto_config_url,
                auto_detect: state.auto_detect,
                desired_mode: view.desired_mode,
                has_ownership: view.has_ownership,
                pac_running: view.pac_running,
                pac_url: view.pac_url,
                pac_port: view.pac_port,
                conflicts: view.conflicts,
                ownership,
                error: None,
            }
        }
        Err(e) => ProxyStateDto {
            ok: false,
            desired_mode: mode_value(desired_mode),
            ownership: Vec::new(),
            conflicts: Vec::new(),
            error: Some(platform_error(e)),
            ..Default::default()
        },
    }
}

impl Default for ProxyStateDto {
    fn default() -> Self {
        Self {
            ok: false,
            enabled: false,
            server: None,
            bypass: None,
            auto_config_url: None,
            auto_detect: false,
            desired_mode: SYSPROXY_UNCHANGED,
            has_ownership: false,
            pac_running: false,
            pac_url: None,
            pac_port: None,
            conflicts: Vec::new(),
            ownership: Vec::new(),
            error: None,
        }
    }
}

fn applied_dto(change: platform::AppliedChange) -> AppliedChangeDto {
    AppliedChangeDto {
        field: format!("{:?}", change.field),
        before: change.before,
        after: change.after,
    }
}

fn mode_symbol(value: i32) -> Option<SysProxyMode> {
    application::platform_service::mode_from_value(value)
}

// ---------------------------------------------------------------------------
// System proxy
// ---------------------------------------------------------------------------

/// Apply one of the four system-proxy modes.
///
/// * `server` / `bypass` are used by `ForcedChange`.
/// * `auto_config_url` is required by `Pac`.
/// * `Unchanged` writes nothing.
///
/// This is the only entry point that can write the host proxy, and only when
/// the real backend was explicitly enabled; tests keep the fake.
#[frb(sync)]
pub fn set_system_proxy(
    mode: i32,
    server: Option<String>,
    bypass: Option<String>,
    auto_config_url: Option<String>,
) -> SysProxyApplyResult {
    let Some(mode) = mode_symbol(mode) else {
        return SysProxyApplyResult {
            ok: false,
            mode,
            applied: Vec::new(),
            current: None,
            error: Some(ErrorDto {
                code: "E_INVALID_ARGUMENT".to_string(),
                message_key: "error.proxy_mode_invalid".to_string(),
                field_path: Some("mode".to_string()),
                retryable: false,
                operation_id: None,
                detail: Some(format!("unknown SysProxyType {mode}")),
            }),
        };
    };
    let service = service();
    let request = ProxyApplyRequest {
        mode,
        server,
        bypass,
        auto_config_url,
        auto_detect: None,
    };
    match service.apply_proxy(&request) {
        Ok(outcome) => SysProxyApplyResult {
            ok: outcome.ok,
            mode: outcome.mode,
            applied: outcome.applied.into_iter().map(applied_dto).collect(),
            current: Some(proxy_state_dto(&service, mode)),
            error: outcome.error_message.map(|message| ErrorDto {
                code: outcome
                    .error_code
                    .unwrap_or_else(|| "E_PLATFORM_BACKEND".to_string()),
                message_key: "error.proxy_apply_failed".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some(message),
            }),
        },
        Err(e) => SysProxyApplyResult {
            ok: false,
            mode: mode_value(mode),
            applied: Vec::new(),
            current: None,
            error: Some(platform_error(e)),
        },
    }
}

/// Read the current proxy state + ownership. `desired_mode` is the persisted
/// `SystemProxyType` the status bar should compare against.
#[frb(sync)]
pub fn get_system_proxy_state(desired_mode: i32) -> ProxyStateDto {
    let mode = mode_symbol(desired_mode).unwrap_or(SysProxyMode::Unchanged);
    proxy_state_dto(&service(), mode)
}

/// Restore only the fields this process still owns (idempotent).
#[frb(sync)]
pub fn restore_system_proxy() -> SysProxyRestoreResult {
    restore_result(service().restore_proxy())
}

/// Exit-path restore (per-field ownership; user edits preserved as conflicts).
#[frb(sync)]
pub fn restore_system_proxy_on_exit(desired_mode: i32) -> SysProxyRestoreResult {
    let mode = mode_symbol(desired_mode).unwrap_or(SysProxyMode::Unchanged);
    restore_result(service().restore_on_exit(mode))
}

fn restore_result(
    result: Result<application::platform_service::ProxyRestoreOutcome, platform::PlatformError>,
) -> SysProxyRestoreResult {
    match result {
        Ok(outcome) => SysProxyRestoreResult {
            ok: outcome.ok,
            clean: outcome.clean,
            restored: outcome
                .restored
                .restored
                .into_iter()
                .map(|action| RestoreActionDto {
                    field: format!("{:?}", action.field),
                    owned_value: action.owned_value,
                    restore_to: action.restore_to,
                })
                .collect(),
            conflicts: outcome
                .restored
                .conflicts
                .into_iter()
                .map(|conflict| RestoreConflictDto {
                    field: format!("{:?}", conflict.field),
                    owned_value: conflict.owned_value,
                    current_value: conflict.current_value,
                })
                .collect(),
            error: None,
        },
        Err(e) => SysProxyRestoreResult {
            ok: false,
            clean: false,
            restored: Vec::new(),
            conflicts: Vec::new(),
            error: Some(platform_error(e)),
        },
    }
}

// ---------------------------------------------------------------------------
// PAC service
// ---------------------------------------------------------------------------

/// Start (or refresh) the loopback PAC server. `port` 0 auto-selects at or
/// above 11808. `proxy_rule` is substituted for the `__PROXY__` placeholder.
/// `pac_text` is the raw PAC script (upstream `pac.txt` or a custom file's
/// content already read by the caller).
#[frb(sync)]
pub fn pac_start(pac_text: String, proxy_rule: Option<String>, port: u32) -> PacHandleDto {
    if port > u16::MAX as u32 {
        return PacHandleDto {
            ok: false,
            running: false,
            url: None,
            port: None,
            error: Some(ErrorDto {
                code: "E_INVALID_ARGUMENT".to_string(),
                message_key: "error.pac_port_range".to_string(),
                field_path: Some("port".to_string()),
                retryable: false,
                operation_id: None,
                detail: Some(format!("port {port} out of range")),
            }),
        };
    }
    match service().pac_start(PacSource::Inline(pac_text), proxy_rule, port as u16) {
        Ok(handle) => PacHandleDto {
            ok: true,
            running: handle.running,
            url: handle.url,
            port: handle.port,
            error: None,
        },
        Err(e) => PacHandleDto {
            ok: false,
            running: false,
            url: None,
            port: None,
            error: Some(platform_error(e)),
        },
    }
}

/// Start the PAC server from a custom PAC file path (existence validated).
#[frb(sync)]
pub fn pac_start_from_file(
    pac_path: String,
    proxy_rule: Option<String>,
    port: u32,
) -> PacHandleDto {
    if port > u16::MAX as u32 {
        return PacHandleDto {
            ok: false,
            running: false,
            url: None,
            port: None,
            error: Some(ErrorDto {
                code: "E_INVALID_ARGUMENT".to_string(),
                message_key: "error.pac_port_range".to_string(),
                field_path: Some("port".to_string()),
                retryable: false,
                operation_id: None,
                detail: Some(format!("port {port} out of range")),
            }),
        };
    }
    match service().pac_start(PacSource::File(pac_path.into()), proxy_rule, port as u16) {
        Ok(handle) => PacHandleDto {
            ok: true,
            running: handle.running,
            url: handle.url,
            port: handle.port,
            error: None,
        },
        Err(e) => PacHandleDto {
            ok: false,
            running: false,
            url: None,
            port: None,
            error: Some(platform_error(e)),
        },
    }
}

/// Stop the PAC server. Idempotent.
#[frb(sync)]
pub fn pac_stop() -> SimpleResult {
    match service().pac_stop() {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(platform_error(e)),
        },
    }
}

/// Current PAC server handle.
#[frb(sync)]
pub fn pac_state() -> PacHandleDto {
    let PacHandle { running, url, port } = service().pac_state();
    PacHandleDto {
        ok: true,
        running,
        url,
        port,
        error: None,
    }
}

/// Resolve the PAC script file the `Pac` mode should serve, mirroring upstream
/// `PacManager.InitText`: use `custom_pac_path` when it names an existing file,
/// otherwise `<config_dir>/pac.txt`, seeding the bundled default template when
/// that file is missing. Returns the raw text (still containing `__PROXY__`).
#[frb(sync)]
pub fn pac_resolve_script(custom_pac_path: Option<String>, config_dir: String) -> PacScriptDto {
    match platform::pac::resolve_pac_script(
        custom_pac_path.as_deref(),
        std::path::Path::new(&config_dir),
    ) {
        Ok(resolved) => PacScriptDto {
            ok: true,
            path: Some(resolved.path.display().to_string()),
            text: Some(resolved.text),
            seeded_default: resolved.seeded_default,
            error: None,
        },
        Err(e) => PacScriptDto {
            ok: false,
            path: None,
            text: None,
            seeded_default: false,
            error: Some(platform_error(e)),
        },
    }
}

// ---------------------------------------------------------------------------
// Autostart
// ---------------------------------------------------------------------------

/// Whether the Run value `name` exists (non-empty).
#[frb(sync)]
pub fn get_autostart(name: String) -> bool {
    service().autostart_enabled(&name).unwrap_or(false)
}

/// Enable/disable autostart for the Run value `name`.
///
/// The real write happens only when the caller explicitly invoked this from a
/// user action and the real backend is enabled; tests keep the fake registry.
#[frb(sync)]
pub fn set_autostart(name: String, enabled: bool, exe: String, args: String) -> SimpleResult {
    match service().set_autostart(&name, enabled, &exe, &args) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(platform_error(e)),
        },
    }
}

/// Compute the Run value name `v2rayNAutoRun_<md5(startup_path)>`.
#[frb(sync)]
pub fn autostart_value_name(startup_path: String) -> String {
    platform::run_value_name(&startup_path)
}

// ---------------------------------------------------------------------------
// Custom proxy script validation
// ---------------------------------------------------------------------------

/// Validate the two custom system-proxy paths (existence only; never executed).
#[frb(sync)]
pub fn validate_custom_proxy_script(
    pac_path: Option<String>,
    script_path: Option<String>,
) -> SimpleResult {
    let setting = CustomSystemProxySetting::new(pac_path, script_path);
    match service().validate_custom_script(&setting) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(platform_error(e)),
        },
    }
}

// ---------------------------------------------------------------------------
// Desktop actions (UWP loopback, admin reboot)
// ---------------------------------------------------------------------------

/// Resolve the `bin/EnableLoopback.exe` companion next to the running binary.
///
/// The actual process start belongs to the desktop runtime, which performs it
/// only on an explicit user action; this returns the resolved path (validated
/// to exist) so the UI can report precisely what would run.
#[frb(sync)]
pub fn resolve_uwp_loopback_tool(bin_dir: Option<String>) -> SimpleResult {
    let base = bin_dir.map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default()
    });
    let candidate = base.join("EnableLoopback.exe");
    if candidate.is_file() {
        SimpleResult {
            ok: true,
            error: None,
        }
    } else {
        SimpleResult {
            ok: false,
            error: Some(ErrorDto {
                code: "E_NOT_FOUND".to_string(),
                message_key: "error.uwp_tool_missing".to_string(),
                field_path: None,
                retryable: false,
                operation_id: None,
                detail: Some(candidate.display().to_string()),
            }),
        }
    }
}

/// Hotkey actions that map to a `SysProxyType` (upstream `EGlobalHotkey`
/// 1..4 -> `(int)e - 1`); `ShowForm` (0) has no proxy mapping.
#[frb(sync)]
pub fn hotkey_proxy_mode(action: i32) -> Option<i32> {
    match action {
        1 => Some(SYSPROXY_FORCED_CLEAR),
        2 => Some(SYSPROXY_FORCED_CHANGE),
        3 => Some(SYSPROXY_UNCHANGED),
        4 => Some(SYSPROXY_PAC),
        _ => None,
    }
}

/// Convert a persisted hotkey list into registration-ready DTOs, dropping
/// entries with no key code (upstream `HotkeyManager.Init` skips `KeyCode=0`).
#[frb(sync)]
pub fn hotkey_list(hotkeys: Vec<HotkeyDto>) -> Vec<HotkeyDto> {
    hotkeys
        .into_iter()
        .filter(|h| h.key_code.unwrap_or(0) != 0)
        .collect()
}

/// Convert a domain `SysProxyType` to the platform mode number (bridge helper).
#[frb(sync)]
pub fn sysproxy_mode_value(value: i32) -> i32 {
    domain::SysProxyType::from_value(value)
        .map(|t| mode_value(mode_from_domain(t)))
        .unwrap_or(SYSPROXY_UNCHANGED)
}

/// Derived local port (`base + InboundProtocol offset`), mirroring upstream.
#[frb(sync)]
pub fn local_port_for_protocol(base_port: i32, offset: u32) -> u16 {
    if offset > u16::MAX as u32 {
        return base_port.max(1).min(u16::MAX as i32) as u16;
    }
    derived_local_port(base_port, offset as u16)
}

/// The reserved PAC port base (11808). Never 10808.
#[frb(sync)]
pub fn pac_port_base() -> u16 {
    DEFAULT_PAC_PORT_BASE
}

/// Domain `SysProxyType` round-trip helper for the settings window.
#[frb(sync)]
pub fn sysproxy_type_valid(value: i32) -> bool {
    SysProxyType::from_value(value).is_some()
}

/// Test-only: reset the process-global platform service to fake backends.
#[cfg(test)]
pub(crate) fn reset_platform_service_for_test() {
    if let Ok(mut guard) = slot().lock() {
        *guard = Some(fake_service());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysproxy_four_modes_apply_and_restore_through_fake() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        // Apply forced-change.
        let applied = set_system_proxy(
            SYSPROXY_FORCED_CHANGE,
            Some("127.0.0.1:10809".to_string()),
            Some("<local>".to_string()),
            None,
        );
        assert!(applied.ok, "{:?}", applied.error.map(|e| e.code));
        assert!(applied.applied.iter().any(|c| c.field == "Server"));
        let state = get_system_proxy_state(SYSPROXY_FORCED_CHANGE);
        assert!(state.ok);
        assert!(state.enabled);
        assert_eq!(state.server.as_deref(), Some("127.0.0.1:10809"));
        assert!(state.has_ownership);

        // Restore returns to default.
        let restored = restore_system_proxy();
        assert!(restored.ok);
        assert!(restored.clean);
        let final_state = get_system_proxy_state(SYSPROXY_FORCED_CHANGE);
        assert!(!final_state.enabled);
        assert_eq!(final_state.server, None);
    }

    #[test]
    fn unchanged_is_noop_and_pac_requires_url() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        let noop = set_system_proxy(SYSPROXY_UNCHANGED, None, None, None);
        assert!(noop.ok);
        assert!(noop.applied.is_empty());
        let bad = set_system_proxy(SYSPROXY_PAC, None, None, None);
        assert!(!bad.ok);
        assert_eq!(bad.error.unwrap().code, "E_INVALID_ARGUMENT");
        let good = set_system_proxy(
            SYSPROXY_PAC,
            None,
            None,
            Some("http://127.0.0.1:11808/pac".to_string()),
        );
        assert!(good.ok);
        let _ = restore_system_proxy();
    }

    #[test]
    fn invalid_mode_value_is_rejected() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        let result = set_system_proxy(99, None, None, None);
        assert!(!result.ok);
        assert_eq!(result.error.unwrap().code, "E_INVALID_ARGUMENT");
    }

    #[test]
    fn pac_lifecycle_through_bridge() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        let handle = pac_start(
            "function FindProxyForURL(){ return \"__PROXY__\"; }".to_string(),
            Some("PROXY 127.0.0.1:10809;DIRECT;".to_string()),
            0,
        );
        assert!(handle.ok);
        assert!(handle.running);
        let port = handle.port.unwrap();
        assert!(port >= DEFAULT_PAC_PORT_BASE);
        assert_ne!(port, 10808);
        assert!(pac_state().running);
        assert!(pac_stop().ok);
        assert!(!pac_state().running);
    }

    #[test]
    fn autostart_round_trip_through_fake() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        let name = autostart_value_name("C:\\app".to_string());
        assert!(name.starts_with(AUTO_RUN_PREFIX));
        assert!(!get_autostart(name.clone()));
        assert!(
            set_autostart(
                name.clone(),
                true,
                "C:\\app\\v2rayn.exe".to_string(),
                String::new()
            )
            .ok
        );
        assert!(get_autostart(name.clone()));
        assert!(set_autostart(name.clone(), false, String::new(), String::new()).ok);
        assert!(!get_autostart(name));
    }

    #[test]
    fn pac_resolve_script_seeds_and_reads_default() {
        let dir = std::env::temp_dir().join(format!("v2rayn-r-pac-bridge-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dto = pac_resolve_script(None, dir.display().to_string());
        assert!(dto.ok, "{:?}", dto.error.map(|e| e.code));
        assert!(dto.seeded_default);
        assert!(dto.text.as_deref().is_some_and(|t| t.contains("__PROXY__")));
        assert!(dto.path.as_deref().is_some_and(|p| p.ends_with("pac.txt")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_script_validation_missing_file() {
        let _guard = crate::api::engine::engine_test_lock();
        reset_platform_service_for_test();
        let result = validate_custom_proxy_script(Some("Z:\\nope.pac".to_string()), None);
        assert!(!result.ok);
        assert_eq!(result.error.unwrap().code, "E_NOT_FOUND");
        assert!(validate_custom_proxy_script(None, None).ok);
    }

    #[test]
    fn hotkey_helpers_filter_and_map() {
        assert_eq!(hotkey_proxy_mode(1), Some(SYSPROXY_FORCED_CLEAR));
        assert_eq!(hotkey_proxy_mode(4), Some(SYSPROXY_PAC));
        assert_eq!(hotkey_proxy_mode(0), None);
        assert_eq!(hotkey_proxy_mode(9), None);
        let list = hotkey_list(vec![
            HotkeyDto {
                action: 0,
                alt: true,
                control: false,
                shift: false,
                key_code: Some(70),
            },
            HotkeyDto {
                action: 2,
                alt: false,
                control: false,
                shift: false,
                key_code: None,
            },
        ]);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].action, 0);
    }

    #[test]
    fn mode_and_port_helpers() {
        assert_eq!(sysproxy_mode_value(3), SYSPROXY_PAC);
        assert_eq!(sysproxy_mode_value(99), SYSPROXY_UNCHANGED);
        assert!(sysproxy_type_valid(2));
        assert!(!sysproxy_type_valid(7));
        assert_eq!(local_port_for_protocol(10808, 3), 10811);
        assert_eq!(pac_port_base(), 11808);
    }

    #[test]
    fn resolve_uwp_tool_reports_missing() {
        let result = resolve_uwp_loopback_tool(Some("Z:\\bin".to_string()));
        assert!(!result.ok);
        assert_eq!(result.error.unwrap().code, "E_NOT_FOUND");
    }
}
