use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::game::Game;
use crate::index::Index;
use crate::launcher;
use crate::library::{self, Resolved};
use crate::modules::{self, HookEnv, Module, SourceEvent};
use crate::paths;
use crate::sessions::{self, Session};
use crate::{Error, Result};

/// Two lifetimes so a caller can reborrow the same callback across several awaited calls.
pub type Progress<'a, 'b> = &'a mut (dyn FnMut(u64, u64, &str) + 'b);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Current {
    pub session_id: String,
    pub id: String,
    pub title: String,
    pub unit: String,
    pub screen: String,
    pub started_at: String,
}

/// state/current-session.json: everything `session-end` needs to close the session from a process that never saw the launch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Marker {
    #[serde(flatten)]
    pub current: Current,
    pub cursor_was_active: bool,
    #[serde(default)]
    pub inputplumber: bool,
    pub post_command: String,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub hook_env: Vec<(String, String)>,
}

pub fn read_marker() -> Option<Marker> {
    let s = std::fs::read_to_string(paths::current_session_file()).ok()?;
    serde_json::from_str(&s).ok()
}

fn write_marker(m: &Marker) -> Result<()> {
    let p = paths::current_session_file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let other = read_marker().map(|m| format!("{} ({})", m.current.title, m.current.session_id)).unwrap_or_else(|| "another launch".into());
            return Err(Error::Busy(format!("{other} is running")));
        }
        Err(e) => return Err(e.into()),
    };
    use std::io::Write;
    f.write_all(serde_json::to_string(m)?.as_bytes())?;
    Ok(())
}

fn remove_marker() {
    let _ = std::fs::remove_file(paths::current_session_file());
}

fn in_cgroup_of(unit: &str) -> bool {
    std::fs::read_to_string("/proc/self/cgroup").map(|s| s.lines().any(|l| l.rsplit('/').next() == Some(unit))).unwrap_or(false)
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

/// Environment the game unit needs beyond the game's own: our binary and dirs for the hooks and `ExecStopPost`, the desktop for the game.
fn passthrough_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for k in ["PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY", "DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE", "GST_PLUGIN_SYSTEM_PATH_1_0", "GI_TYPELIB_PATH", "UNIVERSE_DATA_HOME", "UNIVERSE_CONFIG_HOME", "UNIVERSE_STATE_HOME", "UNIVERSE_CACHE_HOME", "UNIVERSE_MODULES_PATH", "RUST_LOG"] {
        if let Ok(v) = std::env::var(k) {
            env.insert(k.to_string(), v);
        }
    }
    env.insert("UNIVERSE_BIN".into(), paths::self_exe().to_string_lossy().to_string());
    env
}

/// The macro engine for a launch the UI did not make: bound to the game's unit, it waits on the
/// watcher lock, so it only reads the pads once no launcher does.
fn spawn_controller_watch(session_id: &str, game_unit: &str) -> Result<()> {
    let mut cmd = std::process::Command::new("systemd-run");
    cmd.args(["--user", "--collect", "--quiet"])
        .arg(format!("--unit=universe-controller-{session_id}"))
        .arg(format!("--property=BindsTo={game_unit}"))
        .arg(format!("--property=After={game_unit}"));
    for (k, v) in passthrough_env() {
        cmd.arg(format!("--setenv={k}={v}"));
    }
    cmd.arg(paths::self_exe()).args(["controller", "watch", "--wait"]);
    let status = cmd.stdin(std::process::Stdio::null()).status().map_err(|e| Error::Io(format!("systemd-run: {e}")))?;
    if !status.success() {
        return Err(Error::Io("systemd-run failed for the controller watcher".into()));
    }
    Ok(())
}

fn load_source_caches(modules: &[Module]) -> BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>> {
    let mut caches = BTreeMap::new();
    for m in modules.iter().filter(|m| m.is_source()) {
        if let Ok(s) = std::fs::read_to_string(m.data_dir().join("library.json")) {
            if let Ok(v) = serde_json::from_str(&s) {
                caches.insert(m.id().to_string(), v);
            }
        }
    }
    caches
}

pub struct Core {
    pub config: RwLock<Config>,
    pub modules: RwLock<Vec<Module>>,
    pub games: RwLock<Vec<Resolved>>,
    pub index: Mutex<Index>,
    pub source_libraries: Mutex<BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>>>,
    pub source_logins: Mutex<BTreeMap<String, String>>,
    logins_probed: Mutex<bool>,
    scope: std::sync::Mutex<Option<String>>,
}

