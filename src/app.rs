use crate::commands::{Command, Commands};
use crate::dirs::append_log_dir;
use crate::log_cache::{fold_state_from_append_log, get_state_with_append_log};
use crate::store::{RawState, ThingsStore, fold_items};
use crate::wire::WireItem;
use crate::{auth::load_auth, client::ThingsCloudClient};
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "things3")]
#[command(about = "things3: Command-line interface for Things 3 via Cloud API")]
pub struct Cli {
    #[arg(long)]
    pub no_color: bool,
    #[arg(long)]
    pub no_sync: bool,
    /// For testing: load state from a JSON journal file instead of syncing.
    /// The file must contain a JSON array of WireItem objects (each is a
    /// map of uuid -> WireObject).
    #[arg(long, value_name = "FILE")]
    pub load_journal: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn load_state(&self) -> Result<RawState> {
        if let Some(journal_path) = &self.load_journal {
            let raw = std::fs::read_to_string(journal_path).with_context(|| {
                format!("failed to read journal file {}", journal_path.display())
            })?;
            let items: Vec<WireItem> =
                serde_json::from_str(&raw).with_context(|| "failed to parse journal JSON")?;
            return Ok(fold_items(items));
        }

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let cache_dir = append_log_dir();

        if self.no_sync {
            fold_state_from_append_log(&cache_dir)
        } else {
            get_state_with_append_log(&mut client, cache_dir)
        }
    }

    pub fn load_store(&self) -> Result<ThingsStore> {
        let state = self.load_state()?;
        Ok(ThingsStore::from_raw_state(&state))
    }
}

pub fn run() -> Result<()> {
    let mut cli = Cli::parse();
    let command = cli
        .command
        .take()
        .unwrap_or(Commands::Today(Default::default()));
    command.run(&cli, &mut std::io::stdout())
}
