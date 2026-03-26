use crate::common::today_utc;
use crate::things_id::ThingsId;
use crate::wire::{
    EntityType, OperationType, RecurrenceRule, TaskStart, TaskStatus, TaskType, WireItem,
    WireObject,
};
use chrono::{DateTime, Local, TimeZone, Utc};
use serde_json::{Map, Value};
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct StateObject {
    pub entity_type: Option<EntityType>,
    pub properties: Map<String, Value>,
}

pub type RawState = HashMap<String, StateObject>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub uuid: String,
    pub title: String,
    pub shortcut: Option<String>,
    pub index: i32,
    pub parent_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    pub uuid: String,
    pub title: String,
    pub tags: Vec<String>,
    pub index: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChecklistItem {
    pub uuid: String,
    pub title: String,
    pub task_uuid: String,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub uuid: String,
    pub title: String,
    pub status: TaskStatus,
    pub start: TaskStart,
    pub item_type: TaskType,
    pub entity: String,
    pub notes: Option<String>,
    pub project: Option<String>,
    pub area: Option<String>,
    pub action_group: Option<String>,
    pub tags: Vec<String>,
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
    pub recurrence_templates: Vec<String>,
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

    pub fn is_today(&self) -> bool {
        let Some(start_date) = self.start_date else {
            return false;
        };
        if self.start != TaskStart::Anytime {
            return false;
        }
        let today_dt = today_utc();
        start_date <= today_dt
    }

    pub fn is_recurrence_template(&self) -> bool {
        self.recurrence_rule.is_some() && self.recurrence_templates.is_empty()
    }

    pub fn is_recurrence_instance(&self) -> bool {
        self.recurrence_rule.is_none() && !self.recurrence_templates.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct ThingsStore {
    pub tasks_by_uuid: HashMap<String, Task>,
    pub areas_by_uuid: HashMap<String, Area>,
    pub tags_by_uuid: HashMap<String, Tag>,
    pub tags_by_title: HashMap<String, String>,
    pub project_progress_by_uuid: HashMap<String, ProjectProgress>,
    pub short_ids: HashMap<String, String>,
    pub markable_ids: HashSet<String>,
    pub markable_ids_sorted: Vec<String>,
    pub area_ids_sorted: Vec<String>,
    pub task_ids_sorted: Vec<String>,
}

fn ts_to_dt(ts: Option<f64>) -> Option<DateTime<Utc>> {
    let ts = ts?;
    Utc.timestamp_opt(ts as i64, 0).single()
}

fn parse_i32(map: &Map<String, Value>, key: &str, default: i32) -> i32 {
    map.get(key)
        .and_then(Value::as_i64)
        .map(|v| v as i32)
        .unwrap_or(default)
}

fn parse_i64(map: &Map<String, Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(Value::as_i64)
}

fn parse_f64(map: &Map<String, Value>, key: &str) -> Option<f64> {
    map.get(key).and_then(Value::as_f64)
}

fn parse_bool(map: &Map<String, Value>, key: &str, default: bool) -> bool {
    map.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn parse_str(map: &Map<String, Value>, key: &str) -> Option<String> {
    map.get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn parse_str_list(map: &Map<String, Value>, key: &str) -> Vec<String> {
    map.get(key)
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_notes(value: Option<&Value>) -> Option<String> {
    let value = value?;

    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Object(obj) => {
            let t = obj.get("t").and_then(Value::as_i64);
            match t {
                Some(1) => obj
                    .get("v")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .and_then(|s| {
                        let trimmed = s.trim().to_string();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        }
                    }),
                Some(2) => {
                    let paragraphs = obj
                        .get("ps")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let lines: Vec<String> = paragraphs
                        .iter()
                        .filter_map(|p| {
                            p.as_object()
                                .and_then(|o| o.get("r"))
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
                        })
                        .collect();
                    let joined = lines.join("\n");
                    if joined.trim().is_empty() {
                        None
                    } else {
                        Some(joined)
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn lcp_len(a: &str, b: &str) -> usize {
    let mut i = 0usize;
    let max = a.len().min(b.len());
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    while i < max && a_bytes[i] == b_bytes[i] {
        i += 1;
    }
    i
}

fn shortest_unique_prefixes(ids: &[String]) -> HashMap<String, String> {
    if ids.is_empty() {
        return HashMap::new();
    }

    let mut ordered = ids.to_vec();
    ordered.sort();

    let mut result = HashMap::new();
    for (i, value) in ordered.iter().enumerate() {
        let left = if i > 0 {
            lcp_len(value, &ordered[i - 1])
        } else {
            0
        };
        let right = if i + 1 < ordered.len() {
            lcp_len(value, &ordered[i + 1])
        } else {
            0
        };
        let need = left.max(right) + 1;
        result.insert(value.clone(), value.chars().take(need).collect());
    }

    result
}

fn normalize_ids(value: Value) -> Value {
    match value {
        Value::String(s) => s
            .parse::<ThingsId>()
            .ok()
            .map(Into::into)
            .map(Value::String)
            .unwrap_or(Value::String(s)),
        Value::Array(values) => Value::Array(values.into_iter().map(normalize_ids).collect()),
        Value::Object(obj) => {
            let mut out = Map::new();
            for (k, v) in obj {
                let new_key = k
                    .parse::<ThingsId>()
                    .ok()
                    .map(Into::into)
                    .unwrap_or(k);
                out.insert(new_key, normalize_ids(v));
            }
            Value::Object(out)
        }
        other => other,
    }
}

fn normalize_item_ids(item: WireItem) -> BTreeMap<String, WireObject> {
    let mut normalized = BTreeMap::new();
    for (uuid, mut obj) in item {
        let new_uuid = uuid
            .parse::<ThingsId>()
            .ok()
            .map(Into::into)
            .unwrap_or(uuid);
        let mut new_props = BTreeMap::new();
        for (k, v) in obj.properties {
            new_props.insert(k, normalize_ids(v));
        }
        obj.properties = new_props;
        normalized.insert(new_uuid, obj);
    }
    normalized
}

pub fn fold_item(item: WireItem, state: &mut RawState) {
    let normalized = normalize_item_ids(item);

    for (uuid, obj) in normalized {
        match obj.operation_type {
            OperationType::Create => {
                state.insert(
                    uuid,
                    StateObject {
                        entity_type: obj.entity_type,
                        properties: obj.properties.into_iter().collect(),
                    },
                );
            }
            OperationType::Update => {
                if let Some(existing) = state.get_mut(&uuid) {
                    for (k, v) in obj.properties {
                        existing.properties.insert(k, v);
                    }
                    if obj.entity_type.is_some() {
                        existing.entity_type = obj.entity_type;
                    }
                } else {
                    state.insert(
                        uuid,
                        StateObject {
                            entity_type: obj.entity_type,
                            properties: obj.properties.into_iter().collect(),
                        },
                    );
                }
            }
            OperationType::Delete => {
                state.remove(&uuid);
            }
            OperationType::Unknown(_) => {}
        }
    }
}

pub fn fold_items(items: impl IntoIterator<Item = WireItem>) -> RawState {
    let mut state = RawState::new();
    for item in items {
        fold_item(item, &mut state);
    }
    state
}

impl ThingsStore {
    pub fn from_raw_state(raw_state: &RawState) -> Self {
        let mut store = Self::default();
        store.build(raw_state);
        store.build_project_progress_index();
        store.short_ids = shortest_unique_prefixes(&store.short_id_domain(raw_state));
        store.build_mark_indexes();
        store.area_ids_sorted = store.areas_by_uuid.keys().cloned().collect();
        store.area_ids_sorted.sort();
        store.task_ids_sorted = store.tasks_by_uuid.keys().cloned().collect();
        store.task_ids_sorted.sort();
        store
    }

    fn short_id_domain(&self, raw_state: &RawState) -> Vec<String> {
        let mut ids = Vec::new();
        for (uuid, obj) in raw_state {
            match obj.entity_type.as_ref() {
                Some(EntityType::Tombstone2) => continue,
                Some(EntityType::Unknown(s)) if s == "Tombstone" => continue,
                _ => {}
            }

            if uuid.starts_with("TOMBSTONE-") {
                continue;
            }

            ids.push(uuid.clone());
        }
        ids
    }

    fn build_mark_indexes(&mut self) {
        let markable: Vec<&Task> = self
            .tasks_by_uuid
            .values()
            .filter(|task| !task.trashed && !task.is_heading() && task.entity == "Task6")
            .collect();

        self.markable_ids = markable.iter().map(|t| t.uuid.clone()).collect();
        self.markable_ids_sorted = self.markable_ids.iter().cloned().collect();
        self.markable_ids_sorted.sort();
    }

    fn build_project_progress_index(&mut self) {
        let mut totals: HashMap<String, i32> = HashMap::new();
        let mut dones: HashMap<String, i32> = HashMap::new();

        for task in self.tasks_by_uuid.values() {
            if task.trashed || !task.is_todo() {
                continue;
            }

            let Some(project_uuid) = self.effective_project_uuid(task) else {
                continue;
            };

            *totals.entry(project_uuid.clone()).or_insert(0) += 1;
            if task.is_completed() {
                *dones.entry(project_uuid).or_insert(0) += 1;
            }
        }

        self.project_progress_by_uuid = totals
            .into_iter()
            .map(|(project_uuid, total)| {
                let done = *dones.get(&project_uuid).unwrap_or(&0);
                (project_uuid, ProjectProgress { total, done })
            })
            .collect();
    }

    fn build(&mut self, raw_state: &RawState) {
        let mut checklist_items: Vec<ChecklistItem> = Vec::new();

        for (uuid, obj) in raw_state {
            let is_task = matches!(obj.entity_type.as_ref(), Some(EntityType::Task6))
                || matches!(obj.entity_type.as_ref(), Some(EntityType::Unknown(s)) if s.starts_with("Task"));
            let is_area = matches!(obj.entity_type.as_ref(), Some(EntityType::Area3))
                || matches!(obj.entity_type.as_ref(), Some(EntityType::Unknown(s)) if s.starts_with("Area"));
            let is_tag = matches!(obj.entity_type.as_ref(), Some(EntityType::Tag4))
                || matches!(obj.entity_type.as_ref(), Some(EntityType::Unknown(s)) if s.starts_with("Tag"));

            match obj.entity_type.as_ref() {
                _ if is_task => {
                    let entity = match obj.entity_type.as_ref() {
                        Some(EntityType::Task6) => "Task6".to_string(),
                        Some(EntityType::Unknown(s)) => s.clone(),
                        Some(other) => String::from(other.clone()),
                        None => "Task6".to_string(),
                    };
                    let task = self.parse_task(uuid, &obj.properties, &entity);
                    self.tasks_by_uuid.insert(uuid.clone(), task);
                }
                _ if is_area => {
                    let area = self.parse_area(uuid, &obj.properties);
                    self.areas_by_uuid.insert(uuid.clone(), area);
                }
                _ if is_tag => {
                    let tag = self.parse_tag(uuid, &obj.properties);
                    if !tag.title.is_empty() {
                        self.tags_by_title
                            .insert(tag.title.clone(), tag.uuid.clone());
                    }
                    self.tags_by_uuid.insert(uuid.clone(), tag);
                }
                Some(EntityType::ChecklistItem3) => {
                    checklist_items.push(self.parse_checklist_item(uuid, &obj.properties));
                }
                _ => {}
            }
        }

        let mut by_task: HashMap<String, Vec<ChecklistItem>> = HashMap::new();
        for item in checklist_items {
            if self.tasks_by_uuid.contains_key(&item.task_uuid) {
                by_task
                    .entry(item.task_uuid.clone())
                    .or_default()
                    .push(item);
            }
        }

        for (task_uuid, items) in by_task.iter_mut() {
            items.sort_by_key(|i| i.index);
            if let Some(task) = self.tasks_by_uuid.get_mut(task_uuid) {
                task.checklist_items = items.clone();
            }
        }
    }

    fn parse_task(&self, uuid: &str, p: &Map<String, Value>, entity: &str) -> Task {
        let project_list = parse_str_list(p, "pr");
        let area_list = parse_str_list(p, "ar");
        let action_group_list = parse_str_list(p, "agr");
        let recurrence_rule = p
            .get("rr")
            .and_then(|v| serde_json::from_value::<RecurrenceRule>(v.clone()).ok());

        Task {
            uuid: uuid.to_string(),
            title: parse_str(p, "tt").unwrap_or_default(),
            status: TaskStatus::from(parse_i32(p, "ss", 0)),
            start: TaskStart::from(parse_i32(p, "st", 0)),
            item_type: TaskType::from(parse_i32(p, "tp", 0)),
            entity: entity.to_string(),
            notes: parse_notes(p.get("nt")),
            project: project_list.first().cloned(),
            area: area_list.first().cloned(),
            action_group: action_group_list.first().cloned(),
            tags: parse_str_list(p, "tg"),
            trashed: parse_bool(p, "tr", false),
            deadline: ts_to_dt(parse_f64(p, "dd")),
            start_date: ts_to_dt(parse_f64(p, "sr")),
            stop_date: ts_to_dt(parse_f64(p, "sp")),
            creation_date: ts_to_dt(parse_f64(p, "cd")),
            modification_date: ts_to_dt(parse_f64(p, "md")),
            index: parse_i32(p, "ix", 0),
            today_index: parse_i32(p, "ti", 0),
            today_index_reference: parse_i64(p, "tir"),
            leaves_tombstone: parse_bool(p, "lt", false),
            instance_creation_paused: parse_bool(p, "icp", false),
            evening: parse_i32(p, "sb", 0) != 0,
            recurrence_rule,
            recurrence_templates: parse_str_list(p, "rt"),
            checklist_items: Vec::new(),
        }
    }

    fn parse_checklist_item(&self, uuid: &str, p: &Map<String, Value>) -> ChecklistItem {
        let ts = parse_str_list(p, "ts");
        ChecklistItem {
            uuid: uuid.to_string(),
            title: parse_str(p, "tt").unwrap_or_default(),
            task_uuid: ts.first().cloned().unwrap_or_default(),
            status: TaskStatus::from(parse_i32(p, "ss", 0)),
            index: parse_i32(p, "ix", 0),
        }
    }

    fn parse_area(&self, uuid: &str, p: &Map<String, Value>) -> Area {
        Area {
            uuid: uuid.to_string(),
            title: parse_str(p, "tt").unwrap_or_default(),
            tags: parse_str_list(p, "tg"),
            index: parse_i32(p, "ix", 0),
        }
    }

    fn parse_tag(&self, uuid: &str, p: &Map<String, Value>) -> Tag {
        let parents = parse_str_list(p, "pn");
        Tag {
            uuid: uuid.to_string(),
            title: parse_str(p, "tt").unwrap_or_default(),
            shortcut: parse_str(p, "sh"),
            index: parse_i32(p, "ix", 0),
            parent_uuid: parents.first().cloned(),
        }
    }

    pub fn tasks(
        &self,
        status: Option<TaskStatus>,
        trashed: Option<bool>,
        item_type: Option<TaskType>,
    ) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|task| {
                if let Some(expect_trashed) = trashed
                    && task.trashed != expect_trashed
                {
                    return false;
                }
                if let Some(expect_status) = status
                    && task.status != expect_status
                {
                    return false;
                }
                if let Some(expect_type) = item_type
                    && task.item_type != expect_type
                {
                    return false;
                }
                if task.is_heading() {
                    return false;
                }
                true
            })
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn today(&self) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && !t.is_heading()
                    && !t.is_project()
                    && !t.title.trim().is_empty()
                    && t.entity == "Task6"
                    && t.is_today()
            })
            .cloned()
            .collect();

        out.sort_by_key(|task| {
            if task.today_index == 0 {
                let sr_ts = task.start_date.map(|d| d.timestamp()).unwrap_or(0);
                (0i32, Reverse(sr_ts), Reverse(task.index))
            } else {
                (1i32, Reverse(task.today_index as i64), Reverse(task.index))
            }
        });
        out
    }

    pub fn inbox(&self) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && t.start == TaskStart::Inbox
                    && self.effective_project_uuid(t).is_none()
                    && self.effective_area_uuid(t).is_none()
                    && !t.is_project()
                    && !t.is_heading()
                    && !t.title.trim().is_empty()
                    && t.creation_date.is_some()
                    && t.entity == "Task6"
            })
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn anytime(&self) -> Vec<Task> {
        let today_utc = today_utc();

        let project_visible = |task: &Task, store: &ThingsStore| {
            let Some(project_uuid) = store.effective_project_uuid(task) else {
                return true;
            };
            let Some(project) = store.tasks_by_uuid.get(&project_uuid) else {
                return true;
            };
            if project.trashed || project.status != TaskStatus::Incomplete {
                return false;
            }
            if project.start == TaskStart::Someday {
                return false;
            }
            if let Some(start_date) = project.start_date
                && start_date > today_utc
            {
                return false;
            }
            true
        };

        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && t.start == TaskStart::Anytime
                    && !t.is_project()
                    && !t.is_heading()
                    && !t.title.trim().is_empty()
                    && t.entity == "Task6"
                    && (t.start_date.is_none() || t.start_date <= Some(today_utc))
                    && project_visible(t, self)
            })
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn someday(&self) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && t.start == TaskStart::Someday
                    && !t.is_heading()
                    && !t.title.trim().is_empty()
                    && t.entity == "Task6"
                    && !t.is_recurrence_template()
                    && t.start_date.is_none()
                    && (t.is_project() || self.effective_project_uuid(t).is_none())
            })
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn logbook(
        &self,
        from_date: Option<DateTime<Local>>,
        to_date: Option<DateTime<Local>>,
    ) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|task| {
                if task.trashed
                    || !(task.status == TaskStatus::Completed
                        || task.status == TaskStatus::Canceled)
                {
                    return false;
                }
                if task.is_heading() || task.entity != "Task6" {
                    return false;
                }
                let Some(stop_date) = task.stop_date else {
                    return false;
                };

                let stop_day = stop_date
                    .with_timezone(&Local)
                    .date_naive()
                    .and_hms_opt(0, 0, 0)
                    .and_then(|d| Local.from_local_datetime(&d).single());

                if let Some(from_day) = from_date
                    && let Some(sd) = stop_day
                    && sd < from_day
                {
                    return false;
                }
                if let Some(to_day) = to_date
                    && let Some(sd) = stop_day
                    && sd > to_day
                {
                    return false;
                }

                true
            })
            .cloned()
            .collect();

        out.sort_by_key(|t| {
            (
                Reverse(t.stop_date.map(|d| d.timestamp()).unwrap_or(0)),
                Reverse(t.index),
            )
        });
        out
    }

    pub fn effective_project_uuid(&self, task: &Task) -> Option<String> {
        if let Some(project) = &task.project {
            return Some(project.clone());
        }
        if let Some(action_group) = &task.action_group
            && let Some(heading) = self.tasks_by_uuid.get(action_group)
            && let Some(project) = &heading.project
        {
            return Some(project.clone());
        }
        None
    }

    pub fn effective_area_uuid(&self, task: &Task) -> Option<String> {
        if let Some(area) = &task.area {
            return Some(area.clone());
        }

        if let Some(project_uuid) = self.effective_project_uuid(task)
            && let Some(project) = self.tasks_by_uuid.get(&project_uuid)
            && let Some(area) = &project.area
        {
            return Some(area.clone());
        }

        if let Some(action_group) = &task.action_group
            && let Some(heading) = self.tasks_by_uuid.get(action_group)
            && let Some(area) = &heading.area
        {
            return Some(area.clone());
        }

        None
    }

    pub fn projects(&self, status: Option<TaskStatus>) -> Vec<Task> {
        let mut out: Vec<Task> = self
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.is_project()
                    && t.entity == "Task6"
                    && status.map(|s| t.status == s).unwrap_or(true)
            })
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn areas(&self) -> Vec<Area> {
        let mut out: Vec<Area> = self.areas_by_uuid.values().cloned().collect();
        out.sort_by_key(|a| a.index);
        out
    }

    pub fn tags(&self) -> Vec<Tag> {
        let mut out: Vec<Tag> = self
            .tags_by_uuid
            .values()
            .filter(|t| !t.title.trim().is_empty())
            .cloned()
            .collect();
        out.sort_by_key(|t| t.index);
        out
    }

    pub fn get_task(&self, uuid: &str) -> Option<Task> {
        self.tasks_by_uuid.get(uuid).cloned()
    }

    pub fn get_area(&self, uuid: &str) -> Option<Area> {
        self.areas_by_uuid.get(uuid).cloned()
    }

    pub fn get_tag(&self, uuid: &str) -> Option<Tag> {
        self.tags_by_uuid.get(uuid).cloned()
    }

    pub fn resolve_tag_title(&self, uuid: &str) -> String {
        self.tags_by_uuid
            .get(uuid)
            .filter(|t| !t.title.trim().is_empty())
            .map(|t| t.title.clone())
            .unwrap_or_else(|| uuid.to_string())
    }

    pub fn resolve_area_title(&self, uuid: &str) -> String {
        self.areas_by_uuid
            .get(uuid)
            .map(|a| a.title.clone())
            .unwrap_or_else(|| uuid.to_string())
    }

    pub fn resolve_project_title(&self, uuid: &str) -> String {
        if let Some(task) = self.tasks_by_uuid.get(uuid)
            && !task.title.trim().is_empty()
        {
            return task.title.clone();
        }
        if uuid.is_empty() {
            return "(project)".to_string();
        }
        let short: String = uuid.chars().take(8).collect();
        format!("(project {short})")
    }

    pub fn short_id(&self, uuid: &str) -> String {
        self.short_ids
            .get(uuid)
            .cloned()
            .unwrap_or_else(|| uuid.to_string())
    }

    pub fn project_progress(&self, project_uuid: &str) -> ProjectProgress {
        self.project_progress_by_uuid
            .get(project_uuid)
            .cloned()
            .unwrap_or_default()
    }

    pub fn unique_prefix_length(&self, ids: &[String]) -> usize {
        if ids.is_empty() {
            return 0;
        }
        let mut max_need = 1usize;
        for id in ids {
            if let Some(short) = self.short_ids.get(id) {
                max_need = max_need.max(short.len());
            } else {
                max_need = max_need.max(6);
            }
        }
        max_need
    }

    fn resolve_prefix<T: Clone>(
        &self,
        identifier: &str,
        items: &HashMap<String, T>,
        sorted_ids: &[String],
        label: &str,
    ) -> (Option<T>, String, Vec<T>) {
        let ident = identifier.trim();
        if ident.is_empty() {
            return (
                None,
                format!("Missing {} identifier.", label.to_lowercase()),
                Vec::new(),
            );
        }

        if let Some(exact) = items.get(ident) {
            return (Some(exact.clone()), String::new(), Vec::new());
        }

        let matches: Vec<&String> = sorted_ids
            .iter()
            .filter(|id| id.starts_with(ident))
            .collect();
        if matches.len() == 1
            && let Some(item) = items.get(matches[0].as_str())
        {
            return (Some(item.clone()), String::new(), Vec::new());
        }

        if matches.len() > 1 {
            let mut out = Vec::new();
            for m in matches.iter().take(10) {
                if let Some(item) = items.get(m.as_str()) {
                    out.push(item.clone());
                }
            }
            let remaining = matches.len().saturating_sub(out.len());
            let mut msg = format!("Ambiguous {} id prefix.", label.to_lowercase());
            if remaining > 0 {
                msg.push_str(&format!(
                    " ({} matches, showing first {})",
                    matches.len(),
                    out.len()
                ));
            }
            return (None, msg, out);
        }

        (
            None,
            format!("{} not found: {}", label, identifier),
            Vec::new(),
        )
    }

    pub fn resolve_mark_identifier(&self, identifier: &str) -> (Option<Task>, String, Vec<Task>) {
        let markable: HashMap<String, Task> = self
            .markable_ids
            .iter()
            .filter_map(|uid| {
                self.tasks_by_uuid
                    .get(uid)
                    .map(|t| (uid.clone(), t.clone()))
            })
            .collect();
        self.resolve_prefix(identifier, &markable, &self.markable_ids_sorted, "Item")
    }

    pub fn resolve_area_identifier(&self, identifier: &str) -> (Option<Area>, String, Vec<Area>) {
        self.resolve_prefix(
            identifier,
            &self.areas_by_uuid,
            &self.area_ids_sorted,
            "Area",
        )
    }

    pub fn resolve_task_identifier(&self, identifier: &str) -> (Option<Task>, String, Vec<Task>) {
        self.resolve_prefix(
            identifier,
            &self.tasks_by_uuid,
            &self.task_ids_sorted,
            "Task",
        )
    }
}
