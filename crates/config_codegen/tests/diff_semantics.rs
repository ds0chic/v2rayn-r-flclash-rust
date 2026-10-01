mod common;

use config_codegen::diff::{normalize, normalized_diff, semantic_diff, DiffKind, Normalizer};
use serde_json::json;

#[test]
fn identical_documents_are_semantically_equal() {
    let a = json!({"a": 1, "b": [1, 2, 3], "c": {"d": true}});
    let b = json!({"a": 1, "b": [1, 2, 3], "c": {"d": true}});
    assert!(semantic_diff(&a, &b).is_empty());
}

#[test]
fn object_key_order_is_irrelevant() {
    let a = json!({"a": 1, "b": 2});
    let b = json!({"b": 2, "a": 1});
    assert!(semantic_diff(&a, &b).is_empty());
}

#[test]
fn array_order_is_preserved() {
    let a = json!({"rules": [1, 2, 3]});
    let b = json!({"rules": [3, 2, 1]});
    let diffs = semantic_diff(&a, &b);
    assert!(!diffs.is_empty());
    assert!(diffs
        .iter()
        .any(|d| d.path == "$.rules[0]" && d.kind == DiffKind::ValueMismatch));
}

#[test]
fn missing_and_unexpected_members_are_reported() {
    let a = json!({"a": 1, "b": 2});
    let b = json!({"a": 1, "c": 3});
    let diffs = semantic_diff(&a, &b);
    assert!(diffs
        .iter()
        .any(|d| d.path == "$.b" && d.kind == DiffKind::MissingInActual));
    assert!(diffs
        .iter()
        .any(|d| d.path == "$.c" && d.kind == DiffKind::UnexpectedInActual));
}

#[test]
fn array_length_difference_is_reported() {
    let a = json!({"rules": [1, 2]});
    let b = json!({"rules": [1]});
    let diffs = semantic_diff(&a, &b);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].kind, DiffKind::ArrayLength);
}

#[test]
fn normalizer_replaces_paths_and_keys() {
    let a = json!({"log": {"access": "C:/tmp/run1/Vaccess.log"}, "port": 11808});
    let b = json!({"log": {"access": "C:/tmp/run2/Vaccess.log"}, "port": 22999});
    let normalizer = Normalizer::new().path_prefix("C:/tmp/").ignore_key("port");
    assert!(normalized_diff(&a, &b, &normalizer).is_empty());

    let normalized = normalize(&a, &normalizer);
    assert_eq!(normalized["log"]["access"], json!("<path>"));
    assert_eq!(normalized["port"], json!("<normalized>"));
}

#[test]
fn normalizer_keeps_semantic_differences() {
    let a = json!({"log": {"access": "C:/tmp/run1/Vaccess.log"}});
    let b = json!({"log": {"access": "C:/other/Verror.log"}});
    let normalizer = Normalizer::new().path_prefix("C:/tmp/run1/");
    let diffs = normalized_diff(&a, &b, &normalizer);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].path, "$.log.access");
}

#[test]
fn exact_value_replacements_apply() {
    let a = json!({"tag": "node-1-proxy-A"});
    let b = json!({"tag": "node-2-proxy-B"});
    let normalizer = Normalizer::new()
        .replace("node-1-proxy-A", "<tag>")
        .replace("node-2-proxy-B", "<tag>");
    assert!(normalized_diff(&a, &b, &normalizer).is_empty());
}
