//! The player's own screenshots: `games/<id>/screenshots/YYYYMMDD-HHMMSS.<ext>`, one per shot, named by the moment.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use serde::Serialize;

use crate::game::Game;
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
}

/// `YYYYMMDD-HHMMSS` with an image extension: the shape both the hook and the journal key on.
pub fn is_shot_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    if !crate::library::IMAGE_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
        return false;
    }
    let b = stem.as_bytes();
    b.len() == 15 && b[8] == b'-' && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
}

pub fn taken_at(path: &Path) -> Option<DateTime<Local>> {
    crate::sessions::parse_session_id(path.file_stem()?.to_str()?)
}

/// Newest first.
pub fn list_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut shots: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| is_image(p) && p.file_name().and_then(|s| s.to_str()).is_some_and(is_shot_name)).collect();
    shots.sort_unstable_by(|a, b| b.cmp(a));
    shots
}

fn session_of(t: DateTime<Local>, sessions: &[Session]) -> String {
    let parse = |s: &str| DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Local));
    sessions
        .iter()
        .find(|s| {
            let Some(start) = parse(&s.started_at) else { return false };
            let end = parse(&s.ended_at).unwrap_or_else(|| start + chrono::Duration::seconds(s.duration_s as i64));
            start - chrono::Duration::seconds(HEAD_GRACE_S) <= t && t <= end + chrono::Duration::seconds(TAIL_GRACE_S)
        })
        .map(|s| s.session.clone())
        .unwrap_or_default()
}

pub fn list(r: &Resolved) -> Vec<Shot> {
    list_dir(&r.game.screenshots_dir())
        .into_iter()
        .map(|p| Shot {
            game: r.game.id.clone(),
            title: r.game.title.clone(),
            taken_at: taken_at(&p).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)).unwrap_or_default(),
            session: taken_at(&p).map(|t| session_of(t, &r.sessions)).unwrap_or_default(),
            path: p.to_string_lossy().into_owned(),
        })
        .collect()
}

/// Shots taken before they had a directory of their own sat in `journal/attachments/`; each moves once, a name
/// already taken stays where it is. Entries keep naming them by basename, which resolves to either place.
pub fn migrate(game: &Game) -> usize {
    migrate_dirs(&game.journal_dir().join("attachments"), &game.screenshots_dir())
}

pub fn migrate_dirs(old: &Path, dir: &Path) -> usize {
    let shots = list_dir(old);
    if shots.is_empty() {
        return 0;
    }
    if std::fs::create_dir_all(dir).is_err() {
        return 0;
    }
    let mut moved = 0;
    for src in shots {
        let Some(name) = src.file_name() else { continue };
        let dst = dir.join(name);
        if dst.exists() {
            continue;
        }
        match std::fs::rename(&src, &dst) {
            Ok(()) => moved += 1,
            Err(e) => tracing::warn!("{} → {}: {e}", src.display(), dst.display()),
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shot_names() {
        assert!(is_shot_name("20251219-215949.png") && is_shot_name("20251223-004111.JPG"));
        assert!(!is_shot_name("20251219-215949-1.png") && !is_shot_name("frame-20251219-215949.jpg") && !is_shot_name("20251219-215949.txt") && !is_shot_name("20251219-215949"));
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
            Session { session: "20260301-210000".into(), started_at: "2026-03-01T21:00:00+01:00".into(), ended_at: "2026-03-01T22:00:00+01:00".into(), duration_s: 3600, ..Default::default() },
            Session { session: "20260301-180000".into(), started_at: "2026-03-01T18:00:00+01:00".into(), ended_at: "".into(), duration_s: 600, ..Default::default() },
        ];
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Local);
        assert_eq!(session_of(at("2026-03-01T21:30:00+01:00"), &sessions), "20260301-210000");
        assert_eq!(session_of(at("2026-03-01T22:01:30+01:00"), &sessions), "20260301-210000");
        assert_eq!(session_of(at("2026-03-01T18:05:00+01:00"), &sessions), "20260301-180000");
        assert_eq!(session_of(at("2026-03-01T19:00:00+01:00"), &sessions), "");
    }

    #[test]
    fn migrate_moves_the_shots_and_leaves_the_frames() {
        let dir = tempfile::tempdir().unwrap();
        let (att, shots) = (dir.path().join("journal/attachments"), dir.path().join("screenshots"));
        std::fs::create_dir_all(&att).unwrap();
        std::fs::create_dir_all(&shots).unwrap();
        for f in ["20251219-215949.png", "20251223-004111.jpg", "20251219-215949-1.png"] {
            std::fs::write(att.join(f), b"x").unwrap();
        }
        std::fs::write(shots.join("20251223-004111.jpg"), b"kept").unwrap();
        assert_eq!(migrate_dirs(&att, &shots), 1);
        assert!(shots.join("20251219-215949.png").is_file());
        assert_eq!(std::fs::read(shots.join("20251223-004111.jpg")).unwrap(), b"kept");
        assert!(att.join("20251223-004111.jpg").is_file() && att.join("20251219-215949-1.png").is_file());
        assert_eq!(migrate_dirs(&att, &shots), 0);
    }
}
