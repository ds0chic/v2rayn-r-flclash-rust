//! DNS use cases (T11): `DNSItem` CRUD per core, SimpleDNS validation,
//! default-config import and regional presets.
//!
//! Storage mirrors `compat/domain-map.yaml`: `DNSItem` rows in `guiNDB.db`.
//! `SimpleDNSItem` lives in `guiNConfig.json` (the settings tree) and is
//! edited through the same window; this module owns the DNS-item half plus
//! the shared validation helpers.

use domain::{CoreType, DnsProfile, DomainError, SimpleDnsItem};
use persistence::RawRow;
use serde_json::{json, Value};

/// Fresh DNS-profile id.
pub fn new_dns_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("dns-{nanos:x}-{seq:x}")
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Map a `DnsProfile` onto a `DNSItem` row.
pub fn dns_to_row(profile: &DnsProfile) -> RawRow {
    let mut row = RawRow::new("DNSItem");
    row.set("Id", json!(profile.id));
    row.set("Remarks", json!(profile.remarks));
    row.set("Enabled", json!(i64::from(profile.enabled)));
    row.set("CoreType", json!(profile.core_type.value()));
    row.set("UseSystemHosts", json!(i64::from(profile.use_system_hosts)));
    row.set(
        "NormalDNS",
        profile
            .normal_dns
            .clone()
            .map_or(Value::Null, Value::String),
    );
    row.set(
        "TunDNS",
        profile.tun_dns.clone().map_or(Value::Null, Value::String),
    );
    row.set(
        "DomainStrategy4Freedom",
        profile
            .domain_strategy4_freedom
            .clone()
            .map_or(Value::Null, Value::String),
    );
    row.set(
        "DomainDNSAddress",
        profile
            .domain_dns_address
            .clone()
            .map_or(Value::Null, Value::String),
    );
    row
}

/// Map a `DNSItem` row back onto the domain profile.
pub fn dns_from_row(row: &RawRow) -> DnsProfile {
    DnsProfile {
        id: row.string("Id"),
        remarks: row.string("Remarks"),
        enabled: row.bool("Enabled"),
        core_type: row
            .opt_i64("CoreType")
            .and_then(|v| CoreType::from_value(v as i32))
            .unwrap_or(CoreType::Xray),
        use_system_hosts: row.bool("UseSystemHosts"),
        normal_dns: non_empty(row.opt_string("NormalDNS")),
        tun_dns: non_empty(row.opt_string("TunDNS")),
        domain_strategy4_freedom: non_empty(row.opt_string("DomainStrategy4Freedom")),
        domain_dns_address: non_empty(row.opt_string("DomainDNSAddress")),
        extra: Default::default(),
    }
}

/// True when the profile targets sing-box (per-core validation differs).
pub fn is_singbox(profile: &DnsProfile) -> bool {
    profile.core_type == CoreType::SingBox
}

/// Normalize a draft before persisting: fresh id when empty + validation.
pub fn normalize_dns(mut profile: DnsProfile) -> Result<DnsProfile, DomainError> {
    if profile.id.trim().is_empty() {
        profile.id = new_dns_id();
    }
    domain::dns::validate_dns_profile(&profile, is_singbox(&profile))?;
    Ok(profile)
}

