//! Per-format share URI parse/emit and boundary tests. These exercise the
//! public `subscriptions` surface rather than the private module internals.

use domain::{ConfigType, Profile};
use subscriptions::fmt::{self, FmtKind};
use subscriptions::{resolve_uri, to_uri};

fn vless() -> Profile {
    Profile {
        config_type: ConfigType::Vless,
        remarks: "vless demo".into(),
        address: "vless.example".into(),
        port: 8443,
        password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
        network: "raw".into(),
        ..Profile::default()
    }
}

#[test]
fn vmess_json_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Vmess,
        remarks: "vmess demo".into(),
        address: "example.com".into(),
        port: 443,
        password: "b831381d-6324-4d53-ad4f-8cda48b30811".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.proto_extra.alter_id = Some("0".into());
    item.proto_extra.vmess_security = Some("auto".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.config_type, ConfigType::Vmess);
    assert_eq!(parsed.address, "example.com");
    assert_eq!(parsed.password, item.password);
    assert_eq!(parsed.remarks, "vmess demo");
}

#[test]
fn vmess_standard_uri_is_parsed() {
    let parsed =
        resolve_uri("vmess://id@example.com:443/?type=ws&host=a.example&path=/x#n").unwrap();
    assert_eq!(parsed.address, "example.com");
    assert_eq!(parsed.network, "ws");
    assert_eq!(parsed.transport_extra.host.as_deref(), Some("a.example"));
    assert_eq!(parsed.transport_extra.path.as_deref(), Some("/x"));
}

#[test]
fn vmess_aid_written_as_string_still_parses() {
    let json =
        r#"{"v":"2","ps":"p","add":"a.example","port":"443","id":"id","aid":"4","net":"tcp"}"#;
    let uri = format!("vmess://{}", subscriptions::util::base64_encode(json));
    let parsed = resolve_uri(&uri).unwrap();
    assert_eq!(parsed.port, 443);
    assert_eq!(parsed.proto_extra.alter_id.as_deref(), Some("4"));
}

#[test]
fn vless_roundtrip_reality() {
    let mut item = vless();
    item.security.stream_security = Some("reality".into());
    item.security.sni = Some("s.example".into());
    item.security.fingerprint = Some("chrome".into());
    item.security.public_key = Some("KEY".into());
    item.security.short_id = Some("ab12".into());
    item.proto_extra.flow = Some("xtls-rprx-vision".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.security.stream_security.as_deref(), Some("reality"));
    assert_eq!(parsed.security.public_key.as_deref(), Some("KEY"));
    assert_eq!(parsed.proto_extra.flow.as_deref(), Some("xtls-rprx-vision"));
}

#[test]
fn vless_xhttp_transport_roundtrip() {
    let mut item = vless();
    item.network = "xhttp".into();
    item.transport_extra.host = Some("h.example".into());
    item.transport_extra.path = Some("/path".into());
    item.transport_extra.xhttp_mode = Some("stream-up".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.network, "xhttp");
    assert_eq!(
        parsed.transport_extra.xhttp_mode.as_deref(),
        Some("stream-up")
    );
    assert_eq!(parsed.transport_extra.path.as_deref(), Some("/path"));
}

#[test]
fn vless_kcp_and_grpc_transport_roundtrip() {
    let mut kcp = vless();
    kcp.network = "kcp".into();
    kcp.transport_extra.kcp_header_type = Some("srtp".into());
    kcp.transport_extra.kcp_seed = Some("seed".into());
    kcp.transport_extra.kcp_mtu = Some(1350);
    let parsed = resolve_uri(&to_uri(&kcp).unwrap()).unwrap();
    assert_eq!(
        parsed.transport_extra.kcp_header_type.as_deref(),
        Some("srtp")
    );
    assert_eq!(parsed.transport_extra.kcp_seed.as_deref(), Some("seed"));
    assert_eq!(parsed.transport_extra.kcp_mtu, Some(1350));

    let mut grpc = vless();
    grpc.network = "grpc".into();
    grpc.transport_extra.grpc_service_name = Some("svc".into());
    grpc.transport_extra.grpc_authority = Some("auth".into());
    grpc.transport_extra.grpc_mode = Some("gun".into());
    let parsed = resolve_uri(&to_uri(&grpc).unwrap()).unwrap();
    assert_eq!(parsed.network, "grpc");
    assert_eq!(
        parsed.transport_extra.grpc_service_name.as_deref(),
        Some("svc")
    );
    assert_eq!(parsed.transport_extra.grpc_mode.as_deref(), Some("gun"));
}

