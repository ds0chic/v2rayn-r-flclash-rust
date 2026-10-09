//! T13 platform orchestration: system proxy (four modes + ownership restore),
//! PAC service lifecycle, autostart, and custom proxy-script validation.
//!
//! This module is the *pure* coordination layer the Flutter bridge calls into.
//! It owns no OS calls directly: every side effect goes through a
//! [`SystemProxyBackend`] / [`AutoStartBackend`] trait object. Production wires
//! the real Windows backends (compile-only there, real writes happen only on
//! explicit user action); tests inject the in-memory fakes, so no automated
//! test ever mutates the host proxy or the Run key.
//!
//! Ownership follows plan section 13 and `docs/decisions/T13-platform.md`:
//! capture the pre-apply value per field, record only the fields this
//! application actually wrote, and on exit restore a field only while its
//! current value still equals the value we wrote. Externally modified fields
//! are kept and surfaced as conflicts.

use std::sync::{Arc, Mutex};

use platform::pac::DEFAULT_PAC_PATH;
use platform::{
    restore_if_owned, AppliedChange, AutoStartBackend, CustomSystemProxySetting, PacConfig,
    PacServer, PacSource, ProxyField, ProxySettings, ProxyState, RestoreReport, SysProxyMode,
    SystemProxyBackend, DEFAULT_PAC_PORT_BASE,
};

/// Port arithmetic base for derived local ports (upstream `AppManager.GetLocalPort`):
/// `port + <EInboundProtocol value>`. Socks=0, Pac=3, etc.
pub const INBOUND_PROTOCOL_OFFSETS: [(&str, u16); 3] = [("socks", 0), ("pac", 3), ("mixed", 6)];

/// Compute a derived local port from the base inbound port, mirroring upstream
/// `GetLocalPort` (`base + protocol value`). Never returns 10808 unless the
/// caller explicitly passes it as the base.
pub fn derived_local_port(base: i32, offset: u16) -> u16 {
    let port = base.max(1) as u32 + offset as u32;
    port.min(u16::MAX as u32) as u16
}

/// A requested proxy transition plus the runtime context needed to compute the
/// per-mode target values.
#[derive(Debug, Clone)]
pub struct ProxyApplyRequest {
    pub mode: SysProxyMode,
    /// Named proxy server (`127.0.0.1:<socks port>`); only used by
    /// `ForcedChange`. Callers must never use 10808 unless the user's live
    /// inbound really is on that port.
    pub server: Option<String>,
    /// Proxy bypass/exception list.
    pub bypass: Option<String>,
    /// PAC URL; only used by [`SysProxyMode::Pac`].
    pub auto_config_url: Option<String>,
    pub auto_detect: Option<bool>,
}

impl Default for ProxyApplyRequest {
    fn default() -> Self {
        Self {
            mode: SysProxyMode::Unchanged,
            server: None,
            bypass: None,
            auto_config_url: None,
            auto_detect: None,
        }
    }
}

impl ProxyApplyRequest {
    pub fn settings(&self) -> ProxySettings {
        ProxySettings {
            server: self.server.clone(),
            bypass: self.bypass.clone(),
            auto_config_url: self.auto_config_url.clone(),
            auto_detect: self.auto_detect,
        }
    }
}

/// Snapshot+apply result exposed over the bridge.
///
/// This outcome reports *platform effect facts only*: it never claims the
/// settings save succeeded. Callers copy `applied_content_hash` into the save
/// receipt's `platformApply` phase (SP-12 contract) while the `save` phase
/// comes solely from the commit path. `ok == true` means the OS effect is in
/// place (or was already in place and skipped as a duplicate), never that a
/// document was persisted.
#[derive(Debug, Clone, Default)]
pub struct ProxyApplyOutcome {
    pub ok: bool,
    pub mode: i32,
    pub applied: Vec<AppliedChange>,
    pub current: ProxyState,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    /// Normalised desired content identity for this request (SP-15).
    pub desired_content_hash: String,
    /// Last successfully applied content identity after this call. Unchanged
    /// when this call failed or was skipped, so `desired != applied` means a
    /// retry is still due.
    pub applied_content_hash: String,
    /// True when the desired content already matched the applied effect and
    /// the live state still carries it: no OS write happened.
    pub skipped_as_duplicate: bool,
}

