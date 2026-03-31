use crate::app::Cli;
use crate::arg_types::IdentifierToken;
use crate::commands::{Command, TagDeltaArgs};
use crate::common::{DIM, GREEN, ICONS, colored, resolve_tag_ids, task6_note};
use crate::ids::ThingsId;
use crate::wire::checklist::{ChecklistItemPatch, ChecklistItemProps};
use crate::wire::notes::{StructuredTaskNotes, TaskNotes};
use crate::wire::task::{TaskPatch, TaskStart, TaskStatus};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use clap::Args;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Args)]
#[command(about = "Edit a task title, container, notes, tags, or checklist items")]
pub struct EditArgs {
    #[arg(help = "Task UUID(s) (or unique UUID prefixes)")]
    pub task_ids: Vec<IdentifierToken>,
    #[arg(long, help = "Replace title (single task only)")]
    pub title: Option<String>,
    #[arg(
        long,
        help = "Replace notes (single task only; use empty string to clear)"
    )]
    pub notes: Option<String>,
    #[arg(
        long = "move",
        help = "Move to Inbox, clear, project UUID/prefix, or area UUID/prefix"
    )]
    pub move_target: Option<String>,
    #[command(flatten)]
    pub tag_delta: TagDeltaArgs,
    #[arg(
        long = "add-checklist",
        value_name = "TITLE",
        help = "Add a checklist item (repeatable, single task only)"
    )]
    pub add_checklist: Vec<String>,
    #[arg(
        long = "remove-checklist",
        value_name = "IDS",
        help = "Remove checklist items by comma-separated short IDs (single task only)"
    )]
    pub remove_checklist: Option<String>,
    #[arg(
        long = "rename-checklist",
        value_name = "ID:TITLE",
        help = "Rename a checklist item: short-id:new title (repeatable, single task only)"
    )]
    pub rename_checklist: Vec<String>,
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

#[derive(Debug, Clone)]
struct EditPlan {
    tasks: Vec<crate::store::Task>,
    changes: BTreeMap<String, WireObject>,
    labels: Vec<String>,
}

