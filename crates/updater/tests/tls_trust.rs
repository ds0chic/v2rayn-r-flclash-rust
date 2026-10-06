//! SP-25 (RED): updater HTTPS trust follows the `RootCertProvider`
//! selection. No network here: client construction is offline, and the live
//! TLS matrix runs in the application-level SP-25 suite.

use std::time::Duration;

use updater::tls::HttpsTrust;
use updater::{CoreReleaseApi, DownloaderOptions, FileDownloader};

const FAKE_BUNDLE: &[u8] = b"not-a-real-pem";

#[test]
fn provider_names_map_to_expected_roots() {
    assert_eq!(
        HttpsTrust::from_provider("system", FAKE_BUNDLE, FAKE_BUNDLE),
        HttpsTrust::System
    );
    assert_eq!(
        HttpsTrust::from_provider("chrome", b"c", b"m"),
        HttpsTrust::BundledPem(b"c".to_vec())
    );
    assert_eq!(
        HttpsTrust::from_provider(" Mozilla ", b"c", b"m"),
        HttpsTrust::BundledPem(b"m".to_vec())
    );
    assert_eq!(
        HttpsTrust::from_provider("bogus", b"c", b"m"),
        HttpsTrust::System
    );
    assert_eq!(HttpsTrust::default(), HttpsTrust::System);
    assert!(HttpsTrust::System.uses_system_store());
    assert!(!HttpsTrust::bundled(b"x").uses_system_store());
}

#[test]
fn legacy_constructors_keep_system_behaviour() {
    CoreReleaseApi::new(Duration::from_secs(5)).expect("new");
    CoreReleaseApi::new_with_proxy(Duration::from_secs(5), None).expect("proxy ctor");
    FileDownloader::new(DownloaderOptions::default()).expect("legacy downloader");
}

#[test]
fn trust_aware_constructors_accept_system_and_reject_bad_bundles() {
    CoreReleaseApi::new_with_tls(Duration::from_secs(5), None, HttpsTrust::System).expect("system");
    FileDownloader::new_with_trust(DownloaderOptions::default(), HttpsTrust::System)
        .expect("system downloader");
    CoreReleaseApi::new_with_tls(Duration::from_secs(5), None, HttpsTrust::bundled(&[]))
        .expect_err("empty bundle must not build");
    FileDownloader::new_with_trust(
        DownloaderOptions::default(),
        HttpsTrust::bundled(FAKE_BUNDLE),
    )
    .expect_err("garbage PEM must not build");
}

#[test]
fn trust_failure_classifier_marks_tls_strings_only() {
    assert!(HttpsTrust::is_trust_failure(
        "invalid peer certificate: UnknownIssuer"
    ));
    assert!(HttpsTrust::is_trust_failure(
        "invalid peer certificate: NotValidForName"
    ));
    assert!(!HttpsTrust::is_trust_failure("releases status 404"));
    assert!(!HttpsTrust::is_trust_failure("download timed out"));
    assert!(!HttpsTrust::is_trust_failure("connection refused"));
}
