//! Subscription content pipeline: decode, bound and parse a whole payload into
//! profiles while locating per-item failures.
//!
//! Detection order mirrors `ConfigHandler.AddBatchServers` (base64 list,
//! plain list, SIP008, WireGuard, inner `v2rayn://`, structured custom),
//! combined with the resource guards required by the plan (§15).

use domain::Profile;

use crate::error::{ParseIssue, SubError};
use crate::fmt::{self, batch, inner};
use crate::util::{base64_decode, is_base64_string, split_lines, CancellationWatcher};

/// Caller-provided format hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentHint {
    #[default]
    Auto,
    PlainList,
    Base64List,
    Sip008,
    WireGuard,
    Inner,
    Clash,
    Singbox,
    Xray,
    Html,
}

/// Resource limits applied to every parse.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Reject payloads larger than this before decoding (default 32 MiB).
    pub max_content_bytes: usize,
    /// Reject decoded base64 larger than this (default 16 MiB).
    pub max_decompressed_bytes: usize,
    /// Truncate the imported list beyond this many profiles (default 10_000).
    pub max_items: usize,
    /// Recursion bound for nested Xray/sing-box arrays (default 16).
    pub max_recursion_depth: usize,
    /// Subscription id used to resolve the `self` sentinel in inner URIs.
    pub subid: String,
    /// Optional cooperative cancellation observed per item.
    pub cancellation: CancellationWatcher,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_content_bytes: 32 * 1024 * 1024,
            max_decompressed_bytes: 16 * 1024 * 1024,
            max_items: 10_000,
            max_recursion_depth: 16,
            subid: String::new(),
            cancellation: CancellationWatcher::default(),
        }
    }
}

/// The format the parser settled on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedFormat {
    PlainList,
    Base64List,
    Sip008,
    WireGuard,
    Inner,
    V2ray,
    Singbox,
    Clash,
    HtmlPage,
    Hysteria2,
}

impl ParsedFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            ParsedFormat::PlainList => "plain_list",
            ParsedFormat::Base64List => "base64_list",
            ParsedFormat::Sip008 => "sip008",
            ParsedFormat::WireGuard => "wireguard",
            ParsedFormat::Inner => "inner",
            ParsedFormat::V2ray => "v2ray",
            ParsedFormat::Singbox => "singbox",
            ParsedFormat::Clash => "clash",
            ParsedFormat::HtmlPage => "htmlpage",
            ParsedFormat::Hysteria2 => "hysteria2",
        }
    }
}

/// Parse output: successful profiles plus located errors and warnings.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub profiles: Vec<Profile>,
    pub errors: Vec<ParseIssue>,
    pub warnings: Vec<ParseIssue>,
    pub detected: Option<ParsedFormat>,
}

impl ParseResult {
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}

/// Parse subscription content.
pub fn parse_content(content: &str, hint: ContentHint, opts: &ParseOptions) -> ParseResult {
    let mut result = ParseResult::default();
    if content.is_empty() {
        result
            .errors
            .push(ParseIssue::new(SubError::Empty.code(), "empty content"));
        return result;
    }
    if content.len() > opts.max_content_bytes {
        result.errors.push(ParseIssue::new(
            SubError::LimitExceeded {
                kind: "content_bytes",
                value: content.len(),
                max: opts.max_content_bytes,
            }
            .code(),
            "content exceeds max_content_bytes",
        ));
        return result;
    }

    match hint {
        ContentHint::Auto => parse_auto(content, opts, &mut result),
        ContentHint::PlainList => {
            let items = parse_plain_list(content, opts, &mut result.errors);
            set_detected(&mut result, ParsedFormat::PlainList, items);
        }
        ContentHint::Base64List => parse_base64_list(content, opts, &mut result),
        ContentHint::Sip008 => parse_sip008(content, &mut result),
        ContentHint::WireGuard => parse_wireguard(content, &mut result),
        ContentHint::Inner => parse_inner(content, opts, &mut result),
        ContentHint::Clash => parse_custom(content, opts, &mut result, ParsedFormat::Clash),
        ContentHint::Singbox => parse_singbox(content, opts, &mut result),
        ContentHint::Xray => parse_xray(content, opts, &mut result),
        ContentHint::Html => {
            if batch::is_html_page(content) {
                result.detected = Some(ParsedFormat::HtmlPage);
                result.warnings.push(ParseIssue::new(
                    "E_SUB_HTML_PAGE",
                    "content is an HTML page, no profiles extracted",
                ));
            } else {
                result.errors.push(ParseIssue::new(
                    SubError::Unsupported("html".into()).code(),
                    "content is not an HTML page",
                ));
            }
        }
    }

    apply_item_limit(&mut result, opts);
    result
}

