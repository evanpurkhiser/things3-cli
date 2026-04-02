use std::{io::Write, sync::Arc};

use anyhow::Result;
use clap::Args;
use iocraft::prelude::*;

use crate::{
    app::Cli,
    commands::{Command, DetailedArgs},
    common::parse_day,
    ui::{render_element_to_string, views::logbook::LogbookView},
};

#[derive(Debug, Default, Args)]
#[command(about = "Show the Logbook")]
pub struct LogbookArgs {
    #[command(flatten)]
    pub detailed: DetailedArgs,
    #[arg(
        long = "from",
        short = 'f',
        help = "Show items completed on/after this date (YYYY-MM-DD)"
    )]
    pub from_date: Option<String>,
    #[arg(
        long = "to",
        short = 't',
        help = "Show items completed on/before this date (YYYY-MM-DD)"
    )]
    pub to_date: Option<String>,
}

impl Command for LogbookArgs {
    fn run_with_ctx(
        &self,
        cli: &Cli,
        out: &mut dyn Write,
        ctx: &mut dyn crate::cmd_ctx::CmdCtx,
    ) -> Result<()> {
        let store = Arc::new(cli.load_store()?);
        let today = ctx.today();

        let from_day =
            parse_day(self.from_date.as_deref(), "--from").map_err(anyhow::Error::msg)?;
        let to_day = parse_day(self.to_date.as_deref(), "--to").map_err(anyhow::Error::msg)?;

        if let (Some(from), Some(to)) = (from_day, to_day)
            && from > to
        {
            eprintln!("--from date must be before or equal to --to date");
            return Ok(());
        }

        let tasks = store.logbook(from_day, to_day);
        let mut ui = element! {
            ContextProvider(value: Context::owned(store.clone())) {
                ContextProvider(value: Context::owned(today)) {
                    LogbookView(
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
