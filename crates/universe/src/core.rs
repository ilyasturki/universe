use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::config::Config;
use crate::game::Game;
use crate::index::Index;
use crate::launcher;
use crate::library::{self, Resolved};
use crate::modules::{self, HookEnv, Module, SourceEvent};
use crate::paths;
use crate::sessions::{self, Session};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub enum Event {
    LibraryChanged(Vec<String>),
    SessionStarted { session: String, id: String },
    SessionEnded { session: String, id: String, duration_s: u32 },
    CurrentChanged(String),
    RecordingFiled { session: String, id: String, path: String },
    EntryWritten { session: String, id: String },
    Progress { job: String, done: u64, total: u64, message: String },
    JobFinished { job: String, ok: bool, message: String },
    MediaChanged(String),
    ModulesChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Current {
    pub session_id: String,
    pub id: String,
    pub title: String,
    pub unit: String,
    pub screen: String,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub done: u64,
    pub total: u64,
    pub message: String,
    pub finished: bool,
    pub ok: bool,
}

pub struct Core {
    pub config: RwLock<Config>,
    pub modules: RwLock<Vec<Module>>,
    pub games: RwLock<Vec<Resolved>>,
    pub index: Mutex<Index>,
    pub current: Mutex<Option<Current>>,
    pub jobs: Mutex<BTreeMap<String, Job>>,
    pub source_libraries: Mutex<BTreeMap<String, Vec<serde_json::Map<String, serde_json::Value>>>>,
    pub source_logins: Mutex<BTreeMap<String, String>>,
    pub events: broadcast::Sender<Event>,
    pub conn: tokio::sync::OnceCell<zbus::Connection>,
    job_seq: AtomicU64,
    post_processed: Mutex<Vec<String>>,
}

impl Core {
    pub fn new(config: Config) -> Result<Arc<Core>> {
        let modules = modules::discover(&config);
        let games = library::load_all(&config, &modules);
        let mut index = Index::open(&paths::index_file()).or_else(|_| Index::open_memory())?;
        index.rebuild(&games)?;
        let (tx, _) = broadcast::channel(256);
        let core = Arc::new(Core {
            config: RwLock::new(config),
            modules: RwLock::new(modules),
            games: RwLock::new(games),
            index: Mutex::new(index),
            current: Mutex::new(None),
            jobs: Mutex::new(BTreeMap::new()),
            source_libraries: Mutex::new(BTreeMap::new()),
            source_logins: Mutex::new(BTreeMap::new()),
            events: tx,
            conn: tokio::sync::OnceCell::new(),
            job_seq: AtomicU64::new(1),
            post_processed: Mutex::new(vec![]),
        });
        Ok(core)
    }

    fn emit(&self, e: Event) {
        let _ = self.events.send(e);
    }

    pub async fn load_source_caches(&self) {
        let modules = self.modules.read().await;
        let mut caches = self.source_libraries.lock().await;
        for m in modules.iter().filter(|m| m.is_source()) {
            let p = m.data_dir().join("library.json");
            if let Ok(s) = std::fs::read_to_string(&p) {
                if let Ok(v) = serde_json::from_str::<Vec<serde_json::Map<String, serde_json::Value>>>(&s) {
                    caches.insert(m.id().to_string(), v);
                }
            }
        }
        drop(caches);
        let sources: Vec<Module> = modules.iter().filter(|m| m.is_source() && m.active()).cloned().collect();
        drop(modules);
        for m in sources {
            self.refresh_login(&m).await;
        }
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

    // ----- library -----

    pub async fn reload_all(&self) -> Result<()> {
        let config = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        let games = library::load_all(&config, &modules);
        self.index.lock().await.rebuild(&games)?;
        *self.games.write().await = games;
        self.emit(Event::LibraryChanged(vec![]));
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
        drop(games);
        self.emit(Event::LibraryChanged(vec![id.to_string()]));
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
        crate::game::set_key(&r.game.toml_path(), &real_key, value)?;
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

    pub async fn current_json(&self) -> String {
        match self.current.lock().await.as_ref() {
            Some(c) => serde_json::to_string(c).unwrap_or_default(),
            None => String::new(),
        }
    }

    async fn set_current(&self, c: Option<Current>) {
        let json = c.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()).unwrap_or_default();
        *self.current.lock().await = c;
        let p = paths::current_session_file();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        if json.is_empty() {
            let _ = std::fs::remove_file(&p);
        } else {
            let _ = std::fs::write(&p, &json);
        }
        self.emit(Event::CurrentChanged(json));
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
        env.set("UNIVERSE_BUS", crate::BUS_NAME);
        env.set("UNIVERSE_OBJECT", crate::OBJECT_PATH);
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

    pub async fn launch(self: &Arc<Self>, id: &str, screen: &str) -> Result<String> {
        if let Some(c) = self.current.lock().await.as_ref() {
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
        let unit = launcher::unit_name(id, &session_id);

        let mut base = self.hook_env_base(&r, &cfg);
        base.set("SESSION_ID", session_id.clone());
        base.set("SESSION_UNIT", format!("{unit}.scope"));
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

        let profile = crate::desktop::detect(&cfg);
        if r.effective.hide_cursor {
            if let Some(conn) = self.conn.get() {
                crate::desktop::set_cursor_hidden(conn, profile, &cfg.desktop.cursor_extension, true).await;
            }
        }
        tracing::info!("launch {id}: {}", plan.command_line());
        let mut running = launcher::spawn(&plan)?;
        let current = Current { session_id: session_id.clone(), id: id.into(), title: r.game.title.clone(), unit: format!("{unit}.scope"), screen: screen.clone(), started_at: started.to_rfc3339() };
        self.set_current(Some(current)).await;
        self.emit(Event::SessionStarted { session: session_id.clone(), id: id.into() });

        for m in self.hook_modules(&r, "post-launch").await {
            let env = self.module_env(&m, &r, &cfg, &base);
            if let Err(e) = modules::run_async(&m, "post-launch", &env, &session_id) {
                tracing::warn!("post-launch {}: {e}", m.id());
            }
        }

        let core = Arc::clone(self);
        let sid = session_id.clone();
        let gid = id.to_string();
        tokio::spawn(async move {
            let cgroup = launcher::cgroup_dir(&running.unit).await;
            let stderr = running.child.stderr.take();
            let exit = launcher::wait_empty(cgroup.as_deref(), &mut running.child).await;
            if let Some(mut e) = stderr {
                use tokio::io::AsyncReadExt;
                let mut s = String::new();
                let _ = e.read_to_string(&mut s).await;
                if !s.trim().is_empty() {
                    tracing::info!("{}: {}", running.unit, s.trim());
                }
            }
            core.finish_session(&gid, &sid, &plan, exit, &base, started).await;
        });
        Ok(session_id)
    }

    async fn finish_session(self: &Arc<Self>, id: &str, session_id: &str, plan: &launcher::Plan, exit: i32, base: &HookEnv, started: chrono::DateTime<chrono::Local>) {
        let ended = chrono::Local::now();
        let duration_s = (ended - started).num_seconds().max(0) as u64;
        let cfg = self.config.read().await.clone();
        let r = match self.get(id).await {
            Ok(r) => r,
            Err(_) => return,
        };
        let session = Session {
            session: session_id.into(),
            game: id.into(),
            started_at: started.to_rfc3339(),
            ended_at: ended.to_rfc3339(),
            duration_s,
            source: "daemon".into(),
            unit: format!("{}.scope", plan.unit),
            screen: base.vars.iter().find(|(k, _)| k == "SESSION_SCREEN").map(|(_, v)| v.clone()).unwrap_or_default(),
            exit,
            recording: None,
        };
        if let Err(e) = sessions::append(&r.game.sessions_path(), &session) {
            tracing::error!("sessions.jsonl: {e}");
        }
        if r.effective.hide_cursor {
            if let Some(conn) = self.conn.get() {
                crate::desktop::set_cursor_hidden(conn, crate::desktop::detect(&cfg), &cfg.desktop.cursor_extension, false).await;
            }
        }
        let _ = launcher::run_shell(&plan.post_command, &plan.env, &plan.cwd).await;
        let mut env_end = base.clone();
        env_end.set("SESSION_ENDED_AT", ended.to_rfc3339());
        env_end.set("SESSION_DURATION_S", duration_s.to_string());
        let mut expects_recording = false;
        for m in self.hook_modules(&r, "session-end").await {
            let env = self.module_env(&m, &r, &cfg, &env_end);
            if let Err(e) = modules::run_blocking(&m, "session-end", &env).await {
                tracing::warn!("session-end {}: {e}", m.id());
            }
            expects_recording = true;
        }
        self.set_current(None).await;
        let _ = self.reload_game(id).await;
        self.emit(Event::SessionEnded { session: session_id.into(), id: id.into(), duration_s: duration_s as u32 });

        if !expects_recording {
            self.post_process(id, session_id, "").await;
        } else {
            let core = Arc::clone(self);
            let (gid, sid) = (id.to_string(), session_id.to_string());
            let grace = cfg.modules.settings.get("core").and_then(|t| t.get("post_process_grace_s")).and_then(|v| v.as_integer()).unwrap_or(45) as u64;
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(grace)).await;
                core.post_process(&gid, &sid, "").await;
            });
        }
    }

    /// post-process hooks run once per session: after RecordingFiled, or after the grace period without one.
    async fn post_process(self: &Arc<Self>, id: &str, session_id: &str, recording: &str) {
        {
            let mut done = self.post_processed.lock().await;
            if done.iter().any(|s| s == session_id) {
                return;
            }
            done.push(session_id.to_string());
            if done.len() > 64 {
                done.remove(0);
            }
        }
        let Ok(r) = self.get(id).await else { return };
        let cfg = self.config.read().await.clone();
        let sess = sessions::read(&r.game.sessions_path()).unwrap_or_default().into_iter().find(|s| s.session == session_id);
        let mut env = self.hook_env_base(&r, &cfg);
        env.set("SESSION_ID", session_id);
        env.set("RECORDING_PATH", recording);
        if let Some(s) = sess {
            env.set("SESSION_STARTED_AT", s.started_at);
            env.set("SESSION_ENDED_AT", s.ended_at);
            env.set("SESSION_DURATION_S", s.duration_s.to_string());
            env.set("SESSION_SCREEN", s.screen);
        }
        for m in self.hook_modules(&r, "post-process").await {
            let menv = self.module_env(&m, &r, &cfg, &env);
            if let Err(e) = modules::run_async(&m, "post-process", &menv, session_id) {
                tracing::warn!("post-process {}: {e}", m.id());
            }
        }
    }

    pub async fn stop(&self, session_id: &str) -> Result<()> {
        let cur = self.current.lock().await.clone();
        let Some(c) = cur else { return Err(Error::NotFound("no session running".into())) };
        if !session_id.is_empty() && c.session_id != session_id {
            return Err(Error::NotFound(session_id.into()));
        }
        launcher::stop_unit(c.unit.trim_end_matches(".scope")).await
    }

    pub async fn screenshot(&self) -> Result<String> {
        let cur = self.current.lock().await.clone();
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
        if let Some(c) = self.current.lock().await.as_ref() {
            if c.session_id == session_id {
                return Some(c.id.clone());
            }
        }
        let games = self.games.read().await;
        games.iter().find(|g| g.sessions.iter().any(|s| s.session == session_id)).map(|g| g.game.id.clone())
    }

    pub async fn file_recording(self: &Arc<Self>, session_id: &str, path: &str) -> Result<String> {
        let id = self.game_of_session(session_id).await.ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
        let r = self.get(&id).await?;
        let cfg = self.config.read().await.clone();
        let dest = crate::recording::file(&r.game, session_id, Path::new(path), &cfg.recordings_root())?;
        self.reload_game(&id).await?;
        let ds = dest.to_string_lossy().to_string();
        self.emit(Event::RecordingFiled { session: session_id.into(), id: id.clone(), path: ds.clone() });
        self.post_process(&id, session_id, &ds).await;
        Ok(ds)
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
        crate::journal::write(&r.game.journal_dir(), &entry)?;
        self.reload_game(&id).await?;
        self.emit(Event::EntryWritten { session: session_id.into(), id });
        Ok(())
    }

    pub async fn journal_json(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        Ok(serde_json::to_string(&r.journal)?)
    }

    pub async fn render_journal(&self, id: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let dir = cfg.journal_root().join(id);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.md", crate::slug::note_name(&r.game.title)));
        let cover = r.media.iter().find(|(s, _)| s == "box_front").map(|(_, p)| p.clone());
        std::fs::write(&path, crate::journal::render_markdown(&r.game.title, &r.journal, cover.as_deref()))?;
        Ok(path.to_string_lossy().into())
    }

    // ----- modules -----

    pub async fn modules_json(&self) -> String {
        serde_json::Value::Array(self.modules.read().await.iter().map(|m| m.to_json()).collect()).to_string()
    }

    pub async fn reload_modules(&self) {
        let cfg = self.config.read().await.clone();
        *self.modules.write().await = modules::discover(&cfg);
        self.emit(Event::ModulesChanged);
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
        let cfg = Config::load()?;
        *self.config.write().await = cfg;
        self.reload_modules().await;
        self.reload_all().await
    }

    pub async fn module_settings_json(&self, module: &str, game_id: &str) -> Result<String> {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await;
        let m = modules.iter().find(|m| m.id() == module).ok_or_else(|| Error::NotFound(format!("module {module}")))?;
        let game = if game_id.is_empty() { None } else { Some(self.get(game_id).await?.game) };
        Ok(serde_json::Value::Object(m.merged_settings(&cfg, game.as_ref())).to_string())
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
        Config::set_key(&paths::config_file(), key, value)?;
        self.reload_config().await
    }

    pub async fn doctor_json(&self) -> String {
        let cfg = self.config.read().await.clone();
        let modules = self.modules.read().await.clone();
        serde_json::to_string(&crate::doctor::run(&cfg, &modules)).unwrap_or_default()
    }

    // ----- jobs & sources -----

    async fn new_job(&self, kind: &str, target: &str) -> String {
        let n = self.job_seq.fetch_add(1, Ordering::SeqCst);
        let id = format!("job-{n}");
        self.jobs.lock().await.insert(id.clone(), Job { id: id.clone(), kind: kind.into(), target: target.into(), ..Default::default() });
        id
    }

    async fn job_progress(&self, job: &str, done: u64, total: u64, message: &str) {
        if let Some(j) = self.jobs.lock().await.get_mut(job) {
            j.done = done;
            j.total = total;
            j.message = message.into();
        }
        self.emit(Event::Progress { job: job.into(), done, total, message: message.into() });
    }

    async fn job_finish(&self, job: &str, ok: bool, message: &str) {
        if let Some(j) = self.jobs.lock().await.get_mut(job) {
            j.finished = true;
            j.ok = ok;
            j.message = message.into();
        }
        self.emit(Event::JobFinished { job: job.into(), ok, message: message.into() });
    }

    pub async fn jobs_json(&self) -> String {
        serde_json::to_string(&self.jobs.lock().await.values().collect::<Vec<_>>()).unwrap_or_default()
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

    pub async fn sources_json(&self) -> String {
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

    /// Runs a verb to completion, collecting events; `progress` job updates are forwarded when a job id is given.
    async fn run_verb(&self, m: &Module, verb: &str, args: &[String], job: Option<&str>) -> Result<Vec<SourceEvent>> {
        let cfg = self.config.read().await.clone();
        let settings = m.merged_settings(&cfg, None);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(u64, u64, String)>();
        let core_events = self.events.clone();
        let job_id = job.map(|s| s.to_string());
        let fwd = tokio::spawn(async move {
            while let Some((d, t, msg)) = rx.recv().await {
                if let Some(j) = &job_id {
                    let _ = core_events.send(Event::Progress { job: j.clone(), done: d, total: t, message: msg });
                }
            }
        });
        let mut events = Vec::new();
        let res = modules::run_source(m, &settings, verb, args, |ev| {
            if let SourceEvent::Progress { done, total, message } = &ev {
                let _ = tx.send((*done, *total, message.clone()));
            }
            events.push(ev);
        })
        .await;
        drop(tx);
        let _ = fwd.await;
        res?;
        Ok(events)
    }

    pub async fn source_login_url(&self, source: &str) -> Result<String> {
        let m = self.source(source).await?;
        let events = self.run_verb(&m, "login", &[], None).await?;
        events.iter().find_map(|e| if let SourceEvent::LoginUrl { url } = e { Some(url.clone()) } else { None }).ok_or_else(|| Error::Io("no login_url event".into()))
    }

    pub async fn source_login(self: &Arc<Self>, source: &str, code: &str) -> Result<String> {
        let m = self.source(source).await?;
        let job = self.new_job("login", source).await;
        let core = Arc::clone(self);
        let code = code.to_string();
        let j = job.clone();
        tokio::spawn(async move {
            match core.run_verb(&m, "login", &[code], Some(&j)).await {
                Ok(ev) => {
                    let user = ev.iter().find_map(|e| if let SourceEvent::LoggedIn { user } = e { Some(user.clone()) } else { None }).unwrap_or_default();
                    core.source_logins.lock().await.insert(m.id().to_string(), user.clone());
                    core.job_finish(&j, true, &user).await
                }
                Err(e) => core.job_finish(&j, false, &e.to_string()).await,
            }
        });
        Ok(job)
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

    pub async fn source_scan(self: &Arc<Self>, source: &str) -> Result<String> {
        let ids: Vec<String> = if source.is_empty() {
            self.modules.read().await.iter().filter(|m| m.is_source() && m.active()).map(|m| m.id().to_string()).collect()
        } else {
            vec![self.source(source).await?.id().to_string()]
        };
        let job = self.new_job("scan", source).await;
        let core = Arc::clone(self);
        let j = job.clone();
        tokio::spawn(async move {
            let mut found = 0;
            for sid in ids {
                let Ok(m) = core.source(&sid).await else { continue };
                if let Err(e) = core.source_library(&sid, false).await {
                    tracing::warn!("{sid}: library unavailable, scanning without ownership: {e}");
                }
                match core.run_verb(&m, "scan", &[], Some(&j)).await {
                    Ok(events) => {
                        for g in Self::game_events(&events) {
                            if let Ok(Some(_)) = core.apply_source_game(&sid, &g, true).await {
                                found += 1;
                            }
                        }
                    }
                    Err(e) => {
                        core.job_finish(&j, false, &e.to_string()).await;
                        return;
                    }
                }
            }
            core.job_finish(&j, true, &format!("{found} game(s)")).await;
        });
        Ok(job)
    }

    pub async fn source_install(self: &Arc<Self>, source: &str, game_id: &str) -> Result<String> {
        let m = self.source(source).await?;
        let job = self.new_job("install", game_id).await;
        let core = Arc::clone(self);
        let (j, gid, src) = (job.clone(), game_id.to_string(), source.to_string());
        tokio::spawn(async move {
            match core.run_verb(&m, "install", &[gid.clone()], Some(&j)).await {
                Ok(events) => {
                    let mut msg = String::new();
                    for g in Self::game_events(&events) {
                        let mut g = g;
                        g.entry("owned".to_string()).or_insert(serde_json::Value::Bool(true));
                        g.entry("installed".to_string()).or_insert(serde_json::Value::Bool(true));
                        if let Ok(Some(id)) = core.apply_source_game(&src, &g, true).await {
                            msg = id;
                        }
                    }
                    core.job_finish(&j, true, &msg).await;
                }
                Err(e) => core.job_finish(&j, false, &e.to_string()).await,
            }
        });
        Ok(job)
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

    pub async fn source_update(self: &Arc<Self>, source: &str, game_id: &str) -> Result<String> {
        let m = self.source(source).await?;
        let job = self.new_job("update", game_id).await;
        let core = Arc::clone(self);
        let (j, gid, src) = (job.clone(), game_id.to_string(), source.to_string());
        tokio::spawn(async move {
            let targets: Vec<String> = if gid.is_empty() {
                match core.run_verb(&m, "update", &[], Some(&j)).await {
                    Ok(ev) => ev.iter().filter_map(|e| if let SourceEvent::Update(u) = e { u.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) } else { None }).collect(),
                    Err(e) => {
                        core.job_finish(&j, false, &e.to_string()).await;
                        return;
                    }
                }
            } else {
                vec![gid]
            };
            let mut n = 0;
            for t in targets {
                match core.run_verb(&m, "update", &[t.clone()], Some(&j)).await {
                    Ok(events) => {
                        for g in Self::game_events(&events) {
                            let _ = core.apply_source_game(&src, &g, false).await;
                        }
                        n += 1;
                    }
                    Err(e) => {
                        core.job_finish(&j, false, &format!("{t}: {e}")).await;
                        return;
                    }
                }
            }
            core.job_finish(&j, true, &format!("{n} updated")).await;
        });
        Ok(job)
    }

    // ----- media -----

    pub async fn media_refresh(self: &Arc<Self>, id: &str, force: bool) -> Result<String> {
        let ids: Vec<String> = if id.is_empty() { self.games.read().await.iter().filter(|g| g.game.removed_at.is_empty()).map(|g| g.game.id.clone()).collect() } else { vec![self.resolve_one(id).await?] };
        let job = self.new_job("media", id).await;
        let core = Arc::clone(self);
        let j = job.clone();
        tokio::spawn(async move {
            let cfg = core.config.read().await.clone();
            let total = ids.len() as u64;
            let mut ok = 0;
            for (i, gid) in ids.iter().enumerate() {
                let Ok(r) = core.get(gid).await else { continue };
                core.job_progress(&j, i as u64, total, &r.game.title).await;
                match tokio::task::spawn_blocking({
                    let cfg = cfg.clone();
                    let game = r.game.clone();
                    move || crate::media::refresh(&cfg, &game, force)
                })
                .await
                {
                    Ok(Ok(changed)) => {
                        if changed {
                            ok += 1;
                            let _ = core.reload_game(gid).await;
                            core.emit(Event::MediaChanged(gid.clone()));
                        }
                    }
                    Ok(Err(e)) => tracing::warn!("media {gid}: {e}"),
                    Err(e) => tracing::warn!("media {gid}: {e}"),
                }
            }
            core.job_finish(&j, true, &format!("{ok}/{total} updated")).await;
        });
        Ok(job)
    }

    pub async fn media_set_slot(&self, id: &str, slot: &str, path: &str) -> Result<()> {
        let r = self.get(id).await?;
        crate::media::set_slot(&r.game, slot, Path::new(path))?;
        self.reload_game(id).await?;
        self.emit(Event::MediaChanged(id.into()));
        Ok(())
    }

    pub async fn media_unset(&self, id: &str, slot: &str) -> Result<()> {
        let r = self.get(id).await?;
        crate::media::unset(&r.game, slot)?;
        self.reload_game(id).await?;
        self.emit(Event::MediaChanged(id.into()));
        Ok(())
    }

    pub async fn media_candidates(&self, id: &str, slot: &str) -> Result<String> {
        let r = self.get(id).await?;
        let cfg = self.config.read().await.clone();
        let game = r.game.clone();
        let slot = slot.to_string();
        let list = tokio::task::spawn_blocking(move || crate::media::candidates(&cfg, &game, &slot)).await.map_err(|e| Error::Io(e.to_string()))??;
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
