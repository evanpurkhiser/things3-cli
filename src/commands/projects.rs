use crate::app::Cli;
use crate::cloud_writer::{CloudWriter, LiveCloudWriter};
use crate::commands::Command;
use crate::common::{
    colored, day_to_timestamp, fmt_project_with_note, id_prefix, parse_day, resolve_tag_ids,
    task6_note, BOLD, DIM, GREEN, ICONS,
};
use crate::ids::random_task_id;
use crate::wire::{
    EntityType, OperationType, TaskPatch, TaskStart, TaskStatus, TaskType, WireObject,
};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Subcommand)]
pub enum ProjectsSubcommand {
    List(ProjectsListArgs),
    New(ProjectsNewArgs),
    Edit(ProjectsEditArgs),
}

#[derive(Debug, Args)]
pub struct ProjectsArgs {
    /// Show notes for each project.
    #[arg(long)]
    pub detailed: bool,
    #[command(subcommand)]
    pub command: Option<ProjectsSubcommand>,
}

#[derive(Debug, Default, Args)]
pub struct ProjectsListArgs {
    #[arg(long)]
    pub detailed: bool,
}

#[derive(Debug, Args)]
pub struct ProjectsNewArgs {
    pub title: String,
    #[arg(long)]
    pub area: Option<String>,
    #[arg(long)]
    pub when: Option<String>,
    #[arg(long, default_value = "")]
    pub notes: String,
    #[arg(long)]
    pub tags: Option<String>,
    #[arg(long = "deadline")]
    pub deadline_date: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectsEditArgs {
    pub project_id: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long = "move")]
    pub move_target: Option<String>,
    #[arg(long)]
    pub notes: Option<String>,
    #[arg(long = "add-tags")]
    pub add_tags: Option<String>,
    #[arg(long = "remove-tags")]
    pub remove_tags: Option<String>,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

fn today_ts() -> i64 {
    crate::common::today_utc().timestamp()
}

#[derive(Debug, Clone)]
struct ProjectsEditPlan {
    project: crate::store::Task,
    update: TaskPatch,
    labels: Vec<String>,
}

fn build_projects_edit_plan(
    args: &ProjectsEditArgs,
    store: &crate::store::ThingsStore,
    now: f64,
) -> std::result::Result<ProjectsEditPlan, String> {
    let (project_opt, err, _) = store.resolve_mark_identifier(&args.project_id);
    let Some(project) = project_opt else {
        return Err(err);
    };
    if !project.is_project() {
        return Err("The specified ID is not a project.".to_string());
    }

    let mut update = TaskPatch::default();
    let mut labels: Vec<String> = Vec::new();

    if let Some(title) = &args.title {
        let title = title.trim();
        if title.is_empty() {
            return Err("Project title cannot be empty.".to_string());
        }
        update.title = Some(title.to_string());
        labels.push("title".to_string());
    }

    if let Some(notes) = &args.notes {
        update.notes = Some(if notes.is_empty() {
            json!({"_t":"tx","t":1,"ch":0,"v":""})
        } else {
            task6_note(notes)
        });
        labels.push("notes".to_string());
    }

    if let Some(move_target) = &args.move_target {
        let move_raw = move_target.trim();
        let move_l = move_raw.to_lowercase();
        if move_l == "inbox" {
            return Err("Projects cannot be moved to Inbox.".to_string());
        }
        if move_l == "clear" {
            update.area_ids = Some(vec![]);
            labels.push("move=clear".to_string());
        } else {
            let (resolved_project, _, _) = store.resolve_mark_identifier(move_raw);
            let (area, _, _) = store.resolve_area_identifier(move_raw);
            let project_uuid = resolved_project.as_ref().and_then(|p| {
                if p.is_project() {
                    Some(p.uuid.clone())
                } else {
                    None
                }
            });
            let area_uuid = area.as_ref().map(|a| a.uuid.clone());

            if project_uuid.is_some() && area_uuid.is_some() {
                return Err(format!(
                    "Ambiguous --move target '{}' (matches project and area).",
                    move_raw
                ));
            }
            if project_uuid.is_some() {
                return Err("Projects can only be moved to an area or clear.".to_string());
            }
            if let Some(area_uuid) = area_uuid {
                update.area_ids = Some(vec![area_uuid]);
                labels.push(format!("move={move_raw}"));
            } else {
                return Err(format!("Container not found: {move_raw}"));
            }
        }
    }

    let mut current_tags = project.tags.clone();
    if let Some(add_tags) = &args.add_tags {
        let (ids, err) = resolve_tag_ids(store, add_tags);
        if !err.is_empty() {
            return Err(err);
        }
        for id in ids {
            if !current_tags.iter().any(|t| t == &id) {
                current_tags.push(id);
            }
        }
        labels.push("add-tags".to_string());
    }
    if let Some(remove_tags) = &args.remove_tags {
        let (ids, err) = resolve_tag_ids(store, remove_tags);
        if !err.is_empty() {
            return Err(err);
        }
        current_tags.retain(|t| !ids.iter().any(|id| id == t));
        labels.push("remove-tags".to_string());
    }
    if args.add_tags.is_some() || args.remove_tags.is_some() {
        update.tag_ids = Some(current_tags);
    }

    if update.is_empty() {
        return Err("No edit changes requested.".to_string());
    }

    update.modification_date = Some(now);

    Ok(ProjectsEditPlan {
        project,
        update,
        labels,
    })
}

