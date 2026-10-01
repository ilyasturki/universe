use std::path::{Path, PathBuf};

pub fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

#[cfg(test)]
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) struct TestEnv {
    saved: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

/// Every `UNIVERSE_*` and `XDG_*` variable cleared, then the homes, HOME and the XDG dirs set under one tempdir; the whole environment put back on drop.
#[cfg(test)]
pub(crate) fn test_env() -> TestEnv {
    let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved: Vec<_> = std::env::vars_os().collect();
    for (var, _) in &saved {
        if var.to_str().is_some_and(|v| v.starts_with("UNIVERSE_") || v.starts_with("XDG_")) {
            std::env::remove_var(var);
        }
    }
    let dir = tempfile::tempdir().unwrap();
    for (var, sub) in [
        ("UNIVERSE_DATA_HOME", "data"),
        ("UNIVERSE_CONFIG_HOME", "config"),
        ("UNIVERSE_STATE_HOME", "state"),
        ("UNIVERSE_CACHE_HOME", "cache"),
        ("UNIVERSE_MODULES_PATH", "modules"),
        ("UNIVERSE_SOURCES_PATH", "sources"),
        ("HOME", "home"),
        ("XDG_DATA_HOME", "home/.local/share"),
        ("XDG_CONFIG_HOME", "home/.config"),
        ("XDG_STATE_HOME", "home/.local/state"),
        ("XDG_CACHE_HOME", "home/.cache"),
        ("XDG_DATA_DIRS", "share"),
        ("XDG_CONFIG_DIRS", "etc/xdg"),
        ("XDG_RUNTIME_DIR", "run"),
    ] {
        std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        std::env::set_var(var, dir.path().join(sub));
    }
    TestEnv { saved, dir, _lock: lock }
}

#[cfg(test)]
impl TestEnv {
    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }
}

#[cfg(test)]
impl Drop for TestEnv {
    fn drop(&mut self) {
        for (var, _) in std::env::vars_os() {
            if !self.saved.iter().any(|(v, _)| *v == var) {
                std::env::remove_var(var);
            }
        }
        for (var, value) in &self.saved {
            if std::env::var_os(var).as_ref() != Some(value) {
                std::env::set_var(var, value);
            }
        }
    }
}

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

pub fn runtime_dir() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(state_home).join("universe");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// xdg-user-dirs:`$XDG_<NAME>_DIR`, else the entry in `~/.config/user-dirs.dirs`, else `~/<fallback>`.
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

/// The CLI for hooks and ExecStopPost: $UNIVERSE_BIN, else argv[0] (makeWrapper's `exec -a "$0"` keeps the wrapper path where current_exe() would not);
/// inside another program (the UI's Python), `universe` on PATH.
pub fn self_exe() -> PathBuf {
    if let Some(p) = std::env::var_os("UNIVERSE_BIN").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let embedded = std::env::current_exe().ok().and_then(|e| e.file_name().map(|n| !n.to_string_lossy().starts_with("universe"))).unwrap_or(false);
    if let Some(cli) = embedded.then(|| crate::runners::on_path("universe")).flatten() {
        return cli;
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
        let _env = test_env();
        std::env::set_var("XDG_DATA_DIRS", "/a/share:/b/share:/a/share:");
        std::env::remove_var("UNIVERSE_MODULES_PATH");
        assert_eq!(system_module_dirs(), [PathBuf::from("/a/share/universe/modules"), PathBuf::from("/b/share/universe/modules")]);
    }

    #[test]
    fn a_test_env_seals_the_machine_off_and_puts_the_environment_back() {
        let seen = |env: &TestEnv| {
            let dir = env.path().to_string_lossy().into_owned();
            let mut vars: Vec<_> = std::env::vars_os().map(|(k, v)| (k, v.to_string_lossy().replace(&dir, "<tmp>"))).collect();
            vars.sort();
            vars
        };
        let before = seen(&test_env());
        let dir = {
            let env = test_env();
            std::env::set_var("SET_BY_A_TEST", "1");
            std::env::remove_var("PATH");
            for p in [data_home(), config_home(), state_home(), cache_home(), home(), user_dir("GAMES", "Games"), runtime_dir()] {
                assert!(p.starts_with(env.path()), "{}", p.display());
            }
            assert!(system_module_dirs().iter().chain(&system_source_dirs()).all(|d| d.starts_with(env.path())), "nothing installed on the machine");
            env.path().to_path_buf()
        };
        assert!(!dir.exists(), "the tempdir goes with the guard");
        let poisoner = std::thread::spawn(|| {
            let _env = test_env();
            panic!("a failing test");
        });
        assert!(poisoner.join().is_err());
        assert_eq!(seen(&test_env()), before, "what a test set or removed is back, and a panicked holder doesn't fail the next");
    }
}
