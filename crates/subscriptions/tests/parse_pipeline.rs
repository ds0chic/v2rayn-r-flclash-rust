//! Pipeline tests: format detection, partial success with located errors,
//! resource limits and cancellation.

use domain::CancellationToken;
use subscriptions::fmt;
use subscriptions::parse::{parse_content, ContentHint, ParseOptions, ParsedFormat};
use subscriptions::util::CancellationWatcher;

fn opts() -> ParseOptions {
    ParseOptions::default()
}

#[test]
fn plain_list_partial_success_locates_bad_lines() {
    let content = "vmess://broken\nvless://uuid@a.example:443?encryption=none#ok\n";
    let result = parse_content(content, ContentHint::PlainList, &opts());
    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].item_index, Some(0));
    assert_eq!(result.detected, Some(ParsedFormat::PlainList));
}

#[test]
fn auto_partial_list_keeps_good_nodes_and_reports_bad() {
    let content = "vmess://broken\nvless://uuid@a.example:443?encryption=none#ok\n";
    let result = parse_content(content, ContentHint::Auto, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::PlainList));
    assert_eq!(result.profiles.len(), 1);
    assert!(!result.errors.is_empty());
}

#[test]
fn auto_detects_base64_list() {
    let encoded =
        subscriptions::util::base64_encode("vless://uuid@a.example:443?encryption=none#one\n");
    let result = parse_content(&encoded, ContentHint::Auto, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::Base64List));
    assert_eq!(result.profiles.len(), 1);
}

#[test]
fn auto_mixes_standard_and_inner_uris() {
    let mut item = domain::Profile {
        index_id: "id-1".into(),
        config_type: domain::ConfigType::Socks,
        remarks: "inner node".into(),
        address: "127.0.0.1".into(),
        port: 1080,
        ..domain::Profile::default()
    };
    item.username = "u".into();
    let inner = fmt::inner::emit(&[item]).unwrap();
    let content = format!("vless://uuid@a.example:443?encryption=none#std\n{inner}");
    let result = parse_content(&content, ContentHint::Auto, &opts());
    assert_eq!(result.profiles.len(), 2);
}

#[test]
fn sip008_json_is_parsed() {
    let json = r#"{"servers":[{"remarks":"a","server":"1.2.3.4","server_port":"8388","method":"aes-128-gcm","password":"p"}]}"#;
    let result = parse_content(json, ContentHint::Sip008, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::Sip008));
    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.profiles[0].port, 8388);
}

#[test]
fn wireguard_conf_is_parsed() {
    let conf = "[Interface]\nPrivateKey = k\nAddress = 10.0.0.2/32\n[Peer]\nPublicKey = p\nEndpoint = example.com:2408\n";
    let result = parse_content(conf, ContentHint::WireGuard, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::WireGuard));
    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.profiles[0].remarks, "WireGuard Peer 1");
}

#[test]
fn xray_full_config_is_detected() {
    let data = r#"{"inbounds":[{"port":1080,"protocol":"socks"}],"outbounds":[{"protocol":"vmess","tag":"p","settings":{},"streamSettings":{}}]}"#;
    let result = parse_content(data, ContentHint::Auto, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::V2ray));
    assert_eq!(result.profiles[0].config_type, domain::ConfigType::Custom);
    assert!(result.profiles[0].extra.contains_key("RawConfig"));
}

#[test]
fn singbox_outbound_is_detected() {
    let data = r#"{"outbounds":[{"type":"vless","tag":"p","server":"a","server_port":443}]}"#;
    let result = parse_content(data, ContentHint::Singbox, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::Singbox));
    assert_eq!(result.profiles[0].config_type, domain::ConfigType::Outbound);
}

#[test]
fn clash_detection_requires_all_markers() {
    let full = "proxies:\n  - name: a\nrules:\n  - MATCH,DIRECT\n-port: 7890\n";
    let result = parse_content(full, ContentHint::Auto, &opts());
    assert_eq!(result.detected, Some(ParsedFormat::Clash));
    let not_clash = "proxies: []\n";
    let result = parse_content(not_clash, ContentHint::Auto, &opts());
    assert!(result.profiles.is_empty());
}

#[test]
fn html_page_is_flagged_not_imported() {
    let html = "<!doctype html><html><head></head><body>hi</body></html>";
    let result = parse_content(html, ContentHint::Html, &opts());
    assert!(result.profiles.is_empty());
    assert_eq!(result.detected, Some(ParsedFormat::HtmlPage));
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn content_and_item_limits_are_enforced() {
    let mut options = opts();
    options.max_content_bytes = 8;
    let result = parse_content("a very long payload", ContentHint::Auto, &options);
    assert_eq!(result.errors.len(), 1);
    assert!(result.profiles.is_empty());

    let mut options = opts();
    options.max_items = 1;
    let content =
        "vless://a@a.example:443?encryption=none#1\nvless://b@b.example:443?encryption=none#2\n";
    let result = parse_content(content, ContentHint::PlainList, &options);
    assert_eq!(result.profiles.len(), 1);
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn inner_self_sentinel_uses_option_subid() {
    let mut group = domain::Profile {
        index_id: "g".into(),
        config_type: domain::ConfigType::PolicyGroup,
        remarks: "group".into(),
        ..domain::Profile::default()
    };
    group.proto_extra.sub_child_items = Some("original".into());
    let uri = fmt::inner::emit(&[group]).unwrap();
    let mut options = opts();
    options.subid = "sub-42".into();
    let result = parse_content(&uri, ContentHint::Inner, &options);
    assert_eq!(
        result.profiles[0].proto_extra.sub_child_items.as_deref(),
        Some("sub-42")
    );
}

#[test]
fn pre_cancelled_token_stops_the_pipeline() {
    let token = CancellationToken::new();
    token.cancel();
    let mut options = opts();
    options.cancellation = CancellationWatcher::new(Some(token));
    let content =
        "vless://a@a.example:443?encryption=none#1\nvless://b@b.example:443?encryption=none#2\n";
    let result = parse_content(content, ContentHint::PlainList, &options);
    assert!(result.profiles.is_empty());
    assert!(result.errors.iter().any(|e| e.code == "E_CANCELLED"));
}
