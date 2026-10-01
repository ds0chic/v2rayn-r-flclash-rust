//! Autostart Run-key backend tests. Always uses the in-memory fake registry.

use platform::autostart::{
    decode_run_command, encode_run_command, run_value_name, AutoStartBackend, AutoStartEntry,
    FakeRegistry, AUTO_RUN_NAME,
};

const NAME: &str = "v2rayNAutoRun_test";

#[test]
fn enable_query_disable_roundtrip() {
    let reg = FakeRegistry::new();
    assert!(!reg.is_enabled(NAME).expect("enabled"));

    reg.enable(NAME, r"C:\Program Files\v2rayN\v2rayN.exe", "")
        .expect("enable");
    assert!(reg.is_enabled(NAME).expect("enabled"));
    assert_eq!(
        reg.query(NAME).expect("query").as_deref(),
        Some(r#""C:\Program Files\v2rayN\v2rayN.exe""#)
    );

    reg.disable(NAME).expect("disable");
    assert!(!reg.is_enabled(NAME).expect("enabled"));
    assert_eq!(reg.query(NAME).expect("query"), None);
}

#[test]
fn disable_is_idempotent() {
    let reg = FakeRegistry::new();
    reg.disable(NAME).expect("disable 1");
    reg.disable(NAME).expect("disable 2");
    assert!(!reg.is_enabled(NAME).expect("enabled"));
}

#[test]
fn set_overwrites_previous_command() {
    let reg = FakeRegistry::new();
    reg.set(NAME, "first").expect("set 1");
    reg.set(NAME, "second").expect("set 2");
    assert_eq!(reg.query(NAME).expect("query").as_deref(), Some("second"));
    assert_eq!(reg.snapshot().len(), 1);
}

#[test]
fn empty_value_is_not_enabled() {
    let reg = FakeRegistry::new();
    reg.set(NAME, "").expect("set empty");
    assert!(!reg.is_enabled(NAME).expect("enabled"));
}

#[test]
fn encode_decode_roundtrip_without_args() {
    let encoded = encode_run_command(r"C:\apps\v2rayN\v2rayN.exe", "");
    assert_eq!(encoded, r#""C:\apps\v2rayN\v2rayN.exe""#);
    let decoded = decode_run_command(&encoded).expect("decode");
    assert_eq!(decoded.exe, r"C:\apps\v2rayN\v2rayN.exe");
    assert_eq!(decoded.args, "");
}

#[test]
fn encode_decode_roundtrip_with_args() {
    let encoded = encode_run_command("app.exe", "-tray --profile default");
    let decoded = decode_run_command(&encoded).expect("decode");
    assert_eq!(decoded.exe, "app.exe");
    assert_eq!(decoded.args, "-tray --profile default");
}

#[test]
fn decode_tolerates_bare_command() {
    let decoded = decode_run_command("app.exe --flag").expect("decode");
    assert_eq!(decoded.exe, "app.exe");
    assert_eq!(decoded.args, "--flag");
    assert!(decode_run_command("   ").is_none());
}

#[test]
fn run_value_name_matches_upstream_scheme() {
    let name = run_value_name(r"C:\apps\v2rayN");
    assert!(name.starts_with(&format!("{AUTO_RUN_NAME}_")));
    let suffix = name.trim_start_matches(&format!("{AUTO_RUN_NAME}_"));
    assert_eq!(suffix.len(), 32, "md5 hex length");
    assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));

    // Deterministic and path-sensitive.
    assert_eq!(name, run_value_name(r"C:\apps\v2rayN"));
    assert_ne!(name, run_value_name(r"C:\apps\other"));
}

#[test]
fn entry_carries_name_and_command() {
    let entry = AutoStartEntry::new(NAME, "cmd");
    assert_eq!(entry.name, NAME);
    assert_eq!(entry.command, "cmd");
}
