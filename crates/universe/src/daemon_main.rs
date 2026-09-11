use std::sync::Arc;

use universe::config::Config;
use universe::core::Core;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).with_target(false).init();
    let config = Config::load()?;
    std::fs::create_dir_all(universe::paths::games_dir())?;
    let core = Core::new(config)?;
    let _conn = universe::dbus::serve(Arc::clone(&core)).await?;
    tracing::info!("universed {} on {} ({} games)", universe::VERSION, universe::BUS_NAME, core.games.read().await.len());
    if let Some(c) = std::fs::read_to_string(universe::paths::current_session_file()).ok().and_then(|s| serde_json::from_str::<universe::core::Current>(&s).ok()) {
        tracing::warn!("previous session {} of {} was not closed; the daemon restarted while it ran", c.session_id, c.id);
        let _ = std::fs::remove_file(universe::paths::current_session_file());
    }
    tokio::signal::ctrl_c().await?;
    Ok(())
}
