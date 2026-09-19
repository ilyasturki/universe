use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::game::Game;
use crate::host::Host;
use crate::library::{self, Resolved};
use crate::modules::{self, HookEnv, Module};
use crate::sources::{self, Source, SourceEvent};
use crate::paths;
use crate::{Error, Result};

// gamescope encodes its screenshot png on one thread: a launcher frame takes ~450 ms at 4K, a game's more.
const FRAME_WAIT: std::time::Duration = std::time::Duration::from_millis(2000);

/// Two lifetimes: a caller reborrows the same callback across several awaited calls.
pub type Progress<'a, 'b> = &'a mut (dyn FnMut(u64, u64, &str) + 'b);

fn trash(path: &Path) -> Result<()> {
    let st = std::process::Command::new("trash").arg(path).status();
    if !st.map(|s| s.success()).unwrap_or(false) {
        return Err(Error::Io(format!("trash {} failed", path.display())));
    }
    Ok(())
}

fn title_of(path: &Path) -> String {
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
    // The GAMESCOPE_*, STEAM_GAME_DISPLAY_0 and SDL_* names are what gamescope exports to its child: a game started from inside it lands on its display.
    for k in ["PATH", "HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS", "WAYLAND_DISPLAY", "DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE", "GI_TYPELIB_PATH", "UNIVERSE_DATA_HOME", "UNIVERSE_CONFIG_HOME", "UNIVERSE_STATE_HOME", "UNIVERSE_MODULES_PATH", "UNIVERSE_SOURCES_PATH", "RUST_LOG", "GAMESCOPE_WAYLAND_DISPLAY", "STEAM_GAME_DISPLAY_0", "SDL_VIDEODRIVER", "SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS", "vk_xwayland_wait_ready"] {
        if let Ok(v) = std::env::var(k) {
            env.insert(k.to_string(), v);
        }
    }
    env.insert("UNIVERSE_BIN".into(), paths::self_exe().to_string_lossy().to_string());
    env
}

/// The media providers block on the network; a worker thread keeps the runtime free.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Io(e.to_string()))?
}

fn load_source_caches(sources: &[Source]) -> BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>> {
    let mut caches = BTreeMap::new();
    for m in sources {
        if let Some(v) = std::fs::read_to_string(m.data_dir().join("library.json")).ok().and_then(|s| serde_json::from_str(&s).ok()) {
            caches.insert(m.id().to_string(), v);
        }
    }
    caches
}

pub struct Core {
    pub config: RwLock<Config>,
    pub modules: RwLock<Vec<Module>>,
    pub sources: RwLock<Vec<Source>>,
    pub games: RwLock<Vec<Resolved>>,
    source_libraries: Mutex<BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>>>,
    source_logins: Mutex<BTreeMap<String, String>>,
    /// `<source>:<game_id>` → the pid of the source process installing or updating it, while it runs.
    source_jobs: std::sync::Mutex<BTreeMap<String, u32>>,
    logins_probed: tokio::sync::OnceCell<()>,
    pub(crate) scope: std::sync::OnceLock<String>,
    nest: std::sync::OnceLock<Option<crate::nest::Nest>>,
    /// A freeze and the thaw behind it run in order: the hooks behind them toggle state.
    pub(crate) freezes: tokio::sync::Mutex<()>,
    pub(crate) host: Host,
}

impl Core {
    fn new(config: Config, host: Host) -> Core {
        let modules = modules::discover(&config);
        let sources = sources::discover(&config);
        let games = library::load_all(&config, &modules);
        let caches = load_source_caches(&sources);
        Core {
            config: RwLock::new(config),
            modules: RwLock::new(modules),
            sources: RwLock::new(sources),
            games: RwLock::new(games),
            source_libraries: Mutex::new(caches),
            source_logins: Mutex::new(BTreeMap::new()),
            source_jobs: std::sync::Mutex::new(BTreeMap::new()),
            logins_probed: tokio::sync::OnceCell::new(),
            scope: std::sync::OnceLock::new(),
            nest: std::sync::OnceLock::new(),
            freezes: tokio::sync::Mutex::new(()),
            host,
        }
    }

    pub async fn open() -> Result<Core> {
        let config = Config::load()?;
        let host = Host::live(&config);
        Core::open_with(config, host).await
    }

    pub async fn open_with(config: Config, host: Host) -> Result<Core> {
        std::fs::create_dir_all(paths::games_dir())?;
        let core = Core::new(config, host);
        core.reconcile().await?;
        let moved: usize = core.games.read().await.iter().map(|r| crate::screenshots::migrate_dirs(&r.game.journal_dir().join("attachments"), &r.game.screenshots_dir())).sum();
        if moved > 0 {
            tracing::info!("{moved} screenshot(s) moved out of journal/attachments");
        }
        Ok(core)
    }

    async fn refresh_login(&self, m: &Source) {
        let user = self.run_verb(m, "status", &[], None).await.ok().and_then(|ev| ev.into_iter().find_map(|e| if let SourceEvent::LoggedIn { user } = e { Some(user) } else { None }));
        let mut logins = self.source_logins.lock().await;
        match user {
            Some(u) => logins.insert(m.id().to_string(), u),
            None => logins.remove(m.id()),
        };
    }

