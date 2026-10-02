//! Real Windows backend round-trips (opt-in via `cargo test -- --ignored`).
//!
//! These tests touch the **live** current-user registry / WinINET proxy state
//! for a very short window and always restore the snapshot they captured.
//! They are ignored by default so normal test runs never mutate the host.

#![cfg(windows)]

use platform::autostart::{run_value_name, AutoStartBackend};
use platform::sysproxy::{ProxyField, ProxySettings, ProxyState, SysProxyMode, SystemProxyBackend};
use platform::{WindowsRunKeyBackend, WindowsSystemProxyBackend};

struct ProxyGuard<'a> {
    backend: &'a WindowsSystemProxyBackend,
    original: ProxyState,
}

impl Drop for ProxyGuard<'_> {
    fn drop(&mut self) {
        for field in ProxyField::ALL {
            let _ = self
                .backend
                .set_field(field, self.original.field(field).as_deref());
        }
        let _ = self.backend.notify_changed();
    }
}

#[test]
#[ignore = "mutates the live current-user proxy for <1s and restores it"]
fn real_sysproxy_apply_restore_roundtrip() {
    let backend = WindowsSystemProxyBackend::new();
    let original = backend.snapshot().expect("snapshot original");
    println!("before: {original:?}");
    let guard = ProxyGuard {
        backend: &backend,
        original: original.clone(),
    };

    let settings = ProxySettings {
        server: Some("127.0.0.1:11808".to_string()),
        bypass: original.bypass.clone(),
        auto_config_url: None,
        auto_detect: Some(false),
    };
    let applied = backend
        .apply(SysProxyMode::ForcedChange, &settings)
        .expect("apply forced change");
    let current = backend.snapshot().expect("snapshot after apply");
    println!("applied: {applied:?}");
    println!("during : {current:?}");
    assert_eq!(
        current.server.as_deref(),
        Some("127.0.0.1:11808"),
        "server field must be our value while applied"
    );
    assert!(current.enabled, "proxy must be enabled while applied");

    drop(guard);

    let restored = backend.snapshot().expect("snapshot after restore");
    println!("after  : {restored:?}");
    assert_eq!(restored.enabled, original.enabled, "enabled restored");
    assert_eq!(restored.server, original.server, "server restored");
    assert_eq!(restored.bypass, original.bypass, "bypass restored");
    assert_eq!(
        restored.auto_config_url, original.auto_config_url,
        "pac url restored"
    );
    assert_eq!(
        restored.auto_detect, original.auto_detect,
        "autodetect restored"
    );
}

#[test]
#[ignore = "writes and removes one HKCU Run value"]
fn real_autostart_write_read_remove_roundtrip() {
    let backend = WindowsRunKeyBackend::new();
    let temp_exe = std::env::temp_dir()
        .join("v2rayn-r-autostart-probe.exe")
        .to_string_lossy()
        .into_owned();
    let name = run_value_name(&temp_exe);
    assert!(backend.query(&name).expect("query").is_none());

    backend
        .enable(&name, &temp_exe, "--autostart-probe")
        .expect("enable");
    let value = backend.query(&name).expect("query after enable");
    println!("run value: {value:?}");
    assert!(value
        .as_deref()
        .is_some_and(|v| v.contains("v2rayn-r-autostart-probe")));

    backend.disable(&name).expect("disable");
    assert!(backend.query(&name).expect("query after disable").is_none());
}