/// Copy stored unknown-field extras into an incoming save (same contract as
/// `routing::preserve_extras`: stored keys survive unless overridden).
pub fn preserve_dns_extras(existing: Option<&DnsProfile>, incoming: &mut DnsProfile) {
    let Some(stored) = existing else {
        return;
    };
    for (key, value) in &stored.extra {
        incoming
            .extra
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
}

/// Repository boundary for DNS profiles.
pub trait DnsRepository {
    fn list(&self) -> Result<Vec<DnsProfile>, DomainError>;
    fn get(&self, id: &str) -> Result<Option<DnsProfile>, DomainError>;
    fn upsert(&mut self, item: DnsProfile) -> Result<(), DomainError>;
    fn remove(&mut self, id: &str) -> Result<bool, DomainError>;
    fn count(&self) -> usize;
}

/// In-memory DNS repository (tests / non-persistent engines).
#[derive(Default)]
pub struct InMemoryDnsRepository {
    items: Vec<DnsProfile>,
}

impl InMemoryDnsRepository {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_items(items: Vec<DnsProfile>) -> Self {
        Self { items }
    }
}

impl DnsRepository for InMemoryDnsRepository {
    fn list(&self) -> Result<Vec<DnsProfile>, DomainError> {
        let mut items = self.items.clone();
        items.sort_by(|a, b| {
            a.core_type
                .value()
                .cmp(&b.core_type.value())
                .then_with(|| a.remarks.cmp(&b.remarks))
        });
        Ok(items)
    }

    fn get(&self, id: &str) -> Result<Option<DnsProfile>, DomainError> {
        Ok(self.items.iter().find(|d| d.id == id).cloned())
    }

    fn upsert(&mut self, item: DnsProfile) -> Result<(), DomainError> {
        if let Some(slot) = self.items.iter_mut().find(|d| d.id == item.id) {
            *slot = item;
        } else {
            self.items.push(item);
        }
        Ok(())
    }

    fn remove(&mut self, id: &str) -> Result<bool, DomainError> {
        let before = self.items.len();
        self.items.retain(|d| d.id != id);
        Ok(self.items.len() != before)
    }

    fn count(&self) -> usize {
        self.items.len()
    }
}

/// Regional preset outcome. Russia / Iran need external templates; without
/// network the engine applies the offline fallback (URLs + embedded defaults)
/// and reports which remote files are still pending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegionalPreset {
    Default,
    RussiaOffline,
    IranOffline,
}

/// Remote template URLs per region, copied verbatim from upstream `Global`
/// (`GeoFilesSources` / `SingboxRulesetSources` / `RoutingRulesSources` /
/// `DNSTemplateSources`). This is the single source of truth: the regional
/// preset writes these into the settings tree and the consumers read the tree.
pub struct RegionSources {
    pub geo_source: &'static str,
    pub srs_source: &'static str,
    pub routing_rules_source: &'static str,
    pub dns_template_base: &'static str,
}

pub fn region_sources(preset: &RegionalPreset) -> Option<RegionSources> {
    match preset {
        RegionalPreset::Default => None,
        RegionalPreset::RussiaOffline => Some(RegionSources {
            geo_source: "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/{0}.dat",
            srs_source: "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-rules-dat/release/sing-box/rule-set-{0}/{1}.srs",
            routing_rules_source: "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-custom-routing-list/main/v2rayN/template.json",
            dns_template_base: "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-custom-routing-list/main/v2rayN/",
        }),
        RegionalPreset::IranOffline => Some(RegionSources {
            geo_source: "https://github.com/Chocolate4U/Iran-v2ray-rules/releases/latest/download/{0}.dat",
            srs_source: "https://raw.githubusercontent.com/chocolate4u/Iran-sing-box-rules/rule-set/{1}.srs",
            routing_rules_source: "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/template.json",
            dns_template_base: "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/",
        }),
    }
}

/// Built-in Geo `.dat` download template (upstream `Global.GeoUrl`).
pub const BUILTIN_GEO_URL: &str =
    "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/{0}.dat";

/// Built-in SRS download template (upstream `Global.SingboxRulesetUrl`),
/// mirrored by `config_codegen::util::SINGBOX_RULESET_URL`.
pub const BUILTIN_SRS_URL: &str =
    "https://raw.githubusercontent.com/2dust/sing-box-rules/rule-set-{0}/{1}.srs";

/// First built-in subscription-conversion URL (upstream `Global.SubConvertUrls[0]`).
pub const BUILTIN_SUB_CONVERT_URL: &str = "https://sub.xeton.dev/sub?url={0}";

