use std::sync::Arc;

use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;

use crate::{
    app::Cli,
    commands::{Command, DetailedArgs, detailed_json_conflict, write_json},
    ui::{
        render_element_to_string,
        views::{inbox::InboxView, json::common::build_tasks_json},
    },
};

#[derive(Debug, Default, Args)]
pub struct InboxArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for InboxArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn std::io::Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
        let today = ctx.today();
        let tasks = store.inbox();

        let json = cli.json;
        if json {
            if detailed_json_conflict(json, self.detailed.detailed) {
                return Ok(());
            }
            write_json(out, &build_tasks_json(&tasks, &store, &today))?;
            return Ok(());
        }

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    InboxView(
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