/// Desired-vs-actually-applied platform content (SP-15 status query).
#[derive(Debug, Clone, Default)]
pub struct PlatformApplyStatus {
    pub desired_content_hash: Option<String>,
    pub applied_content_hash: Option<String>,
    /// True when a desired effect exists that was never successfully applied
    /// (e.g. the OS write failed): retry the same saved request.
    pub needs_retry: bool,
}

/// Restore/exit outcome exposed over the bridge.
#[derive(Debug, Clone, Default)]
pub struct ProxyRestoreOutcome {
    pub ok: bool,
    pub restored: RestoreReport,
    /// True when every restored/conflict field is settled (no conflicts).
    pub clean: bool,
}

/// Who wrote the field currently owning a proxy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// No apply has run in this process, or the field was never changed by us.
    None,
    /// We wrote the current value and it is still ours to restore.
    Ours,
    /// The field was changed after we wrote it; it is no longer ours.
    ExternallyModified,
}

/// Desired-vs-applied proxy state for the status bar.
#[derive(Debug, Clone, Default)]
pub struct ProxyStateView {
    pub desired_mode: i32,
    pub current: ProxyState,
    /// Whether the PAC server is currently serving.
    pub pac_running: bool,
    pub pac_url: Option<String>,
    pub pac_port: Option<u16>,
    /// True when at least one field is still owned by this process.
    pub has_ownership: bool,
    pub conflicts: Vec<String>,
}

/// The stateful platform service. One instance per process; cloned handles
/// share the underlying ownership ledger and PAC server.
#[derive(Clone)]
pub struct PlatformService {
    proxy: Arc<dyn SystemProxyBackend + Send + Sync>,
    autostart: Arc<dyn AutoStartBackend + Send + Sync>,
    applied: Arc<Mutex<Vec<AppliedChange>>>,
    pac: Arc<Mutex<Option<PacServer>>>,
    /// PAC port chosen by the service (0 = auto-select at/above 11808).
    pac_port: Arc<Mutex<u16>>,
    /// md5 of the last `pac_start` payload (script text + proxy rule).
    /// Included in the SP-15 content identity so same mode/port/URL with a
    /// different served script re-applies. Remote PAC bodies (URL not served
    /// by us) are not tracked; URL edits still re-apply via `auto_config_url`.
    pac_content: Arc<Mutex<Option<String>>>,
    /// Desired content identity of the last apply request (SP-15).
    desired_hash: Arc<Mutex<Option<String>>>,
    /// Content identity of the last *successful* OS apply. Only success
    /// advances this; failures leave it so the gap stays retryable.
    applied_hash: Arc<Mutex<Option<String>>>,
}

impl PlatformService {
    pub fn new(
        proxy: Arc<dyn SystemProxyBackend + Send + Sync>,
        autostart: Arc<dyn AutoStartBackend + Send + Sync>,
    ) -> Self {
        Self {
            proxy,
            autostart,
            applied: Arc::new(Mutex::new(Vec::new())),
            pac: Arc::new(Mutex::new(None)),
            pac_port: Arc::new(Mutex::new(0)),
            pac_content: Arc::new(Mutex::new(None)),
            desired_hash: Arc::new(Mutex::new(None)),
            applied_hash: Arc::new(Mutex::new(None)),
        }
    }

    /// Content identity for a request (SP-15 dedup key): normalised
    /// mode/session/port/bypass/PAC segments. `session_key` scopes the effect
    /// owner; pass `""` while no runtime session wiring exists (the bridge
    /// entry keeps one process-wide service, so `""` is stable there).
    /// `pac_content_hash` carries the served script identity (`None` keeps the
    /// legacy URL-only identity).
    pub fn desired_hash_for(
        request: &ProxyApplyRequest,
        session_key: &str,
        pac_content_hash: Option<&str>,
    ) -> String {
        platform::sysproxy::applied_content_hash_with_pac(
            request.mode,
            &request.settings(),
            session_key,
            pac_content_hash,
        )
    }