fn non_empty_trim(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Effective Geo `.dat` download template: the stored `GeoSourceUrl` when set,
/// else the upstream built-in (`UpdateService.GetGeoFilesRequest`).
pub fn effective_geo_source(const_item: &domain::ConstItem) -> String {
    non_empty_trim(const_item.geo_source_url.as_deref())
        .unwrap_or_else(|| BUILTIN_GEO_URL.to_string())
}

/// Effective SRS download template: the stored `SrsSourceUrl` when set, else
/// the upstream built-in (`UpdateService.GetSrsFileRequest`). This value is
/// what the sing-box rule-set generation emits (`route.rule_set[].url`).
pub fn effective_srs_source(const_item: &domain::ConstItem) -> String {
    non_empty_trim(const_item.srs_source_url.as_deref())
        .unwrap_or_else(|| BUILTIN_SRS_URL.to_string())
}

/// Effective subscription-conversion URL: the stored `SubConvertUrl` when set,
/// else the upstream built-in (`SubscriptionHandler.DownloadMainSubscription`).
pub fn effective_sub_convert_url(const_item: &domain::ConstItem) -> String {
    non_empty_trim(const_item.sub_convert_url.as_deref())
        .unwrap_or_else(|| BUILTIN_SUB_CONVERT_URL.to_string())
}

/// Effective routing-rule template source (`ConfigHandler.InitRouting`).
///
/// `None` = no external template configured: seed the built-in routing set.
/// `Some(url)` = an external template is configured and must be fetched; a
/// download failure is an explicit error, never a silent built-in fallback.
pub fn effective_routing_template_source(const_item: &domain::ConstItem) -> Option<String> {
    non_empty_trim(const_item.route_rules_template_source_url.as_deref())
}

/// Remote DNS template files that could not be downloaded offline.
pub fn pending_remote_templates(preset: &RegionalPreset) -> Vec<String> {
    match region_sources(preset) {
        None => Vec::new(),
        Some(sources) => PRESET_TEMPLATE_FILES
            .into_iter()
            .map(|f| format!("{}{f}", sources.dns_template_base))
            .collect(),
    }
}

/// The three template files served under a region's DNS template base
/// (upstream `GetExternalDNSItem` + `GetExternalSimpleDNSItem`).
pub const PRESET_TEMPLATE_FILES: [&str; 3] = ["v2ray.json", "sing_box.json", "simple_dns.json"];

/// A region's fully downloaded DNS material: both per-core rows plus the
/// `SimpleDNSItem`. It is only produced after every file fetched and parsed,
/// so the caller can persist it without risking a half-written preset.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionalDnsPlan {
    pub xray: DnsProfile,
    pub singbox: DnsProfile,
    pub simple: SimpleDnsItem,
}

/// Join a region base URL (trailing slash optional) with a template file name.
pub fn template_url(base: &str, file: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{file}")
    } else {
        format!("{base}/{file}")
    }
}

fn template_error() -> DomainError {
    DomainError::new(
        domain::codes::INVALID_ARGUMENT,
        "error.dns_template_invalid",
    )
}

fn is_http(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

fn get_field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    let map = value.as_object()?;
    names
        .iter()
        .find_map(|name| map.get(*name).filter(|entry| !entry.is_null()))
}

fn get_string(value: &Value, names: &[&str]) -> Option<String> {
    match get_field(value, names) {
        Some(Value::String(text)) => Some(text.clone()),
        Some(other) => Some(other.to_string()),
        None => None,
    }
}

fn get_bool(value: &Value, names: &[&str]) -> Option<bool> {
    match get_field(value, names) {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(Value::String(text)) => text.parse::<bool>().ok(),
        Some(Value::Number(number)) => number.as_i64().map(|v| v != 0),
        _ => None,
    }
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.trim().is_empty())
}

/// Parse an upstream external DNS template (`v2ray.json` / `sing_box.json`).
///
/// The fixed upstream entity is PascalCase; camelCase is accepted for the
/// synthetic fixtures. Fields are extracted explicitly so unknown keys are
/// ignored. `NormalDNS`/`TunDNS` may hold a URL (upstream fetches it later),
/// so validation happens in [`load_dns_template`], after URL resolution.
pub fn parse_dns_template(core: CoreType, content: &str) -> Result<DnsProfile, DomainError> {
    let value: Value = serde_json::from_str(content).map_err(|_| template_error())?;
    if !value.is_object() {
        return Err(template_error());
    }
    Ok(DnsProfile {
        id: String::new(),
        remarks: if core == CoreType::SingBox {
            "sing-box".to_string()
        } else {
            "V2ray".to_string()
        },
        enabled: get_bool(&value, &["Enabled", "enabled"]).unwrap_or(false),
        core_type: core,
        use_system_hosts: get_bool(&value, &["UseSystemHosts", "useSystemHosts"]).unwrap_or(false),
        normal_dns: non_empty_string(get_string(&value, &["NormalDNS", "normalDns"])),
        tun_dns: non_empty_string(get_string(&value, &["TunDNS", "tunDns"])),
        domain_strategy4_freedom: non_empty_string(get_string(
            &value,
            &["DomainStrategy4Freedom", "domainStrategy4Freedom"],
        )),
        domain_dns_address: non_empty_string(get_string(
            &value,
            &["DomainDNSAddress", "domainDnsAddress"],
        )),
        extra: Default::default(),
    })
}

