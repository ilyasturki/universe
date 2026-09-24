use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::core::{passthrough_env, Core};
use crate::host::{Prop, UnitSpec};
use crate::launcher::{self, Plan};
use crate::library::Resolved;
use crate::modules::{self, HookEnv};
use crate::paths;
use crate::sessions::{self, Session};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Current {
    pub session_id: String,
    pub id: String,
    pub title: String,
    pub unit: String,
    pub screen: String,
    pub started_at: String,
    /// Both 0 when the game got a gamescope of its own.
    #[serde(default)]
    pub gamescope_pid: u32,
    #[serde(default)]
    pub launcher_pid: u32,
}

/// state/current-session.json: everything `session-end` needs to close the session from a process that never saw the launch.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Marker {
    #[serde(flatten)]
    pub current: Current,
    #[serde(default)]
    pub hook_env: Vec<(String, String)>,
    #[serde(default)]
    pub undo: Vec<Undo>,
    #[serde(default)]
    pub command: String,
    /// Set by `stop`: the end that follows was asked for.
    #[serde(default)]
    pub stopped: bool,
}

/// What `launch` began, in begin order; undone in reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Undo {
    Pads,
    Cursor { was_active: bool },
    PostCommand { command: String, cwd: String, env: BTreeMap<String, String> },
    Hud,
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

