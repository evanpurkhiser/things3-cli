use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::{
    ids::ThingsId,
    store::{Area, Tag, Task, ThingsStore},
    wire::task::{TaskStart, TaskStatus, TaskType},
};

#[derive(Debug, Serialize)]
pub struct ResolvedTaskJson {
    #[serde(flatten)]
    core: TaskCoreJson,
    #[serde(flatten)]
    links: TaskLinksJson,
    dates: TaskDatesJson,
    notes: Option<String>,
    checklist: Vec<ResolvedChecklistItemJson>,
    recurrence: TaskRecurrenceJson,
    flags: TaskFlagsJson,
    indexes: TaskIndexesJson,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<TaskProgressJson>,
}

#[derive(Debug, Serialize)]
pub struct ResolvedAreaJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub index: i32,
    pub tags: Vec<TagRefJson>,
}

#[derive(Debug, Serialize)]
pub struct ResolvedTagJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub shortcut: Option<String>,
    pub parent: Option<LinkRefJson>,
    pub index: i32,
}

#[derive(Debug, Serialize)]
struct TaskProgressJson {
    done: i32,
    total: i32,
}

#[derive(Debug, Serialize)]
pub struct TaskCoreJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub status: JsonTaskStatus,
    pub r#type: JsonItemType,
    pub start: TaskStartJson,
}

#[derive(Debug, Serialize)]
pub struct TaskLinksJson {
    pub project: Option<LinkRefJson>,
    pub area: Option<LinkRefJson>,
    pub heading: Option<LinkRefJson>,
    pub tags: Vec<TagRefJson>,
}

#[derive(Debug, Serialize)]
pub struct TaskStartJson {
    pub bucket: JsonStartBucket,
    pub scheduled_at: Option<String>,
    pub today_index_reference: Option<i64>,
    pub evening: bool,
}