fn parse_auto(content: &str, opts: &ParseOptions, result: &mut ParseResult) {
    // 1..3: base64 list / plain list.
    let (common, format) = find_common(content, opts, &mut result.errors);
    let mut found = !common.is_empty();
    if found {
        result.detected = format;
    }
    result.profiles.extend(common);

    // 4..5: SIP008 / WireGuard only when nothing else matched yet.
    if !found {
        if let Some(profiles) = crate::fmt::shadowsocks::resolve_sip008(content) {
            result.detected = Some(ParsedFormat::Sip008);
            result.profiles.extend(profiles);
            found = true;
        }
    }
    if !found {
        if let Some(profiles) = crate::fmt::wireguard::resolve_config(content) {
            result.detected = Some(ParsedFormat::WireGuard);
            result.profiles.extend(profiles);
            found = true;
        }
    }

    // 6: inner URIs may be mixed with standard ones.
    let inner_items = find_inner(content, opts);
    if !inner_items.is_empty() {
        result.profiles.extend(inner_items);
        if result.detected.is_none() {
            result.detected = Some(ParsedFormat::Inner);
        }
        found = true;
    }

    // 7: structured custom formats.
    if !found {
        let custom = batch::resolve_custom(content, None, opts.max_recursion_depth);
        if !custom.is_empty() {
            let format = custom_format_of(content);
            result.profiles.extend(custom);
            result.detected = Some(format);
            found = true;
        }
    }

    if !found {
        result.detected =
            format.or_else(|| batch::is_html_page(content).then_some(ParsedFormat::HtmlPage));
        result.errors.push(ParseIssue::new(
            SubError::Unsupported("unknown subscription format".into()).code(),
            "no profile could be parsed from the content",
        ));
    }
}

fn custom_format_of(content: &str) -> ParsedFormat {
    if batch::is_html_page(content) {
        ParsedFormat::HtmlPage
    } else if batch::is_clash_full(content) {
        ParsedFormat::Clash
    } else if batch::is_hysteria2_full(content) {
        ParsedFormat::Hysteria2
    } else {
        ParsedFormat::V2ray
    }
}

fn find_common(
    content: &str,
    opts: &ParseOptions,
    errors: &mut Vec<ParseIssue>,
) -> (Vec<Profile>, Option<ParsedFormat>) {
    // A successful attempt still reports its located per-line failures, so a
    // partial import keeps both the good nodes and the error positions.
    if is_base64_string(content) {
        if let Ok(decoded) = base64_decode(content) {
            if decoded.len() <= opts.max_decompressed_bytes {
                let mut attempt = Vec::new();
                let items = parse_plain_list(&decoded, opts, &mut attempt);
                if !items.is_empty() {
                    errors.extend(attempt);
                    return (items, Some(ParsedFormat::Base64List));
                }
            }
        }
    }
    let mut attempt = Vec::new();
    let items = parse_plain_list(content, opts, &mut attempt);
    if !items.is_empty() {
        errors.extend(attempt);
        return (items, Some(ParsedFormat::PlainList));
    }
    if let Ok(decoded) = base64_decode(content) {
        if decoded.len() <= opts.max_decompressed_bytes {
            let mut fallback = Vec::new();
            let items = parse_plain_list(&decoded, opts, &mut fallback);
            if !items.is_empty() {
                errors.extend(fallback);
                return (items, Some(ParsedFormat::Base64List));
            }
        }
    }
    errors.extend(attempt);
    (Vec::new(), None)
}

fn find_inner(content: &str, opts: &ParseOptions) -> Vec<Profile> {
    let direct = inner::parse(content, &opts.subid);
    if !direct.is_empty() {
        return direct;
    }
    if let Ok(decoded) = base64_decode(content) {
        if decoded.len() <= opts.max_decompressed_bytes {
            return inner::parse(&decoded, &opts.subid);
        }
    }
    Vec::new()
}

fn parse_plain_list(
    content: &str,
    opts: &ParseOptions,
    errors: &mut Vec<ParseIssue>,
) -> Vec<Profile> {
    let mut profiles = Vec::new();
    let mut offset = 0usize;
    for (index, line) in split_lines(content).iter().enumerate() {
        let line_start = offset;
        offset += line.len() + 1;
        if let Err(err) = opts.cancellation.check() {
            errors.push(ParseIssue::new(err.code(), err.to_string()));
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            errors.push(ParseIssue::new(
                "E_SUB_URL_SKIPPED",
                "subscription URL line is not a node and was skipped",
            ));
            continue;
        }
        match fmt::resolve_uri(trimmed) {
            Ok(profile) => profiles.push(profile),
            Err(err) => errors.push(ParseIssue::from_error(&err, index, line_start)),
        }
    }
    profiles
}

fn parse_base64_list(content: &str, opts: &ParseOptions, result: &mut ParseResult) {
    let decoded = match base64_decode(content) {
        Ok(decoded) => decoded,
        Err(err) => {
            result
                .errors
                .push(ParseIssue::new(err.code(), err.to_string()));
            return;
        }
    };
    if decoded.len() > opts.max_decompressed_bytes {
        result.errors.push(ParseIssue::new(
            SubError::LimitExceeded {
                kind: "decompressed_bytes",
                value: decoded.len(),
                max: opts.max_decompressed_bytes,
            }
            .code(),
            "decoded content exceeds max_decompressed_bytes",
        ));
        return;
    }
    let items = parse_plain_list(&decoded, opts, &mut result.errors);
    set_detected(result, ParsedFormat::Base64List, items);
}

