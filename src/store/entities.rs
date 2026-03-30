use crate::ids::ThingsId;
use crate::wire::area::{AreaPatch, AreaProps};
use crate::wire::checklist::{ChecklistItemPatch, ChecklistItemProps};
use crate::wire::notes::TaskNotes;
use crate::wire::recurrence::RecurrenceRule;
use crate::wire::tags::{TagPatch, TagProps};
use crate::wire::task::{TaskPatch, TaskProps, TaskStart, TaskStatus, TaskType};
use crate::wire::wire_object::EntityType;
use crate::wire::wire_object::Properties;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateObject {
    pub entity_type: Option<EntityType>,
    pub properties: StateProperties,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateProperties {
    Task(TaskStateProps),
    ChecklistItem(ChecklistItemStateProps),
    Area(AreaStateProps),
    Tag(TagStateProps),
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TaskStateProps {
    pub title: String,
    pub notes: Option<String>,
    pub item_type: TaskType,
    pub status: TaskStatus,
    pub stop_date: Option<f64>,
    pub start_location: TaskStart,
    pub scheduled_date: Option<f64>,
    pub today_index_reference: Option<i64>,
    pub deadline: Option<f64>,
    pub parent_project_ids: Vec<ThingsId>,
    pub area_ids: Vec<ThingsId>,
    pub action_group_ids: Vec<ThingsId>,
    pub tag_ids: Vec<ThingsId>,
    pub sort_index: i32,
    pub today_sort_index: i32,
    pub recurrence_rule: Option<RecurrenceRule>,
    pub recurrence_template_ids: Vec<ThingsId>,
    pub instance_creation_paused: bool,
    pub evening_bit: i32,
    pub leaves_tombstone: bool,
    pub trashed: bool,
    pub creation_date: Option<f64>,
    pub modification_date: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChecklistItemStateProps {
    pub title: String,
    pub status: TaskStatus,
    pub task_ids: Vec<ThingsId>,
    pub sort_index: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AreaStateProps {
    pub title: String,
    pub tag_ids: Vec<ThingsId>,
    pub sort_index: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TagStateProps {
    pub title: String,
    pub shortcut: Option<String>,
    pub sort_index: i32,
    pub parent_ids: Vec<ThingsId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub uuid: ThingsId,
    pub title: String,
    pub shortcut: Option<String>,
    pub index: i32,
    pub parent_uuid: Option<ThingsId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    pub uuid: ThingsId,
    pub title: String,
    pub tags: Vec<ThingsId>,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistItem {
    pub uuid: ThingsId,
    pub title: String,
    pub task_uuid: ThingsId,
    pub status: TaskStatus,
    pub index: i32,
}

impl ChecklistItem {
    pub fn is_incomplete(&self) -> bool {
        self.status == TaskStatus::Incomplete
    }

    pub fn is_completed(&self) -> bool {
        self.status == TaskStatus::Completed
    }

    pub fn is_canceled(&self) -> bool {
        self.status == TaskStatus::Canceled
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectProgress {
    pub total: i32,
    pub done: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Task {
    pub uuid: ThingsId,
    pub title: String,
    pub status: TaskStatus,
    pub start: TaskStart,
    pub item_type: TaskType,
    pub entity: String,
    pub notes: Option<String>,
    pub project: Option<ThingsId>,
    pub area: Option<ThingsId>,
    pub action_group: Option<ThingsId>,
    pub tags: Vec<ThingsId>,
    pub trashed: bool,
    pub deadline: Option<DateTime<Utc>>,
    pub start_date: Option<DateTime<Utc>>,
    pub stop_date: Option<DateTime<Utc>>,
    pub creation_date: Option<DateTime<Utc>>,
    pub modification_date: Option<DateTime<Utc>>,
    pub index: i32,
    pub today_index: i32,
    pub today_index_reference: Option<i64>,
    pub leaves_tombstone: bool,
    pub instance_creation_paused: bool,
    pub evening: bool,
    pub recurrence_rule: Option<RecurrenceRule>,
    pub recurrence_templates: Vec<ThingsId>,
    pub checklist_items: Vec<ChecklistItem>,
}

impl Task {
    pub fn is_incomplete(&self) -> bool {
        self.status == TaskStatus::Incomplete
    }

    pub fn is_completed(&self) -> bool {
        self.status == TaskStatus::Completed
    }

    pub fn is_canceled(&self) -> bool {
        self.status == TaskStatus::Canceled
    }

    pub fn is_todo(&self) -> bool {
        self.item_type == TaskType::Todo
    }

    pub fn is_project(&self) -> bool {
        self.item_type == TaskType::Project
    }

    pub fn is_heading(&self) -> bool {
        self.item_type == TaskType::Heading
    }

    pub fn in_someday(&self) -> bool {
        self.start == TaskStart::Someday && self.start_date.is_none()
    }

    pub fn is_today(&self, today: &DateTime<Utc>) -> bool {
        let Some(start_date) = self.start_date else {
            return false;
        };
        if self.start != TaskStart::Anytime && self.start != TaskStart::Someday {
            return false;
        }
        start_date <= *today
    }

    pub fn is_staged_for_today(&self, today: &DateTime<Utc>) -> bool {
        let Some(start_date) = self.start_date else {
            return false;
        };
        self.start == TaskStart::Someday && start_date <= *today
    }

    pub fn is_recurrence_template(&self) -> bool {
        self.recurrence_rule.is_some() && self.recurrence_templates.is_empty()
    }

    pub fn is_recurrence_instance(&self) -> bool {
        self.recurrence_rule.is_none() && !self.recurrence_templates.is_empty()
    }
}

fn i64_to_f64_opt(value: Option<i64>) -> Option<f64> {
    value.map(|v| v as f64)
}

fn parse_notes_from_wire(notes: &Option<TaskNotes>) -> Option<String> {
    notes.as_ref().and_then(TaskNotes::to_plain_text)
}

impl From<TaskProps> for TaskStateProps {
    fn from(props: TaskProps) -> Self {
        Self {
            title: props.title,
            notes: parse_notes_from_wire(&props.notes),
            item_type: props.item_type,
            status: props.status,
            stop_date: props.stop_date,
            start_location: props.start_location,
            scheduled_date: i64_to_f64_opt(props.scheduled_date),
            today_index_reference: props.today_index_reference,
            deadline: i64_to_f64_opt(props.deadline),
            parent_project_ids: props.parent_project_ids,
            area_ids: props.area_ids,
            action_group_ids: props.action_group_ids,
            tag_ids: props.tag_ids,
            sort_index: props.sort_index,
            today_sort_index: props.today_sort_index,
            recurrence_rule: props.recurrence_rule,
            recurrence_template_ids: props.recurrence_template_ids,
            instance_creation_paused: props.instance_creation_paused,
            evening_bit: props.evening_bit,
            leaves_tombstone: props.leaves_tombstone,
            trashed: props.trashed,
            creation_date: props.creation_date,
            modification_date: props.modification_date,
        }
    }
}

impl From<ChecklistItemProps> for ChecklistItemStateProps {
    fn from(props: ChecklistItemProps) -> Self {
        Self {
            title: props.title,
            status: props.status,
            task_ids: props.task_ids,
            sort_index: props.sort_index,
        }
    }
}

impl From<AreaProps> for AreaStateProps {
    fn from(props: AreaProps) -> Self {
        Self {
            title: props.title,
            tag_ids: props.tag_ids,
            sort_index: props.sort_index,
        }
    }
}

impl From<TagProps> for TagStateProps {
    fn from(props: TagProps) -> Self {
        Self {
            title: props.title,
            shortcut: props.shortcut,
            sort_index: props.sort_index,
            parent_ids: props.parent_ids,
        }
    }
}

impl From<ChecklistItemPatch> for ChecklistItemStateProps {
    fn from(patch: ChecklistItemPatch) -> Self {
        let mut item = Self::default();
        if let Some(title) = patch.title {
            item.title = title;
        }
        if let Some(status) = patch.status {
            item.status = status;
        }
        if let Some(task_ids) = patch.task_ids {
            item.task_ids = task_ids;
        }
        if let Some(sort_index) = patch.sort_index {
            item.sort_index = sort_index;
        }
        item
    }
}

impl From<AreaPatch> for AreaStateProps {
    fn from(patch: AreaPatch) -> Self {
        let mut area = Self::default();
        if let Some(title) = patch.title {
            area.title = title;
        }
        if let Some(tag_ids) = patch.tag_ids {
            area.tag_ids = tag_ids;
        }
        if let Some(sort_index) = patch.sort_index {
            area.sort_index = sort_index;
        }
        area
    }
}

impl From<TagPatch> for TagStateProps {
    fn from(patch: TagPatch) -> Self {
        let mut tag = Self::default();
        if let Some(title) = patch.title {
            tag.title = title;
        }
        if let Some(parent_ids) = patch.parent_ids {
            tag.parent_ids = parent_ids;
        }
        if let Some(shortcut) = patch.shortcut {
            tag.shortcut = shortcut;
        }
        if let Some(sort_index) = patch.sort_index {
            tag.sort_index = sort_index;
        }
        tag
    }
}

impl From<TaskPatch> for TaskStateProps {
    fn from(patch: TaskPatch) -> Self {
        let mut task = Self::default();
        if let Some(title) = patch.title {
            task.title = title;
        }
        if let Some(notes) = patch.notes {
            task.notes = notes.to_plain_text();
        }
        if let Some(start_location) = patch.start_location {
            task.start_location = start_location;
        }
        if let Some(scheduled_date) = patch.scheduled_date {
            task.scheduled_date = scheduled_date.map(|v| v as f64);
        }
        if let Some(today_index_reference) = patch.today_index_reference {
            task.today_index_reference = today_index_reference;
        }
        if let Some(parent_project_ids) = patch.parent_project_ids {
            task.parent_project_ids = parent_project_ids;
        }
        if let Some(area_ids) = patch.area_ids {
            task.area_ids = area_ids;
        }
        if let Some(action_group_ids) = patch.action_group_ids {
            task.action_group_ids = action_group_ids;
        }
        if let Some(tag_ids) = patch.tag_ids {
            task.tag_ids = tag_ids;
        }
        if let Some(evening_bit) = patch.evening_bit {
            task.evening_bit = evening_bit;
        }
        if let Some(modification_date) = patch.modification_date {
            task.modification_date = Some(modification_date);
        }
        if let Some(item_type) = patch.item_type {
            task.item_type = item_type;
        }
        if let Some(status) = patch.status {
            task.status = status;
        }
        if let Some(stop_date) = patch.stop_date {
            task.stop_date = stop_date;
        }
        if let Some(deadline) = patch.deadline {
            task.deadline = deadline;
        }
        if let Some(sort_index) = patch.sort_index {
            task.sort_index = sort_index;
        }
        if let Some(today_sort_index) = patch.today_sort_index {
            task.today_sort_index = today_sort_index;
        }
        if let Some(recurrence_rule) = patch.recurrence_rule {
            task.recurrence_rule = recurrence_rule;
        }
        if let Some(recurrence_template_ids) = patch.recurrence_template_ids {
            task.recurrence_template_ids = recurrence_template_ids;
        }
        if let Some(instance_creation_paused) = patch.instance_creation_paused {
            task.instance_creation_paused = instance_creation_paused;
        }
        if let Some(leaves_tombstone) = patch.leaves_tombstone {
            task.leaves_tombstone = leaves_tombstone;
        }
        if let Some(trashed) = patch.trashed {
            task.trashed = trashed;
        }
        if let Some(creation_date) = patch.creation_date {
            task.creation_date = creation_date;
        }
        task
    }
}

impl From<Properties> for StateProperties {
    fn from(payload: Properties) -> Self {
        use Properties::*;
        use StateProperties::*;

        match payload {
            TaskCreate(props) => Task(props.into()),
            TaskUpdate(patch) => Task(patch.into()),
            ChecklistCreate(props) => ChecklistItem(props.into()),
            ChecklistUpdate(patch) => ChecklistItem(patch.into()),
            AreaCreate(props) => Area(props.into()),
            AreaUpdate(patch) => Area(patch.into()),
            TagCreate(props) => Tag(props.into()),
            TagUpdate(patch) => Tag(patch.into()),
            TombstoneCreate(_) | CommandCreate(_) | Ignored(_) | Unknown(_) | Delete => Other,
        }
    }
}
