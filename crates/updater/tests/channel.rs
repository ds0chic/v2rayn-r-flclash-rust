//! Channel policy, update-target support and version caps.

use updater::arch::{HostTarget, Os, PlatformArch};
use updater::{
    check_pre_release, core_spec, core_url_slug, download_template, is_check_update_supported,
    max_allowed_version, read_lock_expected_sha256, spec_version_in_range, UpdateError,
};

#[test]
fn builtin_targets_supported() {
    for core in ["xray", "sing_box", "mihomo", "v2rayN"] {
        assert!(is_check_update_supported(core, false), "{core}");
    }
}

#[test]
fn v2rayn_blocked_for_packaged_install() {
    assert!(!is_check_update_supported("v2rayN", true));
    assert!(is_check_update_supported("xray", true));
}

#[test]
fn non_builtin_targets_unsupported() {
    for core in [
        "v2fly",
        "v2fly_v5",
        "hysteria",
        "hysteria2",
        "tuic",
        "juicity",
    ] {
        assert!(!is_check_update_supported(core, false), "{core}");
    }
}

#[test]
fn prerelease_policy_matches_upstream() {
    assert!(check_pre_release("v2rayN", true));
    assert!(check_pre_release("xray", true));
    assert!(!check_pre_release("sing_box", true));
    assert!(!check_pre_release("mihomo", true));
    assert!(!check_pre_release("xray", false));
    assert!(!check_pre_release("v2rayN", false));
}

#[test]
fn singbox_max_is_1_14() {
    let max = max_allowed_version("sing_box").unwrap();
    assert_eq!(max.major(), 1);
    assert_eq!(max.minor(), 14);
    assert_eq!(max.patch(), u32::MAX as u64);
}

#[test]
fn singbox_version_range_boundaries() {
    assert!(spec_version_in_range("sing_box", &updater::Semver::parse("1.14.0")).is_ok());
    assert!(spec_version_in_range("sing_box", &updater::Semver::parse("1.14.2")).is_ok());
    assert!(spec_version_in_range("sing_box", &updater::Semver::parse("1.14.99")).is_ok());
    let err = spec_version_in_range("sing_box", &updater::Semver::parse("1.15.0")).unwrap_err();
    assert!(matches!(err, UpdateError::VersionOutOfRange(_)));
    assert!(spec_version_in_range("sing_box", &updater::Semver::parse("2.0.0")).is_err());
}

#[test]
fn uncapped_cores_accept_any_version() {
    assert!(max_allowed_version("xray").is_none());
    assert!(spec_version_in_range("xray", &updater::Semver::parse("99.9.9")).is_ok());
    assert!(spec_version_in_range("mihomo", &updater::Semver::parse("1.99.0")).is_ok());
}

#[test]
fn core_url_slugs() {
    assert_eq!(core_url_slug("xray"), Some("XTLS/Xray-core"));
    assert_eq!(core_url_slug("sing_box"), Some("SagerNet/sing-box"));
    assert_eq!(core_url_slug("mihomo"), Some("MetaCubeX/mihomo"));
    assert_eq!(core_url_slug("v2rayN"), Some("2dust/v2rayN"));
    assert_eq!(core_url_slug("v2fly"), Some("v2fly/v2ray-core"));
    assert_eq!(core_url_slug("hysteria2"), Some("apernet/hysteria"));
    assert_eq!(core_url_slug("unknown"), None);
}

#[test]
fn download_templates_match_assets() {
    let win = HostTarget::new(Os::Windows, PlatformArch::X64);
    assert_eq!(
        download_template("xray", win),
        Some("/download/{0}/Xray-windows-64.zip")
    );
    assert_eq!(
        download_template("sing_box", win),
        Some("/download/{0}/sing-box-{1}-windows-amd64.zip")
    );
    let win_arm = HostTarget::new(Os::Windows, PlatformArch::Arm64);
    assert_eq!(
        download_template("xray", win_arm),
        Some("/download/{0}/Xray-windows-arm64-v8a.zip")
    );
    assert_eq!(download_template("unknown", win), None);
}

#[test]
fn core_spec_fields() {
    let spec = core_spec("sing_box").unwrap();
    assert_eq!(spec.repo, "SagerNet/sing-box");
    assert!(spec.locked_max_version.is_some());
    assert!(!spec.prerelease_capable);

    let v2rayn = core_spec("v2rayN").unwrap();
    assert!(v2rayn.prerelease_capable);
    assert!(v2rayn.locked_max_version.is_none());

    assert!(core_spec("unknown").is_none());
}

#[test]
fn reads_sha256_from_core_lock() {
    let lock = r#"{
      "schema": 1,
      "cores": [
        {"core":"xray","asset":"Xray-windows-64.zip","sha256":"D004C39288CE9ADA487C6F398C7C545F7D749E44BDFDD59DBC9F865AFBA4E1AD"},
        {"core":"sing-box","asset":"sing-box-1.14.2-windows-amd64.zip","sha256":"C2D8BFFF918755808781DFDEEB8581B6C91EB3A243D9A7B55483CFC0C0684D32"}
      ]
    }"#;
    assert_eq!(
        read_lock_expected_sha256(lock, "Xray-windows-64.zip"),
        Some("d004c39288ce9ada487c6f398c7c545f7d749e44bdfdd59dbc9f865afba4e1ad".to_string())
    );
    assert_eq!(read_lock_expected_sha256(lock, "missing.zip"), None);
    assert_eq!(read_lock_expected_sha256("not json", "x"), None);
}
