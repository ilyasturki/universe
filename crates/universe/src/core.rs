use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::game::Game;
use crate::host::Host;
use crate::library::{self, Resolved};
use crate::modules::{self, HookEnv, Module, SourceEvent};
use crate::paths;
use crate::{Error, Result};

/// Two lifetimes: a caller reborrows the same callback across several awaited calls.
pub type Progress<'a, 'b> = &'a mut (dyn FnMut(u64, u64, &str) + 'b);

fn trash(path: &Path) -> Result<()> {
    let st = std::process::Command::new("trash").arg(path).status();
    if !st.map(|s| s.success()).unwrap_or(false) {
        return Err(Error::Io(format!("trash {} failed", path.display())));
    }
    Ok(())
}

pub fn title_of(path: &Path) -> String {
    let stem = if path.is_dir() { path.file_name() } else { path.file_stem() }.map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut s = stem.replace('_', " ");
    for (open, close) in [('[', ']'), ('(', ')')] {
        while let (Some(a), Some(b)) = (s.find(open), s.find(close)) {
            if b < a {
                break;
            }
            s.replace_range(a..=b, " ");
        }
    }
    let is_version = |w: &str| w.len() > 1 && w.starts_with('v') && w[1..].chars().all(|c| c.is_ascii_digit() || c == '.') && w[1..].starts_with(|c: char| c.is_ascii_digit());
    let words: Vec<&str> = s.split_whitespace().filter(|w| !is_version(w)).collect();
    words.join(" ").trim_end_matches(['-', ' ']).trim().to_string()
}

/// What the hooks, `ExecStopPost` and the game need of the launcher's environment.
pub(crate) fn passthrough_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for k in ["PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY", "DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE", "GI_TYPELIB_PATH", "UNIVERSE_DATA_HOME", "UNIVERSE_CONFIG_HOME", "UNIVERSE_STATE_HOME", "UNIVERSE_MODULES_PATH", "RUST_LOG"] {
        if let Ok(v) = std::env::var(k) {
            env.insert(k.to_string(), v);
        }
    }
    env.insert("UNIVERSE_BIN".into(), paths::self_exe().to_string_lossy().to_string());
    env
}

fn load_source_caches(modules: &[Module]) -> BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>> {
    let mut caches = BTreeMap::new();
    for m in modules.iter().filter(|m| m.is_source()) {
        if let Some(v) = std::fs::read_to_string(m.data_dir().join("library.json")).ok().and_then(|s| serde_json::from_str(&s).ok()) {
            caches.insert(m.id().to_string(), v);
        }
    }
    caches
}

pub struct Core {
    pub config: RwLock<Config>,
    pub modules: RwLock<Vec<Module>>,
    pub games: RwLock<Vec<Resolved>>,
    pub source_libraries: Mutex<BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>>>,
    pub source_logins: Mutex<BTreeMap<String, String>>,
    logins_probed: Mutex<bool>,
    pub(crate) scope: std::sync::Mutex<Option<String>>,
    pub(crate) host: Host,
}

impl Core {
    fn new(config: Config, host: Host) -> Core {
        let modules = modules::discover(&config);
        let games = library::load_all(&config, &modules);
        let caches = load_source_caches(&modules);
        Core {
            config: RwLock::new(config),
            modules: RwLock::new(modules),
            games: RwLock::new(games),
            source_libraries: Mutex::new(caches),
            source_logins: Mutex::new(BTreeMap::new()),
            logins_probed: Mutex::new(false),
            scope: std::sync::Mutex::new(None),
            host,
        }
    }

    pub async fn open() -> Result<Core> {
        let config = Config::load()?;
        let host = Host::live(&config);
        Core::open_with(config, host).await
    }

    /// The core over a given host: what the tests open on the in-memory one.
    pub async fn open_with(config: Config, host: Host) -> Result<Core> {
        std::fs::create_dir_all(paths::games_dir())?;
        let core = Core::new(config, host);
        core.reconcile().await?;
        Ok(core)
    }

    async fn refresh_login(&self, m: &Module) {
        let user = self.run_verb(m, "status", &[], None).await.ok().and_then(|ev| ev.into_iter().find_map(|e| if let SourceEvent::LoggedIn { user } = e { Some(user) } else { None }));
        let mut logins = self.source_logins.lock().await;
        match user {
            Some(u) => logins.insert(m.id().to_string(), u),
            None => logins.remove(m.id()),
        };
    }

    /// Login probes reach the network: once per process, on the first call that reports them.
    async fn ensure_logins(&self) {
        let mut probed = self.logins_probed.lock().await;
        if *probed {
            return;
        }
        let sources: Vec<Module> = self.modules.read().await.iter().filter(|m| m.is_source() && m.active()).cloned().collect();
        for m in sources {
            self.refresh_login(&m).await;
        }
        *probed = true;
    }

    pub async fn reload_all(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let games = library::load_all(&config, &modules);
        *self.games.write().await = games;
        Ok(())
    }

    pub async fn reload_game(&self, id: &str) -> Result<()> {
        let config = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let toml_path = paths::game_dir(id).join("game.toml");
        let mut games = self.games.write().await;
        games.retain(|g| g.game.id != id);
        if toml_path.exists() {
            games.push(library::resolve(Game::load(&toml_path)?, &config, &modules));
        }
        library::sort_default(&mut games);
        Ok(())
    }

