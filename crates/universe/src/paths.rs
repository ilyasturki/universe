use std::path::{Path, PathBuf};

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

pub fn data_home() -> PathBuf {
    std::env::var_os("UNIVERSE_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg("XDG_DATA_HOME", ".local/share").join("universe"))
}

pub fn config_home() -> PathBuf {
    std::env::var_os("UNIVERSE_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg("XDG_CONFIG_HOME", ".config").join("universe"))
}

pub fn state_home() -> PathBuf {
    std::env::var_os("UNIVERSE_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg("XDG_STATE_HOME", ".local/state").join("universe"))
}

pub fn cache_home() -> PathBuf {
    std::env::var_os("UNIVERSE_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| xdg("XDG_CACHE_HOME", ".cache").join("universe"))
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

pub fn index_file() -> PathBuf {
    cache_home().join("index.db")
}

pub fn current_session_file() -> PathBuf {
    state_home().join("current-session.json")
}

/// System module dirs: $UNIVERSE_MODULES_PATH (colon-separated) then XDG_DATA_DIRS/universe/modules.
pub fn system_module_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("UNIVERSE_MODULES_PATH") {
        out.extend(std::env::split_paths(&p));
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    for d in dirs.split(':').filter(|s| !s.is_empty()) {
        out.push(Path::new(d).join("universe/modules"));
    }
    out
}

pub fn user_modules_dir() -> PathBuf {
    config_home().join("modules")
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

pub fn ensure_dir(p: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(p)
}