impl Command for ProjectsArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let default_list = ProjectsListArgs::default();
        let (list_args, is_list) = match self.command.as_ref() {
            Some(ProjectsSubcommand::List(la)) => (la, true),
            None => (&default_list, true),
            _ => (&default_list, false),
        };
        // Merge top-level --detailed into list args.
        let effective_detailed = if is_list {
            self.detailed || list_args.detailed
        } else {
            false
        };

        match &self.command {
            None | Some(ProjectsSubcommand::List(_)) => {
                let store = cli.load_store()?;
                let projects = store.projects(Some(TaskStatus::Incomplete));
                if projects.is_empty() {
                    writeln!(
                        out,
                        "{}",
                        colored("No active projects.", &[DIM], cli.no_color)
                    )?;
                    return Ok(());
                }

                writeln!(
                    out,
                    "{}",
                    colored(
                        &format!("{} Projects  ({})", ICONS.project, projects.len()),
                        &[BOLD, GREEN],
                        cli.no_color,
                    )
                )?;

                let mut by_area: BTreeMap<Option<String>, Vec<_>> = BTreeMap::new();
                for p in &projects {
                    by_area.entry(p.area.clone()).or_default().push(p.clone());
                }

                let mut id_scope = projects.iter().map(|p| p.uuid.clone()).collect::<Vec<_>>();
                id_scope.extend(by_area.keys().flatten().cloned());
                let id_prefix_len = store.unique_prefix_length(&id_scope);

                let no_area = by_area.remove(&None).unwrap_or_default();
                if !no_area.is_empty() {
                    writeln!(out)?;
                    for p in no_area {
                        writeln!(
                            out,
                            "{}",
                            fmt_project_with_note(
                                &p,
                                &store,
                                "  ",
                                Some(id_prefix_len),
                                true,
                                effective_detailed,
                                cli.no_color,
                            )
                        )?;
                    }
                }

                // Sort areas by their index field so output order matches Python
                let mut area_entries: Vec<(String, Vec<_>)> = by_area
                    .into_iter()
                    .filter_map(|(k, v)| k.map(|uuid| (uuid, v)))
                    .collect();
                area_entries.sort_by_key(|(uuid, _)| {
                    store
                        .areas_by_uuid
                        .get(uuid)
                        .map(|a| a.index)
                        .unwrap_or(i32::MAX)
                });

                for (area_uuid, area_projects) in area_entries {
                    let area_title = store.resolve_area_title(&area_uuid);
                    writeln!(out)?;
                    writeln!(
                        out,
                        "  {} {}",
                        id_prefix(&area_uuid, id_prefix_len, cli.no_color),
                        colored(&area_title, &[BOLD], cli.no_color)
                    )?;
                    for p in area_projects {
                        writeln!(
                            out,
                            "{}",
                            fmt_project_with_note(
                                &p,
                                &store,
                                "    ",
                                Some(id_prefix_len),
                                true,
                                effective_detailed,
                                cli.no_color,
                            )
                        )?;
                    }
                }
            }
            Some(ProjectsSubcommand::New(args)) => {
                let title = args.title.trim();
                if title.is_empty() {
                    eprintln!("Project title cannot be empty.");
                    return Ok(());
                }

                let store = cli.load_store()?;
                let now = now_ts();
                let mut props: BTreeMap<String, Value> = BTreeMap::new();
                props.insert("tt".to_string(), json!(title));
                props.insert("tp".to_string(), json!(i32::from(TaskType::Project)));
                props.insert("ss".to_string(), json!(i32::from(TaskStatus::Incomplete)));
                props.insert("st".to_string(), json!(i32::from(TaskStart::Anytime)));
                props.insert("tr".to_string(), json!(false));
                props.insert("cd".to_string(), json!(now));
                props.insert("md".to_string(), json!(now));
                props.insert(
                    "nt".to_string(),
                    if args.notes.is_empty() {
                        Value::Null
                    } else {
                        task6_note(&args.notes)
                    },
                );
                props.insert("xx".to_string(), json!({"_t":"oo","sn":{}}));
                props.insert("icp".to_string(), json!(true));
                props.insert("rmd".to_string(), Value::Null);
                props.insert("rp".to_string(), Value::Null);

                if let Some(area_id) = &args.area {
                    let (area_opt, err, _) = store.resolve_area_identifier(area_id);
                    let Some(area) = area_opt else {
                        eprintln!("{err}");
                        return Ok(());
                    };
                    props.insert("ar".to_string(), json!([area.uuid]));
                }

                if let Some(when_raw) = &args.when {
                    let when = when_raw.trim().to_lowercase();
                    if when == "anytime" {
                        props.insert("st".to_string(), json!(1));
                        props.insert("sr".to_string(), Value::Null);
                    } else if when == "someday" {
                        props.insert("st".to_string(), json!(2));
                        props.insert("sr".to_string(), Value::Null);
                    } else if when == "today" {
                        let ts = today_ts();
                        props.insert("st".to_string(), json!(1));
                        props.insert("sr".to_string(), json!(ts));
                        props.insert("tir".to_string(), json!(ts));
                    } else {
                        let day = match parse_day(Some(when_raw), "--when") {
                            Ok(Some(day)) => day,
                            Ok(None) => return Ok(()),
                            Err(e) => {
                                eprintln!("{e}");
                                return Ok(());
                            }
                        };
                        let ts = day_to_timestamp(day);
                        props.insert("st".to_string(), json!(2));
                        props.insert("sr".to_string(), json!(ts));
                        props.insert("tir".to_string(), json!(ts));
                    }
                }

                if let Some(tags) = &args.tags {
                    let (tag_ids, err) = resolve_tag_ids(&store, tags);
                    if !err.is_empty() {
                        eprintln!("{err}");
                        return Ok(());
                    }
                    props.insert("tg".to_string(), json!(tag_ids));
                }

                if let Some(deadline) = &args.deadline_date {
                    let day = match parse_day(Some(deadline), "--deadline") {
                        Ok(Some(day)) => day,
                        Ok(None) => return Ok(()),
                        Err(e) => {
                            eprintln!("{e}");
                            return Ok(());
                        }
                    };
                    props.insert("dd".to_string(), json!(day_to_timestamp(day)));
                }

                let uuid = random_task_id();
                let mut writer = LiveCloudWriter::new()?;

                let mut changes = BTreeMap::new();
                changes.insert(
                    uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Create,
                        entity_type: Some(EntityType::Task6),
                        properties: props,
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to create project: {e}");
                    return Ok(());
                }

                writeln!(
                    out,
                    "{} {}  {}",
                    colored(&format!("{} Created", ICONS.done), &[GREEN], cli.no_color),
                    title,
                    colored(&uuid, &[DIM], cli.no_color)
                )?;
            }
            Some(ProjectsSubcommand::Edit(args)) => {
                let store = cli.load_store()?;
                let plan = match build_projects_edit_plan(args, &store, now_ts()) {
                    Ok(plan) => plan,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    plan.project.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::from(plan.project.entity.clone())),
                        properties: plan.update.clone().into_properties(),
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to edit project: {e}");
                    return Ok(());
                }

                let title = plan.update.title.as_deref().unwrap_or(&plan.project.title);
                writeln!(
                    out,
                    "{} {}  {} {}",
                    colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                    title,
                    colored(&plan.project.uuid, &[DIM], cli.no_color),
                    colored(
                        &format!("({})", plan.labels.join(", ")),
                        &[DIM],
                        cli.no_color
                    )
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{fold_items, ThingsStore};
    use crate::wire::{EntityType, OperationType, WireItem, WireObject};

    const NOW: f64 = 1_700_000_222.0;
    const PROJECT_UUID: &str = "KGvAPpMrzHAKMdgMiERP1V";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item: WireItem = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn project(uuid: &str, title: &str, tags: Vec<&str>) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Task6),
                properties: BTreeMap::from([
                    ("tt".to_string(), json!(title)),
                    ("tp".to_string(), json!(1)),
                    ("ss".to_string(), json!(0)),
                    ("st".to_string(), json!(1)),
                    ("ix".to_string(), json!(0)),
                    (
                        "tg".to_string(),
                        json!(tags.into_iter().map(str::to_string).collect::<Vec<_>>()),
                    ),
                    ("cd".to_string(), json!(1)),
                    ("md".to_string(), json!(1)),
                ]),
            },
        )
    }

    fn area(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Area3),
                properties: BTreeMap::from([
                    ("tt".to_string(), json!(title)),
                    ("ix".to_string(), json!(0)),
                ]),
            },
        )
    }

    fn tag(uuid: &str, title: &str) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Tag4),
                properties: BTreeMap::from([
                    ("tt".to_string(), json!(title)),
                    ("ix".to_string(), json!(0)),
                ]),
            },
        )
    }

    #[test]
    fn projects_edit_payload_variants() {
        let target_area_uuid = "JFdhhhp37fpryAKu8UXwzK";
        let store = build_store(vec![
            project(PROJECT_UUID, "Roadmap", vec![]),
            area(target_area_uuid, "Personal"),
        ]);

        let title_plan = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: Some("Roadmap v2".to_string()),
                move_target: None,
                notes: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect("title plan");
        let p = title_plan.update.into_properties();
        assert_eq!(p.get("tt"), Some(&json!("Roadmap v2")));
        assert_eq!(p.get("md"), Some(&json!(NOW)));

        let clear_plan = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: None,
                move_target: Some("clear".to_string()),
                notes: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect("clear plan");
        assert_eq!(
            clear_plan.update.into_properties().get("ar"),
            Some(&json!([]))
        );

        let move_plan = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: None,
                move_target: Some(target_area_uuid.to_string()),
                notes: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect("move area plan");
        assert_eq!(
            move_plan.update.into_properties().get("ar"),
            Some(&json!([target_area_uuid]))
        );
    }

    #[test]
    fn projects_edit_tags_and_errors() {
        let tag1 = "WukwpDdL5Z88nX3okGMKTC";
        let tag2 = "JiqwiDaS3CAyjCmHihBDnB";
        let store = build_store(vec![
            project(PROJECT_UUID, "Roadmap", vec![tag1, tag2]),
            tag(tag1, "Work"),
            tag(tag2, "Focus"),
        ]);

        let remove_plan = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: None,
                move_target: None,
                notes: None,
                add_tags: None,
                remove_tags: Some("Work".to_string()),
            },
            &store,
            NOW,
        )
        .expect("remove tags");
        assert_eq!(
            remove_plan.update.into_properties().get("tg"),
            Some(&json!([tag2]))
        );

        let no_change = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: None,
                move_target: None,
                notes: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect_err("no changes");
        assert_eq!(no_change, "No edit changes requested.");

        let inbox = build_projects_edit_plan(
            &ProjectsEditArgs {
                project_id: PROJECT_UUID.to_string(),
                title: None,
                move_target: Some("inbox".to_string()),
                notes: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect_err("cannot move inbox");
        assert_eq!(inbox, "Projects cannot be moved to Inbox.");
    }
}
