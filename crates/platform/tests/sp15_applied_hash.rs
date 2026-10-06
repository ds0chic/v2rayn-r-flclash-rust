//! SP-15 platform content-hash contract (fake only, never touches the host).
//!
//! Same mode + same port but a different bypass/PAC payload must produce a
//! different applied identity (so the caller re-applies); cosmetic-only
//! differences (surrounding whitespace, empty-vs-absent) must not.

use platform::sysproxy::{applied_content_hash, applied_content_hash_with_pac};
use platform::{ProxySettings, SysProxyMode};

fn change_settings(server: &str, bypass: &str) -> ProxySettings {
    ProxySettings {
        server: Some(server.to_string()),
        bypass: Some(bypass.to_string()),
        auto_config_url: None,
        auto_detect: Some(false),
    }
}

fn pac_settings(url: &str) -> ProxySettings {
    ProxySettings {
        server: None,
        bypass: None,
        auto_config_url: Some(url.to_string()),
        auto_detect: Some(false),
    }
}

#[test]
fn same_port_bypass_change_changes_hash() {
    let base = change_settings("127.0.0.1:11809", "<local>");
    let edited = change_settings("127.0.0.1:11809", "<local>;192.0.2.0/24");
    let a = applied_content_hash(SysProxyMode::ForcedChange, &base, "sess:11809");
    let b = applied_content_hash(SysProxyMode::ForcedChange, &edited, "sess:11809");
    assert_ne!(a, b, "same port with different bypass must re-apply");
    assert_eq!(
        a,
        applied_content_hash(SysProxyMode::ForcedChange, &base, "sess:11809"),
        "hash must be deterministic"
    );
}

#[test]
fn same_url_pac_content_change_changes_hash() {
    let settings = pac_settings("http://127.0.0.1:11808/pac");
    let a = applied_content_hash_with_pac(
        SysProxyMode::Pac,
        &settings,
        "sess:11808",
        Some("function FindProxyForURL(){return \"PROXY a:1\";}"),
    );
    let b = applied_content_hash_with_pac(
        SysProxyMode::Pac,
        &settings,
        "sess:11808",
        Some("function FindProxyForURL(){return \"PROXY a:2\";}"),
    );
    assert_ne!(a, b, "same PAC URL with different script must re-apply");
    // No PAC payload (plain URL mode) keeps the legacy identity.
    assert_eq!(
        a,
        applied_content_hash_with_pac(
            SysProxyMode::Pac,
            &settings,
            "sess:11808",
            Some("function FindProxyForURL(){return \"PROXY a:1\";}")
        )
    );
    assert_eq!(
        applied_content_hash(SysProxyMode::Pac, &settings, "sess:11808"),
        applied_content_hash_with_pac(SysProxyMode::Pac, &settings, "sess:11808", None),
        "absent PAC content must equal the legacy hash"
    );
}

#[test]
fn cosmetic_bypass_differences_keep_hash_stable() {
    let a = change_settings("127.0.0.1:11809", "<local>");
    let b = change_settings("127.0.0.1:11809", "  <local> ; ");
    let c = ProxySettings {
        server: Some("127.0.0.1:11809".to_string()),
        bypass: None,
        auto_config_url: None,
        auto_detect: Some(false),
    };
    let d = ProxySettings {
        server: Some("127.0.0.1:11809".to_string()),
        bypass: Some(String::new()),
        auto_config_url: None,
        auto_detect: Some(false),
    };
    assert_eq!(
        applied_content_hash(SysProxyMode::ForcedChange, &a, "sess:11809"),
        applied_content_hash(SysProxyMode::ForcedChange, &b, "sess:11809"),
        "whitespace-only bypass edits must not re-apply"
    );
    assert_eq!(
        applied_content_hash(SysProxyMode::ForcedChange, &c, "sess:11809"),
        applied_content_hash(SysProxyMode::ForcedChange, &d, "sess:11809"),
        "empty bypass must equal absent bypass"
    );
}

#[test]
fn different_session_key_changes_hash() {
    let settings = change_settings("127.0.0.1:11809", "<local>");
    assert_ne!(
        applied_content_hash(SysProxyMode::ForcedChange, &settings, "sess:A"),
        applied_content_hash(SysProxyMode::ForcedChange, &settings, "sess:B")
    );
}