    /// Login probes reach the network: once per process, on the first call that reports them.
    async fn ensure_logins(&self) {
        self.logins_probed
            .get_or_init(|| async {
                let sources: Vec<Source> = self.sources.read().await.iter().filter(|m| m.active()).cloned().collect();
                for m in sources {
                    self.refresh_login(&m).await;
                }
            })
            .await;
    }

    pub async fn reload_all(&self) {
        let config = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        *self.games.write().await = library::load_all(&config, &modules);
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
        let real_key = if modules.iter().any(|m| m.id() == top) { format!("modules.{key}") } else { key.to_string() };
        if let Some(rest) = real_key.strip_prefix("modules.") {
            let (mid, skey) = rest.split_once('.').ok_or_else(|| Error::Invalid(format!("bad key {key}")))?;
            let m = modules.iter().find(|m| m.id() == mid).ok_or_else(|| Error::NotFound(format!("module {mid}")))?;
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
        let config = self.config.read().await.clone();
        let home = paths::home();
        let shared = self.games.read().await.iter().any(|x| x.game.id != id && !x.game.source.dir.is_empty() && paths::expand(&x.game.source.dir).starts_with(&dir));
        if dir.parent().is_none() || dir == home || dir == config.games_root() || home.starts_with(&dir) || shared {
            return Err(Error::Invalid(format!("refusing to trash {}", dir.display())));
        }
        trash(&dir)?;
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
            self.reload_all().await;
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
        env.set("SCREENSHOTS_DIR", r.game.screenshots_dir().to_string_lossy().to_string());
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
            .filter(|m| m.active() && m.hook(hook).is_some())
            .filter(|m| m.merged_settings(&cfg, Some(&r.game)).get("enabled").and_then(|v| v.as_bool()).unwrap_or(true))
            .cloned()
            .collect()
    }

    pub fn nest(&self) -> Option<&crate::nest::Nest> {
        self.nest.get_or_init(|| crate::nest::Nest::open().inspect_err(|e| tracing::debug!("nest: {e}")).ok()).as_ref()
    }

    fn nest_or(&self) -> Result<&crate::nest::Nest> {
        self.nest().ok_or_else(|| Error::Unavailable("not inside gamescope".into()))
    }

    /// The running game's window, as the shell sees it: `None` before it maps; `Unavailable` off GNOME.
    pub async fn session_window(&self) -> Result<Option<crate::desktop::Toplevel>> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        let windows = crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        if c.gamescope_pid != 0 {
            return Ok(crate::desktop::pick_window(&windows, |pid| pid == i64::from(c.gamescope_pid)));
        }
        let Some(cg) = self.host.units.cgroup(&c.unit).await else { return Ok(None) };
        Ok(crate::desktop::pick_window(&windows, |pid| crate::desktop::pid_in_cgroup(pid, &cg)))
    }

    /// Waits for the session's window, then focuses it; `None` once the session ended or `timeout` ran out.
    pub async fn wait_session_window(&self, session_id: &str, timeout: std::time::Duration) -> Result<Option<crate::desktop::Toplevel>> {
        if !self.current().await.is_some_and(|c| c.gamescope_pid != 0) {
            crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        }
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let Some(c) = self.current().await else { return Ok(None) };
            if c.session_id != session_id {
                return Ok(None);
            }
            // On the launcher's gamescope the shell's toplevel is the gamescope's; without the extension a stand-in carries its pid.
            let window = if c.gamescope_pid == 0 {
                self.session_window().await?
            } else if self.nest_or()?.game_shown(c.launcher_pid)? {
                match self.session_window().await {
                    Err(Error::Unavailable(_)) => Some(crate::desktop::Toplevel { pid: i64::from(c.gamescope_pid), focused: true, ..Default::default() }),
                    w => w?,
                }
            } else {
                None
            };
            if let Some(w) = window {
                if w.id != 0 {
                    if let Err(e) = crate::desktop::activate_window(w.id).await {
                        tracing::warn!("activate {}: {e}", w.id);
                    }
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
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        if c.gamescope_pid != 0 {
            return self.nest_or()?.show(c.launcher_pid, true);
        }
        let w = self.session_window().await?.ok_or_else(|| Error::NotFound("the game has no window yet".into()))?;
        match crate::desktop::activate_window(w.id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::NotFound("the game's window is gone".into())),
            Err(e) => Err(Error::Unavailable(e)),
        }
    }

    pub async fn focus_pid(&self, pid: u32) -> Result<()> {
        if pid == std::process::id() {
            if let Some(n) = self.nest() {
                return n.show(self.launcher_pid(), false);
            }
        }
        let windows = crate::desktop::list_windows().await.map_err(Error::Unavailable)?;
        let w = windows.iter().filter(|w| w.pid == pid as i64 && !w.hidden).max_by_key(|w| w.width * w.height).ok_or_else(|| Error::NotFound(format!("no window of pid {pid}")))?;
        crate::desktop::activate_window(w.id).await.map_err(Error::Unavailable)?;
        Ok(())
    }

    pub fn nest_game_shown(&self) -> Result<bool> {
        self.nest_or()?.game_shown(self.launcher_pid())
    }

    /// The launcher the running game was started from, as the marker says; this process before a launch.
    fn launcher_pid(&self) -> u32 {
        crate::session::read_marker().map(|m| m.current.launcher_pid).filter(|p| *p != 0).unwrap_or_else(std::process::id)
    }

    pub fn nest_overlay(&self, window: u32, input: bool, opacity: u32) -> Result<()> {
        let n = self.nest_or()?;
        n.set_card(window, "STEAM_OVERLAY", 1)?;
        n.set_card(window, "STEAM_INPUT_FOCUS", u32::from(input))?;
        n.set_card(window, "_NET_WM_WINDOW_OPACITY", opacity)
    }

    /// The game as gamescope last painted it, without the overlay: `state/frame.png`, or `None` when nothing was painted in time.
    pub fn nest_frame(&self) -> Result<Option<String>> {
        let started = std::time::Instant::now();
        let shot = self.nest_or()?.frame(&paths::state_home().join("frame.png"), FRAME_WAIT)?;
        match &shot {
            Some(_) => tracing::debug!("nest: frame in {} ms", started.elapsed().as_millis()),
            None => tracing::warn!("nest: no frame within {} ms", FRAME_WAIT.as_millis()),
        }
        Ok(shot.map(|p| p.to_string_lossy().to_string()))
    }

    async fn mangoapp_draws(&self, c: &crate::session::Current, r: &Resolved) -> bool {
        c.gamescope_pid != 0 || (r.effective.gamescope && crate::runners::on_path(&self.config.read().await.launch.gamescope_bin).is_some())
    }

    async fn write_layer_conf(&self, c: &crate::session::Current, r: &Resolved) -> Result<()> {
        let hz = crate::launcher::fps_limit_hz(&r.effective, crate::desktop::screen_mode(&c.screen).await);
        let mangoapp = self.mangoapp_draws(c, r).await;
        Ok(std::fs::write(crate::launcher::layer_conf_path(), crate::launcher::layer_conf_text((!mangoapp).then_some(c.id.as_str()), hz, mangoapp || !r.effective.mangohud))?)
    }

    pub async fn set_fps_limit(&self) -> Result<String> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        self.write_layer_conf(&c, &self.get(&c.id).await?).await?;
        Ok(crate::launcher::RELOAD_CFG.into())
    }

