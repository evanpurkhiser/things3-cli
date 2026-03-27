use crate::app::Cli;
use crate::commands::{Command, TagDeltaArgs};
use crate::common::{
    colored, day_to_timestamp, fmt_project_with_note, id_prefix, parse_day, resolve_tag_ids,
    task6_note, BOLD, DIM, GREEN, ICONS,
};
use crate::ids::ThingsId;
use crate::wire::notes::{StructuredTaskNotes, TaskNotes};
use crate::wire::task::{TaskPatch, TaskProps, TaskStart, TaskStatus, TaskType};
use crate::wire::wire_object::{EntityType, WireObject};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::collections::BTreeMap;

#[derive(Debug, Subcommand)]
pub enum ProjectsSubcommand {
    #[command(about = "Show all active projects")]
    List(ProjectsListArgs),
    #[command(about = "Create a new project")]
    New(ProjectsNewArgs),
    #[command(about = "Edit a project title, notes, area, or tags")]
    Edit(ProjectsEditArgs),
}

#[derive(Debug, Args)]
#[command(about = "Show, create, or edit projects")]
pub struct ProjectsArgs {
    /// Show notes for each project.
    #[arg(long)]
    pub detailed: bool,
    #[command(subcommand)]
    pub command: Option<ProjectsSubcommand>,
}

#[derive(Debug, Default, Args)]
pub struct ProjectsListArgs {
    /// Show notes for each task
    #[arg(long)]
    pub detailed: bool,
}

