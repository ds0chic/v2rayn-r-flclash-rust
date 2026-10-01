//! T06b validation harness: emits representative Xray / sing-box configs.
//!
//! Writes one JSON config per (core, case) pair plus a `manifest.json` that the
//! `tools/validate/t06b_validate.ps1` script feeds to the real cores via
//! `xray run -test` and `sing-box check`. This example never spawns a process
//! and never touches the network.

use std::collections::BTreeMap;
use std::path::PathBuf;

use config_codegen::input::*;
use config_codegen::{generate_singbox, generate_xray};
use serde_json::{json, Value};

/// Valid x25519 public keys (base64url, 32 bytes). Generated with the real
/// `xray x25519` subcommand so the cores accept them during validation. These
/// are synthetic test keys, never used against a live server.
const REALITY_PUBLIC_KEY: &str = "Aw5wKiqFLgYnwzWnzPkjloOkdKhov13wuE0KfIx3Mi0";
const REALITY_PUBLIC_KEY_2: &str = "-lOSaO18t_1NCDd1mxqoz9mEzQSU_emelDtgHinCWEI";
/// Valid 32-byte base64 WireGuard keys (synthetic).
const WG_PRIVATE_KEY: &str = "SwNwLPGEdv6ObH7M1Iihs5Dlq4I5N+RxEw1vSyJiOyI=";
const WG_PUBLIC_KEY: &str = "T4FiAZaL/srRBtyiLfzrwH0dzxuPiOS4iqBzsPYj+sI=";
const WG_PRESHARED_KEY: &str = "aSCZjZclAGEU2PGAypHYF5PTew6YEeiVGsy6UbqPY+s=";
/// Valid 64-hex sha256 (synthetic certificate pin).
const CERT_SHA: &str = "0d3c61a18544a97916ff01682072bba0f6ca447cd04e4b64c67075795529a35c";

/// A synthetic self-signed certificate (CN=example.test) used only so the real
/// sing-box core accepts the mandatory TLS block for hysteria2/tuic/anytls/naive.
const SELF_SIGNED_CERT: &str = "\
-----BEGIN CERTIFICATE-----
MIIDBjCCAe6gAwIBAgIQajvnUbiCkHXCq0SFSxm58jANBgkqhkiG9w0BAQsFADAX
MRUwEwYDVQQDEwxleGFtcGxlLnRlc3QwHhcNMjYxMDAxMDkwMDA1WhcNMjYxMTAx
MTAwMDA1WjAXMRUwEwYDVQQDEwxleGFtcGxlLnRlc3QwggEiMA0GCSqGSIb3DQEB
AQUAA4IBDwAwggEKAoIBAQCwdZZNECKt+0HjZkqvzxOyYoVwxVKY6EJE2WtfFYfL
SMGGKroVhCnz2asPKY2l4m6OsdVJZZL87YOekReQEI6bss74bO3adKVIfoJfRxMt
ldImxf0EGoLYPJa2XQhu5DjmEdCmXC7luG2aiX+pZmDShcs42q1z1k6JHSoGKQMa
LM6x5ahJAUVhG51RZxIB4bgxSplyAnij6d57zZV4NV6Am3EfIrc/ltDk3hOje0/m
FcYbpfre9ud9lXTtCAltt7Al6OWDOPu0bOQJg2RaYW0pEfYfpD70HLWQ1voVIII7
kBgnSLS4VqQFSvZj7+L7WDIaw0bTOHOBbZSs1Huc4arLAgMBAAGjTjBMMA4GA1Ud
DwEB/wQEAwIFoDATBgNVHSUEDDAKBggrBgEFBQcDATAMBgNVHRMBAf8EAjAAMBcG
A1UdEQQQMA6CDGV4YW1wbGUudGVzdDANBgkqhkiG9w0BAQsFAAOCAQEAc60kSDVU
5a+twewvJ+7JJ7AlO2Tj42rjnpobL9NBKdo4ejygmi3wFNHtdAF4qpor8/ZdMuth
fgEeUouWXioPCf53/NfG1z1ieKra8LItKEickUk1cq4tN6qhlFcXel9Fh7Ur5P/q
nFSZzbnS+t8j7fsi3h3qWUHxmgrpUhEz/oVSkJxkpmBS3mUAKtvI30ClkKjXrlf+
GRIz7gsoUsChUWAjGfo9tNI0WTzthIy5GwqxEboJfJD6amXt/hW7oyGlKZVggcT/
vke9FwOzAEeAOjWFUBH+jMI000D5bhQlD5RVayWX9X+EUaSf0CTp5L7hK248xizS
5n1C9vCNrTdQoQ==
-----END CERTIFICATE-----";

