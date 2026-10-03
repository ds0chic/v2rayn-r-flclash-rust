//! T11 FRB functions: routing profiles, rules, import/export and rule mode.
//!
//! The routing windows (`RoutingSettingWindow`, `RoutingRuleSettingWindow`,
//! `RoutingRuleDetailsWindow`) drive these; generation consumes the stored
//! rows through `AppEngine::build_codegen_input`.

use domain::{DomainError, RuleMode, RuleType};
use flutter_rust_bridge::frb;

use crate::api::contract::{ErrorDto, SimpleResult};
use crate::api::engine::{engine, error_dto};

/// One routing rule (`RulesItem`, 13 properties).
#[derive(Clone, Default)]
pub struct RoutingRuleDto {
    pub id: String,
    pub rule_kind: Option<String>,
    pub port: Option<String>,
    pub network: Option<String>,
    pub inbound_tag: Vec<String>,
    pub has_inbound_tag: bool,
    pub outbound_tag: Option<String>,
    pub ip: Vec<String>,
    pub has_ip: bool,
    pub domain: Vec<String>,
    pub has_domain: bool,
    pub protocol: Vec<String>,
    pub has_protocol: bool,
    pub process: Vec<String>,
    pub has_process: bool,
    pub enabled: bool,
    pub remarks: Option<String>,
    pub rule_type: Option<i32>,
}

/// One routing profile (`RoutingItem`, 13 properties).
#[derive(Clone, Default)]
pub struct RoutingProfileDto {
    pub id: String,
    pub remarks: String,
    pub url: String,
    pub rule_set: String,
    pub rule_num: i32,
    pub enabled: bool,
    pub locked: bool,
    pub custom_icon: String,
    pub custom_ruleset_path4_singbox: String,
    pub domain_strategy: String,
    pub domain_strategy4_singbox: String,
    pub sort: i32,
    pub is_active: bool,
}