    pub async fn list(&self) -> Vec<serde_json::Value> {
        let games = self.games.read().await;
        games.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.to_json()).collect()
    }

    pub async fn get(&self, id: &str) -> Result<Resolved> {
        let games = self.games.read().await;
        games.iter().find(|g| g.game.id == id).cloned().ok_or_else(|| Error::NotFound(id.into()))
    }

    pub async fn resolve(&self, query: &str) -> Vec<String> {
        library::resolve_query(&self.games.read().await, query).iter().map(|g| g.game.id.clone()).collect()
    }

    pub async fn resolve_one(&self, query: &str) -> Result<String> {
        if self.games.read().await.iter().any(|g| g.game.id == query) {
            return Ok(query.to_string());
        }
        let ids = self.resolve(query).await;
        match ids.len() {
            0 => Err(Error::NotFound(query.into())),
            1 => Ok(ids[0].clone()),
            _ => Err(Error::Ambiguous(ids.join(", "))),
        }
    }

    pub async fn set(&self, id: &str, key: &str, value: &str) -> Result<()> {
        let r = self.get(id).await?;
        let modules = self.modules.read().await.clone();
        let top = key.split('.').next().unwrap_or("");
        let (real_key, module_id) = if top == "modules" {
            (key.to_string(), key.split('.').nth(1).map(|s| s.to_string()))
        } else if modules.iter().any(|m| m.id() == top) {
            (format!("modules.{key}"), Some(top.to_string()))
        } else {
            (key.to_string(), None)
        };
        if let Some(mid) = module_id {
            let m = modules.iter().find(|m| m.id() == mid).ok_or_else(|| Error::NotFound(format!("module {mid}")))?;
            let skey = real_key.splitn(3, '.').nth(2).ok_or_else(|| Error::Invalid(format!("bad key {key}")))?;
            if !value.is_empty() {
                m.validate_setting(skey, value, true)?;
            }
        }
        let mut value = value.to_string();
        if real_key == "launch.runner" && !value.is_empty() {
            value = crate::runners::spec(&value).ok_or_else(|| Error::Invalid(format!("unknown runner {value}")))?.id.into();
        }
        if let Some(field) = real_key.strip_prefix("launch.") {
            crate::launch_keys::validate(crate::launch_keys::Scope::Game, field, &value)?;
        }
        if let Some(okey) = real_key.strip_prefix("launch.options.") {
            let spec = crate::runners::spec(&r.game.runner_id()).ok_or_else(|| Error::Invalid(format!("{id} has no known runner")))?;
            if !value.is_empty() {
                spec.validate_option(okey, &value)?;
            }
        }
        crate::game::set_key(&r.game.toml_path(), &real_key, &value)?;
        self.reload_game(id).await
    }

    pub async fn add_game(&self, v: &serde_json::Value) -> Result<String> {
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").trim().to_string();
        let spec = crate::runners::spec(&field("runner")).ok_or_else(|| Error::Invalid(format!("unknown runner '{}'", field("runner"))))?;
        let exe = field("exe");
        if exe.is_empty() {
            return Err(Error::Invalid("a game file is needed".into()));
        }
        let path = paths::expand(&exe);
        let path = if path.is_absolute() { path } else { std::env::current_dir().map(|d| d.join(&path)).unwrap_or(path) };
        if spec.file_required && !path.exists() {
            return Err(Error::NotFound(path.to_string_lossy().into()));
        }
        let title = if field("title").is_empty() { title_of(&path) } else { field("title") };
        if title.is_empty() {
            return Err(Error::Invalid("a title is needed".into()));
        }
        let mut g = Game::new(&title);
        if g.id.is_empty() {
            return Err(Error::Invalid(format!("'{title}' makes no id")));
        }
        if g.toml_path().exists() {
            return Err(Error::Invalid(format!("{} is already in the library", g.id)));
        }
        g.launch.runner = spec.id.into();
        g.launch.exe = path.to_string_lossy().into();
        g.platform = if field("platform").is_empty() { spec.default_platform().into() } else { field("platform") };
        if spec.kind == crate::runners::Kind::Emulator {
            g.launch.arch = String::new();
        }
        if matches!(spec.kind, crate::runners::Kind::Proton | crate::runners::Kind::Wine) {
            let cfg = self.config.read().await.clone();
            g.launch.prefix = cfg.prefixes_root().join(&g.id).to_string_lossy().into();
            g.source.dir = path.parent().map(|p| p.to_string_lossy().into()).unwrap_or_default();
        }
        g.save()?;
        self.reload_game(&g.id).await?;
        Ok(g.id)
    }

    pub async fn runners(&self) -> Vec<serde_json::Value> {
        let cfg = self.config.read().await.clone();
        crate::runners::RUNNERS.iter().map(|s| crate::runners::to_json(s, &cfg)).collect()
    }

    pub async fn set_runner_setting(&self, runner: &str, key: &str, value: &str) -> Result<()> {
        let spec = crate::runners::spec(runner).ok_or_else(|| Error::NotFound(format!("runner {runner}")))?;
        if key == "gamescope" && !matches!(value, "" | "true" | "false") {
            return Err(Error::Invalid("gamescope must be true or false".into()));
        }
        if !matches!(key, "exe" | "args" | "gamescope") && !value.is_empty() {
            spec.validate_option(key, value)?;
        }
        Config::set_key(&paths::config_file(), &format!("runners.{}.{key}", spec.id), value)?;
        self.reload_config().await
    }

    /// The game stays in the library with its hours, journal and recordings, not installed.
    pub async fn uninstall(&self, id: &str) -> Result<()> {
        let r = self.get(id).await?;
        let dir = paths::expand(&r.game.source.dir);
        if r.game.source.dir.is_empty() || !dir.is_dir() {
            return Err(Error::Invalid(format!("{id} has no install folder")));
        }
        // Never a root, a home, a games root or a folder another game lives in: only its own.
        let config = self.config.read().await.clone();
        let home = paths::home();
        let shared = self.games.read().await.iter().any(|x| x.game.id != id && !x.game.source.dir.is_empty() && paths::expand(&x.game.source.dir).starts_with(&dir));
        if dir.parent().is_none() || dir == home || dir == config.games_root() || home.starts_with(&dir) || shared {
            return Err(Error::Invalid(format!("refusing to trash {}", dir.display())));
        }
        trash(&dir)?;
        // Cleared so the next install sets them afresh, wherever it lands.
        let toml = r.game.toml_path();
        for key in ["source.dir", "source.build_id", "launch.exe"] {
            crate::game::set_key(&toml, key, "")?;
        }
        self.reload_game(id).await
    }

    pub async fn remove(&self, id: &str, purge: bool) -> Result<()> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        for (root, label) in [(config.recordings_root(), "recordings"), (config.journal_root(), "journal")] {
            let from = root.join(id);
            if from.is_dir() {
                let archive = root.join(".archive");
                std::fs::create_dir_all(&archive)?;
                let to = archive.join(id);
                if to.exists() {
                    tracing::warn!("{label}: {} already archived", to.display());
                } else {
                    std::fs::rename(&from, &to)?;
                }
            }
        }
        if purge && !r.game.launch.prefix.is_empty() {
            let prefix = paths::expand(&r.game.launch.prefix);
            if prefix.is_dir() && trash(&prefix).is_err() {
                tracing::warn!("trash {} failed; left in place", prefix.display());
            }
        }
        crate::game::set_key(&r.game.toml_path(), "hidden", "true")?;
        let mut g = Game::load(&r.game.toml_path())?;
        g.removed_at = chrono::Local::now().to_rfc3339();
        g.save()?;
        self.reload_game(id).await
    }

    pub async fn import_lutris(&self, apply: bool) -> Result<crate::lutris::Report> {
        let config = self.config.read().await.clone();
        let report = crate::lutris::import(&config, apply)?;
        if apply {
            self.reload_all().await?;
        }
        Ok(report)
    }

    pub(crate) fn hook_env_base(&self, r: &Resolved, cfg: &Config) -> HookEnv {
        let mut env = HookEnv::default();
        env.set("GAME_ID", r.game.id.clone());
        env.set("GAME_SLUG", r.game.id.clone());
        env.set("GAME_TITLE", r.game.title.clone());
        env.set("GAME_DIR", r.game.game_root().to_string_lossy().to_string());
        env.set("GAME_EXE", r.game.exe_path().to_string_lossy().to_string());
        env.set("GAME_TOML", r.game.toml_path().to_string_lossy().to_string());
        env.set("JOURNAL_DIR", r.game.journal_dir().to_string_lossy().to_string());
        for (k, v) in passthrough_env() {
            env.set(&k, v);
        }
        env.set("UNIVERSE_GAME_JSON", r.to_json().to_string());
        env.set("UNIVERSE_JOURNAL_ROOT", cfg.journal_root().to_string_lossy().to_string());
        env
    }

    pub(crate) fn module_env(&self, m: &Module, r: &Resolved, cfg: &Config, base: &HookEnv) -> HookEnv {
        let mut env = base.clone();
        env.set("MODULE_SETTINGS_JSON", serde_json::Value::Object(m.merged_settings(cfg, Some(&r.game))).to_string());
        env
    }

    pub(crate) async fn hook_modules(&self, r: &Resolved, hook: &str) -> Vec<Module> {
        let cfg = self.config.read().await.clone();
        self.modules
            .read()
            .await
            .iter()
            .filter(|m| m.active() && m.is_hooks() && m.hook(hook).is_some())
            .filter(|m| m.merged_settings(&cfg, Some(&r.game)).get("enabled").and_then(|v| v.as_bool()).unwrap_or(true))
            .cloned()
            .collect()
    }

    async fn shell_conn(&self) -> Option<zbus::Connection> {
        zbus::Connection::session().await.inspect_err(|e| tracing::warn!("session bus: {e}")).ok()
    }

    /// The running game's window, as the shell sees it: `None` before it maps; `Unavailable` off GNOME.
    pub async fn session_window(&self) -> Result<Option<crate::desktop::Toplevel>> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        let Some(cg) = self.host.units.cgroup(&c.unit).await else { return Ok(None) };
        let windows = crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        Ok(crate::desktop::pick_window(&windows, |pid| crate::desktop::pid_in_cgroup(pid, &cg)))
    }

    /// Blocks until the session's window is on screen, then gives it the focus; the window, or
    /// `None` when the session ended first or `timeout` ran out. `Unavailable` off GNOME, at once.
    pub async fn wait_session_window(&self, session_id: &str, timeout: std::time::Duration) -> Result<Option<crate::desktop::Toplevel>> {
        crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let Some(c) = self.current().await else { return Ok(None) };
            if c.session_id != session_id {
                return Ok(None);
            }
            if let Some(w) = self.session_window().await? {
                if let Err(e) = crate::desktop::activate_window(w.id).await {
                    tracing::warn!("activate {}: {e}", w.id);
                }
                return Ok(Some(w));
            }
            if std::time::Instant::now() > deadline {
                return Ok(None);
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
    }

    pub async fn focus_session(&self) -> Result<()> {
        let w = self.session_window().await?.ok_or_else(|| Error::NotFound("the game has no window yet".into()))?;
        match crate::desktop::activate_window(w.id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::NotFound("the game's window is gone".into())),
            Err(e) => Err(Error::Unavailable(e)),
        }
    }

    pub async fn focus_pid(&self, pid: u32) -> Result<()> {
        let windows = crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        let w = windows.iter().filter(|w| w.pid == pid as i64 && !w.hidden).max_by_key(|w| w.width * w.height).ok_or_else(|| Error::NotFound(format!("no window of pid {pid}")))?;
        crate::desktop::activate_window(w.id).await.map_err(Error::Unavailable)?;
        Ok(())
    }

    pub async fn screenshot(&self) -> Result<String> {
        let cur = self.current().await;
        let cfg = self.config.read().await.clone();
        let (r, env) = match cur {
            Some(c) => {
                let r = self.get(&c.id).await?;
                let mut env = self.hook_env_base(&r, &cfg);
                env.set("SESSION_ID", c.session_id);
                env.set("SESSION_SCREEN", c.screen);
                (r, env)
            }
            None => {
                let games = self.games.read().await;
                let r = games.first().cloned().ok_or_else(|| Error::NotFound("no session running".into()))?;
                let mut env = self.hook_env_base(&r, &cfg);
                env.set("SESSION_SCREEN", crate::desktop::pick_screen(""));
                env.set("JOURNAL_DIR", paths::state_home().join("screenshots").to_string_lossy().to_string());
                (r, env)
            }
        };
        for m in self.hook_modules(&r, "screenshot").await {
            let menv = self.module_env(&m, &r, &cfg, &env);
            let out = modules::run_blocking(&m, "screenshot", &menv).await?;
            let path = out.stdout.trim().to_string();
            if out.status == 0 && !path.is_empty() {
                return Ok(path);
            }
        }
        Err(Error::Unavailable("no module provides a screenshot hook".into()))
    }

    /// Last first.
    pub async fn sessions(&self, id: &str) -> Result<Vec<crate::sessions::Session>> {
        let r = self.get(id).await?;
        Ok(r.sessions.iter().rev().cloned().collect())
    }

    async fn game_of_session(&self, session_id: &str) -> Option<String> {
        if let Some(id) = crate::session::read_marker().filter(|m| m.current.session_id == session_id).map(|m| m.current.id) {
            return Some(id);
        }
        let games = self.games.read().await;
        games.iter().find(|g| g.sessions.iter().any(|s| s.session == session_id)).map(|g| g.game.id.clone())
    }

    pub async fn file_recording(&self, session_id: &str, path: &str) -> Result<String> {
        let id = self.game_of_session(session_id).await.ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
        let r = self.get(&id).await?;
        let cfg = self.config.read().await.clone();
        let dest = crate::recording::file(&r.game, session_id, Path::new(path), &cfg.recordings_root())?;
        self.reload_game(&id).await?;
        Ok(dest.to_string_lossy().to_string())
    }

    pub async fn recordings(&self, id: &str) -> Result<Vec<serde_json::Value>> {
        let r = self.get(id).await?;
        crate::recording::list(&r.game)
    }

    pub async fn remove_recording(&self, id: &str, session_id: &str) -> Result<()> {
        let r = self.get(id).await?;
        let s = r.sessions.iter().find(|s| s.session == session_id).ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
        let Some(path) = s.recording.as_deref().filter(|p| !p.is_empty()) else {
            return Err(Error::NotFound(format!("session {session_id} has no recording")));
        };
        let path = Path::new(path);
        if path.is_file() {
            trash(path)?;
        }
        crate::sessions::update(&r.game.sessions_path(), session_id, |s| s.recording = None)?;
        self.reload_game(id).await
    }

    pub async fn add_entry(&self, session_id: &str, mut entry: crate::journal::Entry) -> Result<()> {
        if entry.session.is_empty() {
            entry.session = session_id.into();
        }
        if entry.session != session_id {
            return Err(Error::Invalid("entry.session does not match".into()));
        }
        let id = if entry.game.is_empty() { self.game_of_session(session_id).await } else { Some(entry.game.clone()) }.ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
        let r = self.get(&id).await?;
        entry.game = id.clone();
        let journal_dir = r.game.journal_dir();
        crate::journal::fill_timing(std::slice::from_mut(&mut entry), &crate::journal::sessions_for_note(&r.sessions, &journal_dir));
        crate::journal::write(&journal_dir, &entry)?;
        self.reload_game(&id).await
    }

    /// Read from disk on every call: a pending entry's timeout is judged now, not at the last reload.
    pub async fn journal(&self, id: &str) -> Result<Vec<crate::journal::Entry>> {
        let r = self.get(id).await?;
        Ok(crate::journal::load(&r.game.journal_dir(), &r.sessions))
    }

    pub async fn remove_journal_entry(&self, id: &str, session_id: &str) -> Result<()> {
        let r = self.get(id).await?;
        let journal_dir = r.game.journal_dir();
        let entries = crate::journal::read_all(&journal_dir)?;
        let entry = entries.iter().find(|e| e.session == session_id).ok_or_else(|| Error::NotFound(format!("journal entry {session_id}")))?;
        let cfg = self.config.read().await.clone();
        let note_dir = cfg.journal_root().join(id);
        if entry.state == "pending" {
            // The hook's `finally` does not run under SIGTERM: the pending file is ours to drop.
            let unit = format!("universe-journal-post-process-{session_id}");
            let _ = std::process::Command::new("systemctl").args(["--user", "stop", &unit]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        }
        for state in ["pending", "failed"] {
            let p = journal_dir.join(format!("{session_id}.{state}.json"));
            if p.is_file() {
                std::fs::remove_file(&p)?;
            }
        }
        let written = journal_dir.join(format!("{session_id}.json"));
        if written.is_file() {
            trash(&written)?;
        }
        for rel in &entry.images {
            if rel.starts_with('/') || rel.split('/').any(|seg| seg == "..") {
                continue;
            }
            for dir in [&journal_dir, &note_dir] {
                let p = dir.join(rel);
                if p.is_file() {
                    trash(&p)?;
                }
            }
        }
        self.reload_game(id).await?;
        if note_dir.is_dir() {
            self.render_journal(id).await?;
        }
        Ok(())
    }

    pub async fn pending_journals(&self) -> Vec<serde_json::Value> {
        let games = self.games.read().await;
        let mut out = Vec::new();
        for r in games.iter() {
            for e in crate::journal::pending(&r.game.journal_dir()) {
                out.push(serde_json::json!({"game": r.game.id, "title": r.game.title, "session": e.session, "started_at": e.started_at}));
            }
        }
        out
    }

    pub async fn render_journal(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let note_dir = cfg.journal_root().join(id);
        let journal_dir = r.game.journal_dir();
        let sessions = crate::journal::sessions_for_note(&r.sessions, &journal_dir);
        let entries = crate::journal::load(&journal_dir, &r.sessions);
        let path = crate::journal::write_note(&r.game.title, &entries, &sessions, &journal_dir, &note_dir, &crate::journal::Locale::from_env())?;
        Ok(path.to_string_lossy().into())
    }

    pub async fn modules(&self) -> Vec<serde_json::Value> {
        self.modules.read().await.iter().map(|m| m.to_json()).collect()
    }

    pub async fn reload_modules(&self) {
        let cfg = self.config.read().await.clone();
        *self.modules.write().await = modules::discover(&cfg);
    }

    pub async fn enable_module(&self, id: &str, enabled: bool) -> Result<()> {
        if !self.modules.read().await.iter().any(|m| m.id() == id) {
            return Err(Error::NotFound(format!("module {id}")));
        }
        let mut list = self.config.read().await.modules.enabled.clone();
        list.retain(|m| m != id);
        if enabled {
            list.push(id.into());
        }
        Config::set_key(&paths::config_file(), "modules.enabled", &list.join(","))?;
        self.reload_config().await
    }

    pub async fn reload_config(&self) -> Result<()> {
        self.reload_settings().await?;
        self.reload_all().await
    }

    /// config.toml and the module list only, not the library: what the controller watcher needs, without a rescan stalling it.
    pub async fn reload_settings(&self) -> Result<()> {
        let cfg = Config::load()?;
        *self.config.write().await = cfg;
        self.reload_modules().await;
        Ok(())
    }

    pub async fn module_settings(&self, module: &str, game_id: &str) -> Result<serde_json::Value> {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await;
        let m = modules.iter().find(|m| m.id() == module).ok_or_else(|| Error::NotFound(format!("module {module}")))?;
        let game = if game_id.is_empty() { None } else { Some(self.get(game_id).await?.game) };
        Ok(serde_json::Value::Object(m.merged_settings(&cfg, game.as_ref())))
    }

    pub async fn module_setting_choices(&self, module: &str, key: &str) -> Result<Vec<String>> {
        let cfg = self.config.read().await.clone();
        let m = self.modules.read().await.iter().find(|m| m.id() == module).cloned().ok_or_else(|| Error::NotFound(format!("module {module}")))?;
        let settings = m.merged_settings(&cfg, None);
        modules::setting_choices(&m, &settings, key).await
    }

    pub async fn set_module_setting(&self, module: &str, game_id: &str, key: &str, value: &str) -> Result<()> {
        {
            let modules = self.modules.read().await;
            let m = modules.iter().find(|m| m.id() == module).ok_or_else(|| Error::NotFound(format!("module {module}")))?;
            if !value.is_empty() {
                m.validate_setting(key, value, !game_id.is_empty())?;
            }
        }
        if game_id.is_empty() {
            Config::set_key(&paths::config_file(), &format!("modules.{module}.{key}"), value)?;
            self.reload_config().await
        } else {
            let r = self.get(game_id).await?;
            crate::game::set_key(&r.game.toml_path(), &format!("modules.{module}.{key}"), value)?;
            self.reload_game(game_id).await
        }
    }

    pub async fn settings(&self) -> serde_json::Value {
        self.config.read().await.to_json()
    }

    /// `{screen, width, height, refresh}`, zeros when no screen can be read.
    pub async fn screen_mode(&self, screen: &str) -> serde_json::Value {
        let screen = crate::desktop::pick_screen(screen);
        let mode = crate::desktop::screen_mode(&screen).await.unwrap_or_default();
        serde_json::json!({ "screen": screen, "width": mode.width, "height": mode.height, "refresh": mode.refresh })
    }

    /// The launch keys of `scope` (`game`, `global`, `both`) as settings rows, the screen's mode sizing the choices.
    pub fn launch_keys(&self, scope: &str, screen: Option<crate::gamescope::Mode>) -> Result<Vec<crate::launch_keys::Row>> {
        Ok(crate::launch_keys::rows(crate::launch_keys::Scope::parse(scope)?, screen))
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        if key == "controller.volume_step" {
            crate::controller::volume_step_value(value)?;
        }
        if let Some(field) = key.strip_prefix("launch.") {
            crate::launch_keys::validate(crate::launch_keys::Scope::Global, field, value)?;
        }
        Config::set_key(&paths::config_file(), key, value)?;
        if key.starts_with("controller.") {
            return self.reload_settings().await;
        }
        self.reload_config().await
    }

    pub async fn controller_state(&self) -> serde_json::Value {
        crate::controller::state_json(&self.config.read().await.controller)
    }

    /// The pads readable right now, as `watch` would announce them; for a frontend whose watcher waits on the lock.
    pub async fn controller_pads(&self) -> Vec<serde_json::Value> {
        let cfg = self.config.read().await.controller.clone();
        crate::controller::watch::enumerate_json(&cfg)
    }

    pub async fn set_controller_macro(&self, m: crate::controller::Macro) -> Result<()> {
        m.validate()?;
        let cfg = self.config.read().await.clone();
        let mut list = cfg.controller.macros();
        crate::controller::upsert_macro(&mut list, m);
        crate::controller::write_macros(&list)?;
        self.reload_settings().await
    }

    /// Removes one macro; an empty trigger removes both of the button's.
    pub async fn remove_controller_macro(&self, family: &str, button: &str, trigger: &str) -> Result<()> {
        let cfg = self.config.read().await.clone();
        let mut list = cfg.controller.macros();
        let before = list.len();
        list.retain(|m| !(m.family == family && m.button == button && (trigger.is_empty() || m.trigger == trigger)));
        if list.len() == before {
            return Err(Error::NotFound(format!("no macro on {family} {button} {trigger}")));
        }
        crate::controller::write_macros(&list)?;
        self.reload_settings().await
    }

    /// `Some([])` leaves the slot unbound, `None` restores the seeds.
    pub async fn set_controller_button(&self, family: &str, slot: &str, codes: Option<Vec<String>>) -> Result<()> {
        let f = crate::controller::family_by_id(family).ok_or_else(|| Error::NotFound(format!("family {family}")))?;
        if !f.slots().any(|s| s.id == slot) {
            return Err(Error::NotFound(format!("{} has no button {slot}", f.name)));
        }
        if let Some(list) = &codes {
            for c in list {
                crate::controller::keys::parse_source(c).ok_or_else(|| Error::Invalid(format!("unknown code {c}")))?;
            }
        }
        crate::controller::write_button(family, slot, codes.as_deref())?;
        self.reload_settings().await
    }

    pub async fn doctor(&self) -> Vec<crate::doctor::Check> {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let conn = if crate::desktop::detect(&cfg) == crate::desktop::Profile::Gnome { self.shell_conn().await } else { None };
        let runners: Vec<String> = self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.effective.runner.clone()).collect();
        crate::doctor::run(&cfg, &modules, conn.as_ref(), &runners).await
    }

    pub async fn source(&self, id: &str) -> Result<Module> {
        let modules = self.modules.read().await;
        let m = modules.iter().find(|m| m.id() == id && m.is_source()).cloned().ok_or_else(|| Error::NotFound(format!("source {id}")))?;
        if !m.available {
            return Err(Error::Unavailable(format!("{id}: missing {}", m.missing.join(", "))));
        }
        if !m.enabled {
            return Err(Error::Unavailable(format!("{id} is disabled")));
        }
        Ok(m)
    }

    pub async fn sources(&self) -> Vec<serde_json::Value> {
        self.ensure_logins().await;
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await;
        let caches = self.source_libraries.lock().await;
        let logins = self.source_logins.lock().await;
        let list: Vec<serde_json::Value> = modules
            .iter()
            .filter(|m| m.is_source())
            .map(|m| {
                let s = m.merged_settings(&cfg, None);
                serde_json::json!({
                    "id": m.id(), "name": m.manifest.name, "available": m.available, "enabled": m.enabled,
                    "missing": m.missing, "games_dir": s.get("games_dir").cloned().unwrap_or(serde_json::Value::Null),
                    "library_cached": caches.get(m.id()).map(|v| v.len()).unwrap_or(0),
                    "logged_in": logins.contains_key(m.id()), "user": logins.get(m.id()).cloned().unwrap_or_default(),
                })
            })
            .collect();
        list
    }

    async fn run_verb(&self, m: &Module, verb: &str, args: &[String], mut progress: Option<Progress<'_, '_>>) -> Result<Vec<SourceEvent>> {
        let cfg = self.config.read().await.clone();
        let settings = m.merged_settings(&cfg, None);
        let mut events = Vec::new();
        modules::run_source(m, &settings, verb, args, |ev| {
            if let (SourceEvent::Progress { done, total, message }, Some(p)) = (&ev, progress.as_mut()) {
                p(*done, *total, message);
            }
            events.push(ev);
        })
        .await?;
        Ok(events)
    }

    pub async fn source_login_url(&self, source: &str) -> Result<String> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "login", &[], None).await?;
        events.iter().find_map(|e| if let SourceEvent::LoginUrl { url } = e { Some(url.clone()) } else { None }).ok_or_else(|| Error::Io("no login_url event".into()))
    }

    pub async fn source_login(&self, source: &str, code: &str) -> Result<String> {
        let m = self.source(source).await?;
        let ev = self.run_verb(&m, "login", &[code.to_string()], None).await?;
        let user = ev.iter().find_map(|e| if let SourceEvent::LoggedIn { user } = e { Some(user.clone()) } else { None }).unwrap_or_default();
        self.source_logins.lock().await.insert(m.id().to_string(), user.clone());
        Ok(user)
    }

    fn game_events(events: &[SourceEvent]) -> Vec<serde_json::Map<String, serde_json::Value>> {
        events.iter().filter_map(|e| if let SourceEvent::Game(g) = e { Some(g.clone()) } else { None }).collect()
    }

    pub async fn source_library(&self, source: &str, refresh: bool) -> Result<Vec<serde_json::Value>> {
        let m = self.source(source).await?;
        if refresh || !self.source_libraries.lock().await.contains_key(source) {
            let events = self.run_verb(&m, "library", &[], None).await?;
            let games = Self::game_events(&events);
            let _ = std::fs::create_dir_all(m.data_dir());
            let _ = std::fs::write(m.data_dir().join("library.json"), serde_json::to_string(&games).unwrap_or_default());
            self.source_libraries.lock().await.insert(source.into(), games);
        }
        let caches = self.source_libraries.lock().await;
        let list = caches.get(source).cloned().unwrap_or_default();
        let games = self.games.read().await;
        let list: Vec<serde_json::Value> = list
            .into_iter()
            .map(|mut g| {
                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if let Some(local) = games.iter().find(|x| x.game.source.gog_id == gid && !gid.is_empty()) {
                    g.insert("installed".into(), serde_json::Value::Bool(local.game.is_installed()));
                    g.insert("game_id".into(), serde_json::Value::String(local.game.id.clone()));
                    g.insert("build".into(), serde_json::Value::String(local.game.source.build_id.clone()));
                }
                serde_json::Value::Object(g)
            })
            .collect();
        Ok(list)
    }

    pub async fn source_search(&self, source: &str, query: &str) -> Result<Vec<serde_json::Value>> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "search", &[query.to_string()], None).await?;
        Ok(Self::game_events(&events).into_iter().map(serde_json::Value::Object).collect())
    }

    pub async fn source_info(&self, source: &str, game_id: &str) -> Result<serde_json::Value> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "info", &[game_id.to_string()], None).await?;
        Ok(events.iter().find_map(|e| if let SourceEvent::Info { data } = e { Some(data.clone()) } else { None }).unwrap_or(serde_json::Value::Null))
    }

    /// Creates the game when owned and installed, updates its source fields otherwise.
    async fn apply_source_game(&self, source: &str, g: &serde_json::Map<String, serde_json::Value>, create: bool) -> Result<Option<String>> {
        let field = |k: &str| g.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let (sid, title, dir, exe, build) = (field("id"), field("title"), field("dir"), field("exe"), field("build"));
        let owned = g.get("owned").and_then(|v| v.as_bool());
        let installed = g.get("installed").and_then(|v| v.as_bool()).unwrap_or(!dir.is_empty());
        if sid.is_empty() || title.is_empty() {
            return Ok(None);
        }
        let cfg = self.config.read().await.clone();
        let existing = {
            let games = self.games.read().await;
            games.iter().find(|x| x.game.source.gog_id == sid).or_else(|| games.iter().find(|x| x.game.id == crate::slug::slug(&title))).map(|x| x.game.clone())
        };
        let mut game = match existing {
            Some(g) => g,
            None => {
                if !(create && installed && owned == Some(true)) {
                    return Ok(None);
                }
                let mut ng = Game::new(&title);
                ng.launch.prefix = cfg.prefixes_root().join(&ng.id).to_string_lossy().into();
                ng
            }
        };
        if !dir.is_empty() {
            game.source.dir = dir.clone();
        }
        if !build.is_empty() {
            game.source.build_id = build;
        }
        if game.source.gog_id.is_empty() {
            game.source.gog_id = sid.clone();
        }
        if owned == Some(true) && installed {
            game.source.kind = source.into();
        }
        if let Some(y) = g.get("release_year").and_then(|v| v.as_u64()) {
            if game.release_year == 0 {
                game.release_year = y as u32;
            }
        }
        if let Some(d) = g.get("dlcs").and_then(|v| v.as_array()) {
            game.source.dlcs = d.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        }
        if game.launch.exe.is_empty() && !exe.is_empty() && !dir.is_empty() {
            let p = if exe.starts_with('/') { PathBuf::from(&exe) } else { Path::new(&dir).join(&exe) };
            game.launch.exe = p.to_string_lossy().into();
        }
        game.save()?;
        self.reload_game(&game.id).await?;
        Ok(Some(game.id))
    }

    /// Scans the installed games of one source (all active ones when empty); returns how many entered the library.
    pub async fn source_scan(&self, source: &str, mut progress: Option<Progress<'_, '_>>) -> Result<usize> {
        let ids: Vec<String> = if source.is_empty() {
            self.modules.read().await.iter().filter(|m| m.is_source() && m.active()).map(|m| m.id().to_string()).collect()
        } else {
            vec![self.source(source).await?.id().to_string()]
        };
        let mut found = 0;
        for sid in ids {
            let m = self.source(&sid).await?;
            if let Err(e) = self.source_library(&sid, false).await {
                tracing::warn!("{sid}: library unavailable, scanning without ownership: {e}");
            }
            let events = self.run_verb(&m, "scan", &[], progress.as_deref_mut()).await?;
            for g in Self::game_events(&events) {
                if let Ok(Some(_)) = self.apply_source_game(&sid, &g, true).await {
                    found += 1;
                }
            }
        }
        Ok(found)
    }

    pub async fn source_install(&self, source: &str, game_id: &str, progress: Option<Progress<'_, '_>>) -> Result<String> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "install", &[game_id.to_string()], progress).await?;
        let mut id = String::new();
        for mut g in Self::game_events(&events) {
            g.entry("owned".to_string()).or_insert(serde_json::Value::Bool(true));
            g.entry("installed".to_string()).or_insert(serde_json::Value::Bool(true));
            if let Ok(Some(i)) = self.apply_source_game(source, &g, true).await {
                id = i;
            }
        }
        Ok(id)
    }

    pub async fn source_updates(&self) -> Result<Vec<serde_json::Value>> {
        let mut out = Vec::new();
        let sources: Vec<Module> = self.modules.read().await.iter().filter(|m| m.is_source() && m.active()).cloned().collect();
        for m in sources {
            let events = self.run_verb(&m, "update", &[], None).await?;
            for e in events {
                if let SourceEvent::Update(mut u) = e {
                    u.insert("source".into(), serde_json::Value::String(m.id().into()));
                    out.push(serde_json::Value::Object(u));
                }
            }
        }
        Ok(out)
    }

    /// Every pending title when `game_id` is empty.
    pub async fn source_update(&self, source: &str, game_id: &str, mut progress: Option<Progress<'_, '_>>) -> Result<usize> {
        let m = self.source(source).await?;
        let targets: Vec<String> = if game_id.is_empty() {
            let ev = self.run_verb(&m, "update", &[], progress.as_deref_mut()).await?;
            ev.iter().filter_map(|e| if let SourceEvent::Update(u) = e { u.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) } else { None }).collect()
        } else {
            vec![game_id.to_string()]
        };
        let mut n = 0;
        for t in targets {
            let events = self.run_verb(&m, "update", std::slice::from_ref(&t), progress.as_deref_mut()).await.map_err(|e| Error::Io(format!("{t}: {e}")))?;
            for g in Self::game_events(&events) {
                let _ = self.apply_source_game(source, &g, false).await;
            }
            n += 1;
        }
        Ok(n)
    }

    /// The whole library when `id` is empty; returns (changed, total).
    pub async fn media_refresh(&self, id: &str, force: bool, mut progress: Option<Progress<'_, '_>>) -> Result<(usize, usize)> {
        let ids: Vec<String> = if id.is_empty() { self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.game.id.clone()).collect() } else { vec![self.resolve_one(id).await?] };
        let cfg = self.config.read().await.clone();
        let total = ids.len();
        let mut changed = 0;
        for (i, gid) in ids.iter().enumerate() {
            let Ok(r) = self.get(gid).await else { continue };
            if let Some(p) = progress.as_mut() {
                p(i as u64, total as u64, &r.game.title);
            }
            let cfg = cfg.clone();
            let game = r.game.clone();
            match tokio::task::spawn_blocking(move || crate::media::refresh(&cfg, &game, force)).await.map_err(|e| Error::Io(e.to_string())).and_then(|r| r) {
                Ok(true) => {
                    changed += 1;
                    let _ = self.reload_game(gid).await;
                }
                Ok(false) => {}
                Err(e) => tracing::warn!("media {gid}: {e}"),
            }
        }
        Ok((changed, total))
    }

    pub async fn media_set_slot(&self, id: &str, slot: &str, path: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let placed = crate::media::set_slot(&cfg, &r.game, slot, Path::new(path))?;
        self.reload_game(id).await?;
        Ok(placed.to_string_lossy().into())
    }

    pub async fn media_set_url(&self, id: &str, slot: &str, url: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let (slot, url) = (slot.to_string(), url.to_string());
        let placed = tokio::task::spawn_blocking(move || crate::media::set_slot_url(&cfg, &game, &slot, &url)).await.map_err(|e| Error::Io(e.to_string()))??;
        self.reload_game(id).await?;
        Ok(placed.to_string_lossy().into())
    }

    pub async fn media_unset(&self, id: &str, slot: &str) -> Result<bool> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let gone = crate::media::unset(&cfg, &r.game, slot)?;
        self.reload_game(id).await?;
        Ok(gone)
    }

    pub async fn media_candidates(&self, id: &str, slot: &str, page: u32) -> Result<crate::media::CandidatePage> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let slot = slot.to_string();
        tokio::task::spawn_blocking(move || crate::media::candidates(&cfg, &game, &slot, page)).await.map_err(|e| Error::Io(e.to_string()))?
    }

    pub async fn media_search(&self, id: &str, query: &str) -> Result<Vec<crate::media::Hit>> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let query = query.to_string();
        tokio::task::spawn_blocking(move || crate::media::search(&cfg, &game, &query)).await.map_err(|e| Error::Io(e.to_string()))?
    }

    pub async fn media_status(&self, id: &str) -> Result<Vec<crate::media::MediaStatus>> {
        let cfg = self.config.read().await.clone();
        let games: Vec<crate::game::Game> = if id.is_empty() {
            self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.game.clone()).collect()
        } else {
            vec![self.get(id).await?.game]
        };
        Ok(games.iter().map(|g| crate::media::status(&cfg, g)).collect())
    }

    pub async fn media_pin(&self, id: &str, provider: &str, provider_id: &str) -> Result<()> {
        let key = match provider {
            "sgdb" => "metadata.sgdb_id",
            "rawg" => "metadata.rawg_id",
            "steam" => "metadata.steam_appid",
            _ => return Err(Error::Invalid(format!("unknown provider {provider}"))),
        };
        self.set(id, key, provider_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_from_files() {
        assert_eq!(title_of(Path::new("/g/F-Zero GX.iso")), "F-Zero GX");
        assert_eq!(title_of(Path::new("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci")), "SUPER MARIO ODYSSEY Eur SuperXCi - CLC");
        assert_eq!(title_of(Path::new("/r/Pokemon - HeartGold Version (USA) [rev 1].nds")), "Pokemon - HeartGold Version");
        assert_eq!(title_of(Path::new("/r/mario_kart_wii.wbfs")), "mario kart wii");
    }
}
