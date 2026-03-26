use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{DIM, GREEN, ICONS, colored, resolve_tag_ids, task6_note};
use crate::ids::random_task_id;
use crate::wire::{EntityType, OperationType, TaskStart, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Args)]
pub struct EditArgs {
    pub task_ids: Vec<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub notes: Option<String>,
    #[arg(long = "move")]
    pub move_target: Option<String>,
    #[arg(long = "add-tags")]
    pub add_tags: Option<String>,
    #[arg(long = "remove-tags")]
    pub remove_tags: Option<String>,
    #[arg(long = "add-checklist")]
    pub add_checklist: Vec<String>,
    #[arg(long = "remove-checklist")]
    pub remove_checklist: Option<String>,
    #[arg(long = "rename-checklist")]
    pub rename_checklist: Vec<String>,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

fn resolve_checklist_items(
    task: &crate::store::Task,
    raw_ids: &str,
) -> (Vec<crate::store::ChecklistItem>, String) {
    let tokens = raw_ids
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return (Vec::new(), "No checklist item IDs provided.".to_string());
    }

    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
    for token in tokens {
        let matches = task
            .checklist_items
            .iter()
            .filter(|item| item.uuid.starts_with(token))
            .cloned()
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return (Vec::new(), format!("Checklist item not found: '{token}'"));
        }
        if matches.len() > 1 {
            return (
                Vec::new(),
                format!("Ambiguous checklist item prefix: '{token}'"),
            );
        }
        let item = matches[0].clone();
        if seen.insert(item.uuid.clone()) {
            resolved.push(item);
        }
    }

    (resolved, String::new())
}

