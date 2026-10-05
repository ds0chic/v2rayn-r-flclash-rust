// R4-20: group/chain topology generation-must-not-hang contract.
//
// A cycle in stored group data (a save-time `validate_group` failure normally
// prevents it, but an import/repair or corrupted store can still deliver one)
// must surface a readable `invalid_reference` error instead of recursing until
// the stack overflows. A nested group without a cycle must still generate.
//
// Synthetic data only; ports unused (addresses are TEST-NET).
mod common;

use common::*;
use config_codegen::generate_singbox;
use config_codegen::generate_xray;
use config_codegen::input::*;

fn group(id: &str, children: &str) -> CodegenProfile {
    let mut p = profile(ConfigType::PolicyGroup, "", 0);
    p.index_id = id.into();
    p.remarks = id.into();
    p.proto_extra.child_items = Some(children.into());
    p
}

fn leaf(id: &str, address: &str) -> CodegenProfile {
    let mut p = profile(ConfigType::Vless, address, 443);
    p.index_id = id.into();
    p.remarks = id.into();
    p.password = "11111111-1111-1111-1111-111111111111".into();
    p
}

#[test]
fn xray_group_cycle_reports_readable_error() {
    let a = group("a", "b");
    let b = group("b", "a");
    let mut input = codegen_input(a.clone());
    input.profiles.insert("b".into(), b);
    let err = generate_xray(&input).unwrap_err();
    assert_eq!(err.code, "invalid_reference");
    assert!(err.message.contains("cycle"), "{}", err.message);
}

#[test]
fn singbox_group_cycle_reports_readable_error() {
    let a = group("a", "b");
    let b = group("b", "a");
    let mut input = codegen_input(a.clone());
    input.profiles.insert("b".into(), b);
    let err = generate_singbox(&input).unwrap_err();
    assert_eq!(err.code, "invalid_reference");
    assert!(err.message.contains("cycle"), "{}", err.message);
}

#[test]
fn nested_group_without_cycle_still_generates() {
    let a = group("a", "b");
    let b = group("b", "leaf");
    let mut input = codegen_input(a.clone());
    input.profiles.insert("b".into(), b);
    input
        .profiles
        .insert("leaf".into(), leaf("leaf", "192.0.2.60"));
    let generated = generate_xray(&input).expect("nested group");
    assert!(!generated.main["outbounds"].as_array().unwrap().is_empty());
}
