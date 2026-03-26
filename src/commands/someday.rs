use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{
    BOLD, CYAN, DIM, ICONS, colored, fmt_project_with_note, fmt_task_line, fmt_task_with_note,
};
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct SomedayArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for SomedayArgs {
    fn run(&self, cli: &Cli, out: &mut dyn Write) -> Result<()> {
        let store = cli.load_store()?;
        let items = store.someday();

        if items.is_empty() {
            writeln!(
                out,
                "{}",
                colored("Someday is empty.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        writeln!(
            out,
            "{}",
            colored(
                &format!("{} Someday  ({} items)", ICONS.task_someday, items.len()),
                &[BOLD, CYAN],
                cli.no_color,
            )
        )?;
        writeln!(out)?;

        let id_prefix_len =
            store.unique_prefix_length(&items.iter().map(|i| i.uuid.clone()).collect::<Vec<_>>());
        let projects = items
            .iter()
            .filter(|i| i.is_project())
            .cloned()
            .collect::<Vec<_>>();
        let tasks = items
            .iter()
            .filter(|i| !i.is_project())
            .cloned()
            .collect::<Vec<_>>();

        for item in projects {
            writeln!(
                out,
                "{}",
                fmt_project_with_note(
                    &item,
                    &store,
                    "  ",
                    Some(id_prefix_len),
                    true,
                    self.detailed.detailed,
                    cli.no_color,
                )
            )?;
        }

        if !tasks.is_empty()
            && !store
                .someday()
                .iter()
                .filter(|i| i.is_project())
                .collect::<Vec<_>>()
                .is_empty()
        {
            writeln!(out)?;
        }

        for item in tasks {
            let line = fmt_task_line(
                &item,
                &store,
                false,
                false,
                Some(id_prefix_len),
                cli.no_color,
            );
            writeln!(
                out,
                "{}",
                fmt_task_with_note(
                    line,
                    &item,
                    "  ",
                    Some(id_prefix_len),
                    self.detailed.detailed,
                    cli.no_color,
                )
            )?;
        }
        Ok(())
    }
}