impl Core {
    pub fn new(config: Config) -> Result<Core> {
        let modules = modules::discover(&config);
        let games = library::load_all(&config, &modules);
        let mut index = Index::open_memory()?;
        index.rebuild(&games)?;
        let caches = load_source_caches(&modules);
        Ok(Core {
            config: RwLock::new(config),
            modules: RwLock::new(modules),
            games: RwLock::new(games),
            index: Mutex::new(index),
            source_libraries: Mutex::new(caches),
            source_logins: Mutex::new(BTreeMap::new()),
            logins_probed: Mutex::new(false),
            scope: std::sync::Mutex::new(None),
        })
    }

    /// The one entry point: config, library, cached source libraries, then the session left open by a dead launcher, if any.
    pub async fn open() -> Result<Core> {
        let config = Config::load()?;
        std::fs::create_dir_all(paths::games_dir())?;
        let core = Core::new(config)?;
        core.reconcile().await?;
        Ok(core)
    }

    async fn refresh_login(&self, m: &Module) {
        let user = match self.run_verb(m, "status", &[], None).await {
            Ok(ev) => ev.into_iter().find_map(|e| if let SourceEvent::LoggedIn { user } = e { Some(user) } else { None }),
            Err(_) => None,
        };
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

    // ----- library -----

    pub async fn reload_all(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let games = library::load_all(&config, &modules);
        self.index.lock().await.rebuild(&games)?;
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
            let r = library::resolve(Game::load(&toml_path)?, &config, &modules);
            self.index.lock().await.upsert(&r)?;
            games.push(r);
        } else {
            self.index.lock().await.remove(id)?;
        }
        library::sort_default(&mut games);
        Ok(())
    }