#[derive(Debug, Serialize)]
pub struct TaskDatesJson {
    pub deadline_at: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskRecurrenceJson {
    pub is_template: bool,
    pub is_instance: bool,
    pub rule: Option<serde_json::Value>,
    pub template_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskFlagsJson {
    pub trashed: bool,
    pub is_new: bool,
    pub instance_creation_paused: bool,
    pub leaves_tombstone: bool,
}

#[derive(Debug, Serialize)]
pub struct TaskIndexesJson {
    pub sort_index: i32,
    pub today_sort_index: i32,
}

#[derive(Debug, Serialize)]
pub struct LinkRefJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct TagRefJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub shortcut: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResolvedChecklistItemJson {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub status: JsonTaskStatus,
    pub index: i32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonTaskStatus {
    Incomplete,
    Completed,
    Canceled,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonStartBucket {
    Inbox,
    Anytime,
    Someday,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JsonItemType {
    Todo,
    Project,
}

pub fn link_ref(id: &str, title: String, store: &ThingsStore) -> LinkRefJson {
    LinkRefJson {
        id: id.to_string(),
        short_id: store.short_id(id),
        title,
    }
}

pub fn resolve_heading_title(id: &ThingsId, store: &ThingsStore) -> String {
    store
        .tasks_by_uuid
        .get(id)
        .map(|task| task.title.clone())
        .unwrap_or_else(|| id.to_string())
}

pub fn task_status_json(status: TaskStatus) -> JsonTaskStatus {
    match status {
        TaskStatus::Incomplete => JsonTaskStatus::Incomplete,
        TaskStatus::Completed => JsonTaskStatus::Completed,
        TaskStatus::Canceled => JsonTaskStatus::Canceled,
        TaskStatus::Unknown(_) => JsonTaskStatus::Incomplete,
    }
}

pub fn task_start_json(start: TaskStart) -> JsonStartBucket {
    match start {
        TaskStart::Inbox => JsonStartBucket::Inbox,
        TaskStart::Anytime => JsonStartBucket::Anytime,
        TaskStart::Someday => JsonStartBucket::Someday,
        TaskStart::Unknown(_) => JsonStartBucket::Inbox,
    }
}

pub fn task_type_json(item_type: TaskType) -> JsonItemType {
    match item_type {
        TaskType::Project => JsonItemType::Project,
        TaskType::Todo | TaskType::Heading | TaskType::Unknown(_) => JsonItemType::Todo,
    }
}

pub fn build_tasks_json(
    tasks: &[Task],
    store: &ThingsStore,
    today: &DateTime<Utc>,
) -> Vec<ResolvedTaskJson> {
    tasks
        .iter()
        .map(|task| task_to_json(task, store, today))
        .collect()
}

pub fn build_area_json(area: &Area, store: &ThingsStore) -> ResolvedAreaJson {
    area_to_json(area, store)
}

pub fn build_tags_json(tags: &[Tag], store: &ThingsStore) -> Vec<ResolvedTagJson> {
    tags.iter().map(|tag| tag_to_json(tag, store)).collect()
}

fn task_to_json(task: &Task, store: &ThingsStore, today: &DateTime<Utc>) -> ResolvedTaskJson {
    ResolvedTaskJson {
        core: TaskCoreJson {
            id: task.uuid.to_string(),
            short_id: store.short_id(&task.uuid),
            title: task.title.clone(),
            status: task_status_json(task.status),
            r#type: task_type_json(task.item_type),
            start: TaskStartJson {
                bucket: task_start_json(task.start),
                scheduled_at: task.start_date.map(|d| d.to_rfc3339()),
                today_index_reference: task.today_index_reference,
                evening: task.evening,
            },
        },
        links: TaskLinksJson {
            project: store
                .effective_project_uuid(task)
                .map(|id| link_ref(&id.to_string(), store.resolve_project_title(&id), store)),
            area: store
                .effective_area_uuid(task)
                .map(|id| link_ref(&id.to_string(), store.resolve_area_title(&id), store)),
            heading: task
                .action_group
                .as_ref()
                .map(|id| link_ref(&id.to_string(), resolve_heading_title(id, store), store)),
            tags: task
                .tags
                .iter()
                .map(|tag_id| {
                    let tag = store.tags_by_uuid.get(tag_id);
                    TagRefJson {
                        id: tag_id.to_string(),
                        short_id: store.short_id(tag_id),
                        title: tag.map(|t| t.title.clone()).unwrap_or_default(),
                        shortcut: tag.and_then(|t| t.shortcut.clone()),
                    }
                })
                .collect(),
        },
        dates: TaskDatesJson {
            deadline_at: task.deadline.map(|d| d.to_rfc3339()),
            created_at: task.creation_date.map(|d| d.to_rfc3339()),
            modified_at: task.modification_date.map(|d| d.to_rfc3339()),
            completed_at: task.stop_date.map(|d| d.to_rfc3339()),
        },
        notes: task.notes.clone(),
        checklist: task
            .checklist_items
            .iter()
            .map(|item| ResolvedChecklistItemJson {
                id: item.uuid.to_string(),
                short_id: store.short_id(&item.uuid),
                title: item.title.clone(),
                status: task_status_json(item.status),
                index: item.index,
            })
            .collect(),
        recurrence: TaskRecurrenceJson {
            is_template: task.is_recurrence_template(),
            is_instance: task.is_recurrence_instance(),
            rule: task
                .recurrence_rule
                .as_ref()
                .map(|rule| serde_json::to_value(rule).unwrap_or(serde_json::Value::Null)),
            template_ids: task
                .recurrence_templates
                .iter()
                .map(ToString::to_string)
                .collect(),
        },
        flags: TaskFlagsJson {
            trashed: task.trashed,
            is_new: task.is_staged_for_today(today),
            instance_creation_paused: task.instance_creation_paused,
            leaves_tombstone: task.leaves_tombstone,
        },
        indexes: TaskIndexesJson {
            sort_index: task.index,
            today_sort_index: task.today_index,
        },
        progress: if task.is_project() {
            let progress = store.project_progress(&task.uuid);
            Some(TaskProgressJson {
                done: progress.done,
                total: progress.total,
            })
        } else {
            None
        },
    }
}

fn area_to_json(area: &Area, store: &ThingsStore) -> ResolvedAreaJson {
    ResolvedAreaJson {
        id: area.uuid.to_string(),
        short_id: store.short_id(&area.uuid),
        title: area.title.clone(),
        index: area.index,
        tags: area
            .tags
            .iter()
            .map(|tag_id| {
                let tag = store.tags_by_uuid.get(tag_id);
                TagRefJson {
                    id: tag_id.to_string(),
                    short_id: store.short_id(tag_id),
                    title: tag.map(|t| t.title.clone()).unwrap_or_default(),
                    shortcut: tag.and_then(|t| t.shortcut.clone()),
                }
            })
            .collect(),
    }
}

fn tag_to_json(tag: &Tag, store: &ThingsStore) -> ResolvedTagJson {
    ResolvedTagJson {
        id: tag.uuid.to_string(),
        short_id: store.short_id(&tag.uuid),
        title: tag.title.clone(),
        shortcut: tag.shortcut.clone(),
        parent: tag.parent_uuid.as_ref().map(|id| {
            link_ref(
                &id.to_string(),
                store.resolve_tag_title(id.to_string()),
                store,
            )
        }),
        index: tag.index,
    }
}
