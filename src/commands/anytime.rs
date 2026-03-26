use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{BOLD, CYAN, DIM, ICONS, colored, fmt_task_line, fmt_task_with_note};
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct AnytimeArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for AnytimeArgs {
    fn run(&self, cli: &Cli, out: &mut dyn Write) -> Result<()> {
        let store = cli.load_store()?;
        let tasks = store.anytime();

        if tasks.is_empty() {
            writeln!(
                out,
                "{}",
                colored("Anytime is empty.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        writeln!(
            out,
            "{}",
            colored(
                &format!("{} Anytime  ({} tasks)", ICONS.anytime, tasks.len()),
                &[BOLD, CYAN],
                cli.no_color,
            )
        )?;
        writeln!(out)?;

        let id_prefix_len =
            store.unique_prefix_length(&tasks.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());
        for task in tasks {
            let line = fmt_task_line(
                &task,
                &store,
                false,
                true,
                Some(id_prefix_len),
                cli.no_color,
            );
            writeln!(
                out,
                "{}",
                fmt_task_with_note(
                    line,
                    &task,
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
