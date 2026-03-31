use crate::app::Cli;
use crate::commands::Command;
use crate::ui::render_element_to_string;
use crate::ui::views::project::{ProjectHeadingGroup, ProjectView};
use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Debug, Args)]
#[command(about = "Show all tasks in a project")]
pub struct ProjectArgs {
    /// Project UUID (or unique UUID prefix)
    pub project_id: String,
    /// Show notes beneath each task
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
        let store = Arc::new(cli.load_store()?);
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
            .filter(|t| store.effective_project_uuid(t).as_ref() == Some(&project.uuid))
            .collect::<Vec<_>>();

        let headings = store
            .tasks_by_uuid
            .values()
            .filter(|t| t.is_heading() && !t.trashed && t.project.as_ref() == Some(&project.uuid))
            .cloned()
            .map(|h| (h.uuid.clone(), h))
            .collect::<BTreeMap<_, _>>();

        let mut ungrouped = Vec::new();
        let mut by_heading: BTreeMap<_, Vec<_>> = BTreeMap::new();
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

        let heading_groups = sorted_heading_uuids
            .iter()
            .filter_map(|heading_uuid| {
                let heading = headings.get(heading_uuid)?;
                let tasks = by_heading.get(heading_uuid)?;
                Some(ProjectHeadingGroup {
                    title: heading.title.clone(),
                    items: tasks.iter().collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    ProjectView(
                        project: &project,
                        ungrouped: ungrouped.iter().collect::<Vec<_>>(),
                        heading_groups,
                        detailed: self.detailed,
                        no_color: cli.no_color,
                    )
                }
            }
        };
        let rendered = render_element_to_string(&mut ui, cli.no_color);
        writeln!(out, "{}", rendered)?;

        Ok(())
    }
}
