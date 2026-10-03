//! Subscription-conversion request construction.
//!
//! Mirrors upstream `SubscriptionHandler.DownloadMainSubscription` (commit
//! `7d6a967`): the configured converter template carries `{0}` for the
//! URL-encoded source subscription, and `&target=` / `&config=` are appended
//! only when the template does not already carry those keys. The module is pure
//! (no network, no secrets) so it can be unit tested in isolation.

use crate::util::url_encode;

/// `Utils.GetPunycode`: rewrite the host to its punycode form.
///
/// An all-ASCII URL is returned unchanged (upstream `uri.Host == uri.IdnHost`),
/// so only genuinely internationalised hosts are normalised through parsing.
pub fn punycode_url(url: &str) -> String {
    if url.is_ascii() {
        return url.to_string();
    }
    match url::Url::parse(url) {
        Ok(parsed) => parsed.to_string(),
        Err(_) => url.to_string(),
    }
}

/// Build the converter request URL exactly like `DownloadMainSubscription`.
///
/// `template` is the effective `SubConvertUrl` (e.g.
/// `https://sub.xeton.dev/sub?url={0}`); `source_url` is the (already
/// punycoded) subscription URL; `target` is `SubItem.ConvertTarget`; and
/// `convert_config` is the effective `Global.SubConvertConfig` value.
pub fn build_convert_url(
    template: &str,
    source_url: &str,
    target: &str,
    convert_config: &str,
) -> String {
    let encoded = url_encode(source_url);
    let mut url = template.replace("{0}", &encoded);
    if !url.contains("target=") {
        url.push_str("&target=");
        url.push_str(target);
    }
    if !url.contains("config=") {
        url.push_str("&config=");
        url.push_str(convert_config);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_format_url_plus_target_and_config() {
        let url = build_convert_url(
            "https://sub.xeton.dev/sub?url={0}",
            "https://example.com/a?b=c",
            "clash",
            "https://cfg.example/online.ini",
        );
        assert_eq!(
            url,
            "https://sub.xeton.dev/sub?url=https%3A%2F%2Fexample.com%2Fa%3Fb%3Dc&target=clash&config=https://cfg.example/online.ini"
        );
    }

    #[test]
    fn does_not_duplicate_existing_target_or_config() {
        let url = build_convert_url(
            "https://c/sub?url={0}&target=ss",
            "https://example.com/s",
            "clash",
            "https://cfg/x.ini",
        );
        assert_eq!(url.matches("target=").count(), 1);
        // `config=` still absent, so it is appended once.
        assert_eq!(url.matches("config=").count(), 1);
        assert!(url.contains("&config=https://cfg/x.ini"));
        assert!(url.ends_with("&config=https://cfg/x.ini"));
    }

    #[test]
    fn template_without_placeholder_is_left_untouched_then_extended() {
        let url = build_convert_url("https://c/sub", "https://example.com/s", "mixed", "cfg");
        assert_eq!(url, "https://c/sub&target=mixed&config=cfg");
    }

    #[test]
    fn punycode_only_rewrites_non_ascii_hosts() {
        assert_eq!(
            punycode_url("https://example.com/sub"),
            "https://example.com/sub"
        );
        assert_eq!(
            punycode_url("https://xn--fsqu00a.com/sub"),
            "https://xn--fsqu00a.com/sub"
        );
        assert_eq!(
            punycode_url("https://例子.com/sub"),
            "https://xn--fsqu00a.com/sub"
        );
    }
}
