use clap::Parser;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).with_target(false).init();
    let cli = universe::cli::Cli::parse();
    if let Err(e) = universe::cli::run(cli).await {
        let msg = e.to_string();
        let msg = msg.strip_prefix("io.github.ilyasturki.Universe.Error.").map(|s| s.to_string()).unwrap_or(msg);
        eprintln!("universe: {msg}");
        std::process::exit(1);
    }
}