    /// Currently served PAC payload identity, if `pac_start` ran.
    fn pac_content_hash(&self) -> Option<String> {
        self.pac_content.lock().ok().and_then(|g| g.clone())
    }

    fn record_pac_content(&self, source_text: Option<&str>, proxy_rule: Option<&str>) {
        let hash = source_text.map(|text| {
            let mut material = text.to_string();
            material.push('|');
            material.push_str(proxy_rule.unwrap_or(""));
            platform::hash::md5_hex(material.as_bytes())
        });
        if let Ok(mut guard) = self.pac_content.lock() {
            *guard = hash;
        }
    }

    /// Desired content identity of the last apply request, if any.
    pub fn desired_content_hash(&self) -> Option<String> {
        self.desired_hash.lock().ok().and_then(|g| g.clone())
    }

    /// Content identity of the last *successful* OS apply, if any.
    pub fn applied_content_hash_state(&self) -> Option<String> {
        self.applied_hash.lock().ok().and_then(|g| g.clone())
    }

    /// Desired-vs-applied status: `needs_retry` is true while a desired effect
    /// was never successfully applied (failed OS write). Retrying re-issues
    /// the same saved request; it never invents a new save.
    pub fn apply_status(&self) -> PlatformApplyStatus {
        let desired = self.desired_content_hash();
        let applied = self.applied_content_hash_state();
        let needs_retry = match (&desired, &applied) {
            (Some(_), None) => true,
            (Some(d), Some(a)) => d != a,
            (None, _) => false,
        };
        PlatformApplyStatus {
            desired_content_hash: desired,
            applied_content_hash: applied,
            needs_retry,
        }
    }

    /// True while a desired effect still needs a (re-)apply.
    pub fn needs_retry(&self) -> bool {
        self.apply_status().needs_retry
    }

    /// The current per-field system proxy state (through the backend).
    pub fn snapshot(&self) -> Result<ProxyState, platform::PlatformError> {
        self.proxy.snapshot()
    }

    /// Apply a proxy transition, recording only the fields we actually wrote.
    ///
    /// SP-15 content semantics:
    /// * the request's normalised content hash (mode/session/port/bypass/PAC)
    ///   is recorded as desired *before* touching the OS;
    /// * an identical repeat while the live state still carries our values is
    ///   deduplicated: no OS write, `skipped_as_duplicate` is true;
    /// * same mode/port with different bypass/PAC content hashes differently
    ///   and re-applies;
    /// * a failed OS write propagates the error and never advances the applied
    ///   hash, so `needs_retry()` stays true until the same saved request is
    ///   retried successfully. The outcome reports platform effect only, never
    ///   settings-save success.
    ///
    /// A repeated apply first resets the ledger: fields we previously owned are
    /// re-evaluated so the ledger reflects the *current* write set. Fields the
    /// user changed in between are dropped from the ledger (they are no longer
    /// ours to restore).
    pub fn apply_proxy(
        &self,
        request: &ProxyApplyRequest,
    ) -> Result<ProxyApplyOutcome, platform::PlatformError> {
        let desired = Self::desired_hash_for(request, "", self.pac_content_hash().as_deref());
        if request.mode == SysProxyMode::Unchanged {
            return Ok(self.unchanged_outcome(request.mode, desired));
        }
        self.record_desired(&desired);
        if self.dedup_hit(&desired) {
            return Ok(ProxyApplyOutcome {
                ok: true,
                mode: mode_value(request.mode),
                applied: Vec::new(),
                current: self.proxy.snapshot()?,
                error_code: None,
                error_message: None,
                desired_content_hash: desired,
                applied_content_hash: self.applied_content_hash_state().unwrap_or_default(),
                skipped_as_duplicate: true,
            });
        }
        let before = self.proxy.snapshot()?;
        // Classify any pre-existing ledger entries against the state just
        // before this apply: a field the user changed in between is dropped so
        // we never restore over a user edit; a field still carrying our value
        // keeps its *original* `before` (the true session origin).
        let current_before: Vec<AppliedChange> =
            self.applied.lock().map(|g| g.clone()).unwrap_or_default();
        let report = restore_if_owned(&current_before, &before);
        // The OS write below may fail partway: the applied hash advances only
        // after it succeeds, so a failure stays honestly retryable.
        let changes = self.proxy.apply(request.mode, &request.settings())?;
        {
            let mut ledger = self.applied.lock().map_err(|_| {
                platform::PlatformError::Backend("proxy ledger poisoned".to_string())
            })?;
            // Fields the user modified before this apply are no longer ours.
            ledger.retain(|change| {
                report
                    .conflicts
                    .iter()
                    .all(|conflict| conflict.field != change.field)
            });
            // Merge the transitions we just wrote. If we already own a field,
            // keep its original `before` and update `after` to the new value.
            for change in &changes {
                let origin = ledger
                    .iter()
                    .find(|existing| existing.field == change.field)
                    .map(|existing| existing.before.clone())
                    .unwrap_or_else(|| change.before.clone());
                ledger.retain(|existing| existing.field != change.field);
                ledger.push(AppliedChange {
                    field: change.field,
                    before: origin,
                    after: change.after.clone(),
                });
            }
        }
        self.record_applied(&desired);
        Ok(ProxyApplyOutcome {
            ok: true,
            mode: mode_value(request.mode),
            applied: changes,
            current: self.proxy.snapshot()?,
            error_code: None,
            error_message: None,
            desired_content_hash: desired.clone(),
            applied_content_hash: desired,
            skipped_as_duplicate: false,
        })
    }

