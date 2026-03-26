/// Integration parity tests: runs each fixture from tests/fixtures/rust/*.json
/// in-process against the Rust command implementation.
///
/// Strategy:
///   1. Write the fixture journal to a temp file.
///   2. Optionally set THINGS3_TODAY env var.
///   3. Parse CLI args via `Cli::try_parse_from`.
///   4. Call `command.run(&cli, &mut buf)` with a Vec<u8> buffer.
///   5. Compare captured output to `expected_output`.
///
/// A global mutex serialises all tests to avoid races on env-var mutations.
use clap::Parser;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Mutex;
use tempfile::NamedTempFile;
use things_cli::app::Cli;
use things_cli::commands::{Command, Commands};

// One global lock so that parallel test threads don't stomp on each other's
// env-var mutations.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct Fixture {
    test_name: String,
    cli_args: String,
    today_ts: Option<i64>,
    journal: Vec<Value>,
    expected_output: String,
}

/// Parse the fixture's `cli_args` string into a `Cli` using the same flag set
/// that the binary uses.  We prefix with a fake argv[0].
fn parse_cli(cli_args: &str, journal_path: &str) -> Cli {
    // Build the full argv:  ["things3", "--no-color", "--load-journal", "<path>", ...subcommand args...]
    let mut argv: Vec<String> = vec!["things3".to_string()];
    argv.push("--no-color".to_string());
    argv.push("--load-journal".to_string());
    argv.push(journal_path.to_string());
    for token in cli_args.split_whitespace() {
        let t = token.trim_matches('\'').trim_matches('"').to_string();
        argv.push(t);
    }

    Cli::try_parse_from(argv).unwrap_or_else(|e| panic!("Failed to parse args '{cli_args}': {e}"))
}

/// Run a single fixture and return the captured output.
fn run_fixture(fixture: &Fixture) -> String {
    // Write the journal array to a temp file.
    let mut tmp = NamedTempFile::new().expect("create temp file");
    serde_json::to_writer(&mut tmp, &fixture.journal).expect("write journal");
    let path = tmp.path().to_str().unwrap().to_string();

    // Build CLI.
    let cli = parse_cli(&fixture.cli_args, &path);

    // Set or clear THINGS3_TODAY.
    unsafe {
        if let Some(ts) = fixture.today_ts {
            std::env::set_var("THINGS3_TODAY", ts.to_string());
        } else {
            std::env::remove_var("THINGS3_TODAY");
        }
    }

    // Capture output into a Vec<u8> buffer.
    let mut buf: Vec<u8> = Vec::new();

    // Run the command (default to Today if no subcommand was given).
    let default_cmd = Commands::Today(Default::default());
    let command = cli.command.as_ref().unwrap_or(&default_cmd);
    let result = command.run(&cli, &mut buf);

    if let Err(e) = result {
        panic!("Command failed for {}: {e}", fixture.test_name);
    }

    String::from_utf8(buf).expect("output is valid UTF-8")
}

/// Load all .json fixture files from tests/fixtures/rust/ relative to the
/// manifest directory.
fn load_all_fixtures() -> Vec<Fixture> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust");
    let mut fixtures = Vec::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("Cannot read fixture dir {dir:?}: {e}"))
    {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Cannot read {path:?}: {e}"));
            let fixture: Fixture =
                serde_json::from_str(&raw).unwrap_or_else(|e| panic!("Cannot parse {path:?}: {e}"));
            fixtures.push(fixture);
        }
    }
    fixtures
}

#[test]
fn parity_all_fixtures() {
    // Collect and sort for deterministic ordering.
    let mut fixtures = load_all_fixtures();
    fixtures.sort_by(|a, b| a.test_name.cmp(&b.test_name));

    let _guard = LOCK.lock().unwrap();

    let mut failures: Vec<String> = Vec::new();

    for fixture in &fixtures {
        let got = run_fixture(fixture);
        if got != fixture.expected_output {
            failures.push(format!(
                "\n--- FAIL: {} ---\n  cli_args: {}\n  expected:\n{}\n  got:\n{}\n",
                fixture.test_name,
                fixture.cli_args,
                indent(&fixture.expected_output),
                indent(&got),
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{}/{} fixtures FAILED:\n{}",
            failures.len(),
            fixtures.len(),
            failures.join("\n")
        );
    }

    println!("{} fixtures all passed.", fixtures.len());
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
