//! End-to-end-ish pipeline test against a loopback mock GitHub API.
//!
//! Covers: metadata fetch → channel selection → asset selection → download →
//! sha256 verify → arch check → safe unpack → atomic install → manifest.

mod common;

use std::path::Path;

use common::{MockHttp, MockResponse};
use updater::arch::{HostTarget, Os, PlatformArch};
use updater::metadata::{parse_releases, ReleasesClient};
use updater::{
    apply_atomic, parse_binary_arch, safe_unpack_zip, sha256_of, DownloadRequest,
    DownloaderOptions, FileDownloader, InstallPlan, Semver, UnpackLimits, UpdateError,
};

/// Build a minimal PE x64 binary so the arch stage has real bytes to inspect.
fn pe_x64() -> Vec<u8> {
    let mut buf = vec![0u8; 0x80];
    buf[..2].copy_from_slice(b"MZ");
    buf[0x3c..0x40].copy_from_slice(&0x40u32.to_le_bytes());
    buf[0x40..0x44].copy_from_slice(b"PE\0\0");
    buf[0x44..0x46].copy_from_slice(&0x8664u16.to_le_bytes());
    buf
}

#[test]
fn full_pipeline_against_mock_github_api() {
    let payload = pe_x64();
    let payload_for_asset = payload.clone();
    let payload_for_server = payload.clone();
    let digest = sha256_of(&payload);
    let digest_for_server = digest.clone();

    // The mock server serves both the releases JSON and the asset bytes.
    let mock = MockHttp::spawn(move |request| {
        if request.url.ends_with("/releases") {
            let host = request.header("Host").unwrap_or("127.0.0.1").to_string();
            let releases = format!(
                r#"[{{"tag_name":"v26.3.27","name":"Xray","prerelease":false,"published_at":"2026-03-27T00:00:00Z",
                   "assets":[{{"name":"Xray-windows-64.zip","size":{},"browser_download_url":"http://{host}/download/Xray-windows-64.zip","digest":"sha256:{}"}}]}}]"#,
                payload_for_asset.len(),
                digest_for_server
            );
            MockResponse::json(releases)
        } else {
            MockResponse::bytes(payload_for_server.clone())
        }
    });

    let tmp = tempfile::tempdir().unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // 1. Fetch + parse metadata.
    let client =
        ReleasesClient::new("XTLS/Xray-core").with_api_base(format!("{}/repos", mock.base_url()));
    let api = updater::CoreReleaseApi::new(std::time::Duration::from_secs(5)).unwrap();
    let releases = rt.block_on(api.fetch(&client)).unwrap();
    assert_eq!(releases.len(), 1);

    // 2. Channel + asset selection.
    let release = client.pick(&releases, false).unwrap();
    assert_eq!(release.version(), Semver::parse("26.3.27"));
    let target = HostTarget::new(Os::Windows, PlatformArch::X64);
    let asset = release.select_for_target("xray", target).unwrap();
    assert_eq!(asset.name, "Xray-windows-64.zip");

    // 3. Download to staging.
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();
    let archive_path = staging.join("Xray-windows-64.zip");
    let request = DownloadRequest::new(asset.browser_download_url.clone(), &archive_path);
    let token = updater::CancellationToken::new();
    let downloaded = rt
        .block_on(
            FileDownloader::new(DownloaderOptions::default())
                .unwrap()
                .download(&request, &token),
        )
        .unwrap();

    // 4. Integrity: sha256 must match the digest GitHub advertised.
    assert_eq!(downloaded.sha256, digest);
    assert!(updater::download::sha256_matches(&payload, &digest));

    // 5. Architecture check on the downloaded bytes (read-only).
    let parsed = parse_binary_arch(&payload).unwrap();
    assert!(parsed.matches(target));

    // 6. Unpack the (here: fake zip) — build a real zip from the payload.
    common::archives::write_zip(
        &archive_path,
        &[common::archives::ZipEntry::file("xray/xray.exe", &payload)],
    );
    let unpacked = safe_unpack_zip(
        &archive_path,
        &staging.join("unpacked"),
        UnpackLimits::default(),
    )
    .unwrap();
    assert_eq!(unpacked.entries, 1);

    // 7. Atomic install.
    std::fs::create_dir_all(tmp.path().join("current")).unwrap();
    std::fs::write(tmp.path().join("current/old.txt"), b"old").unwrap();
    let plan = InstallPlan::new(
        tmp.path(),
        tmp.path().join("current"),
        staging.join("unpacked"),
        "previous",
        "26.3.27",
    );
    let outcome = apply_atomic(&plan).unwrap();
    assert_eq!(outcome.version, "26.3.27");
    assert!(Path::new(&tmp.path().join("current/xray/xray.exe")).exists());
    updater::verify_manifest(&tmp.path().join("current"), &outcome.manifest).unwrap();
}

/// A version that exceeds the sing-box cap must be rejected before download.
#[test]
fn channel_cap_blocks_over_cap_release_before_download() {
    let json = r#"[{"tag_name":"v1.15.0","prerelease":false,"assets":[
       {"name":"sing-box-1.15.0-windows-amd64.zip","size":1,"browser_download_url":"u"}]}]"#;
    let releases = parse_releases(json).unwrap();
    let max = updater::max_allowed_version("sing_box").unwrap();
    let picked = releases[0].parsed_version().unwrap();
    assert!(updater::spec_version_in_range("sing_box", &picked).is_err());
    let client = ReleasesClient::new("SagerNet/sing-box");
    assert_eq!(
        client.pick_with_limit(&releases, false, &max),
        Err(UpdateError::NoRelease)
    );
}

#[test]
fn sha256_mismatch_would_be_detected() {
    let payload = b"real";
    let wrong = sha256_of(b"different");
    assert!(!updater::download::sha256_matches(payload, &wrong));
}