    fn record_desired(&self, desired: &str) {
        if let Ok(mut guard) = self.desired_hash.lock() {
            *guard = Some(desired.to_string());
        }
    }

    fn record_applied(&self, desired: &str) {
        if let Ok(mut guard) = self.applied_hash.lock() {
            *guard = Some(desired.to_string());
        }
    }

    /// True when `desired` equals the last successful apply *and* the live
    /// state still carries every owned value. An externally modified or
    /// restored field forces a real re-apply instead of masking the drift.
    fn dedup_hit(&self, desired: &str) -> bool {
        if self.applied_content_hash_state().as_deref() != Some(desired) {
            return false;
        }
        let (ledger, current) = match (self.applied.lock(), self.proxy.snapshot()) {
            (Ok(ledger), Ok(current)) => (ledger.clone(), current),
            _ => return false,
        };
        if ledger.is_empty() {
            return true;
        }
        ledger
            .iter()
            .all(|change| current.field(change.field) == change.after)
    }

    fn unchanged_outcome(&self, mode: SysProxyMode, desired: String) -> ProxyApplyOutcome {
        let applied_state = self.applied_content_hash_state();
        let skipped = applied_state.as_deref() == Some(desired.as_str());
        if !skipped {
            self.record_desired(&desired);
            self.record_applied(&desired);
        }
        ProxyApplyOutcome {
            ok: true,
            mode: mode_value(mode),
            applied: Vec::new(),
            current: self.proxy.snapshot().unwrap_or_default(),
            error_code: None,
            error_message: None,
            applied_content_hash: self.applied_content_hash_state().unwrap_or_default(),
            desired_content_hash: desired,
            skipped_as_duplicate: skipped,
        }
    }

    /// Restore only the fields this process still owns. Idempotent.
    pub fn restore_proxy(&self) -> Result<ProxyRestoreOutcome, platform::PlatformError> {
        let current = self.proxy.snapshot()?;
        let applied = self.applied.lock().map(|g| g.clone()).unwrap_or_default();
        let report = restore_if_owned(&applied, &current);
        for action in &report.restored {
            self.proxy
                .set_field(action.field, action.restore_to.as_deref())?;
        }
        if !report.restored.is_empty() {
            self.proxy.notify_changed()?;
        }
        // Keep only the fields that are still owned (finished restores drop out;
        // conflicts stay visible until the caller acknowledges them).
        if let Ok(mut ledger) = self.applied.lock() {
            let conflicted: Vec<ProxyField> = report.conflicts.iter().map(|c| c.field).collect();
            ledger.retain(|change| conflicted.contains(&change.field));
        }
        Ok(ProxyRestoreOutcome {
            ok: true,
            clean: report.is_clean(),
            restored: report,
        })
    }

