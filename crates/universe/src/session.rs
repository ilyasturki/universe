use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::{passthrough_env, Core};
use crate::host::{Prop, UnitSpec};
use crate::launcher::{self, Plan};
use crate::library::Resolved;
use crate::modules::{self, HookEnv};
use crate::paths;
use crate::sessions::{self, Session};
use crate::{Error, Result};

const OVERLAY_PROGRAM: &str = "universe-ui";

/// Where splash records the display of a game's own gamescope for the overlay.
fn display_file(session_id: &str) -> std::path::PathBuf {
    paths::runtime_dir().join(format!("display-{session_id}"))
}

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
    /// The process that asked for the launch (Universe Desktop's): the window the overlay's Home raises.
    #[serde(default)]
    pub app_pid: u32,
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
    /// None in a marker written before it existed: the game's settings stand in.
    #[serde(default)]
    pub runtime: Option<Runtime>,
    /// Its unit not started yet: while its launcher holds the starting lock, no one else closes it.
    #[serde(default)]
    pub starting: bool,
}

/// What HOME changed in the running game alone, seeded from its settings at launch: the next launch starts from them again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Runtime {
    pub mangohud: bool,
    pub fps_limit: String,
    pub gamescope_filter: String,
    pub gamescope_sharpness: Option<u32>,
}

impl Runtime {
    pub fn of(e: &crate::library::Effective) -> Runtime {
        Runtime {
            mangohud: e.mangohud,
            fps_limit: e.fps_limit.clone(),
            gamescope_filter: e.gamescope_fields.filter.clone(),
            gamescope_sharpness: e.gamescope_fields.sharpness,
        }
    }

    /// The launcher's gamescope holds `[launch]`'s filter: a game's own reaches only one started for it.
    pub fn at_launch(e: &crate::library::Effective, nested: bool, launch: &crate::config::LaunchDefaults) -> Runtime {
        match nested {
            true => Runtime { gamescope_filter: launch.gamescope_filter.clone(), gamescope_sharpness: launch.gamescope_sharpness, ..Runtime::of(e) },
            false => Runtime::of(e),
        }
    }
}

/// What `launch` began, in begin order; undone in reverse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Undo {
    Cursor {
        #[serde(default)]
        restore: crate::desktop::CursorUndo,
    },
    PostCommand {
        command: String,
        cwd: String,
        env: BTreeMap<String, String>,
    },
    Hud,
    Filter,
    /// The machine's controls the game's own changed, by control id: the values its end puts back.
    System {
        before: BTreeMap<String, String>,
    },
}

pub fn read_marker() -> Option<Marker> {
    let s = std::fs::read_to_string(paths::current_session_file()).ok()?;
    serde_json::from_str(&s).ok()
}

/// Whole or not there at all: written aside, then linked in, which refuses a marker already there.
fn write_marker(m: &Marker) -> Result<()> {
    use std::io::Write;
    let p = paths::current_session_file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension(format!("json.{}", std::process::id()));
    let linked = std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(serde_json::to_string(m)?.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| std::fs::hard_link(&tmp, &p));
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let other = read_marker().map(|m| format!("{} ({})", m.current.title, m.current.session_id)).unwrap_or_else(|| "another launch".into());
            Err(Error::Busy(format!("{other} is running")))
        }
        Err(e) => Err(e.into()),
    }
}

fn remove_marker() {
    let _ = std::fs::remove_file(paths::current_session_file());
}

/// Every process holds it to change or take the marker: a change and the session's end never interleave.
fn marker_lock() -> Result<std::fs::File> {
    let p = paths::current_session_file().with_extension("lock");
    std::fs::create_dir_all(p.parent().unwrap())?;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(p)?;
    f.lock()?;
    Ok(f)
}

fn starting_lock_file() -> std::path::PathBuf {
    paths::current_session_file().with_extension("starting")
}

/// Held by a launch from its marker's write until its unit started; another launch meanwhile is `Busy` at once.
fn hold_starting() -> Result<std::fs::File> {
    let p = starting_lock_file();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(p)?;
    match f.try_lock() {
        Ok(()) => Ok(f),
        Err(std::fs::TryLockError::WouldBlock) => {
            let other = read_marker().map(|m| format!("{} ({})", m.current.title, m.current.session_id)).unwrap_or_else(|| "another launch".into());
            Err(Error::Busy(format!("{other} is starting")))
        }
        Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
    }
}

