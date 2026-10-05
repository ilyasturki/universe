// Runs as root through pkexec (the polkit action io.github.ilyasturki.universe.system-install).
use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tool = match universe::system_install::requested(&args) {
        Ok(tool) => tool,
        Err(e) => {
            eprintln!("universe-system-install: {e}");
            return ExitCode::from(2);
        }
    };
    let mut report = |percent: u64, _: u64, _: &str| println!("{percent}");
    match universe::packagekit::install(tool.packages(universe::distro::detect()), tool.name, Some(&mut report)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("universe-system-install: {e}");
            ExitCode::FAILURE
        }
    }
}