    /// Exit-path restore.
    ///
    /// Upstream `AppManager.AppExitAsync` calls
    /// `SysProxyHandler.UpdateSysProxy(_config, forceDisable: true)`, which
    /// turns any non-`Unchanged` mode into `ForcedClear` and writes it. Plan
    /// section 13 tightens this to per-field ownership: we restore only the
    /// fields this process still owns, and a field the user (or another tool)
    /// changed in the meantime is left as-is and reported as a conflict.
    ///
    /// `Unchanged` is a no-op apart from the idempotent safety restore.
    pub fn restore_on_exit(
        &self,
        _desired_mode: SysProxyMode,
    ) -> Result<ProxyRestoreOutcome, platform::PlatformError> {
        // Per-field ownership already covers both the "still ours" and
        // "externally modified" cases, so the exit path is the same restore.
        self.restore_proxy()
    }

    /// Current ownership classification per field.
    pub fn ownership(&self) -> Result<Vec<(ProxyField, Ownership)>, platform::PlatformError> {
        let current = self.proxy.snapshot()?;
        let applied = self.applied.lock().map(|g| g.clone()).unwrap_or_default();
        let mut result = Vec::new();
        for field in ProxyField::ALL {
            let entry = applied.iter().find(|c| c.field == field);
            let ownership = match entry {
                None => Ownership::None,
                Some(change) => {
                    if current.field(field) == change.after {
                        Ownership::Ours
                    } else {
                        Ownership::ExternallyModified
                    }
                }
            };
            result.push((field, ownership));
        }
        Ok(result)
    }

    // -- PAC service lifecycle ---------------------------------------------

    /// Start (or refresh) the loopback PAC server. `port` 0 auto-selects at or
    /// above [`DEFAULT_PAC_PORT_BASE`] (11808). Running repeats only refresh
    /// content and keep the listener/port (idempotent).
    pub fn pac_start(
        &self,
        source: PacSource,
        proxy_rule: Option<String>,
        port: u16,
    ) -> Result<PacHandle, platform::PlatformError> {
        if port != 0 && port < DEFAULT_PAC_PORT_BASE {
            return Err(platform::PlatformError::Invalid(format!(
                "PAC port {port} is below the reserved base {DEFAULT_PAC_PORT_BASE}"
            )));
        }
        let mut guard = self
            .pac
            .lock()
            .map_err(|_| platform::PlatformError::Backend("pac slot poisoned".to_string()))?;
        // Identity input for SP-15: script text plus the requested rule.
        let text = source.read().ok();
        if let Some(server) = guard.as_mut() {
            if server.is_running() {
                // Refresh content with the requested rule (a new local port or
                // protocol must reach the served script); port stays.
                server.set_proxy_rule(proxy_rule.clone());
                server.start(source)?;
                self.record_pac_content(text.as_deref(), proxy_rule.as_deref());
                if let Ok(mut chosen) = self.pac_port.lock() {
                    *chosen = server.port().unwrap_or(*chosen);
                }
                return Ok(self.pac_handle(guard.as_ref()));
            }
        }
        let config = PacConfig {
            host: "127.0.0.1".to_string(),
            port,
            path: DEFAULT_PAC_PATH.to_string(),
            proxy_rule: proxy_rule.clone(),
        };
        let mut server = PacServer::new(config)?;
        let bound = server.start(source)?;
        self.record_pac_content(text.as_deref(), proxy_rule.as_deref());
        if let Ok(mut chosen) = self.pac_port.lock() {
            *chosen = bound;
        }
        *guard = Some(server);
        Ok(self.pac_handle(guard.as_ref()))
    }

    fn pac_handle(&self, server: Option<&PacServer>) -> PacHandle {
        match server {
            Some(server) => PacHandle {
                running: server.is_running(),
                url: server.url(),
                port: server.port(),
            },
            None => PacHandle::default(),
        }
    }

