use crate::app::Cli;
use crate::auth::write_auth;
use crate::commands::Command;
use anyhow::Result;
use clap::Args;
use std::io::{self, Write};

#[derive(Debug, Default, Args)]
pub struct SetAuthArgs {}

impl Command for SetAuthArgs {
    fn run(&self, _cli: &Cli, out: &mut dyn std::io::Write) -> Result<()> {
        print!("Things Cloud email: ");
        io::stdout().flush()?;
        let mut email = String::new();
        io::stdin().read_line(&mut email)?;

        print!("Things Cloud password: ");
        io::stdout().flush()?;
        let mut password = String::new();
        io::stdin().read_line(&mut password)?;

        let path = write_auth(email.trim(), password.trim_end())?;
        writeln!(out, "Saved auth to {}", path.display())?;
        Ok(())
    }
}
