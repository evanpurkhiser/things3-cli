use crate::app::Cli;
use crate::arg_types::IdentifierToken;
use crate::commands::Command;
use crate::common::{colored, DIM, GREEN, ICONS};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use clap::Args;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Args)]
#[command(about = "Delete tasks/projects/headings/areas")]
pub struct DeleteArgs {
    /// Item UUID(s) (or unique UUID prefixes)
    pub item_ids: Vec<IdentifierToken>,
}

#[derive(Debug, Clone)]
struct DeletePlan {
    targets: Vec<(String, String, String)>,
    changes: BTreeMap<String, WireObject>,
}

fn build_delete_plan(args: &DeleteArgs, store: &crate::store::ThingsStore) -> DeletePlan {
    let mut targets: Vec<(String, String, String)> = Vec::new();
    let mut seen = HashSet::new();

    for identifier in &args.item_ids {
        let (task, task_err, task_ambiguous) = store.resolve_task_identifier(identifier.as_str());
        let (area, area_err, area_ambiguous) = store.resolve_area_identifier(identifier.as_str());

        let task_match = task.is_some();
        let area_match = area.is_some();

        if task_match && area_match {
            eprintln!(
                "Ambiguous identifier '{}' (matches task and area).",
                identifier.as_str()
            );
            continue;
        }

        if !task_match && !area_match {
            if !task_ambiguous.is_empty() && !area_ambiguous.is_empty() {
                eprintln!(
                    "Ambiguous identifier '{}' (matches multiple tasks and areas).",
                    identifier.as_str()
                );
            } else if !task_ambiguous.is_empty() {
                eprintln!("{task_err}");
            } else if !area_ambiguous.is_empty() {
                eprintln!("{area_err}");
            } else {
                eprintln!("Item not found: {}", identifier.as_str());
            }
            continue;
        }

        if let Some(task) = task {
            if task.trashed {
                eprintln!("Item already deleted: {}", task.title);
                continue;
            }
            if !seen.insert(task.uuid.clone()) {
                continue;
            }
            targets.push((
                task.uuid.to_string(),
                task.entity.clone(),
                task.title.clone(),
            ));
            continue;
        }

        if let Some(area) = area {
            if !seen.insert(area.uuid.clone()) {
                continue;
            }
            targets.push((
                area.uuid.to_string(),
                "Area3".to_string(),
                area.title.clone(),
            ));
        }
    }

    let mut changes = BTreeMap::new();
    for (uuid, entity, _title) in &targets {
        changes.insert(
            uuid.clone(),
            WireObject::delete(EntityType::from(entity.clone())),
        );
    }

    DeletePlan { targets, changes }
}

impl Command for DeleteArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let plan = build_delete_plan(self, &store);

        if plan.targets.is_empty() {
            return Ok(());
        }

        if let Err(e) = ctx.commit_changes(plan.changes, None) {
            eprintln!("Failed to delete items: {e}");
            return Ok(());
        }

        for (uuid, _entity, title) in plan.targets {
            writeln!(
                out,
                "{} {}  {}",
                colored(
                    &format!("{} Deleted", ICONS.deleted),
                    &[GREEN],
                    cli.no_color
                ),
                title,
                colored(&uuid, &[DIM], cli.no_color)
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{fold_items, ThingsStore};
    use crate::wire::area::AreaProps;
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};

    const TASK_A: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const TASK_B: &str = "KGvAPpMrzHAKMdgMiERP1V";
    const AREA_A: &str = "MpkEei6ybkFS2n6SXvwfLf";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn task(uuid: &str, title: &str, trashed: bool) -> (String, WireObject) {
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
                    trashed,
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn area(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Area3,
                AreaProps {
                    title: title.to_string(),
                    sort_index: 0,
                    ..Default::default()
                },
            ),
        )
    }

    #[test]
    fn delete_payloads_match_snapshot_cases() {
        let single = build_delete_plan(
            &DeleteArgs {
                item_ids: vec![IdentifierToken::from(TASK_A)],
            },
            &build_store(vec![task(TASK_A, "Alpha", false)]),
        );
        assert_eq!(
            serde_json::to_value(single.changes).expect("to value"),
            serde_json::json!({ TASK_A: {"t":2,"e":"Task6","p":{}} })
        );

        let multi = build_delete_plan(
            &DeleteArgs {
                item_ids: vec![IdentifierToken::from(TASK_A), IdentifierToken::from(AREA_A)],
            },
            &build_store(vec![task(TASK_A, "Alpha", false), area(AREA_A, "Work")]),
        );
        assert_eq!(
            serde_json::to_value(multi.changes).expect("to value"),
            serde_json::json!({
                TASK_A: {"t":2,"e":"Task6","p":{}},
                AREA_A: {"t":2,"e":"Area3","p":{}}
            })
        );

        let skip_trashed = build_delete_plan(
            &DeleteArgs {
                item_ids: vec![IdentifierToken::from(TASK_A), IdentifierToken::from(TASK_B)],
            },
            &build_store(vec![
                task(TASK_A, "Active", false),
                task(TASK_B, "Trashed", true),
            ]),
        );
        assert_eq!(
            serde_json::to_value(skip_trashed.changes).expect("to value"),
            serde_json::json!({ TASK_A: {"t":2,"e":"Task6","p":{}} })
        );
    }
}