/// Parse an upstream `simple_dns.json` template.
pub fn parse_simple_dns_template(content: &str) -> Result<SimpleDnsItem, DomainError> {
    let value: Value = serde_json::from_str(content).map_err(|_| template_error())?;
    if !value.is_object() {
        return Err(template_error());
    }
    Ok(SimpleDnsItem {
        use_system_hosts: get_bool(&value, &["UseSystemHosts", "useSystemHosts"]),
        add_common_hosts: get_bool(&value, &["AddCommonHosts", "addCommonHosts"]),
        fake_ip: get_bool(&value, &["FakeIP", "FakeIp", "fakeIp", "fake_ip"]),
        global_fake_ip: get_bool(&value, &["GlobalFakeIp", "globalFakeIp", "global_fake_ip"]),
        fake_ip_range: non_empty_string(get_string(&value, &["FakeIPRange", "fakeIpRange"])),
        block_binding_query: get_bool(&value, &["BlockBindingQuery", "blockBindingQuery"]),
        block_aaaa_query: get_bool(&value, &["BlockAAAAQuery", "blockAaaaQuery"]),
        direct_dns: non_empty_string(get_string(&value, &["DirectDNS", "directDns"])),
        remote_dns: non_empty_string(get_string(&value, &["RemoteDNS", "remoteDns"])),
        bootstrap_dns: non_empty_string(get_string(&value, &["BootstrapDNS", "bootstrapDns"])),
        strategy4_freedom: non_empty_string(get_string(
            &value,
            &["Strategy4Freedom", "strategy4Freedom"],
        )),
        strategy4_proxy: non_empty_string(get_string(
            &value,
            &["Strategy4Proxy", "strategy4Proxy"],
        )),
        strategy4_proxy_dial: non_empty_string(get_string(
            &value,
            &["Strategy4ProxyDial", "strategy4ProxyDial"],
        )),
        serve_stale: get_bool(&value, &["ServeStale", "serveStale"]),
        parallel_query: get_bool(&value, &["ParallelQuery", "parallelQuery"]),
        hosts: get_string(&value, &["Hosts", "hosts"]),
        direct_expected_ips: non_empty_string(get_string(
            &value,
            &["DirectExpectedIPs", "directExpectedIps"],
        )),
        enable_happy_eyeballs: get_bool(&value, &["EnableHappyEyeballs", "enableHappyEyeballs"]),
        extra: Default::default(),
    })
}

/// Resolve one downloaded DNS template: follow a URL left in `NormalDNS` /
/// `TunDNS`, then validate the per-core text (upstream downloads nested URLs
/// before assigning the item).
pub async fn load_dns_template(core: CoreType, content: &str) -> Result<DnsProfile, DomainError> {
    let mut profile = parse_dns_template(core, content)?;
    if let Some(url) = profile.normal_dns.clone().filter(|s| is_http(s)) {
        profile.normal_dns = Some(crate::routing::fetch_rules_text(&url, false, None).await?);
    }
    if let Some(url) = profile.tun_dns.clone().filter(|s| is_http(s)) {
        profile.tun_dns = Some(crate::routing::fetch_rules_text(&url, false, None).await?);
    }
    domain::dns::validate_dns_profile(&profile, core == CoreType::SingBox)?;
    Ok(profile)
}

