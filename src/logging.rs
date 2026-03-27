use clap::ValueEnum;
use std::sync::OnceLock;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{Layer, prelude::*};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, ValueEnum, Default)]
#[clap(rename_all = "lower")]
pub enum LogFormat {
    #[default]
    Auto,
    Pretty,
    Simplified,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, ValueEnum, Default)]
#[clap(rename_all = "lower")]
pub enum Level {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
    Off,
}

impl Level {
    pub const fn level_filter(self) -> LevelFilter {
        match self {
            Self::Error => LevelFilter::ERROR,
            Self::Warn => LevelFilter::WARN,
            Self::Info => LevelFilter::INFO,
            Self::Debug => LevelFilter::DEBUG,
            Self::Trace => LevelFilter::TRACE,
            Self::Off => LevelFilter::OFF,
        }
    }
}

pub fn init(log_level: Level, log_format: LogFormat) {
    static INIT: OnceLock<()> = OnceLock::new();
    let _ = INIT.get_or_init(|| {
        let subscriber = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true);

        let format = match (log_format, console::user_attended()) {
            (LogFormat::Auto, true) | (LogFormat::Pretty, _) => {
                subscriber.compact().without_time().boxed()
            }
            (LogFormat::Auto, false) | (LogFormat::Simplified, _) => {
                subscriber.with_ansi(false).boxed()
            }
            (LogFormat::Json, _) => subscriber
                .json()
                .flatten_event(true)
                .with_current_span(true)
                .with_span_list(true)
                .with_file(true)
                .with_line_number(true)
                .boxed(),
        };

        tracing_subscriber::registry()
            .with(format.with_filter(log_level.level_filter()))
            .init();
    });
}