    /// Stop the PAC server. Idempotent.
    pub fn pac_stop(&self) -> Result<(), platform::PlatformError> {
        let mut guard = self
            .pac
            .lock()
            .map_err(|_| platform::PlatformError::Backend("pac slot poisoned".to_string()))?;
        if let Some(server) = guard.as_mut() {
            server.stop()?;
        }
        Ok(())
    }

    /// The live PAC handle, if a server object exists.
    pub fn pac_state(&self) -> PacHandle {
        let guard = self.pac.lock();
        match guard {
            Ok(guard) => self.pac_handle(guard.as_ref()),
            Err(_) => PacHandle::default(),
        }
    }

    // -- Autostart ----------------------------------------------------------

    /// Whether the Run value for `name` exists and is non-empty.
    pub fn autostart_enabled(&self, name: &str) -> Result<bool, platform::PlatformError> {
        self.autostart.is_enabled(name)
    }

    /// Enable/disable autostart. Real writes happen only when the caller
    /// explicitly calls this with a user action behind it.
    pub fn set_autostart(
        &self,
        name: &str,
        enabled: bool,
        exe: &str,
        args: &str,
    ) -> Result<(), platform::PlatformError> {
        if enabled {
            self.autostart.enable(name, exe, args)
        } else {
            self.autostart.disable(name)
        }
    }

    // -- Custom proxy script validation ------------------------------------

    /// Validate the two custom system-proxy paths (existence only). Never
    /// executes the script.
    pub fn validate_custom_script(
        &self,
        setting: &CustomSystemProxySetting,
    ) -> Result<(), platform::PlatformError> {
        setting.validate()
    }

    /// Fully resolved view for the status bar/bridge.
    pub fn state_view(&self, desired_mode: SysProxyMode) -> ProxyStateView {
        let current = self.proxy.snapshot().unwrap_or_default();
        let pac = self.pac_state();
        let ownership = self.ownership().unwrap_or_default();
        ProxyStateView {
            desired_mode: mode_value(desired_mode),
            has_ownership: ownership.iter().any(|(_, o)| *o == Ownership::Ours),
            conflicts: ownership
                .iter()
                .filter(|(_, o)| *o == Ownership::ExternallyModified)
                .map(|(f, _)| format!("{f:?}"))
                .collect(),
            current,
            pac_running: pac.running,
            pac_url: pac.url,
            pac_port: pac.port,
        }
    }
}

/// A PAC server handle snapshot.
#[derive(Debug, Clone, Default)]
pub struct PacHandle {
    pub running: bool,
    pub url: Option<String>,
    pub port: Option<u16>,
}

/// Numeric `ESysProxyType` value.
pub fn mode_value(mode: SysProxyMode) -> i32 {
    match mode {
        SysProxyMode::ForcedClear => 0,
        SysProxyMode::ForcedChange => 1,
        SysProxyMode::Unchanged => 2,
        SysProxyMode::Pac => 3,
    }
}

/// Parse an `ESysProxyType` numeric value into the platform mode.
pub fn mode_from_value(value: i32) -> Option<SysProxyMode> {
    Some(match value {
        0 => SysProxyMode::ForcedClear,
        1 => SysProxyMode::ForcedChange,
        2 => SysProxyMode::Unchanged,
        3 => SysProxyMode::Pac,
        _ => return None,
    })
}

