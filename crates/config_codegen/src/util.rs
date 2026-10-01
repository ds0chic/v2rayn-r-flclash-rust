//! Shared constants and JSON helpers for both generators.

use std::net::{Ipv4Addr, Ipv6Addr};

use serde_json::{Map, Value};

pub const DEFAULT_SECURITY: &str = "auto";
pub const DEFAULT_NETWORK: &str = "raw";
pub const RAW_HEADER_HTTP: &str = "http";
pub const NONE: &str = "none";
pub const PROXY_TAG: &str = "proxy";
pub const DIRECT_TAG: &str = "direct";
pub const BLOCK_TAG: &str = "block";
pub const DNS_OUTBOUND_TAG: &str = "dns";
pub const DNS_TAG: &str = "dns-module";
pub const DIRECT_DNS_TAG: &str = "direct-dns";
pub const BALANCER_TAG_SUFFIX: &str = "-balancer";
pub const STREAM_SECURITY: &str = "tls";
pub const STREAM_SECURITY_REALITY: &str = "reality";
pub const LOOPBACK: &str = "127.0.0.1";
pub const AS_IS: &str = "AsIs";
pub const IP_IF_NON_MATCH: &str = "IPIfNonMatch";
pub const IP_ON_DEMAND: &str = "IPOnDemand";
pub const GEOSITE_PREFIX: &str = "geosite:";
pub const GEOIP_PREFIX: &str = "geoip:";
pub const USER_EMAIL: &str = "t@t.tt";
pub const ROUTING_RULE_COMMA: &str = "<COMMA>";
pub const GRPC_MULTI_MODE: &str = "multi";
pub const HYSTERIA2_DEFAULT_HOP_INT: i32 = 30;

pub const SINGBOX_DIRECT_DNS_TAG_PREFIX: &str = "direct-dns-";
pub const SINGBOX_REMOTE_DNS_TAG_PREFIX: &str = "remote-dns-";
pub const SINGBOX_DIRECT_DNS_TAG: &str = "direct-dns-1";
pub const SINGBOX_REMOTE_DNS_TAG: &str = "remote-dns-1";
pub const SINGBOX_LOCAL_DNS_TAG: &str = "local-local";
pub const SINGBOX_HOSTS_DNS_TAG: &str = "hosts-dns";
pub const SINGBOX_FAKE_DNS_TAG: &str = "fake-dns";
pub const SINGBOX_SRS_HTTP_CLIENT_TAG: &str = "srs-download-http-client";
pub const SINGBOX_RULESET_URL: &str =
    "https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{0}/{1}.srs";

pub const TUN_MTU_DEFAULT: i32 = 1280;
pub const TUN_IPV4_DEFAULT: &str = "172.18.0.1/30";
pub const TUN_IPV6_DEFAULT: &str = "fc00::172:18:0:1/126";
pub const FAKE_IP_DEFAULT: &str = "198.18.0.0/15";
pub const DOMAIN_DIRECT_DNS_DEFAULT: &str = "119.29.29.29";
pub const DOMAIN_REMOTE_DNS_DEFAULT: &str = "https://cloudflare-dns.com/dns-query";
pub const DOMAIN_PURE_IP_DNS_DEFAULT: &str = "119.29.29.29";
pub const DEFAULT_XHTTP_MODE: &str = "auto";

