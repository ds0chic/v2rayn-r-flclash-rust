use std::collections::BTreeMap;

use application::codegen::{build_input_full, dns_to_codegen, CodegenOptions};
use config_codegen::{generate_xray, ConfigType};
use domain::{AppSettings, Profile};
use serde_json::json;

fn generated(settings: &AppSettings) -> serde_json::Value {
    let profile = Profile {
        index_id: "synthetic-audit".into(),
        config_type: domain::ConfigType::Socks,
        address: "127.0.0.1".into(),
        port: 12345,
        ..Default::default()
    };
    let mut input = build_input_full(
        &profile,
        &[profile.clone()],
        None,
        BTreeMap::new(),
        None,
        &CodegenOptions::default(),
        settings,
        None,
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

    let mut tun = AppSettings::default().tun_mode_item;
    tun.enable_tun = true;
    tun.auto_route = false;
    tun.route_exclude_address = Some("10.0.0.0/8,".split(',').map(str::to_string).collect());
    let hints = application::tun_plan::TunPlanHints {
        interface_index: 42,
        ..Default::default()
    };
    let empty_exclude_error = application::tun_plan::tun_spec_from_settings(&tun, &hints)
        .err().map(|e| json!({"code": e.code, "detail": e.detail}));

    let mut ipv6_settings = AppSettings::default();
    ipv6_settings.inbound[0].local_port = 11808;
    ipv6_settings.tun_mode_item.enable_tun = true;
    ipv6_settings.tun_mode_item.enable_ipv6_address = true;
    ipv6_settings.tun_mode_item.ipv6_address = Some("fd00::1/64".into());
    let projected = application::codegen::settings_from_app(&ipv6_settings, &CodegenOptions::default());
    let ipv6_config = generated(&ipv6_settings);
    let tun_inbound = ipv6_config["inbounds"].as_array().unwrap().iter()
        .find(|i| i["protocol"] == "tun").unwrap();

    let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("synthetic-storage-unavailable");
    std::fs::create_dir_all(&scratch).unwrap();
    let synthetic_error = domain::DomainError::new(domain::codes::UNAVAILABLE, "error.synthetic_storage_unavailable");
    let engine = application::AppEngine::storage_unavailable(Some(scratch.clone()), synthetic_error);
    let load_blocked = engine.load_settings().is_err();
    let group_save = engine.save_settings_group("UiItem", json!({"CurrentLanguage": "en"}), 0);
    let group_save_ok = group_save.is_ok();
    let group_save_wrote_file = scratch.join("guiNConfig.json").exists();

    assert!(off_happy, "observed bug disappeared: revise audit");
    assert_eq!(off, on, "observed ignored switch disappeared: revise audit");
    assert!(max_split_error.is_some(), "observed range rejection disappeared: revise audit");
    assert!(empty_exclude_error.is_some(), "observed empty exclusion rejection disappeared: revise audit");
    assert!(load_blocked && group_save_ok && group_save_wrote_file,
        "observed missing storage guard disappeared: revise audit");

    println!("{}", serde_json::to_string_pretty(&json!({
        "scope": "pure settings validation/config generation plus dedicated synthetic scratch config writes; no socket, core, OS platform or helper invocation",
        "happy_eyeballs_switch_false_still_emits_object": off_happy,
        "happy_eyeballs_switch_toggle_config_identical": off == on,
        "upstream_valid_max_split_range_1_3_rejected": max_split_error,
        "trailing_comma_exclusion_rejected_after_ui_split": empty_exclude_error,
        "desired_ipv6_enabled_generated_tun_routes": tun_inbound["settings"]["autoSystemRoutingTable"],
        "projected_global_ipv6_context": projected.has_global_ipv6_address,
        "projected_protected_core_count": projected.protect_core_executables.len(),
        "unavailable_storage_load_blocked": load_blocked,
        "unavailable_storage_group_save_returned_ok": group_save_ok,
        "unavailable_storage_group_save_created_gui_config": group_save_wrote_file
    })).unwrap());
}