    /// The SysV queue outlives mangoapp: told with none running, the next to start obeys.
    pub(crate) fn apply_mangoapp(&self, shown: bool, tell: bool) -> Result<()> {
        std::fs::write(crate::launcher::mangoapp_conf_path(), crate::launcher::mangoapp_conf_text(shown))?;
        if tell {
            if let Err(e) = crate::mangoapp::set_shown(shown) {
                tracing::debug!("mangoapp: {e}");
            }
        }
        Ok(())
    }

    /// `None` flips it; returns the new state.
    pub async fn set_mangohud(&self, on: Option<bool>) -> Result<bool> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        // The dock and the watcher each hold a library: the other may have written the key since this one loaded.
        self.reload_game(&c.id).await?;
        let was = self.get(&c.id).await?.effective.mangohud;
        let on = on.unwrap_or(!was);
        self.set(&c.id, "launch.mangohud", if on { "true" } else { "false" }).await?;
        let r = self.get(&c.id).await?;
        if self.mangoapp_draws(&c, &r).await {
            self.apply_mangoapp(on, true)?;
        } else {
            self.write_layer_conf(&c, &r).await?;
            if on != was {
                if let Err(e) = crate::mangoapp::layer_toggle(&c.id) {
                    tracing::debug!("mangohud layer: {e}");
                }
            }
        }
        Ok(on)
    }

    pub fn nest_filter(&self, filter: &str, sharpness: Option<u32>) -> Result<()> {
        self.nest_or()?.set_filter(filter, sharpness)
    }

    pub async fn volume(&self, change: &str, value: u8) -> Result<serde_json::Value> {
        use crate::controller::volume::Change;
        let change = match change {
            "up" => Change::Up,
            "down" => Change::Down,
            "mute" => Change::ToggleMute,
            "set" => Change::Set(value),
            "get" => Change::Get,
            other => return Err(Error::Invalid(format!("volume: up, down, mute, set or get, not '{other}'"))),
        };
        let step = self.config.read().await.controller.volume_step;
        let level = crate::controller::volume::change(change, step).await.map_err(Error::Unavailable)?;
        Ok(serde_json::json!({"percent": level.percent, "muted": level.muted, "output": level.output}))
    }

    pub async fn host_gamescope(&self, screen: &str) -> Option<(String, Vec<String>)> {
        let cfg = self.config.read().await.clone();
        let screen = crate::desktop::pick_screen(screen);
        let mode = crate::desktop::screen_mode(&screen).await;
        let command = crate::launcher::host_gamescope(&cfg, mode)?;
        let _ = std::fs::create_dir_all(paths::state_home());
        if let Err(e) = std::fs::write(crate::launcher::mangoapp_conf_path(), crate::launcher::mangoapp_conf_text(false)) {
            tracing::warn!("mangoapp.conf: {e}");
        }
        Some(command)
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
                env.set("SCREENSHOTS_DIR", paths::state_home().join("screenshots").to_string_lossy().to_string());
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

    /// The player's own shots, newest first; an empty `id` lists every visible game's. Read from disk on every call.
    pub async fn screenshots(&self, id: &str) -> Result<Vec<crate::screenshots::Shot>> {
        let games = self.games.read().await;
        let picked: Vec<&Resolved> = if id.is_empty() {
            games.iter().filter(|g| g.game.removed_at.is_empty() && !g.game.hidden).collect()
        } else {
            vec![games.iter().find(|g| g.game.id == id).ok_or_else(|| Error::NotFound(id.into()))?]
        };
        let mut shots: Vec<crate::screenshots::Shot> = picked.iter().flat_map(|r| crate::screenshots::list(r)).collect();
        shots.sort_by(|a, b| b.path.rsplit('/').next().cmp(&a.path.rsplit('/').next()));
        Ok(shots)
    }

    pub async fn remove_screenshot(&self, id: &str, name: &str) -> Result<()> {
        let r = self.get(id).await?;
        if !crate::screenshots::is_shot_name(name) {
            return Err(Error::Invalid(format!("not a screenshot name: {name}")));
        }
        let path = r.game.screenshots_dir().join(name);
        if !path.is_file() {
            return Err(Error::NotFound(path.to_string_lossy().into()));
        }
        trash(&path)?;
        let journal_dir = r.game.journal_dir();
        let mut named = false;
        for mut entry in crate::journal::read_all(&journal_dir).unwrap_or_default() {
            if entry.images.iter().any(|i| i.rsplit('/').next() == Some(name)) {
                entry.images.retain(|i| i.rsplit('/').next() != Some(name));
                crate::journal::write(&journal_dir, &entry)?;
                named = true;
            }
        }
        if named {
            let cfg = self.config.read().await.clone();
            let note_dir = cfg.journal_root().join(id);
            let mirrored = note_dir.join(name);
            if mirrored.is_file() {
                trash(&mirrored)?;
            }
            if note_dir.is_dir() {
                self.render_journal(id).await?;
            }
        }
        Ok(())
    }

    /// Newest first; an empty `id` lists every visible game. The entry state is read from disk on every call, as `journal` is.
    pub async fn sessions(&self, id: &str) -> Result<Vec<crate::sessions::SessionRow>> {
        let games = self.games.read().await;
        let picked: Vec<&Resolved> = if id.is_empty() {
            games.iter().filter(|g| g.game.removed_at.is_empty() && !g.game.hidden).collect()
        } else {
            vec![games.iter().find(|g| g.game.id == id).ok_or_else(|| Error::NotFound(id.into()))?]
        };
        let mut rows = Vec::new();
        for r in picked {
            let entries = crate::journal::read_all(&r.game.journal_dir()).unwrap_or_default();
            rows.extend(r.sessions.iter().rev().map(|s| crate::sessions::SessionRow::new(s, &r.game.title, entries.iter().find(|e| e.session == s.session))));
        }
        rows.sort_by(|a, b| b.session.ended_at.cmp(&a.session.ended_at));
        Ok(rows)
    }

    async fn game_of_session(&self, session_id: &str) -> Option<String> {
        if let Some(id) = crate::session::read_marker().filter(|m| m.current.session_id == session_id).map(|m| m.current.id) {
            return Some(id);
        }
        let games = self.games.read().await;
        games.iter().find(|g| g.sessions.iter().any(|s| s.session == session_id)).map(|g| g.game.id.clone())
    }

    pub async fn file_recording(&self, session_id: &str, path: &str, timeline: Option<&crate::recording::Timeline>) -> Result<String> {
        let id = self.game_of_session(session_id).await.ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
        let r = self.get(&id).await?;
        let cfg = self.config.read().await.clone();
        let dest = crate::recording::file(&r.game, session_id, Path::new(path), &cfg.recordings_root(), timeline)?;
        self.reload_game(&id).await?;
        Ok(dest.to_string_lossy().to_string())
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

    /// Read from disk on every call, a pending entry's timeout judged now; the images come back absolute.
    pub async fn journal(&self, id: &str) -> Result<Vec<crate::journal::Entry>> {
        let r = self.get(id).await?;
        let journal_dir = r.game.journal_dir();
        let mut entries = crate::journal::load(&journal_dir, &r.sessions);
        for img in entries.iter_mut().flat_map(|e| e.images.iter_mut()) {
            *img = crate::journal::image_path(&journal_dir, &r.game.screenshots_dir(), img).to_string_lossy().into_owned();
        }
        Ok(entries)
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
        // The frames go with the entry; the player's own shots are theirs, not the entry's.
        for rel in entry.images.iter().filter(|rel| !crate::screenshots::is_shot_name(rel.rsplit('/').next().unwrap_or(rel))) {
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
        let path = crate::journal::write_note(&r.game.title, &entries, &sessions, &journal_dir, &r.game.screenshots_dir(), &note_dir, &crate::journal::Locale::from_env())?;
        Ok(path.to_string_lossy().into())
    }

    pub async fn modules(&self) -> Vec<serde_json::Value> {
        self.modules.read().await.iter().map(|m| m.to_json()).collect()
    }

    pub async fn reload_modules(&self) {
        let cfg = self.config.read().await.clone();
        *self.modules.write().await = modules::discover(&cfg);
        *self.sources.write().await = sources::discover(&cfg);
    }

    async fn module_or_hint(&self, id: &str) -> Result<Module> {
        if let Some(m) = self.modules.read().await.iter().find(|m| m.id() == id) {
            return Ok(m.clone());
        }
        if self.sources.read().await.iter().any(|s| s.id() == id) {
            return Err(Error::Invalid(format!("{id} is a source, not a module: universe source … {id}")));
        }
        Err(Error::NotFound(format!("module {id}")))
    }

    async fn source_or_hint(&self, id: &str) -> Result<Source> {
        if let Some(s) = self.sources.read().await.iter().find(|s| s.id() == id) {
            return Ok(s.clone());
        }
        if self.modules.read().await.iter().any(|m| m.id() == id) {
            return Err(Error::Invalid(format!("{id} is a module, not a source: universe module … {id}")));
        }
        Err(Error::NotFound(format!("source {id}")))
    }

    /// An empty list is written as `[]`: an absent key would fall back to the defaults, which enable it again.
    fn enabled_list(mut list: Vec<String>, id: &str, enabled: bool) -> String {
        list.retain(|m| m != id);
        if enabled {
            list.push(id.into());
        }
        if list.is_empty() { "[]".into() } else { list.join(",") }
    }

    pub async fn enable_module(&self, id: &str, enabled: bool) -> Result<()> {
        self.module_or_hint(id).await?;
        let list = self.config.read().await.modules.enabled.clone();
        Config::set_key(&paths::config_file(), "modules.enabled", &Self::enabled_list(list, id, enabled))?;
        self.reload_config().await
    }

    pub async fn enable_source(&self, id: &str, enabled: bool) -> Result<()> {
        self.source_or_hint(id).await?;
        let list = self.config.read().await.sources.enabled.clone();
        Config::set_key(&paths::config_file(), "sources.enabled", &Self::enabled_list(list, id, enabled))?;
        self.reload_config().await
    }

    pub async fn reload_config(&self) -> Result<()> {
        self.reload_settings().await?;
        self.reload_all().await;
        Ok(())
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
        let m = self.module_or_hint(module).await?;
        let game = if game_id.is_empty() { None } else { Some(self.get(game_id).await?.game) };
        Ok(serde_json::Value::Object(m.merged_settings(&cfg, game.as_ref())))
    }

    pub async fn module_setting_choices(&self, module: &str, key: &str) -> Result<Vec<String>> {
        let cfg = self.config.read().await.clone();
        let m = self.module_or_hint(module).await?;
        let settings = m.merged_settings(&cfg, None);
        modules::setting_choices(&m, &settings, key).await
    }

    pub async fn set_module_setting(&self, module: &str, game_id: &str, key: &str, value: &str) -> Result<()> {
        let m = self.module_or_hint(module).await?;
        if !value.is_empty() {
            m.validate_setting(key, value, !game_id.is_empty())?;
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

    pub async fn source_settings(&self, source: &str) -> Result<serde_json::Value> {
        let cfg = self.config.read().await.clone();
        Ok(serde_json::Value::Object(self.source_or_hint(source).await?.merged_settings(&cfg)))
    }

    pub async fn source_setting_choices(&self, source: &str, key: &str) -> Result<Vec<String>> {
        let cfg = self.config.read().await.clone();
        let m = self.source_or_hint(source).await?;
        let settings = m.merged_settings(&cfg);
        sources::setting_choices(&m, &settings, key).await
    }

    pub async fn set_source_setting(&self, source: &str, key: &str, value: &str) -> Result<()> {
        let m = self.source_or_hint(source).await?;
        if !value.is_empty() {
            m.validate_setting(key, value)?;
        }
        Config::set_key(&paths::config_file(), &format!("sources.{source}.{key}"), value)?;
        self.reload_config().await
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

    pub fn launch_keys(&self, scope: &str, screen: Option<crate::gamescope::Mode>) -> Result<Vec<crate::launch_keys::Row>> {
        Ok(crate::launch_keys::rows(crate::launch_keys::Scope::parse(scope)?, screen))
    }

    pub async fn gpu(&self) -> serde_json::Value {
        blocking(|| Ok(crate::gpu::detected().map(|g| g.to_json()).unwrap_or(serde_json::Value::Null))).await.unwrap_or(serde_json::Value::Null)
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
        let sources = self.sources.read().await.clone();
        let conn = if crate::desktop::detect(&cfg) == crate::desktop::Profile::Gnome { crate::host::session_bus().await } else { None };
        let runners: Vec<String> = self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.effective.runner.clone()).collect();
        crate::doctor::run(&cfg, &modules, &sources, conn.as_ref(), &runners).await
    }

    pub async fn source(&self, id: &str) -> Result<Source> {
        let m = self.source_or_hint(id).await?;
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
        let sources = self.sources.read().await;
        let caches = self.source_libraries.lock().await;
        let logins = self.source_logins.lock().await;
        sources
            .iter()
            .map(|m| {
                let s = m.merged_settings(&cfg);
                let mut j = m.to_json();
                j["games_dir"] = s.get("games_dir").cloned().unwrap_or(serde_json::Value::Null);
                j["library_cached"] = serde_json::json!(caches.get(m.id()).map(|v| v.len()).unwrap_or(0));
                j["library_at"] = serde_json::Value::String(Self::library_at(m));
                j["logged_in"] = serde_json::Value::Bool(logins.contains_key(m.id()));
                j["user"] = serde_json::Value::String(logins.get(m.id()).cloned().unwrap_or_default());
                j
            })
            .collect()
    }

    /// When the library was last fetched from the store: the cache file's mtime, RFC 3339; empty without one.
    fn library_at(m: &Source) -> String {
        std::fs::metadata(m.data_dir().join("library.json")).and_then(|md| md.modified()).map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default()
    }

    async fn run_verb(&self, m: &Source, verb: &str, args: &[String], mut progress: Option<Progress<'_, '_>>) -> Result<Vec<SourceEvent>> {
        let cfg = self.config.read().await.clone();
        let settings = m.merged_settings(&cfg);
        let mut events = Vec::new();
        let key = (matches!(verb, "install" | "update") && !args.is_empty()).then(|| format!("{}:{}", m.id(), args[0]));
        let result = sources::run(
            m,
            &settings,
            verb,
            args,
            |pid| {
                if let Some(k) = &key {
                    self.source_jobs.lock().unwrap().insert(k.clone(), pid);
                }
            },
            |ev| {
                if let (SourceEvent::Progress { done, total, message }, Some(p)) = (&ev, progress.as_mut()) {
                    p(*done, *total, message);
                }
                events.push(ev);
            },
        )
        .await;
        if let Some(k) = &key {
            self.source_jobs.lock().unwrap().remove(k);
        }
        result?;
        Ok(events)
    }

    /// SIGTERMs the source process installing or updating `game_id`; the source stops its downloader and keeps
    /// what it has, so the next `install` resumes. False when nothing was running for it.
    pub fn source_cancel(&self, source: &str, game_id: &str) -> bool {
        let Some(pid) = self.source_jobs.lock().unwrap().get(&format!("{source}:{game_id}")).copied() else { return false };
        // SAFETY: kill(2) with a pid this process spawned and still holds a handle to.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) == 0 }
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

    const SIZE_KEYS: [&'static str; 2] = ["download_size", "disk_size"];

    fn save_source_library(m: &Source, games: &[serde_json::Map<String, serde_json::Value>]) {
        let _ = std::fs::create_dir_all(m.data_dir());
        let _ = std::fs::write(m.data_dir().join("library.json"), serde_json::to_string(games).unwrap_or_default());
    }

    /// The store's listing into the cache: on `refresh`, or when there is none yet.
    async fn fetch_source_library(&self, m: &Source, refresh: bool) -> Result<()> {
        let source = m.id();
        if !refresh && self.source_libraries.lock().await.contains_key(source) {
            return Ok(());
        }
        let events = self.run_verb(m, "library", &[], None).await?;
        let mut games = Self::game_events(&events);
        let mut caches = self.source_libraries.lock().await;
        // Sizes come one `info` at a time; a listing without them keeps what was learnt.
        if let Some(old) = caches.get(source) {
            for g in games.iter_mut() {
                let Some(prev) = old.iter().find(|p| p.get("id") == g.get("id")) else { continue };
                for k in Self::SIZE_KEYS {
                    if g.get(k).map_or(true, |v| v.is_null()) {
                        if let Some(v) = prev.get(k).filter(|v| !v.is_null()) {
                            g.insert(k.into(), v.clone());
                        }
                    }
                }
            }
        }
        Self::save_source_library(m, &games);
        caches.insert(source.into(), games);
        Ok(())
    }

    pub async fn source_library(&self, source: &str, refresh: bool) -> Result<Vec<serde_json::Value>> {
        let m = self.source(source).await?;
        self.fetch_source_library(&m, refresh).await?;
        let list = self.source_libraries.lock().await.get(source).cloned().unwrap_or_default();
        // What is on the disk right now: an install's size, a stopped download's folder and bytes.
        let scanned = self.run_verb(&m, "scan", &[], None).await;
        if let Err(e) = &scanned {
            tracing::warn!("{source}: scan failed, listing without disk state: {e}");
        }
        let on_disk: Option<BTreeMap<String, serde_json::Map<String, serde_json::Value>>> = scanned
            .ok()
            .map(|events| Self::game_events(&events).into_iter().filter_map(|g| g.get("id").and_then(|v| v.as_str()).map(|id| id.to_string()).map(|id| (id, g))).collect());
        let games = self.games.read().await;
        let list: Vec<serde_json::Value> = list
            .into_iter()
            .map(|mut g| {
                let gid = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if let Some(local) = games.iter().find(|x| x.game.source.gog_id == gid && !gid.is_empty()) {
                    g.insert("installed".into(), serde_json::Value::Bool(local.game.is_installed()));
                    g.insert("game_id".into(), serde_json::Value::String(local.game.id.clone()));
                    g.insert("build".into(), serde_json::Value::String(local.game.source.build_id.clone()));
                } else if let Some(on_disk) = &on_disk {
                    // The cache holds the install state at fetch time; a folder gone since is not installed.
                    let d = on_disk.get(&gid);
                    let installed = d.and_then(|d| d.get("installed")).and_then(|v| v.as_bool()).unwrap_or(false);
                    g.insert("installed".into(), serde_json::Value::Bool(installed));
                    for k in ["dir", "exe", "build"] {
                        g.insert(k.into(), d.filter(|_| installed).and_then(|d| d.get(k)).cloned().unwrap_or(serde_json::Value::Null));
                    }
                }
                if let Some(d) = on_disk.as_ref().and_then(|m| m.get(&gid)) {
                    for k in ["partial_dir", "partial_bytes"] {
                        g.insert(k.into(), d.get(k).cloned().unwrap_or(serde_json::Value::Null));
                    }
                    let installed = d.get("installed").and_then(|v| v.as_bool()).unwrap_or(false);
                    for k in Self::SIZE_KEYS {
                        if let Some(v) = d.get(k).filter(|v| !v.is_null()) {
                            if installed || g.get(k).map_or(true, |v| v.is_null()) {
                                g.insert(k.into(), v.clone());
                            }
                        }
                    }
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

    /// The source's raw `info` payload; the `download_size` and `disk_size` it reports are added to it and
    /// remembered in the library cache, so the listing carries them from then on.
    pub async fn source_info(&self, source: &str, game_id: &str) -> Result<serde_json::Value> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "info", &[game_id.to_string()], None).await?;
        let Some((data, sizes)) = events.iter().find_map(|e| match e {
            SourceEvent::Info { data, download_size, disk_size } => Some((data.clone(), [*download_size, *disk_size])),
            _ => None,
        }) else {
            return Ok(serde_json::Value::Null);
        };
        let sizes: Vec<(&str, u64)> = Self::SIZE_KEYS.iter().zip(sizes).filter_map(|(k, v)| v.map(|v| (*k, v))).collect();
        if sizes.is_empty() {
            return Ok(data);
        }
        let mut data = data;
        if let Some(obj) = data.as_object_mut() {
            for (k, v) in &sizes {
                obj.insert((*k).into(), serde_json::json!(v));
            }
        }
        let mut caches = self.source_libraries.lock().await;
        if let Some(games) = caches.get_mut(source) {
            if let Some(g) = games.iter_mut().find(|g| g.get("id").and_then(|v| v.as_str()) == Some(game_id)) {
                for (k, v) in &sizes {
                    g.insert((*k).into(), serde_json::json!(v));
                }
                // The file's mtime is `library_at`, the store fetch; learning a size must not move it.
                let path = m.data_dir().join("library.json");
                let fetched = std::fs::metadata(&path).and_then(|md| md.modified()).ok();
                Self::save_source_library(&m, games);
                if let Some(t) = fetched {
                    let _ = std::fs::File::options().write(true).open(&path).and_then(|f| f.set_modified(t));
                }
            }
        }
        Ok(data)
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
            self.sources.read().await.iter().filter(|m| m.active()).map(|m| m.id().to_string()).collect()
        } else {
            vec![self.source(source).await?.id().to_string()]
        };
        let mut found = 0;
        for sid in ids {
            let m = self.source(&sid).await?;
            if let Err(e) = self.fetch_source_library(&m, false).await {
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
        let sources: Vec<Source> = self.sources.read().await.iter().filter(|m| m.active()).cloned().collect();
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
            match blocking(move || crate::media::refresh(&cfg, &game, force)).await {
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
        let placed = blocking(move || crate::media::set_slot_url(&cfg, &game, &slot, &url)).await?;
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
        blocking(move || crate::media::candidates(&cfg, &game, &slot, page)).await
    }

    pub async fn media_search(&self, id: &str, query: &str) -> Result<Vec<crate::media::Hit>> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let query = query.to_string();
        blocking(move || crate::media::search(&cfg, &game, &query)).await
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

    /// A `fake` source whose script is a shell case over the verb; every Universe home under one tempdir.
    fn fake_source(script: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (var, sub) in [("UNIVERSE_DATA_HOME", "data"), ("UNIVERSE_STATE_HOME", "state"), ("UNIVERSE_CONFIG_HOME", "config"), ("UNIVERSE_MODULES_PATH", "modules"), ("UNIVERSE_SOURCES_PATH", "sources")] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::env::set_var(var, dir.path().join(sub));
        }
        std::fs::write(dir.path().join("config/config.toml"), "[modules]\nenabled = []\n[sources]\nenabled = [\"fake\"]\n").unwrap();
        let src = dir.path().join("sources/fake");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("source.toml"), "api = 2\nid = \"fake\"\nname = \"Fake\"\nexe = \"run\"\n").unwrap();
        std::fs::write(src.join("run"), format!("#!/bin/sh\ncase \"$1\" in\n{script}\nesac\necho '{{\"event\":\"done\"}}'\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(src.join("run"), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        dir
    }

    async fn open() -> Core {
        Core::open_with(crate::config::Config::load().unwrap(), Host::memory().0).await.unwrap()
    }

    #[tokio::test]
    async fn sizes_learnt_by_info_survive_a_refresh_and_leave_library_at_alone() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _dir = fake_source(
            r#"library) echo '{"event":"game","id":"1","title":"One","owned":true,"installed":false}' ;;
info) echo '{"event":"info","data":{"folder_name":"One"},"download_size":700,"disk_size":1000}' ;;
scan) echo '{"event":"game","id":"1","title":"One","owned":true,"installed":false,"partial_dir":"/g/One","partial_bytes":300}' ;;"#,
        );
        let core = open().await;
        assert_eq!(core.sources().await[0]["library_at"], "", "no fetch yet");
        let list = core.source_library("fake", true).await.unwrap();
        assert!(list[0].get("download_size").is_none());
        assert_eq!((list[0]["partial_dir"].as_str(), list[0]["partial_bytes"].as_u64()), (Some("/g/One"), Some(300)), "the disk state rides on every listing");
        let at = core.sources().await[0]["library_at"].as_str().unwrap().to_string();
        assert!(!at.is_empty());
        std::fs::File::options().write(true).open(paths::sources_data_dir("fake").join("library.json")).unwrap().set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000)).unwrap();
        let at = core.sources().await[0]["library_at"].as_str().unwrap().to_string();
        let info = core.source_info("fake", "1").await.unwrap();
        assert_eq!((info["download_size"].as_u64(), info["disk_size"].as_u64(), info["folder_name"].as_str()), (Some(700), Some(1000), Some("One")));
        assert_eq!(core.sources().await[0]["library_at"], at, "learning a size is not a fetch");
        let list = core.source_library("fake", false).await.unwrap();
        assert_eq!(list[0]["download_size"], 700);
        let list = core.source_library("fake", true).await.unwrap();
        assert_eq!((list[0]["download_size"].as_u64(), list[0]["disk_size"].as_u64()), (Some(700), Some(1000)), "a listing without sizes keeps the learnt ones");
        assert_eq!(core.sources().await[0]["library_at"].as_str().unwrap() >= at.as_str(), true);
        let saved: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(paths::sources_data_dir("fake").join("library.json")).unwrap()).unwrap();
        assert_eq!(saved[0]["disk_size"], 1000, "the file carries them too");
    }

    #[tokio::test]
    async fn listing_takes_the_install_state_from_the_disk_not_the_cache() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _dir = fake_source(
            r#"library) echo '{"event":"game","id":"1","title":"Gone","owned":true,"installed":true,"dir":"/g/Gone","exe":"gone.exe","build":"7"}'; echo '{"event":"game","id":"2","title":"New","owned":true,"installed":false}' ;;
scan) echo '{"event":"game","id":"2","title":"New","owned":true,"installed":true,"dir":"/g/New","exe":"new.exe","build":"9","disk_size":500}' ;;"#,
        );
        let core = open().await;
        let list = core.source_library("fake", true).await.unwrap();
        assert_eq!((list[0]["installed"].as_bool(), list[0]["dir"].as_str(), list[0]["exe"].as_str()), (Some(false), None, None), "removed since the fetch");
        assert_eq!((list[1]["installed"].as_bool(), list[1]["dir"].as_str(), list[1]["build"].as_str(), list[1]["disk_size"].as_u64()), (Some(true), Some("/g/New"), Some("9"), Some(500)), "installed since the fetch");
    }

    #[tokio::test]
    async fn cancel_sigterms_the_running_install_and_nothing_else() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        // The downloader child is detached from the pipes and killed on TERM, as the gog source does with gogdl.
        let _dir = fake_source(r#"install) sleep 30 >/dev/null 2>&1 & dl=$!; trap 'kill $dl; exit 143' TERM; echo '{"event":"progress","done":1,"total":10,"message":"10%"}'; wait $dl; exit 1 ;;"#);
        let core = open().await;
        assert!(!core.source_cancel("fake", "1"), "nothing running");
        let started = std::time::Instant::now();
        let mut seen = 0u64;
        let mut p = |done: u64, _total: u64, _m: &str| seen = done;
        let install = core.source_install("fake", "1", Some(&mut p));
        let cancel = async {
            while !core.source_jobs.lock().unwrap().contains_key("fake:1") {
                assert!(started.elapsed() < std::time::Duration::from_secs(5), "the install never registered");
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            assert!(core.source_cancel("fake", "1"));
        };
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async { tokio::join!(install, cancel) }).await.expect("cancel ends the install");
        assert!(result.unwrap_err().to_string().contains("143"), "the source exits on the TERM");
        assert_eq!(seen, 1, "progress reached the caller before the cancel");
        assert!(core.source_jobs.lock().unwrap().is_empty(), "the registry is cleared on the way out");
    }

    #[test]
    fn titles_from_files() {
        assert_eq!(title_of(Path::new("/g/F-Zero GX.iso")), "F-Zero GX");
        assert_eq!(title_of(Path::new("/s/SUPER MARIO ODYSSEY v1.0.3 Eur SuperXCi - CLC.xci")), "SUPER MARIO ODYSSEY Eur SuperXCi - CLC");
        assert_eq!(title_of(Path::new("/r/Pokemon - HeartGold Version (USA) [rev 1].nds")), "Pokemon - HeartGold Version");
        assert_eq!(title_of(Path::new("/r/mario_kart_wii.wbfs")), "mario kart wii");
    }
}