/// A live launch holds the starting lock; a free one means its launcher died before the unit started.
fn still_starting() -> bool {
    let Ok(f) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(starting_lock_file()) else { return false };
    matches!(f.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// What the launch is about to begin, on its marker before it begins.
fn note_undo(session_id: &str, step: Undo) -> Result<()> {
    let mut kept = false;
    update_marker(session_id, |m| {
        m.undo.push(step);
        kept = true;
    })?;
    if !kept {
        return Err(Error::Io(format!("the marker of session {session_id} is gone")));
    }
    Ok(())
}

/// A session being closed, claimed once from its marker: each step is noted as it completes, so a close cut short
/// resumes where it stopped and repeats nothing it finished.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Closing {
    row: Session,
    #[serde(default)]
    filed: bool,
    /// What is left to undo, in begin order.
    #[serde(default)]
    undo: Vec<Undo>,
    /// None for a session closed without its marker: the session-end hooks get the game's own environment.
    #[serde(default)]
    hook_env: Option<Vec<(String, String)>>,
    /// The session-end hooks ran and the saves backup started.
    #[serde(default)]
    ended: bool,
    /// A launch whose unit never started: its undo alone, no row, no hooks.
    #[serde(default)]
    failed_launch: bool,
}

fn closing_file(game: &str, session_id: &str) -> std::path::PathBuf {
    paths::closing_sessions_dir().join(format!("{session_id}.{game}.json"))
}

fn write_closing(c: &Closing) -> std::io::Result<()> {
    use std::io::Write;
    let p = closing_file(&c.row.game, &c.row.session);
    std::fs::create_dir_all(paths::closing_sessions_dir())?;
    let tmp = p.with_extension("tmp");
    let kept = std::fs::File::create(&tmp)
        .and_then(|mut f| f.write_all(serde_json::to_string(c)?.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| std::fs::rename(&tmp, &p));
    if kept.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    kept
}

fn read_closing(file: &Path) -> Option<Closing> {
    let c = std::fs::read(file).map_err(|e| e.to_string()).and_then(|text| serde_json::from_slice(&text).map_err(|e| e.to_string()));
    c.inspect_err(|e| tracing::warn!("{}: {e}", file.display())).ok()
}

/// Oldest first.
fn closings() -> Vec<Closing> {
    let Ok(dir) = std::fs::read_dir(paths::closing_sessions_dir()) else { return vec![] };
    let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    files.iter().filter_map(|p| read_closing(p)).collect()
}

pub(crate) fn closing_game(session_id: &str) -> Option<String> {
    closings().into_iter().find(|c| c.row.session == session_id && !c.filed).map(|c| c.row.game)
}

/// The record changed under the marker's lock, re-read so no writer's change is lost, and written only when `f` says it changed it;
/// `None` when it is gone.
fn change_closing(game: &str, session_id: &str, f: impl FnOnce(&mut Closing) -> bool) -> Result<Option<Closing>> {
    let file = closing_file(game, session_id);
    let _lock = marker_lock()?;
    if !file.exists() {
        return Ok(None);
    }
    let Some(mut c) = read_closing(&file) else { return Ok(None) };
    if f(&mut c) {
        write_closing(&c)?;
    }
    Ok(Some(c))
}

/// `false` when no row of the session waits: it is in its history, even with the record's note of it refused, or was never kept.
pub(crate) fn change_unfiled(game: &str, session_id: &str, f: impl FnOnce(&mut Session)) -> Result<bool> {
    let in_history = || sessions::read(&paths::game_dir(game).join("sessions.jsonl")).is_ok_and(|rows| rows.iter().any(|s| s.session == session_id));
    let mut changed = false;
    change_closing(game, session_id, |c| {
        if !c.filed && !in_history() {
            f(&mut c.row);
            changed = true;
        }
        changed
    })?;
    Ok(changed)
}

/// The record with its own lock held, for one process at a time to run its steps: `None` when it is gone or another
/// process holds it. Both files go together under the marker's lock, so no one locks a lock file already let go.
fn hold_closing(game: &str, session_id: &str) -> Result<Option<(std::fs::File, Closing)>> {
    let file = closing_file(game, session_id);
    let _lock = marker_lock()?;
    if !file.exists() {
        return Ok(None);
    }
    let held = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(file.with_extension("lock"))?;
    match held.try_lock() {
        Ok(()) => Ok(read_closing(&file).map(|c| (held, c))),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
    }
}

/// The row into its history, once: the history read under the same lock shows whether it is there already. A game with no folder takes none.
/// The inner `Err`: the row is in, but the record refused the note.
fn file_closing(game: &str, session_id: &str) -> Result<std::io::Result<()>> {
    let file = closing_file(game, session_id);
    let _lock = marker_lock()?;
    let Some(mut c) = read_closing(&file).filter(|c| !c.filed) else { return Ok(Ok(())) };
    let dir = paths::game_dir(game);
    if std::fs::metadata(&dir).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        tracing::warn!("session {session_id} of {game} dropped: the game has no folder");
    } else {
        // A history it cannot read cannot show the row is not there already.
        sessions::append_new(&dir.join("sessions.jsonl"), &c.row)?;
    }
    c.filed = true;
    Ok(write_closing(&c))
}

/// How a close is claimed: from a session that ended (`seen` its marker before the journal read; `ran` per the journal), or by the
/// launch whose start failed.
#[derive(Clone, Copy)]
enum Claim {
    Ended { seen: bool, ran: bool },
    FailedLaunch,
}

/// The session's close, claimed by one process under the marker's lock: its record written, then its marker removed.
/// A record already there, or a marker `seen` before the journal read and gone since, means another process claimed it.
/// Without a marker the row goes only into a history that reads without it. A marker still starting is its launcher's alone.
fn claim_closing(game: &str, session_id: &str, claim: Claim, row: impl FnOnce(Option<&Marker>) -> Session) -> Result<()> {
    let _lock = marker_lock()?;
    let marker = read_marker().filter(|m| m.current.session_id == session_id);
    if closing_file(game, session_id).exists() {
        if marker.is_some() {
            remove_marker();
        }
        return Ok(());
    }
    let never_started = |m: &Marker| match claim {
        Claim::FailedLaunch => Some(true),
        Claim::Ended { .. } if m.starting && still_starting() => None,
        Claim::Ended { ran, .. } => Some(m.starting && !ran),
    };
    let c = match &marker {
        Some(m) => match never_started(m) {
            None => return Ok(()),
            Some(true) => Closing {
                row: Session { session: session_id.into(), game: game.into(), ..Session::default() },
                filed: true,
                undo: m.undo.clone(),
                ended: true,
                failed_launch: true,
                ..Closing::default()
            },
            Some(false) => Closing { row: row(Some(m)), undo: m.undo.clone(), hook_env: Some(m.hook_env.clone()), ..Closing::default() },
        },
        None if matches!(claim, Claim::FailedLaunch | Claim::Ended { seen: true, .. }) => return Ok(()),
        None => {
            let dir = paths::game_dir(game);
            let unfiled = dir.is_dir() && sessions::read(&dir.join("sessions.jsonl")).is_ok_and(|rows| rows.iter().all(|s| s.session != session_id));
            if !unfiled {
                return Ok(());
            }
            Closing { row: row(None), ..Closing::default() }
        }
    };
    write_closing(&c).map_err(|e| {
        let title = marker.as_ref().map_or(game, |m| m.current.title.as_str());
        Error::Io(format!(
            "the last session of {title} could not be closed: {} refuses its record ({e}). Free some disk space or make that folder writable, then try again",
            paths::closing_sessions_dir().display()
        ))
    })?;
    remove_marker();
    Ok(())
}

/// The row of a session that just ended, from its marker when there is one, else from the unit's log.
fn closed_session(
    game: &str,
    session_id: &str,
    exit: Option<i32>,
    ended: Option<chrono::DateTime<chrono::Local>>,
    marker: Option<&Marker>,
    log: &crate::host::UnitLog,
) -> Session {
    let unit = marker.map(|m| m.current.unit.clone()).unwrap_or_else(|| format!("{}.service", launcher::unit_name(game, session_id)));
    let started = marker
        .and_then(|m| chrono::DateTime::parse_from_rfc3339(&m.current.started_at).ok().map(|t| t.with_timezone(&chrono::Local)))
        .or(log.started)
        .or_else(|| sessions::parse_session_id(session_id))
        .unwrap_or_else(chrono::Local::now);
    let ended = ended.or(log.ended).unwrap_or_else(chrono::Local::now);
    let log_dir = paths::session_log_dir(game, session_id);
    let crashed = launcher::crash_report_in(&log_dir);
    // Made for a crash report that never came: no debug_log on the row.
    let _ = std::fs::remove_dir(&log_dir);
    Session {
        session: session_id.into(),
        game: game.into(),
        started_at: started.to_rfc3339(),
        ended_at: ended.to_rfc3339(),
        duration_s: (ended - started).num_seconds().max(0) as u64,
        source: "universe".into(),
        unit,
        screen: marker.map(|m| m.current.screen.clone()).unwrap_or_default(),
        exit: exit.or(log.exit).unwrap_or(-1),
        stopped: Some(marker.is_some_and(|m| m.stopped)),
        crashed,
        command: marker.map(|m| m.command.clone()).unwrap_or_default(),
        ..Default::default()
    }
}

/// A change to the session's marker, in place and only while it is that session's: a temp file renamed over it, so
/// `session-end` never reads half a marker.
fn update_marker(session_id: &str, f: impl FnOnce(&mut Marker)) -> Result<()> {
    let _lock = marker_lock()?;
    let Some(mut m) = read_marker().filter(|m| m.current.session_id == session_id) else { return Ok(()) };
    f(&mut m);
    let p = paths::current_session_file();
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(&m)?)?;
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

/// `None` when the marker is gone or another session's.
pub(crate) fn change_runtime(session_id: &str, seed: impl FnOnce() -> Runtime, f: impl FnOnce(&mut Runtime)) -> Result<Option<Runtime>> {
    let mut changed = None;
    update_marker(session_id, |m| {
        let rt = m.runtime.get_or_insert_with(seed);
        f(rt);
        changed = Some(rt.clone());
    })?;
    Ok(changed)
}

/// HOME's filter in the running game: its end puts the launcher's gamescope back on the settings' filter.
pub(crate) fn change_filter(session_id: &str, seed: impl FnOnce() -> Runtime, filter: &str, sharpness: Option<u32>) -> Result<()> {
    update_marker(session_id, |m| {
        let rt = m.runtime.get_or_insert_with(seed);
        rt.gamescope_filter = filter.to_owned();
        rt.gamescope_sharpness = sharpness;
        if !m.undo.contains(&Undo::Filter) {
            m.undo.push(Undo::Filter);
        }
    })
}

/// A control the running game's own just changed: the first value noted is the one its end puts back.
pub(crate) fn system_before(session_id: &str, id: &str, value: &str) -> Result<()> {
    update_marker(session_id, |m| match m.undo.iter_mut().find_map(|u| if let Undo::System { before } = u { Some(before) } else { None }) {
        Some(before) => {
            before.entry(id.into()).or_insert_with(|| value.into());
        }
        None => m.undo.push(Undo::System { before: BTreeMap::from([(id.into(), value.into())]) }),
    })
}

/// A new value of the machine's own for a control the running game changed: its end puts that one back.
pub(crate) fn system_ends_at(session_id: &str, id: &str, value: &str) -> Result<()> {
    update_marker(session_id, |m| {
        for u in &mut m.undo {
            if let Undo::System { before } = u {
                if let Some(v) = before.get_mut(id) {
                    *v = value.into();
                }
            }
        }
    })
}

impl Core {
    pub async fn current(&self) -> Option<Current> {
        let m = read_marker()?;
        self.host.units.is_active(&m.current.unit).await.then_some(m.current)
    }

    /// The game running, or still starting (`true`): what nothing may take its files from.
    pub(crate) async fn occupant(&self) -> Result<Option<(Current, bool)>> {
        let Some(m) = read_marker() else { return Ok(None) };
        if self.host.units.is_active(&m.current.unit).await {
            return Ok(Some((m.current, false)));
        }
        let starting = {
            let _lock = marker_lock()?;
            read_marker().filter(|m| m.starting && still_starting())
        };
        if let Some(m) = starting {
            return Ok(Some((m.current, true)));
        }
        Ok(self.current().await.map(|c| (c, false)))
    }

    /// A marker without an active unit (crash, reboot) is closed now; one that cannot be read goes, or every launch would be `Busy`.
    /// Then every close left unfinished resumes. An `Err` leaves a marker its close could not be kept for: a launch waits for it.
    pub async fn reconcile(&self) -> Result<()> {
        let left_open = self.close_left_open().await;
        for c in closings() {
            if let Err(e) = self.finish_closing(&c.row.game, &c.row.session).await {
                tracing::warn!("session {} of {}: {e}", c.row.session, c.row.game);
            }
        }
        left_open
    }

    async fn close_left_open(&self) -> Result<()> {
        let path = paths::current_session_file();
        let Ok(text) = std::fs::read_to_string(&path) else { return Ok(()) };
        let m: Marker = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _lock = marker_lock()?;
                if std::fs::read_to_string(&path).is_ok_and(|now| now == text) {
                    tracing::warn!("{}: unreadable marker dropped: {e}", path.display());
                    remove_marker();
                }
                return Ok(());
            }
        };
        if self.host.units.is_active(&m.current.unit).await || (m.starting && marker_lock().map(|_lock| still_starting())?) {
            return Ok(());
        }
        tracing::warn!("session {} of {} was left open; closing it", m.current.session_id, m.current.id);
        self.session_end(&m.current.id, &m.current.session_id, None, None).await
    }

    /// A closed session's row: in its history, or waiting for it.
    pub async fn closed_row(&self, id: &str, session_id: &str) -> Option<Session> {
        let filed = sessions::read(&self.get(id).await.ok()?.game.sessions_path()).unwrap_or_default().into_iter().find(|s| s.session == session_id);
        filed.or_else(|| Some(closing_file(id, session_id)).filter(|f| f.exists()).and_then(|f| read_closing(&f)).filter(|c| !c.failed_launch).map(|c| c.row))
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
            crate::tools::ensure(&cfg.launch.umu_run, "").await?;
        }
        let started = chrono::Local::now();
        let session_id = sessions::session_id(started);
        let screen = crate::desktop::pick_screen(screen);
        let unit = format!("{}.service", launcher::unit_name(id, &session_id));

        let mut base = self.hook_env_base(&r);
        base.set("SESSION_ID", session_id.clone());
        base.set("SESSION_UNIT", unit.clone());
        base.set("SESSION_SCREEN", screen.clone());
        base.set("SESSION_STARTED_AT", started.to_rfc3339());

        for m in self.modules.read().await.iter().filter(|m| m.enabled && !m.available) {
            tracing::warn!("module {} is enabled but its hooks are skipped: {} (see `universe doctor`)", m.id(), m.unavailable());
        }
        let gamescope_pid = self.nest().map_or(0, |n| n.pid);
        let current = Current {
            session_id: session_id.clone(),
            id: id.into(),
            title: r.game.title.clone(),
            unit: unit.clone(),
            screen: screen.clone(),
            started_at: started.to_rfc3339(),
            gamescope_pid,
            launcher_pid: if gamescope_pid != 0 { std::process::id() } else { 0 },
            app_pid: std::process::id(),
        };
        let starting = hold_starting()?;
        write_marker(&Marker { current: current.clone(), hook_env: base.vars.clone(), starting: true, ..Marker::default() })?;
        let prepared = self.prepare(&r, &cfg, &base, &session_id, &screen, splash).await;
        let (plan, runtime, overlay) = match prepared {
            Ok(p) => p,
            Err(e) => {
                let _lock = marker_lock()?;
                if read_marker().is_some_and(|m| m.current.session_id == session_id) {
                    remove_marker();
                }
                return Err(e);
            }
        };
        self.begin(&r, &plan, &current, runtime, starting).await?;
        for m in self.hook_modules(&r, "post-launch").await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &base);
            if let Err(e) = modules::run_async(&self.host.units, &m, "post-launch", &env, &session_id, Some(&unit)).await {
                tracing::warn!("post-launch {}: {e}", m.id());
            }
        }
        let overlaid = match &overlay {
            Some(file) => self.spawn_overlay(&session_id, &unit, file).await.inspect_err(|e| tracing::warn!("overlay: {e}")).is_ok(),
            None => false,
        };
        // The overlay's own watcher reads the pads: a second one would hold the lock it waits on, and HOME would never reach it.
        if cfg.controller.enabled && !overlaid {
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

    /// The pre-launch hooks, then the launch's plan, its runtime and the overlay's display file.
    async fn prepare(
        &self,
        r: &Resolved,
        cfg: &crate::config::Config,
        base: &modules::HookEnv,
        session_id: &str,
        screen: &str,
        splash: &str,
    ) -> Result<(Plan, Runtime, Option<std::path::PathBuf>)> {
        let id = r.game.id.as_str();
        let env_file = paths::state_home().join(format!("env-{session_id}"));
        std::fs::create_dir_all(paths::state_home())?;
        std::fs::write(&env_file, "")?;
        for m in self.hook_modules(r, "pre-launch").await {
            let mut env = self.module_env(&m, Some(&r.game), cfg, base);
            env.set("UNIVERSE_ENV_FILE", env_file.to_string_lossy().to_string());
            let out = modules::run_blocking(&m, "pre-launch", &env).await?;
            if out.status != 0 {
                let _ = std::fs::remove_file(&env_file);
                return Err(Error::Io(format!("pre-launch {} refused the launch: {}", m.id(), out.stderr.trim())));
            }
            for line in out.stderr.lines() {
                tracing::info!("pre-launch {}: {line}", m.id());
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

        let mode = crate::desktop::screen_mode(screen).await;
        let splash = (!splash.is_empty()).then(|| std::path::PathBuf::from(splash));
        let nested = self.nest().is_some_and(|n| n.pid != 0);
        let (mut plan, runtime) = if self.under_steam() {
            // Steam's mangoapp draws its HUD and Steam limits the frame rate: no MangoHud layer, no limit of Universe's.
            let mut steam = r.clone();
            steam.effective.mangohud = false;
            steam.effective.fps_limit = "none".into();
            (launcher::plan(&steam, cfg, &extra_env, mode, splash.as_deref(), true, true)?, Runtime::of(&steam.effective))
        } else {
            (
                launcher::plan(r, cfg, &extra_env, mode, splash.as_deref(), nested, launcher::mangoapp_installed())?,
                Runtime::at_launch(&r.effective, nested, &cfg.launch),
            )
        };
        // Over a gamescope of the game's own, the overlay connects to the display splash records. A launcher's watcher already
        // holding the pads (Big Screen in a window) answers HOME itself, and the overlay's would wait on it for good.
        let overlay =
            (plan.gamescope && crate::runners::on_path(OVERLAY_PROGRAM).is_some() && !crate::controller::watch::lock_held()).then(|| display_file(session_id));
        if let Some(file) = &overlay {
            plan.env.insert(crate::splash::DISPLAY_FILE_ENV.into(), file.to_string_lossy().into());
        }
        for (path, text) in plan.mangohud_conf.iter().chain(&plan.mangoapp_conf) {
            std::fs::write(path, text)?;
        }
        if let Some(prefix) = plan.env.get("WINEPREFIX").map(std::path::PathBuf::from) {
            match crate::core::blocking(move || Ok(crate::wine_fonts::prune_dead(&prefix)?)).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("{id}: dropped {n} font registry entries its prefix made from /nix/store files now gone"),
                Err(e) => tracing::warn!("{id}: prefix fonts: {e}"),
            }
        }
        if let Some(spec) = crate::runners::spec(&r.effective.runner) {
            let dir = paths::session_log_dir(id, session_id);
            let kind = if spec.via_proton { crate::runners::Kind::Proton } else { spec.kind };
            let mut logs = launcher::crash_env(kind, &dir);
            if r.effective.debug_log {
                logs.extend(launcher::debug_env(kind, &dir));
            }
            // winedbg makes the dir on the game's current drive, not under Z:, where it then writes.
            if r.effective.debug_log || !logs.is_empty() {
                std::fs::create_dir_all(&dir)?;
            }
            for (k, v) in logs {
                plan.env.entry(k).or_insert(v);
            }
        }
        Ok((plan, runtime, overlay))
    }

    /// On the marker still starting, its command and runtime, then each effect noted as it begins, then the unit. A start that
    /// fails closes the session through its record, with no row.
    async fn begin(&self, r: &Resolved, plan: &Plan, current: &Current, runtime: Runtime, _starting: std::fs::File) -> Result<()> {
        let (id, session_id) = (&current.id, &current.session_id);
        let noted = update_marker(session_id, |m| {
            m.command = plan.command_line();
            m.runtime = Some(runtime);
        });
        let started = match noted {
            Ok(()) => self.start(r, plan, current).await,
            refused => refused,
        };
        match started {
            Ok(()) => {
                if let Err(e) = update_marker(session_id, |m| m.starting = false) {
                    tracing::warn!("session {session_id} of {id}: {e}");
                }
                Ok(())
            }
            Err(e) => {
                let closed = claim_closing(id, session_id, Claim::FailedLaunch, |_| Session::default());
                match closed {
                    Ok(()) => self.finish_closing(id, session_id).await.unwrap_or_else(|e| tracing::warn!("session {session_id} of {id}: {e}")),
                    Err(e) => tracing::warn!("{e}"),
                }
                Err(e)
            }
        }
    }

    async fn start(&self, r: &Resolved, plan: &Plan, current: &Current) -> Result<()> {
        let session_id = &current.session_id;
        if !plan.post_command.trim().is_empty() {
            note_undo(
                session_id,
                Undo::PostCommand { command: plan.post_command.clone(), cwd: plan.cwd.to_string_lossy().to_string(), env: plan.env.clone() },
            )?;
        }
        launcher::run_shell(&plan.pre_command, &plan.env, &plan.cwd).await?;
        if r.effective.hide_cursor {
            let restore = self.host.shell.cursor_enable().await;
            if let Err(e) = note_undo(session_id, Undo::Cursor { restore: restore.clone() }) {
                self.host.shell.cursor_restore(&restore).await;
                return Err(e);
            }
        }
        if current.gamescope_pid != 0 && !self.under_steam() && launcher::mangoapp_installed() {
            note_undo(session_id, Undo::Hud)?;
            self.apply_mangoapp(r.effective.mangohud, true)?;
        }
        let own: Vec<(&String, &String)> = r.game.system.iter().filter(|(_, v)| !v.is_empty()).collect();
        if !own.is_empty() {
            let controls = self.system_controls().await;
            let before: BTreeMap<String, String> =
                own.iter().filter_map(|(id, _)| controls.iter().find(|c| c.id == id.as_str())).map(|c| (c.id.to_string(), c.value.clone())).collect();
            if !before.is_empty() {
                note_undo(session_id, Undo::System { before: before.clone() })?;
            }
            for (id, value) in own.into_iter().filter(|(id, _)| before.contains_key(*id)) {
                if let Err(e) = self.apply_system_one(id, value).await {
                    tracing::warn!("system.{id} = {value}: {e}");
                }
            }
        }
        tracing::info!("launch {}: {}", r.game.id, plan.command_line());
        let budget: u64 = 60 + self.hook_modules(r, "session-end").await.iter().map(|m| m.timeout().as_secs()).sum::<u64>();
        let mut env = passthrough_env();
        env.extend(plan.env.clone());
        let log_dir = paths::session_log_dir(&current.id, &current.session_id);
        if r.effective.debug_log && log_dir.is_dir() {
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
                Undo::Cursor { restore } => self.host.shell.cursor_restore(restore).await,
                // From ExecStopPost the launcher's gamescope may be gone already: told only while it is there.
                Undo::Hud => {
                    if let Err(e) = self.apply_mangoapp(false, self.nest().is_some()) {
                        tracing::warn!("mangoapp: {e}");
                    }
                }
                Undo::Filter => {
                    if let Some(nest) = self.nest() {
                        let launch = self.config.read().await.launch.clone();
                        if let Err(e) = nest.set_filter(&launch.gamescope_filter, launch.gamescope_sharpness) {
                            tracing::warn!("gamescope filter: {e}");
                        }
                    }
                }
                Undo::PostCommand { command, cwd, env } => {
                    if let Err(e) = launcher::run_shell(command, env, Path::new(cwd)).await {
                        tracing::warn!("post_command: {e}");
                    }
                }
                Undo::System { before } => {
                    for (id, value) in before {
                        if let Err(e) = self.apply_system_one(id, value).await {
                            tracing::warn!("system.{id} = {value}: {e}");
                        }
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

    /// Big Screen's overlay alone over the game's own gamescope: the dock on HOME, unlocks, the shot's flash and the volume level.
    async fn spawn_overlay(&self, session_id: &str, game_unit: &str, display: &Path) -> Result<()> {
        let mut env = passthrough_env();
        // gamescope's display replaces the desktop's: the core inside asks the desktop about its windows through these.
        for var in crate::desktop::HOST_VARS {
            env.entry(crate::desktop::host_var(var)).or_insert_with(|| std::env::var(var).unwrap_or_default());
        }
        // Universe's gamescope whatever its flags: never taken for Steam's.
        env.insert(crate::nest::OWN_ENV.into(), "nested".into());
        let spec = UnitSpec {
            name: format!("universe-overlay-{session_id}"),
            description: "Universe overlay".into(),
            program: paths::self_exe().to_string_lossy().to_string(),
            args: vec!["overlay".into(), display.to_string_lossy().to_string()],
            env,
            bind_to: Some(game_unit.into()),
            ..Default::default()
        };
        self.host.units.start(&spec).await
    }

    /// `universe overlay`: universe-ui over the game's own gamescope. Gone while the game runs (it never came up, or died), the
    /// pads' macros carry on in its stead.
    pub async fn run_overlay(self: std::sync::Arc<Self>, display: std::path::PathBuf) -> Result<()> {
        match crate::runners::on_path(OVERLAY_PROGRAM) {
            Some(ui) => match tokio::process::Command::new(&ui).arg("--overlay").arg(&display).status().await {
                Ok(status) if status.success() => {}
                Ok(status) => tracing::warn!("{OVERLAY_PROGRAM}: {status}"),
                Err(e) => tracing::warn!("{}: {e}", ui.display()),
            },
            None => tracing::warn!("{OVERLAY_PROGRAM} not found"),
        }
        if self.current().await.is_none() || !self.config.read().await.controller.enabled {
            return Ok(());
        }
        tracing::warn!("the overlay is gone while the game runs: the pads' macros carry on without it");
        crate::controller::watch::watch(self, crate::controller::watch::WatchOptions { json: false, wait: true }).await
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

    /// Run by `ExecStopPost`, or by `reconcile` for a session left open; idempotent across processes. Without a marker
    /// (a reboot) the times come from the unit's log.
    pub async fn session_end(&self, id: &str, session_id: &str, exit: Option<i32>, ended: Option<chrono::DateTime<chrono::Local>>) -> Result<()> {
        let _ = std::fs::remove_file(display_file(session_id));
        let seen = read_marker().filter(|m| m.current.session_id == session_id);
        if seen.is_some() || !closing_file(id, session_id).exists() {
            let unit = seen.as_ref().map(|m| m.current.unit.clone()).unwrap_or_else(|| format!("{}.service", launcher::unit_name(id, session_id)));
            let log = self.host.units.log(&unit).await;
            let claim = Claim::Ended { seen: seen.is_some(), ran: log.started.is_some() };
            claim_closing(id, session_id, claim, |marker| closed_session(id, session_id, exit, ended, marker, &log))?;
        }
        self.finish_closing(id, session_id).await
    }

    /// The close's steps not yet noted done, in order: the row filed, the undo, the session-end hooks, then `post-process` once the row is in.
    /// A history that refuses the row leaves the record for the next reconcile; a record that refuses its notes runs every step still,
    /// and stays for the next open to run them again.
    async fn finish_closing(&self, id: &str, session_id: &str) -> Result<()> {
        let Some((held, mut c)) = hold_closing(id, session_id)? else { return Ok(()) };
        let mut refused = false;
        let mut noted = |kept: Result<()>| match kept {
            Err(e) if !refused => {
                tracing::warn!(
                    "{}: the close of session {session_id} of {id} cannot be noted there, so the next open repeats it: {e}",
                    paths::closing_sessions_dir().display()
                );
                refused = true;
            }
            _ => {}
        };
        if !c.filed {
            match file_closing(id, session_id) {
                Ok(kept) => {
                    c.filed = true;
                    noted(kept.map_err(Into::into));
                }
                Err(e) => tracing::warn!("session {session_id} of {id} waits for its history: {e}"),
            }
        }
        while let Some(step) = c.undo.pop() {
            self.rollback(std::slice::from_ref(&step)).await;
            noted(change_closing(id, session_id, |kept| kept.undo.pop().is_some()).map(drop));
        }
        if !c.ended {
            self.run_session_end(id, &c).await;
            noted(
                change_closing(id, session_id, |kept| {
                    kept.ended = true;
                    true
                })
                .map(drop),
            );
        }
        if let Err(e) = self.reload_game(id).await {
            tracing::warn!("{id}: {e}");
        }
        if !c.filed {
            return Ok(());
        }
        {
            let file = closing_file(id, session_id);
            let _lock = marker_lock()?;
            if let Err(e) = std::fs::remove_file(&file) {
                tracing::warn!(
                    "{}: the close of session {session_id} of {id} cannot end there, so the next open repeats it: {e}",
                    paths::closing_sessions_dir().display()
                );
                return Ok(());
            }
            let _ = std::fs::remove_file(file.with_extension("lock"));
        }
        drop(held);
        if !c.failed_launch {
            self.post_process(id, session_id).await;
        }
        Ok(())
    }

    async fn run_session_end(&self, id: &str, c: &Closing) {
        let Ok(r) = self.get(id).await else { return };
        let cfg = self.config.read().await.clone();
        let mut env_end = match &c.hook_env {
            Some(vars) => HookEnv { vars: vars.clone() },
            None => {
                let mut e = self.hook_env_base(&r);
                e.set("SESSION_ID", c.row.session.clone());
                e.set("SESSION_UNIT", c.row.unit.clone());
                e.set("SESSION_SCREEN", c.row.screen.clone());
                e.set("SESSION_STARTED_AT", c.row.started_at.clone());
                e
            }
        };
        env_end.set("SESSION_ENDED_AT", c.row.ended_at.clone());
        env_end.set("SESSION_DURATION_S", c.row.duration_s.to_string());
        for m in self.hook_modules(&r, "session-end").await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &env_end);
            if let Err(e) = modules::run_blocking(&m, "session-end", &env).await {
                tracing::warn!("session-end {}: {e}", m.id());
            }
        }
        if cfg.saves.auto_backup && crate::saves::backs_up(&r.game, &cfg) {
            self.start_saves_backup(&r.game.id, &c.row.session).await;
        }
    }

    /// A unit of its own: ludusavi's first run fetches it and its manifest, too long for `ExecStopPost`.
    async fn start_saves_backup(&self, id: &str, session_id: &str) {
        let spec = crate::host::UnitSpec {
            name: format!("universe-saves-backup-{session_id}"),
            description: format!("Universe saves backup of {id}"),
            program: paths::self_exe().to_string_lossy().into(),
            args: vec!["saves".into(), "backup".into(), id.into(), "--auto".into()],
            env: passthrough_env(),
            ..Default::default()
        };
        if let Err(e) = self.host.units.start(&spec).await {
            tracing::warn!("saves backup of {id}: {e}");
        }
    }

    /// What a `post-process` hook is told about one session of `r`, whether it just ended or is being written again later.
    pub(crate) fn post_process_env(&self, r: &Resolved, session_id: &str) -> HookEnv {
        let sess = r.sessions.iter().find(|s| s.session == session_id).cloned();
        let mut env = self.hook_env_base(r);
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
        let env = self.post_process_env(&r, session_id);
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
        update_marker(&c.session_id, |m| m.stopped = true)?;
        self.host.units.stop(&c.unit, term_twice, crate::host::QUIT_GRACE).await
    }

    pub async fn freeze(&self, on: bool) -> Result<()> {
        let _in_order = self.freezes.lock().await;
        let Some(marker) = read_marker() else { return Err(Error::NotFound("no session running".into())) };
        if !self.host.units.is_active(&marker.current.unit).await {
            return Err(Error::NotFound("no session running".into()));
        }
        self.host.units.freeze(&marker.current.unit, on).await?;
        if !on {
            self.keep_game_unminimized();
        }
        self.session_hooks(marker, if on { "freeze" } else { "thaw" }).await;
        Ok(())
    }

    /// A pad of the running game's dropped and brought back: the modules holding pads for it (the virtual pads') take it up.
    pub async fn reconnect_hooks(&self) -> Result<()> {
        let Some(marker) = read_marker() else { return Err(Error::NotFound("no session running".into())) };
        if !self.host.units.is_active(&marker.current.unit).await {
            return Err(Error::NotFound("no session running".into()));
        }
        self.session_hooks(marker, "reconnect").await;
        Ok(())
    }

    /// `hook` of the running game's modules, under the env its launch gave them; a failing one is logged.
    async fn session_hooks(&self, marker: Marker, hook: &str) {
        let Ok(r) = self.get(&marker.current.id).await else { return };
        let cfg = self.config.read().await.clone();
        let base = HookEnv { vars: marker.hook_env };
        for m in self.hook_modules(&r, hook).await {
            let env = self.module_env(&m, Some(&r.game), &cfg, &base);
            if let Err(e) = modules::run_blocking(&m, hook, &env).await {
                tracing::warn!("{hook} {}: {e}", m.id());
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::Game;
    use crate::host::{Host, Memory};
    use std::sync::Arc;

    /// A library of one Dolphin game on a fake emulator, every Universe home under one tempdir.
    pub(crate) struct Sandbox {
        post_ran: std::path::PathBuf,
        _env: crate::paths::TestEnv,
    }

    pub(crate) fn sandbox() -> Sandbox {
        let env = crate::paths::test_env();
        let dir = env.path();
        std::fs::write(dir.join("config/config.toml"), "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = []\n")
            .unwrap();
        let emu = dir.join("dolphin-emu");
        std::fs::write(&emu, b"#!/bin/sh\n").unwrap();
        let umu = crate::tools::dir().join("umu-run");
        std::fs::create_dir_all(crate::tools::dir()).unwrap();
        std::fs::write(&umu, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&umu, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let rom = dir.join("F-Zero GX.iso");
        std::fs::write(&rom, b"").unwrap();
        let post_ran = dir.join("post-ran");
        let mut g = Game::new("Sample");
        g.launch.runner = "dolphin".into();
        g.launch.runner_exe = emu.to_string_lossy().into();
        g.launch.exe = rom.to_string_lossy().into();
        g.launch.pre_command = "true".into();
        g.launch.post_command = format!("touch {}", post_ran.display());
        g.desktop.hide_cursor = Some(true);
        g.save().unwrap();
        Sandbox { post_ran, _env: env }
    }

    pub(crate) async fn open() -> (Core, Arc<Memory>) {
        let (host, memory) = Host::memory();
        (Core::open_with(crate::config::Config::load().unwrap(), host).await.unwrap(), memory)
    }

    fn undo_steps(m: &Marker) -> Vec<&'static str> {
        m.undo
            .iter()
            .map(|u| match u {
                Undo::PostCommand { .. } => "post_command",
                Undo::Cursor { .. } => "cursor",
                Undo::Hud => "hud",
                Undo::Filter => "filter",
                Undo::System { .. } => "system",
            })
            .collect()
    }

    #[tokio::test]
    async fn a_session_is_launched_ended_and_undone_in_reverse() {
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
        assert!(memory.spec(&format!("universe-overlay-{sid}")).is_none(), "no gamescope of its own, no overlay");
        let marker = read_marker().expect("a marker");
        assert_eq!((marker.current.session_id.as_str(), marker.current.unit.as_str()), (sid.as_str(), unit.as_str()));
        assert_eq!(undo_steps(&marker), ["post_command", "cursor"]);
        assert_eq!(marker.undo[1], Undo::Cursor { restore: crate::desktop::CursorUndo::Nothing });
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
        assert_eq!(calls[..2], ["cursor:enable".to_string(), format!("unit:start {unit}")]);
        assert_eq!(calls.last().map(String::as_str), Some("cursor:restore"), "{calls:?}");

        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap().len(), 1, "idempotent");
    }

    #[tokio::test]
    async fn a_game_in_a_gamescope_of_its_own_gets_the_overlay_bound_to_it_in_place_of_the_controller_watch() {
        let sb = sandbox();
        let bin = sb._env.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        for name in ["gamescope", OVERLAY_PROGRAM] {
            std::fs::write(bin.join(name), b"#!/bin/sh\n").unwrap();
            std::fs::set_permissions(bin.join(name), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        }
        let path_was = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var("PATH", std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&path_was))).unwrap());
        std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME");
        let config = format!(
            "[launch]\ngamescope = true\ngamescope_bin = \"{}\"\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = []\n",
            bin.join("gamescope").display()
        );
        std::fs::write(crate::paths::config_file(), config).unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        let game = memory.spec(&unit).expect("the game unit started");
        assert_eq!(game.program, bin.join("gamescope").to_string_lossy());
        let display = game.env.get(crate::splash::DISPLAY_FILE_ENV).expect("splash is told where to record gamescope's display");
        let overlay = memory.spec(&format!("universe-overlay-{sid}")).expect("the overlay");
        assert_eq!(overlay.program, paths::self_exe().to_string_lossy());
        assert_eq!(overlay.args, ["overlay", display.as_str()]);
        assert_eq!(overlay.bind_to.as_deref(), Some(unit.as_str()), "it goes down with the game");
        assert_eq!(overlay.env[&crate::desktop::host_var("XDG_CURRENT_DESKTOP")], "GNOME", "the desktop is still asked about its windows");
        assert!(memory.spec(&format!("universe-controller-{sid}")).is_none(), "the overlay's own watcher reads the pads");
        assert_eq!(read_marker().expect("a marker").current.app_pid, std::process::id(), "its Home raises the window of the app that launched");
        std::fs::write(display, "DISPLAY=:1\n").unwrap();
        memory.finish(&unit, 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(!Path::new(display).exists(), "the record goes with the session");

        let lock = std::fs::File::create(crate::controller::watch::lock_path()).unwrap();
        lock.lock().unwrap();
        core.launch("sample", "", "").await.unwrap();
        let started = |unit: &str| memory.calls().iter().filter(|c| c.starts_with(&format!("unit:start {unit}-"))).count();
        assert_eq!(started("universe-overlay"), 1, "a launcher in a window already reads the pads and has HOME");
        assert_eq!(started("universe-controller"), 1);
    }

    #[test]
    fn the_pads_macros_carry_on_when_the_overlay_goes_before_the_game() {
        let sb = sandbox();
        let dir = sb._env.path();
        let (bin, input) = (dir.join("bin"), dir.join("input"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&input).unwrap();
        let ui = bin.join(OVERLAY_PROGRAM);
        std::fs::write(&ui, b"#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&ui, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let path_was = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var("PATH", std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&path_was))).unwrap());
        std::env::set_var("UNIVERSE_INPUT_DIR", &input);
        // The watcher reads stdin on a blocking thread that never returns: the runtime is left behind rather than waited for.
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let took = runtime.block_on(async {
            let (core, _memory) = open().await;
            let core = std::sync::Arc::new(core);
            let display = dir.join("display");
            core.clone().run_overlay(display.clone()).await.unwrap();
            assert!(!crate::controller::watch::lock_held(), "no game left: nothing reads the pads");
            core.launch("sample", "", "").await.unwrap();
            let overlay = tokio::spawn(core.clone().run_overlay(display));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !crate::controller::watch::lock_held() && std::time::Instant::now() < deadline {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            overlay.abort();
            crate::controller::watch::lock_held()
        });
        runtime.shutdown_background();
        assert!(took, "the game still runs: the macros' watcher takes the pads");
    }

    #[tokio::test]
    async fn a_session_ending_backs_the_games_saves_up_in_a_unit_of_its_own() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        let mut head = vec![0u8; 0x20];
        head[..6].copy_from_slice(b"GFZE8P");
        head[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        std::fs::write(core.get("sample").await.unwrap().game.exe_path(), head).unwrap();
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let spec = memory.spec(&format!("universe-saves-backup-{sid}")).expect("a backup after the session");
        assert_eq!(spec.args, ["saves", "backup", "sample", "--auto"]);
        assert!(spec.bind_to.is_none(), "it outlives the game's unit");

        let file = crate::paths::config_file();
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{text}[saves]\nauto_backup = false\n")).unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(memory.spec(&format!("universe-saves-backup-{sid}")).is_none(), "auto_backup off: none");
    }

    #[tokio::test]
    async fn the_desktop_is_kept_awake_for_as_long_as_the_game_runs() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        let awake = memory.spec(&format!("universe-awake-{sid}")).expect("the inhibitor");
        assert_eq!(awake.bind_to.as_deref(), Some(format!("universe-game-sample-{sid}.service").as_str()), "it is released with the game");
        assert_eq!(awake.args[0], "keep-awake");
    }

    #[tokio::test]
    async fn keep_awake_off_leaves_the_desktop_to_its_own_idle() {
        let _sb = sandbox();
        let file = crate::paths::config_file();
        let text = std::fs::read_to_string(&file).unwrap();
        std::fs::write(&file, format!("{text}[desktop]\nkeep_awake = false\n")).unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        assert!(memory.spec(&format!("universe-awake-{sid}")).is_none(), "off, nothing holds the desktop awake");
    }

    #[tokio::test]
    async fn the_hud_and_the_limit_change_the_running_game_alone_and_the_next_launch_starts_from_the_settings() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        assert!(matches!(core.set_mangohud(None).await, Err(Error::NotFound(_))), "no game, nothing to show");
        let sid = core.launch("sample", "", "").await.unwrap();
        let conf = launcher::layer_conf_path();
        let read = || std::fs::read_to_string(&conf).unwrap_or_default();
        assert_eq!(core.runtime().await.unwrap(), Runtime { mangohud: false, fps_limit: "none".into(), ..Runtime::default() }, "seeded from the settings");
        if crate::runners::on_system_path("mangohud").is_none() {
            assert!(matches!(core.set_mangohud(None).await, Err(Error::Unavailable(_))), "no MangoHud installed: nothing to draw the HUD");
            assert!(!core.runtime().await.unwrap().mangohud, "and nothing changed");
        }
        std::fs::create_dir_all(crate::tools::dir()).unwrap();
        std::fs::write(crate::tools::dir().join("mangohud"), b"#!/bin/sh\n").unwrap();

        assert!(core.set_mangohud(None).await.unwrap(), "off flips on");
        let text = read();
        assert!(!text.contains("no_display") && text.contains("control=universe-mangohud-sample\n"), "{text}");
        core.set_fps_limit("30").await.unwrap();
        let text = read();
        assert!(text.contains("fps_limit=30\n") && !text.contains("no_display"), "a new limit keeps the HUD shown at runtime: {text}");
        assert!(matches!(core.set_fps_limit("fast").await, Err(Error::Invalid(_))));

        change_runtime(&sid, Runtime::default, |rt| rt.mangohud = false).unwrap();
        assert!(core.set_mangohud(None).await.unwrap(), "what another process left in the marker is read before the flip");
        assert!(!core.set_mangohud(Some(false)).await.unwrap());
        assert!(read().contains("no_display\n"), "hidden");
        let launch = core.get("sample").await.unwrap().game.launch;
        assert_eq!((launch.mangohud, launch.fps_limit.as_str()), (None, ""), "the game's settings untouched");
        assert!(core.config.read().await.launch.fps_limit == "none" && !core.config.read().await.launch.mangohud, "and every game's");

        core.set_mangohud(Some(true)).await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        core.launch("sample", "", "").await.unwrap();
        assert_eq!(
            core.runtime().await.unwrap(),
            Runtime { mangohud: false, fps_limit: "none".into(), ..Runtime::default() },
            "the next launch starts from the settings"
        );
        assert!(read().contains("no_display\n") && !read().contains("fps_limit="), "{}", read());
    }

    #[tokio::test]
    async fn a_filter_set_in_game_is_noted_once_for_the_end_to_put_back() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        assert!(!undo_steps(&read_marker().unwrap()).contains(&"filter"), "nothing to put back until HOME changes it");
        change_filter(&sid, Runtime::default, "fsr", Some(5)).unwrap();
        change_filter(&sid, Runtime::default, "nis", None).unwrap();
        let m = read_marker().unwrap();
        assert_eq!(undo_steps(&m).iter().filter(|s| **s == "filter").count(), 1);
        let rt = m.runtime.unwrap();
        assert_eq!((rt.gamescope_filter.as_str(), rt.gamescope_sharpness), ("nis", None));
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(read_marker().is_none());
        core.launch("sample", "", "").await.unwrap();
        assert_eq!(core.runtime().await.unwrap().gamescope_filter, "", "the next launch starts from the settings");
    }

    #[test]
    fn the_launchers_gamescope_reads_as_the_settings_filter_whatever_the_games_own() {
        let mut e = crate::library::Effective::default();
        e.gamescope_fields.filter = "fsr".into();
        e.gamescope_fields.sharpness = Some(3);
        let launch = crate::config::LaunchDefaults { gamescope_filter: "nis".into(), gamescope_sharpness: Some(7), ..Default::default() };
        let filter = |rt: Runtime| (rt.gamescope_filter, rt.gamescope_sharpness);
        assert_eq!(filter(Runtime::at_launch(&e, true, &launch)), ("nis".into(), Some(7)), "what the running gamescope holds");
        assert_eq!(filter(Runtime::at_launch(&e, false, &launch)), ("fsr".into(), Some(3)), "a gamescope of the game's own starts on its own");
    }

    #[test]
    fn a_change_racing_the_end_never_brings_the_marker_back() {
        let _env = crate::paths::test_env();
        let marker = |sid: &str| Marker { current: Current { session_id: sid.into(), ..Current::default() }, ..Marker::default() };
        let flip = |sid: &'static str| change_runtime(sid, Runtime::default, |rt| rt.mangohud = !rt.mangohud).unwrap();
        assert!(flip("a").is_none() && read_marker().is_none(), "no marker, none made");
        write_marker(&marker("b")).unwrap();
        assert!(flip("a").is_none() && read_marker().unwrap().runtime.is_none(), "another session's is left alone");
        remove_marker();
        for _ in 0..20 {
            write_marker(&marker("a")).unwrap();
            let changes: Vec<_> = (0..3).map(|_| std::thread::spawn(move || (0..10).for_each(|_| drop(flip("a"))))).collect();
            {
                let _lock = marker_lock().unwrap();
                assert!(read_marker().is_some());
                remove_marker();
            }
            changes.into_iter().for_each(|t| t.join().unwrap());
            assert!(read_marker().is_none(), "a change after the end put no marker back");
        }
    }

    #[tokio::test]
    async fn reconcile_closes_a_session_whose_unit_vanished() {
        let sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.drop_unit(&format!("universe-game-sample-{sid}.service"));
        assert!(core.current().await.is_none());
        let m = read_marker().unwrap();
        let row = closed_session("sample", &sid, None, None, Some(&m), &Default::default());
        write_closing(&Closing { row, undo: m.undo.clone(), hook_env: Some(m.hook_env), ..Closing::default() })
            .expect("a close cut between its record and the marker's removal");
        core.reconcile().await.unwrap();
        let rows = sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].session.as_str(), rows[0].exit), (sid.as_str(), -1));
        assert!(read_marker().is_none());
        assert!(sb.post_ran.exists());
        let calls = memory.calls();
        assert_eq!(calls.last().map(String::as_str), Some("cursor:restore"));
        assert_eq!(calls.iter().filter(|c| *c == "cursor:restore").count(), 1, "undone once");
        assert!(core.launch("sample", "", "").await.is_ok(), "the next launch is not Busy");
    }

    #[tokio::test]
    async fn a_launch_still_starting_outlives_another_open_and_runs() {
        let sb = sandbox();
        let (core, memory) = open().await;
        let gate = sb.post_ran.with_file_name("gate");
        let mut g = core.get("sample").await.unwrap().game;
        g.launch.pre_command = format!("touch {0}.entered; while [ ! -e {0}.release ] && [ -e {0} ]; do sleep 0.02; done", gate.display());
        std::fs::write(&gate, "").unwrap();
        g.save().unwrap();
        core.reload_game("sample").await.unwrap();
        let other = async {
            while !gate.with_extension("entered").exists() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            let (other, _) = open().await;
            let left = read_marker().is_some_and(|m| m.starting);
            drop(other);
            std::fs::write(gate.with_extension("release"), "").unwrap();
            assert!(left, "another open leaves the marker of a launch still starting");
        };
        let (sid, ()) = tokio::join!(core.launch("sample", "", ""), other);
        let sid = sid.unwrap();
        assert!(read_marker().is_some_and(|m| m.current.session_id == sid && !m.starting && m.undo.len() == 2));
        assert_eq!(core.current().await.map(|c| c.session_id), Some(sid.clone()));
        assert!(!sb.post_ran.exists() && closings().is_empty(), "nothing undone, nothing closed");

        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(sb.post_ran.exists());
        assert_eq!(sessions::read(&core.get("sample").await.unwrap().game.sessions_path()).unwrap().len(), 1);
    }

    async fn destroy(core: &Core) -> (Result<()>, Result<()>, Result<usize>) {
        (core.uninstall("sample").await, core.remove("sample", true).await, core.source_update("fake", "s1", None).await)
    }

    /// Storage's leftovers of Sample once removed, each kind with what trashing it answers; Sample is back in the library after.
    async fn trash_leftovers(core: &Core) -> Vec<(String, Result<()>)> {
        core.remove("sample", false).await.unwrap();
        let storage = core.storage().await.unwrap();
        let mut out = Vec::new();
        for l in storage["leftovers"].as_array().unwrap().iter().filter(|l| l["id"] == "sample" || l["kind"] == "prefix") {
            out.push((l["kind"].as_str().unwrap().to_string(), core.trash_leftover(l["path"].as_str().unwrap()).await));
        }
        core.restore("sample").await.unwrap();
        out
    }

    fn leftovers_busy(answers: Vec<(String, Result<()>)>, when: &str) {
        let kinds: Vec<&str> = answers.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, ["prefix", "recordings", "game", "logs"], "{when}");
        for (kind, answer) in answers {
            assert!(matches!(answer, Err(Error::Busy(_))), "{when}: trashing its leftover {kind} is {answer:?}");
        }
    }

    #[tokio::test]
    async fn a_game_starting_running_or_backing_up_its_saves_keeps_its_files() {
        let sb = sandbox();
        let dir = sb._env.path().to_path_buf();
        let asked = dir.join("asked");
        let src = dir.join("sources/fake");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("source.toml"),
            "api = 2\nid = \"fake\"\nname = \"Fake\"\nexe = \"run\"\ncapabilities = [\"uninstall\", \"update\"]\n[hooks]\npre-launch = \"pre\"\n",
        )
        .unwrap();
        std::fs::write(
            src.join("run"),
            format!(
                "#!/bin/sh\ncase \"$1$2\" in\nupdate) echo '{{\"event\":\"update\",\"id\":\"s1\"}}' ;;\nscan) echo '{{\"event\":\"game\",\"id\":\"s1\",\"title\":\"Sample\",\"owned\":true,\"installed\":true,\"dir\":\"{}\"}}' ;;\nuninstall*|update*) echo \"$1 $2\" >> '{}' ;;\nesac\necho '{{\"event\":\"done\"}}'\n",
                dir.join("games/sample").display(),
                asked.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(src.join("run"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let gate = sb.post_ran.with_file_name("gate");
        let hook_gate = gate.with_file_name("hook-gate");
        std::fs::write(&hook_gate, "").unwrap();
        std::fs::write(
            src.join("pre"),
            format!("#!/bin/sh\ntouch {0}.entered; while [ ! -e {0}.release ] && [ -e {0} ]; do sleep 0.02; done\n", hook_gate.display()),
        )
        .unwrap();
        std::fs::set_permissions(src.join("pre"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let file = paths::config_file();
        let text = std::fs::read_to_string(&file).unwrap().replace("[modules]\nenabled = []\n", "");
        std::fs::write(&file, format!("{text}[modules]\nenabled = []\n[sources]\nenabled = [\"fake\"]\n")).unwrap();

        let install = |id: &str, store_id: &str| {
            let folder = dir.join("games").join(id);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("launch.exe"), id).unwrap();
            let mut g = Game::load(&paths::game_dir(id).join("game.toml")).unwrap_or_else(|_| Game::new(id));
            g.source.kind = "fake".into();
            g.source.id = store_id.into();
            g.source.dir = folder.to_string_lossy().into();
            g.save().unwrap();
            folder
        };
        let sample = install("sample", "s1");
        let other = install("other", "s2");
        let config = crate::config::Config::load().unwrap();
        let prefix = config.prefixes_root().join("shared");
        let linked = dir.join("linked-prefix");
        std::fs::create_dir_all(&prefix).unwrap();
        std::os::unix::fs::symlink(&prefix, &linked).unwrap();
        let mut g = Game::load(&paths::game_dir("sample").join("game.toml")).unwrap();
        g.launch.pre_command = format!("touch {0}.entered; while [ ! -e {0}.release ] && [ -e {0} ]; do sleep 0.02; done", gate.display());
        g.launch.runner = "wine".into();
        g.launch.runner_exe = "/bin/true".into();
        g.launch.prefix = linked.to_string_lossy().into();
        g.save().unwrap();
        std::fs::write(&gate, "").unwrap();
        let (core, memory) = open().await;
        let clip = config.recordings_root().join("sample/clip.mkv");
        for f in [prefix.join("user.reg"), clip.clone(), paths::game_logs_dir("sample").join("last.log")] {
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(&f, "kept").unwrap();
        }
        let toml_of = || std::fs::read_to_string(paths::game_dir("sample").join("game.toml")).unwrap();
        let game_toml = toml_of();
        let untouched = |when: &str, game_toml: &str| {
            assert_eq!(std::fs::read_to_string(sample.join("launch.exe")).unwrap(), "sample", "{when}: its install");
            for f in [prefix.join("user.reg"), clip.clone(), paths::game_logs_dir("sample").join("last.log")] {
                assert!(f.is_file(), "{when}: {} stays", f.display());
            }
            assert_eq!(toml_of(), game_toml, "{when}: its game.toml");
            assert!(!asked.exists(), "{when}: the store is not asked");
        };
        let busy = |(uninstall, purge, update): (Result<()>, Result<()>, Result<usize>), when: &str| {
            assert!(matches!(uninstall, Err(Error::Busy(_))), "{when}: uninstall is {uninstall:?}");
            assert!(matches!(purge, Err(Error::Busy(_))), "{when}: rm --purge is {purge:?}");
            assert!(matches!(update, Err(Error::Busy(_))), "{when}: a store update is {update:?}");
        };

        let held_at = |gate: std::path::PathBuf| {
            let core = &core;
            async move {
                while !gate.with_extension("entered").exists() {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                let answers = (destroy(core).await, trash_leftovers(core).await);
                std::fs::write(gate.with_extension("release"), "").unwrap();
                answers
            }
        };
        let (sid, preparing, starting) = tokio::join!(core.launch("sample", "", ""), held_at(hook_gate.clone()), held_at(gate.clone()));
        let sid = sid.unwrap();
        busy(preparing.0, "in its pre-launch hook");
        leftovers_busy(preparing.1, "in its pre-launch hook");
        busy(starting.0, "starting");
        leftovers_busy(starting.1, "starting");
        untouched("starting", &game_toml);

        busy(destroy(&core).await, "running");
        leftovers_busy(trash_leftovers(&core).await, "running");
        assert_eq!(core.source_update("fake", "", None).await.unwrap(), 0, "updating all leaves the running game's pending");
        untouched("running", &game_toml);
        core.source_update("fake", "s2", None).await.unwrap();
        core.uninstall("other").await.unwrap();
        assert_eq!(std::fs::read_to_string(&asked).unwrap(), "update s2\nuninstall s2\n", "another game is not refused");
        assert!(other.join("launch.exe").is_file(), "its files are the store's to remove");
        std::fs::remove_file(&asked).unwrap();
        core.remove("sample", false).await.unwrap();
        assert!(matches!(core.source_trash("fake", "s1").await, Err(Error::Busy(_))), "removed from the library, its files still are the running game's");
        core.restore("sample").await.unwrap();

        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let held = crate::saves::hold_game("sample").await.unwrap();
        busy(destroy(&core).await, "its saves backing up");
        leftovers_busy(trash_leftovers(&core).await, "its saves backing up");
        untouched("its saves backing up", &game_toml);
        drop(held);
        core.remove("sample", false).await.unwrap();
        core.trash_leftover(&prefix.to_string_lossy()).await.unwrap();
        assert!(!prefix.exists(), "once its game is idle, its prefix goes to the trash");
        core.restore("sample").await.unwrap();
        core.uninstall("sample").await.unwrap();
        assert_eq!(std::fs::read_to_string(&asked).unwrap(), "uninstall s1\n", "once the game and its backup ended");
    }

    #[tokio::test]
    async fn a_store_update_spares_a_busy_import_it_finds_installed() {
        let sb = sandbox();
        let dir = sb._env.path().to_path_buf();
        let (asked, scan_fails) = (dir.join("asked"), dir.join("scan-fails"));
        let src = dir.join("sources/fake");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("source.toml"), "api = 2\nid = \"fake\"\nname = \"Fake\"\nexe = \"run\"\ncapabilities = [\"update\"]\n").unwrap();
        let game = |id: &str| {
            format!(
                "echo '{{\"event\":\"game\",\"id\":\"{id}\",\"title\":\"{id}\",\"owned\":true,\"installed\":true,\"dir\":\"{}\"}}'",
                dir.join("games").join(id).display()
            )
        };
        std::fs::write(
            src.join("run"),
            format!(
                "#!/bin/sh\ncase \"$1$2\" in\nupdate) for id in s1 s2 s3; do echo \"{{\\\"event\\\":\\\"update\\\",\\\"id\\\":\\\"$id\\\"}}\"; done ;;\nscan) [ -e '{}' ] && exit 1; {}; {}; {} ;;\nupdate*) echo \"$1 $2\" >> '{}' ;;\nesac\necho '{{\"event\":\"done\"}}'\n",
                scan_fails.display(),
                game("s1"),
                game("s2"),
                game("s3"),
                asked.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(src.join("run"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let file = paths::config_file();
        let text = std::fs::read_to_string(&file).unwrap().replace("[modules]\nenabled = []\n", "");
        std::fs::write(&file, format!("{text}[modules]\nenabled = []\n[sources]\nenabled = [\"fake\"]\n")).unwrap();
        std::fs::create_dir_all(dir.join("games/s3")).unwrap();
        for (id, kind, store_id, folder) in [("sample", "", "", Some("s1")), ("other", "fake", "s2", Some("s2")), ("named", "", "s3", None)] {
            let mut g = Game::load(&paths::game_dir(id).join("game.toml")).unwrap_or_else(|_| Game::new(id));
            if let Some(folder) = folder.map(|f| dir.join("games").join(f)) {
                std::fs::create_dir_all(&folder).unwrap();
                g.source.dir = folder.to_string_lossy().into();
            }
            g.source.kind = kind.into();
            g.source.id = store_id.into();
            g.save().unwrap();
        }
        let (core, _memory) = open().await;
        core.launch("sample", "", "").await.unwrap();
        let _held = crate::saves::hold_game("named").await.unwrap();

        for scan in ["answers", "fails"] {
            for (sid, why) in [("s1", "the running import's folder"), ("s3", "the id of an import backing up its saves")] {
                let refused = core.source_update("fake", sid, None).await;
                assert!(matches!(refused, Err(Error::Busy(_))), "the store's {sid} is {why}, its scan {scan}: {refused:?}");
            }
            assert!(!asked.exists(), "its scan {scan}: the store is not asked");
            std::fs::write(&scan_fails, "").unwrap();
        }
        std::fs::remove_file(&scan_fails).unwrap();
        assert_eq!(core.source_update("fake", "", None).await.unwrap(), 1, "updating all skips the busy imports");
        assert_eq!(std::fs::read_to_string(&asked).unwrap(), "update s2\n", "and still updates another game");
    }

    #[tokio::test]
    async fn a_history_that_cannot_be_written_holds_back_neither_the_undo_nor_the_next_game() {
        use std::os::unix::fs::PermissionsExt;
        let sb = sandbox();
        let module = std::path::PathBuf::from(std::env::var_os("UNIVERSE_MODULES_PATH").unwrap()).join("log");
        std::fs::create_dir_all(module.join("bin")).unwrap();
        std::fs::write(module.join("module.toml"), "api = 2\nid = \"log\"\n[hooks]\nsession-end = \"bin/end\"\npost-process = \"bin/post\"\n").unwrap();
        let ended = paths::state_home().join("ended");
        for (hook, body) in [("end", format!("#!/bin/sh\necho \"$GAME_ID\" >> {}\n", ended.display())), ("post", "#!/bin/sh\n".into())] {
            std::fs::write(module.join("bin").join(hook), body).unwrap();
            std::fs::set_permissions(module.join("bin").join(hook), std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            paths::config_home().join("config.toml"),
            "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[saves]\nauto_backup = false\n[modules]\nenabled = [\"log\"]\n",
        )
        .unwrap();
        let (core, memory) = open().await;
        let sample = core.get("sample").await.unwrap().game;
        let mut other = Game::new("Other");
        other.launch = sample.launch.clone();
        other.launch.post_command = "true".into();
        other.save().unwrap();
        core.reload_game("other").await.unwrap();
        let history = sample.sessions_path();

        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        let post_process = format!("universe-log-post-process-{sid}");
        std::fs::create_dir_all(&history).unwrap();
        memory.finish(&unit, 3);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert!(read_marker().is_none(), "the slot is free for the next game");
        assert!(sb.post_ran.exists(), "post_command ran");
        assert_eq!(memory.calls().iter().filter(|c| *c == "cursor:restore").count(), 1);
        assert_eq!(std::fs::read_to_string(&ended).unwrap(), "sample\n", "session-end hooks are cleanup: they run with the undo");
        assert!(memory.spec(&post_process).is_none(), "post-process reads the row: it waits with it");
        assert_eq!(core.closed_row("sample", &sid).await.map(|s| s.exit), Some(3), "a frontend hears how it ended");
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(std::fs::read_to_string(&ended).unwrap(), "sample\n", "idempotent");
        let clip = paths::state_home().join("clip.mkv");
        std::fs::write(&clip, b"").unwrap();
        let filed_clip = core.file_recording(&sid, clip.to_str().unwrap(), None).await.unwrap();

        std::fs::remove_file(&sb.post_ran).unwrap();
        let (core, memory) = open().await;
        assert!(!sb.post_ran.exists() && !memory.calls().contains(&"cursor:restore".to_string()), "reopening repeats no undo");

        let other_sid = core.launch("other", "", "").await.unwrap();
        memory.finish(&format!("universe-game-other-{other_sid}.service"), 0);
        core.session_end("other", &other_sid, None, None).await.unwrap();
        let other_history = core.get("other").await.unwrap().game.sessions_path();
        let other_rows = std::fs::read(&other_history).unwrap();
        assert_eq!(sessions::read(&other_history).unwrap().len(), 1);

        let waiting: Vec<_> = closings().iter().map(|c| closing_file(&c.row.game, &c.row.session)).collect();
        assert_eq!(waiting.len(), 1, "{waiting:?}");
        let kept = std::fs::read(&waiting[0]).unwrap();
        std::fs::remove_dir(&history).unwrap();
        std::fs::write(&history, "{\"session\":\"cut sh").unwrap();
        let (core, memory) = open().await;
        let rows = sessions::read(&history).unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!((rows[0].session.as_str(), rows[0].exit, rows[0].unit.as_str()), (sid.as_str(), 3, unit.as_str()));
        assert_eq!(rows[0].recording.as_deref(), Some(filed_clip.as_str()), "a recording filed while it waited lands on the row");
        assert_eq!(core.get("sample").await.unwrap().sessions.len(), 1, "the library reads it at once");
        assert_eq!(memory.spec(&post_process).expect("post-process, once the row is there").env["RECORDING_PATH"], filed_clip);

        std::fs::write(&waiting[0], kept).unwrap();
        let (_core, memory) = open().await;
        assert_eq!(sessions::read(&history).unwrap().len(), 1, "a stop between the line and the record's note files nothing twice");
        assert!(memory.spec(&post_process).is_some(), "post-process, which follows the note, runs");
        assert!(!waiting[0].exists());
        assert_eq!(std::fs::read(&other_history).unwrap(), other_rows, "the other game's history is as it was");
    }

    #[tokio::test]
    async fn a_history_it_cannot_read_takes_no_row_it_might_already_hold() {
        use std::os::unix::fs::PermissionsExt;
        let _sb = sandbox();
        let (core, memory) = open().await;
        let history = core.get("sample").await.unwrap().game.sessions_path();
        let sid = core.launch("sample", "", "").await.unwrap();
        std::fs::write(&history, "").unwrap();
        std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o200)).unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        std::fs::set_permissions(&history, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(std::fs::read(&history).unwrap(), b"", "nothing landed blind");
        open().await;
        assert_eq!(sessions::read(&history).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_refused_start_leaves_nothing_behind() {
        let sb = sandbox();
        let (core, memory) = open().await;
        memory.refuse_starts(true);
        let err = core.launch("sample", "", "").await.unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err}");
        assert!(read_marker().is_none(), "no marker left");
        assert!(sb.post_ran.exists(), "post_command undoes pre_command");
        let calls = memory.calls();
        assert_eq!(calls[0], "cursor:enable");
        assert!(calls[1].starts_with("unit:start universe-game-sample-"), "{calls:?}");
        assert_eq!(calls[2..], ["cursor:restore"]);
        memory.refuse_starts(false);
        assert!(core.launch("sample", "", "").await.is_ok());
    }

    #[tokio::test]
    async fn adopt_scope_on_the_memory_host_is_idempotent() {
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
        memory.finish(&unit, 11);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let row = &core.sessions("sample").await.unwrap()[0];
        assert!(row.session.stopped == Some(true) && row.session.exit == 11, "{:?}", row.session);
        assert_eq!(sessions::end_of(&row.session), "stopped", "a game segfaulting in the shutdown a stop asked for is no crash");
        assert_eq!(row.session.command, command);
        let json = serde_json::to_value(row).unwrap();
        assert_eq!((json["end"].as_str(), json["debug_log"].is_null(), json.get("command")), (Some("stopped"), true, None));
        let after = core.session_log("sample", &sid, 0).await.unwrap();
        assert_eq!(after[0].message, format!("launch {sid}: {command}"));
        assert!(matches!(core.session_log("sample", "19700101-000000", 0).await, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    async fn an_unasked_signal_is_a_kill_and_a_code_a_crash() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        memory.finish(&format!("universe-game-sample-{sid}.service"), 11);
        core.session_end("sample", &sid, Some(11), None).await.unwrap();
        assert_eq!(sessions::end_of(&core.sessions("sample").await.unwrap()[0].session), "crashed");
    }

    #[tokio::test]
    async fn a_proton_crash_report_files_a_crash_whatever_the_exit() {
        let _sb = sandbox();
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.runner = "proton".into();
        g.launch.runner_exe.clear();
        g.save().unwrap();
        // Two games: two launches within a second share a session id.
        Game { id: "crashy".into(), ..g }.save().unwrap();
        let (core, memory) = open().await;
        for (id, crash) in [("sample", false), ("crashy", true)] {
            let sid = core.launch(id, "", "").await.unwrap();
            let unit = format!("universe-game-{id}-{sid}.service");
            let dir = paths::session_log_dir(id, &sid);
            assert_eq!(memory.spec(&unit).unwrap().env["PROTON_CRASH_REPORT_DIR"], dir.to_string_lossy());
            assert!(dir.is_dir() && !dir.join("launch.txt").exists(), "made for winedbg; the launch file is debug_log's");
            if crash {
                std::fs::write(dir.join("2026-10-10_02:04:06_appid-578330_crash.log"), "Unhandled exception: page fault").unwrap();
            }
            memory.finish(&unit, 0);
            core.session_end(id, &sid, Some(0), None).await.unwrap();
            let row = core.sessions(id).await.unwrap().remove(0);
            assert_eq!((row.session.crashed, sessions::end_of(&row.session)), (crash, if crash { "crashed" } else { "quit" }), "umu-run exits 0 either way");
            assert_eq!(row.debug_log, crash.then(|| dir.to_string_lossy().into_owned()), "an empty dir goes, a report stays readable");
        }
    }

    #[tokio::test]
    async fn a_launch_drops_the_fonts_its_prefix_registered_from_store_files_now_gone() {
        use std::os::unix::fs::PermissionsExt;
        let _sb = sandbox();
        let prefix = paths::state_home().join("compatdata/620");
        let pfx = prefix.join("pfx");
        std::fs::create_dir_all(pfx.join("drive_c")).unwrap();
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.runner = "proton".into();
        g.launch.runner_exe.clear();
        g.launch.prefix = prefix.to_string_lossy().into();
        g.save().unwrap();
        // Two games: two launches within a second share a session id.
        Game { id: "again".into(), ..g }.save().unwrap();
        let live = pfx.join("drive_c/Live.ttf");
        std::fs::write(&live, b"").unwrap();
        let z = |p: &str| format!("Z:{}", p.replace('/', r"\\"));
        let (gone, usr, live) =
            (z("/nix/store/00000000000000000000000000000000-gone/share/fonts/Gone.ttf"), z("/usr/share/fonts/TTF/Usr.ttf"), z(&live.to_string_lossy()));
        let dead = format!("\"Gone (TrueType)\"=\"{gone}\"\n");
        let reg = |keys: &[&str]| {
            let mut text = String::from("WINE REGISTRY Version 2\n;; All keys relative to \\\\Machine\n\n#arch=win64\n");
            for key in keys {
                text += &format!("\n[{key}] 1792921511\n#time=1dc45a1b2c3d4e5\n{dead}\"Usr (TrueType)\"=\"{usr}\"\n\"Live (TrueType)\"=\"{live}\"\n");
            }
            text + &format!("\n[Software\\\\Wine\\\\Fonts\\\\Replacements] 1747721663\n\"Gone\"=\"{gone}\"\n")
        };
        let user = reg(&[r"Software\\Wine\\Fonts\\External Fonts"]);
        let system = reg(&[r"Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts", r"Software\\Microsoft\\Windows\\CurrentVersion\\Fonts"]);
        std::fs::write(pfx.join("user.reg"), &user).unwrap();
        std::fs::write(pfx.join("system.reg"), &system).unwrap();
        std::fs::set_permissions(pfx.join("user.reg"), std::fs::Permissions::from_mode(0o600)).unwrap();
        let read = |name: &str| std::fs::read_to_string(pfx.join(name)).unwrap();

        let server = paths::state_home().join(crate::data::server_dir_name(&pfx).unwrap());
        std::fs::create_dir_all(&server).unwrap();
        let wineserver = paths::state_home().join("wineserver");
        std::fs::write(&wineserver, "#!/bin/sh\nread _\n").unwrap();
        std::fs::set_permissions(&wineserver, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut running = std::process::Command::new(&wineserver).current_dir(&server).stdin(std::process::Stdio::piped()).spawn().unwrap();
        let (core, memory) = open().await;
        let sid = core.launch("sample", "", "").await.unwrap();
        assert_eq!((read("user.reg"), read("system.reg")), (user.clone(), system.clone()), "a running wineserver's registry is left to it");
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, Some(0), None).await.unwrap();
        running.kill().unwrap();
        running.wait().unwrap();

        core.launch("again", "", "").await.unwrap();
        assert_eq!(read("user.reg"), user.replace(&dead, ""), "only the store font now gone goes; /usr/share's and other keys' values stay");
        assert_eq!(read("system.reg"), system.replace(&dead, ""));
        assert_eq!(std::fs::metadata(pfx.join("user.reg")).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[tokio::test]
    async fn a_signal_nobody_asked_for_is_a_kill() {
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
    async fn a_reconnect_runs_the_running_games_reconnect_hooks() {
        let _sb = sandbox();
        let probe = std::path::PathBuf::from(std::env::var_os("UNIVERSE_MODULES_PATH").unwrap()).join("probe");
        std::fs::create_dir_all(probe.join("bin")).unwrap();
        std::fs::write(probe.join("module.toml"), "api = 2\nid = \"probe\"\n[hooks]\nreconnect = \"bin/reconnect\"\n").unwrap();
        let log = paths::state_home().join("hooks.log");
        let exe = probe.join("bin/reconnect");
        std::fs::write(&exe, format!("#!/bin/sh\necho reconnect $SESSION_ID $MODULE_SETTINGS_JSON >> {}\n", log.display())).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            paths::config_home().join("config.toml"),
            "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = [\"probe\"]\n",
        )
        .unwrap();
        let (core, _memory) = open().await;
        assert!(matches!(core.reconnect_hooks().await, Err(Error::NotFound(_))), "no game, no session to tell");
        let sid = core.launch("sample", "", "").await.unwrap();
        core.reconnect_hooks().await.unwrap();
        let lines: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
        assert!(lines.len() == 1 && lines[0].starts_with(&format!("reconnect {sid} {{")), "{lines:?}");
    }

    #[tokio::test]
    async fn a_module_runs_its_hooks_only_for_the_runner_kinds_it_applies_to() {
        let _sb = sandbox();
        let root = std::path::PathBuf::from(std::env::var_os("UNIVERSE_MODULES_PATH").unwrap());
        let log = paths::state_home().join("hooks.log");
        use std::os::unix::fs::PermissionsExt;
        for (id, kinds) in [("emu", "emulator"), ("pc", "proton")] {
            let dir = root.join(id);
            std::fs::create_dir_all(dir.join("bin")).unwrap();
            std::fs::write(
                dir.join("module.toml"),
                format!("api = 2\nid = \"{id}\"\n[applies]\nrunner_kinds = [\"{kinds}\"]\n[hooks]\npre-launch = \"bin/pre\"\n"),
            )
            .unwrap();
            let exe = dir.join("bin/pre");
            std::fs::write(&exe, format!("#!/bin/sh\necho {id} >> {0}\n[ -e {0}.refused ] || {{ touch {0}.refused; exit 1; }}\n", log.display())).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            paths::config_home().join("config.toml"),
            "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = [\"emu\", \"pc\"]\n",
        )
        .unwrap();
        let (core, _memory) = open().await;
        let json = core.get("sample").await.unwrap().to_json();
        let listed: Vec<&str> = json["effective"]["modules"].as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(listed, ["emu"], "a Dolphin game carries no Proton module's settings");
        assert!(matches!(core.launch("sample", "", "").await, Err(Error::Io(_))), "a hook refusing refuses the launch");
        assert!(read_marker().is_none(), "and holds the game no longer");
        core.launch("sample", "", "").await.unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "emu\nemu\n", "the Proton module's hook stays out of an emulator's launch");
    }

    #[tokio::test]
    async fn the_games_own_source_runs_its_hooks_around_the_session() {
        let _sb = sandbox();
        let store = std::path::PathBuf::from(std::env::var_os("UNIVERSE_SOURCES_PATH").unwrap()).join("store");
        std::fs::create_dir_all(store.join("bin")).unwrap();
        std::fs::write(
            store.join("source.toml"),
            "api = 2\nid = \"store\"\nexe = \"bin/run\"\n[hooks]\npre-launch = \"bin/pre\"\npost-launch = \"bin/watch\"\nsession-end = \"bin/end\"\ntimeout_s = 7\n\
             [[settings]]\nkey = \"extras\"\ntype = \"bool\"\ndefault = true\nscope = \"game\"\n",
        )
        .unwrap();
        let log = paths::state_home().join("hooks.log");
        use std::os::unix::fs::PermissionsExt;
        for hook in ["run", "pre", "watch", "end"] {
            let exe = store.join("bin").join(hook);
            std::fs::write(&exe, format!("#!/bin/sh\necho {hook} $SESSION_ID $SOURCE_GAME_ID $SOURCE_SETTINGS_JSON >> {}\n", log.display())).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(
            paths::config_home().join("config.toml"),
            "[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\n[modules]\nenabled = []\n[sources]\nenabled = [\"store\"]\n",
        )
        .unwrap();
        let mut g = Game::load(&paths::game_dir("sample").join("game.toml")).unwrap();
        let (core, memory) = open().await;
        assert!(core.hook_modules(&core.get("sample").await.unwrap(), "pre-launch").await.is_empty(), "a manual game runs no source's hooks");

        g.source.kind = "store".into();
        g.source.id = "77".into();
        g.sources.insert("store".into(), toml::from_str("extras = false").unwrap());
        g.save().unwrap();
        core.reload_game("sample").await.unwrap();
        let sid = core.launch("sample", "", "").await.unwrap();
        let unit = format!("universe-game-sample-{sid}.service");
        assert!(memory.spec(&unit).unwrap().properties.contains(&("TimeoutStopUSec".to_string(), Prop::U64(67_000_000))), "its session-end counts");
        let watch = memory.spec(&format!("universe-store-post-launch-{sid}")).expect("the post-launch unit");
        assert_eq!(watch.bind_to.as_deref(), Some(unit.as_str()));
        assert_eq!(watch.env["SOURCE_DIR"], store.to_string_lossy());
        assert_eq!(watch.env["SOURCE_GAME_ID"], "77");
        assert_eq!(watch.env["RECORDINGS_DIR"], core.config.read().await.recordings_root().join("sample").to_string_lossy());
        memory.finish(&unit, 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        let lines: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with(&format!("pre {sid} 77 {{")) && lines[1].starts_with(&format!("end {sid} 77 {{")), "{lines:?}");
        assert!(lines[0].contains("\"extras\":false"), "the game's own setting: {}", lines[0]);
    }

    #[tokio::test]
    async fn a_decks_controls_set_during_a_game_come_back_at_its_next_launch() {
        let _sb = sandbox();
        let deck = crate::hardware::tests::deck();
        let (host, memory) = Host::memory();
        let host = Host { machine: crate::hardware::Machine::at(deck.path()), ..host };
        let core = Core::open_with(crate::config::Config::load().unwrap(), host).await.unwrap();
        let cap = deck.path().join("sys/class/hwmon/hwmon5/power1_cap");
        let watts = || std::fs::read_to_string(&cap).unwrap().trim().to_string();
        let finish = |sid: &str| memory.finish(&format!("universe-game-sample-{sid}.service"), 0);

        let sid = core.launch("sample", "", "").await.unwrap();
        core.set_system_for("sample", "tdp", "9").await.unwrap();
        assert_eq!(watts(), "9000000", "set during the game, on at once");
        assert_eq!(core.get("sample").await.unwrap().game.system["tdp"], "9", "and kept as the game's own");
        finish(&sid);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(watts(), "15000000", "its end puts the machine's own back");

        let sid = core.launch("sample", "", "").await.unwrap();
        assert_eq!(watts(), "9000000", "the next launch brings the game's back");
        assert_eq!(undo_steps(&read_marker().unwrap()), ["post_command", "cursor", "system"]);
        core.set_system("tdp", "12").await.unwrap();
        assert_eq!(watts(), "9000000", "the machine's own waits while the game holds its own");
        finish(&sid);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(watts(), "12000000", "then comes on with its end");

        let sid = core.launch("sample", "", "").await.unwrap();
        assert!(core.set_system_all("sample", "tdp", "fast").await.is_err());
        assert_eq!(core.get("sample").await.unwrap().game.system["tdp"], "9", "a refused value leaves the game's own");
        core.set_system_all("sample", "tdp", "10").await.unwrap();
        assert_eq!(watts(), "10000000", "every game's value is on at once");
        assert!(core.get("sample").await.unwrap().game.system.is_empty(), "the game lets its own go");
        assert_eq!(core.config.read().await.system.tdp, "10");
        finish(&sid);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(watts(), "10000000", "and stays after the game");
    }

    #[tokio::test]
    async fn a_deck_control_that_cannot_be_kept_leaves_the_machine_as_it_was() {
        let _sb = sandbox();
        let deck = crate::hardware::tests::deck();
        let (host, memory) = Host::memory();
        let host = Host { machine: crate::hardware::Machine::at(deck.path()), ..host };
        let core = Core::open_with(crate::config::Config::load().unwrap(), host).await.unwrap();
        let cap = deck.path().join("sys/class/hwmon/hwmon5/power1_cap");
        let watts = || std::fs::read_to_string(&cap).unwrap().trim().to_string();
        // A directory where the file was: no write lands, whoever the tests run as.
        let unwritable = |file: &Path| {
            std::fs::remove_file(file).unwrap();
            std::fs::create_dir(file).unwrap();
        };
        let sid = core.launch("sample", "", "").await.unwrap();

        let game_toml = core.get("sample").await.unwrap().game.toml_path();
        let text = std::fs::read_to_string(&game_toml).unwrap();
        unwritable(&game_toml);
        assert!(core.set_system_for("sample", "tdp", "9").await.is_err());
        assert_eq!(watts(), "15000000", "a game's value it cannot keep is not left on the machine");
        std::fs::remove_dir(&game_toml).unwrap();
        std::fs::write(&game_toml, text).unwrap();

        core.set_system_for("sample", "tdp", "9").await.unwrap();
        unwritable(&paths::config_file());
        assert!(core.set_system_all("sample", "tdp", "10").await.is_err());
        assert_eq!(watts(), "9000000", "nor is every game's");
        assert_eq!(core.get("sample").await.unwrap().game.system["tdp"], "9", "and the game keeps its own");
        memory.finish(&format!("universe-game-sample-{sid}.service"), 0);
        core.session_end("sample", &sid, None, None).await.unwrap();
        assert_eq!(watts(), "15000000", "its end puts back the machine's own from before");
    }

    #[tokio::test]
    async fn reconcile_drops_a_marker_it_cannot_read() {
        let _sb = sandbox();
        std::fs::write(paths::current_session_file(), "{\"session_id\": 1").unwrap();
        let (core, _memory) = open().await;
        assert!(!paths::current_session_file().exists());
        assert!(core.launch("sample", "", "").await.is_ok());
    }
}