/// Download and parse a whole region preset.
///
/// All three files are fetched before any parse, and every parse must succeed
/// before a [`RegionalDnsPlan`] is returned. A single failure returns an error
/// and yields no plan, so the caller never persists a half-applied preset.
pub async fn fetch_region_dns_plan(base: &str) -> Result<RegionalDnsPlan, DomainError> {
    let xray_text = crate::routing::fetch_rules_text(
        &template_url(base, PRESET_TEMPLATE_FILES[0]),
        false,
        None,
    )
    .await?;
    let sbox_text = crate::routing::fetch_rules_text(
        &template_url(base, PRESET_TEMPLATE_FILES[1]),
        false,
        None,
    )
    .await?;
    let simple_text = crate::routing::fetch_rules_text(
        &template_url(base, PRESET_TEMPLATE_FILES[2]),
        false,
        None,
    )
    .await?;
    let xray = load_dns_template(CoreType::Xray, &xray_text).await?;
    let singbox = load_dns_template(CoreType::SingBox, &sbox_text).await?;
    let simple = parse_simple_dns_template(&simple_text)?;
    Ok(RegionalDnsPlan {
        xray,
        singbox,
        simple,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_row_round_trip() {
        let profile = DnsProfile {
            id: "d1".into(),
            remarks: "V2ray".into(),
            enabled: true,
            core_type: CoreType::Xray,
            use_system_hosts: true,
            normal_dns: Some("{\"servers\": []}".into()),
            tun_dns: None,
            domain_strategy4_freedom: Some("UseIP".into()),
            domain_dns_address: Some("119.29.29.29".into()),
            extra: Default::default(),
        };
        let row = dns_to_row(&profile);
        let loaded = dns_from_row(&row);
        assert_eq!(loaded, profile);
    }

    #[test]
    fn singbox_dns_rejects_typeless_servers() {
        assert!(domain::dns::validate_singbox_dns_text(r#"{"servers": []}"#).is_err());
        assert!(domain::dns::validate_singbox_dns_text(
            r#"{"servers": [{"tag": "remote", "type": "tcp", "server": "8.8.8.8"}]}"#
        )
        .is_ok());
    }

    #[test]
    fn template_url_joins_with_single_slash() {
        assert_eq!(
            template_url("https://example.invalid/dns/", "v2ray.json"),
            "https://example.invalid/dns/v2ray.json"
        );
        assert_eq!(
            template_url("https://example.invalid/dns", "simple_dns.json"),
            "https://example.invalid/dns/simple_dns.json"
        );
    }

    #[test]
    fn parse_dns_template_reads_pascal_and_camel() {
        let pascal = r#"{"Enabled": true, "UseSystemHosts": true,
            "NormalDNS": "8.8.8.8,1.1.1.1", "TunDNS": "1.1.1.1",
            "DomainStrategy4Freedom": "UseIP", "DomainDNSAddress": "119.29.29.29"}"#;
        let profile = parse_dns_template(CoreType::Xray, pascal).expect("parse");
        assert!(profile.enabled);
        assert!(profile.use_system_hosts);
        assert_eq!(profile.normal_dns.as_deref(), Some("8.8.8.8,1.1.1.1"));
        assert_eq!(profile.domain_dns_address.as_deref(), Some("119.29.29.29"));
        assert_eq!(profile.core_type, CoreType::Xray);
        assert_eq!(profile.id, "");

        let camel = r#"{"enabled": false, "normalDns": "9.9.9.9"}"#;
        let profile = parse_dns_template(CoreType::SingBox, camel).expect("parse");
        assert!(!profile.enabled);
        assert_eq!(profile.remarks, "sing-box");
        assert_eq!(profile.normal_dns.as_deref(), Some("9.9.9.9"));
    }

    #[test]
    fn parse_dns_template_rejects_invalid() {
        assert!(parse_dns_template(CoreType::Xray, "not json").is_err());
        assert!(parse_dns_template(CoreType::Xray, "[1,2]").is_err());
        assert!(parse_dns_template(CoreType::Xray, "40").is_err());
        assert!(parse_simple_dns_template("nope").is_err());
    }

    /// SP-13/CP-08: malformed DNS templates are rejected at the parse
    /// boundary and never reach the repository (read failure must not become
    /// a write).
    #[test]
    fn sp13_malformed_dns_template_never_writes() {
        let repo = InMemoryDnsRepository::with_items(vec![DnsProfile {
            id: "dns-keep".into(),
            remarks: "synthetic keep".into(),
            enabled: true,
            core_type: CoreType::Xray,
            ..Default::default()
        }]);
        assert!(parse_dns_template(CoreType::Xray, "not json{{").is_err());
        assert!(parse_dns_template(CoreType::Xray, "[1,2]").is_err());
        assert!(parse_dns_template(CoreType::SingBox, "40").is_err());
        assert!(parse_simple_dns_template("nope").is_err());
        assert!(parse_simple_dns_template("[1,2]").is_err());
        let kept = repo.list().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, "dns-keep");
        assert_eq!(repo.count(), 1);
    }

    /// SP-13/CP-08: a partial multi-delete reconciles against the
    /// authoritative store (committed ids stay gone, failed ids stay listed).
    #[test]
    fn sp13_partial_delete_reconciles_against_authoritative() {
        let mut repo = InMemoryDnsRepository::with_items(vec![
            DnsProfile {
                id: "dns-a".into(),
                remarks: "synthetic a".into(),
                enabled: true,
                core_type: CoreType::Xray,
                ..Default::default()
            },
            DnsProfile {
                id: "dns-b".into(),
                remarks: "synthetic b".into(),
                enabled: true,
                core_type: CoreType::SingBox,
                ..Default::default()
            },
        ]);
        assert!(repo.remove("dns-a").unwrap());
        assert!(!repo.remove("dns-missing").unwrap());
        let ids: Vec<String> = repo.list().unwrap().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, vec!["dns-b".to_string()]);
        assert_eq!(repo.count(), 1);
        assert!(repo.get("dns-a").unwrap().is_none());
    }

    #[test]
    fn parse_simple_dns_template_reads_fields() {
        let text = r#"{"FakeIP": true, "GlobalFakeIp": false,
            "DirectDNS": "119.29.29.29", "UseSystemHosts": true,
            "BlockAAAAQuery": false}"#;
        let simple = parse_simple_dns_template(text).expect("parse");
        assert_eq!(simple.fake_ip, Some(true));
        assert_eq!(simple.global_fake_ip, Some(false));
        assert_eq!(simple.direct_dns.as_deref(), Some("119.29.29.29"));
        assert_eq!(simple.use_system_hosts, Some(true));
        assert_eq!(simple.block_aaaa_query, Some(false));
    }

    #[test]
    fn region_sources_match_upstream_global_arrays() {
        let russia = region_sources(&RegionalPreset::RussiaOffline).expect("russia");
        assert_eq!(
            russia.geo_source,
            "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/{0}.dat"
        );
        assert_eq!(
            russia.srs_source,
            "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-rules-dat/release/sing-box/rule-set-{0}/{1}.srs"
        );
        assert_eq!(
            russia.routing_rules_source,
            "https://raw.githubusercontent.com/runetfreedom/russia-v2ray-custom-routing-list/main/v2rayN/template.json"
        );
        let iran = region_sources(&RegionalPreset::IranOffline).expect("iran");
        assert_eq!(
            iran.geo_source,
            "https://github.com/Chocolate4U/Iran-v2ray-rules/releases/latest/download/{0}.dat"
        );
        assert_eq!(
            iran.srs_source,
            "https://raw.githubusercontent.com/chocolate4u/Iran-sing-box-rules/rule-set/{1}.srs"
        );
        assert_eq!(
            iran.routing_rules_source,
            "https://raw.githubusercontent.com/Chocolate4U/Iran-v2ray-rules/main/v2rayN/template.json"
        );
        assert!(region_sources(&RegionalPreset::Default).is_none());
    }

    #[test]
    fn effective_sources_use_settings_then_builtin() {
        let mut item = domain::ConstItem::default();
        // Empty settings fall back to the upstream built-ins.
        assert_eq!(effective_geo_source(&item), BUILTIN_GEO_URL);
        assert_eq!(effective_srs_source(&item), BUILTIN_SRS_URL);
        assert_eq!(effective_sub_convert_url(&item), BUILTIN_SUB_CONVERT_URL);
        assert_eq!(effective_routing_template_source(&item), None);
        // Blank/whitespace is treated as unset (upstream `IsNullOrEmpty`).
        item.srs_source_url = Some("   ".into());
        assert_eq!(effective_srs_source(&item), BUILTIN_SRS_URL);
        // Explicit values win and are trimmed.
        item.geo_source_url = Some("https://mirror.example/{0}.dat".into());
        item.srs_source_url = Some(" https://mirror.example/{0}/{1}.srs ".into());
        item.sub_convert_url = Some("https://convert.example/sub?url={0}".into());
        item.route_rules_template_source_url =
            Some(" https://mirror.example/template.json ".into());
        assert_eq!(
            effective_geo_source(&item),
            "https://mirror.example/{0}.dat"
        );
        assert_eq!(
            effective_srs_source(&item),
            "https://mirror.example/{0}/{1}.srs"
        );
        assert_eq!(
            effective_sub_convert_url(&item),
            "https://convert.example/sub?url={0}"
        );
        assert_eq!(
            effective_routing_template_source(&item).as_deref(),
            Some("https://mirror.example/template.json")
        );
    }
}