fn profile(config_type: ConfigType, address: &str, port: i32) -> CodegenProfile {
    CodegenProfile {
        index_id: "node-1".into(),
        config_type,
        remarks: "node".into(),
        address: address.into(),
        port,
        ..Default::default()
    }
}

fn settings() -> CodegenSettings {
    CodegenSettings {
        core_basic: CoreBasic {
            loglevel: "warning".into(),
            ..Default::default()
        },
        inbound: InboundSettings {
            local_port: 11808,
            ..Default::default()
        },
        state_port: 11809,
        state_port2: 11810,
        log_directory: "logs".into(),
        bin_directory: "bin".into(),
        log_date: "2026-10-01".into(),
        speed_ping_test_url: Some("https://example.com/ping".into()),
        ..Default::default()
    }
}

fn input(profile: CodegenProfile) -> CodegenInput {
    let mut profiles = BTreeMap::new();
    profiles.insert(profile.index_id.clone(), profile.clone());
    CodegenInput {
        profile,
        profiles,
        settings: settings(),
        ..Default::default()
    }
}

struct Case {
    id: &'static str,
    input: CodegenInput,
}

fn vless() -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, "192.0.2.10", 443);
    p.password = "11111111-2222-3333-4444-555555555555".into();
    p.proto_extra.vless_encryption = Some("none".into());
    p
}

