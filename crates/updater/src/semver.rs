//! Semantic version parsing and comparison.
//!
//! Mirrors upstream `ServiceLib/Models/Dto/SemanticVersion.cs` (commit
//! `7d6a967`): accepts an optional `v`/`V` prefix, 2 or 3 (or 4, ignored)
//! numeric segments, an optional `-prerelease` and `+build`. Parsing never
//! fails — an unparseable string becomes `0.0.0`, matching the upstream
//! constructor's catch-all. Only the prerelease list participates in ordering;
//! build metadata is ignored for comparison (SemVer 2.0.0 §10).

use std::cmp::Ordering;
use std::fmt;

/// A parsed semantic version. `raw` keeps the original text for display.
///
/// Equality and ordering ignore `raw` and build metadata, matching upstream
/// `SemanticVersion.Equals`/`CompareTo`.
#[derive(Debug, Clone)]
pub struct Semver {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Option<String>,
    build: Option<String>,
    raw: String,
}

impl PartialEq for Semver {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.prerelease == other.prerelease
    }
}

impl Eq for Semver {}

impl std::hash::Hash for Semver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
        self.patch.hash(state);
        self.prerelease.hash(state);
    }
}

impl Semver {
    /// Parse `version`, falling back to `0.0.0` on malformed input.
    pub fn parse(version: &str) -> Self {
        Self::try_parse_strict(version).unwrap_or_else(|| Self {
            major: 0,
            minor: 0,
            patch: 0,
            prerelease: None,
            build: None,
            raw: "0.0.0".to_string(),
        })
    }

    /// Numeric triple.
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
            build: None,
            raw: format!("{major}.{minor}.{patch}"),
        }
    }

    /// Like [`parse`](Self::parse) but rejects malformed input.
    pub fn try_parse_strict(version: &str) -> Option<Self> {
        if version.is_empty() {
            return None;
        }
        let raw = version.to_string();
        let trimmed = version
            .strip_prefix('v')
            .or_else(|| version.strip_prefix('V'))
            .unwrap_or(version);

        let (left_of_plus, build) = match trimmed.split_once('+') {
            Some((left, meta)) => (left, (!meta.is_empty()).then(|| meta.to_string())),
            None => (trimmed, None),
        };
        let (version_part, prerelease) = match left_of_plus.split_once('-') {
            Some((left, pre)) => (left, (!pre.is_empty()).then(|| pre.to_string())),
            None => (left_of_plus, None),
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        let (major, minor, patch) = match parts.as_slice() {
            [major, minor] => (major.parse::<u64>().ok()?, minor.parse::<u64>().ok()?, 0),
            [major, minor, patch] | [major, minor, patch, _] => (
                major.parse::<u64>().ok()?,
                minor.parse::<u64>().ok()?,
                patch.parse::<u64>().ok()?,
            ),
            _ => return None,
        };

        Some(Self {
            major,
            minor,
            patch,
            prerelease,
            build,
            raw,
        })
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }

    pub fn prerelease(&self) -> Option<&str> {
        self.prerelease.as_deref()
    }

    pub fn build(&self) -> Option<&str> {
        self.build.as_deref()
    }

    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }

    /// The original, unmodified text of this version.
    pub fn as_raw(&self) -> &str {
        &self.raw
    }

    /// `major.minor.patch` without prefix, prerelease or build.
    pub fn core_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// Render `major.minor.patch[-pre][+build]` with an optional prefix.
    pub fn to_standard_string(&self, prefix: Option<char>) -> String {
        let mut out = String::new();
        if let Some(prefix) = prefix {
            out.push(prefix);
        }
        out.push_str(&self.core_string());
        if let Some(pre) = &self.prerelease {
            out.push('-');
            out.push_str(pre);
        }
        if let Some(build) = &self.build {
            out.push('+');
            out.push_str(build);
        }
        out
    }
}

