use crate::app::Cli;
use crate::arg_types::IdentifierToken;
use crate::commands::Command;
use crate::common::{colored, DIM, GREEN, ICONS};
use crate::wire::checklist::ChecklistItemPatch;
use crate::wire::recurrence::RecurrenceType;
use crate::wire::task::{TaskPatch, TaskStatus};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use clap::{ArgGroup, Args};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Args)]
#[command(about = "Mark a task done, incomplete, or canceled")]
#[command(group(ArgGroup::new("status").args(["done", "incomplete", "canceled", "check_ids", "uncheck_ids", "check_cancel_ids"]).required(true).multiple(false)))]
pub struct MarkArgs {
    /// Task UUID(s) (or unique UUID prefixes)
    pub task_ids: Vec<IdentifierToken>,
    #[arg(long, help = "Mark task(s) as completed")]
    pub done: bool,
    #[arg(long, help = "Mark task(s) as incomplete")]
    pub incomplete: bool,
    #[arg(long, help = "Mark task(s) as canceled")]
    pub canceled: bool,
    #[arg(
        long = "check",
        help = "Mark checklist items done by comma-separated short IDs"
    )]
    pub check_ids: Option<String>,
    #[arg(
        long = "uncheck",
        help = "Mark checklist items incomplete by comma-separated short IDs"
    )]
    pub uncheck_ids: Option<String>,
    #[arg(
        long = "check-cancel",
        help = "Mark checklist items canceled by comma-separated short IDs"
    )]
    pub check_cancel_ids: Option<String>,
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
    let Some(template) = store.get_task(&template_uuid.to_string()) else {
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
        RecurrenceType::FixedSchedule => (true, String::new()),
        RecurrenceType::AfterCompletion => (
            false,
            "Recurring 'after completion' templates (rr.tp=1) are blocked: completion requires coupled template writes (acrd/tir) not implemented yet.".to_string(),
        ),
        RecurrenceType::Unknown(v) => (
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

#[derive(Debug, Clone)]
struct MarkCommitPlan {
    changes: BTreeMap<String, WireObject>,
}

fn build_mark_status_plan(
    args: &MarkArgs,
    store: &crate::store::ThingsStore,
    now: f64,
) -> (MarkCommitPlan, Vec<crate::store::Task>, Vec<String>) {
    let action = if args.done {
        "done"
    } else if args.incomplete {
        "incomplete"
    } else {
        "canceled"
    };

    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for identifier in &args.task_ids {
        let (task_opt, err, _) = store.resolve_mark_identifier(identifier.as_str());
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
    let mut errors = Vec::new();

    for task in targets {
        let validation_error = validate_mark_target(&task, action, store);
        if !validation_error.is_empty() {
            errors.push(format!("{} ({})", validation_error, task.title));
            continue;
        }

        let (task_status, stop_date) = if action == "done" {
            (TaskStatus::Completed, Some(now))
        } else if action == "incomplete" {
            (TaskStatus::Incomplete, None)
        } else {
            (TaskStatus::Canceled, Some(now))
        };

        updates.push((
            task.uuid.clone(),
            task_status,
            task.entity.clone(),
            stop_date,
        ));
        successes.push(task);
    }

    let mut changes = BTreeMap::new();
    for (uuid, status, entity, stop_date) in updates {
        changes.insert(
            uuid.to_string(),
            WireObject::update(
                EntityType::from(entity),
                TaskPatch {
                    status: Some(status),
                    stop_date: Some(stop_date),
                    modification_date: Some(now),
                    ..Default::default()
                },
            ),
        );
    }

    (MarkCommitPlan { changes }, successes, errors)
}

fn build_mark_checklist_plan(
    args: &MarkArgs,
    task: &crate::store::Task,
    checklist_raw: &str,
    now: f64,
) -> std::result::Result<(MarkCommitPlan, Vec<crate::store::ChecklistItem>, String), String> {
    let (items, err) = resolve_checklist_items(task, checklist_raw);
    if !err.is_empty() {
        return Err(err);
    }

    let (label, status): (&str, TaskStatus) = if args.check_ids.is_some() {
        ("checked", TaskStatus::Completed)
    } else if args.uncheck_ids.is_some() {
        ("unchecked", TaskStatus::Incomplete)
    } else {
        ("canceled", TaskStatus::Canceled)
    };

    let mut changes = BTreeMap::new();
    for item in &items {
        changes.insert(
            item.uuid.to_string(),
            WireObject::update(
                EntityType::ChecklistItem3,
                ChecklistItemPatch {
                    status: Some(status),
                    modification_date: Some(now),
                    ..Default::default()
                },
            ),
        );
    }

    Ok((MarkCommitPlan { changes }, items, label.to_string()))
}

impl Command for MarkArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let checklist_raw = self
            .check_ids
            .as_ref()
            .or(self.uncheck_ids.as_ref())
            .or(self.check_cancel_ids.as_ref());

        if let Some(checklist_raw) = checklist_raw {
            if self.task_ids.len() != 1 {
                eprintln!(
                    "Checklist flags (--check, --uncheck, --check-cancel) require exactly one task ID."
                );
                return Ok(());
            }

            let (task_opt, err, _) = store.resolve_mark_identifier(self.task_ids[0].as_str());
            let Some(task) = task_opt else {
                eprintln!("{err}");
                return Ok(());
            };

            if task.checklist_items.is_empty() {
                eprintln!("Task has no checklist items: {}", task.title);
                return Ok(());
            }

            let (plan, items, label) =
                match build_mark_checklist_plan(self, &task, checklist_raw, ctx.now_timestamp()) {
                    Ok(v) => v,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

            if let Err(e) = ctx.commit_changes(plan.changes, None) {
                eprintln!("Failed to mark checklist items: {e}");
                return Ok(());
            }

            let title = match label.as_str() {
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

        let (plan, successes, errors) = build_mark_status_plan(self, &store, ctx.now_timestamp());
        for err in errors {
            eprintln!("{err}");
        }

        if plan.changes.is_empty() {
            return Ok(());
        }

        if let Err(e) = ctx.commit_changes(plan.changes, None) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ThingsId;
    use crate::store::{fold_items, ThingsStore};
    use crate::wire::checklist::ChecklistItemProps;
    use crate::wire::recurrence::{RecurrenceRule, RecurrenceType};
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
    use serde_json::json;

    const NOW: f64 = 1_700_000_111.0;
    const TASK_A: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const CHECK_A: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const CHECK_B: &str = "JFdhhhp37fpryAKu8UXwzK";
    const TPL_A: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const TPL_B: &str = "JFdhhhp37fpryAKu8UXwzK";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn task(uuid: &str, title: &str, status: i32) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Todo,
                    status: TaskStatus::from(status),
                    start_location: TaskStart::Inbox,
                    sort_index: 0,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn task_with_props(
        uuid: &str,
        title: &str,
        recurrence_rule: Option<RecurrenceRule>,
        recurrence_templates: Vec<&str>,
    ) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Todo,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Inbox,
                    sort_index: 0,
                    recurrence_rule,
                    recurrence_template_ids: recurrence_templates
                        .iter()
                        .map(|t| ThingsId::from(*t))
                        .collect(),
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn checklist(uuid: &str, task_uuid: &str, title: &str, ix: i32) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::ChecklistItem3,
                ChecklistItemProps {
                    title: title.to_string(),
                    task_ids: vec![ThingsId::from(task_uuid)],
                    status: TaskStatus::Incomplete,
                    sort_index: ix,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    #[test]
    fn mark_status_payloads() {
        let done_store = build_store(vec![task(TASK_A, "Alpha", 0)]);
        let (done_plan, _, errs) = build_mark_status_plan(
            &MarkArgs {
                task_ids: vec![IdentifierToken::from(TASK_A)],
                done: true,
                incomplete: false,
                canceled: false,
                check_ids: None,
                uncheck_ids: None,
                check_cancel_ids: None,
            },
            &done_store,
            NOW,
        );
        assert!(errs.is_empty());
        assert_eq!(
            serde_json::to_value(done_plan.changes).expect("to value"),
            json!({ TASK_A: {"t":1,"e":"Task6","p":{"ss":3,"sp":NOW,"md":NOW}} })
        );

        let incomplete_store = build_store(vec![task(TASK_A, "Alpha", 3)]);
        let (incomplete_plan, _, _) = build_mark_status_plan(
            &MarkArgs {
                task_ids: vec![IdentifierToken::from(TASK_A)],
                done: false,
                incomplete: true,
                canceled: false,
                check_ids: None,
                uncheck_ids: None,
                check_cancel_ids: None,
            },
            &incomplete_store,
            NOW,
        );
        assert_eq!(
            serde_json::to_value(incomplete_plan.changes).expect("to value"),
            json!({ TASK_A: {"t":1,"e":"Task6","p":{"ss":0,"sp":null,"md":NOW}} })
        );
    }

    #[test]
    fn mark_checklist_payloads() {
        let store = build_store(vec![
            task(TASK_A, "Task with checklist", 0),
            checklist(CHECK_A, TASK_A, "One", 1),
            checklist(CHECK_B, TASK_A, "Two", 2),
        ]);
        let task = store.get_task(TASK_A).expect("task");

        let (checked_plan, _, _) = build_mark_checklist_plan(
            &MarkArgs {
                task_ids: vec![IdentifierToken::from(TASK_A)],
                done: false,
                incomplete: false,
                canceled: false,
                check_ids: Some(format!("{},{}", &CHECK_A[..6], &CHECK_B[..6])),
                uncheck_ids: None,
                check_cancel_ids: None,
            },
            &task,
            &format!("{},{}", &CHECK_A[..6], &CHECK_B[..6]),
            NOW,
        )
        .expect("checked plan");
        assert_eq!(
            serde_json::to_value(checked_plan.changes).expect("to value"),
            json!({
                CHECK_A: {"t":1,"e":"ChecklistItem3","p":{"ss":3,"md":NOW}},
                CHECK_B: {"t":1,"e":"ChecklistItem3","p":{"ss":3,"md":NOW}}
            })
        );
    }

    #[test]
    fn mark_recurring_rejection_cases() {
        let store = build_store(vec![task_with_props(
            TASK_A,
            "Recurring template",
            Some(RecurrenceRule {
                repeat_type: RecurrenceType::FixedSchedule,
                ..Default::default()
            }),
            vec![],
        )]);
        let (plan, _, errs) = build_mark_status_plan(
            &MarkArgs {
                task_ids: vec![IdentifierToken::from(TASK_A)],
                done: true,
                incomplete: false,
                canceled: false,
                check_ids: None,
                uncheck_ids: None,
                check_cancel_ids: None,
            },
            &store,
            NOW,
        );
        assert!(plan.changes.is_empty());
        assert_eq!(
            errs,
            vec![
                "Recurring template tasks are blocked for done (template progression bookkeeping is not implemented). (Recurring template)"
            ]
        );

        let store = build_store(vec![task_with_props(
            TASK_A,
            "Recurring instance",
            None,
            vec![TPL_A, TPL_B],
        )]);
        let (_, _, errs) = build_mark_status_plan(
            &MarkArgs {
                task_ids: vec![IdentifierToken::from(TASK_A)],
                done: true,
                incomplete: false,
                canceled: false,
                check_ids: None,
                uncheck_ids: None,
                check_cancel_ids: None,
            },
            &store,
            NOW,
        );
        assert_eq!(
            errs,
            vec!["Recurring instance has 2 template references; expected exactly 1. (Recurring instance)"]
        );
    }
}
