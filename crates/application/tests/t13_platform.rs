//! T13 application-level platform orchestration tests (fake backends only).
//!
//! Never touches the host system proxy, registry, or ports: the system-proxy
//! and autostart backends are in-memory fakes, and the PAC server binds a
//! loopback port at or above 11808 (never 10808).

use std::sync::Arc;

use application::platform_service::{
    derived_local_port, mode_from_domain, mode_from_value, mode_value, Ownership, PlatformService,
    ProxyApplyRequest,
};
use platform::{
    FakeRegistry, FakeSystemProxyBackend, PacSource, ProxyField, ProxyState, SysProxyMode,
    SystemProxyBackend, DEFAULT_PAC_PORT_BASE,
};

fn service() -> (
    PlatformService,
    Arc<FakeSystemProxyBackend>,
    Arc<FakeRegistry>,
) {
    let proxy = Arc::new(FakeSystemProxyBackend::new(ProxyState::default()));
    let registry = Arc::new(FakeRegistry::new());
    let service = PlatformService::new(proxy.clone(), registry.clone());
    (service, proxy, registry)
}

fn request(mode: SysProxyMode) -> ProxyApplyRequest {
    ProxyApplyRequest {
        mode,
        ..Default::default()
    }
}

fn forced_change(server: &str, bypass: &str) -> ProxyApplyRequest {
    ProxyApplyRequest {
        mode: SysProxyMode::ForcedChange,
        server: Some(server.to_string()),
        bypass: Some(bypass.to_string()),
        ..Default::default()
    }
}

// -- four-mode matrix -------------------------------------------------------

#[test]
fn matrix_forced_clear_then_change_then_restore() {
    let (service, proxy, _reg) = service();
    // Pre-existing user proxy: our clear must not be treated as ownership of
    // the pre-existing values.
    proxy.set_field(ProxyField::Server, Some("pre:1")).unwrap();

    service
        .apply_proxy(&request(SysProxyMode::ForcedClear))
        .unwrap();
    assert_eq!(proxy.state().server, None);

    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    assert!(proxy.state().enabled);
    assert_eq!(proxy.state().server.as_deref(), Some("127.0.0.1:10809"));

    let restored = service.restore_proxy().unwrap();
    assert!(restored.clean);
    // Server returns to `pre:1` (the value before the *change* apply), enabled
    // returns to false (the value before the change, after the clear).
    assert_eq!(proxy.state().server.as_deref(), Some("pre:1"));
    assert!(!proxy.state().enabled);
}

