use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{
    BOLD, DIM, GREEN, ICONS, colored, fmt_date_local, fmt_task_line, fmt_task_with_note, parse_day,
};
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct LogbookArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
    #[arg(long = "from")]
    pub from_date: Option<String>,
    #[arg(long = "to")]
    pub to_date: Option<String>,
}

impl Command for LogbookArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let today = ctx.today();

        let from_day =
            parse_day(self.from_date.as_deref(), "--from").map_err(anyhow::Error::msg)?;
        let to_day = parse_day(self.to_date.as_deref(), "--to").map_err(anyhow::Error::msg)?;

        if let (Some(from), Some(to)) = (from_day, to_day)
            && from > to
        {
            eprintln!("--from date must be before or equal to --to date");
            return Ok(());
        }

        let tasks = store.logbook(from_day, to_day);
        if tasks.is_empty() {
            writeln!(
                out,
                "{}",
                colored("Logbook is empty.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        let id_prefix_len =
            store.unique_prefix_length(&tasks.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());

        writeln!(
            out,
            "{}",
            colored(
                &format!("{} Logbook  ({} tasks)", ICONS.done, tasks.len()),
                &[BOLD, GREEN],
                cli.no_color,
            )
        )?;

        let mut current_day = String::new();
        for task in tasks {
            let day = fmt_date_local(task.stop_date);
            if day != current_day {
                writeln!(out)?;
                writeln!(
                    out,
                    "{}",
                    colored(&format!("  {}", day), &[BOLD], cli.no_color)
                )?;
                current_day = day;
            }
            let line = fmt_task_line(
                &task,
                &store,
                true,
                false,
                false,
                Some(id_prefix_len),
                &today,
                cli.no_color,
            );
            writeln!(
                out,
                "{}",
                fmt_task_with_note(
                    line,
                    &task,
                    "    ",
                    Some(id_prefix_len),
                    self.detailed.detailed,
                    cli.no_color,
                )
            )?;
        }

        Ok(())
    }
}
