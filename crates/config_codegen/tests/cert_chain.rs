//! SP28-L1-002: profile certificate material is a CHAIN (leaf + CA),
//! not leaf-only, with fail-closed behavior on malformed chains.
//!
//! All fixtures are synthetic base64 bodies (`TGVhZg==` decodes to "Leaf",
//! `Q0E=` to "CA"); no real certificates are used.

mod common;

use common::*;
use config_codegen::input::ConfigType;
use config_codegen::{generate_singbox, generate_xray};
use serde_json::json;

/// Synthetic leaf body ("Leaf").
const LEAF_BODY: &str = "TGVhZg==";
/// Synthetic CA body ("CA").
const CA_BODY: &str = "Q0E=";

fn leaf_pem() -> String {
    format!("-----BEGIN CERTIFICATE-----\n{LEAF_BODY}\n-----END CERTIFICATE-----")
}

fn ca_pem() -> String {
    format!("-----BEGIN CERTIFICATE-----\n{CA_BODY}\n-----END CERTIFICATE-----")
}

fn chain_pem() -> String {
    format!("{}\n{}", leaf_pem(), ca_pem())
}

fn tls_vless(cert: &str) -> config_codegen::input::CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.70", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.network = "raw".into();
    p.stream_security = "tls".into();
    p.cert = cert.into();
    p
}

#[test]
fn xray_emits_chain_in_order() {
    let generated = generate_xray(&codegen_input(tls_vless(&chain_pem()))).expect("chain");
    let main = &generated.main;
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/disableSystemRoot"
        ),
        json!(true)
    );
    // Leaf first, CA second: order is significant for chain building.
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/certificates/0/certificate"
        ),
        json!([
            "-----BEGIN CERTIFICATE-----",
            LEAF_BODY,
            "-----END CERTIFICATE-----"
        ])
    );
    assert_eq!(
        json_at(
            main,
            "/outbounds/0/streamSettings/tlsSettings/certificates/1/certificate"
        ),
        json!([
            "-----BEGIN CERTIFICATE-----",
            CA_BODY,
            "-----END CERTIFICATE-----"
        ])
    );
    assert!(json_at(
        main,
        "/outbounds/0/streamSettings/tlsSettings/certificates/2"
    )
    .is_null());
}

#[test]
fn singbox_emits_chain_in_order() {
    let generated = generate_singbox(&codegen_input(tls_vless(&chain_pem()))).expect("chain");
    let main = &generated.main;
    assert_eq!(
        json_at(main, "/outbounds/0/tls/certificate"),
        json!([leaf_pem(), ca_pem()])
    );
    assert_eq!(json_at(main, "/outbounds/0/tls/insecure"), json!(false));
}

#[test]
fn leaf_only_still_works() {
    let xray = generate_xray(&codegen_input(tls_vless(&leaf_pem()))).expect("leaf");
    assert_eq!(
        json_at(
            &xray.main,
            "/outbounds/0/streamSettings/tlsSettings/certificates/0/certificate"
        ),
        json!([
            "-----BEGIN CERTIFICATE-----",
            LEAF_BODY,
            "-----END CERTIFICATE-----"
        ])
    );
    assert!(json_at(
        &xray.main,
        "/outbounds/0/streamSettings/tlsSettings/certificates/1"
    )
    .is_null());

    let singbox = generate_singbox(&codegen_input(tls_vless(&leaf_pem()))).expect("leaf");
    assert_eq!(
        json_at(&singbox.main, "/outbounds/0/tls/certificate"),
        json!([leaf_pem()])
    );
}

#[test]
fn empty_cert_means_no_pin_and_no_error() {
    let xray = generate_xray(&codegen_input(tls_vless(""))).expect("empty cert");
    assert!(json_at(
        &xray.main,
        "/outbounds/0/streamSettings/tlsSettings/certificates"
    )
    .is_null());
    let singbox = generate_singbox(&codegen_input(tls_vless(""))).expect("empty cert");
    assert!(json_at(&singbox.main, "/outbounds/0/tls/certificate").is_null());
}

#[test]
fn malformed_chain_fails_closed_with_structured_error() {
    let cases = [
        ("no markers at all", "this is not a certificate"),
        (
            "unterminated block",
            "-----BEGIN CERTIFICATE-----\nTGVhZg==\n",
        ),
        (
            "empty body",
            "-----BEGIN CERTIFICATE-----\n\n-----END CERTIFICATE-----",
        ),
        (
            "non-base64 body",
            "-----BEGIN CERTIFICATE-----\n!!!not-base64!!!\n-----END CERTIFICATE-----",
        ),
        (
            "bad padding",
            "-----BEGIN CERTIFICATE-----\nAB=C\n-----END CERTIFICATE-----",
        ),
        (
            "broken second block",
            &format!("{}\n-----BEGIN CERTIFICATE-----\n!!!\n", leaf_pem()),
        ),
    ];
    for (name, cert) in cases {
        let xray_err = generate_xray(&codegen_input(tls_vless(cert))).expect_err(name);
        assert_eq!(xray_err.code, "invalid_certificate_chain", "{name}");
        assert_eq!(
            xray_err.field_path.as_deref(),
            Some("profile.cert"),
            "{name}"
        );
        let singbox_err = generate_singbox(&codegen_input(tls_vless(cert))).expect_err(name);
        assert_eq!(singbox_err.code, "invalid_certificate_chain", "{name}");
        assert_eq!(
            singbox_err.field_path.as_deref(),
            Some("profile.cert"),
            "{name}"
        );
    }
}

#[test]
fn reality_ignores_cert_material() {
    // The fail-closed gate only guards the TLS consumer; reality never reads
    // `cert`, so stale text there must not break generation.
    let mut p = tls_vless("not a certificate");
    p.stream_security = "reality".into();
    p.public_key = "synthetic-public-key".into();
    generate_xray(&codegen_input(p.clone())).expect("reality ignores cert");
    generate_singbox(&codegen_input(p)).expect("reality ignores cert");
}

#[test]
fn shadowsocks_plugin_keeps_upstream_leaf_semantics() {
    // Upstream `SingboxOutboundService` emits only `certs.First()` into the
    // single-cert `certRaw` plugin option; the generator mirrors that here.
    // The full chain remains available on the main TLS paths above.
    let mut p = profile(ConfigType::Shadowsocks, "192.0.2.71", 443);
    p.password = "test-password".into();
    p.proto_extra.ss_method = Some("aes-256-gcm".into());
    p.network = "ws".into();
    p.transport_extra.host = Some("ws.example.test".into());
    p.transport_extra.path = Some("/p".into());
    p.stream_security = "tls".into();
    p.cert = chain_pem();
    let generated = generate_singbox(&codegen_input(p)).expect("ss plugin");
    let opts = string_at(&generated.main, "/outbounds/0/plugin_opts");
    assert_eq!(
        string_at(&generated.main, "/outbounds/0/plugin"),
        "v2ray-plugin"
    );
    // Leaf base64 with `=` escaped per plugin option syntax; CA absent.
    assert!(opts.contains("certRaw=TGVhZg\\=\\="), "{opts}");
    assert!(!opts.contains(CA_BODY), "{opts}");
}
