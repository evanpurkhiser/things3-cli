use crate::ids::ThingsId;
use crate::wire::deserialize_optional_field;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Tag wire properties.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagProps {
    /// `tt`: tag title.
    #[serde(rename = "tt", default)]
    pub title: String,

    /// `sh`: keyboard shortcut.
    #[serde(rename = "sh", default)]
    pub shortcut: Option<String>,

    /// `ix`: sort index.
    #[serde(rename = "ix", default)]
    pub sort_index: i32,

    /// `pn`: parent tag IDs (supports nesting).
    #[serde(rename = "pn", default)]
    pub parent_ids: Vec<ThingsId>,

    /// `xx`: conflict override metadata.
    #[serde(rename = "xx", default)]
    pub conflict_overrides: Option<Value>,
}

/// Sparse patch fields for Tag `t=1` updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagPatch {
    /// `tt`: title.
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// `pn`: parent tag IDs.
    #[serde(rename = "pn", skip_serializing_if = "Option::is_none")]
    pub parent_ids: Option<Vec<ThingsId>>,

    /// `md`: modification timestamp.
    #[serde(rename = "md", skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,

    /// `sh`: shortcut.
    #[serde(
        rename = "sh",
        default,
        deserialize_with = "deserialize_optional_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub shortcut: Option<Option<String>>,

    /// `ix`: sort index.
    #[serde(rename = "ix", skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
}

impl TagPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.parent_ids.is_none()
            && self.modification_date.is_none()
            && self.shortcut.is_none()
            && self.sort_index.is_none()
    }

    pub fn into_properties(self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}