pub const NETWORKS: [&str; 6] = ["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
pub const XRAY_NETWORKS: [&str; 6] = ["raw", "xhttp", "kcp", "grpc", "ws", "httpupgrade"];
pub const SINGBOX_REJECTED_NETWORKS: [&str; 2] = ["kcp", "xhttp"];
pub const VMESS_SECURITIES: [&str; 5] =
    ["aes-128-gcm", "chacha20-poly1305", "auto", "none", "zero"];
pub const SS_SECURITIES_IN_XRAY: [&str; 11] = [
    "aes-256-gcm",
    "aes-128-gcm",
    "chacha20-poly1305",
    "chacha20-ietf-poly1305",
    "xchacha20-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "plain",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
];
pub const SS_SECURITIES_IN_SINGBOX: [&str; 19] = [
    "aes-256-gcm",
    "aes-192-gcm",
    "aes-128-gcm",
    "chacha20-ietf-poly1305",
    "xchacha20-ietf-poly1305",
    "none",
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "aes-128-ctr",
    "aes-192-ctr",
    "aes-256-ctr",
    "aes-128-cfb",
    "aes-192-cfb",
    "aes-256-cfb",
    "rc4-md5",
    "chacha20-ietf",
    "xchacha20",
    "plain",
];
pub const XHTTP_MODES: [&str; 4] = ["auto", "packet-up", "stream-up", "stream-one"];
pub const KCP_HEADER_TYPES: [&str; 6] = ["srtp", "utp", "wechat-video", "dtls", "wireguard", "dns"];
pub const KCP_HEADER_MASK_MAP: [(&str, &str); 6] = [
    ("srtp", "srtp"),
    ("utp", "utp"),
    ("wechat-video", "wechat"),
    ("dtls", "dtls"),
    ("wireguard", "wireguard"),
    ("dns", "dns"),
];
pub const RAW_HTTP_USER_AGENT_TEXTS: [(&str, &str); 7] = [
    ("chrome", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36"),
    ("firefox", "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) Gecko/20100101 Firefox/90.0"),
    ("safari", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/14.1.1 Safari/605.1.15"),
    ("edge", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36 Edg/91.0.864.70"),
    ("none", ""),
    ("golang", "Go-http-client/1.1"),
    ("curl", "curl/7.68.0"),
];
pub const TUN_ICMP_ROUTING_POLICIES: [&str; 5] = ["rule", "direct", "unreachable", "drop", "reply"];
pub const TAG_PLACEHOLDER: &str = "{{tag}}";
pub const DETOUR_PLACEHOLDER: &str = "{{detour}}";
pub const INTERFACE_PLACEHOLDER: &str = "{{interface}}";

/// `Global.PredefinedHosts`.
pub fn predefined_hosts() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "dns.google",
            vec![
                "8.8.8.8",
                "8.8.4.4",
                "2001:4860:4860::8888",
                "2001:4860:4860::8844",
            ],
        ),
        (
            "dns.alidns.com",
            vec![
                "223.5.5.5",
                "223.6.6.6",
                "2400:3200::1",
                "2400:3200:baba::1",
            ],
        ),
        (
            "one.one.one.one",
            vec![
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "1dot1dot1dot1.cloudflare-dns.com",
            vec![
                "1.1.1.1",
                "1.0.0.1",
                "2606:4700:4700::1111",
                "2606:4700:4700::1001",
            ],
        ),
        (
            "cloudflare-dns.com",
            vec![
                "104.16.249.249",
                "104.16.248.249",
                "2606:4700::6810:f8f9",
                "2606:4700::6810:f9f9",
            ],
        ),
        (
            "dns.cloudflare.com",
            vec![
                "162.159.61.8",
                "172.64.41.8",
                "2a06:98c1:52::8",
                "2803:f800:53::8",
            ],
        ),
        ("dot.pub", vec!["1.12.12.12", "120.53.53.53"]),
        ("doh.pub", vec!["1.12.12.12", "120.53.53.53"]),
        (
            "dns.quad9.net",
            vec!["9.9.9.9", "149.112.112.112", "2620:fe::fe", "2620:fe::9"],
        ),
        (
            "dns.yandex.net",
            vec![
                "77.88.8.8",
                "77.88.8.1",
                "2a02:6b8::feed:0ff",
                "2a02:6b8:0:1::feed:0ff",
            ],
        ),
        (
            "dns.sb",
            vec!["45.11.45.11", "185.222.222.222", "2a09::", "2a11::"],
        ),
        (
            "dns.umbrella.com",
            vec![
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        (
            "dns.sse.cisco.com",
            vec![
                "208.67.220.220",
                "208.67.222.222",
                "2620:119:35::35",
                "2620:119:53::53",
            ],
        ),
        (
            "engage.cloudflareclient.com",
            vec!["162.159.192.1", "2606:4700:d0::a29f:c001"],
        ),
    ]
}

/// `SampleHttpRequest` with the three upstream placeholders.
pub const SAMPLE_HTTP_REQUEST: &str = r#"{"version":"1.1","method":"GET","path":[$requestPath$],"headers":{"Host":[$requestHost$],"User-Agent":[$requestUserAgent$],"Accept-Encoding":["gzip, deflate"],"Connection":["keep-alive"],"Pragma":"no-cache"}}"#;

/// `SampleTunRules` embedded fallback.
pub fn sample_tun_rules() -> Value {
    serde_json::json!([
        {"network": "udp", "port": "135,137-139,5353", "outboundTag": "block"},
        {"ip": ["224.0.0.0/3", "ff00::/8"], "outboundTag": "block"}
    ])
}

/// `tun_singbox_rules` embedded fallback.
pub fn sample_tun_singbox_rules() -> Value {
    serde_json::json!([
        {"network": ["udp"], "port": [135, 137, 138, 139, 5353], "action": "reject"},
        {"ip_cidr": ["224.0.0.0/3", "ff00::/8"], "action": "reject"}
    ])
}

/// `singbox_fakeip_filter` embedded fallback (trimmed upstream list).
pub fn sample_fakeip_filter() -> Value {
    serde_json::json!({
        "domain": [
            "localhost.ptlogin2.qq.com", "localhost.sec.qq.com", "music.taihe.com", "musicapi.taihe.com",
            "proxy.golang.org", "ps.res.netease.com", "swcdn.apple.com", "swdist.apple.com",
            "swdownload.apple.com", "swquery.apple.com", "swscan.apple.com", "turn.cloudflare.com"
        ],
        "domain_keyword": ["ntp", "stun", "time"],
        "domain_regex": [
            "^[^.]+$", "^[^.]+\\.[^.]+\\.xboxlive\\.com$", "^localhost\\.[^.]+\\.weixin\\.qq\\.com$",
            "^mijia\\scloud$", "^xbox\\.[^.]+\\.microsoft\\.com$", "^xbox\\.[^.]+\\.[^.]+\\.microsoft\\.com$"
        ],
        "domain_suffix": [
            "126.net", "3gppnetwork.org", "home.arpa", "invalid", "lan", "local", "localdomain",
            "localhost", "msftconnecttest.com", "msftncsi.com", "oray.com"
        ]
    })
}

pub fn obj() -> Map<String, Value> {
    Map::new()
}

pub fn put(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.insert(key.to_string(), value);
}

pub fn put_str(map: &mut Map<String, Value>, key: &str, value: &str) {
    map.insert(key.to_string(), Value::String(value.to_string()));
}

pub fn put_opt_str(map: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(v) = value {
        if !v.is_empty() {
            map.insert(key.to_string(), Value::String(v.to_string()));
        }
    }
}

pub fn put_opt_string(map: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    put_opt_str(map, key, value.map(String::as_str));
}

pub fn put_opt_i64(map: &mut Map<String, Value>, key: &str, value: Option<i64>) {
    if let Some(v) = value {
        map.insert(key.to_string(), Value::from(v));
    }
}

pub fn put_opt_i32(map: &mut Map<String, Value>, key: &str, value: Option<i32>) {
    put_opt_i64(map, key, value.map(i64::from));
}

pub fn put_opt_f64(map: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if let Some(v) = value {
        map.insert(key.to_string(), Value::from(v));
    }
}

pub fn put_opt_bool(map: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if let Some(v) = value {
        map.insert(key.to_string(), Value::Bool(v));
    }
}

pub fn put_opt_value(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        if !v.is_null() {
            map.insert(key.to_string(), v);
        }
    }
}

/// Upstream `Utils.String2List`: comma separated, trims empties.
pub fn string2_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn string2_list_opt(input: Option<&String>) -> Option<Vec<String>> {
    let input = input?;
    let list = string2_list(input);
    if list.is_empty() {
        None
    } else {
        Some(list)
    }
}

pub fn first_host(host: &str) -> Option<String> {
    string2_list(host).into_iter().next()
}

/// Upstream `ProfileItem.GetAlpn()`.
pub fn alpn_list(alpn: &str) -> Option<Vec<String>> {
    if alpn.trim().is_empty() {
        None
    } else {
        Some(string2_list(alpn))
    }
}

pub fn is_valid_ipv6(address: &str) -> bool {
    address.parse::<Ipv6Addr>().is_ok()
}

pub fn is_ip_address(value: &str) -> bool {
    value.parse::<Ipv4Addr>().is_ok() || value.parse::<Ipv6Addr>().is_ok()
}

pub fn is_loopback_address(value: &str) -> bool {
    if let Ok(v4) = value.parse::<Ipv4Addr>() {
        return v4.is_loopback();
    }
    if let Ok(v6) = value.parse::<Ipv6Addr>() {
        return v6.is_loopback();
    }
    false
}

/// Upstream `Utils.IsPrivateNetwork` (used by template `ProxyDetour`).
pub fn is_private_network(value: &str) -> bool {
    if let Ok(v4) = value.parse::<Ipv4Addr>() {
        let o = v4.octets();
        return v4.is_loopback()
            || o[0] == 10
            || (o[0] == 172 && (16..=31).contains(&o[1]))
            || (o[0] == 192 && o[1] == 168);
    }
    if let Ok(v6) = value.parse::<Ipv6Addr>() {
        let o = v6.octets();
        if v6.is_loopback() {
            return true;
        }
        if o[0] == 0xfe && (o[1] & 0xc0) == 0x80 {
            return true;
        }
        if (o[0] & 0xfe) == 0xfc {
            return true;
        }
        if v6.to_ipv4_mapped().is_some_and(|m| {
            let o = m.octets();
            o[0] == 10 || (o[0] == 172 && (16..=31).contains(&o[1])) || (o[0] == 192 && o[1] == 168)
        }) {
            return true;
        }
    }
    false
}

/// Upstream `Utils.DomainStrategy4Sbox`.
pub fn domain_strategy4_sbox(strategy: Option<&str>) -> Option<String> {
    let strategy = strategy?;
    if strategy.starts_with("UseIPv6") {
        Some("prefer_ipv6".into())
    } else if strategy.starts_with("UseIP") {
        Some("prefer_ipv4".into())
    } else if strategy.starts_with("ForceIPv6") {
        Some("ipv6_only".into())
    } else if strategy.starts_with("ForceIP") {
        Some("ipv4_only".into())
    } else {
        None
    }
}

/// Upstream `Utils.GetExeName`.
pub fn exe_name(name: &str, platform: crate::input::Platform) -> String {
    if name.is_empty() {
        return name.to_string();
    }
    let lower = name.to_lowercase();
    if lower.contains('.') {
        return name.to_string();
    }
    match platform {
        crate::input::Platform::Windows => format!("{name}.exe"),
        _ => name.to_string(),
    }
}

pub fn join_path(directory: &str, file: &str) -> String {
    if directory.is_empty() {
        file.to_string()
    } else if directory.ends_with('/') || directory.ends_with('\\') {
        format!("{directory}{file}")
    } else {
        format!("{directory}/{file}")
    }
}

/// Upstream `Utils.ParseUrl` subset (scheme/domain/port/path).
pub fn parse_url(url: &str) -> (String, String, i32, String) {
    let url = url.trim();
    if url.is_empty() {
        return (String::new(), String::new(), 0, String::new());
    }
    let (scheme, rest) = match url.find("://") {
        Some(idx) => (url[..idx].to_string(), &url[idx + 3..]),
        None => (String::new(), url),
    };
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(idx) => (rest[..idx].to_string(), rest[idx..].to_string()),
        None => (rest.to_string(), String::new()),
    };
    let authority = match authority.rfind('@') {
        Some(idx) => authority[idx + 1..].to_string(),
        None => authority,
    };
    let (domain, port) = parse_authority(&authority);
    let port = if (scheme.eq_ignore_ascii_case("http") && port == 80)
        || (scheme.eq_ignore_ascii_case("https") && port == 443)
    {
        0
    } else {
        port
    };
    (domain, scheme, port, path)
}

fn parse_authority(authority: &str) -> (String, i32) {
    if authority.is_empty() {
        return (String::new(), 0);
    }
    if let Some(start) = authority.find('[') {
        if let Some(end) = authority.rfind(']') {
            let host = &authority[start..=end];
            let rest = &authority[end + 1..];
            if let Some(port) = rest.strip_prefix(':').and_then(|p| p.parse::<i32>().ok()) {
                return (host.to_string(), port);
            }
            return (authority.to_string(), 0);
        }
    }
    if let Some(idx) = authority.rfind(':') {
        let port_str = &authority[idx + 1..];
        if !port_str.is_empty() && port_str.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(port) = port_str.parse::<i32>() {
                return (authority[..idx].to_string(), port);
            }
        }
    }
    (authority.to_string(), 0)
}

/// Upstream `CertPemManager.ParsePemChain` (best effort, no validation).
pub fn parse_pem_chain(cert: &str) -> Vec<String> {
    const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
    const END: &str = "-----END CERTIFICATE-----";
    let mut result = Vec::new();
    let mut rest = cert;
    while let Some(start) = rest.find(BEGIN) {
        let after_start = &rest[start..];
        let Some(end) = after_start.find(END) else {
            break;
        };
        let block = &after_start[..end + END.len()];
        result.push(block.to_string());
        rest = &after_start[end + END.len()..];
    }
    result
}

/// Parse the upstream `hosts` text format (`host ip` per line, `#` comments).
pub fn parse_hosts_to_dictionary(content: Option<&str>) -> Vec<(String, Vec<String>)> {
    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    let Some(content) = content else {
        return result;
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(host) = parts.next() else { continue };
        let values: Vec<String> = parts.map(ToString::to_string).collect();
        if values.is_empty() {
            continue;
        }
        if let Some(entry) = result.iter_mut().find(|(h, _)| h == host) {
            entry.1.extend(values);
        } else {
            result.push((host.to_string(), values));
        }
    }
    result
}

/// Split a raw user-agent token into its mapped value.
pub fn raw_user_agent_value(user_agent: &str) -> String {
    RAW_HTTP_USER_AGENT_TEXTS
        .iter()
        .find(|(k, _)| *k == user_agent)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| user_agent.to_string())
}

