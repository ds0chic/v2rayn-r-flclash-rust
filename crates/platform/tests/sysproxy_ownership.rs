//! System proxy ownership / apply / restore matrix, exercised on the fake
//! backend only. The host proxy is never touched.

use platform::{
    restore_if_owned, AppliedChange, FakeSystemProxyBackend, ProxyField, ProxySettings, ProxyState,
    SysProxyMode, SystemProxyBackend,
};

fn settings(server: Option<&str>, bypass: Option<&str>) -> ProxySettings {
    ProxySettings {
        server: server.map(str::to_string),
        bypass: bypass.map(str::to_string),
        auto_config_url: None,
        auto_detect: None,
    }
}

fn rich_initial() -> ProxyState {
    ProxyState {
        enabled: true,
        server: Some("127.0.0.1:10809".to_string()),
        bypass: Some("localhost".to_string()),
        auto_config_url: None,
        auto_detect: false,
    }
}

fn fields(changes: &[AppliedChange]) -> Vec<ProxyField> {
    changes.iter().map(|c| c.field).collect()
}

#[test]
fn forced_change_records_only_fields_that_differ() {
    let fake = FakeSystemProxyBackend::default();
    let changes = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost")),
        )
        .expect("apply");

    assert!(fields(&changes).contains(&ProxyField::Enabled));
    assert!(fields(&changes).contains(&ProxyField::Server));
    assert!(fields(&changes).contains(&ProxyField::Bypass));
    assert!(!fields(&changes).contains(&ProxyField::AutoConfigUrl));
    assert!(!fields(&changes).contains(&ProxyField::AutoDetect));

    let state = fake.state();
    assert!(state.enabled);
    assert_eq!(state.server.as_deref(), Some("127.0.0.1:11809"));
    assert_eq!(state.bypass.as_deref(), Some("localhost"));

    assert_eq!(fake.writes().len(), changes.len());
    assert_eq!(fake.notify_count(), 1);
}

#[test]
fn forced_change_is_a_noop_when_already_equal() {
    let fake = FakeSystemProxyBackend::new(ProxyState {
        enabled: true,
        server: Some("127.0.0.1:1".to_string()),
        bypass: Some("b".to_string()),
        auto_config_url: None,
        auto_detect: false,
    });
    let changes = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:1"), Some("b")),
        )
        .expect("apply");
    assert!(changes.is_empty());
    assert!(fake.writes().is_empty());
}

#[test]
fn unchanged_mode_makes_no_change_and_no_notification() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let changes = fake
        .apply(SysProxyMode::Unchanged, &settings(Some("x"), Some("y")))
        .expect("apply");
    assert!(changes.is_empty());
    assert!(fake.writes().is_empty());
    assert_eq!(fake.notify_count(), 0);
    assert_eq!(fake.state(), rich_initial());
}

#[test]
fn forced_clear_records_clearing() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let changes = fake
        .apply(SysProxyMode::ForcedClear, &ProxySettings::default())
        .expect("apply");
    let got = fields(&changes);
    assert!(got.contains(&ProxyField::Enabled));
    assert!(got.contains(&ProxyField::Server));
    assert!(got.contains(&ProxyField::Bypass));

    let state = fake.state();
    assert!(!state.enabled);
    assert_eq!(state.server, None);
    assert_eq!(state.bypass, None);
}

#[test]
fn pac_mode_sets_autoconfig_and_disables_named_proxy() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let changes = fake
        .apply(
            SysProxyMode::Pac,
            &ProxySettings {
                auto_config_url: Some("http://127.0.0.1:11808/pac".to_string()),
                ..ProxySettings::default()
            },
        )
        .expect("apply");
    let got = fields(&changes);
    assert!(got.contains(&ProxyField::Enabled));
    assert!(got.contains(&ProxyField::Server));
    assert!(got.contains(&ProxyField::AutoConfigUrl));

    let state = fake.state();
    assert!(!state.enabled);
    assert_eq!(state.server, None);
    assert_eq!(
        state.auto_config_url.as_deref(),
        Some("http://127.0.0.1:11808/pac")
    );
}

