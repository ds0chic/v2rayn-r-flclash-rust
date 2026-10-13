//! DNS profiles and SimpleDNS helpers (`DNSItem` 9 + `SimpleDNSItem`).
//!
//! Mirrors `compat/fields.entities.yaml` (`FLD-ENT-116..124`) and the upstream
//! `DNSSettingViewModel` save validation: Xray `NormalDNS`/`TunDNS` must be
//! either plain server lists or JSON objects with a `servers` key; sing-box
//! `NormalDNS`/`TunDNS` must deserialize to `Dns4Sbox` with non-empty
//! `servers[].type`. Hosts merging follows `V2rayDnsService.FillDnsHosts`
//! (common hosts, then system hosts via `TryAdd`, then custom hosts).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::entities::DnsProfile;
use crate::error::{codes, DomainError};
use crate::settings::SimpleDnsItem;

/// Embedded default DNS texts (`DNSSettingViewModel` import-default commands).
pub const DEFAULT_V2RAY_DNS: &str =
    include_str!("../../../fixtures/source/upstream/sample/dns_v2ray_normal");
pub const DEFAULT_SINGBOX_DNS: &str =
    include_str!("../../../fixtures/source/upstream/sample/dns_singbox_normal");
pub const DEFAULT_TUN_SINGBOX_DNS: &str =
    include_str!("../../../fixtures/source/upstream/sample/tun_singbox_dns");

/// `Global.PredefinedHosts` (common hosts added when `AddCommonHosts` is set).
pub fn predefined_hosts() -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    map.insert(
        "dns.google".to_string(),
        vec![
            "8.8.8.8".to_string(),
            "8.8.4.4".to_string(),
            "2001:4860:4860::8888".to_string(),
            "2001:4860:4860::8844".to_string(),
        ],
    );
    map.insert(
        "dns.alidns.com".to_string(),
        vec![
            "223.5.5.5".to_string(),
            "223.6.6.6".to_string(),
            "2400:3200::1".to_string(),
            "2400:3200:baba::1".to_string(),
        ],
    );
    map.insert(
        "one.one.one.one".to_string(),
        vec![
            "1.1.1.1".to_string(),
            "1.0.0.1".to_string(),
            "2606:4700:4700::1111".to_string(),
            "2606:4700:4700::1001".to_string(),
        ],
    );
    map.insert(
        "1dot1dot1dot1.cloudflare-dns.com".to_string(),
        vec![
            "1.1.1.1".to_string(),
            "1.0.0.1".to_string(),
            "2606:4700:4700::1111".to_string(),
            "2606:4700:4700::1001".to_string(),
        ],
    );
    map.insert(
        "cloudflare-dns.com".to_string(),
        vec![
            "104.16.249.249".to_string(),
            "104.16.248.249".to_string(),
            "2606:4700::6810:f8f9".to_string(),
            "2606:4700::6810:f9f9".to_string(),
        ],
    );
    map.insert(
        "dns.cloudflare.com".to_string(),
        vec![
            "162.159.61.8".to_string(),
            "172.64.41.8".to_string(),
            "2a06:98c1:52::8".to_string(),
            "2803:f800:53::8".to_string(),
        ],
    );
    map.insert(
        "dot.pub".to_string(),
        vec!["1.12.12.12".to_string(), "120.53.53.53".to_string()],
    );
    map.insert(
        "doh.pub".to_string(),
        vec!["1.12.12.12".to_string(), "120.53.53.53".to_string()],
    );
    map.insert(
        "dns.quad9.net".to_string(),
        vec![
            "9.9.9.9".to_string(),
            "149.112.112.112".to_string(),
            "2620:fe::fe".to_string(),
            "2620:fe::9".to_string(),
        ],
    );
    map.insert(
        "dns.yandex.net".to_string(),
        vec![
            "77.88.8.8".to_string(),
            "77.88.8.1".to_string(),
            "2a02:6b8::feed:0ff".to_string(),
            "2a02:6b8:0:1::feed:0ff".to_string(),
        ],
    );
    map.insert(
        "dns.sb".to_string(),
        vec![
            "45.11.45.11".to_string(),
            "185.222.222.222".to_string(),
            "2a09::".to_string(),
            "2a11::".to_string(),
        ],
    );
    map.insert(
        "dns.umbrella.com".to_string(),
        vec![
            "208.67.220.220".to_string(),
            "208.67.222.222".to_string(),
            "2620:119:35::35".to_string(),
            "2620:119:53::53".to_string(),
        ],
    );
    map.insert(
        "dns.sse.cisco.com".to_string(),
        vec![
            "208.67.220.220".to_string(),
            "208.67.222.222".to_string(),
            "2620:119:35::35".to_string(),
            "2620:119:53::53".to_string(),
        ],
    );
    map.insert(
        "engage.cloudflareclient.com".to_string(),
        vec![
            "162.159.192.1".to_string(),
            "2606:4700:d0::a29f:c001".to_string(),
        ],
    );
    map
}

