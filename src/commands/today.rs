use std::{io::Write, sync::Arc};

use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;

use crate::{
    app::Cli,
    commands::{Command, DetailedArgs, detailed_json_conflict, write_json},
    ui::{
        render_element_to_string,
        views::{json::today::TodayJsonView, today::TodayView},
    },
    wire::task::TaskStatus,
};

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
        let store = Arc::new(cli.load_store()?);
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

        let json = cli.json;
        if json {
            if detailed_json_conflict(json, self.detailed.detailed) {
                return Ok(());
            }
            write_json(out, &TodayJsonView::build(&today_items, &store, &today))?;
            return Ok(());
        }

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    TodayView(
                        items: &today_items,
                        detailed: self.detailed.detailed,
                    )
                }
            }
        };
        let rendered = render_element_to_string(&mut ui, cli.no_color);
        writeln!(out, "{}", rendered)?;

        Ok(())
    }
}