#[test]
fn pac_mode_requires_url() {
    let fake = FakeSystemProxyBackend::default();
    let err = fake
        .apply(SysProxyMode::Pac, &ProxySettings::default())
        .unwrap_err();
    assert!(matches!(err, platform::PlatformError::Invalid(_)));
}

#[test]
fn restore_all_owned_fields_returns_to_original() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost,127.*")),
        )
        .expect("apply");
    let report = fake.restore(&applied).expect("restore");

    assert!(report.is_clean());
    assert!(!report.restored.is_empty());
    assert_eq!(fake.state(), rich_initial());
}

#[test]
fn restore_keeps_user_change_and_reports_conflict() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost")),
        )
        .expect("apply");

    // User (or another tool) edits the proxy server by hand.
    fake.set_field(ProxyField::Server, Some("user:9999"))
        .expect("user edit");

    let report = fake.restore(&applied).expect("restore");
    assert_eq!(report.conflicted_fields(), vec![ProxyField::Server]);
    assert!(report.restored_fields().is_empty());

    let state = fake.state();
    assert_eq!(state.server.as_deref(), Some("user:9999"));
    assert!(state.enabled);
}

#[test]
fn restore_reports_conflict_when_owned_field_is_removed() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost")),
        )
        .expect("apply");

    fake.set_field(ProxyField::Server, None)
        .expect("user clears");
    let report = fake.restore(&applied).expect("restore");
    assert_eq!(report.conflicted_fields(), vec![ProxyField::Server]);
    assert_eq!(report.conflicts[0].current_value, None);
}

#[test]
fn restore_only_touches_fields_we_changed() {
    let fake = FakeSystemProxyBackend::new(ProxyState {
        enabled: true,
        server: Some("127.0.0.1:10809".to_string()),
        bypass: Some("localhost".to_string()),
        auto_config_url: None,
        auto_detect: false,
    });
    // Same enabled/bypass, different server: only Server is owned.
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost")),
        )
        .expect("apply");
    assert_eq!(fields(&applied), vec![ProxyField::Server]);

    let report = fake.restore(&applied).expect("restore");
    assert_eq!(report.restored_fields(), vec![ProxyField::Server]);
    assert_eq!(fake.state().server.as_deref(), Some("127.0.0.1:10809"));
}

#[test]
fn repeated_restore_is_idempotent() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), Some("localhost")),
        )
        .expect("apply");

    let first = fake.restore(&applied).expect("restore 1");
    assert!(!first.restored.is_empty());
    let second = fake.restore(&applied).expect("restore 2");
    assert!(second.restored.is_empty());
    assert!(second.is_clean());
    assert_eq!(fake.state(), rich_initial());
}

#[test]
fn apply_records_before_values_for_restore() {
    let fake = FakeSystemProxyBackend::new(rich_initial());
    let applied = fake
        .apply(
            SysProxyMode::ForcedChange,
            &settings(Some("127.0.0.1:11809"), None),
        )
        .expect("apply");
    let server_change = applied
        .iter()
        .find(|c| c.field == ProxyField::Server)
        .expect("server change");
    assert_eq!(server_change.before.as_deref(), Some("127.0.0.1:10809"));
    assert_eq!(server_change.after.as_deref(), Some("127.0.0.1:11809"));
}

#[test]
fn pure_restore_matrix_handles_absent_fields() {
    let applied = vec![
        AppliedChange {
            field: ProxyField::Server,
            before: Some("old".to_string()),
            after: Some("new".to_string()),
        },
        AppliedChange {
            field: ProxyField::Bypass,
            before: None,
            after: Some("added".to_string()),
        },
    ];
    let untouched = ProxyState {
        enabled: false,
        server: Some("new".to_string()),
        bypass: Some("added".to_string()),
        auto_config_url: None,
        auto_detect: false,
    };
    let report = restore_if_owned(&applied, &untouched);
    assert_eq!(report.restored.len(), 2);
    assert!(report.is_clean());

    let edited = ProxyState {
        enabled: false,
        server: Some("user".to_string()),
        bypass: Some("added".to_string()),
        auto_config_url: None,
        auto_detect: false,
    };
    let report = restore_if_owned(&applied, &edited);
    assert_eq!(report.conflicted_fields(), vec![ProxyField::Server]);
    assert_eq!(report.restored_fields(), vec![ProxyField::Bypass]);
}
