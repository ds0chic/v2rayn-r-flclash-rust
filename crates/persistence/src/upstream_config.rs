//! `guiNConfig.json` storage.
//!
//! The upstream `Config` root is serialized with PascalCase property names and
//! enums as integers (`JsonUtils.cs`). The document is kept as its raw JSON
//! tree for exact preservation, with a dedicated [`ConfigStorage`] serde view
//! for the T04-relevant subtrees (active ids, window/column state, theme). All
//! other `ConfigItems` and unknown keys survive via `#[serde(flatten)]`.

use domain::{ColumnDefinition, GirdOrientation, WindowState};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{PersistenceError, Result};

/// Tri-state of a JSON field, so "absent", "explicit null" and "empty string"
/// stay distinguishable (plan §11 / `compat/fields.yaml` null semantics).
#[derive(Debug, Clone, PartialEq)]
pub enum FieldState {
    Missing,
    Null,
    Value(Value),
}

impl FieldState {
    pub fn is_missing(&self) -> bool {
        matches!(self, FieldState::Missing)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, FieldState::Null)
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            FieldState::Value(Value::String(s)) => Some(s),
            _ => None,
        }
    }
}

/// Dedicated serde view of the `Config` root. `items` carries every
/// `ConfigItem` plus any unknown key untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ConfigStorage {
    pub index_id: String,
    pub sub_index_id: String,
    #[serde(flatten)]
    pub items: Map<String, Value>,
}

/// `WindowSizeItem` as stored inside `UiItem.WindowSizeItem[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct WindowStateStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_grid_height1: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_grid_height2: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// `ColumnItem` as stored inside `UiItem.MainColumnItem[]` /
/// `ClashUIItem.ConnectionsColumnItem[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase", default)]
pub struct ColumnItemStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub width: i32,
    pub index: i32,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn orientation_from_value(value: Option<&Value>) -> GirdOrientation {
    match value {
        Some(Value::Number(n)) => match n.as_i64() {
            Some(0) => GirdOrientation::Horizontal,
            Some(2) => GirdOrientation::Tab,
            _ => GirdOrientation::Vertical,
        },
        Some(Value::String(s)) => match s.to_ascii_lowercase().as_str() {
            "horizontal" => GirdOrientation::Horizontal,
            "tab" => GirdOrientation::Tab,
            _ => GirdOrientation::Vertical,
        },
        _ => GirdOrientation::Vertical,
    }
}

impl WindowStateStorage {
    pub fn to_domain(&self) -> WindowState {
        WindowState {
            type_name: self.type_name.clone().unwrap_or_default(),
            width: self.width,
            height: self.height,
            main_grid_height1: self.main_grid_height1.unwrap_or(0),
            main_grid_height2: self.main_grid_height2.unwrap_or(0),
            orientation: orientation_from_value(self.orientation.as_ref()),
            extra: self.extra.clone(),
        }
    }

    pub fn from_domain(value: &WindowState) -> Self {
        Self {
            type_name: Some(value.type_name.clone()),
            width: value.width,
            height: value.height,
            main_grid_height1: Some(value.main_grid_height1),
            main_grid_height2: Some(value.main_grid_height2),
            orientation: Some(Value::from(match value.orientation {
                GirdOrientation::Horizontal => 0,
                GirdOrientation::Vertical => 1,
                GirdOrientation::Tab => 2,
            })),
            extra: value.extra.clone(),
        }
    }
}

impl ColumnItemStorage {
    pub fn to_domain(&self) -> ColumnDefinition {
        ColumnDefinition {
            name: self.name.clone().unwrap_or_default(),
            width: self.width,
            index: self.index,
            extra: self.extra.clone(),
        }
    }

    pub fn from_domain(value: &ColumnDefinition) -> Self {
        Self {
            name: Some(value.name.clone()),
            width: value.width,
            index: value.index,
            extra: value.extra.clone(),
        }
    }
}

/// A parsed `guiNConfig.json` document.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDocument {
    raw: Value,
}

