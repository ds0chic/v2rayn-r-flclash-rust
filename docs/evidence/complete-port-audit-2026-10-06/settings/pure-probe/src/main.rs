use std::collections::BTreeMap;
use std::sync::Arc;

use application::codegen::{build_input_full, dns_to_codegen, CodegenOptions};
use application::{AppEngine, NullRuntimeClient};
use config_codegen::{generate_xray, ConfigType};
use domain::{AppSettings, DesiredRevision, Profile};
use serde_json::json;

fn generated(settings: &AppSettings) -> serde_json::Value {
    let profile = Profile {
        index_id: "synthetic-audit".into(),
        config_type: domain::ConfigType::Socks,
        address: "127.0.0.1".into(),
        port: 12345,
        ..Default::default()
    };
    let opts = CodegenOptions { local_port: 11808, ..Default::default() };
    let mut input = build_input_full(
        &profile, &[profile.clone()], None, BTreeMap::new(), None, &opts,
        settings, None,
        Some(dns_to_codegen(None, &settings.simple_dns_item, BTreeMap::new(), vec![])),
        None,
    );
    input.profile.config_type = ConfigType::Socks;
    generate_xray(&input).expect("pure generation").main
}

fn main() {
    let mut settings = AppSettings::default();
    settings.inbound[0].local_port = 11808;
    settings.simple_dns_item.strategy4_freedom = Some("UseIP".into());
    settings.simple_dns_item.enable_happy_eyeballs = Some(false);
    settings.happy_eyeballs4_ray_item.try_delay_ms = Some(20);
    let off = generated(&settings);
    settings.simple_dns_item.enable_happy_eyeballs = Some(true);
    let on = generated(&settings);
    let off_happy = off["outbounds"].as_array().unwrap().iter().any(|outbound| {
        outbound.pointer("/streamSettings/sockopt/happyEyeballs").is_some()
    });
    settings.fragment4_ray_item.as_mut().unwrap().max_split = Some("1-3".into());
    let max_split_error = application::settings::validate_settings(&settings)
        .err().map(|e| json!({"code": e.code, "field": e.field_path}));

    let run_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("synthetic-data-{run_id}"));
    std::fs::create_dir_all(&scratch).unwrap();
    let unavailable = scratch.join("unavailable");
    std::fs::create_dir_all(&unavailable).unwrap();
    let engine = AppEngine::storage_unavailable(
        Some(unavailable.clone()),
        domain::DomainError::new(domain::codes::UNAVAILABLE, "error.synthetic_storage_unavailable"),
    );
    let load_blocked = engine.load_settings().is_err();
    let group_save = engine.save_settings_group("UiItem", json!({"CurrentLanguage": "en"}), 0);

    let canonical = scratch.join("canonical");
    std::fs::create_dir_all(&canonical).unwrap();
    let runtime = Arc::new(NullRuntimeClient::new());
    let engine = AppEngine::open_with_runtime(&canonical, runtime.clone()).unwrap();
    let mut known = AppSettings::default();
    known.inbound[0].local_port = 11808;
    engine.save_settings(known, 0).unwrap();
    engine.save_profile(Profile {
        index_id: "synthetic-active".into(),
        remarks: "Synthetic active".into(),
        config_type: domain::ConfigType::Socks,
        address: "127.0.0.1".into(),
        port: 12345,
        ..Default::default()
    }, DesiredRevision::new(engine.desired_revision())).unwrap();
    drop(engine);
    let config_path = canonical.join("guiNConfig.json");
    let mut config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    config.as_object_mut().unwrap().remove("active_index_id");
    config["IndexId"] = json!("synthetic-active");
    std::fs::write(&config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    let reopened = AppEngine::open_with_runtime(&canonical, runtime.clone()).unwrap();
    let profile_exists = reopened.profile_by_id("synthetic-active").unwrap().is_some();
    let active = reopened.active_profile();
    drop(reopened);

    let empty = scratch.join("empty-config");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(empty.join("guiNConfig.json"), "").unwrap();
    let empty_engine = AppEngine::open_with_runtime(&empty, runtime.clone());
    let empty_load_success = empty_engine.as_ref().map(|e| e.load_settings().is_ok()).unwrap_or(false);

    let malformed = scratch.join("malformed-field");
    std::fs::create_dir_all(&malformed).unwrap();
    std::fs::write(malformed.join("guiNConfig.json"), serde_json::to_vec_pretty(&json!({
        "GuiItem":{"TrayMenuServersLimit":"broken-type","KeepOlderDedupl":true},
        "UiItem":{"CurrentLanguage":"en"},
        "Inbound":[{"LocalPort":11808}]
    })).unwrap()).unwrap();
    let malformed_engine = AppEngine::open_with_runtime(&malformed, runtime).unwrap();
    let loaded = malformed_engine.load_settings().unwrap();

    let roundtrip = scratch.join("backup-active-roundtrip");
    std::fs::create_dir_all(&roundtrip).unwrap();
    let roundtrip_engine = AppEngine::open_with_runtime(&roundtrip, Arc::new(NullRuntimeClient::new())).unwrap();
    let mut roundtrip_settings = AppSettings::default();
    roundtrip_settings.inbound[0].local_port = 11808;
    roundtrip_settings.extra.insert("IndexId".into(), json!("synthetic-a"));
    roundtrip_engine.save_settings(roundtrip_settings, 0).unwrap();
    for (id, remarks) in [("synthetic-a", "A"), ("synthetic-b", "B")] {
        roundtrip_engine.save_profile(Profile {
            index_id: id.into(), remarks: remarks.into(),
            config_type: domain::ConfigType::Socks, address: "127.0.0.1".into(),
            port: 12345, ..Default::default()
        }, DesiredRevision::new(roundtrip_engine.desired_revision())).unwrap();
    }
    roundtrip_engine.set_active(Some("synthetic-b".into())).unwrap();
    let before_backup: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(roundtrip.join("guiNConfig.json")).unwrap()).unwrap();
    let service = application::BackupService::new(&roundtrip);
    let bundle = service.create_local(&scratch.join("backup-bundle"), 1791200000).unwrap();
    let archive = scratch.join("synthetic-backup.zip");
    std::fs::write(&archive, application::zip_upstream_layout(&bundle.root).unwrap()).unwrap();
    let restored = service.import_upstream_with_lifecycle(&roundtrip_engine, &archive, &scratch.join("backup-work"), 1791200010).unwrap();
    let active_after_restore = roundtrip_engine.active_profile();
    let remarks_after_restore = active_after_restore.as_ref()
        .and_then(|id| roundtrip_engine.profile_by_id(id).ok().flatten())
        .map(|p| p.remarks);
    let result = json!({
        "scope":"actual application/domain/config_codegen with synthetic SQLite/config; NullRuntimeClient; no socket/core/OS/network",
        "happy_switch_false_still_emits":off_happy,
        "happy_switch_toggle_identical":off==on,
        "valid_max_split_range_1_3_rejected":max_split_error,
        "unavailable_load_blocked":load_blocked,
        "unavailable_group_save_rejected":group_save.is_err(),
        "unavailable_group_save_created_config":unavailable.join("guiNConfig.json").exists(),
        "canonical_index_profile_exists":profile_exists,
        "canonical_index_active_after_reopen":active,
        "present_empty_config_load_success":empty_load_success,
        "malformed_field_load_success":true,
        "malformed_field_language":loaded.settings.ui_item.current_language,
        "malformed_field_keep_older_dedupl":loaded.settings.gui_item.keep_older_dedupl,
        "malformed_field_local_port":loaded.settings.inbound.first().map(|i| i.local_port),
        "backup_active_before":before_backup["active_index_id"],
        "backup_canonical_index_before":before_backup["IndexId"],
        "backup_restore_status":format!("{:?}",restored.status),
        "backup_active_remarks_after_restore":remarks_after_restore
    });
    let text=serde_json::to_string_pretty(&result).unwrap();
    std::fs::write(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("observations.json"), &text).unwrap();
    println!("{text}");
}
