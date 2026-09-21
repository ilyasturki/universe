use std::path::{Path, PathBuf};

pub fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

fn universe_home(var: &str, xdg_var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| xdg(xdg_var, fallback).join("universe"))
}

pub fn data_home() -> PathBuf {
    universe_home("UNIVERSE_DATA_HOME", "XDG_DATA_HOME", ".local/share")
}

pub fn config_home() -> PathBuf {
    universe_home("UNIVERSE_CONFIG_HOME", "XDG_CONFIG_HOME", ".config")
}

pub fn state_home() -> PathBuf {
    universe_home("UNIVERSE_STATE_HOME", "XDG_STATE_HOME", ".local/state")
}

pub fn cache_home() -> PathBuf {
    universe_home("UNIVERSE_CACHE_HOME", "XDG_CACHE_HOME", ".cache")
}

/// xdg-user-dirs: `$XDG_<NAME>_DIR`, else the entry in `~/.config/user-dirs.dirs`, else `~/<fallback>`.
pub fn user_dir(name: &str, fallback: &str) -> PathBuf {
    let var = format!("XDG_{name}_DIR");
    if let Some(p) = std::env::var_os(&var).map(PathBuf::from).filter(|p| p.is_absolute()) {
        return p;
    }
    let file = xdg("XDG_CONFIG_HOME", ".config").join("user-dirs.dirs");
    let listed = std::fs::read_to_string(file).ok().and_then(|text| {
        text.lines().find_map(|l| {
            let (k, v) = l.trim().split_once('=')?;
            (k == var).then(|| v.trim_matches('"').replace("$HOME", &home().to_string_lossy()))
        })
    });
    listed.map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub fn config_file() -> PathBuf {
    config_home().join("config.toml")
}

pub fn games_dir() -> PathBuf {
    data_home().join("games")
}

pub fn game_dir(id: &str) -> PathBuf {
    games_dir().join(id)
}

pub fn modules_data_dir(id: &str) -> PathBuf {
    data_home().join("modules").join(id)
}

pub fn sources_data_dir(id: &str) -> PathBuf {
    data_home().join("sources").join(id)
}

pub fn current_session_file() -> PathBuf {
    state_home().join("current-session.json")
}

/// Where a launch with `debug_log` on sends Proton's and DXVK's files.
pub fn game_logs_dir(id: &str) -> PathBuf {
    state_home().join("logs").join(id)
}

pub fn session_log_dir(id: &str, session_id: &str) -> PathBuf {
    game_logs_dir(id).join(session_id)
}

/// The CLI for hooks and ExecStopPost: $UNIVERSE_BIN, else argv[0] (makeWrapper's `exec -a "$0"` keeps the wrapper path where current_exe() would not).
pub fn self_exe() -> PathBuf {
    if let Some(p) = std::env::var_os("UNIVERSE_BIN").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let argv0 = std::env::args_os().next().map(PathBuf::from).unwrap_or_default();
    let resolved = if argv0.components().count() > 1 {
        std::env::current_dir().ok().map(|d| d.join(&argv0))
    } else {
        std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(&argv0)).find(|p| p.is_file()))
    };
    resolved.filter(|p| p.is_file()).or_else(|| std::env::current_exe().ok()).unwrap_or(argv0)
}

fn system_dirs(var: &str, sub: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os(var) {
        out.extend(std::env::split_paths(&p));
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in dirs.split(':').filter(|s| !s.is_empty()) {
        out.push(Path::new(d).join("universe").join(sub));
    }
    // A profile listed twice in XDG_DATA_DIRS would read every manifest twice.
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

pub fn system_module_dirs() -> Vec<PathBuf> {
    system_dirs("UNIVERSE_MODULES_PATH", "modules")
}

pub fn user_modules_dir() -> PathBuf {
    config_home().join("modules")
}

pub fn system_source_dirs() -> Vec<PathBuf> {
    system_dirs("UNIVERSE_SOURCES_PATH", "sources")
}

pub fn user_sources_dir() -> PathBuf {
    config_home().join("sources")
}

pub fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else if p == "~" {
        home()
    } else {
        PathBuf::from(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_dir_listed_twice_is_read_once() {
        let _guard = ENV_LOCK.lock().unwrap();
        let was = (std::env::var_os("XDG_DATA_DIRS"), std::env::var_os("UNIVERSE_MODULES_PATH"));
        std::env::set_var("XDG_DATA_DIRS", "/a/share:/b/share:/a/share:");
        std::env::remove_var("UNIVERSE_MODULES_PATH");
        let dirs = system_module_dirs();
        for (var, v) in [("XDG_DATA_DIRS", was.0), ("UNIVERSE_MODULES_PATH", was.1)] {
            match v {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
        assert_eq!(dirs, [PathBuf::from("/a/share/universe/modules"), PathBuf::from("/b/share/universe/modules")]);
    }
}
