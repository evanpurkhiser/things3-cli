use crate::app::Cli;
use crate::cloud_writer::{CloudWriter, LiveCloudWriter};
use crate::commands::Command;
use crate::common::{
    DIM, GREEN, ICONS, colored, day_to_timestamp, parse_day, resolve_tag_ids, task6_note,
};
use crate::ids::random_task_id;
use crate::store::Task;
use crate::wire::{EntityType, OperationType, TaskStart, TaskStatus, TaskType, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde_json::{Value, json};
use std::cmp::Reverse;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct NewArgs {
    pub title: String,
    #[arg(long = "in", default_value = "inbox")]
    pub in_target: String,
    #[arg(long)]
    pub when: Option<String>,
    #[arg(long = "before")]
    pub before_id: Option<String>,
    #[arg(long = "after")]
    pub after_id: Option<String>,
    #[arg(long, default_value = "")]
    pub notes: String,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long = "deadline")]
    pub deadline_date: Option<String>,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

fn base_new_props(title: &str, now: f64) -> serde_json::Map<String, Value> {
    let mut p = serde_json::Map::new();
    p.insert("tt".to_string(), json!(title));
    p.insert("tp".to_string(), json!(i32::from(TaskType::Todo)));
    p.insert("ss".to_string(), json!(i32::from(TaskStatus::Incomplete)));
    p.insert("st".to_string(), json!(i32::from(TaskStart::Inbox)));
    p.insert("tr".to_string(), json!(false));
    p.insert("cd".to_string(), json!(now));
    p.insert("md".to_string(), json!(now));
    p.insert("nt".to_string(), Value::Null);
    p.insert("xx".to_string(), json!({"_t": "oo", "sn": {}}));
    p.insert("rmd".to_string(), Value::Null);
    p.insert("rp".to_string(), Value::Null);
    p.insert("ix".to_string(), json!(0));
    p
}

fn task_bucket(task: &Task, store: &crate::store::ThingsStore) -> Vec<String> {
    if task.is_heading() {
        return vec![
            "heading".to_string(),
            task.project.clone().unwrap_or_default(),
        ];
    }
    if task.is_project() {
        return vec!["project".to_string(), task.area.clone().unwrap_or_default()];
    }
    if let Some(project_uuid) = store.effective_project_uuid(task) {
        return vec![
            "task-project".to_string(),
            project_uuid,
            task.action_group.clone().unwrap_or_default(),
        ];
    }
    if let Some(area_uuid) = store.effective_area_uuid(task) {
        return vec![
            "task-area".to_string(),
            area_uuid,
            i32::from(task.start).to_string(),
        ];
    }
    vec!["task-root".to_string(), i32::from(task.start).to_string()]
}

fn props_bucket(props: &serde_json::Map<String, Value>) -> Vec<String> {
    if let Some(project_uuid) = props
        .get("pr")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
    {
        return vec![
            "task-project".to_string(),
            project_uuid.to_string(),
            String::new(),
        ];
    }
    if let Some(area_uuid) = props
        .get("ar")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_str)
    {
        let st = props.get("st").and_then(Value::as_i64).unwrap_or(0);
        return vec![
            "task-area".to_string(),
            area_uuid.to_string(),
            st.to_string(),
        ];
    }
    let st = props.get("st").and_then(Value::as_i64).unwrap_or(0);
    vec!["task-root".to_string(), st.to_string()]
}

fn plan_ix_insert(ordered: &[Task], insert_at: usize) -> (i32, Vec<(String, i32, String)>) {
    let prev_ix = if insert_at > 0 {
        Some(ordered[insert_at - 1].index)
    } else {
        None
    };
    let next_ix = if insert_at < ordered.len() {
        Some(ordered[insert_at].index)
    } else {
        None
    };
    let mut updates = Vec::new();

    if prev_ix.is_none() && next_ix.is_none() {
        return (0, updates);
    }
    if prev_ix.is_none() {
        return (next_ix.unwrap_or(0) - 1, updates);
    }
    if next_ix.is_none() {
        return (prev_ix.unwrap_or(0) + 1, updates);
    }
    if prev_ix.unwrap_or(0) + 1 < next_ix.unwrap_or(0) {
        return ((prev_ix.unwrap_or(0) + next_ix.unwrap_or(0)) / 2, updates);
    }

    let stride = 1024;
    let mut new_index = stride;
    let mut idx = 1;
    for i in 0..=ordered.len() {
        let target_ix = idx * stride;
        if i == insert_at {
            new_index = target_ix;
            idx += 1;
            continue;
        }
        let source_idx = if i < insert_at { i } else { i - 1 };
        if source_idx < ordered.len() {
            let entry = &ordered[source_idx];
            if entry.index != target_ix {
                updates.push((entry.uuid.clone(), target_ix, entry.entity.clone()));
            }
            idx += 1;
        }
    }
    (new_index, updates)
}

