//! SP-25 (RED): subscription HTTPS trust follows the `RootCertProvider`
//! selection. No network here: building the client is offline, and the live
//! TLS matrix runs in the application-level SP-25 suite.

use subscriptions::tls::HttpsTrust;
use subscriptions::{build_client, build_client_with_trust, DownloadOptions};

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
fn system_build_keeps_legacy_path() {
    // The pre-SP-25 constructor keeps working and keeps system trust.
    build_client(&DownloadOptions::default()).expect("legacy build");
    build_client_with_trust(&DownloadOptions::default(), &HttpsTrust::System)
        .expect("system build");
}

#[test]
fn empty_bundle_is_a_build_error() {
    let err = build_client_with_trust(&DownloadOptions::default(), &HttpsTrust::bundled(&[]))
        .expect_err("empty bundle must not build");
    assert!(
        !matches!(
            err,
            subscriptions::SubError::Cancelled | subscriptions::SubError::Timeout
        ),
        "unexpected class: {err:?}"
    );
}

#[test]
fn garbage_bundle_is_a_build_error() {
    build_client_with_trust(
        &DownloadOptions::default(),
        &HttpsTrust::bundled(FAKE_BUNDLE),
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
    assert!(!HttpsTrust::is_trust_failure("status 404"));
    assert!(!HttpsTrust::is_trust_failure(
        "error sending request: connection refused"
    ));
    assert!(!HttpsTrust::is_trust_failure("timeout"));
}
