use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rustix::fs::inotify::{self, ReadFlags, WatchFlags};
use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::core::Core;
use crate::session::{read_marker, Current};
use crate::{paths, Error, Result};

// The module's 30-minute timeout turns an entry failed without writing a file.
const PENDING_POLL: Duration = Duration::from_secs(10);
const SWEEP_FLOOR: Duration = Duration::from_secs(60);
const SWEEP_CEILING: Duration = Duration::from_secs(30 * 60);
// `session-end` files the line before it removes the marker: one still there this long after the unit went was left by a failed end.
const MARKER_GRACE: Duration = Duration::from_secs(5);
const GAME_PARTS: [&str; 6] = ["journal", "journal/attachments", "media", "media/picked", "media/picked/screenshots", "screenshots"];

#[derive(Debug, Clone)]
pub enum Event {
    /// These games were reloaded (a new or a removed one among them); empty when the whole library was.
    Library(Vec<String>),
    /// The game's art, the player's picks in `media/picked/` included. Each of these follows the game's `Library`.
    Media(String),
    Journal(String),
    Screenshots(String),
    SessionStarted(Current),
    SessionEnded(Ended),
    /// Once per session.
    JournalWriting {
        id: String,
        session: String,
        title: String,
    },
    /// The entry left `pending`: `text` is a `written` entry's title, a `deferred` or `failed` one's reason.
    JournalDone {
        id: String,
        session: String,
        state: String,
        text: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Ended {
    pub session_id: String,
    pub id: String,
    pub title: String,
    pub duration_s: u64,
    /// `sessions::end_of`; empty when the session filed no line.
    pub end: String,
    pub exit: i32,
    pub recording: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Options {
    /// One reload per burst of writes: a media refresh, a session end.
    pub debounce: Duration,
    /// Behind the `state/` watch while a marker exists: the game is a systemd unit, not a child.
    pub poll: Duration,
    /// Hand the journal module the entries it owes, one at a time, and report the ones it writes.
    pub journal_sweep: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { debounce: Duration::from_millis(300), poll: Duration::from_secs(2), journal_sweep: true }
    }
}

#[derive(Debug)]
pub struct Watch {
    commands: mpsc::UnboundedSender<Command>,
    task: tokio::task::JoinHandle<()>,
}

enum Command {
    Check,
}

impl Watch {
    /// Reads the session marker now rather than on the next write or poll: the frontend's own `launch` or `stop` just returned.
    pub fn check(&self) {
        let _ = self.commands.send(Command::Check);
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Only what changes after it returns is reported: read the library then. A session already running comes first, as `SessionStarted`.
pub async fn watch(core: Arc<Core>, options: Options) -> Result<(Watch, mpsc::UnboundedReceiver<Event>)> {
    let roots = Roots { games: paths::games_dir(), state: paths::state_home() };
    for dir in [&roots.games, &roots.state] {
        std::fs::create_dir_all(dir)?;
    }
    let fd = inotify::init(inotify::CreateFlags::NONBLOCK | inotify::CreateFlags::CLOEXEC).map_err(|e| Error::Io(format!("inotify: {e}")))?;
    let (events, receiver) = mpsc::unbounded_channel();
    let (commands, command_rx) = mpsc::unbounded_channel();
    let mut watcher = Watcher {
        core,
        options,
        events,
        fd: AsyncFd::new(fd)?,
        roots,
        watched: HashMap::new(),
        limit_told: false,
        dirty: Dirty::default(),
        tracked: None,
        ending: None,
        writing: BTreeMap::new(),
        announced: HashSet::new(),
        awaiting: None,
        flush_at: None,
        poll_at: None,
        pending_at: None,
        sweep_at: None,
    };
    watcher.rewatch();
    let task = tokio::spawn(watcher.run(command_rx));
    Ok((Watch { commands, task }, receiver))
}

/// When to sweep again for the entry owed at `next` (RFC 3339, `""` for none).
pub fn sweep_wake(next: &str, now: chrono::DateTime<chrono::Local>) -> Option<Duration> {
    let at = chrono::DateTime::parse_from_rfc3339(next).ok()?;
    let secs = (at.with_timezone(&chrono::Local) - now).num_seconds().max(0) as u64;
    Some(Duration::from_secs(secs).clamp(SWEEP_FLOOR, SWEEP_CEILING))
}

struct Roots {
    games: PathBuf,
    state: PathBuf,
}

#[derive(Default)]
struct Dirty {
    whole: bool,
    state: bool,
    games: BTreeSet<String>,
    media: BTreeSet<String>,
    journal: BTreeSet<String>,
    screenshots: BTreeSet<String>,
}

impl Roots {
    fn classify(&self, dirty: &mut Dirty, dir: &Path, name: Option<&str>) {
        if dir == self.state {
            dirty.state |= name.is_none_or(|n| n.starts_with("current-session"));
        } else if dir == self.games {
            match name {
                Some(n) => {
                    dirty.games.insert(n.into());
                }
                None => dirty.whole = true,
            }
        } else if let Ok(rel) = dir.strip_prefix(&self.games) {
            let mut parts = rel.iter().map(|p| p.to_string_lossy().into_owned());
            let Some(id) = parts.next() else { return };
            let set = match parts.next().as_deref() {
                None => &mut dirty.games,
                Some("journal") => &mut dirty.journal,
                Some("media") => &mut dirty.media,
                Some("screenshots") => &mut dirty.screenshots,
                Some(_) => return,
            };
            set.insert(id);
        }
    }
}

struct Raw {
    wd: i32,
    flags: ReadFlags,
    name: Option<String>,
}

struct Watcher {
    core: Arc<Core>,
    options: Options,
    events: mpsc::UnboundedSender<Event>,
    fd: AsyncFd<OwnedFd>,
    roots: Roots,
    watched: HashMap<i32, PathBuf>,
    limit_told: bool,
    dirty: Dirty,
    tracked: Option<Current>,
    /// When the tracked session's unit was first seen gone while its marker stayed.
    ending: Option<Instant>,
    /// Session → (game, game title): the entries being written.
    writing: BTreeMap<String, (String, String)>,
    announced: HashSet<String>,
    /// An entry was just handed to the module, which has not marked it pending yet: no second one before then.
    awaiting: Option<Instant>,
    flush_at: Option<Instant>,
    poll_at: Option<Instant>,
    pending_at: Option<Instant>,
    sweep_at: Option<Instant>,
}

fn mask() -> WatchFlags {
    WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::CLOSE_WRITE
        | WatchFlags::DELETE_SELF
        | WatchFlags::ONLYDIR
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    rd.flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')).map(|e| e.path()).filter(|p| p.is_dir()).collect()
}

async fn read_events(fd: &AsyncFd<OwnedFd>) -> std::io::Result<Vec<Raw>> {
    let mut guard = fd.readable().await?;
    let mut out = Vec::new();
    {
        let mut buf = [MaybeUninit::uninit(); 8192];
        let mut reader = inotify::Reader::new(guard.get_inner(), &mut buf);
        loop {
            match reader.next() {
                Ok(e) => out.push(Raw { wd: e.wd(), flags: e.events(), name: e.file_name().map(|n| n.to_string_lossy().into_owned()) }),
                Err(rustix::io::Errno::WOULDBLOCK) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
    guard.clear_ready();
    Ok(out)
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

fn due(at: &mut Option<Instant>, now: Instant) -> bool {
    at.take_if(|t| *t <= now).is_some()
}

fn text_of(v: &serde_json::Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

impl Watcher {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        self.check_session().await;
        if self.options.journal_sweep {
            self.refresh_writing().await;
        }
        while !self.events.is_closed() {
            let wake = [self.flush_at, self.poll_at, self.pending_at, self.sweep_at].into_iter().flatten().min();
            tokio::select! {
                read = read_events(&self.fd) => match read {
                    Ok(raw) => self.mark(raw),
                    Err(e) => {
                        tracing::warn!("change watch: {e}");
                        return;
                    }
                },
                command = commands.recv() => match command {
                    Some(Command::Check) => self.check_session().await,
                    None => return,
                },
                () = sleep_until(wake) => self.wake().await,
            }
        }
    }

    fn send(&self, event: Event) {
        let _ = self.events.send(event);
    }

    async fn wake(&mut self) {
        let now = Instant::now();
        if due(&mut self.flush_at, now) {
            self.flush().await;
        }
        if due(&mut self.poll_at, now) {
            self.check_session().await;
        }
        if due(&mut self.pending_at, now) | due(&mut self.sweep_at, now) {
            self.refresh_writing().await;
        }
    }

    fn mark(&mut self, raw: Vec<Raw>) {
        for e in raw {
            if e.flags.contains(ReadFlags::QUEUE_OVERFLOW) {
                self.dirty.whole = true;
                self.dirty.state = true;
                continue;
            }
            let Some(dir) = self.watched.get(&e.wd).cloned() else { continue };
            if e.flags.contains(ReadFlags::IGNORED) {
                self.watched.remove(&e.wd);
            }
            if e.name.as_deref().is_some_and(|n| n.starts_with('.')) {
                continue;
            }
            self.roots.classify(&mut self.dirty, &dir, e.name.as_deref());
        }
        self.flush_at = Some(Instant::now() + self.options.debounce);
    }

    /// Every directory a frontend shows something from, watched; returns the ones newly watched, whose content nobody saw arrive.
    fn rewatch(&mut self) -> Vec<PathBuf> {
        let mut wanted = vec![self.roots.games.clone(), self.roots.state.clone()];
        for game in subdirs(&self.roots.games) {
            wanted.extend(GAME_PARTS.iter().map(|p| game.join(p)).filter(|p| p.is_dir()));
            wanted.push(game);
        }
        let wanted: HashSet<PathBuf> = wanted.into_iter().collect();
        let stale: Vec<i32> = self.watched.iter().filter(|(_, dir)| !wanted.contains(*dir)).map(|(wd, _)| *wd).collect();
        for wd in stale {
            let _ = inotify::remove_watch(self.fd.get_ref(), wd);
            self.watched.remove(&wd);
        }
        let have: HashSet<PathBuf> = self.watched.values().cloned().collect();
        let mut added = Vec::new();
        for dir in wanted.into_iter().filter(|d| !have.contains(d)) {
            match inotify::add_watch(self.fd.get_ref(), &dir, mask()) {
                Ok(wd) => {
                    self.watched.insert(wd, dir.clone());
                    added.push(dir);
                }
                Err(e) if !self.limit_told => {
                    self.limit_told = true;
                    tracing::warn!("watch {}: {e}", dir.display());
                }
                Err(_) => {}
            }
        }
        added
    }

    async fn flush(&mut self) {
        let mut dirty = std::mem::take(&mut self.dirty);
        for dir in self.rewatch() {
            self.roots.classify(&mut dirty, &dir, None);
        }
        if dirty.whole {
            self.core.reload_all().await;
            self.send(Event::Library(vec![]));
        } else {
            let ids: BTreeSet<String> = dirty.games.iter().chain(&dirty.media).chain(&dirty.journal).chain(&dirty.screenshots).cloned().collect();
            let mut reloaded = Vec::new();
            for id in ids {
                match self.core.reload_game(&id).await {
                    Ok(()) => reloaded.push(id),
                    Err(e) => tracing::debug!("reload {id}: {e}"),
                }
            }
            if !reloaded.is_empty() {
                self.send(Event::Library(reloaded.clone()));
            }
            for id in reloaded {
                for (set, event) in
                    [(&dirty.media, Event::Media as fn(String) -> Event), (&dirty.journal, Event::Journal), (&dirty.screenshots, Event::Screenshots)]
                {
                    if set.contains(&id) {
                        self.send(event(id.clone()));
                    }
                }
            }
        }
        if dirty.state {
            self.check_session().await;
        }
        if self.options.journal_sweep && (dirty.whole || !dirty.journal.is_empty()) {
            self.refresh_writing().await;
        }
    }

    async fn check_session(&mut self) {
        let current = self.core.current().await;
        match (self.tracked.take(), current) {
            (Some(t), Some(c)) if t.session_id == c.session_id => {
                self.tracked = Some(c);
                self.ending = None;
            }
            (Some(t), Some(c)) => {
                self.end(t).await;
                self.start(c);
            }
            (None, Some(c)) => self.start(c),
            (Some(t), None) => {
                let lingering = read_marker().is_some_and(|m| m.current.session_id == t.session_id);
                let since = *self.ending.get_or_insert_with(Instant::now);
                if lingering && since.elapsed() < MARKER_GRACE {
                    self.tracked = Some(t);
                } else {
                    self.end(t).await;
                }
            }
            (None, None) => {}
        }
        self.poll_at = (self.tracked.is_some() || read_marker().is_some()).then(|| Instant::now() + self.options.poll);
    }

    fn start(&mut self, current: Current) {
        self.ending = None;
        self.tracked = Some(current.clone());
        self.send(Event::SessionStarted(current));
    }

    async fn end(&mut self, t: Current) {
        self.ending = None;
        if let Err(e) = self.core.reload_game(&t.id).await {
            tracing::debug!("reload {}: {e}", t.id);
        }
        let line = self.core.get(&t.id).await.ok().and_then(|r| r.sessions.into_iter().find(|s| s.session == t.session_id));
        let mut ended = Ended { session_id: t.session_id, id: t.id.clone(), title: t.title, ..Ended::default() };
        if let Some(s) = line {
            ended.end = crate::sessions::end_of(&s).into();
            ended.duration_s = s.duration_s;
            ended.exit = s.exit;
            ended.recording = s.recording.filter(|p| !p.is_empty());
        }
        self.send(Event::SessionEnded(ended));
        self.send(Event::Library(vec![t.id]));
        if self.options.journal_sweep {
            // The session's own post-process may be starting the module: give it the time to mark its entry pending.
            self.awaiting = Some(Instant::now() + PENDING_POLL);
            self.refresh_writing().await;
        }
    }

    async fn refresh_writing(&mut self) {
        let now: BTreeMap<String, (String, String)> =
            self.core.pending_journals().await.iter().map(|v| (text_of(v, "session"), (text_of(v, "game"), text_of(v, "title")))).collect();
        let mut moved = false;
        for (session, (id, title)) in &now {
            if self.announced.insert(session.clone()) {
                moved = true;
                self.send(Event::JournalWriting { id: id.clone(), session: session.clone(), title: title.clone() });
            }
        }
        let gone: Vec<(String, String)> = self.writing.iter().filter(|(s, _)| !now.contains_key(*s)).map(|(s, (id, _))| (s.clone(), id.clone())).collect();
        self.writing = now;
        for (session, id) in gone {
            moved = true;
            let Some(entry) = self.core.journal(&id).await.ok().and_then(|es| es.into_iter().find(|e| e.session == session)) else { continue };
            let text = if entry.state == "written" { entry.title } else { entry.paragraphs.into_iter().next().unwrap_or_default() };
            self.send(Event::JournalDone { id, session, state: entry.state, text });
        }
        if moved {
            self.awaiting = None;
        }
        if !self.writing.is_empty() {
            self.pending_at = Some(Instant::now() + PENDING_POLL);
        } else if let Some(until) = self.awaiting.filter(|t| *t > Instant::now()) {
            self.pending_at = Some(until);
        } else {
            self.awaiting = None;
            self.pending_at = None;
            self.sweep().await;
        }
    }

    async fn sweep(&mut self) {
        let report = self.core.sweep_journals().await;
        if report["started"].is_object() {
            self.sweep_at = None;
            self.awaiting = Some(Instant::now() + SWEEP_FLOOR);
            self.pending_at = self.awaiting;
            return;
        }
        self.sweep_at = sweep_wake(report["next"].as_str().unwrap_or_default(), chrono::Local::now()).map(|d| Instant::now() + d);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::session::tests::{open, sandbox};

    const QUICK: Options = Options { debounce: Duration::from_millis(20), poll: Duration::from_millis(50), journal_sweep: true };

    async fn until(rx: &mut mpsc::UnboundedReceiver<Event>, what: impl Fn(&Event) -> bool) -> Event {
        let wait = async {
            loop {
                let e = rx.recv().await.expect("the watch runs");
                if what(&e) {
                    return e;
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(5), wait).await.expect("no such event within 5 s")
    }

    fn names(e: &Event, id: &str) -> bool {
        matches!(e, Event::Library(ids) if ids.iter().any(|i| i == id))
    }

    #[tokio::test]
    async fn a_game_that_arrives_is_reloaded_and_named() {
        let _sb = sandbox();
        let core = Arc::new(open().await.0);
        let (_watch, mut rx) = watch(core.clone(), QUICK).await.unwrap();
        std::fs::create_dir_all(paths::game_dir("new")).unwrap();
        until(&mut rx, |e| names(e, "new")).await;
        assert!(core.get("new").await.is_err(), "a directory without its game.toml is no game yet");

        Game::new("New").save().unwrap();
        until(&mut rx, |e| names(e, "new")).await;
        assert_eq!(core.get("new").await.unwrap().game.title, "New", "the directory was watched as it appeared");

        std::fs::remove_dir_all(paths::game_dir("new")).unwrap();
        until(&mut rx, |e| names(e, "new")).await;
        assert!(core.get("new").await.is_err(), "and dropped as it went");
    }

    #[tokio::test]
    async fn a_journal_made_after_the_start_is_watched_and_its_entries_reported() {
        let _sb = sandbox();
        let core = Arc::new(open().await.0);
        let (_watch, mut rx) = watch(core.clone(), QUICK).await.unwrap();
        let journal = paths::game_dir("sample").join("journal");
        std::fs::create_dir_all(&journal).unwrap();
        until(&mut rx, |e| matches!(e, Event::Journal(id) if id == "sample")).await;

        let pending = serde_json::json!({"session": "s1", "game": "sample", "started_at": chrono::Local::now().to_rfc3339(), "provider": "test"});
        std::fs::write(journal.join("s1.pending.json"), pending.to_string()).unwrap();
        let e = until(&mut rx, |e| matches!(e, Event::JournalWriting { .. })).await;
        assert!(matches!(&e, Event::JournalWriting { id, session, title } if id == "sample" && session == "s1" && title == "Sample"), "{e:?}");

        let entry = serde_json::json!({"session": "s1", "game": "sample", "title": "A first run", "paragraphs": ["It went well."]});
        std::fs::write(journal.join("s1.json"), entry.to_string()).unwrap();
        let e = until(&mut rx, |e| matches!(e, Event::JournalDone { .. })).await;
        assert!(matches!(&e, Event::JournalDone { state, text, .. } if state == "written" && text == "A first run"), "{e:?}");
    }

    #[tokio::test]
    async fn a_pick_made_after_the_start_is_the_games_art() {
        let _sb = sandbox();
        let core = Arc::new(open().await.0);
        let picked = core.get("sample").await.unwrap().game.picked_dir();
        assert!(!picked.exists(), "the pick directory is watched once it appears");
        let (_watch, mut rx) = watch(core.clone(), QUICK).await.unwrap();
        std::fs::create_dir_all(&picked).unwrap();
        std::fs::write(picked.join("box_front.png"), b"").unwrap();
        until(&mut rx, |e| matches!(e, Event::Media(id) if id == "sample")).await;

        std::fs::remove_file(picked.join("box_front.png")).unwrap();
        until(&mut rx, |e| matches!(e, Event::Media(id) if id == "sample")).await;
    }

    #[tokio::test]
    async fn the_marker_starts_the_session_and_its_removal_ends_it_with_the_filed_line() {
        let _sb = sandbox();
        let (core, memory) = open().await;
        let core = Arc::new(core);
        let (_watch, mut rx) = watch(core.clone(), QUICK).await.unwrap();
        let sid = core.launch("sample", "", "").await.unwrap();
        let e = until(&mut rx, |e| matches!(e, Event::SessionStarted(_))).await;
        assert!(matches!(&e, Event::SessionStarted(c) if c.session_id == sid && c.id == "sample"), "{e:?}");

        let unit = format!("universe-game-sample-{sid}.service");
        memory.finish(&unit, 0);
        tokio::time::sleep(Duration::from_millis(200)).await;
        while let Ok(e) = rx.try_recv() {
            assert!(!matches!(e, Event::SessionEnded(_)), "a unit gone with its marker still there is a session still ending");
        }

        core.session_end("sample", &sid, None, None).await.unwrap();
        let e = until(&mut rx, |e| matches!(e, Event::SessionEnded(_))).await;
        let Event::SessionEnded(ended) = e else { unreachable!() };
        assert_eq!((ended.session_id.as_str(), ended.id.as_str(), ended.title.as_str(), ended.end.as_str()), (sid.as_str(), "sample", "Sample", "quit"));
        until(&mut rx, |e| names(e, "sample")).await;
    }

    #[tokio::test]
    async fn a_session_already_running_is_reported_first() {
        let _sb = sandbox();
        let core = Arc::new(open().await.0);
        let sid = core.launch("sample", "", "").await.unwrap();
        let (_watch, mut rx) = watch(core.clone(), QUICK).await.unwrap();
        let e = until(&mut rx, |_| true).await;
        assert!(matches!(&e, Event::SessionStarted(c) if c.session_id == sid), "{e:?}");
    }

    #[test]
    fn the_sweep_wakes_between_a_minute_and_half_an_hour_out() {
        let now = chrono::Local::now();
        let at = |secs: i64| (now + chrono::Duration::seconds(secs)).to_rfc3339();
        assert_eq!(sweep_wake(&at(-30), now), Some(SWEEP_FLOOR), "overdue");
        assert_eq!(sweep_wake(&at(300), now), Some(Duration::from_secs(300)));
        assert_eq!(sweep_wake(&at(86_400), now), Some(SWEEP_CEILING), "a quota lifting tomorrow");
        assert_eq!(sweep_wake("", now), None, "nothing deferred");
    }
}