impl Command for NewArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let title = self.title.trim();
        if title.is_empty() {
            eprintln!("Task title cannot be empty.");
            return Ok(());
        }

        let store = cli.load_store()?;
        let now = now_ts();
        let mut props = base_new_props(title, now);
        if !self.notes.is_empty() {
            props.insert("nt".to_string(), task6_note(&self.notes));
        }

        let anchor_id = self.before_id.as_ref().or(self.after_id.as_ref());
        let mut anchor: Option<Task> = None;
        if let Some(anchor_id) = anchor_id {
            let (task, err, _ambiguous) = store.resolve_task_identifier(anchor_id);
            if task.is_none() {
                eprintln!("{err}");
                return Ok(());
            }
            anchor = task;
        }

        let in_target = self.in_target.trim();
        if !in_target.eq_ignore_ascii_case("inbox") {
            let (project, _, _) = store.resolve_mark_identifier(in_target);
            let (area, _, _) = store.resolve_area_identifier(in_target);
            let project_uuid = project.as_ref().and_then(|p| {
                if p.is_project() {
                    Some(p.uuid.clone())
                } else {
                    None
                }
            });
            let area_uuid = area.map(|a| a.uuid);

            if project_uuid.is_some() && area_uuid.is_some() {
                eprintln!(
                    "Ambiguous --in target '{}' (matches project and area).",
                    in_target
                );
                return Ok(());
            }

            if project.is_some() && project_uuid.is_none() {
                eprintln!("--in target must be inbox, a project ID, or an area ID.");
                return Ok(());
            }

            if let Some(project_uuid) = project_uuid {
                props.insert("pr".to_string(), json!([project_uuid]));
                props.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
            } else if let Some(area_uuid) = area_uuid {
                props.insert("ar".to_string(), json!([area_uuid]));
                props.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
            } else {
                eprintln!("Container not found: {}", in_target);
                return Ok(());
            }
        }

        if let Some(when_raw) = &self.when {
            let when = when_raw.trim();
            if when.eq_ignore_ascii_case("anytime") {
                props.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
                props.insert("sr".to_string(), Value::Null);
            } else if when.eq_ignore_ascii_case("someday") {
                props.insert("st".to_string(), json!(i32::from(TaskStart::Someday)));
                props.insert("sr".to_string(), Value::Null);
            } else if when.eq_ignore_ascii_case("today") {
                let day_ts = crate::common::today_utc().timestamp();
                props.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
                props.insert("sr".to_string(), json!(day_ts));
                props.insert("tir".to_string(), json!(day_ts));
            } else {
                let parsed = match parse_day(Some(when), "--when") {
                    Ok(Some(day)) => day,
                    Ok(None) => {
                        eprintln!("--when requires anytime, someday, today, or YYYY-MM-DD");
                        return Ok(());
                    }
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };
                let day_ts = day_to_timestamp(parsed);
                props.insert("st".to_string(), json!(i32::from(TaskStart::Someday)));
                props.insert("sr".to_string(), json!(day_ts));
                props.insert("tir".to_string(), json!(day_ts));
            }
        }

        if let Some(tags) = &self.tags {
            let (tag_ids, tag_err) = resolve_tag_ids(&store, tags);
            if !tag_err.is_empty() {
                eprintln!("{tag_err}");
                return Ok(());
            }
            props.insert("tg".to_string(), json!(tag_ids));
        }

        if let Some(deadline_date) = &self.deadline_date {
            let parsed = match parse_day(Some(deadline_date), "--deadline") {
                Ok(Some(day)) => day,
                Ok(None) => return Ok(()),
                Err(err) => {
                    eprintln!("{err}");
                    return Ok(());
                }
            };
            props.insert("dd".to_string(), json!(day_to_timestamp(parsed)));
        }

        let anchor_is_today = anchor
            .as_ref()
            .map(|a| a.start == TaskStart::Anytime && (a.is_today() || a.evening))
            .unwrap_or(false);
        let target_bucket = props_bucket(&props);

        if let Some(anchor) = &anchor
            && !anchor_is_today && task_bucket(anchor, &store) != target_bucket
        {
            eprintln!(
                "Cannot place new task relative to an item in a different container/list."
            );
            return Ok(());
        }

        let mut index_updates: Vec<(String, i32, String)> = Vec::new();
        let mut siblings = store
            .tasks_by_uuid
            .values()
            .filter(|t| {
                !t.trashed
                    && t.status == TaskStatus::Incomplete
                    && task_bucket(t, &store) == target_bucket
            })
            .cloned()
            .collect::<Vec<_>>();
        siblings.sort_by_key(|t| (t.index, t.uuid.clone()));

        let mut structural_insert_at = 0usize;
        if let Some(anchor) = &anchor
            && task_bucket(anchor, &store) == target_bucket
        {
            let anchor_pos = siblings.iter().position(|t| t.uuid == anchor.uuid);
            let Some(anchor_pos) = anchor_pos else {
                eprintln!("Anchor not found in target list.");
                return Ok(());
            };
            structural_insert_at = if self.before_id.is_some() {
                anchor_pos
            } else {
                anchor_pos + 1
            };
        }

        let (structural_ix, structural_updates) = plan_ix_insert(&siblings, structural_insert_at);
        props.insert("ix".to_string(), json!(structural_ix));
        index_updates.extend(structural_updates);

        let new_is_today = crate::common::is_today_from_props(&props);
        if new_is_today {
            let mut section_evening = if props.get("sb").and_then(Value::as_i64).unwrap_or(0) != 0 {
                1
            } else {
                0
            };

            if anchor_is_today
                && let Some(anchor) = &anchor
            {
                section_evening = if anchor.evening { 1 } else { 0 };
                props.insert("sb".to_string(), json!(section_evening));
            }

            let mut today_siblings = store
                .tasks_by_uuid
                .values()
                .filter(|t| {
                    !t.trashed
                        && t.status == TaskStatus::Incomplete
                        && t.start == TaskStart::Anytime
                        && (t.is_today() || t.evening)
                        && (if t.evening { 1 } else { 0 }) == section_evening
                })
                .cloned()
                .collect::<Vec<_>>();
            today_siblings.sort_by_key(|task| {
                let tir = task.today_index_reference.unwrap_or(0);
                (Reverse(tir), task.today_index, Reverse(task.index))
            });

            let mut today_insert_at = 0usize;
            if anchor_is_today
                && let Some(anchor) = &anchor
                && (if anchor.evening { 1 } else { 0 }) == section_evening
                && let Some(anchor_pos) =
                    today_siblings.iter().position(|t| t.uuid == anchor.uuid)
            {
                today_insert_at = if self.before_id.is_some() {
                    anchor_pos
                } else {
                    anchor_pos + 1
                };
            }

            let prev_today = if today_insert_at > 0 {
                today_siblings.get(today_insert_at - 1)
            } else {
                None
            };
            let next_today = today_siblings.get(today_insert_at);

            let today_ts = crate::common::today_utc().timestamp();

            if let Some(next_today) = next_today {
                let next_tir = next_today.today_index_reference.unwrap_or(today_ts);
                props.insert("tir".to_string(), json!(next_tir));
                props.insert("ti".to_string(), json!(next_today.today_index - 1));
            } else if let Some(prev_today) = prev_today {
                let prev_tir = prev_today.today_index_reference.unwrap_or(today_ts);
                props.insert("tir".to_string(), json!(prev_tir));
                props.insert("ti".to_string(), json!(prev_today.today_index + 1));
            } else {
                props.insert("tir".to_string(), json!(today_ts));
                props.insert("ti".to_string(), json!(0));
            }
        }

        let new_uuid = random_task_id();
        let mut writer = LiveCloudWriter::new()?;

        let mut changes = BTreeMap::new();
        changes.insert(
            new_uuid.clone(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Task6),
                properties: props.clone().into_iter().collect(),
            },
        );

        for (task_uuid, task_index, task_entity) in index_updates {
            let mut p = BTreeMap::new();
            p.insert("ix".to_string(), json!(task_index));
            p.insert("md".to_string(), json!(now));
            changes.insert(
                task_uuid,
                WireObject {
                    operation_type: OperationType::Update,
                    entity_type: Some(EntityType::from(task_entity)),
                    properties: p,
                },
            );
        }

        if let Err(e) = writer.commit(changes, None) {
            eprintln!("Failed to create task: {e}");
            return Ok(());
        }

        writeln!(
            out,
            "{} {}  {}",
            colored(&format!("{} Created", ICONS.done), &[GREEN], cli.no_color),
            title,
            colored(&new_uuid, &[DIM], cli.no_color)
        )?;
        Ok(())
    }
}