#[test]
fn shadowsocks_sip002_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Shadowsocks,
        remarks: "ss demo".into(),
        address: "1.2.3.4".into(),
        port: 8388,
        password: "pass123".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.proto_extra.ss_method = Some("aes-128-gcm".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.address, "1.2.3.4");
    assert_eq!(parsed.proto_extra.ss_method.as_deref(), Some("aes-128-gcm"));
}

#[test]
fn shadowsocks_legacy_and_obfs_plugin() {
    let payload = subscriptions::util::base64_encode("aes-256-gcm:secret@example.com:443");
    let parsed = resolve_uri(&format!("ss://{payload}#node")).unwrap();
    assert_eq!(parsed.address, "example.com");
    assert_eq!(parsed.password, "secret");

    let uri = "ss://YWVzLTEyOC1nY206cGFzczEyMw==@1.2.3.4:8388/?plugin=obfs-local;obfs=http;obfs-host=example.com#ss";
    let parsed = resolve_uri(uri).unwrap();
    assert_eq!(parsed.transport_extra.host.as_deref(), Some("example.com"));
    assert_eq!(
        parsed.transport_extra.raw_header_type.as_deref(),
        Some("http")
    );
}

#[test]
fn socks_roundtrip_and_legacy() {
    let item = Profile {
        config_type: ConfigType::Socks,
        remarks: "socks".into(),
        address: "127.0.0.1".into(),
        port: 1080,
        username: "user".into(),
        password: "pass".into(),
        ..Profile::default()
    };
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.username, "user");
    assert_eq!(parsed.password, "pass");

    let payload = subscriptions::util::base64_encode("u:p@example.com:1080");
    let parsed = resolve_uri(&format!("socks://{payload}#tag")).unwrap();
    assert_eq!(parsed.address, "example.com");
    assert_eq!(parsed.port, 1080);
}

#[test]
fn trojan_roundtrip_writes_both_insecure_spellings() {
    let mut item = Profile {
        config_type: ConfigType::Trojan,
        remarks: "trojan".into(),
        address: "t.example".into(),
        port: 443,
        password: "p:a@ss#% +/=".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.security.stream_security = Some("tls".into());
    item.security.sni = Some("sni.t.example".into());
    item.security.allow_insecure = Some("true".into());
    item.proto_extra.flow = Some("xtls-rprx-vision".into());
    let uri = to_uri(&item).unwrap();
    assert!(uri.contains("allowInsecure=1"));
    assert!(uri.contains("insecure=1"));
    let parsed = resolve_uri(&uri).unwrap();
    assert_eq!(parsed.password, "p:a@ss#% +/=");
    assert_eq!(parsed.security.allow_insecure.as_deref(), Some("true"));
}

#[test]
fn tuic_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Tuic,
        remarks: "tuic".into(),
        address: "tuic.example".into(),
        port: 8443,
        username: "01234567-89ab-cdef-0123-456789abcdef".into(),
        password: "tuic-pass".into(),
        ..Profile::default()
    };
    item.security.alpn = Some("h3".into());
    item.proto_extra.congestion_control = Some("bbr".into());
    item.security.allow_insecure = Some("true".into());
    let uri = to_uri(&item).unwrap();
    assert!(uri.contains("allow_insecure=1"));
    let parsed = resolve_uri(&uri).unwrap();
    assert_eq!(parsed.username, item.username);
    assert_eq!(
        parsed.proto_extra.congestion_control.as_deref(),
        Some("bbr")
    );
}

