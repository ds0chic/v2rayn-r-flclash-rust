//! Version parsing and comparison boundaries (upstream SemanticVersion.cs).

use std::cmp::Ordering;
use updater::Semver;

#[test]
fn parse_v_prefix_and_case() {
    assert_eq!(Semver::parse("v26.3.27"), Semver::new(26, 3, 27));
    assert_eq!(Semver::parse("V26.3.27"), Semver::new(26, 3, 27));
    assert_eq!(Semver::parse("26.3.27"), Semver::new(26, 3, 27));
}

#[test]
fn parse_two_and_four_segments() {
    assert_eq!(Semver::parse("1.14"), Semver::new(1, 14, 0));
    assert_eq!(Semver::parse("1.14.2.77"), Semver::new(1, 14, 2));
}

#[test]
fn malformed_falls_back_to_zero() {
    assert_eq!(Semver::parse(""), Semver::new(0, 0, 0));
    assert_eq!(Semver::parse("v"), Semver::new(0, 0, 0));
    assert_eq!(Semver::parse("abc.def.ghi"), Semver::new(0, 0, 0));
    assert_eq!(Semver::parse("1.x.2"), Semver::new(0, 0, 0));
}

#[test]
fn prerelease_lower_than_release() {
    assert!(Semver::parse("7.25.4-beta.1") < Semver::parse("7.25.4"));
    assert!(Semver::parse("7.25.4") > Semver::parse("7.25.4-rc.1"));
    assert_eq!(
        Semver::parse("7.25.4").cmp(&Semver::parse("7.25.4")),
        Ordering::Equal
    );
}

#[test]
fn prerelease_numeric_and_alnum_ordering() {
    assert!(Semver::parse("1.0.0-alpha.1") < Semver::parse("1.0.0-alpha.2"));
    assert!(Semver::parse("1.0.0-alpha.10") > Semver::parse("1.0.0-alpha.2"));
    assert!(Semver::parse("1.0.0-alpha") < Semver::parse("1.0.0-alpha.1"));
    assert!(Semver::parse("1.0.0-alpha.beta") < Semver::parse("1.0.0-beta"));
    assert!(Semver::parse("1.0.0-2") < Semver::parse("1.0.0-11"));
}

#[test]
fn prerelease_numeric_lower_than_alphanumeric() {
    // SemVer 2.0.0 §11.4.3: numeric identifiers have lower precedence.
    assert!(Semver::parse("1.0.0-alpha") > Semver::parse("1.0.0-1"));
    assert!(Semver::parse("1.0.0-1") < Semver::parse("1.0.0-alpha"));
}

#[test]
fn build_metadata_ignored_in_ordering() {
    assert_eq!(
        Semver::parse("1.0.0+abc").cmp(&Semver::parse("1.0.0+xyz")),
        Ordering::Equal
    );
    assert_eq!(Semver::parse("1.0.0+abc").build(), Some("abc"));
}

#[test]
fn major_minor_patch_ordering() {
    assert!(Semver::parse("1.14.2") < Semver::parse("1.15.0"));
    assert!(Semver::parse("2.0.0") > Semver::parse("1.99.99"));
    assert!(Semver::parse("1.14.3") > Semver::parse("1.14.2"));
}

#[test]
fn singbox_cap_boundaries() {
    // Locked max is 1.14.int::MAX.
    let cap = Semver::new(1, 14, u32::MAX as u64);
    assert!(Semver::parse("1.14.2") <= cap);
    assert!(Semver::parse("1.14.4294967295") <= cap);
    assert!(Semver::parse("1.15.0") > cap);
    assert!(Semver::parse("2.0.0") > cap);
}

#[test]
fn standard_string_roundtrip() {
    let v = Semver::parse("v7.25.4-beta.1+sha.abc0");
    assert_eq!(v.to_standard_string(Some('v')), "v7.25.4-beta.1+sha.abc0");
    assert_eq!(v.to_standard_string(None), "7.25.4-beta.1+sha.abc0");
    assert_eq!(v.core_string(), "7.25.4");
    assert!(v.is_prerelease());
    assert_eq!(v.as_raw(), "v7.25.4-beta.1+sha.abc0");
}