/// Convert the domain `SysProxyType` into the platform mode so the two crates
/// never drift. Kept here (not in `platform`) so the platform crate stays free
/// of domain coupling.
pub fn mode_from_domain(value: domain::SysProxyType) -> SysProxyMode {
    match value {
        domain::SysProxyType::ForcedClear => SysProxyMode::ForcedClear,
        domain::SysProxyType::ForcedChange => SysProxyMode::ForcedChange,
        domain::SysProxyType::Unchanged => SysProxyMode::Unchanged,
        domain::SysProxyType::Pac => SysProxyMode::Pac,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform::FakeRegistry;
    use platform::{FakeSystemProxyBackend, ProxyState};

    fn service_with(initial: ProxyState) -> (PlatformService, Arc<FakeSystemProxyBackend>) {
        let proxy = Arc::new(FakeSystemProxyBackend::new(initial));
        let registry = Arc::new(FakeRegistry::new());
        let service = PlatformService::new(proxy.clone(), registry);
        (service, proxy)
    }

    fn forced_change(server: &str, bypass: &str) -> ProxyApplyRequest {
        ProxyApplyRequest {
            mode: SysProxyMode::ForcedChange,
            server: Some(server.to_string()),
            bypass: Some(bypass.to_string()),
            auto_config_url: None,
            auto_detect: None,
        }
    }

    #[test]
    fn forced_change_sets_enabled_server_bypass() {
        let (service, fake) = service_with(ProxyState::default());
        let out = service
            .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
            .unwrap();
        assert!(out.ok);
        let state = fake.state();
        assert!(state.enabled);
        assert_eq!(state.server.as_deref(), Some("127.0.0.1:10809"));
        assert_eq!(state.bypass.as_deref(), Some("<local>"));
        assert_eq!(state.auto_config_url, None);
    }

    #[test]
    fn forced_clear_clears_everything() {
        let initial = ProxyState {
            enabled: true,
            server: Some("127.0.0.1:10809".into()),
            bypass: Some("<local>".into()),
            auto_config_url: Some("http://127.0.0.1:10809/pac".into()),
            auto_detect: false,
        };
        let (service, fake) = service_with(initial);
        let out = service
            .apply_proxy(&ProxyApplyRequest {
                mode: SysProxyMode::ForcedClear,
                ..Default::default()
            })
            .unwrap();
        assert!(out.ok);
        let state = fake.state();
        assert!(!state.enabled);
        assert_eq!(state.server, None);
        assert_eq!(state.bypass, None);
        assert_eq!(state.auto_config_url, None);
    }

    #[test]
    fn pac_requires_url_and_sets_autoconfig() {
        let (service, fake) = service_with(ProxyState::default());
        let missing = service
            .apply_proxy(&ProxyApplyRequest {
                mode: SysProxyMode::Pac,
                ..Default::default()
            })
            .unwrap_err();
        assert!(matches!(missing, platform::PlatformError::Invalid(_)));
        // Now with a URL.
        let out = service
            .apply_proxy(&ProxyApplyRequest {
                mode: SysProxyMode::Pac,
                auto_config_url: Some("http://127.0.0.1:11808/pac".into()),
                ..Default::default()
            })
            .unwrap();
        assert!(out.ok);
        let state = fake.state();
        assert!(!state.enabled);
        assert_eq!(
            state.auto_config_url.as_deref(),
            Some("http://127.0.0.1:11808/pac")
        );
    }

    #[test]
    fn unchanged_writes_nothing_and_records_no_ownership() {
        let (service, fake) = service_with(ProxyState::default());
        let out = service
            .apply_proxy(&ProxyApplyRequest {
                mode: SysProxyMode::Unchanged,
                ..Default::default()
            })
            .unwrap();
        assert!(out.ok);
        assert!(out.applied.is_empty());
        assert!(fake.writes().is_empty());
        assert!(service
            .ownership()
            .unwrap()
            .iter()
            .all(|(_, o)| *o == Ownership::None));
    }

    #[test]
    fn restore_returns_to_original_when_still_owned() {
        let initial = ProxyState {
            enabled: false,
            server: Some("orig:1".into()),
            ..Default::default()
        };
        let (service, fake) = service_with(initial);
        service
            .apply_proxy(&forced_change("127.0.0.1:10809", ""))
            .unwrap();
        assert_eq!(fake.state().server.as_deref(), Some("127.0.0.1:10809"));
        let restore = service.restore_proxy().unwrap();
        assert!(restore.ok);
        assert!(restore.clean);
        assert_eq!(fake.state().server.as_deref(), Some("orig:1"));
        assert!(!fake.state().enabled);
    }

    #[test]
    fn externally_modified_field_is_kept_and_reported() {
        let (service, fake) = service_with(ProxyState::default());
        service
            .apply_proxy(&forced_change("127.0.0.1:10809", ""))
            .unwrap();
        // User (or another tool) changes the server after our apply.
        fake.set_field(ProxyField::Server, Some("user:9")).unwrap();
        let restore = service.restore_proxy().unwrap();
        assert!(!restore.clean);
        assert_eq!(
            fake.state().server.as_deref(),
            Some("user:9"),
            "user change must not be overwritten"
        );
        assert_eq!(restore.restored.conflicts.len(), 1);
        assert_eq!(restore.restored.conflicts[0].field, ProxyField::Server);
    }

    #[test]
    fn restore_is_idempotent() {
        let (service, _fake) = service_with(ProxyState::default());
        service
            .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
            .unwrap();
        let first = service.restore_proxy().unwrap();
        assert!(first.clean);
        let second = service.restore_proxy().unwrap();
        assert!(second.clean);
        assert!(second.restored.restored.is_empty());
        assert!(second.restored.conflicts.is_empty());
    }

    #[test]
    fn repeated_apply_keeps_only_current_write_set() {
        let (service, _fake) = service_with(ProxyState::default());
        service
            .apply_proxy(&forced_change("127.0.0.1:1", ""))
            .unwrap();
        service
            .apply_proxy(&forced_change("127.0.0.1:2", ""))
            .unwrap();
        let owned: Vec<ProxyField> = service
            .ownership()
            .unwrap()
            .into_iter()
            .filter(|(_, o)| *o == Ownership::Ours)
            .map(|(f, _)| f)
            .collect();
        assert!(owned.contains(&ProxyField::Server));
        assert!(owned.contains(&ProxyField::Enabled));
    }

    #[test]
    fn exit_restore_from_unchanged_is_safe() {
        let (service, _fake) = service_with(ProxyState::default());
        let restore = service.restore_on_exit(SysProxyMode::Unchanged).unwrap();
        assert!(restore.ok);
        assert!(restore.clean);
    }

    #[test]
    fn exit_restore_force_clears_non_unchanged_mode() {
        let (service, fake) = service_with(ProxyState::default());
        service
            .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
            .unwrap();
        let restore = service.restore_on_exit(SysProxyMode::ForcedChange).unwrap();
        assert!(restore.ok);
        // Server restored to its pre-apply (absent) value.
        assert_eq!(fake.state().server, None);
        assert!(!fake.state().enabled);
    }

    #[test]
    fn mode_helpers_round_trip() {
        for value in 0..=3 {
            let mode = mode_from_value(value).unwrap();
            assert_eq!(mode_value(mode), value);
        }
        assert!(mode_from_value(9).is_none());
    }

    #[test]
    fn derived_ports_match_upstream_offsets() {
        assert_eq!(derived_local_port(10808, 0), 10808);
        assert_eq!(derived_local_port(10808, 3), 10811);
        assert_eq!(derived_local_port(10808, 6), 10814);
    }

    #[test]
    fn autostart_toggle_writes_and_removes() {
        let registry = Arc::new(FakeRegistry::new());
        let proxy = Arc::new(FakeSystemProxyBackend::default());
        let service = PlatformService::new(proxy, registry.clone());
        assert!(!service.autostart_enabled("v2rayNAutoRun_x").unwrap());
        service
            .set_autostart("v2rayNAutoRun_x", true, r"C:\app\v2rayn.exe", "")
            .unwrap();
        assert!(service.autostart_enabled("v2rayNAutoRun_x").unwrap());
        service
            .set_autostart("v2rayNAutoRun_x", false, "", "")
            .unwrap();
        assert!(!service.autostart_enabled("v2rayNAutoRun_x").unwrap());
    }

    #[test]
    fn pac_rejects_port_below_reserved_base() {
        let (service, _fake) = service_with(ProxyState::default());
        let err = service
            .pac_start(
                PacSource::Inline("function FindProxyForURL(){}".into()),
                None,
                10808,
            )
            .unwrap_err();
        assert!(matches!(err, platform::PlatformError::Invalid(_)));
    }
}
