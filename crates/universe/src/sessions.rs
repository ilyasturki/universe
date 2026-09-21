use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Session {
    pub session: String,
    pub game: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_s: u64,
    pub source: String,
    pub unit: String,
    pub screen: String,
    pub exit: i32,
    /// `stop` asked for the end: a signal exit is a quit, not a crash. Absent on a line written before the key existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped: Option<bool>,
    /// The launched command line, for the log's first line.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub command: String,
    pub recording: Option<String>,
    /// The recording's media length; 0 when never probed
    pub recording_duration_s: u64,
    /// When the recorder began, RFC3339; empty when unknown
    pub recording_started_at: String,
    /// Stretches the recorder skipped while the game was frozen, RFC3339 pairs
    pub recording_pauses: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecordingInfo {
    pub path: String,
    pub size: u64,
    pub exists: bool,
    pub duration_s: u64,
    pub started_at: String,
    pub pauses: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct JournalState {
    pub state: String,
    pub title: String,
    pub written_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRow {
    pub session: Session,
    pub title: String,
    pub recording: Option<RecordingInfo>,
    pub journal: Option<JournalState>,
    /// Proton's and DXVK's own files, when the launch turned them on.
    pub debug_log: Option<String>,
}

/// How a session ended, from the line's `exit` and `stopped`: `quit` (0), `stopped` (asked), `crashed` (a code), `killed` (a signal nobody asked
/// for), `ended` (a signal on a line too old to say).
pub fn end_of(session: &Session) -> &'static str {
    match (session.exit, session.stopped) {
        (0, _) => "quit",
        (_, Some(true)) => "stopped",
        (-1, Some(false)) => "killed",
        (-1, None) => "ended",
        _ => "crashed",
    }
}

impl SessionRow {
    pub fn new(session: &Session, title: &str, entry: Option<&crate::journal::Entry>) -> SessionRow {
        let recording = session.recording.as_deref().filter(|p| !p.is_empty()).map(|p| {
            let md = std::fs::metadata(p).ok();
            RecordingInfo {
                path: p.into(),
                size: md.as_ref().map(|m| m.len()).unwrap_or(0),
                exists: md.is_some(),
                duration_s: session.recording_duration_s,
                started_at: session.recording_started_at.clone(),
                pauses: session.recording_pauses.clone(),
            }
        });
        let journal = entry.map(|e| JournalState { state: e.state.clone(), title: e.title.clone(), written_at: e.written_at.clone() });
        let debug_log = Some(crate::paths::session_log_dir(&session.game, &session.session)).filter(|d| d.is_dir()).map(|d| d.to_string_lossy().into_owned());
        SessionRow { session: session.clone(), title: title.into(), recording, journal, debug_log }
    }
}

// Not a flatten: the row's `recording` (the file) takes the line's key (the path).
impl Serialize for SessionRow {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::Error;
        let mut v = serde_json::to_value(&self.session).map_err(S::Error::custom)?;
        let m = v.as_object_mut().ok_or_else(|| S::Error::custom("session is not an object"))?;
        for k in ["recording_duration_s", "recording_started_at", "recording_pauses", "command"] {
            m.remove(k);
        }
        m.insert("title".into(), self.title.clone().into());
        m.insert("end".into(), end_of(&self.session).into());
        m.insert("debug_log".into(), serde_json::to_value(&self.debug_log).map_err(S::Error::custom)?);
        m.insert("recording".into(), serde_json::to_value(&self.recording).map_err(S::Error::custom)?);
        m.insert("journal".into(), serde_json::to_value(&self.journal).map_err(S::Error::custom)?);
        v.serialize(s)
    }
}

pub fn session_id(t: chrono::DateTime<chrono::Local>) -> String {
    t.format("%Y%m%d-%H%M%S").to_string()
}

pub fn parse_session_id(id: &str) -> Option<chrono::DateTime<chrono::Local>> {
    use chrono::TimeZone;
    let naive = chrono::NaiveDateTime::parse_from_str(id, "%Y%m%d-%H%M%S").ok()?;
    chrono::Local.from_local_datetime(&naive).single()
}

pub fn read(path: &Path) -> crate::Result<Vec<Session>> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for line in std::io::BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Session>(&line) {
            Ok(s) => out.push(s),
            Err(e) => tracing::warn!("{}: bad session line: {e}", path.display()),
        }
    }
    Ok(out)
}

pub fn append(path: &Path, s: &Session) -> crate::Result<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    let mut line = serde_json::to_string(s)?;
    line.push('\n');
    f.write_all(line.as_bytes())?;
    Ok(())
}

