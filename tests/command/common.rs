use clap::Parser;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;
use tempfile::NamedTempFile;
use things_cli::app::Cli;
use things_cli::commands::{Command, Commands};

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct Fixture {
    test_name: String,
    cli_args: String,
    today_ts: Option<i64>,
    journal: Vec<Value>,
    expected_output: String,
}

fn parse_cli(cli_args: &str, journal_path: &str) -> Cli {
    let mut argv: Vec<String> = vec!["things3".to_owned()];
    argv.push("--no-color".to_owned());
    argv.push("--load-journal".to_owned());
    argv.push(journal_path.to_owned());
    for token in cli_args.split_whitespace() {
        let t = token.trim_matches('\'').trim_matches('"').to_owned();
        argv.push(t);
    }

    Cli::try_parse_from(argv).unwrap_or_else(|e| panic!("Failed to parse args '{cli_args}': {e}"))
}

fn run_fixture(fixture: &Fixture) -> String {
    let mut tmp = NamedTempFile::new().expect("create temp file");
    serde_json::to_writer(&mut tmp, &fixture.journal).expect("write journal");
    let path = tmp.path().to_str().expect("valid path").to_owned();

    let cli = parse_cli(&fixture.cli_args, &path);

    // SAFETY: tests are serialized with LOCK to avoid env var races.
    unsafe {
        if let Some(ts) = fixture.today_ts {
            std::env::set_var("THINGS3_TODAY", ts.to_string());
        } else {
            std::env::remove_var("THINGS3_TODAY");
        }
    }

    let mut buf: Vec<u8> = Vec::new();
    let default_cmd = Commands::Today(Default::default());
    let command = cli.command.as_ref().unwrap_or(&default_cmd);
    let result = command.run(&cli, &mut buf);

    if let Err(e) = result {
        panic!("Command failed for {}: {e}", fixture.test_name);
    }

    String::from_utf8(buf).expect("output is valid UTF-8")
}

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust")
}

fn load_fixture(name: &str) -> Fixture {
    let path = fixture_dir().join(format!("{name}.json"));
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Cannot read {path:?}: {e}"));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("Cannot parse {path:?}: {e}"))
}

pub fn run_named_fixture(name: &str) {
    let fixture = load_fixture(name);
    let _guard = LOCK.lock().expect("test lock");
    let got = run_fixture(&fixture);
    assert_eq!(
        got, fixture.expected_output,
        "fixture failed: {}\ncli_args: {}\n",
        fixture.test_name, fixture.cli_args
    );
}

macro_rules! fixture_test {
    ($fixture_name:ident) => {
        #[test]
        #[allow(non_snake_case)]
        fn $fixture_name() {
            crate::command::common::run_named_fixture(stringify!($fixture_name));
        }
    };
}

pub(crate) use fixture_test;