impl Command for EditArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let now = ctx.now_timestamp();
        let mut id_gen = || ctx.next_id();
        let plan = match build_edit_plan(self, &store, now, &mut id_gen) {
            Ok(plan) => plan,
            Err(err) => {
                eprintln!("{err}");
                return Ok(());
            }
        };

        if let Err(e) = ctx.commit_changes(plan.changes.clone(), None) {
            eprintln!("Failed to edit item: {e}");
            return Ok(());
        }

        let label_str = colored(
            &format!("({})", plan.labels.join(", ")),
            &[DIM],
            cli.no_color,
        );
        for task in plan.tasks {
            let title_display = plan
                .changes
                .get(&task.uuid.to_string())
                .and_then(|obj| obj.properties_map().get("tt").cloned())
                .and_then(|v| v.as_str().map(ToString::to_string))
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

fn build_edit_plan(
    args: &EditArgs,
    store: &crate::store::ThingsStore,
    now: f64,
    next_id: &mut dyn FnMut() -> String,
) -> std::result::Result<EditPlan, String> {
    let multiple = args.task_ids.len() > 1;
    if multiple && args.title.is_some() {
        return Err("--title requires a single task ID.".to_string());
    }
    if multiple && args.notes.is_some() {
        return Err("--notes requires a single task ID.".to_string());
    }
    if multiple
        && (!args.add_checklist.is_empty()
            || args.remove_checklist.is_some()
            || !args.rename_checklist.is_empty())
    {
        return Err(
            "--add-checklist/--remove-checklist/--rename-checklist require a single task ID."
                .to_string(),
        );
    }

    let mut tasks = Vec::new();
    for identifier in &args.task_ids {
        let (task_opt, err, _) = store.resolve_mark_identifier(identifier.as_str());
        let Some(task) = task_opt else {
            return Err(err);
        };
        if task.is_project() {
            return Err("Use 'projects edit' to edit a project.".to_string());
        }
        tasks.push(task);
    }

    let mut shared_update = TaskPatch::default();
    let mut move_from_inbox_st: Option<TaskStart> = None;
    let mut labels: Vec<String> = Vec::new();
    let move_raw = args.move_target.clone().unwrap_or_default();
    let move_l = move_raw.to_lowercase();

    if !move_raw.trim().is_empty() {
        if move_l == "inbox" {
            shared_update.parent_project_ids = Some(vec![]);
            shared_update.area_ids = Some(vec![]);
            shared_update.action_group_ids = Some(vec![]);
            shared_update.start_location = Some(TaskStart::Inbox);
            shared_update.scheduled_date = Some(None);
            shared_update.today_index_reference = Some(None);
            shared_update.evening_bit = Some(0);
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
                return Err(format!(
                    "Ambiguous --move target '{}' (matches project and area).",
                    move_raw
                ));
            }
            if project_opt.is_some() && project_uuid.is_none() {
                return Err(
                    "--move target must be Inbox, clear, a project ID, or an area ID.".to_string(),
                );
            }

            if let Some(project_uuid) = project_uuid {
                let project_id = ThingsId::from(project_uuid);
                shared_update.parent_project_ids = Some(vec![project_id]);
                shared_update.area_ids = Some(vec![]);
                shared_update.action_group_ids = Some(vec![]);
                move_from_inbox_st = Some(TaskStart::Anytime);
                labels.push(format!("move={move_raw}"));
            } else if let Some(area_uuid) = area_uuid {
                let area_id = ThingsId::from(area_uuid);
                shared_update.area_ids = Some(vec![area_id]);
                shared_update.parent_project_ids = Some(vec![]);
                shared_update.action_group_ids = Some(vec![]);
                move_from_inbox_st = Some(TaskStart::Anytime);
                labels.push(format!("move={move_raw}"));
            } else {
                return Err(format!("Container not found: {move_raw}"));
            }
        }
    }

    let mut add_tag_ids = Vec::new();
    let mut remove_tag_ids = Vec::new();
    if let Some(raw) = &args.tag_delta.add_tags {
        let (ids, err) = resolve_tag_ids(store, raw);
        if !err.is_empty() {
            return Err(err);
        }
        add_tag_ids = ids;
        labels.push("add-tags".to_string());
    }
    if let Some(raw) = &args.tag_delta.remove_tags {
        let (ids, err) = resolve_tag_ids(store, raw);
        if !err.is_empty() {
            return Err(err);
        }
        remove_tag_ids = ids;
        if !labels.iter().any(|l| l == "remove-tags") {
            labels.push("remove-tags".to_string());
        }
    }

    let mut rename_map: HashMap<String, String> = HashMap::new();
    for token in &args.rename_checklist {
        let Some((short_id, new_title)) = token.split_once(':') else {
            return Err(format!(
                "--rename-checklist requires 'id:new title' format, got: {token:?}"
            ));
        };
        let short_id = short_id.trim();
        let new_title = new_title.trim();
        if short_id.is_empty() || new_title.is_empty() {
            return Err(format!(
                "--rename-checklist requires 'id:new title' format, got: {token:?}"
            ));
        }
        rename_map.insert(short_id.to_string(), new_title.to_string());
    }

    let mut changes: BTreeMap<String, WireObject> = BTreeMap::new();

    for task in &tasks {
        let mut update = shared_update.clone();

        if let Some(title) = &args.title {
            let title = title.trim();
            if title.is_empty() {
                return Err("Task title cannot be empty.".to_string());
            }
            update.title = Some(title.to_string());
            if !labels.iter().any(|l| l == "title") {
                labels.push("title".to_string());
            }
        }

        if let Some(notes) = &args.notes {
            if notes.is_empty() {
                update.notes = Some(TaskNotes::Structured(StructuredTaskNotes {
                    object_type: Some("tx".to_string()),
                    format_type: 1,
                    ch: Some(0),
                    v: Some(String::new()),
                    ps: Vec::new(),
                    unknown_fields: Default::default(),
                }));
            } else {
                update.notes = Some(task6_note(notes));
            }
            if !labels.iter().any(|l| l == "notes") {
                labels.push("notes".to_string());
            }
        }

        if move_l == "clear" {
            update.parent_project_ids = Some(vec![]);
            update.area_ids = Some(vec![]);
            update.action_group_ids = Some(vec![]);
            if task.start == TaskStart::Inbox {
                update.start_location = Some(TaskStart::Anytime);
            }
        }

        if let Some(move_from_inbox_st) = move_from_inbox_st
            && task.start == TaskStart::Inbox
        {
            update.start_location = Some(move_from_inbox_st);
        }

        if !add_tag_ids.is_empty() || !remove_tag_ids.is_empty() {
            let mut current = task.tags.clone();
            for uuid in &add_tag_ids {
                if !current.iter().any(|c| c == uuid) {
                    current.push(uuid.clone());
                }
            }
            current.retain(|uuid| !remove_tag_ids.iter().any(|r| r == uuid));
            update.tag_ids = Some(current);
        }

        if let Some(remove_raw) = &args.remove_checklist {
            let (items, err) = resolve_checklist_items(task, remove_raw);
            if !err.is_empty() {
                return Err(err);
            }
            for uuid in items.into_iter().map(|i| i.uuid).collect::<HashSet<_>>() {
                changes.insert(
                    uuid.to_string(),
                    WireObject::delete(EntityType::ChecklistItem3),
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
                    return Err(format!("Checklist item not found: '{short_id}'"));
                }
                if matches.len() > 1 {
                    return Err(format!("Ambiguous checklist item prefix: '{short_id}'"));
                }
                changes.insert(
                    matches[0].uuid.to_string(),
                    WireObject::update(
                        EntityType::ChecklistItem3,
                        ChecklistItemPatch {
                            title: Some(new_title.to_string()),
                            modification_date: Some(now),
                            ..Default::default()
                        },
                    ),
                );
            }
            if !labels.iter().any(|l| l == "rename-checklist") {
                labels.push("rename-checklist".to_string());
            }
        }

        if !args.add_checklist.is_empty() {
            let max_ix = task
                .checklist_items
                .iter()
                .map(|i| i.index)
                .max()
                .unwrap_or(0);
            for (idx, title) in args.add_checklist.iter().enumerate() {
                let title = title.trim();
                if title.is_empty() {
                    return Err("Checklist item title cannot be empty.".to_string());
                }
                changes.insert(
                    next_id(),
                    WireObject::create(
                        EntityType::ChecklistItem3,
                        ChecklistItemProps {
                            title: title.to_string(),
                            task_ids: vec![task.uuid.clone()],
                            status: TaskStatus::Incomplete,
                            sort_index: max_ix + idx as i32 + 1,
                            creation_date: Some(now),
                            modification_date: Some(now),
                            ..Default::default()
                        },
                    ),
                );
            }
            if !labels.iter().any(|l| l == "add-checklist") {
                labels.push("add-checklist".to_string());
            }
        }

        let has_checklist_changes = !args.add_checklist.is_empty()
            || args.remove_checklist.is_some()
            || !rename_map.is_empty();
        if update.is_empty() && !has_checklist_changes {
            return Err("No edit changes requested.".to_string());
        }

        if !update.is_empty() {
            update.modification_date = Some(now);
            changes.insert(
                task.uuid.to_string(),
                WireObject::update(EntityType::from(task.entity.clone()), update),
            );
        }
    }

    Ok(EditPlan {
        tasks,
        changes,
        labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ThingsId;
    use crate::store::{ThingsStore, fold_items};
    use crate::wire::area::AreaProps;
    use crate::wire::checklist::ChecklistItemProps;
    use crate::wire::tags::TagProps;
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
    use crate::wire::wire_object::WireItem;
    use crate::wire::wire_object::{EntityType, OperationType, WireObject};
    use serde_json::json;
    use std::collections::BTreeMap;

    const NOW: f64 = 1_700_000_222.0;
    const TASK_UUID: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const TASK_UUID2: &str = "3H9jsMx3kYMrQ4M7DReSRn";
    const PROJECT_UUID: &str = "KGvAPpMrzHAKMdgMiERP1V";
    const AREA_UUID: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const CHECK_A: &str = "5uwoHPi5m5i8QJa6Rae6Cn";
    const CHECK_B: &str = "CwhFwmHxjHkR7AFn9aJH9Q";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item: WireItem = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        let raw = fold_items([item]);
        ThingsStore::from_raw_state(&raw)
    }

    fn task(uuid: &str, title: &str) -> (String, WireObject) {
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
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn task_with(uuid: &str, title: &str, tag_ids: Vec<&str>) -> (String, WireObject) {
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
                    tag_ids: tag_ids.iter().map(|t| ThingsId::from(*t)).collect(),
                    creation_date: Some(1.0),
                    modification_date: Some(1.0),
                    ..Default::default()
                },
            ),
        )
    }

    fn project(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Project,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Anytime,
                    sort_index: 0,
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

    fn tag(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Tag4,
                TagProps {
                    title: title.to_string(),
                    sort_index: 0,
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

    fn assert_task_update(plan: &EditPlan, uuid: &str) -> BTreeMap<String, serde_json::Value> {
        let obj = plan.changes.get(uuid).expect("missing task change");
        assert_eq!(obj.operation_type, OperationType::Update);
        assert_eq!(obj.entity_type, Some(EntityType::Task6));
        obj.properties_map()
    }

    #[test]
    fn edit_title_and_notes_payloads() {
        let store = build_store(vec![task(TASK_UUID, "Old title")]);
        let args = EditArgs {
            task_ids: vec![IdentifierToken::from(TASK_UUID)],
            title: Some("New title".to_string()),
            notes: Some("new notes".to_string()),
            move_target: None,
            tag_delta: TagDeltaArgs {
                add_tags: None,
                remove_tags: None,
            },
            add_checklist: vec![],
            remove_checklist: None,
            rename_checklist: vec![],
        };
        let mut id_gen = || "X".to_string();
        let plan = build_edit_plan(&args, &store, NOW, &mut id_gen).expect("plan");
        let p = assert_task_update(&plan, TASK_UUID);
        assert_eq!(p.get("tt"), Some(&json!("New title")));
        assert_eq!(p.get("md"), Some(&json!(NOW)));
        assert!(p.get("nt").is_some());
    }

    #[test]
    fn edit_move_targets_payload() {
        let store = build_store(vec![
            task(TASK_UUID, "Movable"),
            project(PROJECT_UUID, "Roadmap"),
            area(AREA_UUID, "Work"),
        ]);

        let mut id_gen = || "X".to_string();
        let inbox = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: Some("inbox".to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("inbox plan");
        let p = assert_task_update(&inbox, TASK_UUID);
        assert_eq!(p.get("st"), Some(&json!(0)));
        assert_eq!(p.get("pr"), Some(&json!([])));
        assert_eq!(p.get("ar"), Some(&json!([])));

        let clear = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: Some("clear".to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("clear plan");
        let p = assert_task_update(&clear, TASK_UUID);
        assert_eq!(p.get("st"), Some(&json!(1)));

        let project_move = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: Some(PROJECT_UUID.to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("project move plan");
        let p = assert_task_update(&project_move, TASK_UUID);
        assert_eq!(p.get("pr"), Some(&json!([PROJECT_UUID])));
        assert_eq!(p.get("st"), Some(&json!(1)));
    }

    #[test]
    fn edit_multi_id_move_and_rejections() {
        let store = build_store(vec![
            task(TASK_UUID, "Task One"),
            task(TASK_UUID2, "Task Two"),
            project(PROJECT_UUID, "Roadmap"),
        ]);

        let mut id_gen = || "X".to_string();
        let plan = build_edit_plan(
            &EditArgs {
                task_ids: vec![
                    IdentifierToken::from(TASK_UUID),
                    IdentifierToken::from(TASK_UUID2),
                ],
                title: None,
                notes: None,
                move_target: Some(PROJECT_UUID.to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("multi move");
        assert_eq!(plan.changes.len(), 2);

        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![
                    IdentifierToken::from(TASK_UUID),
                    IdentifierToken::from(TASK_UUID2),
                ],
                title: Some("New".to_string()),
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("title should reject");
        assert_eq!(err, "--title requires a single task ID.");
    }

    #[test]
    fn edit_tag_payloads() {
        let tag1 = "WukwpDdL5Z88nX3okGMKTC";
        let tag2 = "JiqwiDaS3CAyjCmHihBDnB";
        let store = build_store(vec![
            task_with(TASK_UUID, "A", vec![tag1]),
            tag(tag1, "Work"),
            tag(tag2, "Focus"),
        ]);

        let mut id_gen = || "X".to_string();
        let plan = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: Some("Focus".to_string()),
                    remove_tags: Some("Work".to_string()),
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("tag plan");

        let p = assert_task_update(&plan, TASK_UUID);
        assert_eq!(p.get("tg"), Some(&json!([tag2])));
    }

    #[test]
    fn edit_checklist_mutations() {
        let store = build_store(vec![
            task(TASK_UUID, "A"),
            checklist(CHECK_A, TASK_UUID, "Step one", 1),
            checklist(CHECK_B, TASK_UUID, "Step two", 2),
        ]);

        let mut ids = vec!["NEW_CHECK_1".to_string(), "NEW_CHECK_2".to_string()].into_iter();
        let mut id_gen = || ids.next().expect("next id");
        let plan = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec!["Step three".to_string(), "Step four".to_string()],
                remove_checklist: Some(format!("{},{}", &CHECK_A[..6], &CHECK_B[..6])),
                rename_checklist: vec![format!("{}:Renamed", &CHECK_A[..6])],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect("checklist plan");

        assert!(matches!(
            plan.changes.get(CHECK_A).map(|o| o.operation_type),
            Some(OperationType::Update)
        ));
        assert!(matches!(
            plan.changes.get(CHECK_B).map(|o| o.operation_type),
            Some(OperationType::Delete)
        ));
        assert!(plan.changes.contains_key("NEW_CHECK_1"));
        assert!(plan.changes.contains_key("NEW_CHECK_2"));
    }

    #[test]
    fn edit_no_changes_project_and_move_errors() {
        let store = build_store(vec![task(TASK_UUID, "A")]);
        let mut id_gen = || "X".to_string();
        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("no changes");
        assert_eq!(err, "No edit changes requested.");

        let store = build_store(vec![task(TASK_UUID, "A"), project(PROJECT_UUID, "Roadmap")]);
        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(PROJECT_UUID)],
                title: Some("New".to_string()),
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("project edit reject");
        assert_eq!(err, "Use 'projects edit' to edit a project.");

        let store = build_store(vec![
            task(TASK_UUID, "Movable"),
            task(PROJECT_UUID, "Not a project"),
        ]);
        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: Some(PROJECT_UUID.to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("invalid move target kind");
        assert_eq!(
            err,
            "--move target must be Inbox, clear, a project ID, or an area ID."
        );
    }

    #[test]
    fn edit_move_target_ambiguous() {
        let ambiguous_project = "ABCD1234efgh5678JKLMno";
        let ambiguous_area = "ABCD1234pqrs9123TUVWxy";
        let store = build_store(vec![
            task(TASK_UUID, "Movable"),
            project(ambiguous_project, "Project match"),
            area(ambiguous_area, "Area match"),
        ]);
        let mut id_gen = || "X".to_string();
        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: None,
                notes: None,
                move_target: Some("ABCD1234".to_string()),
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("ambiguous move target");
        assert_eq!(
            err,
            "Ambiguous --move target 'ABCD1234' (matches project and area)."
        );
    }

    #[test]
    fn checklist_single_task_constraint_and_empty_title() {
        let store = build_store(vec![task(TASK_UUID, "A"), task(TASK_UUID2, "B")]);
        let mut id_gen = || "X".to_string();

        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![
                    IdentifierToken::from(TASK_UUID),
                    IdentifierToken::from(TASK_UUID2),
                ],
                title: None,
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec!["Step".to_string()],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("single task constraint");
        assert_eq!(
            err,
            "--add-checklist/--remove-checklist/--rename-checklist require a single task ID."
        );

        let store = build_store(vec![task(TASK_UUID, "A")]);
        let err = build_edit_plan(
            &EditArgs {
                task_ids: vec![IdentifierToken::from(TASK_UUID)],
                title: Some("   ".to_string()),
                notes: None,
                move_target: None,
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
                add_checklist: vec![],
                remove_checklist: None,
                rename_checklist: vec![],
            },
            &store,
            NOW,
            &mut id_gen,
        )
        .expect_err("empty title");
        assert_eq!(err, "Task title cannot be empty.");
    }

    #[test]
    fn checklist_patch_has_expected_fields() {
        let patch = ChecklistItemPatch {
            title: Some("Step".to_string()),
            status: Some(TaskStatus::Incomplete),
            task_ids: Some(vec![crate::ids::ThingsId::from(TASK_UUID)]),
            sort_index: Some(3),
            creation_date: Some(NOW),
            modification_date: Some(NOW),
        };
        let props = patch.into_properties();
        assert_eq!(props.get("tt"), Some(&json!("Step")));
        assert_eq!(props.get("ss"), Some(&json!(0)));
        assert_eq!(props.get("ix"), Some(&json!(3)));
    }
}
