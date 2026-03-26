use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::common::{
    colored, fmt_project_with_note, fmt_task_line, fmt_task_with_note, BLUE, BOLD, DIM, ICONS,
    YELLOW,
};
use crate::wire::TaskStatus;
use anyhow::Result;
use clap::Args;
use std::io::Write;

#[derive(Debug, Default, Args)]
pub struct TodayArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for TodayArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let today = ctx.today();
        let tasks = store.today(&today);
        let mut today_items: Vec<_> = store
            .tasks(Some(TaskStatus::Incomplete), Some(false), None)
            .into_iter()
            .filter(|t| {
                !t.is_heading()
                    && !t.title.trim().is_empty()
                    && t.entity == "Task6"
                    && (t.is_today(&today) || t.evening)
            })
            .collect();

        today_items.sort_by_key(|task| {
            let tir = task.today_index_reference.unwrap_or(0);
            (
                std::cmp::Reverse(tir),
                task.today_index,
                std::cmp::Reverse(task.index),
            )
        });

        if today_items.is_empty() {
            writeln!(
                out,
                "{}",
                colored("No tasks for today.", &[DIM], cli.no_color)
            )?;
            return Ok(());
        }

        let regular: Vec<_> = today_items.iter().filter(|t| !t.evening).cloned().collect();
        let evening: Vec<_> = today_items.iter().filter(|t| t.evening).cloned().collect();
        let project_count = today_items.iter().filter(|t| t.is_project()).count();
        let id_prefix_len = store.unique_prefix_length(
            &today_items
                .iter()
                .map(|i| i.uuid.clone())
                .collect::<Vec<_>>(),
        );

        if project_count > 0 {
            let label = if project_count == 1 {
                "project"
            } else {
                "projects"
            };
            writeln!(
                out,
                "{}",
                colored(
                    &format!(
                        "{} Today  ({} tasks, {} {})",
                        ICONS.today,
                        tasks.len(),
                        project_count,
                        label
                    ),
                    &[BOLD, YELLOW],
                    cli.no_color,
                )
            )?;
        } else {
            writeln!(
                out,
                "{}",
                colored(
                    &format!("{} Today  ({} tasks)", ICONS.today, tasks.len()),
                    &[BOLD, YELLOW],
                    cli.no_color,
                )
            )?;
        }

        if !regular.is_empty() {
            writeln!(out)?;
            for item in regular {
                if item.is_project() {
                    writeln!(
                        out,
                        "{}",
                        fmt_project_with_note(
                            &item,
                            &store,
                            "  ",
                            Some(id_prefix_len),
                            false,
                            true,
                            self.detailed.detailed,
                            &today,
                            cli.no_color,
                        )
                    )?;
                } else {
                    let line = fmt_task_line(
                        &item,
                        &store,
                        false,
                        false,
                        true,
                        Some(id_prefix_len),
                        &today,
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
            }
        }

        if !evening.is_empty() {
            writeln!(out)?;
            writeln!(
                out,
                "{}",
                colored(
                    &format!("{} This Evening", ICONS.evening),
                    &[BOLD, BLUE],
                    cli.no_color,
                )
            )?;
            writeln!(out)?;

            for item in evening {
                if item.is_project() {
                    writeln!(
                        out,
                        "{}",
                        fmt_project_with_note(
                            &item,
                            &store,
                            "  ",
                            Some(id_prefix_len),
                            false,
                            true,
                            self.detailed.detailed,
                            &today,
                            cli.no_color,
                        )
                    )?;
                } else {
                    let line = fmt_task_line(
                        &item,
                        &store,
                        false,
                        false,
                        true,
                        Some(id_prefix_len),
                        &today,
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
            }
        }

        Ok(())
    }
}
