use crate::app::Cli;
use crate::auth::load_auth;
use crate::client::ThingsCloudClient;
use crate::commands::Command;
use crate::common::{BOLD, DIM, GREEN, ICONS, MAGENTA, colored, id_prefix, resolve_tag_ids};
use crate::ids::random_task_id;
use crate::wire::{EntityType, OperationType, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Subcommand)]
pub enum AreasSubcommand {
    List(AreasListArgs),
    New(AreasNewArgs),
    Edit(AreasEditArgs),
}

#[derive(Debug, Args)]
pub struct AreasArgs {
    #[command(subcommand)]
    pub command: Option<AreasSubcommand>,
}

#[derive(Debug, Default, Args)]
pub struct AreasListArgs {}

#[derive(Debug, Args)]
pub struct AreasNewArgs {
    pub title: String,
    #[arg(long)]
    pub tags: Option<String>,
}

#[derive(Debug, Args)]
pub struct AreasEditArgs {
    pub area_id: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long = "add-tags")]
    pub add_tags: Option<String>,
    #[arg(long = "remove-tags")]
    pub remove_tags: Option<String>,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

impl Command for AreasArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        match self
            .command
            .as_ref()
            .unwrap_or(&AreasSubcommand::List(AreasListArgs::default()))
        {
            AreasSubcommand::List(_) => {
                let store = cli.load_store()?;
                let areas = store.areas();
                if areas.is_empty() {
                    writeln!(out, "{}", colored("No areas.", &[DIM], cli.no_color))?;
                    return Ok(());
                }

                writeln!(
                    out,
                    "{}",
                    colored(
                        &format!("{} Areas  ({})", ICONS.area, areas.len()),
                        &[BOLD, MAGENTA],
                        cli.no_color,
                    )
                )?;
                writeln!(out)?;

                let id_prefix_len = store.unique_prefix_length(
                    &areas.iter().map(|a| a.uuid.clone()).collect::<Vec<_>>(),
                );
                for area in areas {
                    let tags = if area.tags.is_empty() {
                        String::new()
                    } else {
                        let names = area
                            .tags
                            .iter()
                            .map(|t| store.resolve_tag_title(t))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("  {}", colored(&format!("[{names}]"), &[DIM], cli.no_color))
                    };
                    writeln!(
                        out,
                        "  {} {} {}{}",
                        id_prefix(&area.uuid, id_prefix_len, cli.no_color),
                        colored(ICONS.area, &[DIM], cli.no_color),
                        area.title,
                        tags
                    )?;
                }
            }
            AreasSubcommand::New(args) => {
                let title = args.title.trim();
                if title.is_empty() {
                    eprintln!("Area title cannot be empty.");
                    return Ok(());
                }

                let store = cli.load_store()?;
                let now = now_ts();
                let mut props = BTreeMap::new();
                props.insert("tt".to_string(), json!(title));
                props.insert("ix".to_string(), json!(0));
                props.insert("xx".to_string(), json!({"_t":"oo","sn":{}}));
                props.insert("cd".to_string(), json!(now));
                props.insert("md".to_string(), json!(now));

                if let Some(tags) = &args.tags {
                    let (tag_ids, err) = resolve_tag_ids(&store, tags);
                    if !err.is_empty() {
                        eprintln!("{err}");
                        return Ok(());
                    }
                    props.insert("tg".to_string(), json!(tag_ids));
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
                        entity_type: Some(EntityType::Area3),
                        properties: props,
                    },
                );
                if let Err(e) = client.commit(changes, None) {
                    eprintln!("Failed to create area: {e}");
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
            AreasSubcommand::Edit(args) => {
                let store = cli.load_store()?;
                let (area_opt, err, _) = store.resolve_area_identifier(&args.area_id);
                let Some(area) = area_opt else {
                    eprintln!("{err}");
                    return Ok(());
                };

                let mut update: BTreeMap<String, Value> = BTreeMap::new();
                let mut labels = Vec::new();

                if let Some(title) = &args.title {
                    let title = title.trim();
                    if title.is_empty() {
                        eprintln!("Area title cannot be empty.");
                        return Ok(());
                    }
                    update.insert("tt".to_string(), json!(title));
                    labels.push("title".to_string());
                }

                let mut current_tags = area.tags.clone();
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
                    area.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::Area3),
                        properties: update.clone(),
                    },
                );
                if let Err(e) = client.commit(changes, None) {
                    eprintln!("Failed to edit area: {e}");
                    return Ok(());
                }

                let title = update
                    .get("tt")
                    .and_then(Value::as_str)
                    .unwrap_or(&area.title);
                writeln!(
                    out,
                    "{} {}  {} {}",
                    colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                    title,
                    colored(&area.uuid, &[DIM], cli.no_color),
                    colored(&format!("({})", labels.join(", ")), &[DIM], cli.no_color)
                )?;
            }
        }
        Ok(())
    }
}
