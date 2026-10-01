//! Shared transport/security query codec, modelled on upstream `BaseFmt.cs`.

use domain::Profile;

use crate::util::{bracket_ipv6, normalize_json_compact, url_encode, Query};

pub const DEFAULT_SECURITY: &str = "auto";
pub const DEFAULT_NETWORK: &str = "raw";
pub const RAW_HEADER_HTTP: &str = "http";
pub const NONE: &str = "none";
/// Upstream exports the `raw` network under the `tcp` alias.
pub const RAW_NETWORK_ALIAS: &str = "tcp";
pub const DEFAULT_XHTTP_MODE: &str = "auto";
pub const STREAM_SECURITY: &str = "tls";
pub const STREAM_SECURITY_REALITY: &str = "reality";
pub const GRPC_GUN_MODE: &str = "gun";
pub const GRPC_MULTI_MODE: &str = "multi";
pub const STRING_TRUE: &str = "true";

/// Transports accepted by the shared query codec (`Global.Networks`).
pub const NETWORKS: [&str; 6] = ["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
/// Allowed xhttp mode tokens (`Global.XhttpMode`).
pub const XHTTP_MODES: [&str; 4] = ["auto", "packet-up", "stream-up", "stream-one"];

/// Per-protocol share scheme prefixes (`Global.ProtocolShares`).
pub const VMESS: &str = "vmess://";
pub const SS: &str = "ss://";
pub const SOCKS: &str = "socks://";
pub const VLESS: &str = "vless://";
pub const TROJAN: &str = "trojan://";
pub const HYSTERIA2: &str = "hysteria2://";
pub const HY2_ALIAS: &str = "hy2://";
pub const HY2_REALM: &str = "hysteria2+realm://";
pub const HY2_REALM_HTTP: &str = "hysteria2+realm+http://";
pub const TUIC: &str = "tuic://";
pub const WIREGUARD: &str = "wireguard://";
pub const ANYTLS: &str = "anytls://";
pub const NAIVE: &str = "naive://";
pub const NAIVE_HTTPS: &str = "naive+https://";
pub const NAIVE_QUIC: &str = "naive+quic://";
pub const SOCKS5: &str = "socks5://";
pub const SOCKS4: &str = "socks4://";
pub const INNER: &str = "v2rayn://";

/// Network token after upstream normalization (case-sensitive `Global.Networks`
/// membership, empty/unknown collapses to `raw`).
pub fn get_network(item: &Profile) -> String {
    let network = item.network.trim();
    if network.is_empty() || !NETWORKS.contains(&network) {
        DEFAULT_NETWORK.to_string()
    } else {
        network.to_string()
    }
}

pub fn stream_security(item: &Profile) -> &str {
    item.security.stream_security.as_deref().unwrap_or("")
}

pub fn set_optional(target: &mut Option<String>, value: &str) {
    *target = if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    };
}

pub fn allow_insecure(item: &Profile) -> bool {
    item.security.allow_insecure.as_deref() == Some(STRING_TRUE)
}

pub fn set_allow_insecure(item: &mut Profile, enabled: bool) {
    item.security.allow_insecure = if enabled {
        Some(STRING_TRUE.to_string())
    } else {
        None
    };
}

/// Build `...@host:port?query#remark`. `user_info` is percent-encoded here, so
/// callers pass the raw value.
pub fn build_uri(
    scheme: &str,
    address: &str,
    port: i32,
    user_info: &str,
    query: &[(String, String)],
    remark: &str,
) -> String {
    let query_string = if query.is_empty() {
        String::new()
    } else {
        let joined = query
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{joined}")
    };
    format!(
        "{scheme}{}@{}:{port}{query_string}{remark}",
        url_encode(user_info),
        bracket_ipv6(address)
    )
}

