use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::ui::render_element_to_string;
use crate::ui::views::inbox::InboxView;
use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;
use std::sync::Arc;

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
