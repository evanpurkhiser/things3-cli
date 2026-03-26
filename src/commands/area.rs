use crate::app::Cli;
use crate::commands::Command;
use crate::common::{
    colored, fmt_project_with_note, fmt_task_line, fmt_task_with_note, BOLD, DIM, ICONS, MAGENTA,
};
use crate::wire::TaskStatus;
use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct AreaArgs {
    pub area_id: String,
    #[arg(long)]
    pub detailed: bool,
    #[arg(long)]
    pub all: bool,
}

impl Command for AreaArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let today = ctx.today();
        let (area_opt, err, ambiguous) = store.resolve_area_identifier(&self.area_id);
        let Some(area) = area_opt else {
            eprintln!("{err}");
            for match_area in ambiguous {
                eprintln!(
                    "  {} {}  ({})",
                    ICONS.area, match_area.title, match_area.uuid
                );
            }
            return Ok(());
        };

        let status_filter = if self.all {
            None
        } else {
            Some(TaskStatus::Incomplete)
        };
        let mut projects = store
            .projects(status_filter)
            .into_iter()
            .filter(|p| p.area.as_deref() == Some(area.uuid.as_str()))
            .collect::<Vec<_>>();
        projects.sort_by_key(|p| p.index);

        let mut loose_tasks = store
            .tasks(status_filter, Some(false), None)
            .into_iter()
            .filter(|t| {
                t.area.as_deref() == Some(area.uuid.as_str())
                    && !t.is_project()
                    && store.effective_project_uuid(t).is_none()
            })
            .collect::<Vec<_>>();
        loose_tasks.sort_by_key(|t| t.index);

        let project_count = projects.len();
        let task_count = loose_tasks.len();

        let tags = if area.tags.is_empty() {
            String::new()
        } else {
            let names = area
                .tags
                .iter()
                .map(|t| store.resolve_tag_title(t))
                .collect::<Vec<_>>()
                .join(", ");
            colored(&format!(" [{names}]"), &[DIM], cli.no_color)
        };

        let mut parts = Vec::new();
        if project_count > 0 {
            parts.push(format!(
                "{} project{}",
                project_count,
                if project_count == 1 { "" } else { "s" }
            ));
        }
        if task_count > 0 {
            parts.push(format!(
                "{} task{}",
                task_count,
                if task_count == 1 { "" } else { "s" }
            ));
        }
        let count_str = if parts.is_empty() {
            String::new()
        } else {
            format!("  ({})", parts.join(", "))
        };

        writeln!(
            out,
            "{}{}",
            colored(
                &format!("{} {}{}", ICONS.area, area.title, count_str),
                &[BOLD, MAGENTA],
                cli.no_color,
            ),
            tags
        )?;

        let mut all_uuids = vec![area.uuid.clone()];
        all_uuids.extend(projects.iter().map(|p| p.uuid.clone()));
        all_uuids.extend(loose_tasks.iter().map(|t| t.uuid.clone()));
        let id_prefix_len = store.unique_prefix_length(&all_uuids);

        if !loose_tasks.is_empty() {
            writeln!(out)?;
            for task in loose_tasks {
                let line = fmt_task_line(
                    &task,
                    &store,
                    false,
                    true,
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
                        "  ",
                        Some(id_prefix_len),
                        self.detailed,
                        cli.no_color,
                    )
                )?;
            }
        }

        if !projects.is_empty() {
            writeln!(out)?;
            for project in projects {
                writeln!(
                    out,
                    "{}",
                    fmt_project_with_note(
                        &project,
                        &store,
                        "  ",
                        Some(id_prefix_len),
                        true,
                        false,
                        self.detailed,
                        &today,
                        cli.no_color,
                    )
                )?;
            }
        }

        Ok(())
    }
}