/// Shared query writer (`BaseFmt.ToUriQuery`). `security_def` is the fallback
/// `security` value used when the profile has none (usually `none`).
pub fn to_uri_query(item: &Profile, security_def: Option<&str>, query: &mut Vec<(String, String)>) {
    let transport = &item.transport_extra;
    let security = stream_security(item);
    if !security.is_empty() {
        query.push(("security".into(), security.to_string()));
    } else if let Some(def) = security_def {
        query.push(("security".into(), def.to_string()));
    }

    let security = &item.security;
    if let Some(v) = security.sni.as_deref().filter(|v| !v.is_empty()) {
        query.push(("sni".into(), url_encode(v)));
    }
    if let Some(v) = security.fingerprint.as_deref().filter(|v| !v.is_empty()) {
        query.push(("fp".into(), url_encode(v)));
    }
    if let Some(v) = security.public_key.as_deref().filter(|v| !v.is_empty()) {
        query.push(("pbk".into(), url_encode(v)));
    }
    if let Some(v) = security.short_id.as_deref().filter(|v| !v.is_empty()) {
        query.push(("sid".into(), url_encode(v)));
    }
    if let Some(v) = security.spider_x.as_deref().filter(|v| !v.is_empty()) {
        query.push(("spx".into(), url_encode(v)));
    }
    if let Some(v) = security.mldsa65_verify.as_deref().filter(|v| !v.is_empty()) {
        query.push(("pqv".into(), url_encode(v)));
    }
    if security.stream_security.as_deref() == Some(STREAM_SECURITY) {
        if let Some(v) = security.alpn.as_deref().filter(|v| !v.is_empty()) {
            query.push(("alpn".into(), url_encode(v)));
        }
    }
    if let Some(v) = security
        .ech_config_list
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        query.push(("ech".into(), url_encode(v)));
    }
    if let Some(v) = security
        .verify_peer_cert_by_name
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        query.push(("vcn".into(), url_encode(v)));
    }
    if let Some(v) = security.cert_sha.as_deref().filter(|v| !v.is_empty()) {
        query.push(("pcs".into(), url_encode(v)));
    }
    if let Some(v) = item.finalmask.as_deref().filter(|v| !v.is_empty()) {
        let encoded = normalize_json_compact(v).unwrap_or_else(|| v.to_string());
        query.push(("fm".into(), url_encode(&encoded)));
    }

    let network = get_network(item);
    let type_token = if network == DEFAULT_NETWORK {
        RAW_NETWORK_ALIAS
    } else {
        network.as_str()
    };
    query.push(("type".into(), type_token.to_string()));

    match network.as_str() {
        "raw" => {
            let header = non_empty(transport.raw_header_type.as_deref()).unwrap_or(NONE);
            query.push(("headerType".into(), header.to_string()));
            if let Some(v) = non_empty(transport.host.as_deref()) {
                query.push(("host".into(), url_encode(v)));
            }
            if let Some(v) = non_empty(transport.path.as_deref()) {
                query.push(("path".into(), url_encode(v)));
            }
        }
        "kcp" => {
            let header = non_empty(transport.kcp_header_type.as_deref()).unwrap_or(NONE);
            query.push(("headerType".into(), header.to_string()));
            if let Some(v) = non_empty(transport.kcp_seed.as_deref()) {
                query.push(("seed".into(), url_encode(v)));
            }
            if let Some(mtu) = transport.kcp_mtu.filter(|m| *m > 0) {
                query.push(("mtu".into(), mtu.to_string()));
            }
        }
        "ws" | "httpupgrade" => {
            if let Some(v) = non_empty(transport.host.as_deref()) {
                query.push(("host".into(), url_encode(v)));
            }
            if let Some(v) = non_empty(transport.path.as_deref()) {
                query.push(("path".into(), url_encode(v)));
            }
        }
        "xhttp" => {
            if let Some(v) = non_empty(transport.host.as_deref()) {
                query.push(("host".into(), url_encode(v)));
            }
            if let Some(v) = non_empty(transport.path.as_deref()) {
                query.push(("path".into(), url_encode(v)));
            }
            if let Some(mode) = non_empty(transport.xhttp_mode.as_deref()) {
                if XHTTP_MODES.contains(&mode) {
                    query.push(("mode".into(), url_encode(mode)));
                }
            }
            if let Some(v) = non_empty(transport.xhttp_extra.as_deref()) {
                let encoded = normalize_json_compact(v).unwrap_or_else(|| v.to_string());
                query.push(("extra".into(), url_encode(&encoded)));
            }
        }
        "grpc" => {
            if let Some(service) = non_empty(transport.grpc_service_name.as_deref()) {
                query.push((
                    "authority".into(),
                    url_encode(transport.grpc_authority.as_deref().unwrap_or("")),
                ));
                query.push(("serviceName".into(), url_encode(service)));
                if let Some(mode) = non_empty(transport.grpc_mode.as_deref()) {
                    if mode == GRPC_GUN_MODE || mode == GRPC_MULTI_MODE {
                        query.push(("mode".into(), url_encode(mode)));
                    }
                }
            }
        }
        _ => {}
    }
}

