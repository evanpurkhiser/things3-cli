use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use clap::{Args, Subcommand};
use iocraft::prelude::*;

use crate::{
    app::Cli,
    commands::{Command, write_json},
    common::{DIM, GREEN, ICONS, colored, resolve_single_tag},
    ui::{
        render_element_to_string,
        views::{json::common::build_tags_json, tags::TagsView},
    },
    wire::{
        tags::{TagPatch, TagProps},
        wire_object::{EntityType, WireObject},
    },
};

#[derive(Debug, Subcommand)]
pub enum TagsSubcommand {
    #[command(about = "Show all tags")]
    List(TagsListArgs),
    #[command(about = "Create a new tag")]
    New(TagsNewArgs),
    #[command(about = "Rename or reparent a tag")]
    Edit(TagsEditArgs),
    #[command(about = "Delete a tag")]
    Delete(TagsDeleteArgs),
}

#[derive(Debug, Args)]
#[command(about = "Show or edit tags")]
pub struct TagsArgs {
    #[command(subcommand)]
    pub command: Option<TagsSubcommand>,
}

#[derive(Debug, Default, Args)]
pub struct TagsListArgs {}

#[derive(Debug, Args)]
pub struct TagsNewArgs {
    /// Tag title
    pub name: String,
    #[arg(long, short = 'p', help = "Parent tag title or UUID/prefix")]
    pub parent: Option<String>,
}

#[derive(Debug, Args)]
pub struct TagsEditArgs {
    /// Tag title or UUID/prefix
    pub tag_id: String,
    #[arg(long, short = 'n', help = "Replace tag title")]
    pub name: Option<String>,
    #[arg(long = "move", short = 'm', help = "Move under another tag or clear")]
    pub move_target: Option<String>,
}

#[derive(Debug, Args)]
pub struct TagsDeleteArgs {
    /// Tag title or UUID/prefix
    pub tag_id: String,
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
            let parent_id = parent.uuid;
            update.parent_ids = Some(vec![parent_id]);
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
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        match self
            .command
            .as_ref()
            .unwrap_or(&TagsSubcommand::List(TagsListArgs::default()))
        {
            TagsSubcommand::List(_) => {
                let store = cli.load_store()?;
                let tags = store.tags();

                if cli.json {
                    write_json(out, &build_tags_json(&tags, &store))?;
                    return Ok(());
                }

                let by_uuid: HashMap<_, _> =
                    tags.iter().map(|t| (t.uuid.clone(), t.clone())).collect();
                let mut children: BTreeMap<_, Vec<_>> = BTreeMap::new();
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

                let mut ui = element! {
                    TagsView(tags_count: by_uuid.len(), top_level, children)
                };
                let rendered = render_element_to_string(&mut ui, cli.no_color);
                writeln!(out, "{}", rendered)?;
            }
            TagsSubcommand::New(args) => {
                let name = args.name.trim();
                if name.is_empty() {
                    eprintln!("Tag name cannot be empty.");
                    return Ok(());
                }

                let store = cli.load_store()?;
                let mut props = TagProps {
                    title: name.to_string(),
                    sort_index: 0,
                    ..Default::default()
                };

                if let Some(parent_raw) = &args.parent {
                    let (parent, err) = resolve_single_tag(&store, parent_raw);
                    let Some(parent) = parent else {
                        eprintln!("{err}");
                        return Ok(());
                    };
                    props.parent_ids = vec![parent.uuid.into()];
                }

                let uuid = ctx.next_id();
                let mut changes = BTreeMap::new();
                changes.insert(uuid.clone(), WireObject::create(EntityType::Tag4, props));
                if let Err(e) = ctx.commit_changes(changes, None) {
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
                let plan = match build_tags_edit_plan(args, &store, ctx.now_timestamp()) {
                    Ok(plan) => plan,
                    Err(err) => {
                        eprintln!("{err}");
                        return Ok(());
                    }
                };

                let mut changes = BTreeMap::new();
                changes.insert(
                    plan.tag.uuid.to_string(),
                    WireObject::update(EntityType::Tag4, plan.update.clone()),
                );
                if let Err(e) = ctx.commit_changes(changes, None) {
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

                let mut changes = BTreeMap::new();
                changes.insert(tag.uuid.to_string(), WireObject::delete(EntityType::Tag4));
                if let Err(e) = ctx.commit_changes(changes, None) {
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
    use serde_json::json;

    use super::*;
    use crate::{
        ids::ThingsId,
        store::{ThingsStore, fold_items},
        wire::{
            tags::TagProps,
            wire_object::{EntityType, WireItem, WireObject},
        },
    };

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
        (
            uuid.to_string(),
            WireObject::create(
                EntityType::Tag4,
                TagProps {
                    title: title.to_string(),
                    sort_index: 0,
                    parent_ids: parent
                        .map(|p| {
                            vec![
                                p.parse::<ThingsId>()
                                    .expect("test parent id should parse as ThingsId"),
                            ]
                        })
                        .unwrap_or_default(),
                    ..Default::default()
                },
            ),
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
