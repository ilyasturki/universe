use clap::Parser;

#[tokio::main]
async fn main() {
    // Logs never share stdout with `--json` output or a piped listing.
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).with_target(false).init();
    let cli = universe::cli::Cli::parse();
    if let Err(e) = universe::cli::run(cli).await {
        eprintln!("universe: {e:#}");
        std::process::exit(1);
    }
}