/// Direct / remote / bootstrap DNS candidates (`Global.Domain*DNSAddress`).
pub const DIRECT_DNS_CANDIDATES: &[&str] = &[
    "119.29.29.29",
    "223.5.5.5",
    "119.29.29.29,223.5.5.5,https://doh.pub/dns-query",
    "https://doh.pub/dns-query",
    "https://dns.alidns.com/dns-query",
    "https://doh.pub/dns-query,https://dns.alidns.com/dns-query",
    "localhost",
];
pub const REMOTE_DNS_CANDIDATES: &[&str] = &[
    "https://cloudflare-dns.com/dns-query",
    "https://dns.google/dns-query",
    "https://cloudflare-dns.com/dns-query,https://dns.google/dns-query,8.8.8.8",
    "https://dns.cloudflare.com/dns-query",
    "https://doh.dns.sb/dns-query",
    "https://doh.opendns.com/dns-query",
    "https://common.dot.dns.yandex.net/dns-query",
    "8.8.8.8",
    "1.1.1.1",
    "185.222.222.222",
    "208.67.222.222",
    "77.88.8.8",
];
pub const BOOTSTRAP_DNS_CANDIDATES: &[&str] = &["119.29.29.29", "223.5.5.5", "localhost"];
pub const FAKE_IP_RANGES: &[&str] = &["198.18.0.0/15", "11.0.0.0/8"];

/// Validate one Xray custom DNS text (upstream `SaveSettingAsync`).
///
/// Empty text is allowed (falls back to the embedded default at generation
/// time). Otherwise plain server lists pass; JSON-looking text must parse to
/// an object with a `servers` key.
pub fn validate_xray_dns_text(text: &str) -> Result<(), DomainError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    if !(text.contains('{') || text.contains('}')) {
        return Ok(());
    }
    let value: serde_json::Value = serde_json::from_str(text).map_err(|_| {
        DomainError::new(codes::FIELD_FORMAT, "error.dns_text_invalid").with_field("NormalDNS")
    })?;
    if value.get("servers").is_none() {
        return Err(
            DomainError::new(codes::FIELD_FORMAT, "error.dns_text_invalid").with_field("NormalDNS"),
        );
    }
    Ok(())
}

/// Minimal `Dns4Sbox` shape for validation (servers with `type`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SboxDnsShim {
    #[serde(default)]
    pub servers: Vec<SboxServerShim>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SboxServerShim {
    #[serde(rename = "type", default)]
    pub server_type: Option<String>,
}

/// Validate one sing-box custom DNS text (upstream `SaveSettingAsync`).
pub fn validate_singbox_dns_text(text: &str) -> Result<(), DomainError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let dns: SboxDnsShim = serde_json::from_str(text).map_err(|_| {
        DomainError::new(codes::FIELD_FORMAT, "error.dns_text_invalid").with_field("NormalDNS")
    })?;
    if dns.servers.is_empty()
        || dns
            .servers
            .iter()
            .any(|s| s.server_type.as_deref().unwrap_or("").trim().is_empty())
    {
        return Err(
            DomainError::new(codes::FIELD_FORMAT, "error.dns_text_invalid").with_field("NormalDNS"),
        );
    }
    Ok(())
}

/// Validate a DNS profile draft (remarks required + per-core DNS texts).
pub fn validate_dns_profile(profile: &DnsProfile, is_singbox: bool) -> Result<(), DomainError> {
    if profile.remarks.trim().is_empty() {
        return Err(
            DomainError::new(codes::FIELD_REQUIRED, "error.remarks_required").with_field("remarks"),
        );
    }
    for text in [&profile.normal_dns, &profile.tun_dns]
        .into_iter()
        .flatten()
    {
        if is_singbox {
            validate_singbox_dns_text(text)?;
        } else {
            validate_xray_dns_text(text)?;
        }
    }
    Ok(())
}

/// Parse `domain ip...` lines into a host -> ips map
/// (upstream `Utils.ParseHostsToDictionary`, `Utils.cs:359-381`): a line must
/// contain a literal space, tokens split on spaces/tabs only (commas stay
/// inside tokens), and repeated domains concatenate.
pub fn parse_hosts_to_map(text: Option<&str>) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Some(text) = text else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.contains(' ') {
            continue;
        }
        let mut parts = line.split([' ', '\t']).filter(|s| !s.is_empty());
        let Some(domain) = parts.next() else {
            continue;
        };
        let ips: Vec<String> = parts.map(ToString::to_string).collect();
        if ips.is_empty() {
            continue;
        }
        map.entry(domain.to_string()).or_default().extend(ips);
    }
    map
}

