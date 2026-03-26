use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{DIM, GREEN, ICONS, colored};
use crate::wire::{EntityType, OperationType, TaskStatus, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::Args;
use serde_json::json;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Args)]
pub struct MarkArgs {
    pub task_ids: Vec<String>,
    #[arg(long)]
    pub done: bool,
    #[arg(long)]
    pub incomplete: bool,
    #[arg(long)]
    pub canceled: bool,
    #[arg(long = "check")]
    pub check_ids: Option<String>,
    #[arg(long = "uncheck")]
    pub uncheck_ids: Option<String>,
    #[arg(long = "check-cancel")]
    pub check_cancel_ids: Option<String>,
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

fn validate_recurring_done(
    task: &crate::store::Task,
    store: &crate::store::ThingsStore,
) -> (bool, String) {
    if task.is_recurrence_template() {
        return (
            false,
            "Recurring template tasks are blocked for done (template progression bookkeeping is not implemented).".to_string(),
        );
    }

    if !task.is_recurrence_instance() {
        return (
            false,
            "Recurring task shape is unsupported (expected an instance with rt set and rr unset)."
                .to_string(),
        );
    }

    if task.recurrence_templates.len() != 1 {
        return (
            false,
            format!(
                "Recurring instance has {} template references; expected exactly 1.",
                task.recurrence_templates.len()
            ),
        );
    }

    let template_uuid = &task.recurrence_templates[0];
    let Some(template) = store.get_task(template_uuid) else {
        return (
            false,
            format!(
                "Recurring instance template {} is missing from current state.",
                template_uuid
            ),
        );
    };

    let Some(rr) = template.recurrence_rule else {
        return (
            false,
            "Recurring instance template has unsupported recurrence rule shape (expected dict)."
                .to_string(),
        );
    };

    match rr.repeat_type {
        crate::wire::RecurrenceType::FixedSchedule => (true, String::new()),
        crate::wire::RecurrenceType::AfterCompletion => (
            false,
            "Recurring 'after completion' templates (rr.tp=1) are blocked: completion requires coupled template writes (acrd/tir) not implemented yet.".to_string(),
        ),
        crate::wire::RecurrenceType::Unknown(v) => (
            false,
            format!("Recurring template type rr.tp={v:?} is unsupported for safe completion."),
        ),
    }
}

fn validate_mark_target(
    task: &crate::store::Task,
    action: &str,
    store: &crate::store::ThingsStore,
) -> String {
    if task.entity != "Task6" {
        return "Only Task6 tasks are supported by mark right now.".to_string();
    }
    if task.is_heading() {
        return "Headings cannot be marked.".to_string();
    }
    if task.trashed {
        return "Task is in Trash and cannot be completed.".to_string();
    }
    if action == "done" && task.status == TaskStatus::Completed {
        return "Task is already completed.".to_string();
    }
    if action == "incomplete" && task.status == TaskStatus::Incomplete {
        return "Task is already incomplete/open.".to_string();
    }
    if action == "canceled" && task.status == TaskStatus::Canceled {
        return "Task is already canceled.".to_string();
    }
    if action == "done" && (task.is_recurrence_instance() || task.is_recurrence_template()) {
        let (ok, reason) = validate_recurring_done(task, store);
        if !ok {
            return reason;
        }
    }
    String::new()
}

impl Command for MarkArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let store = cli.load_store()?;
        let checklist_raw = self
            .check_ids
            .as_ref()
            .or(self.uncheck_ids.as_ref())
            .or(self.check_cancel_ids.as_ref());

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let _ = client.authenticate();