// ---------------------------------------------------------------------------
// IPv4 / IPv6 subnet math for the TUN route-exclude logic.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Net4 {
    pub addr: u32,
    pub prefix: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Net6 {
    pub addr: u128,
    pub prefix: u8,
}

fn mask4(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix))
    }
}

fn mask6(prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - u32::from(prefix))
    }
}

pub fn parse_net4(s: &str) -> Option<Net4> {
    let (addr, prefix) = s.split_once('/')?;
    let addr: Ipv4Addr = addr.trim().parse().ok()?;
    let prefix: u8 = prefix.trim().parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let raw = u32::from(addr) & mask4(prefix);
    Some(Net4 { addr: raw, prefix })
}

pub fn parse_net6(s: &str) -> Option<Net6> {
    let (addr, prefix) = s.split_once('/')?;
    let addr: Ipv6Addr = addr.trim().parse().ok()?;
    let prefix: u8 = prefix.trim().parse().ok()?;
    if prefix > 128 {
        return None;
    }
    let raw = u128::from(addr) & mask6(prefix);
    Some(Net6 { addr: raw, prefix })
}

pub fn net4_to_string(net: Net4) -> String {
    format!("{}/{}", Ipv4Addr::from(net.addr), net.prefix)
}

pub fn net6_to_string(net: Net6) -> String {
    format!("{}/{}", Ipv6Addr::from(net.addr), net.prefix)
}

