use std::path::PathBuf;

use crate::paths;

/// Searched after PATH (`runners::on_path`, `search_path`): an installed program wins over a fetched one.
pub fn dir() -> PathBuf {
    paths::data_home().join("bin")
}

pub fn find(bin: &str) -> Option<(String, String)> {
    let catalogue = crate::components::cached(&crate::config::Config::load().unwrap_or_default());
    let (_, entry) = catalogue.tool(bin)?;
    entry.latest().map(|b| (entry.name.clone(), b.version.clone()))
}

pub fn search_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push(dir());
    std::env::join_paths(dirs).unwrap_or_default()
}

/// `Ok` for a missing program that is no tool Universe fetches: its caller reports it.
pub async fn ensure(bin: &str) -> crate::Result<()> {
    if crate::runners::on_path(bin).is_some() {
        return Ok(());
    }
    static FETCHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = FETCHING.lock().await;
    if crate::runners::on_path(bin).is_some() {
        return Ok(());
    }
    let config = crate::config::Config::load().unwrap_or_default();
    let Some(_) = crate::components::cached(&config).tool(bin) else { return Ok(()) };
    let loaded = crate::components::load(&config, false).await;
    let never = std::sync::atomic::AtomicBool::new(false);
    let mut failures = Vec::new();
    for catalogue in [loaded.catalogue, crate::components::builtin()] {
        let Some((id, entry)) = catalogue.tool(bin) else { continue };
        let Some(build) = entry.latest() else { continue };
        tracing::info!("{bin} not installed: fetching {} {}", entry.name, build.version);
        match crate::components::install(id, entry, build, true, None, &never).await {
            Ok(_) => return Ok(()),
            Err(e) => failures.push(format!("{} {}: {e}", entry.name, build.version)),
        }
    }
    Err(crate::Error::Unavailable(format!("{bin} is not installed and could not be fetched: {}", failures.join("; "))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "live: downloads every pinned tool from GitHub"]
    fn live_the_pinned_tools_download_and_match() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("UNIVERSE_DATA_HOME", home.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let never = std::sync::atomic::AtomicBool::new(false);
        for (id, entry) in crate::components::builtin().components.iter().filter(|(_, e)| !e.bin.is_empty()) {
            let Some(build) = entry.latest() else { continue };
            let installed = rt.block_on(crate::components::install(id, entry, build, false, None, &never)).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(std::fs::metadata(installed.program_path()).is_ok_and(|m| m.len() > 0), "{id}");
            assert!(dir().join(&entry.bin).is_file(), "{id}: linked into {}", dir().display());
        }
        std::env::remove_var("UNIVERSE_DATA_HOME");
    }

    #[test]
    fn fetched_tools_are_searched_after_path() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        assert!(["umu-run", "gogdl", "legendary", "butler"].into_iter().all(|bin| find(bin).is_some()) && find("wine").is_none());
        let path = search_path();
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(dirs.last(), Some(&dir()), "an installed tool wins over a fetched one");
    }
}
