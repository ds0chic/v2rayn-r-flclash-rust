//! DNS use cases (T11): `DNSItem` CRUD per core, SimpleDNS validation,
//! default-config import and regional presets.
//!
//! Storage mirrors `compat/domain-map.yaml`: `DNSItem` rows in `guiNDB.db`.
//! `SimpleDNSItem` lives in `guiNConfig.json` (the settings tree) and is
//! edited through the same window; this module owns the DNS-item half plus
//! the shared validation helpers.

use domain::{CoreType, DnsProfile, DomainError};
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

/// Remote template URLs per region (upstream `Global.*Sources`).
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
            srs_source: "https://github.com/runetfreedom/sing-box-rules/rule-set-{0}/{1}.srs",
            routing_rules_source: "https://github.com/runetfreedom/russia-v2ray-rules-dat/release/routing.json",
            dns_template_base: "https://github.com/runetfreedom/russia-v2ray-rules-dat/release/dns/",
        }),
        RegionalPreset::IranOffline => Some(RegionSources {
            geo_source: "https://github.com/Chocolate4U/Iran-v2ray-rules/releases/latest/download/{0}.dat",
            srs_source: "https://github.com/Chocolate4U/Iran-sing-box-rules/rule-set-{0}/{1}.srs",
            routing_rules_source: "https://github.com/Chocolate4U/Iran-v2ray-rules/release/routing.json",
            dns_template_base: "https://github.com/Chocolate4U/Iran-v2ray-rules/release/dns/",
        }),
    }
}

/// Remote DNS template files that could not be downloaded offline.
pub fn pending_remote_templates(preset: &RegionalPreset) -> Vec<String> {
    match region_sources(preset) {
        None => Vec::new(),
        Some(sources) => ["v2ray.json", "sing_box.json", "simple_dns.json"]
            .into_iter()
            .map(|f| format!("{}{f}", sources.dns_template_base))
            .collect(),
    }
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
}