fn contains4(a: Net4, b: Net4) -> bool {
    a.prefix <= b.prefix && (b.addr & mask4(a.prefix)) == a.addr
}

fn contains6(a: Net6, b: Net6) -> bool {
    a.prefix <= b.prefix && (b.addr & mask6(a.prefix)) == a.addr
}

fn split4(net: Net4) -> (Net4, Net4) {
    let prefix = net.prefix + 1;
    let bit = 1u32 << (32 - u32::from(prefix));
    (
        Net4 {
            addr: net.addr,
            prefix,
        },
        Net4 {
            addr: net.addr | bit,
            prefix,
        },
    )
}

fn split6(net: Net6) -> (Net6, Net6) {
    let prefix = net.prefix + 1;
    let bit = 1u128 << (128 - u32::from(prefix));
    (
        Net6 {
            addr: net.addr,
            prefix,
        },
        Net6 {
            addr: net.addr | bit,
            prefix,
        },
    )
}

pub fn subtract4(a: Net4, b: Net4) -> Vec<Net4> {
    if !contains4(a, b) {
        return vec![a];
    }
    if a.prefix == b.prefix {
        return vec![];
    }
    let (l, r) = split4(a);
    let mut out = subtract4(l, b);
    out.extend(subtract4(r, b));
    out
}

