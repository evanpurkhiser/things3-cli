use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{DIM, GREEN, ICONS, colored};
use crate::wire::{EntityType, OperationType, TaskStart, TaskStatus, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde_json::json;
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct ReorderArgs {
    pub item_id: String,
    #[arg(long)]
    pub before_id: Option<String>,
    #[arg(long)]
    pub after_id: Option<String>,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

fn now_day_ts() -> i64 {
    crate::common::today_utc().timestamp()
}

impl Command for ReorderArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let store = cli.load_store()?;
        let (item_opt, err, _) = store.resolve_task_identifier(&self.item_id);
        let Some(item) = item_opt else {
            eprintln!("{err}");
            return Ok(());
        };

        let anchor_id = self
            .before_id
            .as_ref()
            .or(self.after_id.as_ref())
            .cloned()
            .unwrap_or_default();
        let (anchor_opt, err, _) = store.resolve_task_identifier(&anchor_id);
        let Some(anchor) = anchor_opt else {
            eprintln!("{err}");
            return Ok(());
        };

        if item.uuid == anchor.uuid {
            eprintln!("Cannot reorder an item relative to itself.");
            return Ok(());
        }

        let is_today_orderable = |task: &crate::store::Task| {
            task.start == TaskStart::Anytime && (task.is_today() || task.evening)
        };

        let is_today_reorder = is_today_orderable(&item) && is_today_orderable(&anchor);
        let reorder_label: String;

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let _ = client.authenticate();

        if is_today_reorder {
            let anchor_tir = anchor
                .today_index_reference
                .or_else(|| anchor.start_date.map(|d| d.timestamp()))
                .unwrap_or(now_day_ts());
            let new_ti = if self.before_id.is_some() {
                anchor.today_index - 1
            } else {
                anchor.today_index + 1
            };

            let mut props = BTreeMap::new();
            props.insert("tir".to_string(), json!(anchor_tir));
            props.insert("ti".to_string(), json!(new_ti));
            if item.evening != anchor.evening {
                props.insert("sb".to_string(), json!(if anchor.evening { 1 } else { 0 }));
            }
            props.insert("md".to_string(), json!(now_ts()));

            let mut changes = BTreeMap::new();
            changes.insert(
                item.uuid.clone(),
                WireObject {
                    operation_type: OperationType::Update,
                    entity_type: Some(EntityType::from(item.entity.clone())),
                    properties: props,
                },
            );

            if let Err(e) = client.commit(changes, None) {
                eprintln!("Failed to reorder item: {e}");
                return Ok(());
            }

            reorder_label = if self.before_id.is_some() {
                format!(
                    "(before={}, today_ref={}, today_index={})",
                    anchor.title, anchor_tir, new_ti
                )
            } else {
                format!(
                    "(after={}, today_ref={}, today_index={})",
                    anchor.title, anchor_tir, new_ti
                )
            };
        } else {
            let bucket = |task: &crate::store::Task| -> Vec<String> {
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
            };

            let item_bucket = bucket(&item);
            let anchor_bucket = bucket(&anchor);
            if item_bucket != anchor_bucket {
                eprintln!("Cannot reorder across different containers/lists.");
                return Ok(());
            }

            let mut siblings = store
                .tasks_by_uuid
                .values()
                .filter(|t| {
                    !t.trashed && t.status == TaskStatus::Incomplete && bucket(t) == item_bucket
                })
                .cloned()
                .collect::<Vec<_>>();
            siblings.sort_by(|a, b| match a.index.cmp(&b.index) {
                Ordering::Equal => a.uuid.cmp(&b.uuid),
                other => other,
            });

            let by_uuid = siblings
                .iter()
                .map(|t| (t.uuid.clone(), t.clone()))
                .collect::<BTreeMap<_, _>>();
            if !by_uuid.contains_key(&item.uuid) || !by_uuid.contains_key(&anchor.uuid) {
                eprintln!("Cannot reorder item in the selected list.");
                return Ok(());
            }

            let mut order = siblings
                .into_iter()
                .filter(|t| t.uuid != item.uuid)
                .collect::<Vec<_>>();
            let anchor_pos = order.iter().position(|t| t.uuid == anchor.uuid);
            let Some(anchor_pos) = anchor_pos else {
                eprintln!("Anchor not found in reorder list.");
                return Ok(());
            };
            let insert_at = if self.before_id.is_some() {
                anchor_pos
            } else {
                anchor_pos + 1
            };
            order.insert(insert_at, item.clone());

            let moved_pos = order.iter().position(|t| t.uuid == item.uuid).unwrap_or(0);
            let prev_ix = if moved_pos > 0 {
                Some(order[moved_pos - 1].index)
            } else {
                None
            };
            let next_ix = if moved_pos + 1 < order.len() {
                Some(order[moved_pos + 1].index)
            } else {
                None
            };

            let mut index_updates: Vec<(String, i32, String)> = Vec::new();
            let new_index = if prev_ix.is_none() && next_ix.is_none() {
                0
            } else if prev_ix.is_none() {
                next_ix.unwrap_or(0) - 1
            } else if next_ix.is_none() {
                prev_ix.unwrap_or(0) + 1
            } else if prev_ix.unwrap_or(0) + 1 < next_ix.unwrap_or(0) {
                (prev_ix.unwrap_or(0) + next_ix.unwrap_or(0)) / 2
            } else {
                let stride = 1024;
                for (idx, task) in order.iter().enumerate() {
                    let target_ix = (idx as i32 + 1) * stride;
                    if task.index != target_ix {
                        index_updates.push((task.uuid.clone(), target_ix, task.entity.clone()));
                    }
                }
                index_updates
                    .iter()
                    .find(|(uid, _, _)| uid == &item.uuid)
                    .map(|(_, ix, _)| *ix)
                    .unwrap_or(item.index)
            };

            if index_updates.is_empty() && new_index != item.index {
                index_updates.push((item.uuid.clone(), new_index, item.entity.clone()));
            }

            let mut ancestor_index = None;
            for (task_uuid, task_index, task_entity) in index_updates {
                let mut props = BTreeMap::new();
                props.insert("ix".to_string(), json!(task_index));
                props.insert("md".to_string(), json!(now_ts()));
                let mut changes = BTreeMap::new();
                changes.insert(
                    task_uuid,
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::from(task_entity)),
                        properties: props,
                    },
                );
                if let Err(e) = client.commit(changes, ancestor_index) {
                    eprintln!("Failed to reorder item: {e}");
                    return Ok(());
                }
                ancestor_index = Some(client.head_index);
            }

            reorder_label = if self.before_id.is_some() {
                format!("(before={}, index={})", anchor.title, new_index)
            } else {
                format!("(after={}, index={})", anchor.title, new_index)
            };
        }

        writeln!(
            out,
            "{} {}  {} {}",
            colored(&format!("{} Reordered", ICONS.done), &[GREEN], cli.no_color),
            item.title,
            colored(&item.uuid, &[DIM], cli.no_color),
            colored(&reorder_label, &[DIM], cli.no_color)
        )?;

        Ok(())
    }
}
