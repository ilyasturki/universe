use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::{passthrough_env, Core};
use crate::host::UnitSpec;
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
}

/// What `launch` began, in begin order; undone in reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Undo {
    Pads,
    Cursor { was_active: bool },
    PostCommand { command: String, cwd: String, env: BTreeMap<String, String> },
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

impl Core {
    /// A marker whose unit the manager still reports as active.
    pub async fn current(&self) -> Option<Current> {
        let m = read_marker()?;
        self.host.units.is_active(&m.current.unit).await.then_some(m.current)
    }

    /// A marker without an active unit is a session whose `session-end` never ran (crash, reboot): close it now.
    /// One that cannot be read goes too, or every later launch would be `Busy`.
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
        if let Some(name) = self.scope.lock().unwrap().clone() {
            return Ok(name);
        }
        let name = self.host.units.adopt_scope(std::process::id()).await?;
        *self.scope.lock().unwrap() = Some(name.clone());
        Ok(name)
    }

    /// Starts the game as a transient service and returns; the manager runs `universe session-end` when its cgroup empties.
    /// `splash` is a poster the frontend grabbed for gamescope's keep-alive window (`splash.rs`'s format), `""` for none.
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

        let mode = crate::desktop::screen_mode(&screen).await;
        let splash = (!splash.is_empty()).then(|| std::path::PathBuf::from(splash));
        let plan = launcher::plan(&r, &cfg, &session_id, &extra_env, mode, splash.as_deref())?;
        if let Some((path, text)) = &plan.mangohud_conf {
            std::fs::write(path, text)?;
        }

        let current = Current { session_id: session_id.clone(), id: id.into(), title: r.game.title.clone(), unit: unit.clone(), screen: screen.clone(), started_at: started.to_rfc3339() };
        let mut undo = Vec::new();
        if let Err(e) = self.begin(&r, &plan, &current, base.vars.clone(), &mut undo).await {
            if read_marker().is_some_and(|m| m.current.session_id == session_id) {
                remove_marker();
            }
            self.rollback(&undo).await;
            return Err(e);
        }
        for m in self.hook_modules(&r, "post-launch").await {
            let env = self.module_env(&m, &r, &cfg, &base);
            if let Err(e) = modules::run_async(&self.host.units, &m, "post-launch", &env, &session_id, Some(&unit)).await {
                tracing::warn!("post-launch {}: {e}", m.id());
            }
        }
        if cfg.controller.enabled {
            if let Err(e) = self.spawn_controller_watch(&session_id, &unit).await {
                tracing::warn!("controller watch: {e}");
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
        if r.effective.inputplumber && self.host.pads.engage().await {
            undo.push(Undo::Pads);
        }
        if r.effective.hide_cursor {
            let was_active = self.host.shell.cursor_enable().await;
            undo.push(Undo::Cursor { was_active });
        }
        write_marker(&Marker { current: current.clone(), hook_env, undo: undo.clone() })?;
        tracing::info!("launch {}: {}", r.game.id, plan.command_line());
        let budget: u64 = 60 + self.hook_modules(r, "session-end").await.iter().map(|m| m.timeout().as_secs()).sum::<u64>();
        let mut env = passthrough_env();
        env.extend(plan.env.clone());
        let bind_to = self.scope.lock().unwrap().clone();
        let spec = UnitSpec {
            name: current.unit.clone(),
            program: plan.program.clone(),
            args: plan.args.clone(),
            env,
            cwd: Some(plan.cwd.clone()),
            // ExitType=cgroup: the unit ends with the last game process, not with the one systemd-run started.
            properties: vec![("ExitType".into(), "cgroup".into()), ("TimeoutStopSec".into(), budget.to_string())],
            bind_to,
            stop_post: vec![paths::self_exe().to_string_lossy().to_string(), "session-end".into(), current.id.clone(), current.session_id.clone()],
        };
        self.host.units.start(&spec).await
    }

    async fn rollback(&self, undo: &[Undo]) {
        for step in undo.iter().rev() {
            match step {
                Undo::Cursor { was_active } => self.host.shell.cursor_restore(*was_active).await,
                Undo::Pads => self.host.pads.release().await,
                Undo::PostCommand { command, cwd, env } => {
                    if let Err(e) = launcher::run_shell(command, env, Path::new(cwd)).await {
                        tracing::warn!("post_command: {e}");
                    }
                }
            }
        }
    }

    async fn close_marker(&self, m: &Marker) {
        remove_marker();
        self.rollback(&m.undo).await;
    }

    /// The macro engine for a launch the UI did not make: bound to the game's unit, it waits on the
    /// watcher lock, so it only reads the pads once no launcher does.
    async fn spawn_controller_watch(&self, session_id: &str, game_unit: &str) -> Result<()> {
        let spec = UnitSpec {
            name: format!("universe-controller-{session_id}"),
            program: paths::self_exe().to_string_lossy().to_string(),
            args: vec!["controller".into(), "watch".into(), "--wait".into()],
            env: passthrough_env(),
            bind_to: Some(game_unit.into()),
            ..Default::default()
        };
        self.host.units.start(&spec).await
    }

    /// Closes a session: run by systemd's `ExecStopPost`, or by `reconcile` for one that was left open. Idempotent.
    /// Without a marker (a reboot) the times come from the unit's log and the session id.
    pub async fn session_end(&self, id: &str, session_id: &str, exit: Option<i32>, ended: Option<chrono::DateTime<chrono::Local>>) -> Result<()> {
        let marker = read_marker().filter(|m| m.current.session_id == session_id);
        let r = match self.get(id).await {
            Ok(r) => r,
            Err(e) => {
                if let Some(m) = &marker {
                    self.close_marker(m).await;
                }
                return Err(e);
            }
        };
        if sessions::read(&r.game.sessions_path()).unwrap_or_default().iter().any(|s| s.session == session_id) {
            if let Some(m) = &marker {
                self.close_marker(m).await;
            }
            return Ok(());
        }
        let cfg = self.config.read().await.clone();
        let unit = marker.as_ref().map(|m| m.current.unit.clone()).unwrap_or_else(|| format!("{}.service", launcher::unit_name(id, session_id)));
        let log = self.host.units.log(&unit).await;
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
        if let Some(m) = &marker {
            self.close_marker(m).await;
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
        self.host.units.stop(&c.unit).await
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
        for (var, sub) in [("UNIVERSE_DATA_HOME", "data"), ("UNIVERSE_STATE_HOME", "state"), ("UNIVERSE_CONFIG_HOME", "config"), ("UNIVERSE_MODULES_PATH", "modules")] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::env::set_var(var, dir.path().join(sub));
        }
        std::fs::write(dir.path().join("config/config.toml"), "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = []\n").unwrap();
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
        assert!(spec.properties.contains(&("ExitType".to_string(), "cgroup".to_string())));
        assert!(spec.properties.contains(&("TimeoutStopSec".to_string(), "60".to_string())), "{:?}", spec.properties);
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
        assert_eq!(rows[0].ended_at, memory.log_of(&unit).ended.unwrap().to_rfc3339());
        assert!(read_marker().is_none(), "the marker is gone");
        assert!(sb.post_ran.exists(), "post_command ran");
        let calls = memory.calls();
        assert_eq!(calls[..3], ["pads:engage".to_string(), "cursor:enable".into(), format!("unit:start {unit}")]);
        assert_eq!(calls[calls.len() - 2..], ["cursor:restore(false)", "pads:release"], "{calls:?}");

        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap().len(), 1, "idempotent");
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
    async fn reconcile_drops_a_marker_it_cannot_read() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let _sb = sandbox();
        std::fs::write(paths::current_session_file(), "{\"session_id\": 1").unwrap();
        let (core, _memory) = open().await;
        assert!(!paths::current_session_file().exists());
        assert!(core.launch("sample", "", "").await.is_ok());
    }
}