/// Result of `list_routings`.
#[derive(Clone)]
pub struct RoutingsPageDto {
    pub items: Vec<RoutingProfileDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `save_routing` / `get_routing`.
#[derive(Clone)]
pub struct RoutingDtoResult {
    pub ok: bool,
    pub item: Option<RoutingProfileDto>,
    pub error: Option<ErrorDto>,
}

/// Result of a routing-rules query (parsed rule list + warnings).
#[derive(Clone)]
pub struct RoutingRulesPageDto {
    pub ok: bool,
    pub rules: Vec<RoutingRuleDto>,
    pub warnings: Vec<RoutingWarningDto>,
    pub error: Option<ErrorDto>,
}

/// One structured outbound-tag warning.
#[derive(Clone)]
pub struct RoutingWarningDto {
    pub code: String,
    pub message: String,
    pub field_path: Option<String>,
}

/// Result of a rules text operation (import/export/move).
#[derive(Clone)]
pub struct RoutingRulesTextResult {
    pub ok: bool,
    pub text: String,
    pub rule_count: u32,
    pub error: Option<ErrorDto>,
}

/// Current routing mode (`ERuleMode`).
#[derive(Clone)]
pub struct RuleModeResult {
    pub mode: String,
    pub error: Option<ErrorDto>,
}

fn rule_to_dto(r: domain::RoutingRule) -> RoutingRuleDto {
    RoutingRuleDto {
        id: r.id,
        rule_kind: r.rule_kind,
        port: r.port,
        network: r.network,
        has_inbound_tag: r.inbound_tag.is_some(),
        inbound_tag: r.inbound_tag.unwrap_or_default(),
        outbound_tag: r.outbound_tag,
        has_ip: r.ip.is_some(),
        ip: r.ip.unwrap_or_default(),
        has_domain: r.domain.is_some(),
        domain: r.domain.unwrap_or_default(),
        has_protocol: r.protocol.is_some(),
        protocol: r.protocol.unwrap_or_default(),
        has_process: r.process.is_some(),
        process: r.process.unwrap_or_default(),
        enabled: r.enabled,
        remarks: r.remarks,
        rule_type: r.rule_type.map(|t| t.value()),
    }
}

fn dto_to_rule(d: RoutingRuleDto) -> domain::RoutingRule {
    domain::RoutingRule {
        id: d.id,
        rule_kind: d.rule_kind.filter(|s| !s.trim().is_empty()),
        port: d.port.filter(|s| !s.trim().is_empty()),
        network: d.network.filter(|s| !s.trim().is_empty()),
        inbound_tag: if d.has_inbound_tag {
            Some(d.inbound_tag)
        } else {
            None
        },
        outbound_tag: d.outbound_tag.filter(|s| !s.trim().is_empty()),
        ip: if d.has_ip { Some(d.ip) } else { None },
        domain: if d.has_domain { Some(d.domain) } else { None },
        protocol: if d.has_protocol {
            Some(d.protocol)
        } else {
            None
        },
        process: if d.has_process { Some(d.process) } else { None },
        enabled: d.enabled,
        remarks: d.remarks.filter(|s| !s.trim().is_empty()),
        rule_type: d.rule_type.and_then(RuleType::from_value),
        extra: Default::default(),
    }
}

fn profile_to_dto(p: domain::RoutingProfile) -> RoutingProfileDto {
    RoutingProfileDto {
        id: p.id,
        remarks: p.remarks,
        url: p.url,
        rule_set: p.rule_set,
        rule_num: p.rule_num,
        enabled: p.enabled,
        locked: p.locked,
        custom_icon: p.custom_icon,
        custom_ruleset_path4_singbox: p.custom_ruleset_path4_singbox,
        domain_strategy: p.domain_strategy,
        domain_strategy4_singbox: p.domain_strategy4_singbox,
        sort: p.sort,
        is_active: p.is_active,
    }
}

fn dto_to_profile(d: RoutingProfileDto) -> domain::RoutingProfile {
    domain::RoutingProfile {
        id: d.id,
        remarks: d.remarks,
        url: d.url,
        rule_set: d.rule_set,
        rule_num: d.rule_num,
        enabled: d.enabled,
        locked: d.locked,
        custom_icon: d.custom_icon,
        custom_ruleset_path4_singbox: d.custom_ruleset_path4_singbox,
        domain_strategy: d.domain_strategy,
        domain_strategy4_singbox: d.domain_strategy4_singbox,
        sort: d.sort,
        is_active: d.is_active,
        extra: Default::default(),
    }
}

fn err_dto(e: DomainError) -> ErrorDto {
    error_dto(e)
}

/// `list_routings` — all routing profiles in `Sort` order.
#[frb(sync)]
pub fn list_routings() -> RoutingsPageDto {
    match engine().list_routings() {
        Ok(items) => RoutingsPageDto {
            items: items.into_iter().map(profile_to_dto).collect(),
            error: None,
        },
        Err(e) => RoutingsPageDto {
            items: Vec::new(),
            error: Some(err_dto(e)),
        },
    }
}

/// `get_routing` — one routing profile by id.
#[frb(sync)]
pub fn get_routing(id: String) -> RoutingDtoResult {
    match engine().get_routing(&id) {
        Ok(Some(item)) => RoutingDtoResult {
            ok: true,
            item: Some(profile_to_dto(item)),
            error: None,
        },
        Ok(None) => RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(DomainError::not_found("routing", &id))),
        },
        Err(e) => RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `save_routing` — insert or replace one routing profile.
#[frb(sync)]
pub fn save_routing(draft: RoutingProfileDto) -> RoutingDtoResult {
    match engine().save_routing(dto_to_profile(draft)) {
        Ok(saved) => RoutingDtoResult {
            ok: true,
            item: Some(profile_to_dto(saved)),
            error: None,
        },
        Err(e) => RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `delete_routing` — remove one routing profile.
#[frb(sync)]
pub fn delete_routing(id: String) -> SimpleResult {
    match engine().delete_routing(&id) {
        Ok(true) => SimpleResult {
            ok: true,
            error: None,
        },
        Ok(false) => SimpleResult {
            ok: false,
            error: Some(err_dto(DomainError::not_found("routing", &id))),
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(err_dto(e)),
        },
    }
}

/// `set_default_routing` — mark one routing profile active.
#[frb(sync)]
pub fn set_default_routing(id: String) -> SimpleResult {
    match engine().set_default_routing(&id) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(err_dto(e)),
        },
    }
}

/// `list_routing_rules` — parsed rules of a profile plus warnings.
#[frb(sync)]
pub fn list_routing_rules(routing_id: String) -> RoutingRulesPageDto {
    let rules = match engine().get_routing(&routing_id) {
        Ok(Some(profile)) => match domain::routing::parse_rules(&profile) {
            Ok(rules) => rules,
            Err(e) => {
                return RoutingRulesPageDto {
                    ok: false,
                    rules: Vec::new(),
                    warnings: Vec::new(),
                    error: Some(err_dto(e)),
                };
            }
        },
        Ok(None) => {
            return RoutingRulesPageDto {
                ok: false,
                rules: Vec::new(),
                warnings: Vec::new(),
                error: Some(err_dto(DomainError::not_found("routing", &routing_id))),
            };
        }
        Err(e) => {
            return RoutingRulesPageDto {
                ok: false,
                rules: Vec::new(),
                warnings: Vec::new(),
                error: Some(err_dto(e)),
            };
        }
    };
    let warnings = engine()
        .routing_warnings(&routing_id)
        .unwrap_or_default()
        .into_iter()
        .map(|w| RoutingWarningDto {
            code: w.code,
            message: w.message,
            field_path: w.field_path,
        })
        .collect();
    RoutingRulesPageDto {
        ok: true,
        rules: rules.into_iter().map(rule_to_dto).collect(),
        warnings,
        error: None,
    }
}

/// `save_routing_rules` — replace a profile's whole rule list.
#[frb(sync)]
pub fn save_routing_rules(routing_id: String, rules: Vec<RoutingRuleDto>) -> RoutingDtoResult {
    let mut profile = match engine().get_routing(&routing_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return RoutingDtoResult {
                ok: false,
                item: None,
                error: Some(err_dto(DomainError::not_found("routing", &routing_id))),
            };
        }
        Err(e) => {
            return RoutingDtoResult {
                ok: false,
                item: None,
                error: Some(err_dto(e)),
            };
        }
    };
    let rules: Vec<domain::RoutingRule> = rules.into_iter().map(dto_to_rule).collect();
    if domain::routing::set_rules(&mut profile, &rules).is_err() {
        return RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(DomainError::new(
                domain::codes::INTERNAL,
                "error.routing_rules_serialize",
            ))),
        };
    }
    match engine().save_routing(profile) {
        Ok(saved) => RoutingDtoResult {
            ok: true,
            item: Some(profile_to_dto(saved)),
            error: None,
        },
        Err(e) => RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `move_routing_rule` — Top(0)/Up(1)/Down(2)/Bottom(3) one rule.
#[frb(sync)]
pub fn move_routing_rule(routing_id: String, index: u32, direction: i32) -> RoutingDtoResult {
    let Some(direction) = domain::routing::MoveDirection::from_int(direction) else {
        return RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(
                DomainError::new(domain::codes::FIELD_RANGE, "error.routing_move_invalid")
                    .with_field("direction"),
            )),
        };
    };
    match engine().move_routing_rule(&routing_id, index as usize, direction) {
        Ok(saved) => RoutingDtoResult {
            ok: true,
            item: Some(profile_to_dto(saved)),
            error: None,
        },
        Err(e) => RoutingDtoResult {
            ok: false,
            item: None,
            error: Some(err_dto(e)),
        },
    }
}

