use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::ui::render_element_to_string;
use crate::ui::views::upcoming::UpcomingView;
use crate::wire::task::TaskStatus;
use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;
use std::io::Write;
use std::sync::Arc;

#[derive(Debug, Default, Args)]
pub struct UpcomingArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for UpcomingArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
        let today = ctx.today();
        let now_ts = today.timestamp();

        let mut tasks = Vec::new();
        for t in store.tasks(Some(TaskStatus::Incomplete), Some(false), None) {
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

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    UpcomingView(
                        items: &tasks,
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
