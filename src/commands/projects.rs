use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{
    BOLD, DIM, GREEN, ICONS, colored, day_to_timestamp, fmt_project_with_note, id_prefix,
    parse_day, resolve_tag_ids, task6_note,
};
use crate::ids::random_task_id;
use crate::wire::{EntityType, OperationType, TaskStart, TaskStatus, TaskType, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand};
use serde_json::{Value, json};
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
                let (email, password) = load_auth()?;
                let mut client = ThingsCloudClient::new(email, password)?;
                let _ = client.authenticate();

                let mut changes = BTreeMap::new();
                changes.insert(
                    uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Create,
                        entity_type: Some(EntityType::Task6),
                        properties: props,
                    },
                );
                if let Err(e) = client.commit(changes, None) {
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
                let (project_opt, err, _) = store.resolve_mark_identifier(&args.project_id);
                let Some(project) = project_opt else {
                    eprintln!("{err}");
                    return Ok(());
                };
                if !project.is_project() {
                    eprintln!("The specified ID is not a project.");
                    return Ok(());
                }

                let mut update: BTreeMap<String, Value> = BTreeMap::new();
                let mut labels: Vec<String> = Vec::new();

                if let Some(title) = &args.title {
                    let title = title.trim();
                    if title.is_empty() {
                        eprintln!("Project title cannot be empty.");
                        return Ok(());
                    }
                    update.insert("tt".to_string(), json!(title));
                    labels.push("title".to_string());
                }

                if let Some(notes) = &args.notes {
                    update.insert(
                        "nt".to_string(),
                        if notes.is_empty() {
                            json!({"_t":"tx","t":1,"ch":0,"v":""})
                        } else {
                            task6_note(notes)
                        },
                    );
                    labels.push("notes".to_string());
                }

                if let Some(move_target) = &args.move_target {
                    let move_raw = move_target.trim();
                    let move_l = move_raw.to_lowercase();
                    if move_l == "inbox" {
                        eprintln!("Projects cannot be moved to Inbox.");
                        return Ok(());
                    }
                    if move_l == "clear" {
                        update.insert("ar".to_string(), json!([]));
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
                            eprintln!(
                                "Ambiguous --move target '{}' (matches project and area).",
                                move_raw
                            );
                            return Ok(());
                        }
                        if project_uuid.is_some() {
                            eprintln!("Projects can only be moved to an area or clear.");
                            return Ok(());
                        }
                        if let Some(area_uuid) = area_uuid {
                            update.insert("ar".to_string(), json!([area_uuid]));
                            labels.push(format!("move={move_raw}"));
                        } else {
                            eprintln!("Container not found: {move_raw}");
                            return Ok(());
                        }
                    }
                }

                let mut current_tags = project.tags.clone();
                if let Some(add_tags) = &args.add_tags {
                    let (ids, err) = resolve_tag_ids(&store, add_tags);
                    if !err.is_empty() {
                        eprintln!("{err}");
                        return Ok(());
                    }
                    for id in ids {
                        if !current_tags.iter().any(|t| t == &id) {
                            current_tags.push(id);
                        }
                    }
                    labels.push("add-tags".to_string());
                }
                if let Some(remove_tags) = &args.remove_tags {
                    let (ids, err) = resolve_tag_ids(&store, remove_tags);
                    if !err.is_empty() {
                        eprintln!("{err}");
                        return Ok(());
                    }
                    current_tags.retain(|t| !ids.iter().any(|id| id == t));
                    labels.push("remove-tags".to_string());
                }
                if args.add_tags.is_some() || args.remove_tags.is_some() {
                    update.insert("tg".to_string(), json!(current_tags));
                }

                if update.is_empty() {
                    eprintln!("No edit changes requested.");
                    return Ok(());
                }

                update.insert("md".to_string(), json!(now_ts()));

                let (email, password) = load_auth()?;
                let mut client = ThingsCloudClient::new(email, password)?;
                let _ = client.authenticate();
                let mut changes = BTreeMap::new();
                changes.insert(
                    project.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::from(project.entity.clone())),
                        properties: update.clone(),
                    },
                );
                if let Err(e) = client.commit(changes, None) {
                    eprintln!("Failed to edit project: {e}");
                    return Ok(());
                }

                let title = update
                    .get("tt")
                    .and_then(Value::as_str)
                    .unwrap_or(&project.title);
                writeln!(
                    out,
                    "{} {}  {} {}",
                    colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                    title,
                    colored(&project.uuid, &[DIM], cli.no_color),
                    colored(&format!("({})", labels.join(", ")), &[DIM], cli.no_color)
                )?;
            }
        }
        Ok(())
    }
}
