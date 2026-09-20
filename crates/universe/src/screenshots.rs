use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::library::{is_image, Resolved};
use crate::sessions::Session;

/// The journal module's own window around a session, so a shot and the entry agree on which session it belongs to.
const HEAD_GRACE_S: i64 = 90;
const TAIL_GRACE_S: i64 = 120;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Shot {
    pub game: String,
    pub title: String,
    pub path: String,
    pub taken_at: String,
    /// The session played at that moment, empty when none
    pub session: String,
    /// The shot's thumbnail (`thumbs`), made or not; `list` leaves it empty, the core fills it
    pub thumb: String,
    pub thumb_ready: bool,
}

/// `YYYYMMDD-HHMMSS` with an image extension: the shape both the hook and the journal key on.
pub fn is_shot_name(name: &str) -> bool {
    let Some((stem, _)) = name.rsplit_once('.') else { return false };
    let b = stem.as_bytes();
    is_image(Path::new(name)) && b.len() == 15 && b[8] == b'-' && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
}

pub fn taken_at(path: &Path) -> Option<DateTime<Local>> {
    crate::sessions::parse_session_id(path.file_stem()?.to_str()?)
}

pub fn list_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut shots: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.file_name().and_then(|s| s.to_str()).is_some_and(is_shot_name)).collect();
    shots.sort_unstable_by(|a, b| b.cmp(a));
    shots
}

fn spans(sessions: &[Session]) -> Vec<(DateTime<Local>, DateTime<Local>, &str)> {
    let parse = |s: &str| DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Local));
    sessions
        .iter()
        .filter_map(|s| {
            let start = parse(&s.started_at)?;
            let end = parse(&s.ended_at).unwrap_or_else(|| start + chrono::Duration::seconds(s.duration_s as i64));
            Some((start - chrono::Duration::seconds(HEAD_GRACE_S), end + chrono::Duration::seconds(TAIL_GRACE_S), s.session.as_str()))
        })
        .collect()
}

fn session_of(t: DateTime<Local>, spans: &[(DateTime<Local>, DateTime<Local>, &str)]) -> String {
    spans.iter().find(|(start, end, _)| *start <= t && t <= *end).map(|(_, _, id)| id.to_string()).unwrap_or_default()
}

pub fn list(r: &Resolved) -> Vec<Shot> {
    let spans = spans(&r.sessions);
    list_dir(&r.game.screenshots_dir())
        .into_iter()
        .map(|p| {
            let t = taken_at(&p);
            Shot {
                game: r.game.id.clone(),
                title: r.game.title.clone(),
                taken_at: t.map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default(),
                session: t.map(|t| session_of(t, &spans)).unwrap_or_default(),
                path: p.to_string_lossy().into_owned(),
                thumb: String::new(),
                thumb_ready: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shot_names() {
        assert!(is_shot_name("20251219-215949.png") && is_shot_name("20251223-004111.JPG"));
        assert!(
            !is_shot_name("20251219-215949-1.png")
                && !is_shot_name("frame-20251219-215949.jpg")
                && !is_shot_name("20251219-215949.txt")
                && !is_shot_name("20251219-215949")
        );
    }

    #[test]
    fn list_is_the_players_only_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for f in ["20251219-215949.png", "20251223-004111.jpg", "20251219-215949-1.png", "frame.png", "notes.txt"] {
            std::fs::write(dir.path().join(f), b"x").unwrap();
        }
        assert_eq!(list_dir(dir.path()), [dir.path().join("20251223-004111.jpg"), dir.path().join("20251219-215949.png")]);
        assert!(list_dir(&dir.path().join("none")).is_empty());
    }

    #[test]
    fn a_shot_finds_its_session_within_the_journals_grace() {
        let sessions = vec![
            Session {
                session: "20260301-210000".into(),
                started_at: "2026-03-01T21:00:00+01:00".into(),
                ended_at: "2026-03-01T22:00:00+01:00".into(),
                duration_s: 3600,
                ..Default::default()
            },
            Session {
                session: "20260301-180000".into(),
                started_at: "2026-03-01T18:00:00+01:00".into(),
                ended_at: "".into(),
                duration_s: 600,
                ..Default::default()
            },
        ];
        let spans = spans(&sessions);
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Local);
        assert_eq!(session_of(at("2026-03-01T21:30:00+01:00"), &spans), "20260301-210000");
        assert_eq!(session_of(at("2026-03-01T22:01:30+01:00"), &spans), "20260301-210000");
        assert_eq!(session_of(at("2026-03-01T18:05:00+01:00"), &spans), "20260301-180000");
        assert_eq!(session_of(at("2026-03-01T19:00:00+01:00"), &spans), "");
    }
}
