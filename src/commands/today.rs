use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::ui::render_to_string;
use crate::ui::views::today::TodayView;
use crate::wire::task::TaskStatus;
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

        let mut id_candidates = today_items
            .iter()
            .map(|task| task.uuid.clone())
            .collect::<Vec<_>>();
        for task in &today_items {
            if let Some(project_uuid) = store.effective_project_uuid(task) {
                id_candidates.push(project_uuid);
            }
            if let Some(area_uuid) = store.effective_area_uuid(task) {
                id_candidates.push(area_uuid);
            }
        }
        let id_prefix_len = store.unique_prefix_length(&id_candidates);

        let view = TodayView {
            store: &store,
            today: &today,
            items: today_items,
            id_prefix_len,
            detailed: self.detailed.detailed,
        };

        let height = view.height();
        let rendered = render_to_string(view, 4096, height, cli.no_color);
        writeln!(out, "{}", rendered)?;

        Ok(())
    }
}
