//! SP-15 platform effect by content (fake backends only, dry-run on host).
//!
//! Correct contract under test:
//! * identical desired content is deduplicated (no new OS writes, reported as
//!   skipped) while same mode + same port with a different bypass/PAC payload
//!   re-applies;
//! * desired and actually-applied content are tracked separately; a failed OS
//!   apply never advances the applied hash and the same saved request can be
//!   retried honestly;
//! * a successful platform apply never claims the settings save succeeded
//!   (the outcome carries only platform-effect facts, no revision/token).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use application::platform_service::{PlatformService, ProxyApplyRequest};
use platform::{
    FakeRegistry, FakeSystemProxyBackend, PacSource, PlatformError, ProxyField, ProxyState,
    SysProxyMode, SystemProxyBackend,
};

/// Fake backend wrapper that fails `set_field` while `fail_writes` is set,
/// simulating a failed OS write (WinINET/registry) without touching the host.
struct FlakyBackend {
    inner: FakeSystemProxyBackend,
    fail_writes: AtomicBool,
}

impl FlakyBackend {
    fn new() -> Self {
        Self {
            inner: FakeSystemProxyBackend::new(ProxyState::default()),
            fail_writes: AtomicBool::new(false),
        }
    }

    fn state(&self) -> ProxyState {
        self.inner.state()
    }

    fn writes(&self) -> Vec<(ProxyField, Option<String>)> {
        self.inner.writes()
    }
}

impl SystemProxyBackend for FlakyBackend {
    fn snapshot(&self) -> Result<ProxyState, PlatformError> {
        self.inner.snapshot()
    }

    fn set_field(&self, field: ProxyField, value: Option<&str>) -> Result<(), PlatformError> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Err(PlatformError::Backend(
                "synthetic os write failure".to_string(),
            ));
        }
        self.inner.set_field(field, value)
    }

    fn notify_changed(&self) -> Result<(), PlatformError> {
        self.inner.notify_changed()
    }
}

fn service_with(
    backend: Arc<FlakyBackend>,
) -> (PlatformService, Arc<FlakyBackend>, Arc<FakeRegistry>) {
    let registry = Arc::new(FakeRegistry::new());
    let service = PlatformService::new(backend.clone(), registry.clone());
    (service, backend, registry)
}

fn forced_change(server: &str, bypass: &str) -> ProxyApplyRequest {
    ProxyApplyRequest {
        mode: SysProxyMode::ForcedChange,
        server: Some(server.to_string()),
        bypass: Some(bypass.to_string()),
        ..Default::default()
    }
}

fn pac_request(url: &str) -> ProxyApplyRequest {
    ProxyApplyRequest {
        mode: SysProxyMode::Pac,
        auto_config_url: Some(url.to_string()),
        ..Default::default()
    }
}

#[test]
fn identical_repeat_apply_is_deduplicated_without_new_writes() {
    let (service, backend, _) = service_with(Arc::new(FlakyBackend::new()));
    let request = forced_change("127.0.0.1:11809", "<local>");

    let first = service.apply_proxy(&request).unwrap();
    assert!(first.ok);
    assert!(!first.skipped_as_duplicate);
    assert!(!first.desired_content_hash.is_empty());
    assert_eq!(first.applied_content_hash, first.desired_content_hash);
    assert!(!service.needs_retry());

    let writes_after_first = backend.writes().len();
    assert!(writes_after_first > 0);

    let second = service.apply_proxy(&request).unwrap();
    assert!(second.ok);
    assert!(
        second.skipped_as_duplicate,
        "identical content must not rewrite the OS"
    );
    assert_eq!(second.desired_content_hash, first.desired_content_hash);
    assert_eq!(second.applied_content_hash, first.applied_content_hash);
    assert_eq!(backend.writes().len(), writes_after_first);
    assert!(!service.needs_retry());
}

#[test]
fn same_port_bypass_change_reapplies() {
    let (service, backend, _) = service_with(Arc::new(FlakyBackend::new()));
    let first = forced_change("127.0.0.1:11809", "<local>");
    let out_first = service.apply_proxy(&first).unwrap();
    let writes_after_first = backend.writes().len();

    let edited = forced_change("127.0.0.1:11809", "<local>;192.0.2.0/24");
    let out_second = service.apply_proxy(&edited).unwrap();
    assert!(!out_second.skipped_as_duplicate);
    assert_ne!(
        out_second.desired_content_hash, out_first.desired_content_hash,
        "same port with different bypass must change the desired identity"
    );
    assert_eq!(
        out_second.applied_content_hash, out_second.desired_content_hash,
        "successful re-apply must advance the applied hash"
    );
    assert!(backend.writes().len() > writes_after_first);
    assert_eq!(
        backend.state().bypass.as_deref(),
        Some("<local>;192.0.2.0/24")
    );
}

