use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type WireItem = BTreeMap<String, WireObject>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireObject {
    #[serde(rename = "t")]
    pub operation_type: OperationType,
    #[serde(rename = "e")]
    pub entity_type: Option<EntityType>,
    #[serde(rename = "p", default)]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum OperationType {
    #[default]
    Create,
    Update,
    Delete,
    Unknown(i32),
}

impl From<i32> for OperationType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Create,
            1 => Self::Update,
            2 => Self::Delete,
            other => Self::Unknown(other),
        }
    }
}

impl From<OperationType> for i32 {
    fn from(value: OperationType) -> Self {
        match value {
            OperationType::Create => 0,
            OperationType::Update => 1,
            OperationType::Delete => 2,
            OperationType::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "String", into = "String")]
pub enum EntityType {
    Task6,
    ChecklistItem3,
    Tag4,
    Area3,
    Settings5,
    Tombstone2,
    Command,
    Unknown(String),
}

impl From<String> for EntityType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Task6" => Self::Task6,
            "ChecklistItem3" => Self::ChecklistItem3,
            "Tag4" => Self::Tag4,
            "Area3" => Self::Area3,
            "Settings5" => Self::Settings5,
            "Tombstone2" => Self::Tombstone2,
            "Command" => Self::Command,
            _ => Self::Unknown(value),
        }
    }
}

impl From<EntityType> for String {
    fn from(value: EntityType) -> Self {
        match value {
            EntityType::Task6 => "Task6".to_string(),
            EntityType::ChecklistItem3 => "ChecklistItem3".to_string(),
            EntityType::Tag4 => "Tag4".to_string(),
            EntityType::Area3 => "Area3".to_string(),
            EntityType::Settings5 => "Settings5".to_string(),
            EntityType::Tombstone2 => "Tombstone2".to_string(),
            EntityType::Command => "Command".to_string(),
            EntityType::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TaskProps {
    #[serde(rename = "tt", default)]
    pub title: String,
    #[serde(rename = "nt", default)]
    pub notes: Option<Value>,
    #[serde(rename = "tp", default)]
    pub item_type: TaskType,
    #[serde(rename = "ss", default)]
    pub status: TaskStatus,
    #[serde(rename = "sp", default)]
    pub stop_date: Option<f64>,
    #[serde(rename = "st", default)]
    pub start_location: TaskStart,
    #[serde(rename = "sr", default)]
    pub scheduled_date: Option<i64>,
    #[serde(rename = "tir", default)]
    pub today_index_reference: Option<i64>,
    #[serde(rename = "dd", default)]
    pub deadline: Option<i64>,
    #[serde(rename = "dds", default)]
    pub deadline_suppressed_date: Option<Value>,
    #[serde(rename = "pr", default)]
    pub parent_project_ids: Vec<String>,
    #[serde(rename = "ar", default)]
    pub area_ids: Vec<String>,
    #[serde(rename = "agr", default)]
    pub action_group_ids: Vec<String>,
    #[serde(rename = "tg", default)]
    pub tag_ids: Vec<String>,
    #[serde(rename = "ix", default)]
    pub sort_index: i32,
    #[serde(rename = "ti", default)]
    pub today_sort_index: i32,
    #[serde(rename = "do", default)]
    pub due_date_offset: i32,
    #[serde(rename = "rr", default)]
    pub recurrence_rule: Option<Value>,
    #[serde(rename = "rt", default)]
    pub recurrence_template_ids: Vec<String>,
    #[serde(rename = "icsd", default)]
    pub instance_creation_suppressed_date: Option<i64>,
    #[serde(rename = "acrd", default)]
    pub after_completion_reference_date: Option<i64>,
    #[serde(rename = "icc", default)]
    pub checklist_item_count: i32,
    #[serde(rename = "icp", default)]
    pub instance_creation_paused: bool,
    #[serde(rename = "ato", default)]
    pub alarm_time_offset: Option<i64>,
    #[serde(rename = "lai", default)]
    pub last_alarm_interaction: Option<f64>,
    #[serde(rename = "sb", default)]
    pub evening_bit: i32,
    #[serde(rename = "lt", default)]
    pub leaves_tombstone: bool,
    #[serde(rename = "tr", default)]
    pub trashed: bool,
    #[serde(rename = "dl", default)]
    pub deadline_list: Vec<Value>,
    #[serde(rename = "xx", default)]
    pub conflict_overrides: Option<Value>,
    #[serde(rename = "cd", default)]
    pub creation_date: Option<f64>,
    #[serde(rename = "md", default)]
    pub modification_date: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TaskPatch {
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "nt", skip_serializing_if = "Option::is_none")]
    pub notes: Option<Value>,
    #[serde(rename = "st", skip_serializing_if = "Option::is_none")]
    pub start_location: Option<TaskStart>,
    #[serde(rename = "sr", skip_serializing_if = "Option::is_none")]
    pub scheduled_date: Option<Value>,
    #[serde(rename = "tir", skip_serializing_if = "Option::is_none")]
    pub today_index_reference: Option<Value>,
    #[serde(rename = "pr", skip_serializing_if = "Option::is_none")]
    pub parent_project_ids: Option<Vec<String>>,
    #[serde(rename = "ar", skip_serializing_if = "Option::is_none")]
    pub area_ids: Option<Vec<String>>,
    #[serde(rename = "agr", skip_serializing_if = "Option::is_none")]
    pub action_group_ids: Option<Vec<String>>,
    #[serde(rename = "tg", skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    #[serde(rename = "sb", skip_serializing_if = "Option::is_none")]
    pub evening_bit: Option<i32>,
    #[serde(rename = "md", skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,
}

impl TaskPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.notes.is_none()
            && self.start_location.is_none()
            && self.scheduled_date.is_none()
            && self.today_index_reference.is_none()
            && self.parent_project_ids.is_none()
            && self.area_ids.is_none()
            && self.action_group_ids.is_none()
            && self.tag_ids.is_none()
            && self.evening_bit.is_none()
            && self.modification_date.is_none()
    }

