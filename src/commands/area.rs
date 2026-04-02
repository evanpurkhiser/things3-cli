use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;

use crate::{
    app::Cli,
    commands::Command,
    common::ICONS,
    ui::{render_element_to_string, views::area::AreaView},
    wire::task::TaskStatus,
};

#[derive(Debug, Args)]
#[command(about = "Show projects and tasks in an area")]
pub struct AreaArgs {
    /// Area UUID (or unique UUID prefix)
    pub area_id: String,
    /// Show notes beneath each task/project
    #[arg(long, short = 'd')]
    pub detailed: bool,
    /// Include completed and canceled items
    #[arg(long, short = 'a')]
    pub all: bool,
}

impl Command for AreaArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
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
            .filter(|p| p.area.as_ref() == Some(&area.uuid))
            .collect::<Vec<_>>();
        projects.sort_by_key(|p| p.index);

        let mut loose_tasks = store
            .tasks(status_filter, Some(false), None)
            .into_iter()
            .filter(|t| {
                t.area.as_ref() == Some(&area.uuid)
                    && !t.is_project()
                    && store.effective_project_uuid(t).is_none()
            })
            .collect::<Vec<_>>();
        loose_tasks.sort_by_key(|t| t.index);

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    AreaView(
                        area: &area,
                        tasks: loose_tasks.iter().collect::<Vec<_>>(),
                        projects: projects.iter().collect::<Vec<_>>(),
                        detailed: self.detailed,
                    )
                }
            }
        };
        let rendered = render_element_to_string(&mut ui, cli.no_color);
        writeln!(out, "{}", rendered)?;

        Ok(())
    }
}
