use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{colored, fmt_tasks_grouped, BOLD, CYAN, DIM, ICONS};
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct AnytimeArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for AnytimeArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let today = ctx.today();
        let tasks = store.anytime(&today);

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

        writeln!(
            out,
            "{}",
            fmt_tasks_grouped(
                &tasks,
                &store,
                "  ",
                true,
                self.detailed.detailed,
                &today,
                cli.no_color,
            )
        )?;
        Ok(())
    }
}
