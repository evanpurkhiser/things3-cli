use crate::app::Cli;
use crate::cloud_writer::{CloudWriter, LiveCloudWriter};
use crate::commands::Command;
use crate::common::{colored, resolve_single_tag, BOLD, DIM, GREEN, ICONS};
use crate::ids::random_task_id;
use crate::wire::{EntityType, OperationType, TagPatch, WireObject};
use anyhow::Result;
use chrono::Utc;
use clap::{Args, Subcommand};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

#[derive(Debug, Subcommand)]
pub enum TagsSubcommand {
    List(TagsListArgs),
    New(TagsNewArgs),
    Edit(TagsEditArgs),
    Delete(TagsDeleteArgs),
}

#[derive(Debug, Args)]
pub struct TagsArgs {
    #[command(subcommand)]
    pub command: Option<TagsSubcommand>,
}

#[derive(Debug, Default, Args)]
pub struct TagsListArgs {}

#[derive(Debug, Args)]
pub struct TagsNewArgs {
    pub name: String,
    #[arg(long)]
    pub parent: Option<String>,
}

#[derive(Debug, Args)]
pub struct TagsEditArgs {
    pub tag_id: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long = "move")]
    pub move_target: Option<String>,
}

#[derive(Debug, Args)]
pub struct TagsDeleteArgs {
    pub tag_id: String,
}

fn now_ts() -> f64 {
    Utc::now().timestamp_millis() as f64 / 1000.0
}

#[derive(Debug, Clone)]
struct TagsEditPlan {
    tag: crate::store::Tag,
    update: TagPatch,
    labels: Vec<String>,
}

fn build_tags_edit_plan(
    args: &TagsEditArgs,
    store: &crate::store::ThingsStore,
    now: f64,
) -> std::result::Result<TagsEditPlan, String> {
    let (tag, err) = resolve_single_tag(store, &args.tag_id);
    let Some(tag) = tag else {
        return Err(err);
    };

    let mut update = TagPatch::default();
    let mut labels = Vec::new();

    if let Some(name) = &args.name {
        let name = name.trim();
        if name.is_empty() {
            return Err("Tag name cannot be empty.".to_string());
        }
        update.title = Some(name.to_string());
        labels.push("name".to_string());
    }

    if let Some(move_target) = &args.move_target {
        let move_raw = move_target.trim();
        if move_raw.eq_ignore_ascii_case("clear") {
            update.parent_ids = Some(vec![]);
            labels.push("move=clear".to_string());
        } else {
            let (parent, err) = resolve_single_tag(store, move_raw);
            let Some(parent) = parent else {
                return Err(err);
            };
            if parent.uuid == tag.uuid {
                return Err("A tag cannot be its own parent.".to_string());
            }
            update.parent_ids = Some(vec![parent.uuid]);
            labels.push(format!("move={move_raw}"));
        }
    }

    if update.is_empty() {
        return Err("No edit changes requested.".to_string());
    }

    update.modification_date = Some(now);

    Ok(TagsEditPlan {
        tag,
        update,
        labels,
    })
}

