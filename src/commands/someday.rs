use crate::app::Cli;
use crate::commands::{Command, DetailedArgs};
use crate::ui::render_element_to_string;
use crate::ui::views::someday::SomedayView;
use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;
use std::io::Write;
use std::sync::Arc;

#[derive(Debug, Default, Args)]
pub struct SomedayArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
}

impl Command for SomedayArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
        let today = ctx.today();
        let items = store.someday();

        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    SomedayView(
                        items: &items,
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
