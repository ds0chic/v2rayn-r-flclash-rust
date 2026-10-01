//! Model serialization and custom PAC/script path validation.

use std::time::{SystemTime, UNIX_EPOCH};

use platform::{
    AppliedChange, CustomSystemProxySetting, ProxyField, ProxySettings, ProxyState, SysProxyMode,
    CUSTOM_PAC_FIELD, CUSTOM_SCRIPT_FIELD,
};

fn temp_path(suffix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "t13-script-{}-{nanos}-{suffix}",
        std::process::id()
    ))
}

#[test]
fn mode_serializes_with_upstream_names() {
    let json = serde_json::to_string(&SysProxyMode::ForcedClear).expect("ser");
    assert_eq!(json, "\"ForcedClear\"");
    let json = serde_json::to_string(&SysProxyMode::Pac).expect("ser");
    assert_eq!(json, "\"Pac\"");
    let back: SysProxyMode = serde_json::from_str("\"ForcedChange\"").expect("de");
    assert_eq!(back, SysProxyMode::ForcedChange);
}

#[test]
fn proxy_state_roundtrips() {
    let state = ProxyState {
        enabled: true,
        server: Some("127.0.0.1:11809".to_string()),
        bypass: Some("localhost;127.*".to_string()),
        auto_config_url: None,
        auto_detect: true,
    };
    let json = serde_json::to_string(&state).expect("ser");
    let back: ProxyState = serde_json::from_str(&json).expect("de");
    assert_eq!(state, back);
}

#[test]
fn applied_change_roundtrips_with_field() {
    let change = AppliedChange {
        field: ProxyField::AutoConfigUrl,
        before: None,
        after: Some("http://127.0.0.1:11808/pac".to_string()),
    };
    let json = serde_json::to_string(&change).expect("ser");
    let back: AppliedChange = serde_json::from_str(&json).expect("de");
    assert_eq!(change, back);
}

#[test]
fn field_name_constants_match_ledger() {
    assert_eq!(CUSTOM_PAC_FIELD, "CustomSystemProxyPacPath");
    assert_eq!(CUSTOM_SCRIPT_FIELD, "CustomSystemProxyScriptPath");
}

#[test]
fn existing_regular_file_validates() {
    let path = temp_path("ok.pac");
    std::fs::write(&path, "PAC").expect("write");
    let setting = CustomSystemProxySetting::new(Some(path.display().to_string()), None);
    assert!(setting.validate().is_ok());
    let resolved = setting.pac_file().expect("pac").expect("some");
    assert_eq!(resolved, path);
    let _ = std::fs::remove_file(path);
}

#[test]
fn directory_is_rejected_as_not_a_file() {
    let dir = std::env::temp_dir();
    let setting = CustomSystemProxySetting::new(None, Some(dir.display().to_string()));
    assert!(matches!(
        setting.validate(),
        Err(platform::PlatformError::Invalid(_))
    ));
}

#[test]
fn missing_script_path_is_not_found() {
    let setting =
        CustomSystemProxySetting::new(None, Some(temp_path("missing.sh").display().to_string()));
    assert!(matches!(
        setting.validate(),
        Err(platform::PlatformError::NotFound(_))
    ));
}

#[test]
fn settings_carrier_roundtrips() {
    let settings = ProxySettings {
        server: Some("127.0.0.1:11809".to_string()),
        bypass: Some("localhost".to_string()),
        auto_config_url: None,
        auto_detect: Some(false),
    };
    let json = serde_json::to_string(&settings).expect("ser");
    let back: ProxySettings = serde_json::from_str(&json).expect("de");
    assert_eq!(settings, back);
}