fn parse_sip008(content: &str, result: &mut ParseResult) {
    match crate::fmt::shadowsocks::resolve_sip008(content) {
        Some(profiles) => {
            result.detected = Some(ParsedFormat::Sip008);
            result.profiles.extend(profiles);
        }
        None => result.errors.push(ParseIssue::new(
            SubError::Unsupported("sip008".into()).code(),
            "content is not a SIP008 server list",
        )),
    }
}

fn parse_wireguard(content: &str, result: &mut ParseResult) {
    match crate::fmt::wireguard::resolve_config(content) {
        Some(profiles) => {
            result.detected = Some(ParsedFormat::WireGuard);
            result.profiles.extend(profiles);
        }
        None => result.errors.push(ParseIssue::new(
            SubError::Unsupported("wireguard conf".into()).code(),
            "content is not a WireGuard config",
        )),
    }
}

fn parse_inner(content: &str, opts: &ParseOptions, result: &mut ParseResult) {
    let items = inner::parse(content, &opts.subid);
    if items.is_empty() {
        result.errors.push(ParseIssue::new(
            SubError::Unsupported("inner uri".into()).code(),
            "no v2rayn:// entry parsed",
        ));
    } else {
        result.detected = Some(ParsedFormat::Inner);
        result.profiles.extend(items);
    }
}

fn parse_custom(
    content: &str,
    opts: &ParseOptions,
    result: &mut ParseResult,
    format: ParsedFormat,
) {
    let items = batch::resolve_custom(content, None, opts.max_recursion_depth);
    if items.is_empty() {
        result.errors.push(ParseIssue::new(
            SubError::Unsupported("custom".into()).code(),
            "content is not a supported structured config",
        ));
    } else {
        result.detected = Some(format);
        result.profiles.extend(items);
    }
}

fn parse_singbox(content: &str, opts: &ParseOptions, result: &mut ParseResult) {
    let mut items = batch::singbox_resolve(content, None, false, opts.max_recursion_depth);
    if items.is_empty() {
        items = batch::singbox_resolve(content, None, true, opts.max_recursion_depth);
    }
    if items.is_empty() {
        result.errors.push(ParseIssue::new(
            SubError::Unsupported("singbox".into()).code(),
            "content is not a sing-box config",
        ));
    } else {
        result.detected = Some(ParsedFormat::Singbox);
        result.profiles.extend(items);
    }
}

fn parse_xray(content: &str, opts: &ParseOptions, result: &mut ParseResult) {
    let mut items = batch::v2ray_resolve(content, None, false, opts.max_recursion_depth);
    if items.is_empty() {
        items = batch::v2ray_resolve(content, None, true, opts.max_recursion_depth);
    }
    if items.is_empty() {
        result.errors.push(ParseIssue::new(
            SubError::Unsupported("xray".into()).code(),
            "content is not an Xray config",
        ));
    } else {
        result.detected = Some(ParsedFormat::V2ray);
        result.profiles.extend(items);
    }
}

fn set_detected(result: &mut ParseResult, format: ParsedFormat, items: Vec<Profile>) {
    if !items.is_empty() {
        result.detected = Some(format);
        result.profiles.extend(items);
    }
}

fn apply_item_limit(result: &mut ParseResult, opts: &ParseOptions) {
    if result.profiles.len() > opts.max_items {
        let overflow = result.profiles.len() - opts.max_items;
        result.profiles.truncate(opts.max_items);
        result.warnings.push(ParseIssue::new(
            SubError::LimitExceeded {
                kind: "items",
                value: opts.max_items + overflow,
                max: opts.max_items,
            }
            .code(),
            "profile list truncated to max_items",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> ParseOptions {
        ParseOptions::default()
    }

    #[test]
    fn plain_list_partial_success_locates_bad_lines() {
        let content = "vmess://bad\nvless://uuid@a.example:443?encryption=none#ok\n";
        let result = parse_content(content, ContentHint::PlainList, &opts());
        assert_eq!(result.profiles.len(), 1);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].item_index, Some(0));
    }

    #[test]
    fn base64_list_is_detected() {
        let inner = "vless://uuid@a.example:443?encryption=none#one\n";
        let encoded = crate::util::base64_encode(inner);
        let result = parse_content(&encoded, ContentHint::Auto, &opts());
        assert_eq!(result.detected, Some(ParsedFormat::Base64List));
        assert_eq!(result.profiles.len(), 1);
    }

    #[test]
    fn content_limit_is_enforced() {
        let mut options = opts();
        options.max_content_bytes = 4;
        let result = parse_content("vmess://long-enough", ContentHint::Auto, &options);
        assert!(result.profiles.is_empty());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn item_limit_truncates_with_warning() {
        let mut options = opts();
        options.max_items = 1;
        let content = "vless://a@a.example:443?encryption=none#1\nvless://b@b.example:443?encryption=none#2\n";
        let result = parse_content(content, ContentHint::PlainList, &options);
        assert_eq!(result.profiles.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn unknown_content_reports_error() {
        let result = parse_content("just some free text", ContentHint::Auto, &opts());
        assert!(result.profiles.is_empty());
        assert!(!result.errors.is_empty());
    }
}
