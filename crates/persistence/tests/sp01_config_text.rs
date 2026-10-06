//! SP-01 red contract (persistence): `guiNConfig.json` text classification.
//!
//! The engine open path must tell these apart before any typed parse runs:
//! present-but-empty and truncated inputs are `Corrupt`, never silent
//! defaults; unknown keys are preserved by the document model.

use persistence::upstream_config::{parse_config_text, ConfigDocument};

#[test]
fn empty_text_is_corrupt() {
    let err = parse_config_text("").expect_err("empty must be corrupt");
    assert_eq!(err.code(), persistence::error::codes::CORRUPT);
}

#[test]
fn whitespace_only_is_corrupt() {
    let err = parse_config_text("  \n ").expect_err("blank must be corrupt");
    assert_eq!(err.code(), persistence::error::codes::CORRUPT);
}

#[test]
fn truncated_json_is_corrupt() {
    let err = parse_config_text(r#"{"GuiItem": {"TrayMenuServersLimit": "#)
        .expect_err("truncated must be corrupt");
    assert_eq!(err.code(), persistence::error::codes::CORRUPT);
}

#[test]
fn valid_value_with_unknown_keys_parses() {
    let value =
        parse_config_text(r#"{"UiItem": {"CurrentLanguage": "en"}, "FutureRoot": {"x": 1}}"#)
            .expect("valid JSON must parse");
    assert_eq!(
        value
            .get("UiItem")
            .and_then(|ui| ui.get("CurrentLanguage"))
            .and_then(|v| v.as_str()),
        Some("en")
    );
    assert!(value.get("FutureRoot").is_some());
}

#[test]
fn document_model_keeps_unknown_keys_and_field_states() {
    let doc = ConfigDocument::parse(
        r#"{"A": null, "B": "", "GuiItem": {"TrayMenuServersLimit": "oops"}}"#,
    )
    .unwrap();
    assert!(doc.field_state(&["A"]).is_null());
    assert_eq!(doc.field_state(&["B"]).as_str(), Some(""));
    assert!(doc.field_state(&["Z"]).is_missing());
}
