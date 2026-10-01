//! Release metadata parsing and asset selection.

use updater::arch::{HostTarget, Os, PlatformArch};
use updater::metadata::{parse_releases, ReleaseInfo, ReleasesClient};
use updater::UpdateError;

const SAMPLE: &str = r#"[
  {"tag_name":"v26.3.27","name":"Xray 26.3.27","prerelease":false,"published_at":"2026-03-27T00:00:00Z",
   "assets":[
     {"name":"Xray-windows-64.zip","size":20913304,"browser_download_url":"https://x/win.zip","digest":"sha256:d004c392"},
     {"name":"Xray-windows-arm64-v8a.zip","size":123,"browser_download_url":"https://x/arm.zip"},
     {"name":"Xray-linux-64.zip","size":456,"browser_download_url":"https://x/lin.zip"},
     {"name":"Xray-macos-64.zip","size":789,"browser_download_url":"https://x/mac.zip"}
   ]},
  {"tag_name":"v26.4.0-beta.2","name":"beta","prerelease":true,"published_at":"2026-04-02T00:00:00Z",
   "assets":[{"name":"Xray-windows-64.zip","size":1,"browser_download_url":"https://x/beta.zip"}]},
  {"tag_name":"v25.1.1","name":"old","prerelease":false,"published_at":"2025-01-01T00:00:00Z",
   "assets":[]}
]"#;

#[test]
fn model_fields_parse() {
    let releases = parse_releases(SAMPLE).unwrap();
    assert_eq!(releases.len(), 3);
    let first = &releases[0];
    assert_eq!(first.tag_name, "v26.3.27");
    assert_eq!(first.name.as_deref(), Some("Xray 26.3.27"));
    assert!(!first.prerelease);
    assert_eq!(first.published_at.as_deref(), Some("2026-03-27T00:00:00Z"));
    assert_eq!(first.assets.len(), 4);
    assert_eq!(first.assets[0].size, 20913304);
    assert_eq!(first.assets[0].browser_download_url, "https://x/win.zip");
    assert_eq!(first.assets[0].sha256(), Some("d004c392"));
    assert_eq!(first.assets[1].sha256(), None);
}

#[test]
fn parses_missing_optional_fields() {
    let json = r#"[{"tag_name":"1.0.0"}]"#;
    let releases = parse_releases(json).unwrap();
    assert_eq!(releases[0].assets.len(), 0);
    assert!(releases[0].published_at.is_none());
    assert!(!releases[0].prerelease);
}

#[test]
fn invalid_json_is_rejected() {
    assert!(matches!(
        parse_releases("{not json}"),
        Err(UpdateError::InvalidMetadata(_))
    ));
}

#[test]
fn picks_stable_head() {
    let releases = parse_releases(SAMPLE).unwrap();
    let client = ReleasesClient::new("XTLS/Xray-core");
    assert_eq!(client.pick(&releases, false).unwrap().tag_name, "v26.3.27");
}

#[test]
fn picks_prerelease_head_when_requested() {
    let releases = parse_releases(SAMPLE).unwrap();
    let client = ReleasesClient::new("XTLS/Xray-core");
    // Prerelease channel takes the first entry as GitHub returns it (newest
    // first); this fixture lists the stable release first.
    assert_eq!(client.pick(&releases, true).unwrap().tag_name, "v26.3.27");
    // A newest-first fixture returns the beta.
    let newest_first: Vec<ReleaseInfo> = parse_releases(
        r#"[{"tag_name":"v26.4.0-beta.2","prerelease":true,"assets":[]},
            {"tag_name":"v26.3.27","prerelease":false,"assets":[]}]"#,
    )
    .unwrap();
    assert_eq!(
        client.pick(&newest_first, true).unwrap().tag_name,
        "v26.4.0-beta.2"
    );
}

#[test]
fn select_windows_x64_asset() {
    let releases = parse_releases(SAMPLE).unwrap();
    let asset = releases[0]
        .select_for_target("xray", HostTarget::new(Os::Windows, PlatformArch::X64))
        .unwrap();
    assert_eq!(asset.name, "Xray-windows-64.zip");
}

