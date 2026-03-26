use crate::app::Cli;
use crate::cloud_writer::{CloudWriter, LiveCloudWriter};
use crate::commands::Command;
use crate::common::{colored, id_prefix, resolve_tag_ids, BOLD, DIM, GREEN, ICONS, MAGENTA};
use crate::ids::random_task_id;
use crate::wire::{AreaPatch, EntityType, OperationType, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand};
use serde_json::json;
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

#[derive(Debug, Clone)]
struct AreasEditPlan {
    area: crate::store::Area,
    update: AreaPatch,
    labels: Vec<String>,
}

fn build_areas_edit_plan(
    args: &AreasEditArgs,
    store: &crate::store::ThingsStore,
    now: f64,
) -> std::result::Result<AreasEditPlan, String> {
    let (area_opt, err, _) = store.resolve_area_identifier(&args.area_id);
    let Some(area) = area_opt else {
        return Err(err);
    };

    let mut update = AreaPatch::default();
    let mut labels = Vec::new();

    if let Some(title) = &args.title {
        let title = title.trim();
        if title.is_empty() {
            return Err("Area title cannot be empty.".to_string());
        }
        update.title = Some(title.to_string());
        labels.push("title".to_string());
    }

    let mut current_tags = area.tags.clone();
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

    Ok(AreasEditPlan {
        area,
        update,
        labels,
    })
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
                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Create,
                        entity_type: Some(EntityType::Area3),
                        properties: props,
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
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
                let plan = match build_areas_edit_plan(args, &store, now_ts()) {
                    Ok(plan) => plan,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    plan.area.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::Area3),
                        properties: plan.update.clone().into_properties(),
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to edit area: {e}");
                    return Ok(());
                }

                let title = plan.update.title.as_deref().unwrap_or(&plan.area.title);
                writeln!(
                    out,
                    "{} {}  {} {}",
                    colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                    title,
                    colored(&plan.area.uuid, &[DIM], cli.no_color),
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
    const AREA_UUID: &str = "MpkEei6ybkFS2n6SXvwfLf";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item: WireItem = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn area(uuid: &str, title: &str, tags: Vec<&str>) -> (String, WireObject) {
        (
            uuid.to_string(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Area3),
                properties: BTreeMap::from([
                    ("tt".to_string(), json!(title)),
                    ("tg".to_string(), json!(tags)),
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
    fn areas_edit_payload_and_errors() {
        let tag1 = "WukwpDdL5Z88nX3okGMKTC";
        let tag2 = "JiqwiDaS3CAyjCmHihBDnB";
        let store = build_store(vec![
            area(AREA_UUID, "Home", vec![tag1, tag2]),
            tag(tag1, "Work"),
            tag(tag2, "Focus"),
        ]);

        let title = build_areas_edit_plan(
            &AreasEditArgs {
                area_id: AREA_UUID.to_string(),
                title: Some("New Name".to_string()),
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect("title plan");
        let p = title.update.into_properties();
        assert_eq!(p.get("tt"), Some(&json!("New Name")));
        assert_eq!(p.get("md"), Some(&json!(NOW)));

        let remove = build_areas_edit_plan(
            &AreasEditArgs {
                area_id: AREA_UUID.to_string(),
                title: None,
                add_tags: None,
                remove_tags: Some("Work".to_string()),
            },
            &store,
            NOW,
        )
        .expect("remove tag");
        assert_eq!(
            remove.update.into_properties().get("tg"),
            Some(&json!([tag2]))
        );

        let no_change = build_areas_edit_plan(
            &AreasEditArgs {
                area_id: AREA_UUID.to_string(),
                title: None,
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect_err("no change");
        assert_eq!(no_change, "No edit changes requested.");

        let empty_title = build_areas_edit_plan(
            &AreasEditArgs {
                area_id: AREA_UUID.to_string(),
                title: Some("".to_string()),
                add_tags: None,
                remove_tags: None,
            },
            &store,
            NOW,
        )
        .expect_err("empty title");
        assert_eq!(empty_title, "Area title cannot be empty.");
    }
}
