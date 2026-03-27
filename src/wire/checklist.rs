use crate::ids::ThingsId;
use crate::wire::task::TaskStatus;
use crate::wire::{deserialize_default_on_null, deserialize_vec_or_single};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

fn is_false(v: &bool) -> bool {
    !*v
}

/// Checklist item wire properties.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChecklistItemProps {
    /// `tt`: checklist item title.
    #[serde(rename = "tt", default)]
    pub title: String,

    /// `ss`: checklist item status.
    #[serde(rename = "ss", default)]
    pub status: TaskStatus,

    /// `sp`: completion/cancellation timestamp.
    #[serde(rename = "sp", default, skip_serializing_if = "Option::is_none")]
    pub stop_date: Option<f64>,

    /// `ts`: parent task IDs (normally a single task UUID).
    #[serde(rename = "ts", default, deserialize_with = "deserialize_vec_or_single")]
    pub task_ids: Vec<ThingsId>,

    /// `ix`: sort index within checklist.
    #[serde(rename = "ix", default)]
    pub sort_index: i32,

    /// `cd`: creation timestamp.
    #[serde(rename = "cd", default, skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<f64>,

    /// `md`: modification timestamp.
    #[serde(rename = "md", default, skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,

    /// `lt`: leaves tombstone on delete.
    #[serde(
        rename = "lt",
        default,
        deserialize_with = "deserialize_default_on_null",
        skip_serializing_if = "is_false"
    )]
    pub leaves_tombstone: bool,

    /// `xx`: conflict override metadata.
    #[serde(rename = "xx", default, skip_serializing_if = "Option::is_none")]
    pub conflict_overrides: Option<Value>,
}

/// Sparse patch fields for ChecklistItem `t=1` updates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChecklistItemPatch {
    /// `tt`: title.
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// `ss`: status.
    #[serde(rename = "ss", skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,

    /// `ts`: parent task IDs.
    #[serde(rename = "ts", skip_serializing_if = "Option::is_none")]
    pub task_ids: Option<Vec<ThingsId>>,

    /// `ix`: sort index.
    #[serde(rename = "ix", skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,

    /// `cd`: creation timestamp.
    #[serde(rename = "cd", skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<f64>,

    /// `md`: modification timestamp.
    #[serde(rename = "md", skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,
}

impl ChecklistItemPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.status.is_none()
            && self.task_ids.is_none()
            && self.sort_index.is_none()
            && self.creation_date.is_none()
            && self.modification_date.is_none()
    }

    pub fn into_properties(self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}