/// A change to the running session's marker, in place: a temp file renamed over it, so `session-end` never reads half a marker.
fn update_marker(f: impl FnOnce(&mut Marker)) -> Result<()> {
    let Some(mut m) = read_marker() else { return Ok(()) };
    f(&mut m);
    let p = paths::current_session_file();
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(&m)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

impl Core {
    pub async fn current(&self) -> Option<Current> {
        let m = read_marker()?;
        self.host.units.is_active(&m.current.unit).await.then_some(m.current)
    }

    /// A marker without an active unit (crash, reboot) is closed now; one that cannot be read goes, or every launch would be `Busy`.
    pub async fn reconcile(&self) -> Result<()> {
        let path = paths::current_session_file();
        let Ok(text) = std::fs::read_to_string(&path) else { return Ok(()) };
        let m: Marker = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("{}: unreadable marker dropped: {e}", path.display());
                remove_marker();
                return Ok(());
            }
        };
        if self.host.units.is_active(&m.current.unit).await {
            return Ok(());
        }
        tracing::warn!("session {} of {} was left open; closing it", m.current.session_id, m.current.id);
        self.session_end(&m.current.id, &m.current.session_id, None, None).await
    }

    /// Every game launched afterwards is bound to the scope, so it goes down with the launcher; idempotent.
    pub async fn adopt_scope(&self) -> Result<String> {
        if let Some(name) = self.scope.get() {
            return Ok(name.clone());
        }
        let name = self.host.units.adopt_scope(std::process::id()).await?;
        Ok(self.scope.get_or_init(|| name).clone())
    }

    /// Starts the game as a transient service; `splash` is a poster for gamescope's keep-alive window (`splash.rs`'s format), `""` for none.
    pub async fn launch(&self, id: &str, screen: &str, splash: &str) -> Result<String> {
        self.reconcile().await?;
        if let Some(c) = self.current().await {
            return Err(Error::Busy(format!("{} is running ({})", c.title, c.session_id)));
        }
        let r = self.get(id).await?;
        if !r.game.removed_at.is_empty() {
            return Err(Error::Unavailable(format!("{id} was removed")));
        }
        let cfg = self.config.read().await.clone();
        let spec = crate::runners::spec(&r.effective.runner);
        if spec.is_some_and(|s| (s.kind == crate::runners::Kind::Proton && r.game.launch.runner_exe.is_empty()) || s.via_proton) {
            crate::tools::ensure(&cfg.launch.umu_run).await?;
        }
        let started = chrono::Local::now();
        let session_id = sessions::session_id(started);
        let screen = crate::desktop::pick_screen(screen);
        let unit = format!("{}.service", launcher::unit_name(id, &session_id));

        let mut base = self.hook_env_base(&r, &cfg);
        base.set("SESSION_ID", session_id.clone());
        base.set("SESSION_UNIT", unit.clone());
        base.set("SESSION_SCREEN", screen.clone());
        base.set("SESSION_STARTED_AT", started.to_rfc3339());

        for m in self.modules.read().await.iter().filter(|m| m.enabled && !m.available) {
            tracing::warn!("module {} is enabled but its hooks are skipped: missing {} (see `universe doctor`)", m.id(), m.missing.join(", "));
        }
        let env_file = paths::state_home().join(format!("env-{session_id}"));
        std::fs::create_dir_all(paths::state_home())?;
        std::fs::write(&env_file, "")?;
        for m in self.hook_modules(&r, "pre-launch").await {
            let mut env = self.module_env(&m, Some(&r.game), &cfg, &base);
            env.set("UNIVERSE_ENV_FILE", env_file.to_string_lossy().to_string());
            let out = modules::run_blocking(&m, "pre-launch", &env).await?;
            if out.status != 0 {
                let _ = std::fs::remove_file(&env_file);
                return Err(Error::Io(format!("pre-launch {} refused the launch: {}", m.id(), out.stderr.trim())));
            }
        }
        let extra_env: BTreeMap<String, String> = std::fs::read_to_string(&env_file)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| l.split_once('='))
            .filter(|(k, _)| !k.trim().is_empty())
            .map(|(k, v)| (k.trim().to_string(), v.to_string()))
            .collect();
        let _ = std::fs::remove_file(&env_file);

        let mode = crate::desktop::screen_mode(&screen).await;
        let splash = (!splash.is_empty()).then(|| std::path::PathBuf::from(splash));
        let gamescope_pid = self.nest().map_or(0, |n| n.pid);
        let launcher_pid = if gamescope_pid != 0 { std::process::id() } else { 0 };
        let mut plan = launcher::plan(&r, &cfg, &extra_env, mode, splash.as_deref(), gamescope_pid != 0, launcher::mangoapp_installed())?;
        for (path, text) in plan.mangohud_conf.iter().chain(&plan.mangoapp_conf) {
            std::fs::write(path, text)?;
        }
        if r.effective.debug_log {
            if let Some(spec) = crate::runners::spec(&r.effective.runner) {
                let dir = paths::session_log_dir(id, &session_id);
                std::fs::create_dir_all(&dir)?;
                let kind = if spec.via_proton { crate::runners::Kind::Proton } else { spec.kind };
                for (k, v) in launcher::debug_env(kind, &dir) {
                    plan.env.entry(k).or_insert(v);
                }
            }
        }

        let current = Current {
            session_id: session_id.clone(),
            id: id.into(),
            title: r.game.title.clone(),
            unit: unit.clone(),
            screen: screen.clone(),
            started_at: started.to_rfc3339(),
            gamescope_pid,
            launcher_pid,
        };
        let mut undo = Vec::new();
        if let Err(e) = self.begin(&r, &plan, &current, base.vars.clone(), &mut undo).await {
            if read_marker().is_some_and(|m| m.current.session_id == session_id) {
                remove_marker();
            }
            self.rollback(&undo).await;
            return Err(e);
        }
        for m in self.hook_modules(&r, "post-launch").await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &base);
            if let Err(e) = modules::run_async(&self.host.units, &m, "post-launch", &env, &session_id, Some(&unit)).await {
                tracing::warn!("post-launch {}: {e}", m.id());
            }
        }
        if cfg.controller.enabled {
            if let Err(e) = self.spawn_controller_watch(&session_id, &unit).await {
                tracing::warn!("controller watch: {e}");
            }
        }
        if cfg.desktop.keep_awake {
            if let Err(e) = self.spawn_keep_awake(&session_id, &unit, &r.game.title).await {
                tracing::warn!("keep awake: {e}");
            }
        }
        Ok(session_id)
    }

    /// Each effect pushes its undo as it begins; the marker carries the list before the unit starts.
    async fn begin(&self, r: &Resolved, plan: &Plan, current: &Current, hook_env: Vec<(String, String)>, undo: &mut Vec<Undo>) -> Result<()> {
        if !plan.post_command.trim().is_empty() {
            undo.push(Undo::PostCommand { command: plan.post_command.clone(), cwd: plan.cwd.to_string_lossy().to_string(), env: plan.env.clone() });
        }
        launcher::run_shell(&plan.pre_command, &plan.env, &plan.cwd).await?;
        if r.effective.inputplumber {
            if self.host.pads.engage().await {
                undo.push(Undo::Pads);
            }
        } else {
            self.host.pads.ensure_free().await;
        }
        if r.effective.hide_cursor {
            let was_active = self.host.shell.cursor_enable().await;
            undo.push(Undo::Cursor { was_active });
        }
        if current.gamescope_pid != 0 && launcher::mangoapp_installed() {
            self.apply_mangoapp(r.effective.mangohud, true)?;
            undo.push(Undo::Hud);
        }
        write_marker(&Marker { current: current.clone(), hook_env, undo: undo.clone(), command: plan.command_line(), stopped: false })?;
        tracing::info!("launch {}: {}", r.game.id, plan.command_line());
        let budget: u64 = 60 + self.hook_modules(r, "session-end").await.iter().map(|m| m.timeout().as_secs()).sum::<u64>();
        let mut env = passthrough_env();
        env.extend(plan.env.clone());
        let log_dir = paths::session_log_dir(&current.id, &current.session_id);
        if log_dir.is_dir() {
            let vars: String = env.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
            std::fs::write(log_dir.join("launch.txt"), format!("{}\n\n{vars}", plan.command_line()))?;
        }
        // gamescope strips WAYLAND_DISPLAY from its child, but a --user unit inherits the manager's, and Qt connects there before DISPLAY.
        let unset_env = if env.contains_key("WAYLAND_DISPLAY") { vec![] } else { vec!["WAYLAND_DISPLAY".to_string()] };
        let spec = UnitSpec {
            name: current.unit.clone(),
            description: format!("Universe: {}", r.game.title),
            program: plan.program.clone(),
            args: plan.args.clone(),
            env,
            unset_env,
            cwd: Some(plan.cwd.clone()),
            // ExitType=cgroup: the unit ends with the last game process, not with the one systemd-run started.
            properties: vec![("ExitType".into(), Prop::Str("cgroup".into())), ("TimeoutStopUSec".into(), Prop::U64(budget * 1_000_000))],
            bind_to: self.scope.get().cloned(),
            stop_post: vec![paths::self_exe().to_string_lossy().to_string(), "session-end".into(), current.id.clone(), current.session_id.clone()],
        };
        self.host.units.start(&spec).await
    }

    async fn rollback(&self, undo: &[Undo]) {
        for step in undo.iter().rev() {
            match step {
                Undo::Cursor { was_active } => self.host.shell.cursor_restore(*was_active).await,
                Undo::Pads => self.host.pads.release().await,
                // From ExecStopPost the launcher's gamescope may be gone already: told only while it is there.
                Undo::Hud => {
                    if let Err(e) = self.apply_mangoapp(false, self.nest().is_some()) {
                        tracing::warn!("mangoapp: {e}");
                    }
                }
                Undo::PostCommand { command, cwd, env } => {
                    if let Err(e) = launcher::run_shell(command, env, Path::new(cwd)).await {
                        tracing::warn!("post_command: {e}");
                    }
                }
            }
        }
    }

    /// The macro engine for a launch the UI did not make: bound to the game's unit, it waits on the watcher lock.
    async fn spawn_controller_watch(&self, session_id: &str, game_unit: &str) -> Result<()> {
        let spec = UnitSpec {
            name: format!("universe-controller-{session_id}"),
            description: "Universe controller watch".into(),
            program: paths::self_exe().to_string_lossy().to_string(),
            args: vec!["controller".into(), "watch".into(), "--wait".into()],
            env: passthrough_env(),
            bind_to: Some(game_unit.into()),
            ..Default::default()
        };
        self.host.units.start(&spec).await
    }

    /// The desktop's idle inhibitor for the session: a unit of its own, since the inhibit stands only as long as the connection holding it.
    async fn spawn_keep_awake(&self, session_id: &str, game_unit: &str, title: &str) -> Result<()> {
        let spec = UnitSpec {
            name: format!("universe-awake-{session_id}"),
            description: "Universe keep awake".into(),
            program: paths::self_exe().to_string_lossy().to_string(),
            args: vec!["keep-awake".into(), "--reason".into(), format!("{title} is running")],
            env: passthrough_env(),
            bind_to: Some(game_unit.into()),
            ..Default::default()
        };
        self.host.units.start(&spec).await
    }

    /// Run by `ExecStopPost`, or by `reconcile` for a session left open; idempotent. Without a marker (a reboot) the times come from the unit's log.
    pub async fn session_end(&self, id: &str, session_id: &str, exit: Option<i32>, ended: Option<chrono::DateTime<chrono::Local>>) -> Result<()> {
        let marker = read_marker().filter(|m| m.current.session_id == session_id);
        let filed = match self.get(id).await {
            Err(e) => Err(e),
            Ok(r) if sessions::read(&r.game.sessions_path()).unwrap_or_default().iter().any(|s| s.session == session_id) => Ok(None),
            // A row that could not be appended keeps the marker: `reconcile` files it on the next open.
            Ok(r) => Ok(Some((self.file_session(&r, session_id, exit, ended, marker.as_ref()).await?, r))),
        };
        if let Some(m) = &marker {
            remove_marker();
            self.rollback(&m.undo).await;
        }
        let Some((session, r)) = filed? else { return Ok(()) };
        let cfg = self.config.read().await.clone();
        let mut env_end = match &marker {
            Some(m) => HookEnv { vars: m.hook_env.clone() },
            None => {
                let mut e = self.hook_env_base(&r, &cfg);
                e.set("SESSION_ID", session_id);
                e.set("SESSION_UNIT", session.unit.clone());
                e.set("SESSION_SCREEN", session.screen.clone());
                e.set("SESSION_STARTED_AT", session.started_at.clone());
                e
            }
        };
        env_end.set("SESSION_ENDED_AT", session.ended_at.clone());
        env_end.set("SESSION_DURATION_S", session.duration_s.to_string());
        for m in self.hook_modules(&r, "session-end").await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &env_end);
            if let Err(e) = modules::run_blocking(&m, "session-end", &env).await {
                tracing::warn!("session-end {}: {e}", m.id());
            }
        }
        self.reload_game(id).await?;
        self.post_process(id, session_id).await;
        Ok(())
    }

    async fn file_session(
        &self,
        r: &Resolved,
        session_id: &str,
        exit: Option<i32>,
        ended: Option<chrono::DateTime<chrono::Local>>,
        marker: Option<&Marker>,
    ) -> Result<Session> {
        let unit = marker.map(|m| m.current.unit.clone()).unwrap_or_else(|| format!("{}.service", launcher::unit_name(&r.game.id, session_id)));
        let log = self.host.units.log(&unit).await;
        let started = marker
            .and_then(|m| chrono::DateTime::parse_from_rfc3339(&m.current.started_at).ok().map(|t| t.with_timezone(&chrono::Local)))
            .or(log.started)
            .or_else(|| sessions::parse_session_id(session_id))
            .unwrap_or_else(chrono::Local::now);
        let ended = ended.or(log.ended).unwrap_or_else(chrono::Local::now);
        let session = Session {
            session: session_id.into(),
            game: r.game.id.clone(),
            started_at: started.to_rfc3339(),
            ended_at: ended.to_rfc3339(),
            duration_s: (ended - started).num_seconds().max(0) as u64,
            source: "universe".into(),
            unit,
            screen: marker.map(|m| m.current.screen.clone()).unwrap_or_default(),
            exit: exit.or(log.exit).unwrap_or(-1),
            stopped: Some(marker.is_some_and(|m| m.stopped)),
            command: marker.map(|m| m.command.clone()).unwrap_or_default(),
            ..Default::default()
        };
        sessions::append(&r.game.sessions_path(), &session)?;
        Ok(session)
    }

    /// What a `post-process` hook is told about one session of `r`, whether it just ended or is being written again later.
    pub(crate) fn post_process_env(&self, r: &Resolved, cfg: &Config, session_id: &str) -> HookEnv {
        let sess = r.sessions.iter().find(|s| s.session == session_id).cloned();
        let mut env = self.hook_env_base(r, cfg);
        env.set("SESSION_ID", session_id);
        env.set("RECORDING_PATH", sess.as_ref().and_then(|s| s.recording.clone()).unwrap_or_default());
        env.set("RECORDING_STARTED_AT", sess.as_ref().map(|s| s.recording_started_at.clone()).unwrap_or_default());
        env.set("RECORDING_PAUSES", serde_json::to_string(&sess.as_ref().map(|s| s.recording_pauses.clone()).unwrap_or_default()).unwrap_or_default());
        if let Some(s) = sess {
            env.set("SESSION_STARTED_AT", s.started_at);
            env.set("SESSION_ENDED_AT", s.ended_at);
            env.set("SESSION_DURATION_S", s.duration_s.to_string());
            env.set("SESSION_SCREEN", s.screen);
        }
        env
    }

    async fn post_process(&self, id: &str, session_id: &str) {
        let Ok(r) = self.get(id).await else { return };
        let cfg = self.config.read().await.clone();
        let env = self.post_process_env(&r, &cfg, session_id);
        for m in self.hook_modules(&r, "post-process").await {
            let menv = self.module_env(&m, Some(&r.game), &cfg, &env);
            if let Err(e) = modules::run_async(&self.host.units, &m, "post-process", &menv, session_id, None).await {
                tracing::warn!("post-process {}: {e}", m.id());
            }
        }
    }

    pub async fn stop(&self, session_id: &str) -> Result<()> {
        let Some(c) = self.current().await else { return Err(Error::NotFound("no session running".into())) };
        if !session_id.is_empty() && c.session_id != session_id {
            return Err(Error::NotFound(session_id.into()));
        }
        let term_twice = match self.get(&c.id).await {
            Ok(r) => crate::runners::spec(&r.game.runner_id()).is_none_or(|s| s.term_twice),
            Err(_) => true,
        };
        update_marker(|m| m.stopped = true)?;
        self.host.units.stop(&c.unit, term_twice).await
    }

    pub async fn freeze(&self, on: bool) -> Result<()> {
        let _in_order = self.freezes.lock().await;
        let Some(marker) = read_marker() else { return Err(Error::NotFound("no session running".into())) };
        if !self.host.units.is_active(&marker.current.unit).await {
            return Err(Error::NotFound("no session running".into()));
        }
        self.host.units.freeze(&marker.current.unit, on).await?;
        let hook = if on { "freeze" } else { "thaw" };
        let Ok(r) = self.get(&marker.current.id).await else { return Ok(()) };
        let cfg = self.config.read().await.clone();
        let base = HookEnv { vars: marker.hook_env };
        for m in self.hook_modules(&r, hook).await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &base);
            if let Err(e) = modules::run_blocking(&m, hook, &env).await {
                tracing::warn!("{hook} {}: {e}", m.id());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::host::{Host, Memory};
    use std::sync::Arc;

    /// A library of one Dolphin game on a fake emulator, every Universe home under one tempdir.
    struct Sandbox {
        _dir: tempfile::TempDir,
        post_ran: std::path::PathBuf,
    }

    fn sandbox() -> Sandbox {
        let dir = tempfile::tempdir().unwrap();
        for (var, sub) in [
            ("UNIVERSE_DATA_HOME", "data"),
            ("UNIVERSE_STATE_HOME", "state"),
            ("UNIVERSE_CONFIG_HOME", "config"),
            ("UNIVERSE_MODULES_PATH", "modules"),
            ("UNIVERSE_SOURCES_PATH", "sources"),
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::env::set_var(var, dir.path().join(sub));
        }
        std::fs::write(dir.path().join("config/config.toml"), "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = []\n")
            .unwrap();
        let emu = dir.path().join("dolphin-emu");
        std::fs::write(&emu, b"#!/bin/sh\n").unwrap();
        let rom = dir.path().join("F-Zero GX.iso");
        std::fs::write(&rom, b"").unwrap();
        let post_ran = dir.path().join("post-ran");
        let mut g = Game::new("Sample");
        g.launch.runner = "dolphin".into();
        g.launch.runner_exe = emu.to_string_lossy().into();
        g.launch.exe = rom.to_string_lossy().into();
        g.launch.pre_command = "true".into();
        g.launch.post_command = format!("touch {}", post_ran.display());
        g.desktop.hide_cursor = Some(true);
        g.save().unwrap();
        Sandbox { _dir: dir, post_ran }
    }

    async fn open() -> (Core, Arc<Memory>) {
        let (host, memory) = Host::memory();
        (Core::open_with(crate::config::Config::load().unwrap(), host).await.unwrap(), memory)
    }

    fn undo_steps(m: &Marker) -> Vec<&'static str> {
        m.undo
            .iter()
            .map(|u| match u {
                Undo::PostCommand { .. } => "post_command",
                Undo::Pads => "pads",
                Undo::Cursor { .. } => "cursor",
                Undo::Hud => "hud",
            })
            .collect()
    }

    #[tokio::test]
    async fn a_session_is_launched_ended_and_undone_in_reverse() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        let spec = memory.spec(&unit).expect("the game unit started");
        assert!(spec.properties.contains(&("ExitType".to_string(), Prop::Str("cgroup".into()))));
        assert!(spec.properties.contains(&("TimeoutStopUSec".to_string(), Prop::U64(60_000_000))), "{:?}", spec.properties);
        assert_eq!(spec.stop_post[0], paths::self_exe().to_string_lossy());
        assert_eq!(spec.stop_post[1..], ["session-end".to_string(), "sample".into(), sid.clone()]);
        assert!(spec.bind_to.is_none(), "no scope adopted");
        assert_eq!(spec.env["UNIVERSE_BIN"], paths::self_exe().to_string_lossy());
        assert_eq!(memory.spec(&format!("universe-controller-{sid}")).expect("the watcher").bind_to.as_deref(), Some(unit.as_str()));
        let marker = read_marker().expect("a marker");
        assert_eq!((marker.current.session_id.as_str(), marker.current.unit.as_str()), (sid.as_str(), unit.as_str()));
        assert_eq!(undo_steps(&marker), ["post_command", "pads", "cursor"]);
        assert_eq!(marker.undo[2], Undo::Cursor { was_active: false });
        assert_eq!(core.current().await.map(|c| c.id).as_deref(), Some("sample"));
        assert!(!sb.post_ran.exists());

        memory.finish(&unit, 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let rows = sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].session.as_str(), rows[0].exit, rows[0].unit.as_str()), (sid.as_str(), 0, unit.as_str()));
        assert_eq!(rows[0].started_at, marker.current.started_at);
        assert_eq!(rows[0].ended_at, core.host.units.log(&unit).await.ended.unwrap().to_rfc3339());
        assert!(read_marker().is_none(), "the marker is gone");
        assert!(sb.post_ran.exists(), "post_command ran");
        let calls = memory.calls();
        assert_eq!(calls[..3], ["pads:engage".to_string(), "cursor:enable".into(), format!("unit:start {unit}")]);
        assert_eq!(calls[calls.len() - 2..], ["cursor:restore(false)", "pads:release"], "{calls:?}");

        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap().len(), 1, "idempotent");
    }

    #[tokio::test]
    async fn the_desktop_is_kept_awake_for_as_long_as_the_game_runs() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let awake = memory.spec(&format!("universe-awake-{sid}")).expect("the inhibitor");
        assert_eq!(awake.bind_to.as_deref(), Some(format!("universe-game-sample-{sid}.service").as_str()), "it is released with the game");
        assert_eq!(awake.args[0], "keep-awake");
    }

    #[tokio::test]
    async fn keep_awake_off_leaves_the_desktop_to_its_own_idle() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let file = crate::paths::config_file();
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{text}[desktop]\nkeep_awake = false\n")).unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        assert!(memory.spec(&format!("universe-awake-{sid}")).is_none(), "off, nothing holds the desktop awake");
    }

    #[tokio::test]
    async fn a_game_that_reads_the_raw_pads_frees_inputplumber_rather_than_engaging() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        // Opting the emulator out of the composite is the same path a Proton or native game takes.
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.options.insert("inputplumber".into(), toml::Value::Boolean(false));
        g.save().unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        let calls = memory.calls();
        assert!(calls.contains(&"pads:ensure_free".to_string()), "the raw pads are freed before launch: {calls:?}");
        assert!(!calls.contains(&"pads:engage".to_string()), "no composite is taken: {calls:?}");
        assert!(!undo_steps(&read_marker().unwrap()).contains(&"pads"), "nothing engaged, so nothing to hand back");

        memory.finish(&unit, 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(!memory.calls().contains(&"pads:release".to_string()), "release only undoes an engage");
    }

    #[tokio::test]
    async fn the_hud_toggle_is_the_games_key_and_the_layers_conf() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, _memory) = open().await;
        assert!(matches!(core.set_mangohud(None).await, Err(Error::NotFound(_))), "no game, nothing to show");
        core.launch("sample", "", "").await.unwrap();
        let conf = launcher::layer_conf_path();
        assert!(
            std::fs::read_to_string(&conf).map(|t| t.contains("no_display\n")).unwrap_or(true),
            "off by config, hidden: {:?}",
            std::fs::read_to_string(&conf)
        );

        assert!(core.set_mangohud(None).await.unwrap(), "off flips on");
        assert_eq!(core.get("sample").await.unwrap().game.launch.mangohud, Some(true), "written as the game's own key");
        let text = std::fs::read_to_string(&conf).unwrap();
        assert!(!text.contains("no_display") && text.contains("control=universe-mangohud-sample\n"), "{text}");

        crate::game::set_key(&core.get("sample").await.unwrap().game.toml_path(), "launch.mangohud", "false").unwrap();
        assert!(core.set_mangohud(None).await.unwrap(), "a key another process wrote is read before the flip");
        assert!(!core.set_mangohud(Some(false)).await.unwrap());
        let text = std::fs::read_to_string(&conf).unwrap();
        assert!(text.contains("no_display\n") && text.contains("control=universe-mangohud-sample\n"), "hidden, still the desktop's HUD: {text}");
    }

    #[tokio::test]
    async fn reconcile_closes_a_session_whose_unit_vanished() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.drop_unit(&format!("universe-game-sample-{sid}.service"));
        assert!(core.current().await.is_none());
        core.reconcile().await.unwrap();
        let rows = sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].session.as_str(), rows[0].exit), (sid.as_str(), -1));
        assert!(read_marker().is_none());
        assert!(sb.post_ran.exists());
        let calls = memory.calls();
        assert_eq!(calls[calls.len() - 2..], ["cursor:restore(false)", "pads:release"]);
        assert!(core.launch("sample", "", "").await.is_ok(), "the next launch is not Busy");
    }

    #[tokio::test]
    async fn a_refused_start_leaves_nothing_behind() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let sb = sandbox();
        let (core, memory) = open().await;
        memory.refuse_starts(true);
        let err = core.launch("sample", "", "").await.unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err}");
        assert!(read_marker().is_none(), "no marker left");
        assert!(sb.post_ran.exists(), "post_command undoes pre_command");
        let calls = memory.calls();
        assert_eq!(calls[..2], ["pads:engage", "cursor:enable"]);
        assert!(calls[2].starts_with("unit:start universe-game-sample-"), "{calls:?}");
        assert_eq!(calls[3..], ["cursor:restore(false)", "pads:release"]);
        memory.refuse_starts(false);
        assert!(core.launch("sample", "", "").await.is_ok());
    }

    #[tokio::test]
    async fn adopt_scope_on_the_memory_host_is_idempotent() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let name = core.adopt_scope().await.unwrap();
        assert_eq!(name, format!("universe-launcher-{}.scope", std::process::id()));
        assert_eq!(core.adopt_scope().await.unwrap(), name);
        assert_eq!(memory.calls().iter().filter(|c| c.starts_with("unit:adopt_scope")).count(), 1);
        let sid = core.launch("sample", "", "").await.unwrap();
        assert_eq!(memory.spec(&format!("universe-game-sample-{sid}.service")).unwrap().bind_to.as_deref(), Some(name.as_str()));
    }

    #[tokio::test]
    async fn a_stop_files_a_stopped_end_and_the_log_opens_on_the_command_line() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        let command = read_marker().unwrap().command;
        assert!(command.contains("dolphin-emu") && command.contains("F-Zero"), "{command}");
        memory.write_line(&unit, "dolphin-emu", "booting");
        let live = core.session_log("sample", "", 0).await.unwrap();
        assert_eq!(live.iter().map(|l| l.message.as_str()).collect::<Vec<_>>(), [format!("launch {sid}: {command}").as_str(), "booting"]);
        assert_eq!(live[0].source, "universe");
        assert_eq!(core.session_log("sample", "", 1).await.unwrap().len(), 2, "the tail keeps the command line");

        core.stop("").await.unwrap();
        core.session_end("sample", &sid, None, None).await.unwrap();
        let row = &core.sessions("sample").await.unwrap()[0];
        assert!(row.session.stopped == Some(true) && row.session.exit == 143, "{:?}", row.session);
        assert_eq!(sessions::end_of(&row.session), "stopped");
        assert_eq!(row.session.command, command);
        let json = serde_json::to_value(row).unwrap();
        assert_eq!((json["end"].as_str(), json["debug_log"].is_null(), json.get("command")), (Some("stopped"), true, None));
        let after = core.session_log("sample", &sid, 0).await.unwrap();
        assert_eq!(after[0].message, format!("launch {sid}: {command}"));
        assert!(matches!(core.session_log("sample", "19700101-000000", 0).await, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn an_unasked_signal_is_a_kill_and_a_code_a_crash() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 11);
        core.session_end("sample", &sid, Some(11), None).await.unwrap();
        assert_eq!(sessions::end_of(&core.sessions("sample").await.unwrap()[0].session), "crashed");
    }

    #[tokio::test]
    async fn a_signal_nobody_asked_for_is_a_kill() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), -1);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let rows = core.sessions("sample").await.unwrap();
        assert_eq!((rows[0].session.exit, sessions::end_of(&rows[0].session)), (-1, "killed"));
    }

    #[tokio::test]
    async fn debug_log_makes_the_session_dir_and_a_launch_file() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.debug_log = Some(true);
        g.save().unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let dir = paths::session_log_dir("sample", &sid);
        let launch = std::fs::read_to_string(dir.join("launch.txt")).unwrap();
        assert!(launch.starts_with(&read_marker().unwrap().command), "{launch}");
        assert!(launch.contains("\nUNIVERSE_BIN="), "the env follows");
        assert!(!launch.contains("PROTON_LOG"), "an emulator gets no Proton log");
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let json = serde_json::to_value(core.sessions("sample").await.unwrap()).unwrap();
        assert_eq!(json[0]["debug_log"].as_str(), Some(dir.to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn current_is_none_once_the_unit_finished() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        assert!(matches!(core.launch("sample", "", "").await, Err(Error::Busy(_))));
        memory.finish(&format!("universe-game-sample-{sid}.service"), 3);
        assert!(core.current().await.is_none());
        assert!(read_marker().is_some(), "the marker waits for session-end");
        assert!(matches!(core.stop("").await, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn freeze_runs_the_hooks_behind_the_unit() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        let probe = std::path::PathBuf::from(std::env::var_os("UNIVERSE_MODULES_PATH").unwrap()).join("probe");
        std::fs::create_dir_all(probe.join("bin")).unwrap();
        std::fs::write(probe.join("module.toml"), "api = 2\nid = \"probe\"\n[hooks]\nfreeze = \"bin/freeze\"\nthaw = \"bin/thaw\"\n").unwrap();
        let log = paths::state_home().join("hooks.log");
        use std::os::unix::fs::PermissionsExt;
        for hook in ["freeze", "thaw"] {
            let exe = probe.join("bin").join(hook);
            std::fs::write(&exe, format!("#!/bin/sh\necho {hook} $SESSION_ID $MODULE_SETTINGS_JSON >> {}\n", log.display())).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            paths::config_home().join("config.toml"),
            "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = [\"probe\"]\n",
        )
        .unwrap();
        let (core, memory) = open().await;
        assert!(matches!(core.freeze(true).await, Err(Error::NotFound(_))));
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        core.freeze(true).await.unwrap();
        core.freeze(false).await.unwrap();
        let calls = memory.calls();
        assert!(calls.contains(&format!("unit:freeze {unit}")) && calls.contains(&format!("unit:thaw {unit}")), "{calls:?}");
        let lines: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with(&format!("freeze {sid} {{")) && lines[1].starts_with(&format!("thaw {sid} {{")), "{lines:?}");
    }

    #[tokio::test]
    async fn reconcile_drops_a_marker_it_cannot_read() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        std::fs::write(paths::current_session_file(), "{\"session_id\": 1").unwrap();
        let (core, _memory) = open().await;
        assert!(!paths::current_session_file().exists());
        assert!(core.launch("sample", "", "").await.is_ok());
    }
}