    pub fn into_properties(self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum TaskType {
    #[default]
    Todo,
    Project,
    Heading,
    Unknown(i32),
}

impl From<i32> for TaskType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Todo,
            1 => Self::Project,
            2 => Self::Heading,
            other => Self::Unknown(other),
        }
    }
}

impl From<TaskType> for i32 {
    fn from(value: TaskType) -> Self {
        match value {
            TaskType::Todo => 0,
            TaskType::Project => 1,
            TaskType::Heading => 2,
            TaskType::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum TaskStatus {
    #[default]
    Incomplete,
    Canceled,
    Completed,
    Unknown(i32),
}

impl From<i32> for TaskStatus {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Incomplete,
            2 => Self::Canceled,
            3 => Self::Completed,
            other => Self::Unknown(other),
        }
    }
}

impl From<TaskStatus> for i32 {
    fn from(value: TaskStatus) -> Self {
        match value {
            TaskStatus::Incomplete => 0,
            TaskStatus::Canceled => 2,
            TaskStatus::Completed => 3,
            TaskStatus::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum TaskStart {
    #[default]
    Inbox,
    Anytime,
    Someday,
    Unknown(i32),
}

impl From<i32> for TaskStart {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::Inbox,
            1 => Self::Anytime,
            2 => Self::Someday,
            other => Self::Unknown(other),
        }
    }
}

impl From<TaskStart> for i32 {
    fn from(value: TaskStart) -> Self {
        match value {
            TaskStart::Inbox => 0,
            TaskStart::Anytime => 1,
            TaskStart::Someday => 2,
            TaskStart::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RecurrenceRule {
    #[serde(rename = "tp", default)]
    pub repeat_type: RecurrenceType,
    #[serde(rename = "fu", default = "default_frequency_unit")]
    pub frequency_unit: FrequencyUnit,
    #[serde(rename = "fa", default = "default_frequency_amount")]
    pub frequency_amount: i32,
    #[serde(rename = "of", default)]
    pub offsets: Vec<BTreeMap<String, Value>>,
    #[serde(rename = "sr", default)]
    pub start_reference: Option<i64>,
    #[serde(rename = "ia", default)]
    pub initial_anchor: Option<i64>,
    #[serde(rename = "ed", default = "default_recurrence_end_date")]
    pub end_date: i64,
    #[serde(rename = "rc", default)]
    pub repeat_count: i32,
    #[serde(rename = "ts", default)]
    pub task_skip: i32,
    #[serde(rename = "rrv", default = "default_recurrence_rule_version")]
    pub recurrence_rule_version: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum RecurrenceType {
    #[default]
    FixedSchedule,
    AfterCompletion,
    Unknown(i32),
}

impl From<i32> for RecurrenceType {
    fn from(value: i32) -> Self {
        match value {
            0 => Self::FixedSchedule,
            1 => Self::AfterCompletion,
            other => Self::Unknown(other),
        }
    }
}

impl From<RecurrenceType> for i32 {
    fn from(value: RecurrenceType) -> Self {
        match value {
            RecurrenceType::FixedSchedule => 0,
            RecurrenceType::AfterCompletion => 1,
            RecurrenceType::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(from = "i32", into = "i32")]
pub enum FrequencyUnit {
    Daily,
    Monthly,
    #[default]
    Weekly,
    Unknown(i32),
}

impl From<i32> for FrequencyUnit {
    fn from(value: i32) -> Self {
        match value {
            8 => Self::Daily,
            16 => Self::Monthly,
            256 => Self::Weekly,
            other => Self::Unknown(other),
        }
    }
}

impl From<FrequencyUnit> for i32 {
    fn from(value: FrequencyUnit) -> Self {
        match value {
            FrequencyUnit::Daily => 8,
            FrequencyUnit::Monthly => 16,
            FrequencyUnit::Weekly => 256,
            FrequencyUnit::Unknown(other) => other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChecklistItemProps {
    #[serde(rename = "tt", default)]
    pub title: String,
    #[serde(rename = "ss", default)]
    pub status: TaskStatus,
    #[serde(rename = "sp", default)]
    pub stop_date: Option<f64>,
    #[serde(rename = "ts", default)]
    pub task_ids: Vec<String>,
    #[serde(rename = "ix", default)]
    pub sort_index: i32,
    #[serde(rename = "cd", default)]
    pub creation_date: Option<f64>,
    #[serde(rename = "md", default)]
    pub modification_date: Option<f64>,
    #[serde(rename = "lt", default)]
    pub leaves_tombstone: bool,
    #[serde(rename = "xx", default)]
    pub conflict_overrides: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChecklistItemPatch {
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "ss", skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    #[serde(rename = "ts", skip_serializing_if = "Option::is_none")]
    pub task_ids: Option<Vec<String>>,
    #[serde(rename = "ix", skip_serializing_if = "Option::is_none")]
    pub sort_index: Option<i32>,
    #[serde(rename = "cd", skip_serializing_if = "Option::is_none")]
    pub creation_date: Option<f64>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagProps {
    #[serde(rename = "tt", default)]
    pub title: String,
    #[serde(rename = "sh", default)]
    pub shortcut: Option<String>,
    #[serde(rename = "ix", default)]
    pub sort_index: i32,
    #[serde(rename = "pn", default)]
    pub parent_ids: Vec<String>,
    #[serde(rename = "xx", default)]
    pub conflict_overrides: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagPatch {
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "pn", skip_serializing_if = "Option::is_none")]
    pub parent_ids: Option<Vec<String>>,
    #[serde(rename = "md", skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,
}

impl TagPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.parent_ids.is_none() && self.modification_date.is_none()
    }

    pub fn into_properties(self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AreaProps {
    #[serde(rename = "tt", default)]
    pub title: String,
    #[serde(rename = "tg", default)]
    pub tag_ids: Vec<String>,
    #[serde(rename = "ix", default)]
    pub sort_index: i32,
    #[serde(rename = "xx", default)]
    pub conflict_overrides: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AreaPatch {
    #[serde(rename = "tt", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "tg", skip_serializing_if = "Option::is_none")]
    pub tag_ids: Option<Vec<String>>,
    #[serde(rename = "md", skip_serializing_if = "Option::is_none")]
    pub modification_date: Option<f64>,
}

impl AreaPatch {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.tag_ids.is_none() && self.modification_date.is_none()
    }

    pub fn into_properties(self) -> BTreeMap<String, Value> {
        match serde_json::to_value(self) {
            Ok(Value::Object(map)) => map.into_iter().collect(),
            _ => BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TombstoneProps {
    #[serde(rename = "dloid", default)]
    pub deleted_object_id: String,
    #[serde(rename = "dld", default)]
    pub delete_date: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CommandProps {
    #[serde(rename = "tp", default)]
    pub command_type: i32,
    #[serde(rename = "cd", default)]
    pub creation_date: Option<i64>,
    #[serde(rename = "if", default)]
    pub initial_fields: Option<BTreeMap<String, Value>>,
}

fn default_frequency_unit() -> FrequencyUnit {
    FrequencyUnit::Weekly
}

const fn default_frequency_amount() -> i32 {
    1
}

const fn default_recurrence_end_date() -> i64 {
    64_092_211_200
}

const fn default_recurrence_rule_version() -> i32 {
    4
}