#[test]
fn select_linux_and_macos_assets() {
    let releases = parse_releases(SAMPLE).unwrap();
    let linux = releases[0]
        .select_for_target("xray", HostTarget::new(Os::Linux, PlatformArch::X64))
        .unwrap();
    assert_eq!(linux.name, "Xray-linux-64.zip");
    let mac = releases[0]
        .select_for_target("xray", HostTarget::new(Os::Macos, PlatformArch::X64))
        .unwrap();
    assert_eq!(mac.name, "Xray-macos-64.zip");
}

#[test]
fn missing_asset_reports_needles() {
    let releases = parse_releases(SAMPLE).unwrap();
    let err = releases[0]
        .select_for_target("xray", HostTarget::new(Os::Macos, PlatformArch::Arm64))
        .unwrap_err();
    match err {
        UpdateError::NoMatchingAsset(needles) => assert!(needles.contains("arm64")),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn singbox_asset_needles() {
    let json = r#"[{"tag_name":"v1.14.2","prerelease":false,"assets":[
       {"name":"sing-box-1.14.2-windows-amd64.zip","size":1,"browser_download_url":"u1"},
       {"name":"sing-box-1.14.2-linux-arm64.tar.gz","size":2,"browser_download_url":"u2"}]}]"#;
    let releases = parse_releases(json).unwrap();
    assert_eq!(
        releases[0]
            .select_for_target("sing_box", HostTarget::new(Os::Windows, PlatformArch::X64))
            .unwrap()
            .name,
        "sing-box-1.14.2-windows-amd64.zip"
    );
    assert_eq!(
        releases[0]
            .select_for_target("sing_box", HostTarget::new(Os::Linux, PlatformArch::Arm64))
            .unwrap()
            .name,
        "sing-box-1.14.2-linux-arm64.tar.gz"
    );
}

#[test]
fn locked_max_falls_back_to_highest_allowed() {
    let releases = parse_releases(SAMPLE).unwrap();
    let client = ReleasesClient::new("XTLS/Xray-core");
    let max = updater::Semver::parse("26.3.99");
    // Head is 26.4.0-beta.2 (prerelease); stable head is 26.3.27 which is within cap.
    assert_eq!(
        client
            .pick_with_limit(&releases, false, &max)
            .unwrap()
            .tag_name,
        "v26.3.27"
    );
}

#[test]
fn locked_max_selects_older_stable_when_head_exceeds() {
    let releases = parse_releases(SAMPLE).unwrap();
    let client = ReleasesClient::new("XTLS/Xray-core");
    let max = updater::Semver::parse("26.3.20");
    assert_eq!(
        client
            .pick_with_limit(&releases, false, &max)
            .unwrap()
            .tag_name,
        "v25.1.1"
    );
}

#[test]
fn locked_max_with_nothing_allowed_errors() {
    let releases = parse_releases(SAMPLE).unwrap();
    let client = ReleasesClient::new("XTLS/Xray-core");
    let max = updater::Semver::parse("25.0.0");
    assert_eq!(
        client.pick_with_limit(&releases, false, &max),
        Err(UpdateError::NoRelease)
    );
}

#[test]
fn releases_url_from_repo_and_base() {
    let client =
        ReleasesClient::new("SagerNet/sing-box").with_api_base("http://127.0.0.1:11808/repos/");
    assert_eq!(
        client.releases_url(),
        "http://127.0.0.1:11808/repos/SagerNet/sing-box/releases"
    );
}

#[test]
fn only_prerelease_and_stable_requested_errors() {
    let releases: Vec<ReleaseInfo> =
        parse_releases(r#"[{"tag_name":"1.0.0-rc.1","prerelease":true,"assets":[]}]"#).unwrap();
    let client = ReleasesClient::new("a/b");
    assert_eq!(client.pick(&releases, false), Err(UpdateError::NoRelease));
}
