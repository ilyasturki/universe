use std::path::PathBuf;

use crate::paths;

/// Searched after PATH (`runners::on_path`, `search_path`): an installed program wins over a fetched one.
pub fn dir() -> PathBuf {
    paths::data_home().join("bin")
}

/// `owner`: the source asking, empty for the core and the modules.
pub fn find(bin: &str, owner: &str) -> Option<(String, String)> {
    let catalogue = crate::components::cached(&crate::config::Config::load().unwrap_or_default());
    let (_, entry) = catalogue.tool(bin, owner)?;
    entry.latest().map(|b| (entry.name.clone(), b.version.clone()))
}

pub fn search_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push(dir());
    std::env::join_paths(dirs).unwrap_or_default()
}

/// `Ok` for a missing program that is no tool Universe fetches: its caller reports it.
pub async fn ensure(bin: &str, owner: &str) -> crate::Result<()> {
    if crate::runners::on_path(bin).is_some() {
        return Ok(());
    }
    static FETCHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = FETCHING.lock().await;
    if crate::runners::on_path(bin).is_some() {
        return Ok(());
    }
    let config = crate::config::Config::load().unwrap_or_default();
    let Some(_) = crate::components::cached(&config).tool(bin, owner) else { return Ok(()) };
    let loaded = crate::components::load(&config, false).await;
    let never = std::sync::atomic::AtomicBool::new(false);
    let mut failures = Vec::new();
    for catalogue in [loaded.catalogue, crate::components::pinned(&config)] {
        let Some((id, entry)) = catalogue.tool(bin, owner) else { continue };
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
        let _env = crate::paths::test_env();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let never = std::sync::atomic::AtomicBool::new(false);
        std::env::set_var("UNIVERSE_SOURCES_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../../sources"));
        let config: crate::config::Config = toml::from_str("[sources]\nenabled = [\"gog\", \"epic\", \"itch\"]").unwrap();
        for (id, entry) in crate::components::pinned(&config).components.iter().filter(|(_, e)| !e.bin.is_empty()) {
            let Some(build) = entry.latest() else { continue };
            let installed = rt.block_on(crate::components::install(id, entry, build, false, None, &never)).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(std::fs::metadata(installed.program_path()).is_ok_and(|m| m.len() > 0), "{id}");
            assert!(dir().join(&entry.bin).is_file(), "{id}: linked into {}", dir().display());
        }
    }

    fn pin(root: &std::path::Path, source: &str, id: &str, bin: &str, version: &str) {
        let dir = root.join(source);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("source.toml"),
            format!(
                r#"api = 2
id = "{source}"
exe = "bin/source"
[[tools]]
id = "{id}"
name = "{source}-cli"
kind = "tool"
bin = "{bin}"
builds = [{{ version = "{version}", assets = [{{ format = "binary", url = "https://example.org/{bin}", sha256 = "00" }}] }}]
"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_source_pins_its_own_tools_and_no_other_sources() {
        let env = crate::paths::test_env();
        let (shipped, installed) = (env.path().join("sources"), crate::paths::extensions_dir("source"));
        pin(&shipped, "store", "storecli", "storecli", "1.0.0");
        pin(&installed, "aaa", "aaa", "storecli", "6.6.6");
        pin(&installed, "late", "storecli", "storecli", "6.6.6");
        pin(&shipped, "off", "offcli", "offcli", "1.0.0");
        std::fs::write(crate::paths::config_file(), "[sources]\nenabled = [\"store\", \"aaa\", \"late\"]\n").unwrap();
        assert_eq!(find("storecli", "store"), Some(("store-cli".into(), "1.0.0".into())), "its own pin, not the one of a source sorted before it");
        assert_eq!(find("storecli", "aaa"), Some(("aaa-cli".into(), "6.6.6".into())));
        assert_eq!(find("storecli", ""), None, "a module or the core never runs a source's pin");
        let pinned = crate::components::pinned(&crate::config::Config::load().unwrap());
        assert_eq!(pinned.components["storecli"].name, "store-cli", "a shipped pin wins over an installed source's of the same id");
        assert!(!pinned.components.contains_key("offcli"), "a disabled source pins nothing");
        assert!(find("umu-run", "").is_some() && find("wine", "").is_none());
        let path = search_path();
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(dirs.last(), Some(&dir()), "an installed tool wins over a fetched one");
    }
}
