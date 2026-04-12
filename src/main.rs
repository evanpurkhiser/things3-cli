fn main() {
    if let Err(err) = things3_cloud::app::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
