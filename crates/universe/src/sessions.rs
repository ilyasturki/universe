use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub recording: Option<String>,
}

impl Default for Session {
    fn default() -> Self {
        Session {
            session: String::new(),
            game: String::new(),
            started_at: String::new(),
            ended_at: String::new(),
            duration_s: 0,
            source: "daemon".into(),
            unit: String::new(),
            screen: String::new(),
            exit: 0,
            recording: None,
        }
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

/// Rewrites one session line (used to attach a recording); other lines are untouched.
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
    fn roundtrip_and_stats() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sessions.jsonl");
        let a = Session { session: "20260910-213045".into(), game: "x".into(), started_at: "2026-09-10T21:30:45+02:00".into(), ended_at: "2026-09-10T22:00:45+02:00".into(), duration_s: 1800, ..Default::default() };
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
}