pub fn subtract6(a: Net6, b: Net6) -> Vec<Net6> {
    if !contains6(a, b) {
        return vec![a];
    }
    if a.prefix == b.prefix {
        return vec![];
    }
    let (l, r) = split6(a);
    let mut out = subtract6(l, b);
    out.extend(subtract6(r, b));
    out
}

fn supernet4(mut nets: Vec<Net4>) -> Vec<Net4> {
    nets.sort_by_key(|n| (n.addr, n.prefix));
    nets.dedup();
    let mut out: Vec<Net4> = Vec::new();
    for net in nets {
        if out.iter().any(|existing| contains4(*existing, net)) {
            continue;
        }
        out.retain(|existing| !contains4(net, *existing));
        out.push(net);
    }
    loop {
        let mut merged = false;
        let mut next: Vec<Net4> = Vec::new();
        let mut used = vec![false; out.len()];
        for i in 0..out.len() {
            if used[i] {
                continue;
            }
            let a = out[i];
            let mut paired = false;
            for j in (i + 1)..out.len() {
                if used[j] {
                    continue;
                }
                let b = out[j];
                if a.prefix == b.prefix && a.prefix > 0 {
                    let parent = Net4 {
                        addr: a.addr & mask4(a.prefix - 1),
                        prefix: a.prefix - 1,
                    };
                    if a.addr | (1u32 << (32 - u32::from(a.prefix))) == b.addr
                        && parent.addr == a.addr & mask4(parent.prefix)
                    {
                        next.push(parent);
                        used[i] = true;
                        used[j] = true;
                        paired = true;
                        merged = true;
                        break;
                    }
                }
            }
            if !paired {
                next.push(a);
                used[i] = true;
            }
        }
        out = next;
        if !merged {
            break;
        }
    }
    out.sort_by_key(|n| (n.addr, n.prefix));
    out
}

