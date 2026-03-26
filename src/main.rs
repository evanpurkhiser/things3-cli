fn main() {
    if let Err(err) = things_cli::app::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
