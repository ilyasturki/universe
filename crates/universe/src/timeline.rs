//! What a player kept from their sessions as one list: shots, recordings and written journal entries, each game's journal read once.

use std::collections::HashSet;

use serde::Serialize;

use crate::journal::Entry;
use crate::library::Resolved;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MediaRow {
    /// `shot` | `recording` | `journal`
    pub kind: String,
    pub game: String,
    pub title: String,
    /// The session the row belongs to; a shot taken outside one has none
    pub session: String,
    /// RFC3339, what the list is sorted by: the shot's time, the recording's end, the entry's writing
    pub when: String,
    /// RFC3339, what a row shows: the same as `when` but for an entry, which shows when it was played
    pub date: String,
    /// The shot, the recording, or the entry's first picture; empty for an entry without one
    pub path: String,
    /// The picture's thumbnail, made or not (`thumb_ready`), filled by the core; empty for a recording, whose frames are the frontend's
    pub thumb: String,
    pub thumb_ready: bool,
    /// A journal entry covers the session, written or on its way
    pub has_journal: bool,
    /// The entry's title
    pub heading: String,
    /// The recording's or the entry's session length
    pub duration_s: u64,
}

pub fn rows(r: &Resolved, entries: &[Entry]) -> Vec<MediaRow> {
    let journaled: HashSet<&str> = entries.iter().map(|e| e.session.as_str()).collect();
    let game = &r.game;
    let mut out = Vec::new();
    for shot in crate::screenshots::list(r) {
        out.push(MediaRow {
            kind: "shot".into(),
            game: game.id.clone(),
            title: game.title.clone(),
            has_journal: !shot.session.is_empty() && journaled.contains(shot.session.as_str()),
            session: shot.session,
            when: shot.taken_at.clone(),
            date: shot.taken_at,
            path: shot.path,
            thumb: String::new(),
            thumb_ready: false,
            heading: String::new(),
            duration_s: 0,
        });
    }
    for s in &r.sessions {
        let Some(path) = s.recording.as_deref().filter(|p| !p.is_empty()) else { continue };
        out.push(MediaRow {
            kind: "recording".into(),
            game: game.id.clone(),
            title: game.title.clone(),
            session: s.session.clone(),
            when: s.ended_at.clone(),
            date: s.ended_at.clone(),
            path: path.into(),
            thumb: String::new(),
            thumb_ready: false,
            has_journal: journaled.contains(s.session.as_str()),
            heading: String::new(),
            duration_s: s.duration_s,
        });
    }
    let journal_dir = game.journal_dir();
    let shots_dir = game.screenshots_dir();
    for e in entries.iter().filter(|e| e.state == "written") {
        let first = e.images.first().map(|rel| crate::journal::image_path(&journal_dir, &shots_dir, rel));
        let when = if e.written_at.is_empty() { e.started_at.clone() } else { e.written_at.clone() };
        let date = if e.started_at.is_empty() { e.written_at.clone() } else { e.started_at.clone() };
        out.push(MediaRow {
            kind: "journal".into(),
            game: game.id.clone(),
            title: game.title.clone(),
            session: e.session.clone(),
            when,
            date,
            path: first.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            thumb: String::new(),
            thumb_ready: false,
            has_journal: true,
            heading: if e.title.is_empty() { "Untitled".into() } else { e.title.clone() },
            duration_s: e.duration_s.max(0) as u64,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::sessions::Session;

    fn resolved() -> Resolved {
        let game = Game { id: "zelda".into(), title: "Zelda".into(), ..Default::default() };
        Resolved {
            sessions: vec![
                Session {
                    session: "20250101-100000".into(),
                    game: "zelda".into(),
                    ended_at: "2025-01-01T11:00:00+00:00".into(),
                    duration_s: 3600,
                    recording: Some("/r/1.mkv".into()),
                    ..Default::default()
                },
                Session {
                    session: "20250102-100000".into(),
                    game: "zelda".into(),
                    ended_at: "2025-01-02T11:00:00+00:00".into(),
                    duration_s: 60,
                    ..Default::default()
                },
            ],
            game,
            ..Default::default()
        }
    }

    #[test]
    fn one_row_per_kept_thing() {
        let r = resolved();
        let entries = vec![
            Entry {
                session: "20250101-100000".into(),
                title: "A night out".into(),
                state: "written".into(),
                started_at: "2025-01-01T10:00:00+00:00".into(),
                written_at: "2025-01-01T11:30:00+00:00".into(),
                duration_s: 3600,
                ..Default::default()
            },
            Entry { session: "20250102-100000".into(), state: "pending".into(), ..Default::default() },
        ];
        let rows = rows(&r, &entries);
        let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, ["recording", "journal"], "a pending entry is not on the list");
        assert!(rows[0].has_journal && rows[0].thumb.is_empty() && rows[0].duration_s == 3600);
        assert_eq!(
            (rows[1].heading.as_str(), rows[1].when.as_str(), rows[1].date.as_str()),
            ("A night out", "2025-01-01T11:30:00+00:00", "2025-01-01T10:00:00+00:00")
        );
    }
}