#[test]
fn matrix_pac_sets_url_and_clears_named_proxy() {
    let (service, proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    service
        .apply_proxy(&ProxyApplyRequest {
            mode: SysProxyMode::Pac,
            auto_config_url: Some("http://127.0.0.1:11808/pac".into()),
            ..Default::default()
        })
        .unwrap();
    let state = proxy.state();
    assert!(!state.enabled);
    assert_eq!(state.server, None);
    assert_eq!(state.bypass, None);
    assert_eq!(
        state.auto_config_url.as_deref(),
        Some("http://127.0.0.1:11808/pac")
    );
}

#[test]
fn matrix_unchanged_is_a_total_noop() {
    let (service, proxy, _reg) = service();
    proxy.set_field(ProxyField::Server, Some("pre:1")).unwrap();
    let writes_before = proxy.writes().len();
    let before = proxy.state();
    let out = service
        .apply_proxy(&request(SysProxyMode::Unchanged))
        .unwrap();
    assert!(out.ok);
    assert_eq!(proxy.state(), before);
    assert_eq!(proxy.writes().len(), writes_before);
    assert!(service
        .ownership()
        .unwrap()
        .iter()
        .all(|(_, o)| *o == Ownership::None));
}

// -- conflict matrix --------------------------------------------------------

#[test]
fn conflict_user_changes_one_field_others_still_restored() {
    let (service, proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    // User edits the server only.
    proxy.set_field(ProxyField::Server, Some("user:9")).unwrap();
    let restored = service.restore_proxy().unwrap();
    assert!(!restored.clean);
    // Server kept; bypass restored to its pre-apply (absent) value.
    assert_eq!(proxy.state().server.as_deref(), Some("user:9"));
    assert_eq!(proxy.state().bypass, None);
    assert_eq!(restored.restored.conflicts.len(), 1);
    assert_eq!(restored.restored.conflicts[0].field, ProxyField::Server);
}

#[test]
fn conflict_every_text_field_changed_reports_all_and_writes_nothing() {
    let (service, proxy, _reg) = service();
    // ForcedChange owns Enabled/Server/Bypass. Server and Bypass have distinct
    // origins ("pre:1"/"pre:2"), so both can become genuine conflicts.
    proxy.set_field(ProxyField::Server, Some("pre:1")).unwrap();
    proxy.set_field(ProxyField::Bypass, Some("pre:2")).unwrap();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    // Change both owned text fields externally to a third value.
    proxy.set_field(ProxyField::Server, Some("user:1")).unwrap();
    proxy.set_field(ProxyField::Bypass, Some("user:2")).unwrap();
    let writes_before = proxy.writes().len();
    let restored = service.restore_proxy().unwrap();
    assert!(!restored.clean);
    // Only the still-owned Enabled field is restored; the two text fields are
    // left untouched.
    assert_eq!(proxy.writes().len(), writes_before + 1);
    assert_eq!(restored.restored.restored.len(), 1);
    assert_eq!(restored.restored.restored[0].field, ProxyField::Enabled);
    assert_eq!(restored.restored.conflicts.len(), 2);
    assert_eq!(proxy.state().server.as_deref(), Some("user:1"));
    assert_eq!(proxy.state().bypass.as_deref(), Some("user:2"));
}

#[test]
fn ownership_classifies_ours_and_externally_modified() {
    let (service, proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    let ours: Vec<ProxyField> = service
        .ownership()
        .unwrap()
        .into_iter()
        .filter(|(_, o)| *o == Ownership::Ours)
        .map(|(f, _)| f)
        .collect();
    assert!(ours.contains(&ProxyField::Enabled));
    assert!(ours.contains(&ProxyField::Server));
    assert!(ours.contains(&ProxyField::Bypass));

    proxy.set_field(ProxyField::Server, Some("x")).unwrap();
    let server_ownership = service
        .ownership()
        .unwrap()
        .into_iter()
        .find(|(f, _)| *f == ProxyField::Server)
        .map(|(_, o)| o)
        .unwrap();
    assert_eq!(server_ownership, Ownership::ExternallyModified);
}

// -- exit recovery ----------------------------------------------------------

#[test]
fn exit_recovery_clears_owned_and_keeps_user_changes() {
    let (service, proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    proxy
        .set_field(ProxyField::Server, Some("user:keep"))
        .unwrap();
    let out = service.restore_on_exit(SysProxyMode::ForcedChange).unwrap();
    assert!(out.ok);
    // User's server survives (reported as a conflict); enabled/bypass are
    // restored to their pre-apply origin (both absent/false).
    assert_eq!(proxy.state().server.as_deref(), Some("user:keep"));
    assert!(!proxy.state().enabled);
    assert!(!out.clean);
}

#[test]
fn exit_recovery_is_idempotent_across_two_runs() {
    let (service, _proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    let first = service.restore_on_exit(SysProxyMode::ForcedChange).unwrap();
    assert!(first.clean);
    let second = service.restore_on_exit(SysProxyMode::ForcedChange).unwrap();
    assert!(second.clean);
    assert!(second.restored.restored.is_empty());
}

// -- PAC lifecycle ----------------------------------------------------------

#[test]
fn pac_start_stop_and_idempotent_restart() {
    let (service, _proxy, _reg) = service();
    let text = "function FindProxyForURL(url, host) { return \"__PROXY__\"; }".to_string();
    let handle = service
        .pac_start(
            PacSource::Inline(text.clone()),
            Some("PROXY 127.0.0.1:10809".into()),
            0,
        )
        .unwrap();
    assert!(handle.running);
    let port = handle.port.expect("pac port");
    assert!(port >= DEFAULT_PAC_PORT_BASE, "port {port} below base");
    assert_ne!(port, 10808);
    let url = handle.url.clone().unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(url.ends_with("/pac"));

    // Repeat start keeps the same port (idempotent).
    let again = service
        .pac_start(
            PacSource::Inline(text),
            Some("PROXY 127.0.0.1:10809".into()),
            0,
        )
        .unwrap();
    assert_eq!(again.port, handle.port);

    service.pac_stop().unwrap();
    assert!(!service.pac_state().running);

    // Stop is idempotent.
    service.pac_stop().unwrap();
}

#[test]
fn pac_serves_rendered_script_over_loopback() {
    use std::io::{BufReader, Write};
    use std::net::TcpStream;

    let (service, _proxy, _reg) = service();
    let handle = service
        .pac_start(
            PacSource::Inline(
                "function FindProxyForURL(url, host) { return \"__PROXY__\"; }".into(),
            ),
            Some("PROXY 127.0.0.1:10809;DIRECT;".into()),
            0,
        )
        .unwrap();
    let port = handle.port.unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect pac");
    stream
        .set_read_timeout(Some(std::time::Duration::from_millis(2000)))
        .unwrap();
    stream
        .write_all(b"GET /pac HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut all = String::new();
    use std::io::Read;
    reader.read_to_string(&mut all).unwrap();
    assert!(all.contains("200 OK"), "response: {all}");
    assert!(all.contains("PROXY 127.0.0.1:10809"));
    assert!(!all.contains("__PROXY__"), "placeholder not replaced");
    let _ = reader;
    service.pac_stop().unwrap();
}

// -- autostart encode/decode ------------------------------------------------

#[test]
fn autostart_command_round_trips_through_service() {
    let (_service, _proxy, reg) = service();
    let (svc2, _p2, reg2) = service();
    svc2.set_autostart(
        "v2rayNAutoRun_abc",
        true,
        r"C:\Program Files\v2rayN\v2rayN.exe",
        "",
    )
    .unwrap();
    let stored = reg2.snapshot();
    let command = stored.get("v2rayNAutoRun_abc").expect("value written");
    assert!(command.starts_with("\"C:\\Program Files\\v2rayN\\v2rayN.exe\""));
    assert!(reg.snapshot().is_empty());
    svc2.set_autostart("v2rayNAutoRun_abc", false, "", "")
        .unwrap();
    assert!(!svc2.autostart_enabled("v2rayNAutoRun_abc").unwrap());
}

// -- script validation ------------------------------------------------------

#[test]
fn custom_script_validation_missing_path_errors() {
    use platform::CustomSystemProxySetting;
    let (service, _proxy, _reg) = service();
    let setting = CustomSystemProxySetting::new(Some("Z:\\does\\not\\exist.pac".to_string()), None);
    let err = service.validate_custom_script(&setting).unwrap_err();
    assert!(matches!(err, platform::PlatformError::NotFound(_)));
}

#[test]
fn custom_script_validation_unset_is_ok() {
    use platform::CustomSystemProxySetting;
    let (service, _proxy, _reg) = service();
    let setting = CustomSystemProxySetting::default();
    assert!(service.validate_custom_script(&setting).is_ok());
}

// -- mode + port helpers ----------------------------------------------------

#[test]
fn mode_helpers_and_domain_conversion() {
    assert_eq!(mode_value(SysProxyMode::Pac), 3);
    assert_eq!(mode_from_value(3), Some(SysProxyMode::Pac));
    assert_eq!(mode_from_value(99), None);
    assert_eq!(
        mode_from_domain(domain::SysProxyType::ForcedChange),
        SysProxyMode::ForcedChange
    );
    assert_eq!(derived_local_port(10808, 3), 10811);
}

#[test]
fn state_view_reports_conflicts_and_pac() {
    let (service, proxy, _reg) = service();
    service
        .apply_proxy(&forced_change("127.0.0.1:10809", "<local>"))
        .unwrap();
    proxy.set_field(ProxyField::Server, Some("user:1")).unwrap();
    let view = service.state_view(SysProxyMode::ForcedChange);
    assert_eq!(view.desired_mode, 1);
    assert!(view.has_ownership);
    assert!(view.conflicts.iter().any(|c| c == "Server"));
    assert!(!view.pac_running);
}