impl fmt::Display for Semver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl PartialOrd for Semver {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Semver {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| {
                compare_prerelease(self.prerelease.as_deref(), other.prerelease.as_deref())
            })
    }
}

/// SemVer 2.0.0 §11.4 ordering: a version with a prerelease is *lower* than the
/// same triple without one; identifiers are compared field by field, numeric
/// ones numerically and lower than alphanumeric ones.
fn compare_prerelease(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => {
            let mut left = left.split('.');
            let mut right = right.split('.');
            loop {
                match (left.next(), right.next()) {
                    (None, None) => return Ordering::Equal,
                    (None, Some(_)) => return Ordering::Less,
                    (Some(_), None) => return Ordering::Greater,
                    (Some(a), Some(b)) => match compare_identifier(a, b) {
                        Ordering::Equal => continue,
                        other => return other,
                    },
                }
            }
        }
    }
}

fn compare_identifier(left: &str, right: &str) -> Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(a), Ok(b)) => a.cmp(&b),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefixed_and_bare() {
        assert_eq!(Semver::parse("v1.14.2"), Semver::new(1, 14, 2));
        assert_eq!(Semver::parse("1.14.2"), Semver::new(1, 14, 2));
        assert_eq!(Semver::parse("V7.25.4"), Semver::new(7, 25, 4));
    }

    #[test]
    fn parses_two_segment_and_four_segment() {
        assert_eq!(Semver::parse("1.14"), Semver::new(1, 14, 0));
        assert_eq!(Semver::parse("1.14.2.9"), Semver::new(1, 14, 2));
    }

    #[test]
    fn malformed_becomes_zero() {
        assert_eq!(Semver::parse("not-a-version"), Semver::new(0, 0, 0));
        assert_eq!(Semver::parse(""), Semver::new(0, 0, 0));
    }

    #[test]
    fn prerelease_is_lower_than_stable() {
        assert!(Semver::parse("1.14.0-rc.1") < Semver::parse("1.14.0"));
        assert!(Semver::parse("v7.25.4") > Semver::parse("7.25.4-pre.1"));
    }

    #[test]
    fn prerelease_numeric_ordering() {
        assert!(Semver::parse("1.0.0-alpha.1") < Semver::parse("1.0.0-alpha.2"));
        assert!(Semver::parse("1.0.0-alpha.2") < Semver::parse("1.0.0-alpha.10"));
        assert!(Semver::parse("1.0.0-2") < Semver::parse("1.0.0-11"));
    }

    #[test]
    fn prerelease_alpha_greater_than_numeric() {
        // SemVer 2.0.0 §11.4.3: numeric identifiers have lower precedence
        // than alphanumeric ones.
        assert!(Semver::parse("1.0.0-alpha") > Semver::parse("1.0.0-1"));
        assert!(Semver::parse("1.0.0-alpha.beta") > Semver::parse("1.0.0-alpha.1"));
    }

    #[test]
    fn build_metadata_is_ignored_for_ordering() {
        let a = Semver::parse("1.0.0+build.1");
        let b = Semver::parse("1.0.0+build.2");
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert_eq!(a.build(), Some("build.1"));
    }

    #[test]
    fn prerelease_segment_count_ordering() {
        assert!(Semver::parse("1.0.0-alpha") < Semver::parse("1.0.0-alpha.1"));
        assert!(Semver::parse("1.0.0-alpha.beta") < Semver::parse("1.0.0-beta"));
    }

    #[test]
    fn version_range_helpers() {
        assert!(Semver::parse("1.14.99") <= Semver::parse("1.14.4294967295"));
        assert!(Semver::parse("1.15.0") > Semver::parse("1.14.4294967295"));
    }

    #[test]
    fn standard_string_roundtrip() {
        let v = Semver::parse("v7.25.4-beta.1+sha.abc");
        assert_eq!(v.to_standard_string(Some('v')), "v7.25.4-beta.1+sha.abc");
        assert_eq!(v.core_string(), "7.25.4");
        assert!(v.is_prerelease());
    }
}
