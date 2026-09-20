use clap::Parser;

#[tokio::main]
async fn main() {
    universe::init_tracing();
    let cli = universe::cli::Cli::parse();
    if let Err(e) = universe::cli::run(cli).await {
        eprintln!("universe: {e:#}");
        std::process::exit(1);
    }
}
