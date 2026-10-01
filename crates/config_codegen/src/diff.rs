//! Semantic diff helpers for differential testing (plan §12).
//!
//! Comparisons keep array order and only neutralise explicitly declared
//! non-semantic items (random ports, temporary paths, generated ids).

use std::collections::BTreeSet;

use serde_json::{Map, Value};

/// Normalization rules for non-semantic values.
#[derive(Debug, Clone, Default)]
pub struct Normalizer {
    /// Object keys whose value is replaced by the `<normalized>` placeholder.
    pub ignore_keys: BTreeSet<String>,
    /// String values starting with one of these prefixes are replaced by
    /// `<path>`. Useful for temporary or machine specific paths.
    pub path_prefixes: Vec<String>,
    /// Exact string replacements applied after the prefix rules.
    pub value_replacements: Vec<(String, String)>,
}

impl Normalizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ignore_key(mut self, key: &str) -> Self {
        self.ignore_keys.insert(key.to_string());
        self
    }

    pub fn path_prefix(mut self, prefix: &str) -> Self {
        self.path_prefixes.push(prefix.to_string());
        self
    }

    pub fn replace(mut self, from: &str, to: &str) -> Self {
        self.value_replacements
            .push((from.to_string(), to.to_string()));
        self
    }
}

const NORMALIZED: &str = "<normalized>";
const PATH: &str = "<path>";

/// Normalize a value according to the rules, preserving array order.
pub fn normalize(value: &Value, normalizer: &Normalizer) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, value) in map {
                if normalizer.ignore_keys.contains(key) {
                    out.insert(key.clone(), Value::String(NORMALIZED.into()));
                } else {
                    out.insert(key.clone(), normalize(value, normalizer));
                }
            }
            Value::Object(out)
        }
        Value::Array(list) => Value::Array(list.iter().map(|v| normalize(v, normalizer)).collect()),
        Value::String(text) => {
            for (from, to) in &normalizer.value_replacements {
                if text == from {
                    return Value::String(to.clone());
                }
            }
            if normalizer
                .path_prefixes
                .iter()
                .any(|p| text.starts_with(p.as_str()))
            {
                Value::String(PATH.into())
            } else {
                Value::String(text.clone())
            }
        }
        other => other.clone(),
    }
}

/// Kind of one semantic difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    MissingInActual,
    UnexpectedInActual,
    ValueMismatch,
    ArrayLength,
}

/// One semantic difference entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Diff {
    pub path: String,
    pub kind: DiffKind,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            DiffKind::MissingInActual => write!(
                f,
                "{}: missing (expected {})",
                self.path,
                self.expected
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_default()
            ),
            DiffKind::UnexpectedInActual => {
                write!(
                    f,
                    "{}: unexpected {}",
                    self.path,
                    self.actual
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_default()
                )
            }
            DiffKind::ValueMismatch => write!(
                f,
                "{}: expected {}, actual {}",
                self.path,
                self.expected
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_default(),
                self.actual
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_default()
            ),
            DiffKind::ArrayLength => write!(
                f,
                "{}: array length expected {}, actual {}",
                self.path,
                self.expected
                    .as_ref()
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0),
                self.actual
                    .as_ref()
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            ),
        }
    }
}

/// Compare two JSON values semantically (objects unordered, arrays ordered).
pub fn semantic_diff(expected: &Value, actual: &Value) -> Vec<Diff> {
    let mut diffs = Vec::new();
    diff_at("$", expected, actual, &mut diffs);
    diffs
}

pub fn semantically_equal(expected: &Value, actual: &Value) -> bool {
    semantic_diff(expected, actual).is_empty()
}

fn diff_at(path: &str, expected: &Value, actual: &Value, diffs: &mut Vec<Diff>) {
    match (expected, actual) {
        (Value::Object(expected_map), Value::Object(actual_map)) => {
            for (key, expected_value) in expected_map {
                let child_path = format!("{path}.{key}");
                match actual_map.get(key) {
                    Some(actual_value) => diff_at(&child_path, expected_value, actual_value, diffs),
                    None => diffs.push(Diff {
                        path: child_path,
                        kind: DiffKind::MissingInActual,
                        expected: Some(expected_value.clone()),
                        actual: None,
                    }),
                }
            }
            for (key, actual_value) in actual_map {
                if !expected_map.contains_key(key) {
                    diffs.push(Diff {
                        path: format!("{path}.{key}"),
                        kind: DiffKind::UnexpectedInActual,
                        expected: None,
                        actual: Some(actual_value.clone()),
                    });
                }
            }
        }
        (Value::Array(expected_list), Value::Array(actual_list)) => {
            if expected_list.len() != actual_list.len() {
                diffs.push(Diff {
                    path: path.to_string(),
                    kind: DiffKind::ArrayLength,
                    expected: Some(expected.clone()),
                    actual: Some(actual.clone()),
                });
                return;
            }
            for (index, (expected_item, actual_item)) in
                expected_list.iter().zip(actual_list.iter()).enumerate()
            {
                diff_at(
                    &format!("{path}[{index}]"),
                    expected_item,
                    actual_item,
                    diffs,
                );
            }
        }
        (expected, actual) => {
            if expected != actual {
                diffs.push(Diff {
                    path: path.to_string(),
                    kind: DiffKind::ValueMismatch,
                    expected: Some(expected.clone()),
                    actual: Some(actual.clone()),
                });
            }
        }
    }
}

/// Convenience: normalize both sides then compare.
pub fn normalized_diff(expected: &Value, actual: &Value, normalizer: &Normalizer) -> Vec<Diff> {
    semantic_diff(
        &normalize(expected, normalizer),
        &normalize(actual, normalizer),
    )
}
