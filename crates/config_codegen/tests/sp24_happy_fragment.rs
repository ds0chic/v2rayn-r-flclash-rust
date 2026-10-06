//! SP-24 field instances: Happy Eyeballs gate (FLD-CFG-176..180) and
//! Fragment wire semantics (FLD-CFG-150..153).
//!
//! Upstream truth (`7d6a967`): `FillSockoptDomainStrategy` always writes a
//! non-empty `domainStrategy` and writes the `happyEyeballs` block only when
//! `SimpleDNSItem.EnableHappyEyeballs` is true. `BuildFragmentsMasks` falls
//! back to `tlshello` / `["50-100"]` / `["10-20"]` and takes the first int of
//! a `maxSplit` range string.
//!
//! Synthetic fixtures only; ports >= 11808.

mod common;

use common::*;
use config_codegen::generate_xray;
use config_codegen::input::*;
use serde_json::json;

fn tls_vless() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.90", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "tls".into();
    p
}

fn dns_with(simple: SimpleDns, happy: HappyEyeballs4Ray) -> CodegenInput {
    let mut input = codegen_input(tls_vless());
    input.dns = Some(CodegenDns {
        simple,
        ..Default::default()
    });
    input.settings.happy_eyeballs4_ray = happy;
    input
}

fn happy_params() -> HappyEyeballs4Ray {
    HappyEyeballs4Ray {
        try_delay_ms: Some(300),
        prioritize_ipv6: Some(true),
        interleave: Some(2),
        max_concurrent_try: Some(3),
    }
}

fn happy_paths(main: &serde_json::Value) -> Vec<String> {
    main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o.pointer("/streamSettings/sockopt/happyEyeballs"))
        .map(|v| v.to_string())
        .collect()
}

// FLD-CFG-176 (gate, freedom site): OFF + non-default params => no block.
#[test]
fn sp24_happy_off_hides_block_freedom() {
    let input = dns_with(
        SimpleDns {
            strategy4_freedom: Some("UseIP".into()),
            enable_happy_eyeballs: false,
            ..Default::default()
        },
        happy_params(),
    );
    let main = generate_xray(&input).expect("xray").main;
    let freedom = main["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o.get("protocol").and_then(|v| v.as_str()) == Some("freedom"))
        .expect("freedom outbound");
    assert_eq!(
        freedom.pointer("/streamSettings/sockopt/domainStrategy"),
        Some(&json!("UseIP")),
        "{freedom}"
    );
    assert!(
        happy_paths(&main).is_empty(),
        "OFF must not emit happyEyeballs: {:?}",
        happy_paths(&main)
    );
}

// FLD-CFG-176 (gate, dial site): OFF + non-default params => no block.
#[test]
fn sp24_happy_off_hides_block_dial() {
    let input = dns_with(
        SimpleDns {
            strategy4_proxy_dial: Some("UseIP".into()),
            enable_happy_eyeballs: false,
            ..Default::default()
        },
        happy_params(),
    );
    let main = generate_xray(&input).expect("xray").main;
    assert!(happy_paths(&main).is_empty(), "{:?}", happy_paths(&main));
}

// FLD-CFG-177..180 (params, dial site): ON + values => block with values.
#[test]
fn sp24_happy_on_emits_params() {
    let input = dns_with(
        SimpleDns {
            strategy4_proxy_dial: Some("UseIP".into()),
            enable_happy_eyeballs: true,
            ..Default::default()
        },
        happy_params(),
    );
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/tryDelayMs"
        ),
        json!(300)
    );
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/prioritizeIPv6"
        ),
        json!(true)
    );
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/interleave"
        ),
        json!(2)
    );
    assert_eq!(
        json_at(
            &main,
            "/outbounds/0/streamSettings/sockopt/happyEyeballs/maxConcurrentTry"
        ),
        json!(3)
    );
}

// Upstream `sockopt.happyEyeballs ??= new()`: ON + all-default => empty block.
#[test]
fn sp24_happy_on_with_defaults_emits_empty_block() {
    let input = dns_with(
        SimpleDns {
            strategy4_proxy_dial: Some("UseIP".into()),
            enable_happy_eyeballs: true,
            ..Default::default()
        },
        HappyEyeballs4Ray::default(),
    );
    let main = generate_xray(&input).expect("xray").main;
    assert_eq!(
        json_at(&main, "/outbounds/0/streamSettings/sockopt/happyEyeballs"),
        json!({})
    );
}

// FLD-CFG-150..152: packets/lengths/delays passthrough + empty fallbacks.
#[test]
fn sp24_fragment_packets_lengths_delays() {
    let mut s = settings();
    s.core_basic.enable_fragment = true;
    s.fragment4_ray.packets = Some("tlshello".into());
    s.fragment4_ray.lengths = vec!["50-100".into()];
    s.fragment4_ray.delays = vec!["10-20".into()];
    s.fragment4_ray.max_split = Some("0".into());
    let mut input = codegen_input(tls_vless());
    input.settings = s;
    let main = generate_xray(&input).expect("fragment").main;
    let base = "/outbounds/0/streamSettings/finalmask/tcp/0/settings";
    assert_eq!(string_at(&main, &format!("{base}/packets")), "tlshello");
    assert_eq!(
        json_at(&main, &format!("{base}/lengths")),
        json!(["50-100"])
    );
    assert_eq!(json_at(&main, &format!("{base}/delays")), json!(["10-20"]));
    assert_eq!(string_at(&main, &format!("{base}/length")), "50-100");
    assert_eq!(string_at(&main, &format!("{base}/delay")), "10-20");
    assert_no_live_port(&main);
}

// FLD-CFG-153: wire takes the first int of a range string ("1-3" -> 1).
// Upstream `int.TryParse` tolerates surrounding whitespace, so padded forms
// wire the same first int ("1 - 3" -> 1).
#[test]
fn sp24_fragment_max_split_range_takes_first() {
    for (raw, wire) in [
        ("1-3", 1),
        ("2-5", 2),
        ("7", 7),
        ("", 0),
        ("1 - 3", 1),
        (" 2 ", 2),
    ] {
        let mut s = settings();
        s.core_basic.enable_fragment = true;
        s.fragment4_ray.max_split = Some(raw.into());
        let mut input = codegen_input(tls_vless());
        input.settings = s;
        let main = generate_xray(&input).expect("fragment").main;
        assert_eq!(
            json_at(
                &main,
                "/outbounds/0/streamSettings/finalmask/tcp/0/settings/maxSplit"
            ),
            json!(wire),
            "raw={raw:?}"
        );
    }
}

// Switches off: no fragment mask is emitted.
#[test]
fn sp24_fragment_disabled_emits_no_mask() {
    let mut s = settings();
    s.core_basic.enable_fragment = false;
    s.core_basic.enable_final_fragment = false;
    s.fragment4_ray.max_split = Some("1-3".into());
    let mut input = codegen_input(tls_vless());
    input.settings = s;
    let main = generate_xray(&input).expect("fragment").main;
    assert!(
        !main.to_string().contains("finalmask"),
        "{}",
        main.to_string()
    );
}
