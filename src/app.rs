use crate::commands::{Command, Commands};
use crate::dirs::append_log_dir;
use crate::log_cache::{fold_state_from_append_log, get_state_with_append_log};
use crate::logging;
use crate::store::{fold_items, RawState, ThingsStore};
use crate::wire::wire_object::WireItem;
use crate::{auth::load_auth, client::ThingsCloudClient};
use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "things3")]
#[command(bin_name = "things3")]
#[command(disable_help_subcommand = true)]
#[command(about = "things3: Command-line interface for Things 3 via Cloud API")]
pub struct Cli {
    /// Disable color output
    #[arg(long)]
    pub no_color: bool,
    /// Skip cloud sync and use local cache only
    #[arg(long)]
    pub no_sync: bool,
    /// For testing: disable cloud sync and cloud writes
    #[arg(long, hide = true)]
    pub no_cloud: bool,
    /// Set the log level filter
    #[arg(long, global = true, value_enum, default_value_t = logging::Level::Info)]
    pub log_level: logging::Level,
    /// Set the logging output format
    #[arg(long, global = true, value_enum, default_value_t = logging::LogFormat::Auto)]
    pub log_format: logging::LogFormat,
    /// For testing: load state from a JSON journal file instead of syncing.
    /// The file must contain a JSON array of WireItem objects (each is a
    /// map of uuid -> WireObject).
    #[arg(long, value_name = "FILE", hide = true)]
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

        if self.no_sync || self.no_cloud {
            let cache_dir = append_log_dir();
            return fold_state_from_append_log(&cache_dir);
        }

        let (email, password) = load_auth()?;
        let mut client = ThingsCloudClient::new(email, password)?;
        let cache_dir = append_log_dir();
        get_state_with_append_log(&mut client, cache_dir)
    }

    pub fn load_store(&self) -> Result<ThingsStore> {
        let state = self.load_state()?;
        Ok(ThingsStore::from_raw_state(&state))
    }
}

pub fn run() -> Result<()> {
    let mut cli = Cli::parse();
    logging::init(cli.log_level, cli.log_format);
    let command = cli
        .command
        .take()
        .unwrap_or(Commands::Today(Default::default()));
    command.run(&cli, &mut std::io::stdout())
}