#[test]
fn hysteria2_roundtrip_obfs_and_port_range() {
    let mut item = Profile {
        config_type: ConfigType::Hysteria2,
        remarks: "hy2".into(),
        address: "hy2.example".into(),
        port: 8443,
        password: "pw".into(),
        ..Profile::default()
    };
    item.security.sni = Some("sni.hy2.example".into());
    item.security.ech_config_list = Some("AAj+DQAEAAAAAA==".into());
    item.proto_extra.salamander_pass = Some("obfs-pass".into());
    item.proto_extra.ports = Some("5000:6000".into());
    item.security.allow_insecure = Some("true".into());
    let uri = to_uri(&item).unwrap();
    assert!(uri.contains("obfs=salamander"));
    assert!(uri.contains("mport=5000-6000"));
    let parsed = resolve_uri(&uri).unwrap();
    assert_eq!(
        parsed.proto_extra.salamander_pass.as_deref(),
        Some("obfs-pass")
    );
    assert_eq!(parsed.proto_extra.ports.as_deref(), Some("5000-6000"));
}

#[test]
fn hysteria2_port_defaults_to_443() {
    let parsed = resolve_uri("hy2://password@hy2.example/?sni=real.example").unwrap();
    assert_eq!(parsed.port, 443);
    let zero = resolve_uri("hysteria2://password@hy2.example:0/").unwrap();
    assert_eq!(zero.port, 0);
}

#[test]
fn hysteria2_realm_roundtrip() {
    let input = "hysteria2+realm://mytoken@rendezvous.example.com/my-cabin?auth=your_password&insecure=1&pinSHA256=deadbeef#remark";
    let parsed = resolve_uri(input).unwrap();
    assert_eq!(parsed.password, "your_password");
    assert_eq!(parsed.address, "rendezvous.example.com");
    let uri = to_uri(&parsed).unwrap();
    assert!(uri.contains("hysteria2+realm://mytoken@rendezvous.example.com"));
    assert!(uri.ends_with("#remark"));
}

#[test]
fn hysteria2_percent_value_survives_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Hysteria2,
        remarks: "percent".into(),
        address: "hy2.example".into(),
        port: 8443,
        password: "pw".into(),
        ..Profile::default()
    };
    item.proto_extra.salamander_pass = Some("ob%41fs".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(
        parsed.proto_extra.salamander_pass.as_deref(),
        Some("ob%41fs")
    );
}

#[test]
fn wireguard_roundtrip_brackets_ipv6() {
    use base64::Engine as _;
    let key = |b: u8| base64::engine::general_purpose::STANDARD.encode(vec![b; 32]);
    let mut item = Profile {
        config_type: ConfigType::WireGuard,
        remarks: "wg 東京".into(),
        address: "2001:db8::40".into(),
        port: 51820,
        password: key(0xFE),
        ..Profile::default()
    };
    item.proto_extra.wg_public_key = Some(key(0xFD));
    item.proto_extra.wg_interface_address = Some("10.0.0.2/32,fd00::2/128".into());
    item.proto_extra.wg_mtu = Some(1420);
    let uri = to_uri(&item).unwrap();
    assert!(uri.contains("@[2001:db8::40]:51820"));
    let parsed = resolve_uri(&uri).unwrap();
    assert_eq!(parsed.password, item.password);
    assert_eq!(parsed.proto_extra.wg_mtu, Some(1420));
    assert_eq!(parsed.remarks, "wg 東京");
}

#[test]
fn wireguard_conf_parses_two_peers() {
    let config = "[Interface]\nPrivateKey = k\nAddress = 10.0.0.2/32\n[Peer]\nPublicKey = p1\nEndpoint = [2001:db8::1]:51820\n[Peer]\nPublicKey = p2\nEndpoint = example.com:12345\n";
    let list = fmt::wireguard::resolve_config(config).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].address, "2001:db8::1");
    assert_eq!(list[1].port, 12345);
}

