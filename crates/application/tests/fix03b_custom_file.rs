//! FIX-03B: browse step for custom/outbound nodes — copy the selected config
//! file into `<data>/config/` and make codegen read it back (RT-08).

use std::path::PathBuf;

use application::AppEngine;
use domain::{ConfigType, CoreType, DesiredRevision, Profile};

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("v2rayn_r_{tag}_{nanos:x}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn import_custom_file_copies_and_codegen_reads_it() {
    let dir = temp_dir("fix03b_custom");
    let engine = AppEngine::open(&dir).expect("open engine");

    let source = dir.join("picked-config.json");
    std::fs::write(&source, r#"{"outbounds":[{"protocol":"freedom"}]}"#)
        .expect("write source file");

    let stored = engine.import_custom_file(&source).expect("copy file");
    assert!(stored.ends_with(".json"), "keeps extension: {stored}");
    assert!(
        dir.join("config").join(&stored).is_file(),
        "copied under <data>/config"
    );

    let profile = Profile {
        index_id: "fix03b-custom".into(),
        config_type: ConfigType::Custom,
        core_type: Some(CoreType::Xray),
        remarks: "合成自定义".into(),
        address: stored.clone(),
        ..Default::default()
    };
    let revision = engine.desired_revision();
    engine
        .save_profile(profile, DesiredRevision::new(revision))
        .expect("save custom profile");

    let input = engine
        .build_codegen_input(
            "fix03b-custom",
            CoreType::Xray,
            &application::codegen::CodegenOptions::default(),
        )
        .expect("build codegen input");
    let text = input
        .custom_outbound_content
        .get("fix03b-custom")
        .expect("file-backed custom content");
    assert!(text.contains("freedom"));

    let missing = engine.import_custom_file(&dir.join("nope.json"));
    assert!(missing.is_err(), "missing source must fail");

    std::fs::remove_dir_all(&dir).ok();
}