impl Command for EditArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let multiple = self.task_ids.len() > 1;
        if multiple && self.title.is_some() {
            eprintln!("--title requires a single task ID.");
            return Ok(());
        }
        if multiple && self.notes.is_some() {
            eprintln!("--notes requires a single task ID.");
            return Ok(());
        }
        if multiple
            && (!self.add_checklist.is_empty()
                || self.remove_checklist.is_some()
                || !self.rename_checklist.is_empty())
        {
            eprintln!(
                "--add-checklist/--remove-checklist/--rename-checklist require a single task ID."
            );
            return Ok(());
        }

        let store = cli.load_store()?;
        let mut tasks = Vec::new();
        for identifier in &self.task_ids {
            let (task_opt, err, _) = store.resolve_mark_identifier(identifier);
            let Some(task) = task_opt else {
                eprintln!("{err}");
                return Ok(());
            };
            if task.is_project() {
                eprintln!("Use 'projects edit' to edit a project.");
                return Ok(());
            }
            tasks.push(task);
        }

        let mut shared_update: serde_json::Map<String, Value> = serde_json::Map::new();
        let mut labels: Vec<String> = Vec::new();
        let move_raw = self.move_target.clone().unwrap_or_default();
        let move_l = move_raw.to_lowercase();

        if !move_raw.trim().is_empty() {
            if move_l == "inbox" {
                shared_update.insert("pr".to_string(), json!([]));
                shared_update.insert("ar".to_string(), json!([]));
                shared_update.insert("agr".to_string(), json!([]));
                shared_update.insert("st".to_string(), json!(i32::from(TaskStart::Inbox)));
                shared_update.insert("sr".to_string(), Value::Null);
                shared_update.insert("tir".to_string(), Value::Null);
                shared_update.insert("sb".to_string(), json!(0));
                labels.push("move=inbox".to_string());
            } else if move_l == "clear" {
                labels.push("move=clear".to_string());
            } else {
                let (project_opt, _, _) = store.resolve_mark_identifier(&move_raw);
                let (area_opt, _, _) = store.resolve_area_identifier(&move_raw);

                let project_uuid = project_opt.as_ref().and_then(|p| {
                    if p.is_project() {
                        Some(p.uuid.clone())
                    } else {
                        None
                    }
                });
                let area_uuid = area_opt.as_ref().map(|a| a.uuid.clone());

                if project_uuid.is_some() && area_uuid.is_some() {
                    eprintln!(
                        "Ambiguous --move target '{}' (matches project and area).",
                        move_raw
                    );
                    return Ok(());
                }
                if project_opt.is_some() && project_uuid.is_none() {
                    eprintln!("--move target must be Inbox, clear, a project ID, or an area ID.");
                    return Ok(());
                }

                if let Some(project_uuid) = project_uuid {
                    shared_update.insert("pr".to_string(), json!([project_uuid]));
                    shared_update.insert("ar".to_string(), json!([]));
                    shared_update.insert("agr".to_string(), json!([]));
                    shared_update.insert(
                        "_move_from_inbox_st".to_string(),
                        json!(i32::from(TaskStart::Anytime)),
                    );
                    labels.push(format!("move={move_raw}"));
                } else if let Some(area_uuid) = area_uuid {
                    shared_update.insert("ar".to_string(), json!([area_uuid]));
                    shared_update.insert("pr".to_string(), json!([]));
                    shared_update.insert("agr".to_string(), json!([]));
                    shared_update.insert(
                        "_move_from_inbox_st".to_string(),
                        json!(i32::from(TaskStart::Anytime)),
                    );
                    labels.push(format!("move={move_raw}"));
                } else {
                    eprintln!("Container not found: {move_raw}");
                    return Ok(());
                }
            }
        }

        let mut add_tag_ids = Vec::new();
        let mut remove_tag_ids = Vec::new();
        if let Some(raw) = &self.add_tags {
            let (ids, err) = resolve_tag_ids(&store, raw);
            if !err.is_empty() {
                eprintln!("{err}");
                return Ok(());
            }
            add_tag_ids = ids;
            labels.push("add-tags".to_string());
        }
        if let Some(raw) = &self.remove_tags {
            let (ids, err) = resolve_tag_ids(&store, raw);
            if !err.is_empty() {
                eprintln!("{err}");
                return Ok(());
            }
            remove_tag_ids = ids;
            if !labels.iter().any(|l| l == "remove-tags") {
                labels.push("remove-tags".to_string());
            }
        }

        let mut rename_map: HashMap<String, String> = HashMap::new();
        for token in &self.rename_checklist {
            let Some((short_id, new_title)) = token.split_once(':') else {
                eprintln!("--rename-checklist requires 'id:new title' format, got: {token:?}");
                return Ok(());
            };
            let short_id = short_id.trim();
            let new_title = new_title.trim();
            if short_id.is_empty() || new_title.is_empty() {
                eprintln!("--rename-checklist requires 'id:new title' format, got: {token:?}");
                return Ok(());
            }
            rename_map.insert(short_id.to_string(), new_title.to_string());
        }

        let now = now_ts();
        let mut changes: BTreeMap<String, WireObject> = BTreeMap::new();

        for task in &tasks {
            let mut update = shared_update.clone();

            if let Some(title) = &self.title {
                let title = title.trim();
                if title.is_empty() {
                    eprintln!("Task title cannot be empty.");
                    return Ok(());
                }
                update.insert("tt".to_string(), json!(title));
                if !labels.iter().any(|l| l == "title") {
                    labels.push("title".to_string());
                }
            }

            if let Some(notes) = &self.notes {
                if notes.is_empty() {
                    update.insert(
                        "nt".to_string(),
                        json!({"_t": "tx", "t": 1, "ch": 0, "v": ""}),
                    );
                } else {
                    update.insert("nt".to_string(), task6_note(notes));
                }
                if !labels.iter().any(|l| l == "notes") {
                    labels.push("notes".to_string());
                }
            }

            if move_l == "clear" {
                update.insert("pr".to_string(), json!([]));
                update.insert("ar".to_string(), json!([]));
                update.insert("agr".to_string(), json!([]));
                if task.start == TaskStart::Inbox {
                    update.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
                }
            }

            if let Some(move_from_inbox_st) = update.get("_move_from_inbox_st").cloned() {
                if task.start == TaskStart::Inbox {
                    update.insert("st".to_string(), move_from_inbox_st);
                }
                update.remove("_move_from_inbox_st");
            }

            if !add_tag_ids.is_empty() || !remove_tag_ids.is_empty() {
                let mut current = task.tags.clone();
                for uuid in &add_tag_ids {
                    if !current.iter().any(|c| c == uuid) {
                        current.push(uuid.clone());
                    }
                }
                current.retain(|uuid| !remove_tag_ids.iter().any(|r| r == uuid));
                update.insert("tg".to_string(), json!(current));
            }

            if let Some(remove_raw) = &self.remove_checklist {
                let (items, err) = resolve_checklist_items(task, remove_raw);
                if !err.is_empty() {
                    eprintln!("{err}");
                    return Ok(());
                }
                for uuid in items.into_iter().map(|i| i.uuid).collect::<HashSet<_>>() {
                    changes.insert(
                        uuid,
                        WireObject {
                            operation_type: OperationType::Delete,
                            entity_type: Some(EntityType::ChecklistItem3),
                            properties: BTreeMap::new(),
                        },
                    );
                }
                if !labels.iter().any(|l| l == "remove-checklist") {
                    labels.push("remove-checklist".to_string());
                }
            }

            if !rename_map.is_empty() {
                for (short_id, new_title) in &rename_map {
                    let matches = task
                        .checklist_items
                        .iter()
                        .filter(|i| i.uuid.starts_with(short_id))
                        .cloned()
                        .collect::<Vec<_>>();
                    if matches.is_empty() {
                        eprintln!("Checklist item not found: '{short_id}'");
                        return Ok(());
                    }
                    if matches.len() > 1 {
                        eprintln!("Ambiguous checklist item prefix: '{short_id}'");
                        return Ok(());
                    }
                    let mut p = BTreeMap::new();
                    p.insert("tt".to_string(), json!(new_title));
                    p.insert("md".to_string(), json!(now));
                    changes.insert(
                        matches[0].uuid.clone(),
                        WireObject {
                            operation_type: OperationType::Update,
                            entity_type: Some(EntityType::ChecklistItem3),
                            properties: p,
                        },
                    );
                }
                if !labels.iter().any(|l| l == "rename-checklist") {
                    labels.push("rename-checklist".to_string());
                }
            }

            if !self.add_checklist.is_empty() {
                let max_ix = task
                    .checklist_items
                    .iter()
                    .map(|i| i.index)
                    .max()
                    .unwrap_or(0);
                for (idx, title) in self.add_checklist.iter().enumerate() {
                    let title = title.trim();
                    if title.is_empty() {
                        eprintln!("Checklist item title cannot be empty.");
                        return Ok(());
                    }
                    let mut p = BTreeMap::new();
                    p.insert("tt".to_string(), json!(title));
                    p.insert("ts".to_string(), json!([task.uuid.clone()]));
                    p.insert("ss".to_string(), json!(0));
                    p.insert("ix".to_string(), json!(max_ix + idx as i32 + 1));
                    p.insert("cd".to_string(), json!(now));
                    p.insert("md".to_string(), json!(now));

                    changes.insert(
                        random_task_id(),
                        WireObject {
                            operation_type: OperationType::Create,
                            entity_type: Some(EntityType::ChecklistItem3),
                            properties: p,
                        },
                    );
                }
                if !labels.iter().any(|l| l == "add-checklist") {
                    labels.push("add-checklist".to_string());
                }
            }

            let has_checklist_changes = !self.add_checklist.is_empty()
                || self.remove_checklist.is_some()
                || !rename_map.is_empty();
            if update.is_empty() && !has_checklist_changes {
                eprintln!("No edit changes requested.");
                return Ok(());
            }

            if !update.is_empty() {
                update.insert("md".to_string(), json!(now));
                changes.insert(
                    task.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::from(task.entity.clone())),
                        properties: update.into_iter().collect(),
                    },
                );
            }
        }

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let _ = client.authenticate();
        if let Err(e) = client.commit(changes.clone(), None) {
            eprintln!("Failed to edit item: {e}");
            return Ok(());
        }

        let label_str = colored(&format!("({})", labels.join(", ")), &[DIM], cli.no_color);
        for task in tasks {
            let title_display = changes
                .get(&task.uuid)
                .and_then(|obj| obj.properties.get("tt"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or(task.title);
            writeln!(
                out,
                "{} {}  {} {}",
                colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                title_display,
                colored(&task.uuid, &[DIM], cli.no_color),
                label_str
            )?;
        }

        Ok(())
    }
}
