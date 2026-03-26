use crate::app::Cli;
use crate::commands::Command;
use crate::common::{
    BOLD, DIM, GREEN, ICONS, colored, fmt_deadline, fmt_task_line, fmt_task_with_note,
};
use anyhow::Result;
use clap::Args;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct ProjectArgs {
    pub project_id: String,
    #[arg(long)]
    pub detailed: bool,
}

impl Command for ProjectArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = cli.load_store()?;
        let today = ctx.today();
        let (task_opt, err, ambiguous) = store.resolve_mark_identifier(&self.project_id);
        let Some(project) = task_opt else {
            eprintln!("{err}");
            for match_task in ambiguous {
                eprintln!("  {}", match_task.title);
            }
            return Ok(());
        };

        if !project.is_project() {
            eprintln!("Not a project: {}", project.title);
            return Ok(());
        }

        let children = store
            .tasks(None, Some(false), None)
            .into_iter()
            .filter(|t| store.effective_project_uuid(t).as_deref() == Some(project.uuid.as_str()))
            .collect::<Vec<_>>();

        let headings = store
            .tasks_by_uuid
            .values()
            .filter(|t| {
                t.is_heading() && !t.trashed && t.project.as_deref() == Some(project.uuid.as_str())
            })
            .cloned()
            .map(|h| (h.uuid.clone(), h))
            .collect::<BTreeMap<_, _>>();

        let mut ungrouped = Vec::new();
        let mut by_heading: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for t in children.clone() {
            if let Some(heading_uuid) = &t.action_group
                && headings.contains_key(heading_uuid)
            {
                by_heading.entry(heading_uuid.clone()).or_default().push(t);
                continue;
            }
            ungrouped.push(t);
        }

        let mut sorted_heading_uuids = by_heading.keys().cloned().collect::<Vec<_>>();
        sorted_heading_uuids.sort_by_key(|u| headings.get(u).map(|h| h.index).unwrap_or(0));
        ungrouped.sort_by_key(|t| t.index);
        for items in by_heading.values_mut() {
            items.sort_by_key(|t| t.index);
        }

        let total = children.len();
        let progress = store.project_progress(&project.uuid);
        let done_count = progress.done;

        let tags = if project.tags.is_empty() {
            String::new()
        } else {
            let tag_names = project
                .tags
                .iter()
                .map(|t| store.resolve_tag_title(t))
                .collect::<Vec<_>>()
                .join(", ");
            colored(&format!(" [{tag_names}]"), &[DIM], cli.no_color)
        };
        writeln!(
            out,
            "{}{}{}",
            colored(
                &format!(
                    "{} {}  ({}/{})",
                    ICONS.project,
                    project.title,
                    done_count,
                    done_count + total as i32
                ),
                &[BOLD, GREEN],
                cli.no_color,
            ),
            fmt_deadline(project.deadline, &today, cli.no_color),
            tags
        )?;

        if let Some(notes) = &project.notes {
            let lines = notes.lines().collect::<Vec<_>>();
            for note in lines.iter().take(lines.len().saturating_sub(1)) {
                writeln!(
                    out,
                    "{} {}",
                    colored("  │", &[DIM], cli.no_color),
                    colored(note, &[DIM], cli.no_color)
                )?;
            }
            if let Some(last) = lines.last() {
                writeln!(
                    out,
                    "{} {}",
                    colored("  └", &[DIM], cli.no_color),
                    colored(last, &[DIM], cli.no_color)
                )?;
            }
        }

        let mut all_uuids = vec![project.uuid.clone()];
        all_uuids.extend(children.iter().map(|t| t.uuid.clone()));
        let id_prefix_len = store.unique_prefix_length(&all_uuids);

        if children.is_empty() {
            writeln!(out, "{}", colored("  No tasks.", &[DIM], cli.no_color))?;
            return Ok(());
        }

        if !ungrouped.is_empty() {
            writeln!(out)?;
            for t in ungrouped {
                let line =
                    fmt_task_line(
                        &t,
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
                        &t,
                        "  ",
                        Some(id_prefix_len),
                        self.detailed,
                        cli.no_color,
                    )
                )?;
            }
        }

        for heading_uuid in sorted_heading_uuids {
            if let Some(heading) = headings.get(&heading_uuid) {
                writeln!(out)?;
                writeln!(
                    out,
                    "{}",
                    colored(&format!("  {}", heading.title), &[BOLD], cli.no_color)
                )?;
                if let Some(tasks) = by_heading.get(&heading_uuid) {
                    for t in tasks {
                        let line = fmt_task_line(
                            t,
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
                                t,
                                "    ",
                                Some(id_prefix_len),
                                self.detailed,
                                cli.no_color,
                            )
                        )?;
                    }
                }
            }
        }

        Ok(())
    }
}
