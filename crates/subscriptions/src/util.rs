//! Low-level helpers that mirror the upstream `ServiceLib.Common.Utils`,
//! `NameValueCollection` query parsing and `CertPemManager` behaviour that the
//! Fmt codecs depend on.

use std::collections::HashMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use sha2::{Digest, Sha256};

use crate::error::SubError;

/// `Uri.EscapeDataString`: encode everything except the RFC 3986 unreserved set.
const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Percent-encode a value the way upstream `Utils.UrlEncode` does.
pub fn url_encode(value: &str) -> String {
    utf8_percent_encode(value, UNRESERVED).to_string()
}

/// Percent-decode a value once. Invalid sequences are left untouched, matching
/// `Uri.UnescapeDataString`.
pub fn url_decode(value: &str) -> String {
    percent_decode_str(value).decode_utf8_lossy().into_owned()
}

/// Standard base64 with padding, as upstream `Utils.Base64Encode`.
pub fn base64_encode(value: &str) -> String {
    STANDARD.encode(value.as_bytes())
}

/// Standard base64 without trailing padding (`removePadding = true`).
pub fn base64_encode_nopad(value: &str) -> String {
    let mut out = base64_encode(value);
    while out.ends_with('=') {
        out.pop();
    }
    out
}

/// Tolerant base64 decode: trims whitespace, accepts URL-safe alphabet and
/// missing padding, exactly like `Utils.Base64Decode`.
pub fn base64_decode(value: &str) -> Result<String, SubError> {
    let cleaned: String = value.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Ok(String::new());
    }
    let swapped: String = cleaned
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .filter(|c| *c != '=')
        .collect();
    let pad = (4 - (swapped.len() % 4)) % 4;
    let mut padded = swapped;
    for _ in 0..pad {
        padded.push('=');
    }
    let bytes = STANDARD
        .decode(padded.as_bytes())
        .map_err(|_| SubError::Decode("base64".into()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// URL-safe base64 without padding, used by the inner `v2rayn://` format.
pub fn base64_urlsafe_nopad(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Standard base64 of raw bytes without padding (upstream reproducible ids).
pub fn base64_encode_nopad_bytes(bytes: &[u8]) -> String {
    let mut out = STANDARD.encode(bytes);
    while out.ends_with('=') {
        out.pop();
    }
    out
}

/// Strict-ish base64 detection close to `Convert.TryFromBase64String`.
pub fn is_base64_string(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.len().is_multiple_of(4) {
        return false;
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
    {
        return false;
    }
    STANDARD.decode(trimmed.as_bytes()).is_ok()
}

/// Case-insensitive, insertion-ordered query map whose values are already
/// percent-decoded once (upstream `Utils.ParseQueryString`).
#[derive(Debug, Clone, Default)]
pub struct Query {
    pairs: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

impl Query {
    /// Parse a raw query (with or without a leading `?`). Pairs without `=` are
    /// skipped; only the first occurrence of a key is addressable via [`get`],
    /// but every occurrence is retained for [`get_all`].
    ///
    /// [`get`]: Query::get
    /// [`get_all`]: Query::get_all
    pub fn parse(raw: &str) -> Self {
        let mut query = Query::default();
        let body = raw.strip_prefix('?').unwrap_or(raw);
        if body.is_empty() {
            return query;
        }
        for part in body.split('&') {
            if part.is_empty() {
                continue;
            }
            let Some((key, value)) = part.split_once('=') else {
                continue;
            };
            let key = url_decode(key);
            let value = url_decode(value);
            let lower = key.to_ascii_lowercase();
            query.pairs.push((key, value));
            query.index.entry(lower).or_insert(query.pairs.len() - 1);
        }
        query
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// First value for `key` (case-insensitive), or `None`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.index
            .get(&key.to_ascii_lowercase())
            .map(|i| self.pairs[*i].1.as_str())
    }

    /// First value for `key`, or `default` when absent/empty.
    pub fn get_or<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        match self.get(key) {
            Some(v) if !v.is_empty() => v,
            _ => default,
        }
    }

    /// All values for `key`, in insertion order.
    pub fn get_all(&self, key: &str) -> Vec<&str> {
        let lower = key.to_ascii_lowercase();
        self.pairs
            .iter()
            .filter(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.pairs.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// `true` when the string parses as an IPv6 literal.
pub fn is_ipv6(address: &str) -> bool {
    address
        .parse::<std::net::Ipv6Addr>()
        .map(|_| true)
        .unwrap_or(false)
}

/// Add square brackets around an IPv6 literal if not already present.
pub fn bracket_ipv6(address: &str) -> String {
    if is_ipv6(address) {
        if address.starts_with('[') && address.ends_with(']') {
            address.to_string()
        } else {
            format!("[{address}]")
        }
    } else {
        address.to_string()
    }
}

/// Normalize CRLF/CR to LF and split into owned lines.
pub fn split_lines(content: &str) -> Vec<&str> {
    content
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect()
}

/// Comma-separated list to `Vec`, dropping empty entries (upstream
/// `String2List`).
pub fn string2list(value: &str) -> Vec<String> {
    let flattened: String = value.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    flattened
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Join a list with commas (upstream `List2String`).
pub fn list2string(values: &[String]) -> String {
    values.join(",")
}

/// Regex match with the upstream fail-open guard: an empty pattern matches
/// everything, an empty input never matches, and an invalid/oversized pattern
/// returns `true` instead of dropping a node.
///
/// The Rust `regex` engine is linear-time, so this is inherently ReDoS-safe; the
/// size cap is a second line of defence for hostile patterns.
pub fn is_regex_match(input: &str, pattern: &str, max_pattern_len: usize) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if pattern.len() > max_pattern_len {
        return true;
    }
    if input.is_empty() {
        return false;
    }
    match regex::RegexBuilder::new(pattern)
        .size_limit(8 * 1024 * 1024)
        .dfa_size_limit(8 * 1024 * 1024)
        .build()
    {
        Ok(re) => re.is_match(input),
        Err(_) => true,
    }
}

/// Decode a response body using the charset advertised in `Content-Type`.
/// Falls back to UTF-8 with replacement when the charset is unknown.
pub fn decode_body_bytes(bytes: &[u8], content_type: Option<&str>) -> String {
    let charset = content_type
        .and_then(|ct| {
            ct.split(';').map(str::trim).find_map(|part| {
                let lower = part.to_ascii_lowercase();
                lower
                    .strip_prefix("charset=")
                    .map(|c| c.trim_matches('"').to_string())
            })
        })
        .unwrap_or_default();
    if charset.is_empty() || charset.eq_ignore_ascii_case("utf-8") {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let encoding =
        encoding_rs::Encoding::for_label(charset.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (decoded, _, _) = encoding.decode(bytes);
    decoded.into_owned()
}

/// Extract the base64 content of each PEM certificate block.
pub fn pem_cert_bodies(pem_chain: &str) -> Vec<String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let mut certs = Vec::new();
    let mut rest = pem_chain;
    while let Some(begin) = rest.find(BEGIN) {
        let after_begin = &rest[begin + BEGIN.len()..];
        let Some(end) = after_begin.find(END) else {
            break;
        };
        let body: String = after_begin[..end]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if !body.is_empty() {
            certs.push(body);
        }
        rest = &after_begin[end + END.len()..];
    }
    certs
}

/// SHA-256 thumbprint (uppercase hex, no colons) of the first PEM certificate,
/// matching `CertPemManager.GetLeafCertSha256Thumbprint` on a leaf-first chain.
pub fn leaf_cert_sha256_hex(pem_chain: &str) -> Option<String> {
    let body = pem_cert_bodies(pem_chain).into_iter().next()?;
    let der = STANDARD.decode(body.as_bytes()).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&der);
    let digest = hasher.finalize();
    Some(digest.iter().map(|b| format!("{b:02X}")).collect())
}

/// Normalize a JSON text to the compact serialization upstream emits for the
/// `fm` / `extra` query parameters. Returns `None` when the text is not JSON.
pub fn normalize_json_compact(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    Some(value.to_string())
}

/// Pretty JSON with 2-space indentation (upstream `WriteIndented = true`).
pub fn normalize_json_pretty(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

/// Observer around the cooperative [`domain::CancellationToken`] used by the
/// pure pipeline and the downloader. `None` means "never cancelled".
#[derive(Debug, Clone, Default)]
pub struct CancellationWatcher {
    token: Option<domain::CancellationToken>,
}

impl CancellationWatcher {
    pub fn new(token: Option<domain::CancellationToken>) -> Self {
        Self { token }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(domain::CancellationToken::is_cancelled)
    }

    /// Abort the operation at the current safe point if cancellation was asked.
    pub fn check(&self) -> Result<(), SubError> {
        if self.is_cancelled() {
            Err(SubError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_roundtrip_tolerates_urlsafe_and_padding() {
        let encoded = base64_encode("hello 東京");
        assert!(is_base64_string(&encoded));
        assert_eq!(base64_decode(&encoded).unwrap(), "hello 東京");
        let urlsafe = encoded.replace('+', "-").replace('/', "_");
        assert_eq!(base64_decode(&urlsafe).unwrap(), "hello 東京");
        let nopad = encoded.trim_end_matches('=');
        assert_eq!(base64_decode(nopad).unwrap(), "hello 東京");
    }

    #[test]
    fn url_encode_matches_rfc3986_unreserved() {
        assert_eq!(url_encode("a b/c?d=e&f"), "a%20b%2Fc%3Fd%3De%26f");
        assert_eq!(url_encode("AZaz09-._~"), "AZaz09-._~");
        assert_eq!(url_decode("a%20b%2Fc"), "a b/c");
    }

    #[test]
    fn query_splits_on_first_equals_and_keeps_stun_occurrences() {
        let query = Query::parse("?ech=AAj+DQAEAAAAAA==&stun=a:3478&stun=b:3478&sni=real.example");
        assert_eq!(query.get("ech"), Some("AAj+DQAEAAAAAA=="));
        assert_eq!(query.get("SNI"), Some("real.example"));
        assert_eq!(query.get_all("stun"), vec!["a:3478", "b:3478"]);
    }

    #[test]
    fn query_decodes_exactly_once() {
        let query = Query::parse("obfs-password=a%20b&ech=AAj%2BDQAEAAAAAA%3D%3D");
        assert_eq!(query.get("obfs-password"), Some("a b"));
        assert_eq!(query.get("ech"), Some("AAj+DQAEAAAAAA=="));
    }

    #[test]
    fn ipv6_bracketting_is_idempotent() {
        assert_eq!(bracket_ipv6("2001:db8::1"), "[2001:db8::1]");
        assert_eq!(bracket_ipv6("[2001:db8::1]"), "[2001:db8::1]");
        assert_eq!(bracket_ipv6("example.com"), "example.com");
    }

    #[test]
    fn regex_guard_fails_open_on_invalid_and_oversized_patterns() {
        assert!(is_regex_match("node", "(?P<", 1024));
        assert!(is_regex_match("node", &"a".repeat(4096), 1024));
        assert!(is_regex_match("node", "", 1024));
        assert!(!is_regex_match("", "node", 1024));
        assert!(is_regex_match("東京 node", "東京", 1024));
        assert!(!is_regex_match("node", "^us", 1024));
    }

    #[test]
    fn decode_body_bytes_honours_gbk_charset() {
        let (bytes, _, _) = encoding_rs::GBK.encode("东京");
        let decoded = decode_body_bytes(&bytes, Some("text/plain; charset=gbk"));
        assert_eq!(decoded, "东京");
    }

    #[test]
    fn pem_helpers_extract_and_hash() {
        let pem = "-----BEGIN CERTIFICATE-----\nAAEC\n-----END CERTIFICATE-----\n";
        assert_eq!(pem_cert_bodies(pem), vec!["AAEC".to_string()]);
        assert_eq!(
            leaf_cert_sha256_hex(pem),
            Some("AE4B3280E56E2FAF83F414A6E3DABE9D5FBE18976544C05FED121ACCB85B53FC".to_string())
        );
    }
}