#[test]
fn anytls_roundtrip() {
    let mut item = Profile {
        config_type: ConfigType::Anytls,
        remarks: "anytls".into(),
        address: "anytls.example".into(),
        port: 8443,
        password: "anytls-pass".into(),
        network: "raw".into(),
        ..Profile::default()
    };
    item.security.stream_security = Some("tls".into());
    item.security.alpn = Some("h2,http/1.1".into());
    item.security.allow_insecure = Some("true".into());
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.password, "anytls-pass");
    assert_eq!(parsed.security.alpn.as_deref(), Some("h2,http/1.1"));
}

#[test]
fn naive_https_and_quic_roundtrip() {
    for quic in [false, true] {
        let mut item = Profile {
            config_type: ConfigType::Naive,
            remarks: "naive".into(),
            address: "naive.example".into(),
            port: 443,
            username: "naive-user".into(),
            password: "päss:word@/?#&=+ 東京".into(),
            network: "raw".into(),
            ..Profile::default()
        };
        item.proto_extra.naive_quic = Some(quic);
        item.proto_extra.insecure_concurrency = Some(4);
        let uri = to_uri(&item).unwrap();
        assert!(uri.starts_with(if quic {
            "naive+quic://"
        } else {
            "naive+https://"
        }));
        let parsed = resolve_uri(&uri).unwrap();
        assert_eq!(parsed.username, "naive-user");
        assert_eq!(parsed.password, item.password);
        assert_eq!(parsed.proto_extra.insecure_concurrency, Some(4));
    }
}

#[test]
fn unicode_and_emoji_remarks_survive() {
    let mut item = vless();
    item.remarks = "节点 🚀 東京 — test".into();
    let parsed = resolve_uri(&to_uri(&item).unwrap()).unwrap();
    assert_eq!(parsed.remarks, "节点 🚀 東京 — test");
}

#[test]
fn group_profile_has_no_share_uri() {
    let group = Profile {
        config_type: ConfigType::PolicyGroup,
        remarks: "group".into(),
        ..Profile::default()
    };
    assert!(to_uri(&group).is_err());
}

#[test]
fn unsupported_and_empty_inputs_error() {
    assert!(resolve_uri("not-a-uri").is_err());
    assert!(resolve_uri("   ").is_err());
    assert!(resolve_uri("http://example.com").is_err());
}

#[test]
fn fmt_kind_matrix() {
    assert_eq!(fmt::fmt_kind_of("vmess://x"), Some(FmtKind::Vmess));
    assert_eq!(fmt::fmt_kind_of("ss://x"), Some(FmtKind::Shadowsocks));
    assert_eq!(fmt::fmt_kind_of("socks://x"), Some(FmtKind::Socks));
    assert_eq!(fmt::fmt_kind_of("trojan://x"), Some(FmtKind::Trojan));
    assert_eq!(fmt::fmt_kind_of("vless://x"), Some(FmtKind::Vless));
    assert_eq!(fmt::fmt_kind_of("hy2://x"), Some(FmtKind::Hysteria2));
    assert_eq!(fmt::fmt_kind_of("tuic://x"), Some(FmtKind::Tuic));
    assert_eq!(fmt::fmt_kind_of("wireguard://x"), Some(FmtKind::WireGuard));
    assert_eq!(fmt::fmt_kind_of("anytls://x"), Some(FmtKind::Anytls));
    assert_eq!(fmt::fmt_kind_of("naive+quic://x"), Some(FmtKind::Naive));
    assert_eq!(fmt::fmt_kind_of("v2rayn://vless/AAA"), Some(FmtKind::Inner));
    assert_eq!(fmt::fmt_kind_of("https://x"), None);
}