pub fn update<F: FnMut(&mut Session)>(path: &Path, session_id: &str, mut f: F) -> crate::Result<bool> {
    let mut all = read(path)?;
    let mut found = false;
    for s in all.iter_mut() {
        if s.session == session_id {
            f(s);
            found = true;
        }
    }
    if found {
        let text: String = all.iter().map(|s| serde_json::to_string(s).unwrap() + "\n").collect();
        crate::game::atomic_write(path, text.as_bytes())?;
    }
    Ok(found)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Stats {
    pub hours: f64,
    pub play_count: u64,
    pub last_played: Option<String>,
}

pub fn stats(sessions: &[Session]) -> Stats {
    let secs: u64 = sessions.iter().map(|s| s.duration_s).sum();
    let mut last: Option<&Session> = None;
    for s in sessions {
        if s.source == "import-lutris" && s.duration_s == 0 {
            continue;
        }
        if last.map(|l| s.ended_at > l.ended_at).unwrap_or(true) {
            last = Some(s);
        }
    }
    Stats {
        hours: (secs as f64 / 3600.0 * 100.0).round() / 100.0,
        play_count: sessions.iter().filter(|s| s.source != "import-lutris").count() as u64,
        last_played: last.map(|s| s.ended_at.clone()).filter(|s| !s.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_end_reads_exit_and_stopped_and_the_line_keeps_its_shape() {
        let s = Session { exit: 0, ..Default::default() };
        assert_eq!(end_of(&s), "quit");
        assert_eq!(end_of(&Session { exit: 0, stopped: Some(true), ..Default::default() }), "quit");
        assert_eq!(end_of(&Session { exit: -1, stopped: Some(true), ..Default::default() }), "stopped");
        assert_eq!(end_of(&Session { exit: 6, stopped: Some(true), ..Default::default() }), "stopped");
        assert_eq!(end_of(&Session { exit: -1, stopped: Some(false), ..Default::default() }), "killed");
        assert_eq!(end_of(&Session { exit: -1, ..Default::default() }), "ended");
        assert_eq!(end_of(&Session { exit: 6, ..Default::default() }), "crashed");
        let line = serde_json::to_value(&s).unwrap();
        assert!(line.get("stopped").is_none() && line.get("command").is_none(), "{line}");
        let old: Session = serde_json::from_str(r#"{"session":"20260910-213045","game":"g","exit":-1}"#).unwrap();
        assert!(old.stopped.is_none() && old.command.is_empty());
    }

    #[test]
    fn roundtrip_and_stats() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sessions.jsonl");
        let a = Session {
            session: "20260910-213045".into(),
            game: "x".into(),
            started_at: "2026-09-10T21:30:45+02:00".into(),
            ended_at: "2026-09-10T22:00:45+02:00".into(),
            duration_s: 1800,
            ..Default::default()
        };
        let b = Session { session: "import".into(), game: "x".into(), duration_s: 5400, source: "import-lutris".into(), ..Default::default() };
        append(&p, &a).unwrap();
        append(&p, &b).unwrap();
        assert!(update(&p, "20260910-213045", |s| s.recording = Some("/r.mkv".into())).unwrap());
        let all = read(&p).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].recording.as_deref(), Some("/r.mkv"));
        let st = stats(&all);
        assert_eq!(st.hours, 2.0);
        assert_eq!(st.play_count, 1);
        assert_eq!(st.last_played.as_deref(), Some("2026-09-10T22:00:45+02:00"));
    }

    #[test]
    fn row_joins_the_file_and_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let rec = dir.path().join("r.mkv");
        std::fs::write(&rec, b"abc").unwrap();
        let s = Session {
            session: "20260910-213045".into(),
            duration_s: 1800,
            recording: Some(rec.to_string_lossy().into()),
            recording_duration_s: 1790,
            ..Default::default()
        };
        let entry = crate::journal::Entry { session: s.session.clone(), title: "Into the Dome".into(), state: "written".into(), ..Default::default() };
        let v = serde_json::to_value(SessionRow::new(&s, "X", Some(&entry))).unwrap();
        assert_eq!(v["title"], "X");
        assert_eq!(
            v["recording"],
            serde_json::json!({"path": rec.to_string_lossy(), "size": 3, "exists": true, "duration_s": 1790, "started_at": "", "pauses": []})
        );
        assert_eq!(v["journal"], serde_json::json!({"state": "written", "title": "Into the Dome", "written_at": ""}));
        assert!(v.get("recording_duration_s").is_none());
        let bare = serde_json::to_value(SessionRow::new(&Session::default(), "", None)).unwrap();
        assert!(bare["recording"].is_null() && bare["journal"].is_null());
    }
}
