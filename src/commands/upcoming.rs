use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{
    BOLD, CYAN, DIM, ICONS, colored, fmt_date, fmt_task_line, fmt_task_with_note, today_utc,
};
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct UpcomingArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for UpcomingArgs {
    fn run(&self, cli: &Cli, out: &mut dyn Write) -> Result<()> {
        let store = cli.load_store()?;
        let now_ts = today_utc().timestamp();

        let mut tasks = Vec::new();
        for t in store.tasks(Some(crate::wire::TaskStatus::Incomplete), Some(false), None) {
            if t.in_someday() {
                continue;
            }
            let Some(start_date) = t.start_date else {
                continue;
            };
            if start_date.timestamp() > now_ts {
                tasks.push(t);
            }
        }
        tasks.sort_by_key(|t| t.start_date);

        if tasks.is_empty() {
            writeln!(
                out,
                "{}",
                colored("No upcoming tasks.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        writeln!(
            out,
            "{}",
            colored(
                &format!("{} Upcoming  ({} tasks)", ICONS.upcoming, tasks.len()),
                &[BOLD, CYAN],
                cli.no_color,
            )
        )?;

        let id_prefix_len =
            store.unique_prefix_length(&tasks.iter().map(|t| t.uuid.clone()).collect::<Vec<_>>());

        let mut current_date = String::new();
        for task in tasks {
            let day = fmt_date(task.start_date);
            if day != current_date {
                writeln!(out)?;
                writeln!(
                    out,
                    "{}",
                    colored(&format!("  {}", day), &[BOLD], cli.no_color)
                )?;
                current_date = day;
            }
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