#[test]
fn same_url_pac_content_change_reapplies() {
    let (service, _, _) = service_with(Arc::new(FlakyBackend::new()));
    let pac_url = "http://127.0.0.1:11808/pac";
    let rule = Some("PROXY 127.0.0.1:11809".to_string());
    service
        .pac_start(
            PacSource::Inline("function FindProxyForURL(){return \"PROXY a:1\";}".into()),
            rule.clone(),
            0,
        )
        .unwrap();
    let request = pac_request(pac_url);
    let out_first = service.apply_proxy(&request).unwrap();
    assert!(!out_first.skipped_as_duplicate);

    // Same served script again: deduplicated.
    let repeat = service.apply_proxy(&request).unwrap();
    assert!(repeat.skipped_as_duplicate);

    // Same mode/port/URL but a new served PAC script: must re-apply.
    service
        .pac_start(
            PacSource::Inline("function FindProxyForURL(){return \"PROXY a:2\";}".into()),
            rule,
            0,
        )
        .unwrap();
    let out_second = service.apply_proxy(&request).unwrap();
    assert!(
        !out_second.skipped_as_duplicate,
        "same PAC URL with different script must re-apply"
    );
    assert_ne!(
        out_second.desired_content_hash,
        out_first.desired_content_hash
    );
    assert_eq!(
        out_second.applied_content_hash,
        out_second.desired_content_hash
    );
    service.pac_stop().unwrap();
}

#[test]
fn failed_apply_keeps_applied_and_retry_succeeds() {
    let backend = Arc::new(FlakyBackend::new());
    backend.fail_writes.store(true, Ordering::SeqCst);
    let (service, backend, _) = service_with(backend);
    let request = forced_change("127.0.0.1:11809", "<local>");

    let err = service.apply_proxy(&request).unwrap_err();
    assert!(
        matches!(err, PlatformError::Backend(_)),
        "OS failure must surface honestly, got: {err}"
    );
    // Desired is recorded, applied never advanced: retry is due.
    assert!(service.desired_content_hash().is_some());
    assert_eq!(service.applied_content_hash_state(), None);
    assert!(service.needs_retry());
    let status = service.apply_status();
    assert_eq!(status.desired_content_hash, service.desired_content_hash());
    assert_eq!(status.applied_content_hash, None);
    assert!(status.needs_retry);

    // Same saved request retries the platform effect (no new save involved).
    backend.fail_writes.store(false, Ordering::SeqCst);
    let retry = service.apply_proxy(&request).unwrap();
    assert!(retry.ok);
    assert!(!retry.skipped_as_duplicate);
    assert_eq!(
        retry.applied_content_hash, retry.desired_content_hash,
        "retry success must advance applied to desired"
    );
    assert!(!service.needs_retry());
    assert_eq!(backend.state().server.as_deref(), Some("127.0.0.1:11809"));
}

#[test]
fn external_change_after_apply_forces_reapply_not_skip() {
    let (service, backend, _) = service_with(Arc::new(FlakyBackend::new()));
    let request = forced_change("127.0.0.1:11809", "<local>");
    service.apply_proxy(&request).unwrap();

    // External tool edits one owned field afterwards.
    backend
        .inner
        .set_field(ProxyField::Server, Some("user:9"))
        .unwrap();

    let repeat = service.apply_proxy(&request).unwrap();
    assert!(
        !repeat.skipped_as_duplicate,
        "externally modified state must not be masked by dedup"
    );
    assert_eq!(
        backend.state().server.as_deref(),
        Some("127.0.0.1:11809"),
        "re-apply restores the desired effect"
    );
}

#[test]
fn restore_does_not_rewrite_applied_hash() {
    let (service, _, _) = service_with(Arc::new(FlakyBackend::new()));
    let request = forced_change("127.0.0.1:11809", "<local>");
    let applied = service.apply_proxy(&request).unwrap();
    let hash_before_restore = service.applied_content_hash_state();

    let restore = service.restore_proxy().unwrap();
    assert!(restore.ok);
    // Applied still describes the last successful apply; the live OS state is
    // observed via snapshot/ownership instead (desired != actual stays honest).
    assert_eq!(service.applied_content_hash_state(), hash_before_restore);
    assert_eq!(
        hash_before_restore.as_deref(),
        Some(applied.applied_content_hash.as_str())
    );
}