        if let Some(checklist_raw) = checklist_raw {
            if self.task_ids.len() != 1 {
                eprintln!(
                    "Checklist flags (--check, --uncheck, --check-cancel) require exactly one task ID."
                );
                return Ok(());
            }

            let (task_opt, err, _) = store.resolve_mark_identifier(&self.task_ids[0]);
            let Some(task) = task_opt else {
                eprintln!("{err}");
                return Ok(());
            };

            if task.checklist_items.is_empty() {
                eprintln!("Task has no checklist items: {}", task.title);
                return Ok(());
            }

            let (items, err) = resolve_checklist_items(&task, checklist_raw);
            if !err.is_empty() {
                eprintln!("{err}");
                return Ok(());
            }

            let (label, status): (&str, i32) = if self.check_ids.is_some() {
                ("checked", 3)
            } else if self.uncheck_ids.is_some() {
                ("unchecked", 0)
            } else {
                ("canceled", 2)
            };

            let now = now_ts();
            let stop_date = if status == 3 || status == 2 {
                Some(now)
            } else {
                None
            };

            let mut changes = BTreeMap::new();
            for item in &items {
                let mut props = BTreeMap::new();
                props.insert("ss".to_string(), json!(status));
                props.insert("sp".to_string(), json!(stop_date));
                props.insert("md".to_string(), json!(now));
                changes.insert(
                    item.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::ChecklistItem3),
                        properties: props,
                    },
                );
            }

            if let Err(e) = client.commit(changes, None) {
                eprintln!("Failed to mark checklist items: {e}");
                return Ok(());
            }

            let title = match label {
                "checked" => format!("{} Checked", ICONS.checklist_done),
                "unchecked" => format!("{} Unchecked", ICONS.checklist_open),
                _ => format!("{} Canceled", ICONS.checklist_canceled),
            };

            for item in items {
                writeln!(
                    out,
                    "{} {}  {}",
                    colored(&title, &[GREEN], cli.no_color),
                    item.title,
                    colored(&item.uuid, &[DIM], cli.no_color)
                )?;
            }
            return Ok(());
        }

        let action = if self.done {
            "done"
        } else if self.incomplete {
            "incomplete"
        } else {
            "canceled"
        };

        let mut targets = Vec::new();
        let mut seen = HashSet::new();
        for identifier in &self.task_ids {
            let (task_opt, err, _) = store.resolve_mark_identifier(identifier);
            let Some(task) = task_opt else {
                eprintln!("{err}");
                continue;
            };
            if !seen.insert(task.uuid.clone()) {
                continue;
            }
            targets.push(task);
        }

        let mut updates = Vec::new();
        let mut successes = Vec::new();

        for task in targets {
            let validation_error = validate_mark_target(&task, action, &store);
            if !validation_error.is_empty() {
                eprintln!("{} ({})", validation_error, task.title);
                continue;
            }

            let stop_date = if action == "done" || action == "canceled" {
                Some(now_ts())
            } else {
                None
            };

            updates.push((
                task.uuid.clone(),
                if action == "done" {
                    3
                } else if action == "incomplete" {
                    0
                } else {
                    2
                },
                task.entity.clone(),
                stop_date,
            ));
            successes.push(task);
        }

        if updates.is_empty() {
            return Ok(());
        }

        let now = now_ts();
        let mut changes = BTreeMap::new();
        for (uuid, status, entity, stop_date) in updates {
            let mut props = BTreeMap::new();
            props.insert("ss".to_string(), json!(status));
            props.insert("sp".to_string(), json!(stop_date));
            props.insert("md".to_string(), json!(now));
            changes.insert(
                uuid,
                WireObject {
                    operation_type: OperationType::Update,
                    entity_type: Some(EntityType::from(entity)),
                    properties: props,
                },
            );
        }

        if let Err(e) = client.commit(changes, None) {
            eprintln!("Failed to mark items {}: {}", action, e);
            return Ok(());
        }

        let label = match action {
            "done" => format!("{} Done", ICONS.done),
            "incomplete" => format!("{} Incomplete", ICONS.incomplete),
            _ => format!("{} Canceled", ICONS.canceled),
        };
        for task in successes {
            writeln!(
                out,
                "{} {}  {}",
                colored(&label, &[GREEN], cli.no_color),
                task.title,
                colored(&task.uuid, &[DIM], cli.no_color)
            )?;
        }

        Ok(())
    }
}