impl Command for TagsArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        match self
            .command
            .as_ref()
            .unwrap_or(&TagsSubcommand::List(TagsListArgs::default()))
        {
            TagsSubcommand::List(_) => {
                let store = cli.load_store()?;
                let tags = store.tags();
                if tags.is_empty() {
                    writeln!(out, "{}", colored("No tags.", &[DIM], cli.no_color))?;
                    return Ok(());
                }

                writeln!(
                    out,
                    "{}",
                    colored(
                        &format!("{} Tags  ({})", ICONS.tag, tags.len()),
                        &[BOLD],
                        cli.no_color,
                    )
                )?;
                writeln!(out)?;

                let by_uuid: HashMap<_, _> =
                    tags.iter().map(|t| (t.uuid.clone(), t.clone())).collect();
                let mut children: BTreeMap<String, Vec<_>> = BTreeMap::new();
                let mut top_level = Vec::new();

                for tag in tags {
                    if let Some(parent_uuid) = &tag.parent_uuid {
                        if by_uuid.contains_key(parent_uuid) {
                            children.entry(parent_uuid.clone()).or_default().push(tag);
                        } else {
                            top_level.push(tag);
                        }
                    } else {
                        top_level.push(tag);
                    }
                }

                fn shortcut(tag: &crate::store::Tag, no_color: bool) -> String {
                    if let Some(shortcut) = &tag.shortcut {
                        return colored(&format!("  [{shortcut}]"), &[DIM], no_color);
                    }
                    String::new()
                }

                fn print_subtags(
                    subtags: &[crate::store::Tag],
                    indent: &str,
                    children: &BTreeMap<String, Vec<crate::store::Tag>>,
                    no_color: bool,
                    out: &mut dyn Write,
                ) -> Result<()> {
                    for (i, tag) in subtags.iter().enumerate() {
                        let is_last = i == subtags.len() - 1;
                        let connector =
                            colored(if is_last { "└╴" } else { "├╴" }, &[DIM], no_color);
                        writeln!(
                            out,
                            "  {}{}{} {}{}",
                            indent,
                            connector,
                            colored(ICONS.tag, &[DIM], no_color),
                            tag.title,
                            shortcut(tag, no_color)
                        )?;
                        if let Some(grandchildren) = children.get(&tag.uuid) {
                            let child_indent = if is_last {
                                format!("{}  ", indent)
                            } else {
                                format!("{}{} ", indent, colored("│", &[DIM], no_color))
                            };
                            print_subtags(grandchildren, &child_indent, children, no_color, out)?;
                        }
                    }
                    Ok(())
                }

                for tag in top_level {
                    writeln!(
                        out,
                        "  {} {}{}",
                        colored(ICONS.tag, &[DIM], cli.no_color),
                        tag.title,
                        shortcut(&tag, cli.no_color)
                    )?;
                    if let Some(subtags) = children.get(&tag.uuid) {
                        print_subtags(subtags, "", &children, cli.no_color, out)?;
                    }
                }
            }
            TagsSubcommand::New(args) => {
                let name = args.name.trim();
                if name.is_empty() {
                    eprintln!("Tag name cannot be empty.");
                    return Ok(());
                }

                let store = cli.load_store()?;
                let mut props = BTreeMap::new();
                props.insert("tt".to_string(), json!(name));
                props.insert("ix".to_string(), json!(0));
                props.insert("xx".to_string(), json!({"_t":"oo","sn":{}}));

                if let Some(parent_raw) = &args.parent {
                    let (parent, err) = resolve_single_tag(&store, parent_raw);
                    let Some(parent) = parent else {
                        eprintln!("{err}");
                        return Ok(());
                    };
                    props.insert("pn".to_string(), json!([parent.uuid]));
                }

                let uuid = random_task_id();
                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Create,
                        entity_type: Some(EntityType::Tag4),
                        properties: props,
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to create tag: {e}");
                    return Ok(());
                }

                writeln!(
                    out,
                    "{} {}  {}",
                    colored(&format!("{} Created", ICONS.done), &[GREEN], cli.no_color),
                    name,
                    colored(&uuid, &[DIM], cli.no_color)
                )?;
            }
            TagsSubcommand::Edit(args) => {
                let store = cli.load_store()?;
                let plan = match build_tags_edit_plan(args, &store, now_ts()) {
                    Ok(plan) => plan,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    plan.tag.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Update,
                        entity_type: Some(EntityType::Tag4),
                        properties: plan.update.clone().into_properties(),
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to edit tag: {e}");
                    return Ok(());
                }

                let name = plan.update.title.as_deref().unwrap_or(&plan.tag.title);
                writeln!(
                    out,
                    "{} {}  {} {}",
                    colored(&format!("{} Edited", ICONS.done), &[GREEN], cli.no_color),
                    name,
                    colored(&plan.tag.uuid, &[DIM], cli.no_color),
                    colored(
                        &format!("({})", plan.labels.join(", ")),
                        &[DIM],
                        cli.no_color
                    )
                )?;
            }
            TagsSubcommand::Delete(args) => {
                let store = cli.load_store()?;
                let (tag, err) = resolve_single_tag(&store, &args.tag_id);
                let Some(tag) = tag else {
                    eprintln!("{err}");
                    return Ok(());
                };

                let mut writer = LiveCloudWriter::new()?;
                let mut changes = BTreeMap::new();
                changes.insert(
                    tag.uuid.clone(),
                    WireObject {
                        operation_type: OperationType::Delete,
                        entity_type: Some(EntityType::Tag4),
                        properties: BTreeMap::new(),
                    },
                );
                if let Err(e) = writer.commit(changes, None) {
                    eprintln!("Failed to delete tag: {e}");
                    return Ok(());
                }

                writeln!(
                    out,
                    "{} {}  {}",
                    colored(
                        &format!("{} Deleted", ICONS.deleted),
                        &[GREEN],
                        cli.no_color
                    ),
                    tag.title,
                    colored(&tag.uuid, &[DIM], cli.no_color)
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
    const TAG_UUID: &str = "WukwpDdL5Z88nX3okGMKTC";
    const CHILD_UUID: &str = "JiqwiDaS3CAyjCmHihBDnB";

    fn build_store(entries: Vec<(String, WireObject)>) -> ThingsStore {
        let mut item: WireItem = BTreeMap::new();
        for (uuid, obj) in entries {
            item.insert(uuid, obj);
        }
        ThingsStore::from_raw_state(&fold_items([item]))
    }

    fn tag(uuid: &str, title: &str, parent: Option<&str>) -> (String, WireObject) {
        let mut props = BTreeMap::from([
            ("tt".to_string(), json!(title)),
            ("ix".to_string(), json!(0)),
        ]);
        if let Some(parent) = parent {
            props.insert("pn".to_string(), json!([parent]));
        }
        (
            uuid.to_string(),
            WireObject {
                operation_type: OperationType::Create,
                entity_type: Some(EntityType::Tag4),
                properties: props,
            },
        )
    }

    #[test]
    fn tags_edit_payloads_and_errors() {
        let store = build_store(vec![
            tag(TAG_UUID, "Work", None),
            tag(CHILD_UUID, "Meetings", Some(TAG_UUID)),
        ]);

        let rename = build_tags_edit_plan(
            &TagsEditArgs {
                tag_id: TAG_UUID.to_string(),
                name: Some("Work Stuff".to_string()),
                move_target: None,
            },
            &store,
            NOW,
        )
        .expect("rename");
        let p = rename.update.into_properties();
        assert_eq!(p.get("tt"), Some(&json!("Work Stuff")));
        assert_eq!(p.get("md"), Some(&json!(NOW)));

        let reparent = build_tags_edit_plan(
            &TagsEditArgs {
                tag_id: CHILD_UUID.to_string(),
                name: None,
                move_target: Some(TAG_UUID.to_string()),
            },
            &store,
            NOW,
        )
        .expect("reparent");
        assert_eq!(
            reparent.update.into_properties().get("pn"),
            Some(&json!([TAG_UUID]))
        );

        let clear = build_tags_edit_plan(
            &TagsEditArgs {
                tag_id: CHILD_UUID.to_string(),
                name: None,
                move_target: Some("clear".to_string()),
            },
            &store,
            NOW,
        )
        .expect("clear");
        assert_eq!(clear.update.into_properties().get("pn"), Some(&json!([])));

        let no_change = build_tags_edit_plan(
            &TagsEditArgs {
                tag_id: TAG_UUID.to_string(),
                name: None,
                move_target: None,
            },
            &store,
            NOW,
        )
        .expect_err("no changes");
        assert_eq!(no_change, "No edit changes requested.");

        let self_parent = build_tags_edit_plan(
            &TagsEditArgs {
                tag_id: TAG_UUID.to_string(),
                name: None,
                move_target: Some(TAG_UUID.to_string()),
            },
            &store,
            NOW,
        )
        .expect_err("self parent");
        assert_eq!(self_parent, "A tag cannot be its own parent.");
    }
}