fn xray_cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // 1. VLESS + raw + reality (D1).
    let mut c = vless();
    c.network = "raw".into();
    c.stream_security = "reality".into();
    c.public_key = REALITY_PUBLIC_KEY.into();
    c.short_id = "0a1b2c".into();
    c.sni = "example.test".into();
    c.fingerprint = "chrome".into();
    cases.push(Case {
        id: "xray-vless-raw-reality",
        input: input(c),
    });

    // 2. VMess + ws + tls + cert pin + mux (D2).
    let mut c = profile(ConfigType::Vmess, "192.0.2.11", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    c.proto_extra.alter_id = Some("4".into());
    c.proto_extra.vmess_security = Some("chacha20-poly1305".into());
    c.network = "ws".into();
    c.transport_extra.host = Some("ws.test".into());
    c.transport_extra.path = Some("/p".into());
    c.stream_security = "tls".into();
    c.cert_sha = CERT_SHA.into();
    c.alpn = "h2".into();
    c.sni = "ws.test".into();
    c.mux_enabled = Some(true);
    cases.push(Case {
        id: "xray-vmess-ws-tls",
        input: input(c),
    });

    // 3. VLESS + raw + none.
    let mut c = vless();
    c.network = "raw".into();
    cases.push(Case {
        id: "xray-vless-raw-none",
        input: input(c),
    });

    // 4. Shadowsocks + uot.
    let mut c = profile(ConfigType::Shadowsocks, "192.0.2.12", 8388);
    c.password = "synthetic-pass".into();
    c.proto_extra.ss_method = Some("aes-256-gcm".into());
    c.proto_extra.uot = Some(true);
    cases.push(Case {
        id: "xray-ss-uot",
        input: input(c),
    });

    // 5. Trojan + grpc + tls.
    let mut c = profile(ConfigType::Trojan, "192.0.2.13", 443);
    c.password = "trojan-pass".into();
    c.network = "grpc".into();
    c.stream_security = "tls".into();
    c.sni = "grpc.test".into();
    c.transport_extra.grpc_service_name = Some("svc".into());
    cases.push(Case {
        id: "xray-trojan-grpc-tls",
        input: input(c),
    });

    // 6. HTTP outbound + headers.
    let mut c = profile(ConfigType::Http, "192.0.2.14", 11821);
    c.username = "user2".into();
    c.password = "pass2".into();
    c.proto_extra.http_headers = Some(r#"{"X-Test": "1"}"#.into());
    cases.push(Case {
        id: "xray-http-headers",
        input: input(c),
    });

    // 7. SOCKS outbound + user/pass.
    let mut c = profile(ConfigType::Socks, "192.0.2.15", 11822);
    c.username = "user1".into();
    c.password = "pass1".into();
    cases.push(Case {
        id: "xray-socks-userpass",
        input: input(c),
    });

    // 8. Hysteria2 (D3).
    let mut c = profile(ConfigType::Hysteria2, "192.0.2.16", 443);
    c.password = "hy2-pass".into();
    c.proto_extra.ports = Some("20000-30000,40000".into());
    c.proto_extra.up_mbps = Some(50);
    c.proto_extra.down_mbps = Some(0);
    c.proto_extra.salamander_pass = Some("pw".into());
    cases.push(Case {
        id: "xray-hysteria2",
        input: input(c),
    });

    // 9. WireGuard (D4).
    let mut c = profile(ConfigType::WireGuard, "192.0.2.17", 443);
    c.password = WG_PRIVATE_KEY.into();
    c.proto_extra.wg_public_key = Some(WG_PUBLIC_KEY.into());
    c.proto_extra.wg_preshared_key = Some(WG_PRESHARED_KEY.into());
    c.proto_extra.wg_interface_address = Some("10.0.0.2/32,fd00::2/128".into());
    c.proto_extra.wg_mtu = Some(1420);
    c.proto_extra.wg_dns = Some("1.1.1.1,8.8.8.8".into());
    c.proto_extra.wg_reserved = Some("1, 2, 3".into());
    cases.push(Case {
        id: "xray-wireguard",
        input: input(c),
    });

    // 10. raw + http header + tls.
    let mut c = vless();
    c.network = "raw".into();
    c.stream_security = "tls".into();
    c.transport_extra.raw_header_type = Some("http".into());
    c.transport_extra.host = Some("raw.example.test".into());
    c.transport_extra.path = Some("/p".into());
    c.sni = "raw.example.test".into();
    let mut i = input(c);
    i.settings.core_basic.def_user_agent = Some("chrome".into());
    cases.push(Case {
        id: "xray-raw-http-header-tls",
        input: i,
    });

    // 11. httpupgrade + tls.
    let mut c = vless();
    c.network = "httpupgrade".into();
    c.stream_security = "tls".into();
    c.transport_extra.host = Some("hu.example.test".into());
    c.transport_extra.path = Some("/hu".into());
    c.sni = "hu.example.test".into();
    cases.push(Case {
        id: "xray-httpupgrade-tls",
        input: input(c),
    });

    // 12. xhttp + tls.
    let mut c = vless();
    c.network = "xhttp".into();
    c.stream_security = "tls".into();
    c.transport_extra.path = Some("/xh".into());
    c.transport_extra.host = Some("xh.example.test".into());
    c.transport_extra.xhttp_mode = Some("stream-up".into());
    c.transport_extra.xhttp_extra =
        Some(r#"{"downloadSettings":{"sockopt":{"interface":"eth0"}}}"#.into());
    c.sni = "xh.example.test".into();
    c.mux_enabled = Some(true);
    cases.push(Case {
        id: "xray-xhttp-tls",
        input: input(c),
    });

    // 13. kcp (D5).
    let mut c = vless();
    c.network = "kcp".into();
    c.transport_extra.kcp_header_type = Some("wechat-video".into());
    c.transport_extra.kcp_seed = Some("seed".into());
    c.transport_extra.kcp_mtu = Some(1350);
    cases.push(Case {
        id: "xray-kcp",
        input: input(c),
    });

    // 14. grpc + reality + settings.
    let mut c = vless();
    c.network = "grpc".into();
    c.stream_security = "reality".into();
    c.public_key = REALITY_PUBLIC_KEY_2.into();
    c.short_id = "abcd".into();
    c.sni = "grpc.example.test".into();
    c.transport_extra.grpc_service_name = Some("svc".into());
    c.transport_extra.grpc_mode = Some("multi".into());
    c.transport_extra.grpc_authority = Some("authority.example.test".into());
    let mut i = input(c);
    i.settings.grpc.idle_timeout = Some(10);
    i.settings.grpc.health_check_timeout = Some(5);
    i.settings.grpc.permit_without_stream = Some(true);
    i.settings.grpc.initial_windows_size = Some(65535);
    cases.push(Case {
        id: "xray-grpc-reality",
        input: i,
    });

    // 15. PolicyGroup with 2 children.
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    group.proto_extra.multiple_load = Some(MultipleLoad::LeastPing);
    let mut c1 = vless();
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    let mut c2 = vless();
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.address = "192.0.2.20".into();
    let mut i = input(group);
    i.profiles.insert(c1.index_id.clone(), c1);
    i.profiles.insert(c2.index_id.clone(), c2);
    cases.push(Case {
        id: "xray-policy-group",
        input: i,
    });

    // 16. ProxyChain with 2 children.
    let mut chain = profile(ConfigType::ProxyChain, "", 0);
    chain.index_id = "chain-1".into();
    chain.proto_extra.child_items = Some("c1,c2".into());
    let mut c1 = vless();
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    let mut c2 = vless();
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.address = "192.0.2.21".into();
    let mut i = input(chain);
    i.profiles.insert(c1.index_id.clone(), c1);
    i.profiles.insert(c2.index_id.clone(), c2);
    cases.push(Case {
        id: "xray-proxy-chain",
        input: i,
    });

    // 17. Global DNS + fakeip + routing + statistics.
    let routing = CodegenRouting {
        rule_set: vec![
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:direct.example.test".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "proxy".into(),
                domain: Some(vec!["full:proxy.example.test".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Dns,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:dns.example.test".into()]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let dns = CodegenDns {
        simple: SimpleDns {
            direct_dns: Some("223.5.5.5".into()),
            remote_dns: Some("1.1.1.1".into()),
            fake_ip: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut i = input(vless());
    i.routing = Some(routing);
    i.dns = Some(dns);
    i.settings.gui.enable_statistics = true;
    i.settings.gui.display_real_time_speed = true;
    i.settings.core_basic.log_enabled = true;
    i.settings.core_basic.loglevel = "info".into();
    cases.push(Case {
        id: "xray-global-dns-stat",
        input: i,
    });

    // 18. Global TUN + bind interface + fragment.
    let mut i = input(vless());
    i.settings.tun.enabled = true;
    i.settings.tun.mtu = 1400;
    i.settings.tun.ipv4_address = Some("172.18.0.1/30".into());
    i.settings.tun.enable_ipv6_address = true;
    i.settings.tun.ipv6_address = Some("fc00::172:18:0:1/126".into());
    i.settings.tun.route_exclude_address = vec!["10.0.0.0/8".into()];
    i.settings.core_basic.bind_interface = Some("wg0".into());
    i.settings.core_basic.send_through = Some("192.0.2.200".into());
    i.settings.protect_core_executables = vec!["xray.exe".into()];
    i.settings.has_global_ipv6_address = true;
    i.settings.core_basic.enable_fragment = true;
    i.settings.core_basic.enable_final_fragment = true;
    cases.push(Case {
        id: "xray-global-tun-fragment",
        input: i,
    });

    cases
}

fn singbox_cases() -> Vec<Case> {
    let mut cases = Vec::new();

    // S1. VLESS + tls.
    let mut c = profile(ConfigType::Vless, "192.0.2.20", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    c.stream_security = "tls".into();
    c.sni = "a.test".into();
    c.fingerprint = "chrome".into();
    cases.push(Case {
        id: "singbox-vless-tls",
        input: input(c),
    });

    // S2. VMess + ws + early data.
    let mut c = profile(ConfigType::Vmess, "192.0.2.21", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    c.proto_extra.alter_id = Some("2".into());
    c.network = "ws".into();
    c.transport_extra.host = Some("ws.test".into());
    c.transport_extra.path = Some("/p?ed=2048".into());
    cases.push(Case {
        id: "singbox-vmess-ws-ed",
        input: input(c),
    });

    // Shadowsocks + obfs plugin.
    let mut c = profile(ConfigType::Shadowsocks, "192.0.2.22", 8388);
    c.password = "synthetic-pass".into();
    c.proto_extra.ss_method = Some("aes-256-gcm".into());
    c.proto_extra.uot = Some(true);
    c.network = "raw".into();
    c.transport_extra.raw_header_type = Some("http".into());
    c.transport_extra.host = Some("obfs.test".into());
    cases.push(Case {
        id: "singbox-ss-obfs",
        input: input(c),
    });

    // Shadowsocks + v2ray-plugin (ws + tls).
    let mut c = profile(ConfigType::Shadowsocks, "192.0.2.23", 8389);
    c.password = "synthetic-pass".into();
    c.proto_extra.ss_method = Some("aes-256-gcm".into());
    c.network = "ws".into();
    c.transport_extra.host = Some("plugin.test".into());
    c.transport_extra.path = Some("/wp".into());
    c.stream_security = "tls".into();
    cases.push(Case {
        id: "singbox-ss-v2ray-plugin",
        input: input(c),
    });

    // Trojan + grpc.
    let mut c = profile(ConfigType::Trojan, "192.0.2.24", 443);
    c.password = "trojan-pass".into();
    c.network = "grpc".into();
    c.stream_security = "tls".into();
    c.sni = "grpc.test".into();
    c.transport_extra.grpc_service_name = Some("svc".into());
    let mut i = input(c);
    i.settings.grpc.idle_timeout = Some(10);
    i.settings.grpc.health_check_timeout = Some(5);
    i.settings.grpc.permit_without_stream = Some(true);
    cases.push(Case {
        id: "singbox-trojan-grpc",
        input: i,
    });

    // HTTP outbound + auth.
    let mut c = profile(ConfigType::Http, "192.0.2.25", 11831);
    c.username = "user".into();
    c.password = "pass".into();
    cases.push(Case {
        id: "singbox-http-auth",
        input: input(c),
    });

    // SOCKS outbound + auth.
    let mut c = profile(ConfigType::Socks, "192.0.2.26", 11830);
    c.username = "user".into();
    c.password = "pass".into();
    cases.push(Case {
        id: "singbox-socks-auth",
        input: input(c),
    });

    // S4. Hysteria2 (TLS is mandatory in sing-box).
    let mut c = profile(ConfigType::Hysteria2, "192.0.2.27", 443);
    c.password = "hy2-pass".into();
    c.proto_extra.ports = Some("20000-30000,40000".into());
    c.proto_extra.up_mbps = Some(50);
    c.proto_extra.salamander_pass = Some("pw".into());
    c.proto_extra.hop_interval = Some("30".into());
    c.stream_security = "tls".into();
    c.sni = "hy2.test".into();
    c.cert = SELF_SIGNED_CERT.into();
    cases.push(Case {
        id: "singbox-hysteria2",
        input: input(c),
    });

    // S3. TUIC (TLS is mandatory in sing-box).
    let mut c = profile(ConfigType::Tuic, "192.0.2.28", 443);
    c.username = "11111111-2222-3333-4444-555555555555".into();
    c.password = "tuic-pass".into();
    c.proto_extra.congestion_control = Some("bbr".into());
    c.stream_security = "tls".into();
    c.sni = "tuic.test".into();
    c.cert = SELF_SIGNED_CERT.into();
    cases.push(Case {
        id: "singbox-tuic",
        input: input(c),
    });

    // Anytls (TLS is mandatory in sing-box).
    let mut c = profile(ConfigType::Anytls, "192.0.2.29", 443);
    c.password = "anytls-pass".into();
    c.stream_security = "tls".into();
    c.sni = "anytls.test".into();
    c.cert = SELF_SIGNED_CERT.into();
    cases.push(Case {
        id: "singbox-anytls",
        input: input(c),
    });

    // Naive (TLS/quic is mandatory in sing-box).
    let mut c = profile(ConfigType::Naive, "192.0.2.30", 443);
    c.username = "user".into();
    c.password = "pass".into();
    c.proto_extra.naive_quic = Some(true);
    c.proto_extra.congestion_control = Some("bbr".into());
    c.proto_extra.insecure_concurrency = Some(4);
    c.stream_security = "tls".into();
    c.sni = "naive.test".into();
    c.cert = SELF_SIGNED_CERT.into();
    cases.push(Case {
        id: "singbox-naive",
        input: input(c),
    });

    // S5. WireGuard endpoint.
    let mut c = profile(ConfigType::WireGuard, "192.0.2.31", 51820);
    c.password = WG_PRIVATE_KEY.into();
    c.proto_extra.wg_public_key = Some(WG_PUBLIC_KEY.into());
    c.proto_extra.wg_interface_address = Some("10.0.0.2/32".into());
    c.proto_extra.wg_mtu = Some(1420);
    c.proto_extra.wg_reserved = Some("1, 2, 3".into());
    cases.push(Case {
        id: "singbox-wireguard",
        input: input(c),
    });

    // httpupgrade.
    let mut c = profile(ConfigType::Vless, "192.0.2.32", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    c.network = "httpupgrade".into();
    c.transport_extra.host = Some("hu.test".into());
    c.transport_extra.path = Some("/hu".into());
    c.stream_security = "tls".into();
    c.sni = "hu.test".into();
    cases.push(Case {
        id: "singbox-httpupgrade",
        input: input(c),
    });

    // reality + fragment + ech.
    let mut c = profile(ConfigType::Vless, "192.0.2.33", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    c.network = "raw".into();
    c.stream_security = "reality".into();
    c.public_key = REALITY_PUBLIC_KEY.into();
    c.short_id = "abcd".into();
    c.sni = "reality.test".into();
    c.fingerprint = "firefox".into();
    c.alpn = "h2,http/1.1".into();
    let mut i = input(c);
    i.settings.core_basic.enable_fragment = true;
    cases.push(Case {
        id: "singbox-reality-fragment",
        input: i,
    });

    // PolicyGroup with 2 children.
    let mut group = profile(ConfigType::PolicyGroup, "", 0);
    group.index_id = "group-1".into();
    group.proto_extra.child_items = Some("c1,c2".into());
    group.proto_extra.multiple_load = Some(MultipleLoad::LeastPing);
    let mut c1 = profile(ConfigType::Vless, "192.0.2.34", 443);
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    c1.password = "11111111-2222-3333-4444-555555555555".into();
    let mut c2 = profile(ConfigType::Vless, "192.0.2.35", 443);
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.password = "11111111-2222-3333-4444-555555555555".into();
    let mut i = input(group);
    i.profiles.insert(c1.index_id.clone(), c1);
    i.profiles.insert(c2.index_id.clone(), c2);
    cases.push(Case {
        id: "singbox-policy-group",
        input: i,
    });

    // ProxyChain with 2 children.
    let mut chain = profile(ConfigType::ProxyChain, "", 0);
    chain.index_id = "chain-1".into();
    chain.proto_extra.child_items = Some("c1,c2".into());
    let mut c1 = profile(ConfigType::Vless, "192.0.2.36", 443);
    c1.index_id = "c1".into();
    c1.remarks = "c1".into();
    c1.password = "11111111-2222-3333-4444-555555555555".into();
    let mut c2 = profile(ConfigType::Vless, "192.0.2.37", 443);
    c2.index_id = "c2".into();
    c2.remarks = "c2".into();
    c2.password = "11111111-2222-3333-4444-555555555555".into();
    let mut i = input(chain);
    i.profiles.insert(c1.index_id.clone(), c1);
    i.profiles.insert(c2.index_id.clone(), c2);
    cases.push(Case {
        id: "singbox-proxy-chain",
        input: i,
    });

    // Global DNS + cache_file + ruleset.
    let mut c = profile(ConfigType::Vless, "192.0.2.38", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    let routing = CodegenRouting {
        rule_set: vec![
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Routing,
                outbound_tag: "proxy".into(),
                domain: Some(vec!["geosite:google".into()]),
                ..Default::default()
            },
            CodegenRule {
                enabled: true,
                rule_type: RuleType::Dns,
                outbound_tag: "direct".into(),
                domain: Some(vec!["full:dns.test".into()]),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let dns = CodegenDns {
        simple: SimpleDns {
            direct_dns: Some("223.5.5.5".into()),
            remote_dns: Some("1.1.1.1".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut i = input(c);
    i.routing = Some(routing);
    i.dns = Some(dns);
    i.settings.core_basic.enable_cache_file4_sbox = true;
    i.settings.gui.enable_statistics = true;
    i.settings.core_basic.log_enabled = true;
    cases.push(Case {
        id: "singbox-global-dns-ruleset",
        input: i,
    });

    // Global TUN + final fragment.
    let mut c = profile(ConfigType::Vless, "192.0.2.39", 443);
    c.password = "11111111-2222-3333-4444-555555555555".into();
    let mut i = input(c);
    i.settings.tun.enabled = true;
    i.settings.tun.mtu = 1420;
    i.settings.tun.ipv4_address = Some("172.18.0.1/30".into());
    i.settings.tun.enable_ipv6_address = true;
    i.settings.tun.ipv6_address = Some("fc00::172:18:0:1/126".into());
    i.settings.tun.route_exclude_address = vec!["10.0.0.0/8".into()];
    i.settings.protect_core_executables = vec!["sing-box.exe".into()];
    i.settings.has_global_ipv6_address = true;
    i.settings.core_basic.enable_final_fragment = true;
    cases.push(Case {
        id: "singbox-global-tun-fragment",
        input: i,
    });

    cases
}

fn main() {
    let out_dir: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/t06b/matrix".into())
        .into();
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let mut manifest: Vec<Value> = Vec::new();

    for (core, cases, generator) in [
        (
            "xray",
            xray_cases(),
            generate_xray
                as fn(
                    &CodegenInput,
                )
                    -> Result<config_codegen::GeneratedConfigs, config_codegen::CodegenError>,
        ),
        ("singbox", singbox_cases(), generate_singbox),
    ] {
        for case in cases {
            let file_name = format!("{core}--{}.json", case.id);
            let path = out_dir.join(&file_name);
            match generator(&case.input) {
                Ok(generated) => {
                    let text = serde_json::to_string_pretty(&generated.main).expect("serialize");
                    std::fs::write(&path, text).expect("write config");
                    manifest.push(json!({
                        "core": core,
                        "case": case.id,
                        "file": path.to_string_lossy(),
                        "generated": true,
                        "error": Value::Null,
                    }));
                }
                Err(error) => {
                    manifest.push(json!({
                        "core": core,
                        "case": case.id,
                        "file": Value::Null,
                        "generated": false,
                        "error": error.to_string(),
                    }));
                }
            }
        }
    }

    let manifest_path = out_dir.join("manifest.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("manifest"),
    )
    .expect("write manifest");
    println!("wrote {} cases to {}", manifest.len(), out_dir.display());
}
