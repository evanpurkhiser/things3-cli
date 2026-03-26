use crate::app::Cli;
use crate::cloud_writer::{CloudWriter, LiveCloudWriter};
use crate::commands::Command;
use crate::common::{colored, DIM, GREEN, ICONS};
use crate::wire::{EntityType, OperationType, WireObject};
use anyhow::Result;
use clap::Args;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub item_ids: Vec<String>,
}

impl Command for DeleteArgs {
    fn run(&self, cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        let store = cli.load_store()?;
        let mut targets: Vec<(String, String, String)> = Vec::new();
        let mut seen = HashSet::new();

        for identifier in &self.item_ids {
            let (task, task_err, task_ambiguous) = store.resolve_task_identifier(identifier);
            let (area, area_err, area_ambiguous) = store.resolve_area_identifier(identifier);

            let task_match = task.is_some();
            let area_match = area.is_some();

            if task_match && area_match {
                eprintln!(
                    "Ambiguous identifier '{}' (matches task and area).",
                    identifier
                );
                continue;
            }

            if !task_match && !area_match {
                if !task_ambiguous.is_empty() && !area_ambiguous.is_empty() {
                    eprintln!(
                        "Ambiguous identifier '{}' (matches multiple tasks and areas).",
                        identifier
                    );
                } else if !task_ambiguous.is_empty() {
                    eprintln!("{task_err}");
                } else if !area_ambiguous.is_empty() {
                    eprintln!("{area_err}");
                } else {
                    eprintln!("Item not found: {identifier}");
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
                targets.push((task.uuid.clone(), task.entity.clone(), task.title.clone()));
                continue;
            }

            if let Some(area) = area {
                if !seen.insert(area.uuid.clone()) {
                    continue;
                }
                targets.push((area.uuid.clone(), "Area3".to_string(), area.title.clone()));
            }
        }

        if targets.is_empty() {
            return Ok(());
        }

        let mut writer = LiveCloudWriter::new()?;

        let mut changes = BTreeMap::new();
        for (uuid, entity, _title) in &targets {
            changes.insert(
                uuid.clone(),
                WireObject {
                    operation_type: OperationType::Delete,
                    entity_type: Some(EntityType::from(entity.clone())),
                    properties: BTreeMap::new(),
                },
            );
        }

        if let Err(e) = writer.commit(changes, None) {
            eprintln!("Failed to delete items: {e}");
            return Ok(());
        }

        for (uuid, _entity, title) in targets {
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