/// `BaseFmt.ToUriQueryLite`: only `sni` and `alpn` (Hysteria2/TUIC).
pub fn to_uri_query_lite(item: &Profile, query: &mut Vec<(String, String)>) {
    if let Some(v) = non_empty(item.security.sni.as_deref()) {
        query.push(("sni".into(), url_encode(v)));
    }
    if let Some(v) = non_empty(item.security.alpn.as_deref()) {
        query.push(("alpn".into(), url_encode(v)));
    }
}

/// `BaseFmt.ResolveUriQuery`: read the shared security/transport parameters.
pub fn resolve_uri_query(query: &Query, item: &mut Profile) {
    let security = &mut item.security;
    set_optional(
        &mut security.stream_security,
        query.get("security").unwrap_or(""),
    );
    set_optional(&mut security.sni, query.get("sni").unwrap_or(""));
    set_optional(&mut security.alpn, query.get("alpn").unwrap_or(""));
    set_optional(&mut security.fingerprint, query.get("fp").unwrap_or(""));
    set_optional(&mut security.public_key, query.get("pbk").unwrap_or(""));
    set_optional(&mut security.short_id, query.get("sid").unwrap_or(""));
    set_optional(&mut security.spider_x, query.get("spx").unwrap_or(""));
    set_optional(&mut security.mldsa65_verify, query.get("pqv").unwrap_or(""));
    set_optional(
        &mut security.ech_config_list,
        query.get("ech").unwrap_or(""),
    );
    set_optional(
        &mut security.verify_peer_cert_by_name,
        query.get("vcn").unwrap_or(""),
    );
    set_optional(&mut security.cert_sha, query.get("pcs").unwrap_or(""));

    let finalmask = query.get("fm").unwrap_or("");
    item.finalmask = if finalmask.is_empty() {
        None
    } else {
        Some(crate::util::normalize_json_pretty(finalmask).unwrap_or_else(|| finalmask.to_string()))
    };

    let mut net = query.get_or("type", DEFAULT_NETWORK).to_string();
    if net == RAW_NETWORK_ALIAS {
        net = DEFAULT_NETWORK.to_string();
    }
    if !NETWORKS.contains(&net.as_str()) {
        net = DEFAULT_NETWORK.to_string();
    }
    item.network = net.clone();

    let transport = &mut item.transport_extra;
    match net.as_str() {
        "raw" => {
            set_optional(
                &mut transport.raw_header_type,
                query.get_or("headerType", NONE),
            );
            set_optional(&mut transport.host, query.get("host").unwrap_or(""));
            set_optional(&mut transport.path, query.get("path").unwrap_or(""));
        }
        "kcp" => {
            set_optional(
                &mut transport.kcp_header_type,
                query.get_or("headerType", NONE),
            );
            set_optional(&mut transport.kcp_seed, query.get("seed").unwrap_or(""));
            let mtu = query
                .get("mtu")
                .and_then(|v| v.parse::<i32>().ok())
                .filter(|m| *m > 0);
            transport.kcp_mtu = mtu;
        }
        "ws" | "httpupgrade" => {
            set_optional(&mut transport.host, query.get("host").unwrap_or(""));
            set_optional(&mut transport.path, query.get_or("path", "/"));
        }
        "xhttp" => {
            set_optional(&mut transport.host, query.get("host").unwrap_or(""));
            set_optional(&mut transport.path, query.get_or("path", "/"));
            set_optional(&mut transport.xhttp_mode, query.get("mode").unwrap_or(""));
            let extra = query.get("extra").unwrap_or("");
            transport.xhttp_extra = if extra.is_empty() {
                None
            } else {
                Some(crate::util::normalize_json_pretty(extra).unwrap_or_else(|| extra.to_string()))
            };
        }
        "grpc" => {
            set_optional(
                &mut transport.grpc_authority,
                query.get("authority").unwrap_or(""),
            );
            set_optional(
                &mut transport.grpc_service_name,
                query.get("serviceName").unwrap_or(""),
            );
            set_optional(
                &mut transport.grpc_mode,
                query.get_or("mode", GRPC_GUN_MODE),
            );
        }
        _ => {
            item.network = DEFAULT_NETWORK.to_string();
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}

/// Split a `userinfo` string into `(username, password)` following upstream:
/// `resolvedUrl.UserInfo` is percent-decoded once and then split on the first
/// `:` into at most two parts.
pub fn split_user_info(url: &url::Url) -> (String, String) {
    let mut raw = url.username().to_string();
    if let Some(password) = url.password() {
        raw.push(':');
        raw.push_str(password);
    }
    let decoded = crate::util::url_decode(&raw);
    match decoded.split_once(':') {
        Some((user, pass)) => (user.to_string(), pass.to_string()),
        None => (String::new(), decoded),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile::default()
    }

    #[test]
    fn network_normalizes_unknown_and_empty_to_raw() {
        let mut p = profile();
        p.network = "WS".into();
        assert_eq!(get_network(&p), "raw");
        p.network = "ws".into();
        assert_eq!(get_network(&p), "ws");
        p.network = String::new();
        assert_eq!(get_network(&p), "raw");
    }

    #[test]
    fn build_uri_encodes_user_info_and_brackets_ipv6() {
        let uri = build_uri(
            "trojan://",
            "2001:db8::1",
            443,
            "p:a@ss#% +/=",
            &[],
            "#node",
        );
        assert_eq!(
            uri,
            "trojan://p%3Aa%40ss%23%25%20%2B%2F%3D@[2001:db8::1]:443#node"
        );
    }

    #[test]
    fn resolve_then_emit_query_roundtrips_transport() {
        let query = Query::parse(
            "type=ws&security=tls&host=a.example&path=%2Fws&sni=a.example&alpn=h2%2Chttp%2F1.1",
        );
        let mut p = profile();
        resolve_uri_query(&query, &mut p);
        assert_eq!(p.network, "ws");
        assert_eq!(p.transport_extra.host.as_deref(), Some("a.example"));
        assert_eq!(p.transport_extra.path.as_deref(), Some("/ws"));
        assert_eq!(p.security.alpn.as_deref(), Some("h2,http/1.1"));

        let mut out = Vec::new();
        to_uri_query(&p, Some(NONE), &mut out);
        let rendered = out
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        assert!(rendered.contains("type=ws"));
        assert!(rendered.contains("security=tls"));
        assert!(rendered.contains("host=a.example"));
        assert!(rendered.contains("path=%2Fws"));
        assert!(rendered.contains("alpn=h2%2Chttp%2F1.1"));
    }

    #[test]
    fn raw_uses_tcp_alias_and_none_header() {
        let mut p = profile();
        p.network = "raw".into();
        let mut out = Vec::new();
        to_uri_query(&p, Some(NONE), &mut out);
        assert!(out.contains(&("type".to_string(), "tcp".to_string())));
        assert!(out.contains(&("headerType".to_string(), "none".to_string())));
    }
}
