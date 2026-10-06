//! T10 FRB functions: full-config templates plus group/chain helpers.
//!
//! The editor dialogs (`AddGroupServerWindow`, `AddServer2Window`,
//! `FullConfigTemplateWindow`) drive these; node persistence itself keeps
//! going through `save_profile`.

use domain::{CoreType, DomainError, FullConfigTemplate};
use flutter_rust_bridge::frb;

use crate::api::contract::{ErrorDto, ProfileDto, ProfilePageDto, SaveProfileResult, SimpleResult};
use crate::api::engine::{engine, error_dto};

/// The 8-field `FullConfigTemplateItem` row.
#[derive(Clone)]
pub struct FullConfigTemplateDto {
    pub id: String,
    pub remarks: String,
    pub enabled: bool,
    pub core_type: CoreType,
    pub config: Option<String>,
    pub tun_config: Option<String>,
    pub add_proxy_only: Option<bool>,
    pub proxy_detour: Option<String>,
}

/// Result of `list_templates`.
#[derive(Clone)]
pub struct TemplatesPageDto {
    pub items: Vec<FullConfigTemplateDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `save_template`.
#[derive(Clone)]
pub struct TemplateDtoResult {
    pub ok: bool,
    pub item: Option<FullConfigTemplateDto>,
    pub error: Option<ErrorDto>,
}

/// Result of `gen_group_region`: one saved profile per matching region.
#[derive(Clone)]
pub struct GroupGenResult {
    pub ok: bool,
    pub profiles: Vec<ProfileDto>,
    pub error: Option<ErrorDto>,
}

fn template_to_dto(t: FullConfigTemplate) -> FullConfigTemplateDto {
    FullConfigTemplateDto {
        id: t.id,
        remarks: t.remarks,
        enabled: t.enabled,
        core_type: t.core_type,
        config: t.config,
        tun_config: t.tun_config,
        add_proxy_only: t.add_proxy_only,
        proxy_detour: t.proxy_detour,
    }
}

fn dto_to_template(d: FullConfigTemplateDto) -> FullConfigTemplate {
    FullConfigTemplate {
        id: d.id,
        remarks: d.remarks,
        enabled: d.enabled,
        core_type: d.core_type,
        config: d.config.filter(|text| !text.trim().is_empty()),
        tun_config: d.tun_config.filter(|text| !text.trim().is_empty()),
        add_proxy_only: d.add_proxy_only,
        proxy_detour: d.proxy_detour.filter(|text| !text.trim().is_empty()),
        extra: Default::default(),
    }
}

/// `list_templates` — the stored rows (builtins seeded on first open).
#[frb(sync)]
pub fn list_templates() -> TemplatesPageDto {
    match engine().list_templates() {
        Ok(items) => TemplatesPageDto {
            items: items.into_iter().map(template_to_dto).collect(),
            error: None,
        },
        Err(e) => TemplatesPageDto {
            items: Vec::new(),
            error: Some(error_dto(e)),
        },
    }
}

/// `get_template` — the row for one core, if any.
#[frb(sync)]
pub fn get_template(core: CoreType) -> Option<FullConfigTemplateDto> {
    engine()
        .get_template_for_core(core)
        .ok()
        .flatten()
        .map(template_to_dto)
}

/// `save_template` — validate (JSON objects), upsert by core, persist.
#[frb(sync)]
pub fn save_template(item: FullConfigTemplateDto) -> TemplateDtoResult {
    match engine().save_template(dto_to_template(item)) {
        Ok(saved) => TemplateDtoResult {
            ok: true,
            item: Some(template_to_dto(saved)),
            error: None,
        },
        Err(e) => TemplateDtoResult {
            ok: false,
            item: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `delete_template` — remove one row by id.
#[frb(sync)]
pub fn delete_template(id: String) -> SimpleResult {
    match engine().delete_template(&id) {
        Ok(removed) => SimpleResult {
            ok: removed,
            error: if removed {
                None
            } else {
                Some(error_dto(DomainError::not_found("template", &id)))
            },
        },
        Err(e) => SimpleResult {
            ok: false,
            error: Some(error_dto(e)),
        },
    }
}

/// `group_children` — ordered child profiles of a group/chain node
/// (subscription matches first, then the explicit `ChildItems` order).
#[frb(sync)]
pub fn group_children(index_id: String) -> ProfilePageDto {
    let dataset_revision = engine().desired_revision();
    match engine().group_children(&index_id) {
        Ok(children) => {
            let total = children.len() as u64;
            ProfilePageDto {
                items: children
                    .into_iter()
                    .map(crate::api::engine::profile_dto)
                    .collect(),
                total,
                next_cursor: None,
                dataset_revision,
                request_generation: 0,
            }
        }
        Err(_) => ProfilePageDto {
            items: Vec::new(),
            total: 0,
            next_cursor: None,
            dataset_revision,
            request_generation: 0,
        },
    }
}

/// `gen_group_all` — create the "all nodes of this subscription" policy group.
#[frb(sync)]
pub fn gen_group_all(sub_id: String) -> SaveProfileResult {
    match engine().gen_group_all(&sub_id) {
        Ok(saved) => SaveProfileResult {
            ok: true,
            profile: Some(crate::api::engine::profile_dto(saved)),
            new_revision: Some(engine().desired_revision()),
            error: None,
        },
        Err(e) => SaveProfileResult {
            ok: false,
            profile: None,
            new_revision: None,
            error: Some(error_dto(e)),
        },
    }
}

/// `gen_group_region` — create one policy group per region with matches.
#[frb(sync)]
pub fn gen_group_region(sub_id: String) -> GroupGenResult {
    match engine().gen_group_region(&sub_id) {
        Ok(profiles) => GroupGenResult {
            ok: true,
            profiles: profiles
                .into_iter()
                .map(crate::api::engine::profile_dto)
                .collect(),
            error: None,
        },
        Err(e) => GroupGenResult {
            ok: false,
            profiles: Vec::new(),
            error: Some(error_dto(e)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_round_trip_through_bridge() {
        let _guard = crate::api::engine::engine_test_lock();
        let before = list_templates();
        assert!(before.error.is_none());
        assert!(before.items.iter().any(|t| t.core_type == CoreType::Xray));

        let mut xray = get_template(CoreType::Xray).expect("xray template");
        xray.enabled = true;
        xray.config = Some(r#"{"log": {}}"#.into());
        xray.add_proxy_only = Some(true);
        let saved = save_template(xray);
        assert!(saved.ok, "{:?}", saved.error.map(|e| e.code));
        assert_eq!(
            saved.item.unwrap().config.as_deref(),
            Some(r#"{"log": {}}"#)
        );

        let mut bad = get_template(CoreType::SingBox).expect("sbox template");
        bad.config = Some("{not json".into());
        assert!(!save_template(bad).ok);
    }

    #[test]
    fn group_children_empty_for_unknown_group() {
        let _guard = crate::api::engine::engine_test_lock();
        let page = group_children("no-such-group".into());
        assert_eq!(page.total, 0);
        assert!(page.items.is_empty());
    }
}
