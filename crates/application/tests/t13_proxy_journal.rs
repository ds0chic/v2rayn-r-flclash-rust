//! H19 crash-recovery at the service layer: apply persists the ownership
//! journal, and a fresh service reconciles it with one `init_persistence`
//! call. Fake proxy backend plus temp data dirs only; the host proxy,
//! registry and network are never touched.

use std::sync::Arc;

use application::platform_service::{PlatformService, ProxyApplyRequest};
use platform::{
    FakeRegistry, FakeSystemProxyBackend, ProxyField, ProxyState, SysProxyMode, SystemProxyBackend,
};

fn service_on(proxy: Arc<FakeSystemProxyBackend>) -> (PlatformService, Arc<FakeRegistry>) {
    let registry = Arc::new(FakeRegistry::new());
    let service = PlatformService::new(proxy.clone(), registry.clone());
    (service, registry)
}

fn initial_state() -> ProxyState {
    ProxyState {
        enabled: false,
        server: None,
        bypass: None,
        auto_config_url: None,
        auto_detect: false,
    }
}

fn change_request() -> ProxyApplyRequest {
    ProxyApplyRequest {
        mode: SysProxyMode::ForcedChange,
        server: Some("127.0.0.1:11809".to_string()),
        bypass: Some("localhost".to_string()),
        auto_config_url: None,
        auto_detect: None,
    }
}

fn journal_file(data: &std::path::Path) -> std::path::PathBuf {
    data.join(platform::JOURNAL_FILE_NAME)
}

/// A dead owner for tests: no pid is alive under it.
fn nobody_alive(_pid: u32, _created_at_ms: i64) -> bool {
    false
}

#[test]
fn crash_then_restart_recovers_in_one_init_call() {
    let data = tempfile::tempdir().expect("data dir");
    // Session 1 shares the "machine" backend, applies, then crashes: no
    // restore call, the service is simply dropped.
    let machine = Arc::new(FakeSystemProxyBackend::new(initial_state()));
    let (session1, _) = service_on(machine.clone());
    session1
        .init_persistence(data.path(), nobody_alive)
        .expect("init with no journal");
    session1.apply_proxy(&change_request()).expect("apply");
    assert!(
        journal_file(data.path()).is_file(),
        "apply must persist the ownership journal"
    );
    assert_eq!(
        machine.state().server.as_deref(),
        Some("127.0.0.1:11809"),
        "crash leaves the applied value live"
    );
    drop(session1);

    // Session 2 is a fresh process on the same machine and data dir.
    let (session2, _) = service_on(machine.clone());
    let outcome = session2
        .init_persistence(data.path(), nobody_alive)
        .expect("reconcile")
        .expect("a journaled session must reconcile");
    assert!(!outcome.skipped_owner_alive);
    assert!(outcome.ok, "reconcile must succeed");
    assert!(outcome.clean);
    assert_eq!(
        machine.state(),
        initial_state(),
        "one init call restores the pre-apply values"
    );
    assert!(
        !journal_file(data.path()).exists(),
        "a clean reconcile consumes the journal"
    );
    // A repeat init is a silent no-op.
    let repeat = session2
        .init_persistence(data.path(), nobody_alive)
        .expect("repeat");
    assert!(repeat.is_none());
}

#[test]
fn live_owner_skips_restore_and_keeps_journal() {
    let data = tempfile::tempdir().expect("data dir");
    let machine = Arc::new(FakeSystemProxyBackend::new(initial_state()));
    let (session1, _) = service_on(machine.clone());
    session1
        .init_persistence(data.path(), nobody_alive)
        .expect("init");
    session1.apply_proxy(&change_request()).expect("apply");
    drop(session1);

    let (session2, _) = service_on(machine.clone());
    let outcome = session2
        .init_persistence(data.path(), |_, _| true)
        .expect("reconcile")
        .expect("outcome");
    assert!(outcome.skipped_owner_alive);
    assert_eq!(
        machine.state().server.as_deref(),
        Some("127.0.0.1:11809"),
        "a live owner's values must not be touched"
    );
    assert!(
        journal_file(data.path()).is_file(),
        "the journal stays for the live owner"
    );
}

#[test]
fn user_edit_after_crash_is_conflict_not_restore() {
    let data = tempfile::tempdir().expect("data dir");
    let machine = Arc::new(FakeSystemProxyBackend::new(initial_state()));
    let (session1, _) = service_on(machine.clone());
    session1
        .init_persistence(data.path(), nobody_alive)
        .expect("init");
    session1.apply_proxy(&change_request()).expect("apply");
    drop(session1);

    // The user reconfigures the proxy by hand before the next start.
    machine
        .set_field(ProxyField::Server, Some("user:9999"))
        .expect("user edit");

    let (session2, _) = service_on(machine.clone());
    let outcome = session2
        .init_persistence(data.path(), nobody_alive)
        .expect("reconcile")
        .expect("outcome");
    assert!(outcome.ok);
    assert!(!outcome.clean);
    assert_eq!(
        machine.state().server.as_deref(),
        Some("user:9999"),
        "externally modified fields are never written back"
    );
}

#[test]
fn partial_restore_failure_is_reported_and_retained() {
    let data = tempfile::tempdir().expect("data dir");
    let machine = Arc::new(FakeSystemProxyBackend::new(initial_state()));
    let (service, _) = service_on(machine.clone());
    service
        .init_persistence(data.path(), nobody_alive)
        .expect("init");
    service.apply_proxy(&change_request()).expect("apply");
    // Fail the second owned write (Server restores, Bypass fails).
    machine.fail_on_field(ProxyField::Bypass);
    let outcome = service.restore_proxy().expect("restore reports");
    assert!(!outcome.ok, "a failed field must flip ok");
    assert_eq!(outcome.restored.failed_fields(), vec![ProxyField::Bypass]);
    assert_eq!(
        machine.state().server.as_deref(),
        None,
        "the field restored before the failure keeps its original value"
    );
    assert_eq!(
        machine.state().bypass.as_deref(),
        Some("localhost"),
        "the failed field still carries our value"
    );
}
