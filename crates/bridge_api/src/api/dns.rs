//! T11 FRB functions: DNS profiles, SimpleDNS and regional presets.
//!
//! The DNS window (`DNSSettingWindow`) drives these; generation consumes the
//! stored rows through `AppEngine::build_codegen_input`.

use domain::{CoreType, DomainError};
use flutter_rust_bridge::frb;

use crate::api::contract::ErrorDto;
use crate::api::engine::{engine, error_dto};

/// One DNS profile (`DNSItem`, 9 properties).
#[derive(Clone, Default)]
pub struct DnsProfileDto {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    pub core_type: CoreType,
    pub use_system_hosts: bool,
    pub normal_dns: Option<String>,
    pub tun_dns: Option<String>,
    pub domain_strategy4_freedom: Option<String>,
    pub domain_dns_address: Option<String>,
}

/// Result of `list_dns`.
#[derive(Clone)]
pub struct DnsPageDto {
    pub items: Vec<DnsProfileDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `save_dns` / `get_dns`.
#[derive(Clone)]
pub struct DnsDtoResult {
    pub ok: bool,
    pub item: Option<DnsProfileDto>,
    pub error: Option<ErrorDto>,
}

/// The `SimpleDNSItem` half of the DNS window.
#[derive(Clone, Default)]
pub struct SimpleDnsDto {
    pub use_system_hosts: Option<bool>,
    pub add_common_hosts: Option<bool>,
    pub fake_ip: Option<bool>,
    pub global_fake_ip: Option<bool>,
    pub fake_ip_range: Option<String>,
    pub block_binding_query: Option<bool>,
    pub block_aaaa_query: Option<bool>,
    pub direct_dns: Option<String>,
    pub remote_dns: Option<String>,
    pub bootstrap_dns: Option<String>,
    pub strategy4_freedom: Option<String>,
    pub strategy4_proxy: Option<String>,
    pub strategy4_proxy_dial: Option<String>,
    pub serve_stale: Option<bool>,
    pub parallel_query: Option<bool>,
    pub hosts: Option<String>,
    pub direct_expected_ips: Option<String>,
    pub enable_happy_eyeballs: Option<bool>,
}

/// Result of loading/saving SimpleDNS.
#[derive(Clone)]
pub struct SimpleDnsDtoResult {
    pub ok: bool,
    pub item: Option<SimpleDnsDto>,
    pub revision: u64,
    pub error: Option<ErrorDto>,
}

/// Result of `apply_regional_preset`.
#[derive(Clone)]
pub struct RegionalPresetResult {
    pub ok: bool,
    pub preset: String,
    pub pending_urls: Vec<String>,
    pub error: Option<ErrorDto>,
}

fn dns_to_dto(d: domain::DnsProfile) -> DnsProfileDto {
    DnsProfileDto {
        id: d.id,
        remarks: d.remarks,
        enabled: d.enabled,
        core_type: d.core_type,
        use_system_hosts: d.use_system_hosts,
        normal_dns: d.normal_dns,
        tun_dns: d.tun_dns,
        domain_strategy4_freedom: d.domain_strategy4_freedom,
        domain_dns_address: d.domain_dns_address,
    }
}

fn dto_to_dns(d: DnsProfileDto) -> domain::DnsProfile {
    domain::DnsProfile {
        id: d.id,
        remarks: d.remarks,
        enabled: d.enabled,
        core_type: d.core_type,
        use_system_hosts: d.use_system_hosts,
        normal_dns: d.normal_dns.filter(|s| !s.trim().is_empty()),
        tun_dns: d.tun_dns.filter(|s| !s.trim().is_empty()),
        domain_strategy4_freedom: d.domain_strategy4_freedom.filter(|s| !s.trim().is_empty()),
        domain_dns_address: d.domain_dns_address.filter(|s| !s.trim().is_empty()),
        extra: Default::default(),
    }
}

fn simple_to_dto(s: &domain::SimpleDnsItem) -> SimpleDnsDto {
    SimpleDnsDto {
        use_system_hosts: s.use_system_hosts,
        add_common_hosts: s.add_common_hosts,
        fake_ip: s.fake_ip,
        global_fake_ip: s.global_fake_ip,
        fake_ip_range: s.fake_ip_range.clone(),
        block_binding_query: s.block_binding_query,
        block_aaaa_query: s.block_aaaa_query,
        direct_dns: s.direct_dns.clone(),
        remote_dns: s.remote_dns.clone(),
        bootstrap_dns: s.bootstrap_dns.clone(),
        strategy4_freedom: s.strategy4_freedom.clone(),
        strategy4_proxy: s.strategy4_proxy.clone(),
        strategy4_proxy_dial: s.strategy4_proxy_dial.clone(),
        serve_stale: s.serve_stale,
        parallel_query: s.parallel_query,
        hosts: s.hosts.clone(),
        direct_expected_ips: s.direct_expected_ips.clone(),
        enable_happy_eyeballs: s.enable_happy_eyeballs,
    }
}

fn dto_to_simple(d: SimpleDnsDto) -> domain::SimpleDnsItem {
    domain::SimpleDnsItem {
        use_system_hosts: d.use_system_hosts,
        add_common_hosts: d.add_common_hosts,
        fake_ip: d.fake_ip,
        global_fake_ip: d.global_fake_ip,
        fake_ip_range: d.fake_ip_range.filter(|s| !s.trim().is_empty()),
        block_binding_query: d.block_binding_query,
        block_aaaa_query: d.block_aaaa_query,
        direct_dns: d.direct_dns.filter(|s| !s.trim().is_empty()),
        remote_dns: d.remote_dns.filter(|s| !s.trim().is_empty()),
        bootstrap_dns: d.bootstrap_dns.filter(|s| !s.trim().is_empty()),
        strategy4_freedom: d.strategy4_freedom.filter(|s| !s.trim().is_empty()),
        strategy4_proxy: d.strategy4_proxy.filter(|s| !s.trim().is_empty()),
        strategy4_proxy_dial: d.strategy4_proxy_dial.filter(|s| !s.trim().is_empty()),
        serve_stale: d.serve_stale,
        parallel_query: d.parallel_query,
        hosts: d.hosts.filter(|s| !s.trim().is_empty()),
        direct_expected_ips: d.direct_expected_ips.filter(|s| !s.trim().is_empty()),
        enable_happy_eyeballs: d.enable_happy_eyeballs,
        extra: Default::default(),
    }
}

fn err_dto(e: DomainError) -> ErrorDto {
    error_dto(e)
}

/// `list_dns` — all DNS profiles ordered by core then remarks.
#[frb(sync)]
pub fn list_dns() -> DnsPageDto {
    match engine().list_dns() {
        Ok(items) => DnsPageDto {
            items: items.into_iter().map(dns_to_dto).collect(),
            error: None,
        },
        Err(e) => DnsPageDto {
            items: Vec::new(),
            error: Some(err_dto(e)),
        },
    }
}

/// `save_dns` — insert or replace one DNS profile.
#[frb(sync)]
pub fn save_dns(draft: DnsProfileDto) -> DnsDtoResult {
    match engine().save_dns(dto_to_dns(draft)) {
        Ok(saved) => DnsDtoResult {
            ok: true,
            item: Some(dns_to_dto(saved)),
            error: None,
        },
        Err(e) => DnsDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `import_default_dns` — fill a core row with the embedded default config
/// (`v2ray` / `sing-box` two commands; offline-safe, no network).
#[frb(sync)]
pub fn import_default_dns(core: CoreType) -> DnsDtoResult {
    match engine().import_default_dns(core) {
        Ok(saved) => DnsDtoResult {
            ok: true,
            item: Some(dns_to_dto(saved)),
            error: None,
        },
        Err(e) => DnsDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `load_simple_dns` — the current `SimpleDNSItem` plus settings revision.
#[frb(sync)]
pub fn load_simple_dns() -> SimpleDnsDtoResult {
    match engine().load_settings() {
        Ok(loaded) => SimpleDnsDtoResult {
            ok: true,
            item: Some(simple_to_dto(&loaded.settings.simple_dns_item)),
            revision: loaded.revision,
            error: None,
        },
        Err(e) => SimpleDnsDtoResult {
            ok: false,
            item: None,
            revision: 0,
            error: Some(err_dto(e)),
        },
    }
}

/// `save_simple_dns` — whole-tree save with the SimpleDNS group replaced.
/// `expected_revision` is the settings revision from `load_simple_dns`.
///
/// The DNS window has no `GlobalFakeIp` control (neither does upstream's),
/// so a `None` here means "not edited", not "clear": the stored value is
/// kept. Stored unknown keys (`extra`) are likewise preserved because this
/// DTO has no channel for them.
#[frb(sync)]
pub fn save_simple_dns(draft: SimpleDnsDto, expected_revision: u64) -> SimpleDnsDtoResult {
    let mut settings = match engine().load_settings() {
        Ok(loaded) => loaded.settings,
        Err(e) => {
            return SimpleDnsDtoResult {
                ok: false,
                item: None,
                revision: 0,
                error: Some(err_dto(e)),
            };
        }
    };
    let stored = settings.simple_dns_item.clone();
    let mut simple = dto_to_simple(draft);
    if simple.global_fake_ip.is_none() {
        simple.global_fake_ip = stored.global_fake_ip;
    }
    if simple.extra.is_empty() {
        simple.extra = stored.extra;
    }
    settings.simple_dns_item = simple;
    match engine().save_settings(settings, expected_revision) {
        Ok(outcome) => SimpleDnsDtoResult {
            ok: true,
            item: engine()
                .load_settings()
                .ok()
                .map(|l| simple_to_dto(&l.settings.simple_dns_item)),
            revision: outcome.new_revision,
            error: None,
        },
        Err(e) => SimpleDnsDtoResult {
            ok: false,
            item: None,
            revision: 0,
            error: Some(err_dto(e)),
        },
    }
}

fn preset_enum(name: &str) -> application::dns::RegionalPreset {
    match name {
        "Russia" => application::dns::RegionalPreset::RussiaOffline,
        "Iran" => application::dns::RegionalPreset::IranOffline,
        _ => application::dns::RegionalPreset::Default,
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("dns preset runtime")
        .block_on(future)
}

/// Persist a fully downloaded region plan. Only reached after every template
/// fetched and parsed, so a download/parse failure can never write a partial
/// preset. Existing rows keep their identity (upstream `GetExternalDNSItem`
/// assigns `template.Id/Enabled/Remarks = currentItem.*`).
fn apply_downloaded_plan(
    preset: application::dns::RegionalPreset,
    mut plan: application::dns::RegionalDnsPlan,
) -> Result<(), DomainError> {
    for profile in [&mut plan.xray, &mut plan.singbox] {
        if let Some(existing) = engine().get_dns_for_core(profile.core_type)? {
            profile.id = existing.id;
            profile.remarks = existing.remarks;
            profile.enabled = existing.enabled;
        }
    }
    engine().save_dns(plan.xray.clone())?;
    engine().save_dns(plan.singbox.clone())?;

    let loaded = engine().load_settings()?;
    let mut settings = loaded.settings;
    if let Some(sources) = application::dns::region_sources(&preset) {
        settings.const_item.geo_source_url = Some(sources.geo_source.to_string());
        settings.const_item.srs_source_url = Some(sources.srs_source.to_string());
        settings.const_item.route_rules_template_source_url =
            Some(sources.routing_rules_source.to_string());
    }
    settings.simple_dns_item = plan.simple;
    engine().save_settings(settings, loaded.revision)?;
    Ok(())
}

/// `apply_regional_preset` — `Default` resets the built-in DNS. `Russia` /
/// `Iran` download the region's `v2ray.json` / `sing_box.json` /
/// `simple_dns.json` templates (upstream `Global.DomainDNSAddress` sources),
/// and persist them only when all three fetched and parsed. A failed remote
/// chain reports the error and leaves the stored DNS untouched.
#[frb(sync)]
pub fn apply_regional_preset(preset: String) -> RegionalPresetResult {
    let region = preset_enum(&preset);
    let name = match region {
        application::dns::RegionalPreset::RussiaOffline => "Russia",
        application::dns::RegionalPreset::IranOffline => "Iran",
        application::dns::RegionalPreset::Default => "Default",
    };
    if region == application::dns::RegionalPreset::Default {
        return match engine().apply_regional_preset(region) {
            Ok(_) => RegionalPresetResult {
                ok: true,
                preset: name.to_string(),
                pending_urls: Vec::new(),
                error: None,
            },
            Err(e) => RegionalPresetResult {
                ok: false,
                preset: name.to_string(),
                pending_urls: Vec::new(),
                error: Some(err_dto(e)),
            },
        };
    }
    let Some(sources) = application::dns::region_sources(&region) else {
        return RegionalPresetResult {
            ok: false,
            preset: name.to_string(),
            pending_urls: Vec::new(),
            error: Some(err_dto(DomainError::new(
                domain::codes::INTERNAL,
                "error.internal",
            ))),
        };
    };
    let plan = match block_on(application::dns::fetch_region_dns_plan(
        sources.dns_template_base,
    )) {
        Ok(plan) => plan,
        Err(e) => {
            return RegionalPresetResult {
                ok: false,
                preset: name.to_string(),
                pending_urls: application::dns::pending_remote_templates(&region),
                error: Some(err_dto(e)),
            };
        }
    };
    match apply_downloaded_plan(region, plan) {
        Ok(()) => RegionalPresetResult {
            ok: true,
            preset: name.to_string(),
            pending_urls: Vec::new(),
            error: None,
        },
        Err(e) => RegionalPresetResult {
            ok: false,
            preset: name.to_string(),
            pending_urls: Vec::new(),
            error: Some(err_dto(e)),
        },
    }
}

/// `default_dns_text` — the embedded template for the import buttons,
/// so the UI can preview without a round-trip (`v2ray` / `singbox` / `tun`).
#[frb(sync)]
pub fn default_dns_text(kind: String) -> String {
    match kind.as_str() {
        "singbox" => domain::dns::DEFAULT_SINGBOX_DNS.to_string(),
        "tun" => domain::dns::DEFAULT_TUN_SINGBOX_DNS.to_string(),
        _ => domain::dns::DEFAULT_V2RAY_DNS.to_string(),
    }
}

#[cfg(test)]
mod fix08_tests {
    use super::*;

    /// FIX-08 (SET-11): `save_simple_dns` without `global_fake_ip` (the DNS
    /// window has no such control, neither does upstream's
    /// `DNSSettingViewModel`) keeps the stored value instead of clearing it,
    /// and stored unknown keys survive. Upstream basis:
    /// `DNSSettingViewModel.SaveSettingAsync` mutates only its own fields on
    /// the live `_config` object; `ConfigHandler.LoadConfig` defaults
    /// `GlobalFakeIp` to true when null.
    #[test]
    fn save_simple_dns_preserves_global_fake_ip_and_extra_when_unset() {
        let _guard = crate::api::engine::engine_test_lock();
        // Seed an explicit false plus an unknown key through the whole-tree
        // path (the in-memory test engine starts at revision 0).
        let loaded = engine().load_settings().expect("load");
        let mut settings = loaded.settings;
        settings.simple_dns_item.global_fake_ip = Some(false);
        settings
            .simple_dns_item
            .extra
            .insert("FutureDnsFlag".into(), serde_json::json!(7));
        engine()
            .save_settings(settings, loaded.revision)
            .expect("seed");

        let loaded = engine().load_settings().expect("reload");
        // A plain-DNS edit carries no global_fake_ip: it must not wipe the
        // stored false, and the unknown key must survive.
        let draft = SimpleDnsDto {
            direct_dns: Some("119.29.29.29".to_string()),
            ..Default::default()
        };
        assert!(draft.global_fake_ip.is_none());
        let saved = save_simple_dns(draft, loaded.revision);
        assert!(
            saved.ok,
            "unexpected error: {:?}",
            saved.error.map(|e| e.code)
        );
        let reloaded = engine().load_settings().expect("reload");
        assert_eq!(
            reloaded.settings.simple_dns_item.global_fake_ip,
            Some(false)
        );
        assert_eq!(
            reloaded.settings.simple_dns_item.extra.get("FutureDnsFlag"),
            Some(&serde_json::json!(7))
        );
        assert_eq!(
            reloaded.settings.simple_dns_item.direct_dns.as_deref(),
            Some("119.29.29.29")
        );

        // An explicit value still overrides.
        let loaded = engine().load_settings().expect("reload");
        let draft = SimpleDnsDto {
            global_fake_ip: Some(true),
            ..Default::default()
        };
        let saved = save_simple_dns(draft, loaded.revision);
        assert!(saved.ok);
        let reloaded = engine().load_settings().expect("reload");
        assert_eq!(reloaded.settings.simple_dns_item.global_fake_ip, Some(true));
    }
}
