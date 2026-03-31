use crate::commands::{Command, Commands};
use crate::dirs::append_log_dir;
use crate::log_cache::{fold_state_from_append_log, get_state_with_append_log};
use crate::logging;
use crate::store::{RawState, ThingsStore, fold_items};
use crate::wire::wire_object::WireItem;
use crate::{auth::load_auth, client::ThingsCloudClient};
use anyhow::{Context, Result};
use clap::Parser;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "things3")]
#[command(bin_name = "things3")]
#[command(version)]
#[command(before_help = concat!("things3 ", env!("CARGO_PKG_VERSION")))]
#[command(disable_help_subcommand = true)]
#[command(about = concat!(
    "things3 v",
    env!("CARGO_PKG_VERSION"),
    ": Command-line interface for Things 3 via Cloud API"
))]
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
    #[arg(long, value_enum, default_value_t = logging::Level::Info)]
    pub log_level: logging::Level,
    /// Set the logging output format
    #[arg(long, value_enum, default_value_t = logging::LogFormat::Auto)]
    pub log_format: logging::LogFormat,
    /// For testing: advanced tracing filter directive
    #[arg(long, global = true, hide = true, value_name = "DIRECTIVE")]
    pub log_filter: Option<String>,
    /// For testing: override "today" UTC midnight timestamp
    #[arg(long, global = true, hide = true, value_name = "TIMESTAMP")]
    pub today_ts: Option<i64>,
    /// For testing: override current UNIX timestamp
    #[arg(long, global = true, hide = true, value_name = "TIMESTAMP")]
    pub now_ts: Option<f64>,
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
            let raw = if journal_path == std::path::Path::new("-") {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .with_context(|| "failed to read journal JSON from stdin")?;
                buf
            } else {
                std::fs::read_to_string(journal_path).with_context(|| {
                    format!("failed to read journal file {}", journal_path.display())
                })?
            };
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
    logging::init(cli.log_level, cli.log_format, cli.log_filter.as_deref());
    let command = cli
        .command
        .take()
        .unwrap_or(Commands::Today(Default::default()));
    command.run(&cli, &mut std::io::stdout())
}
