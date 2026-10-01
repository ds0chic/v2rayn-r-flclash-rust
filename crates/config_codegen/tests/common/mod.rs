//! Shared helpers for the T07/T08 codegen tests.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use config_codegen::input::*;
use serde_json::Value;

/// Builds a profile with test-safe defaults. Ports used in tests must stay
/// >= 11808 (the machine's live proxy on 10808 must never be referenced).
pub fn profile(config_type: ConfigType, address: &str, port: i32) -> CodegenProfile {
    CodegenProfile {
        index_id: "node-1".into(),
        config_type,
        remarks: "node".into(),
        address: address.into(),
        port,
        ..Default::default()
    }
}

pub fn settings() -> CodegenSettings {
    CodegenSettings {
        core_basic: CoreBasic {
            loglevel: "warning".into(),
            ..Default::default()
        },
        inbound: InboundSettings {
            local_port: 11808,
            ..Default::default()
        },
        state_port: 11809,
        state_port2: 11810,
        log_directory: "logs".into(),
        bin_directory: "bin".into(),
        log_date: "2026-10-01".into(),
        speed_ping_test_url: Some("https://example.com/ping".into()),
        ..Default::default()
    }
}

pub fn codegen_input(profile: CodegenProfile) -> CodegenInput {
    let mut profiles = BTreeMap::new();
    profiles.insert(profile.index_id.clone(), profile.clone());
    CodegenInput {
        profile,
        profiles,
        settings: settings(),
        ..Default::default()
    }
}

pub fn json_at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

pub fn string_at(value: &Value, pointer: &str) -> String {
    json_at(value, pointer).as_str().unwrap_or("").to_string()
}

pub fn sample_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/source/upstream/sample")
        .canonicalize()
        .expect("sample fixture directory")
}

pub fn read_sample(name: &str) -> String {
    std::fs::read_to_string(sample_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

/// The fixtures directory itself must never be written by tests.
pub fn assert_no_live_port(value: &Value) {
    let text = value.to_string();
    assert!(
        !text.contains("10808"),
        "generated config must not reference port 10808: {text}"
    );
}