#[derive(Debug, Args)]
pub struct ProjectsNewArgs {
    /// Project title
    pub title: String,
    #[arg(long, help = "Area UUID/prefix to place the project in")]
    pub area: Option<String>,
    #[arg(
        long,
        help = "Schedule: anytime (default), someday, today, or YYYY-MM-DD"
    )]
    pub when: Option<String>,
    #[arg(long, default_value = "", help = "Project notes")]
    pub notes: String,
    #[arg(long, help = "Comma-separated tags (titles or UUID prefixes)")]
    pub tags: Option<String>,
    #[arg(long = "deadline", help = "Deadline date (YYYY-MM-DD)")]
    pub deadline_date: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectsEditArgs {
    /// Project UUID (or unique UUID prefix)
    pub project_id: String,
    #[arg(long, help = "Replace title")]
    pub title: Option<String>,
    #[arg(long = "move", help = "Move to clear or area UUID/prefix")]
    pub move_target: Option<String>,
    #[arg(long, help = "Replace notes (use empty string to clear)")]
    pub notes: Option<String>,
    #[command(flatten)]
    pub tag_delta: TagDeltaArgs,
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
            TaskNotes::Structured(StructuredTaskNotes {
                object_type: Some("tx".to_string()),
                format_type: 1,
                ch: Some(0),
                v: Some(String::new()),
                ps: Vec::new(),
                unknown_fields: Default::default(),
            })
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
                let area_id = ThingsId::from(area_uuid);
                update.area_ids = Some(vec![area_id]);
                labels.push(format!("move={move_raw}"));
            } else {
                return Err(format!("Container not found: {move_raw}"));
            }
        }
    }

    let mut current_tags = project.tags.clone();
    if let Some(add_tags) = &args.tag_delta.add_tags {
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
    if let Some(remove_tags) = &args.tag_delta.remove_tags {
        let (ids, err) = resolve_tag_ids(store, remove_tags);
        if !err.is_empty() {
            return Err(err);
        }
        current_tags.retain(|t| !ids.iter().any(|id| id == t));
        labels.push("remove-tags".to_string());
    }
    if args.tag_delta.add_tags.is_some() || args.tag_delta.remove_tags.is_some() {
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
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        // Match Python argparse behavior:
        // - `projects --detailed` (no subcommand) => detailed output
        // - `projects list --detailed` => detailed output
        // - `projects --detailed list` => not detailed (subcommand parser default wins)
        let effective_detailed = match self.command.as_ref() {
            None => self.detailed,
            Some(ProjectsSubcommand::List(la)) => la.detailed,
            _ => false,
        };

        match &self.command {
            None | Some(ProjectsSubcommand::List(_)) => {
                let store = cli.load_store()?;
                let today = ctx.today();
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

                let mut by_area: BTreeMap<Option<ThingsId>, Vec<_>> = BTreeMap::new();
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
                                false,
                                effective_detailed,
                                &today,
                                cli.no_color,
                            )
                        )?;
                    }
                }

                // Sort areas by their index field so output order matches Python
                let mut area_entries: Vec<(ThingsId, Vec<_>)> = by_area
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
                                false,
                                effective_detailed,
                                &today,
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
                let now = ctx.now_timestamp();
                let mut props = TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Project,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Anytime,
                    instance_creation_paused: true,
                    creation_date: Some(now),
                    modification_date: Some(now),
                    ..Default::default()
                };
                if !args.notes.is_empty() {
                    props.notes = Some(task6_note(&args.notes));
                }

                if let Some(area_id) = &args.area {
                    let (area_opt, err, _) = store.resolve_area_identifier(area_id);
                    let Some(area) = area_opt else {
                        eprintln!("{err}");
                        return Ok(());
                    };
                    props.area_ids = vec![area.uuid.into()];
                }

                if let Some(when_raw) = &args.when {
                    let when = when_raw.trim().to_lowercase();
                    if when == "anytime" {
                        props.start_location = TaskStart::Anytime;
                        props.scheduled_date = None;
                    } else if when == "someday" {
                        props.start_location = TaskStart::Someday;
                        props.scheduled_date = None;
                    } else if when == "today" {
                        let ts = ctx.today_timestamp();
                        props.start_location = TaskStart::Anytime;
                        props.scheduled_date = Some(ts);
                        props.today_index_reference = Some(ts);
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
                        props.start_location = TaskStart::Someday;
                        props.scheduled_date = Some(ts);
                        props.today_index_reference = Some(ts);
                    }
                }

                if let Some(tags) = &args.tags {
                    let (tag_ids, err) = resolve_tag_ids(&store, tags);
                    if !err.is_empty() {
                        eprintln!("{err}");
                        return Ok(());
                    }
                    props.tag_ids = tag_ids;
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
                    props.deadline = Some(day_to_timestamp(day) as i64);
                }

                let uuid = ctx.next_id();

                let mut changes = BTreeMap::new();
                changes.insert(uuid.clone(), WireObject::create(EntityType::Task6, props));
                if let Err(e) = ctx.commit_changes(changes, None) {
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
                let plan = match build_projects_edit_plan(args, &store, ctx.now_timestamp()) {
                    Ok(plan) => plan,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

                let mut changes = BTreeMap::new();
                changes.insert(
                    plan.project.uuid.to_string(),
                    WireObject::update(
                        EntityType::from(plan.project.entity.clone()),
                        plan.update.clone(),
                    ),
                );
                if let Err(e) = ctx.commit_changes(changes, None) {
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
    use crate::ids::ThingsId;
    use crate::store::{fold_items, ThingsStore};
    use crate::wire::area::AreaProps;
    use crate::wire::tags::TagProps;
    use crate::wire::task::{TaskProps, TaskStart, TaskStatus, TaskType};
    use crate::wire::wire_object::WireItem;
    use crate::wire::wire_object::{EntityType, WireObject};
    use serde_json::json;

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
            WireObject::create(
                EntityType::Task6,
                TaskProps {
                    title: title.to_string(),
                    item_type: TaskType::Project,
                    status: TaskStatus::Incomplete,
                    start_location: TaskStart::Anytime,
                    sort_index: 0,
                    tag_ids: tags.iter().map(|t| ThingsId::from(*t)).collect(),
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: Some("Work".to_string()),
                },
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
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
                tag_delta: TagDeltaArgs {
                    add_tags: None,
                    remove_tags: None,
                },
            },
            &store,
            NOW,
        )
        .expect_err("cannot move inbox");
        assert_eq!(inbox, "Projects cannot be moved to Inbox.");
    }
}