    pub async fn list_json(&self) -> String {
        let games = self.games.read().await;
        serde_json::Value::Array(games.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.to_json()).collect()).to_string()
    }

    pub async fn get(&self, id: &str) -> Result<Resolved> {
        let games = self.games.read().await;
        games.iter().find(|g| g.game.id == id).cloned().ok_or_else(|| Error::NotFound(id.into()))
    }

    pub async fn resolve(&self, query: &str) -> Vec<String> {
        let games = self.games.read().await;
        let direct: Vec<String> = library::resolve_query(&games, query).iter().map(|g| g.game.id.clone()).collect();
        if !direct.is_empty() {
            return direct;
        }
        self.index.lock().await.search(query).unwrap_or_default()
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
        if let Some(okey) = real_key.strip_prefix("launch.options.") {
            let spec = crate::runners::spec(&r.game.runner_id()).ok_or_else(|| Error::Invalid(format!("{id} has no known runner")))?;
            if !value.is_empty() {
                spec.validate_option(okey, &value)?;
            }
        }
        crate::game::set_key(&r.game.toml_path(), &real_key, &value)?;
        self.reload_game(id).await
    }

    pub async fn add_game(&self, json: &str) -> Result<String> {
        let v: serde_json::Value = serde_json::from_str(json)?;
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

    // ----- runners -----

    pub async fn runners_json(&self) -> String {
        let cfg = self.config.read().await.clone();
        serde_json::Value::Array(crate::runners::RUNNERS.iter().map(|s| crate::runners::to_json(s, &cfg)).collect()).to_string()
    }

    pub async fn set_runner_setting(&self, runner: &str, key: &str, value: &str) -> Result<()> {
        let spec = crate::runners::spec(runner).ok_or_else(|| Error::NotFound(format!("runner {runner}")))?;
        if !matches!(key, "exe" | "args") && !value.is_empty() {
            spec.validate_option(key, value)?;
        }
        Config::set_key(&paths::config_file(), &format!("runners.{}.{key}", spec.id), value)?;
        self.reload_config().await
    }

    /// Trashes the install folder; the game stays in the library with its hours, journal and
    /// recordings, marked not installed until a source installs it again.
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
        let st = std::process::Command::new("trash").arg(&dir).status();
        if !st.map(|s| s.success()).unwrap_or(false) {
            return Err(Error::Io(format!("trash {} failed", dir.display())));
        }
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
            if prefix.is_dir() {
                let st = std::process::Command::new("trash").arg(&prefix).status();
                if !st.map(|s| s.success()).unwrap_or(false) {
                    tracing::warn!("trash {} failed; left in place", prefix.display());
                }
            }
        }
        crate::game::set_key(&r.game.toml_path(), "hidden", "true")?;
        let mut g = Game::load(&r.game.toml_path())?;
        g.removed_at = chrono::Local::now().to_rfc3339();
        g.save()?;
        self.reload_game(id).await
    }

    pub async fn import_lutris(&self, apply: bool) -> Result<String> {
        let config = self.config.read().await.clone();
        let report = crate::lutris::import(&config, apply)?;
        if apply {
            self.reload_all().await?;
        }
        Ok(serde_json::to_string(&report)?)
    }

    // ----- sessions -----

    /// The running session: a marker whose unit systemd still reports as active.
    pub async fn current(&self) -> Option<Current> {
        let m = read_marker()?;
        if launcher::is_active(&m.current.unit).await {
            Some(m.current)
        } else {
            None
        }
    }

    pub async fn current_json(&self) -> String {
        match self.current().await {
            Some(c) => serde_json::to_string(&c).unwrap_or_default(),
            None => String::new(),
        }
    }

    /// A marker without an active unit is a session whose `session-end` never ran (crash, reboot): close it now.
    pub async fn reconcile(&self) -> Result<()> {
        let Some(m) = read_marker() else { return Ok(()) };
        if launcher::is_active(&m.current.unit).await {
            return Ok(());
        }
        tracing::warn!("session {} of {} was left open; closing it", m.current.session_id, m.current.id);
        self.session_end(&m.current.id, &m.current.session_id, None, None).await
    }

    fn hook_env_base(&self, r: &Resolved, cfg: &Config) -> HookEnv {
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
        env.set("UNIVERSE_RECORDINGS_ROOT", cfg.recordings_root().to_string_lossy().to_string());
        env.set("UNIVERSE_JOURNAL_ROOT", cfg.journal_root().to_string_lossy().to_string());
        env
    }

    fn module_env(&self, m: &Module, r: &Resolved, cfg: &Config, base: &HookEnv) -> HookEnv {
        let mut env = base.clone();
        env.set("MODULE_SETTINGS_JSON", serde_json::Value::Object(m.merged_settings(cfg, Some(&r.game))).to_string());
        env
    }

    async fn hook_modules(&self, r: &Resolved, hook: &str) -> Vec<Module> {
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
        match zbus::Connection::session().await {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!("session bus: {e}");
                None
            }
        }
    }

    /// Moves this process into `universe-launcher-<pid>.scope` under the user manager; every game launched
    /// afterwards is bound to it, so it goes down with the launcher. Idempotent: the name is remembered.
    pub async fn adopt_scope(&self) -> Result<String> {
        if let Some(name) = self.scope.lock().unwrap().clone() {
            return Ok(name);
        }
        let pid = std::process::id();
        let name = format!("universe-launcher-{pid}.scope");
        let conn = zbus::Connection::session().await.map_err(|e| Error::Unavailable(format!("session bus: {e}")))?;
        let proxy = zbus::Proxy::new(&conn, "org.freedesktop.systemd1", "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager").await.map_err(|e| Error::Unavailable(format!("systemd: {e}")))?;
        let props: Vec<(&str, zbus::zvariant::Value)> = vec![("PIDs", vec![pid].into()), ("Description", "Universe launcher".into())];
        let aux: Vec<(String, Vec<(String, zbus::zvariant::Value)>)> = vec![];
        match proxy.call::<_, _, zbus::zvariant::OwnedObjectPath>("StartTransientUnit", &(name.as_str(), "fail", props, aux)).await {
            Ok(_) => {}
            // The scope survives from an earlier core of this process; it only counts if we are in it.
            Err(e) if e.to_string().contains("UnitExists") && in_cgroup_of(&name) => {}
            Err(e) => return Err(Error::Unavailable(format!("StartTransientUnit({name}): {e}"))),
        }
        // The reply only queues the start job; the move into the scope's cgroup lands when it runs.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !in_cgroup_of(&name) {
            if std::time::Instant::now() > deadline {
                return Err(Error::Unavailable(format!("{name}: this process was not moved into it")));
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        *self.scope.lock().unwrap() = Some(name.clone());
        Ok(name)
    }

    /// Starts the game as a transient service and returns; systemd runs `universe session-end` when its cgroup empties.
    pub async fn launch(&self, id: &str, screen: &str) -> Result<String> {
        self.reconcile().await?;
        if let Some(c) = self.current().await {
            return Err(Error::Busy(format!("{} is running ({})", c.title, c.session_id)));
        }
        let r = self.get(id).await?;
        if !r.game.removed_at.is_empty() {
            return Err(Error::Unavailable(format!("{id} was removed")));
        }
        let cfg = self.config.read().await.clone();
        let started = chrono::Local::now();
        let session_id = sessions::session_id(started);
        let screen = crate::desktop::pick_screen(screen);
        let unit = format!("{}.service", launcher::unit_name(id, &session_id));

        let mut base = self.hook_env_base(&r, &cfg);
        base.set("SESSION_ID", session_id.clone());
        base.set("SESSION_UNIT", unit.clone());
        base.set("SESSION_SCREEN", screen.clone());
        base.set("SESSION_STARTED_AT", started.to_rfc3339());

        let mut extra_env = BTreeMap::new();
        let env_file = paths::state_home().join(format!("env-{session_id}"));
        std::fs::create_dir_all(paths::state_home())?;
        std::fs::write(&env_file, "")?;
        for m in self.hook_modules(&r, "pre-launch").await {
            let mut env = self.module_env(&m, &r, &cfg, &base);
            env.set("UNIVERSE_ENV_FILE", env_file.to_string_lossy().to_string());
            let out = modules::run_blocking(&m, "pre-launch", &env).await?;
            if out.status != 0 {
                let _ = std::fs::remove_file(&env_file);
                return Err(Error::Io(format!("pre-launch {} refused the launch: {}", m.id(), out.stderr.trim())));
            }
        }
        if let Ok(s) = std::fs::read_to_string(&env_file) {
            for line in s.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if !k.trim().is_empty() {
                        extra_env.insert(k.trim().to_string(), v.to_string());
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&env_file);

        let plan = launcher::plan(&r, &cfg, &session_id, &extra_env)?;
        launcher::run_shell(&plan.pre_command, &plan.env, &plan.cwd).await?;

        let inputplumber = r.effective.inputplumber && tokio::task::spawn_blocking(crate::inputplumber::engage).await.unwrap_or(false);

        let profile = crate::desktop::detect(&cfg);
        let mut cursor_was_active = false;
        if r.effective.hide_cursor {
            if let Some(conn) = self.shell_conn().await {
                cursor_was_active = crate::desktop::cursor_extension_enable(&conn, profile, &cfg.desktop.cursor_extension).await;
            }
        }
        let marker = Marker {
            current: Current { session_id: session_id.clone(), id: id.into(), title: r.game.title.clone(), unit: unit.clone(), screen: screen.clone(), started_at: started.to_rfc3339() },
            cursor_was_active,
            inputplumber,
            post_command: plan.post_command.clone(),
            cwd: plan.cwd.to_string_lossy().to_string(),
            env: plan.env.clone(),
            hook_env: base.vars.clone(),
        };
        if let Err(e) = write_marker(&marker) {
            if inputplumber {
                let _ = tokio::task::spawn_blocking(crate::inputplumber::release).await;
            }
            return Err(e);
        }
        tracing::info!("launch {id}: {}", plan.command_line());
        let stop_post = vec![paths::self_exe().to_string_lossy().to_string(), "session-end".into(), id.into(), session_id.clone()];
        let budget: u64 = 60 + self.hook_modules(&r, "session-end").await.iter().map(|m| m.timeout().as_secs()).sum::<u64>();
        let scope = self.scope.lock().unwrap().clone();
        if let Err(e) = launcher::spawn(&plan, &stop_post, &passthrough_env(), budget, scope.as_deref()).await {
            remove_marker();
            if inputplumber {
                let _ = tokio::task::spawn_blocking(crate::inputplumber::release).await;
            }
            if r.effective.hide_cursor {
                if let Some(conn) = self.shell_conn().await {
                    crate::desktop::cursor_extension_restore(&conn, profile, &cfg.desktop.cursor_extension, cursor_was_active).await;
                }
            }
            return Err(e);
        }
        for m in self.hook_modules(&r, "post-launch").await {
            let env = self.module_env(&m, &r, &cfg, &base);
            if let Err(e) = modules::run_async(&m, "post-launch", &env, &session_id, Some(&unit)) {
                tracing::warn!("post-launch {}: {e}", m.id());
            }
        }
        if cfg.controller.enabled {
            if let Err(e) = spawn_controller_watch(&session_id, &unit) {
                tracing::warn!("controller watch: {e}");
            }
        }
        Ok(session_id)
    }

    /// Closes a session: run by systemd's `ExecStopPost`, or by `reconcile` for one that was left open. Idempotent.
    pub async fn session_end(&self, id: &str, session_id: &str, exit: Option<i32>, ended: Option<chrono::DateTime<chrono::Local>>) -> Result<()> {
        let marker = read_marker().filter(|m| m.current.session_id == session_id);
        let r = match self.get(id).await {
            Ok(r) => r,
            Err(e) => {
                if marker.is_some() {
                    remove_marker();
                }
                return Err(e);
            }
        };
        if sessions::read(&r.game.sessions_path()).unwrap_or_default().iter().any(|s| s.session == session_id) {
            if marker.is_some() {
                remove_marker();
            }
            return Ok(());
        }
        let cfg = self.config.read().await.clone();
        let unit = marker.as_ref().map(|m| m.current.unit.clone()).unwrap_or_else(|| format!("{}.service", launcher::unit_name(id, session_id)));
        let log = launcher::unit_log(&unit).await;
        let started = marker
            .as_ref()
            .and_then(|m| chrono::DateTime::parse_from_rfc3339(&m.current.started_at).ok().map(|t| t.with_timezone(&chrono::Local)))
            .or(log.started)
            .or_else(|| sessions::parse_session_id(session_id))
            .unwrap_or_else(chrono::Local::now);
        let ended = ended.or(log.ended).unwrap_or_else(chrono::Local::now);
        let exit = exit.or(log.exit).unwrap_or(-1);
        let duration_s = (ended - started).num_seconds().max(0) as u64;
        let screen = marker.as_ref().map(|m| m.current.screen.clone()).unwrap_or_default();
        let session = Session {
            session: session_id.into(),
            game: id.into(),
            started_at: started.to_rfc3339(),
            ended_at: ended.to_rfc3339(),
            duration_s,
            source: "universe".into(),
            unit: unit.clone(),
            screen: screen.clone(),
            exit,
            recording: None,
        };
        sessions::append(&r.game.sessions_path(), &session)?;
        remove_marker();

        if let Some(m) = &marker {
            if m.inputplumber {
                let _ = tokio::task::spawn_blocking(crate::inputplumber::release).await;
            }
            if r.effective.hide_cursor {
                if let Some(conn) = self.shell_conn().await {
                    crate::desktop::cursor_extension_restore(&conn, crate::desktop::detect(&cfg), &cfg.desktop.cursor_extension, m.cursor_was_active).await;
                }
            }
            let _ = launcher::run_shell(&m.post_command, &m.env, Path::new(&m.cwd)).await;
        }
        let mut env_end = match &marker {
            Some(m) => HookEnv { vars: m.hook_env.clone() },
            None => {
                let mut e = self.hook_env_base(&r, &cfg);
                e.set("SESSION_ID", session_id);
                e.set("SESSION_UNIT", unit.clone());
                e.set("SESSION_SCREEN", screen);
                e.set("SESSION_STARTED_AT", started.to_rfc3339());
                e
            }
        };
        env_end.set("SESSION_ENDED_AT", ended.to_rfc3339());
        env_end.set("SESSION_DURATION_S", duration_s.to_string());
        for m in self.hook_modules(&r, "session-end").await {
            let env = self.module_env(&m, &r, &cfg, &env_end);
            if let Err(e) = modules::run_blocking(&m, "session-end", &env).await {
                tracing::warn!("session-end {}: {e}", m.id());
            }
        }
        self.reload_game(id).await?;
        self.post_process(id, session_id).await;
        Ok(())
    }

    /// post-process hooks, once the session-end hooks have filed the recording (or not).
    async fn post_process(&self, id: &str, session_id: &str) {
        let Ok(r) = self.get(id).await else { return };
        let cfg = self.config.read().await.clone();
        let sess = r.sessions.iter().find(|s| s.session == session_id).cloned();
        let mut env = self.hook_env_base(&r, &cfg);
        env.set("SESSION_ID", session_id);
        env.set("RECORDING_PATH", sess.as_ref().and_then(|s| s.recording.clone()).unwrap_or_default());
        if let Some(s) = sess {
            env.set("SESSION_STARTED_AT", s.started_at);
            env.set("SESSION_ENDED_AT", s.ended_at);
            env.set("SESSION_DURATION_S", s.duration_s.to_string());
            env.set("SESSION_SCREEN", s.screen);
        }
        for m in self.hook_modules(&r, "post-process").await {
            let menv = self.module_env(&m, &r, &cfg, &env);
            if let Err(e) = modules::run_async(&m, "post-process", &menv, session_id, None) {
                tracing::warn!("post-process {}: {e}", m.id());
            }
        }
    }

    pub async fn stop(&self, session_id: &str) -> Result<()> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        if !session_id.is_empty() && c.session_id != session_id {
            return Err(Error::NotFound(session_id.into()));
        }
        launcher::stop_unit(&c.unit).await
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

    pub async fn sessions_json(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        let mut s = r.sessions.clone();
        s.reverse();
        Ok(serde_json::to_string(&s)?)
    }

    // ----- recordings & journal -----

    async fn game_of_session(&self, session_id: &str) -> Option<String> {
        if let Some(m) = read_marker() {
            if m.current.session_id == session_id {
                return Some(m.current.id);
            }
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

    pub async fn recordings_json(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        Ok(serde_json::to_string(&crate::recording::list(&r.game)?)?)
    }

    pub async fn add_entry(&self, session_id: &str, json: &str) -> Result<()> {
        let mut entry: crate::journal::Entry = serde_json::from_str(json)?;
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
    pub async fn journal_json(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        Ok(serde_json::to_string(&crate::journal::load(&r.game.journal_dir(), &r.sessions))?)
    }

    pub async fn pending_journals_json(&self) -> String {
        let games = self.games.read().await;
        let mut out = Vec::new();
        for r in games.iter() {
            for e in crate::journal::read_all(&r.game.journal_dir()).unwrap_or_default().into_iter().filter(|e| e.state == "pending") {
                out.push(serde_json::json!({"game": r.game.id, "title": r.game.title, "session": e.session, "started_at": e.started_at}));
            }
        }
        serde_json::Value::Array(out).to_string()
    }

    pub async fn render_journal(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let note_dir = cfg.journal_root().join(id);
        let journal_dir = r.game.journal_dir();
        let sessions = crate::journal::sessions_for_note(&r.sessions, &journal_dir);
        let path = crate::journal::write_note(&r.game.title, &r.journal, &sessions, &journal_dir, &note_dir, &crate::journal::Locale::from_env())?;
        Ok(path.to_string_lossy().into())
    }

    // ----- modules -----

    pub async fn modules_json(&self) -> String {
        serde_json::Value::Array(self.modules.read().await.iter().map(|m| m.to_json()).collect()).to_string()
    }

    pub async fn reload_modules(&self) {
        let cfg = self.config.read().await.clone();
        *self.modules.write().await = modules::discover(&cfg);
    }

    pub async fn enable_module(&self, id: &str, enabled: bool) -> Result<()> {
        let mut cfg = self.config.write().await;
        let known = self.modules.read().await.iter().any(|m| m.id() == id);
        if !known {
            return Err(Error::NotFound(format!("module {id}")));
        }
        cfg.modules.enabled.retain(|m| m != id);
        if enabled {
            cfg.modules.enabled.push(id.into());
        }
        let list = cfg.modules.enabled.join(",");
        drop(cfg);
        Config::set_key(&paths::config_file(), "modules.enabled", &list)?;
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

    pub async fn module_settings_json(&self, module: &str, game_id: &str) -> Result<String> {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await;
        let m = modules.iter().find(|m| m.id() == module).ok_or_else(|| Error::NotFound(format!("module {module}")))?;
        let game = if game_id.is_empty() { None } else { Some(self.get(game_id).await?.game) };
        Ok(serde_json::Value::Object(m.merged_settings(&cfg, game.as_ref())).to_string())
    }

    /// The choices of one global setting, listed live by the module when it declares `choices_exec`.
    pub async fn module_setting_choices(&self, module: &str, key: &str) -> Result<String> {
        let cfg = self.config.read().await.clone();
        let m = self.modules.read().await.iter().find(|m| m.id() == module).cloned().ok_or_else(|| Error::NotFound(format!("module {module}")))?;
        let settings = m.merged_settings(&cfg, None);
        let list = modules::setting_choices(&m, &settings, key).await?;
        Ok(serde_json::to_string(&list)?)
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

    pub async fn settings_json(&self) -> String {
        self.config.read().await.to_json().to_string()
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let value = if key == "controller.volume_step" { crate::controller::volume_step_value(value)? } else { value.to_string() };
        Config::set_key(&paths::config_file(), key, &value)?;
        self.reload_config().await
    }

    // ----- controller -----

    pub async fn controller_state_json(&self) -> String {
        crate::controller::state_json(&self.config.read().await.controller).to_string()
    }

    /// The pads readable right now, as `watch` would announce them; for a frontend whose watcher waits on the lock.
    pub async fn controller_pads_json(&self) -> String {
        let cfg = self.config.read().await.controller.clone();
        serde_json::Value::Array(crate::controller::watch::enumerate_json(&cfg)).to_string()
    }

    /// Adds or replaces the macro with the same family, button and trigger.
    pub async fn set_controller_macro(&self, json: &str) -> Result<()> {
        let m: crate::controller::Macro = serde_json::from_str(json)?;
        m.validate()?;
        let cfg = self.config.read().await.clone();
        let mut list = cfg.controller.macros();
        crate::controller::upsert_macro(&mut list, m);
        crate::controller::write_macros(&list)?;
        self.reload_config().await
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
        self.reload_config().await
    }

    /// The codes a slot answers to, learned by hand; `[]` leaves it unbound, `null` restores the seeds.
    pub async fn set_controller_button(&self, family: &str, slot: &str, codes_json: &str) -> Result<()> {
        let f = crate::controller::family_by_id(family).ok_or_else(|| Error::NotFound(format!("family {family}")))?;
        if !f.slots.iter().any(|s| s.id == slot) {
            return Err(Error::NotFound(format!("{} has no button {slot}", f.name)));
        }
        let codes: Option<Vec<String>> = serde_json::from_str(codes_json)?;
        if let Some(list) = &codes {
            for c in list {
                crate::controller::keys::parse_source(c).ok_or_else(|| Error::Invalid(format!("unknown code {c}")))?;
            }
        }
        crate::controller::write_button(family, slot, codes.as_deref())?;
        self.reload_config().await
    }

    pub async fn doctor_json(&self) -> String {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let conn = if crate::desktop::detect(&cfg) == crate::desktop::Profile::Gnome { self.shell_conn().await } else { None };
        let runners: Vec<String> = self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.effective.runner.clone()).collect();
        serde_json::to_string(&crate::doctor::run(&cfg, &modules, conn.as_ref(), &runners).await).unwrap_or_default()
    }

    // ----- sources -----

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

    pub async fn sources_json(&self) -> String {
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
        serde_json::Value::Array(list).to_string()
    }

    /// Runs a verb to completion, collecting events; `progress` events are forwarded as they arrive.
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

    /// Returns the user name reported by the source.
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

    pub async fn source_library(&self, source: &str, refresh: bool) -> Result<String> {
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
        Ok(serde_json::Value::Array(list).to_string())
    }

    pub async fn source_search(&self, source: &str, query: &str) -> Result<String> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "search", &[query.to_string()], None).await?;
        Ok(serde_json::to_string(&Self::game_events(&events))?)
    }

    pub async fn source_info(&self, source: &str, game_id: &str) -> Result<String> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "info", &[game_id.to_string()], None).await?;
        let data = events.iter().find_map(|e| if let SourceEvent::Info { data } = e { Some(data.clone()) } else { None }).unwrap_or(serde_json::Value::Null);
        Ok(data.to_string())
    }

    /// Applies a source `game` event to the library: create when owned, update source fields otherwise (plan §3).
    async fn apply_source_game(&self, source: &str, g: &serde_json::Map<String, serde_json::Value>, create: bool) -> Result<Option<String>> {
        let sid = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let title = g.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let dir = g.get("dir").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let exe = g.get("exe").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let build = g.get("build").and_then(|v| v.as_str()).unwrap_or("").to_string();
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

    /// Installs a title; returns its library id.
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

    pub async fn source_updates(&self) -> Result<String> {
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
        Ok(serde_json::Value::Array(out).to_string())
    }

    /// Updates one title, or every pending one when `game_id` is empty; returns how many were updated.
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

    // ----- media -----

    /// Refreshes the artwork of one game, or of the whole library when `id` is empty; returns (changed, total).
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
            match tokio::task::spawn_blocking(move || crate::media::refresh(&cfg, &game, force)).await {
                Ok(Ok(true)) => {
                    changed += 1;
                    let _ = self.reload_game(gid).await;
                }
                Ok(Ok(false)) => {}
                Ok(Err(e)) => tracing::warn!("media {gid}: {e}"),
                Err(e) => tracing::warn!("media {gid}: {e}"),
            }
        }
        Ok((changed, total))
    }

    /// Copies a local image over a slot as its override; returns where it landed.
    pub async fn media_set_slot(&self, id: &str, slot: &str, path: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let placed = crate::media::set_slot(&cfg, &r.game, slot, Path::new(path))?;
        self.reload_game(id).await?;
        Ok(placed.to_string_lossy().into())
    }

    /// Downloads a candidate's URL over a slot as its override; returns where it landed.
    pub async fn media_set_url(&self, id: &str, slot: &str, url: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let (slot, url) = (slot.to_string(), url.to_string());
        let placed = tokio::task::spawn_blocking(move || crate::media::set_slot_url(&cfg, &game, &slot, &url)).await.map_err(|e| Error::Io(e.to_string()))??;
        self.reload_game(id).await?;
        Ok(placed.to_string_lossy().into())
    }

    /// Removes a slot's override; returns whether there was one.
    pub async fn media_unset(&self, id: &str, slot: &str) -> Result<bool> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let gone = crate::media::unset(&cfg, &r.game, slot)?;
        self.reload_game(id).await?;
        Ok(gone)
    }

    /// One page of a slot's candidates: `{items: [{provider, id, url, thumb, score, slot}], page, more}`.
    pub async fn media_candidates(&self, id: &str, slot: &str, page: u32) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let slot = slot.to_string();
        let list = tokio::task::spawn_blocking(move || crate::media::candidates(&cfg, &game, &slot, page)).await.map_err(|e| Error::Io(e.to_string()))??;
        Ok(serde_json::to_string(&list)?)
    }

    /// The provider's games matching `query` (the title when empty): `[{provider, id, name, year, verified, current}]`.
    pub async fn media_search(&self, id: &str, query: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let query = query.to_string();
        let hits = tokio::task::spawn_blocking(move || crate::media::search(&cfg, &game, &query)).await.map_err(|e| Error::Io(e.to_string()))??;
        Ok(serde_json::to_string(&hits)?)
    }

    /// Every slot of one game, or of every game when `id` is empty: `[{id, title, sgdb_id, slots: [{slot, path,
    /// default, override, origin, kind}], screenshots: {count, override_count, origin, kind}}]`.
    pub async fn media_status(&self, id: &str) -> Result<String> {
        let cfg = self.config.read().await.clone();
        let games: Vec<crate::game::Game> = if id.is_empty() {
            self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.game.clone()).collect()
        } else {
            vec![self.get(id).await?.game]
        };
        let list: Vec<crate::media::MediaStatus> = games.iter().map(|g| crate::media::status(&cfg, g)).collect();
        Ok(serde_json::to_string(&list)?)
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

    // Needs a user systemd and a session bus: `cargo test -- --ignored adopt_scope`.
    #[tokio::test]
    #[ignore]
    async fn adopt_scope_moves_the_process_and_is_idempotent() {
        let core = Core::new(Config::default()).unwrap();
        let name = core.adopt_scope().await.unwrap();
        assert_eq!(name, format!("universe-launcher-{}.scope", std::process::id()));
        assert_eq!(core.adopt_scope().await.unwrap(), name);
        assert!(in_cgroup_of(&name), "{}", std::fs::read_to_string("/proc/self/cgroup").unwrap());
        let out = std::process::Command::new("systemctl").args(["--user", "is-active", &name]).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "active");
    }

    #[test]
    fn titles_from_files() {
        assert_eq!(title_of(Path::new("/g/F-Zero GX.iso")), "F-Zero GX");
        assert_eq!(title_of(Path::new("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci")), "SUPER MARIO ODYSSEY Eur SuperXCi - CLC");
        assert_eq!(title_of(Path::new("/r/Pokemon - HeartGold Version (USA) [rev 1].nds")), "Pokemon - HeartGold Version");
        assert_eq!(title_of(Path::new("/r/mario_kart_wii.wbfs")), "mario kart wii");
    }
}
