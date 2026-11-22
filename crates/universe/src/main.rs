use clap::Parser;

#[tokio::main]
async fn main() {
    universe::init_tracing();
    let cli = universe::cli::Cli::parse();
    let json = universe::cli::prints_failures(&cli);
    if let Err(e) = universe::cli::run(cli).await {
        std::process::exit(universe::cli::failed(json, &e));
    }
}