/// Read the OS hosts file (read-only) into a host -> ip map.
/// Failures yield an empty map; the caller decides whether that matters.
pub fn read_system_hosts() -> BTreeMap<String, String> {
    if cfg!(windows) {
        // Upstream `Utils.GetSystemHosts()` (`Utils.cs:1032-1053`) merges
        // `hosts.ics` over `hosts`; a missing file reads as empty.
        let mut merged = read_hosts_file("C:\\Windows\\System32\\drivers\\etc\\hosts");
        merged.extend(read_hosts_file(
            "C:\\Windows\\System32\\drivers\\etc\\hosts.ics",
        ));
        merged
    } else {
        read_hosts_file("/etc/hosts")
    }
}

/// Parse a hosts file into host -> ip, mirroring upstream
/// `Utils.GetSystemHosts(path)` (`Utils.cs:964-1030`): only the second token
/// is a hostname, later rows overwrite earlier ones, and `localhost` entries
/// are kept. Non-IP first tokens and over-long names are skipped.
pub fn read_hosts_file(path: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return map;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip inline comments.
        let line = line.split('#').next().unwrap_or("").trim();
        let mut tokens = line.split([' ', '\t']).filter(|s| !s.is_empty());
        let (Some(ip), Some(host)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        // Upstream `IsIpAddress` gate.
        if ip.parse::<std::net::IpAddr>().is_err() {
            continue;
        }
        if host.is_empty() || host.len() > 255 {
            continue;
        }
        map.insert(host.to_string(), ip.to_string());
    }
    map
}

/// Merge hosts for generation: common hosts, then system hosts (`TryAdd`
/// semantics: never overwrite), then custom hosts (overwrite).
pub fn merge_hosts(
    simple: &SimpleDnsItem,
    system_hosts: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<String>> {
    let mut merged: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if simple.add_common_hosts == Some(true) {
        merged.extend(predefined_hosts());
    }
    if simple.use_system_hosts == Some(true) {
        for (host, ip) in system_hosts {
            merged
                .entry(host.clone())
                .or_insert_with(|| vec![ip.clone()]);
        }
    }
    for (host, ips) in parse_hosts_to_map(simple.hosts.as_deref()) {
        merged.insert(host, ips);
    }
    merged
}

/// The two built-in DNS rows (`ConfigHandler.InitBuiltinDNS`).
pub fn builtin_dns_profiles() -> Vec<DnsProfile> {
    vec![
        DnsProfile {
            id: String::new(),
            remarks: "V2ray".to_string(),
            enabled: false,
            core_type: crate::enums::CoreType::Xray,
            use_system_hosts: false,
            normal_dns: None,
            tun_dns: None,
            domain_strategy4_freedom: None,
            domain_dns_address: None,
            extra: Default::default(),
        },
        DnsProfile {
            id: String::new(),
            remarks: "sing-box".to_string(),
            enabled: false,
            core_type: crate::enums::CoreType::SingBox,
            use_system_hosts: false,
            normal_dns: None,
            tun_dns: None,
            domain_strategy4_freedom: None,
            domain_dns_address: None,
            extra: Default::default(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hosts_matches_upstream_dictionary_semantics() {
        // S1-10, upstream `Utils.ParseHostsToDictionary` (`Utils.cs:359-381`):
        // comma-separated addresses stay one token (no comma split).
        let map = parse_hosts_to_map(Some("d ip1,ip2"));
        assert_eq!(map.get("d"), Some(&vec!["ip1,ip2".to_string()]));
        // A line without a literal space is dropped, even if tab-separated.
        let map = parse_hosts_to_map(Some("d\t10.0.0.1"));
        assert!(map.is_empty(), "tab-only line must be dropped: {map:?}");
        // Repeated domains concatenate (GroupBy + SelectMany), not overwrite.
        let map = parse_hosts_to_map(Some("d 1.1.1.1\nd 2.2.2.2"));
        assert_eq!(
            map.get("d"),
            Some(&vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()])
        );
    }

    #[test]
    fn read_hosts_file_matches_upstream_get_system_hosts() {
        // S1-11, upstream `Utils.GetSystemHosts(path)` (`Utils.cs:964-1030`):
        // only the second token is a hostname, later rows overwrite,
        // `localhost` is kept.
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "b04-hosts-{}-{}.txt",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(
            &path,
            "1.2.3.4 a b c\n1.2.3.5 a\n127.0.0.1 localhost\n# comment\n\n",
        )
        .unwrap();
        let map = read_hosts_file(path.to_str().unwrap());
        let _ = std::fs::remove_file(&path);
        assert_eq!(map.get("a"), Some(&"1.2.3.5".to_string()));
        assert!(!map.contains_key("b") && !map.contains_key("c"));
        assert_eq!(map.get("localhost"), Some(&"127.0.0.1".to_string()));
    }
}