impl ConfigDocument {
    /// Parse from JSON text. Comments are not accepted (upstream writes clean
    /// JSON); anything but an object is rejected as not-a-source.
    pub fn parse(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text)?;
        Self::from_value(value)
    }

    pub fn from_value(value: Value) -> Result<Self> {
        if !value.is_object() {
            return Err(PersistenceError::NotASource(
                "guiNConfig.json root is not a JSON object".into(),
            ));
        }
        Ok(Self { raw: value })
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }

    /// The typed serde storage view (flatten preserves unknown keys).
    pub fn storage(&self) -> Result<ConfigStorage> {
        Ok(serde_json::from_value(self.raw.clone())?)
    }

    /// Exact raw JSON, preserving key order and null/absent distinctions.
    pub fn to_json_value(&self) -> Value {
        self.raw.clone()
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string(&self.raw)?)
    }

    /// Resolve a dotted path, reporting absent vs. explicit null.
    pub fn field_state(&self, path: &[&str]) -> FieldState {
        let mut current = &self.raw;
        for segment in path {
            match current.get(*segment) {
                Some(next) => current = next,
                None => return FieldState::Missing,
            }
        }
        match current {
            Value::Null => FieldState::Null,
            other => FieldState::Value(other.clone()),
        }
    }

    pub fn index_id(&self) -> String {
        self.raw
            .get("IndexId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    pub fn sub_index_id(&self) -> String {
        self.raw
            .get("SubIndexId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    fn item(&self, key: &str) -> Option<&Value> {
        // Upstream serializes the `UIItem` CLR class as the JSON key `"UiItem"`
        // (PascalCase of the property name). Older/hand-edited configs sometimes
        // carry `"UIItem"`; accept both, preferring the canonical spelling.
        if key == "UiItem" {
            self.raw.get("UiItem").or_else(|| self.raw.get("UIItem"))
        } else {
            self.raw.get(key)
        }
    }

    fn array_at<'a>(&'a self, parent: &str, key: &str) -> Result<Vec<&'a Value>> {
        match self.item(parent).and_then(|v| v.get(key)) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => Ok(items.iter().collect()),
            Some(_) => Err(PersistenceError::corrupt(format!(
                "{parent}.{key} is not an array"
            ))),
        }
    }

    pub fn window_states(&self) -> Result<Vec<WindowStateStorage>> {
        self.array_at("UiItem", "WindowSizeItem")?
            .into_iter()
            .map(|v| Ok(serde_json::from_value(v.clone())?))
            .collect()
    }

    pub fn window_states_domain(&self) -> Result<Vec<WindowState>> {
        Ok(self
            .window_states()?
            .iter()
            .map(WindowStateStorage::to_domain)
            .collect())
    }

    pub fn main_columns(&self) -> Result<Vec<ColumnItemStorage>> {
        self.array_at("UiItem", "MainColumnItem")?
            .into_iter()
            .map(|v| Ok(serde_json::from_value(v.clone())?))
            .collect()
    }

    pub fn clash_columns(&self) -> Result<Vec<ColumnItemStorage>> {
        self.array_at("ClashUIItem", "ConnectionsColumnItem")?
            .into_iter()
            .map(|v| Ok(serde_json::from_value(v.clone())?))
            .collect()
    }

    pub fn theme(&self) -> Option<String> {
        self.item("UiItem")
            .and_then(|ui| ui.get("CurrentTheme"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn language(&self) -> Option<String> {
        self.item("UiItem")
            .and_then(|ui| ui.get("CurrentLanguage"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn font_family(&self) -> Option<String> {
        self.item("UiItem")
            .and_then(|ui| ui.get("CurrentFontFamily"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn font_size(&self) -> Option<i64> {
        self.item("UiItem")
            .and_then(|ui| ui.get("CurrentFontSize"))
            .and_then(Value::as_i64)
    }

    /// The `HysteriaItem` blob used as a V2->V3 migration input.
    pub fn hysteria_item(&self) -> HysteriaStorage {
        let item = self.item("HysteriaItem");
        HysteriaStorage {
            up_mbps: item.and_then(|v| v.get("UpMbps")).and_then(Value::as_i64),
            down_mbps: item.and_then(|v| v.get("DownMbps")).and_then(Value::as_i64),
            hop_interval: item
                .and_then(|v| v.get("HopInterval"))
                .and_then(Value::as_i64),
        }
    }
}

/// The subset of `HysteriaItem` the V2->V3 migration reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HysteriaStorage {
    pub up_mbps: Option<i64>,
    pub down_mbps: Option<i64>,
    pub hop_interval: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_flatten_preserves_unknown_items() {
        let raw = r#"{"IndexId":"a","SubIndexId":"s","UIItem":{"CurrentTheme":"Dark"},
            "BrandNewItem":{"x":1}}"#;
        let doc = ConfigDocument::parse(raw).unwrap();
        let storage = doc.storage().unwrap();
        assert_eq!(storage.index_id, "a");
        assert!(storage.items.contains_key("UIItem"));
        assert!(storage.items.contains_key("BrandNewItem"));
    }

    #[test]
    fn field_state_distinguishes_missing_null_empty() {
        let raw = r#"{"A":null,"B":"","UIItem":{"C":null}}"#;
        let doc = ConfigDocument::parse(raw).unwrap();
        assert!(doc.field_state(&["B"]).as_str() == Some(""));
        assert!(doc.field_state(&["A"]).is_null());
        assert!(doc.field_state(&["Z"]).is_missing());
        assert!(doc.field_state(&["UIItem", "C"]).is_null());
        assert!(doc.field_state(&["UIItem", "D"]).is_missing());
    }

    #[test]
    fn window_and_column_state_map_to_domain() {
        let raw = r#"{
            "IndexId": "n9",
            "UIItem": {
                "WindowSizeItem": [{"TypeName":"MainWindow","Width":1200,"Height":800,
                                    "MainGridHeight1":300,"Orientation":2}],
                "MainColumnItem": [{"Name":"Remarks","Width":180,"Index":0}]
            },
            "ClashUIItem": {"ConnectionsColumnItem":[{"Name":"Time","Width":90,"Index":0}]}
        }"#;
        let doc = ConfigDocument::parse(raw).unwrap();
        let windows = doc.window_states_domain().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].type_name, "MainWindow");
        assert_eq!(windows[0].orientation, GirdOrientation::Tab);
        let cols = doc.main_columns().unwrap();
        assert_eq!(cols[0].to_domain().name, "Remarks");
        assert_eq!(doc.clash_columns().unwrap().len(), 1);
    }

    #[test]
    fn non_object_root_is_rejected() {
        assert!(ConfigDocument::parse("[]").is_err());
        assert!(ConfigDocument::parse("\"x\"").is_err());
    }

    #[test]
    fn canonical_ui_item_key_is_read() {
        // Upstream writes `UiItem` (PascalCase of the CLR property), not `UIItem`.
        let raw = r#"{
            "IndexId": "n1",
            "UiItem": {
                "CurrentTheme": "Dark",
                "CurrentLanguage": "zh-Hans",
                "CurrentFontFamily": "Microsoft YaHei",
                "CurrentFontSize": 13,
                "MainColumnItem": [{"Name": "Remarks", "Width": 180, "Index": 0}],
                "WindowSizeItem": [{"TypeName": "MainWindow", "Width": 1200, "Height": 800}]
            }
        }"#;
        let doc = ConfigDocument::parse(raw).unwrap();
        assert_eq!(doc.theme().as_deref(), Some("Dark"));
        assert_eq!(doc.language().as_deref(), Some("zh-Hans"));
        assert_eq!(doc.font_family().as_deref(), Some("Microsoft YaHei"));
        assert_eq!(doc.font_size(), Some(13));
        assert_eq!(doc.window_states().unwrap().len(), 1);
        assert_eq!(doc.main_columns().unwrap()[0].width, 180);
    }
}