/// `import_routing_rules` — parse text and append/replace on the profile.
#[frb(sync)]
pub fn import_routing_rules(
    routing_id: String,
    text: String,
    replace: bool,
) -> RoutingRulesTextResult {
    match engine().import_routing_rules(&routing_id, &text, replace) {
        Ok(saved) => RoutingRulesTextResult {
            ok: true,
            text: String::new(),
            rule_count: saved.rule_num as u32,
            error: None,
        },
        Err(e) => RoutingRulesTextResult {
            ok: false,
            text: String::new(),
            rule_count: 0,
            error: Some(err_dto(e)),
        },
    }
}

/// `export_routing_rules` — all rules (or the `ids` selection) as JSON.
#[frb(sync)]
pub fn export_routing_rules(routing_id: String, ids: Vec<String>) -> RoutingRulesTextResult {
    let profile = match engine().get_routing(&routing_id) {
        Ok(Some(profile)) => profile,
        Ok(None) => return export_error(DomainError::not_found("routing", &routing_id)),
        Err(e) => return export_error(e),
    };
    let rules = match domain::routing::parse_rules(&profile) {
        Ok(rules) => rules,
        Err(e) => return export_error(e),
    };
    let selected: Vec<domain::RoutingRule> = if ids.is_empty() {
        rules
    } else {
        rules.into_iter().filter(|r| ids.contains(&r.id)).collect()
    };
    if selected.is_empty() {
        return export_error(
            DomainError::new(domain::codes::NOT_FOUND, "error.routing_no_selection")
                .with_field("ids"),
        );
    }
    match application::routing::export_rules_camel(&selected) {
        Ok(text) => RoutingRulesTextResult {
            ok: true,
            rule_count: selected.len() as u32,
            text,
            error: None,
        },
        Err(e) => export_error(e),
    }
}

fn export_error(error: DomainError) -> RoutingRulesTextResult {
    RoutingRulesTextResult {
        ok: false,
        text: String::new(),
        rule_count: 0,
        error: Some(err_dto(error)),
    }
}

/// `get_rule_mode` — current `ERuleMode` name.
#[frb(sync)]
pub fn get_rule_mode() -> RuleModeResult {
    let mode = match engine().rule_mode() {
        RuleMode::Global => "Global",
        RuleMode::Direct => "Direct",
        _ => "Rule",
    };
    RuleModeResult {
        mode: mode.to_string(),
        error: None,
    }
}

/// `set_rule_mode` — switch `Rule` / `Global` / `Direct`.
#[frb(sync)]
pub fn set_rule_mode(mode: String) -> SimpleResult {
    let mode = match mode.as_str() {
        "Global" => RuleMode::Global,
        "Direct" => RuleMode::Direct,
        "Rule" => RuleMode::Rule,
        _ => {
            return SimpleResult {
                ok: false,
                error: Some(err_dto(
                    DomainError::new(domain::codes::FIELD_RANGE, "error.rule_mode_invalid")
                        .with_field("mode"),
                )),
            };
        }
    };
    match engine().set_rule_mode(mode) {
        Ok(()) => SimpleResult {
            ok: true,
            error: None,
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(err_dto(e)),
        },
    }
}
