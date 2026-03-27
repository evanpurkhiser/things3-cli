fn main() {
    if let Err(err) = things3_cloud::app::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::sync::OnceLock;

    fn run_trycmd_cases(case_glob: &str) {
        let things3_bin = things3_bin_path();

        trycmd::TestCases::new()
            .env("TRYCMD_BIN_THINGS3", things3_bin.display().to_string())
            .register_bin("things3", &things3_bin)
            .register_bin(
                "run.sh",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("trycmd")
                    .join("run.sh"),
            )
            .case(case_glob);
    }

    fn things3_bin_path() -> std::path::PathBuf {
        for env_var in [
            "CARGO_BIN_EXE_things3",
            "CARGO_BIN_EXE_things3-cloud",
            "CARGO_BIN_EXE_things3-cli",
            "CARGO_BIN_EXE_things-cli",
        ] {
            if let Some(path) = std::env::var_os(env_var) {
                return std::path::PathBuf::from(path);
            }
        }

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let bin_path = manifest_dir
            .join("target")
            .join("debug")
            .join(format!("things3{}", std::env::consts::EXE_SUFFIX));

        if !bin_path.exists() {
            static BUILD_ONCE: OnceLock<()> = OnceLock::new();
            BUILD_ONCE.get_or_init(|| {
                let status = Command::new("cargo")
                    .arg("build")
                    .arg("--bin")
                    .arg("things3")
                    .current_dir(manifest_dir)
                    .status()
                    .expect("failed to build things3 test binary");
                assert!(status.success(), "failed to build things3 test binary");
            });
        }

        bin_path
    }

    #[test]
    fn cli_trycmd_anytime() {
        run_trycmd_cases("trycmd/anytime/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_area() {
        run_trycmd_cases("trycmd/area/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_areas() {
        run_trycmd_cases("trycmd/areas/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_delete() {
        run_trycmd_cases("trycmd/delete/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_find() {
        run_trycmd_cases("trycmd/find/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_inbox() {
        run_trycmd_cases("trycmd/inbox/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_logbook() {
        run_trycmd_cases("trycmd/logbook/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_mark() {
        run_trycmd_cases("trycmd/mark/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_new() {
        run_trycmd_cases("trycmd/new/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_project() {
        run_trycmd_cases("trycmd/project/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_projects() {
        run_trycmd_cases("trycmd/projects/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_reorder() {
        run_trycmd_cases("trycmd/reorder/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_schedule() {
        run_trycmd_cases("trycmd/schedule/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_someday() {
        run_trycmd_cases("trycmd/someday/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_tags() {
        run_trycmd_cases("trycmd/tags/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_today() {
        run_trycmd_cases("trycmd/today/**/*.trycmd");
    }

    #[test]
    fn cli_trycmd_upcoming() {
        run_trycmd_cases("trycmd/upcoming/**/*.trycmd");
    }
}