fn supernet6(mut nets: Vec<Net6>) -> Vec<Net6> {
    nets.sort_by_key(|n| (n.addr, n.prefix));
    nets.dedup();
    let mut out: Vec<Net6> = Vec::new();
    for net in nets {
        if out.iter().any(|existing| contains6(*existing, net)) {
            continue;
        }
        out.retain(|existing| !contains6(net, *existing));
        out.push(net);
    }
    loop {
        let mut merged = false;
        let mut next: Vec<Net6> = Vec::new();
        let mut used = vec![false; out.len()];
        for i in 0..out.len() {
            if used[i] {
                continue;
            }
            let a = out[i];
            let mut paired = false;
            for j in (i + 1)..out.len() {
                if used[j] {
                    continue;
                }
                let b = out[j];
                if a.prefix == b.prefix && a.prefix > 0 {
                    let parent = Net6 {
                        addr: a.addr & mask6(a.prefix - 1),
                        prefix: a.prefix - 1,
                    };
                    if a.addr | (1u128 << (128 - u32::from(a.prefix))) == b.addr
                        && parent.addr == a.addr & mask6(parent.prefix)
                    {
                        next.push(parent);
                        used[i] = true;
                        used[j] = true;
                        paired = true;
                        merged = true;
                        break;
                    }
                }
            }
            if !paired {
                next.push(a);
                used[i] = true;
            }
        }
        out = next;
        if !merged {
            break;
        }
    }
    out.sort_by_key(|n| (n.addr, n.prefix));
    out
}

/// Upstream `TunModeItem.RouteExcludeAddress` -> `autoSystemRoutingTable`.
pub fn tun_route_table(exclude: &[String], has_global_ipv6: bool) -> Vec<String> {
    let mut include4 = vec![parse_net4("0.0.0.0/0").unwrap()];
    let mut include6 = if has_global_ipv6 {
        vec![parse_net6("::/0").unwrap()]
    } else {
        vec![]
    };
    for item in exclude {
        if let Some(net) = parse_net4(item) {
            include4 = include4
                .into_iter()
                .flat_map(|a| subtract4(a, net))
                .collect();
        } else if let Some(net) = parse_net6(item) {
            include6 = include6
                .into_iter()
                .flat_map(|a| subtract6(a, net))
                .collect();
        }
    }
    let mut out: Vec<String> = supernet4(include4)
        .into_iter()
        .map(net4_to_string)
        .collect();
    out.extend(supernet6(include6).into_iter().map(net6_to_string));
    out
}

/// Strip a trailing prefix (`x/30` -> `x/32`), used for sing-box TUN self rules.
pub fn to_single_address_prefix(address: &str) -> String {
    let addr = address.split('/').next().unwrap_or(address);
    if let Ok(v4) = addr.parse::<Ipv4Addr>() {
        return format!("{v4}/32");
    }
    if let Ok(v6) = addr.parse::<Ipv6Addr>() {
        return format!("{v6}/128");
    }
    address.to_string()
}
