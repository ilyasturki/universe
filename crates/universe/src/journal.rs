use crate::sessions::Session;
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A journal module that has not written `<sid>.json` this long after its `<sid>.pending.json` is taken for dead.
const PENDING_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Entry {
    pub session: String,
    pub game: String,
    pub written_at: String,
    pub started_at: String,
    pub ended_at: String,
    pub duration_s: i64,
    pub lang: String,
    pub title: String,
    pub provider: String,
    pub paragraphs: Vec<String>,
    pub next_up: String,
    pub images: Vec<String>,
    /// `written` | `pending` | `deferred` | `failed`: the file's kind, never read from its contents
    pub state: String,
    /// When a `deferred` entry is due for another run; empty on every other state
    #[serde(skip_serializing_if = "String::is_empty")]
    pub retry_at: String,
}

impl Default for Entry {
    fn default() -> Self {
        Entry {
            session: String::new(),
            game: String::new(),
            written_at: String::new(),
            started_at: String::new(),
            ended_at: String::new(),
            duration_s: 0,
            lang: String::new(),
            title: String::new(),
            provider: String::new(),
            paragraphs: vec![],
            next_up: String::new(),
            images: vec![],
            state: "written".into(),
            retry_at: String::new(),
        }
    }
}

impl Entry {
    pub fn validate(&self) -> crate::Result<()> {
        if self.session.is_empty() {
            return Err(crate::Error::Invalid("entry.session missing".into()));
        }
        if self.session.starts_with('.') || self.session.contains(['/', '\0']) {
            return Err(crate::Error::Invalid(format!("entry.session must be a plain file name: {}", self.session)));
        }
        if self.title.is_empty() && self.paragraphs.is_empty() {
            return Err(crate::Error::Invalid("entry has neither title nor paragraphs".into()));
        }
        for img in &self.images {
            if img.starts_with('/') || img.contains("..") {
                return Err(crate::Error::Invalid(format!("image path must be relative to the journal dir: {img}")));
            }
        }
        Ok(())
    }
}

fn read_json(p: &Path) -> crate::Result<serde_json::Value> {
    Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?)
}

fn field(v: &serde_json::Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").to_string()
}

fn pending_entry(p: &Path, sid: &str) -> crate::Result<Entry> {
    let v = read_json(p)?;
    let age = std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).unwrap_or_default();
    let timed_out = age > PENDING_TIMEOUT;
    Ok(Entry {
        session: sid.into(),
        game: field(&v, "game"),
        started_at: field(&v, "started_at"),
        provider: field(&v, "provider"),
        paragraphs: if timed_out { vec!["timed out".into()] } else { vec![] },
        state: if timed_out { "failed" } else { "pending" }.into(),
        ..Entry::default()
    })
}

/// `<sid>.failed.json` (`{session, game, written_at, reason}`): the reason is the entry's one paragraph.
fn failed_entry(p: &Path, sid: &str) -> crate::Result<Entry> {
    let v = read_json(p)?;
    let reason = field(&v, "reason");
    Ok(Entry {
        session: sid.into(),
        game: field(&v, "game"),
        written_at: field(&v, "written_at"),
        paragraphs: if reason.is_empty() { vec![] } else { vec![reason] },
        state: "failed".into(),
        ..Entry::default()
    })
}

/// `<sid>.deferred.json` (`{session, game, provider, written_at, until, reason, attempts}`): a failure worth another run.
fn deferred_entry(p: &Path, sid: &str) -> crate::Result<Entry> {
    let v = read_json(p)?;
    let reason = field(&v, "reason");
    Ok(Entry {
        session: sid.into(),
        game: field(&v, "game"),
        written_at: field(&v, "written_at"),
        provider: field(&v, "provider"),
        paragraphs: if reason.is_empty() { vec![] } else { vec![reason] },
        state: "deferred".into(),
        retry_at: field(&v, "until"),
        ..Entry::default()
    })
}

/// Every session holding a `<sid>.pending.json` with no entry behind it, however old the file is:
/// `pending()` hides the ones past the timeout, and those are exactly the runs a sweep has to pick up.
pub fn unfinished(journal_dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(journal_dir) else { return vec![] };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let sid = name.strip_suffix(".pending.json").filter(|s| !s.starts_with('.'))?.to_string();
            (!journal_dir.join(format!("{sid}.json")).exists()).then_some(sid)
        })
        .collect()
}

/// When the module means to try this session again, from its `<sid>.deferred.json`.
pub fn deferrals(journal_dir: &Path) -> Vec<(String, DateTime<Local>)> {
    let Ok(rd) = std::fs::read_dir(journal_dir) else { return vec![] };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let sid = name.strip_suffix(".deferred.json").filter(|s| !s.starts_with('.'))?.to_string();
            if journal_dir.join(format!("{sid}.json")).exists() {
                return None;
            }
            let until = parse_rfc3339(&field(&read_json(&e.path()).ok()?, "until"))?;
            Some((sid, until))
        })
        .collect()
}

/// A session with a written entry hides its failed one, a failed one its deferred one, and that one its pending one.
pub fn read_all(journal_dir: &Path) -> crate::Result<Vec<Entry>> {
    let (mut written, mut failed, mut pending) = (Vec::new(), Vec::new(), Vec::new());
    let mut deferred = Vec::new();
    let rd = match std::fs::read_dir(journal_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(written),
        Err(e) => return Err(e.into()),
    };
    for e in rd.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|s| s.to_str()) else { continue };
        if name.starts_with('.') || !name.ends_with(".json") {
            continue;
        }
        let parsed = if let Some(sid) = name.strip_suffix(".pending.json") {
            pending_entry(&p, sid).map(|en| pending.push(en))
        } else if let Some(sid) = name.strip_suffix(".failed.json") {
            failed_entry(&p, sid).map(|en| failed.push(en))
        } else if let Some(sid) = name.strip_suffix(".deferred.json") {
            deferred_entry(&p, sid).map(|en| deferred.push(en))
        } else {
            read_json(&p).and_then(|v| serde_json::from_value::<Entry>(v).map_err(Into::into)).map(|mut en| {
                en.state = "written".into();
                written.push(en)
            })
        };
        if let Err(err) = parsed {
            tracing::warn!("{}: {err}", p.display());
        }
    }
    let mut out = written;
    for en in failed.into_iter().chain(deferred).chain(pending) {
        if !out.iter().any(|w| w.session == en.session) {
            out.push(en);
        }
    }
    out.sort_by(|a, b| b.session.cmp(&a.session));
    Ok(out)
}

/// The pending entries alone, under `read_all`'s hiding rule, without parsing the written ones.
pub fn pending(journal_dir: &Path) -> Vec<Entry> {
    let Ok(rd) = std::fs::read_dir(journal_dir) else { return vec![] };
    rd.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let sid = name.strip_suffix(".pending.json").filter(|s| !s.starts_with('.'))?.to_string();
            if ["json", "failed.json", "deferred.json"].iter().any(|ext| journal_dir.join(format!("{sid}.{ext}")).exists()) {
                return None;
            }
            pending_entry(&e.path(), &sid).ok().filter(|en| en.state == "pending")
        })
        .collect()
}

pub fn count_written(journal_dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(journal_dir) else { return 0 };
    let state = |n: &str| ["pending", "deferred", "failed"].iter().any(|s| n.ends_with(&format!(".{s}.json")));
    rd.flatten().filter(|e| matches!(e.file_name().to_str(), Some(n) if !n.starts_with('.') && n.ends_with(".json") && !state(n))).count()
}

pub fn fill_timing(entries: &mut [Entry], sessions: &HashMap<String, Session>) {
    for en in entries {
        let Some(s) = sessions.get(&en.session) else { continue };
        if en.started_at.is_empty() {
            en.started_at = s.started_at.clone();
        }
        if en.ended_at.is_empty() {
            en.ended_at = s.ended_at.clone();
        }
        if en.duration_s == 0 {
            en.duration_s = s.duration_s as i64;
        }
    }
}

pub fn load(journal_dir: &Path) -> Vec<Entry> {
    read_all(journal_dir).unwrap_or_default()
}

pub fn write(journal_dir: &Path, entry: &Entry) -> crate::Result<std::path::PathBuf> {
    entry.validate()?;
    std::fs::create_dir_all(journal_dir)?;
    let p = journal_dir.join(format!("{}.json", entry.session));
    crate::game::atomic_write(&p, serde_json::to_string_pretty(entry)?.as_bytes())?;
    Ok(p)
}

pub fn sessions_by_id(sessions: &[Session]) -> HashMap<String, Session> {
    sessions.iter().map(|s| (s.session.clone(), s.clone())).collect()
}

pub struct Locale(libc::locale_t);

impl Locale {
    pub fn from_env() -> Self {
        Self::new(c"")
    }

    fn new(name: &std::ffi::CStr) -> Self {
        let loc = unsafe { libc::newlocale(libc::LC_TIME_MASK, name.as_ptr(), std::ptr::null_mut()) };
        let loc = if loc.is_null() { unsafe { libc::newlocale(libc::LC_TIME_MASK, c"C".as_ptr(), std::ptr::null_mut()) } } else { loc };
        Locale(loc)
    }

    /// `%x` of the locale, the same bytes glibc gives Python's strftime.
    pub fn date(&self, t: &DateTime<Local>) -> String {
        self.fmt(c"%x", "%Y-%m-%d", t)
    }

    pub fn datetime(&self, t: &DateTime<Local>) -> String {
        self.fmt(c"%c", "%Y-%m-%d %H:%M:%S", t)
    }

    fn fmt(&self, spec: &std::ffi::CStr, fallback: &str, t: &DateTime<Local>) -> String {
        if self.0.is_null() {
            return t.format(fallback).to_string();
        }
        let tm = libc::tm {
            tm_sec: t.second() as _,
            tm_min: t.minute() as _,
            tm_hour: t.hour() as _,
            tm_mday: t.day() as _,
            tm_mon: t.month0() as _,
            tm_year: (t.year() - 1900) as _,
            tm_wday: t.weekday().num_days_from_sunday() as _,
            tm_yday: t.ordinal0() as _,
            tm_isdst: -1,
            tm_gmtoff: t.offset().local_minus_utc() as _,
            tm_zone: std::ptr::null(),
        };
        let mut buf = [0u8; 128];
        let n = unsafe { libc::strftime_l(buf.as_mut_ptr().cast(), buf.len(), spec.as_ptr(), &tm, self.0) };
        if n == 0 {
            return t.format(fallback).to_string();
        }
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }
}

impl Drop for Locale {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { libc::freelocale(self.0) }
        }
    }
}

fn parse_rfc3339(s: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Local))
}

/// The player's own shots are named bare and live in `screenshots/`; the rest is relative to the journal.
pub fn image_path(journal_dir: &Path, screenshots_dir: &Path, rel: &str) -> PathBuf {
    if crate::screenshots::is_shot_name(rel) {
        screenshots_dir.join(rel)
    } else {
        journal_dir.join(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(session: &str, lang: &str, title: &str, provider: &str, paragraphs: &[&str], next_up: &str, images: &[&str]) -> Entry {
        Entry {
            session: session.into(),
            game: "sample".into(),
            lang: lang.into(),
            title: title.into(),
            provider: provider.into(),
            paragraphs: paragraphs.iter().map(|s| s.to_string()).collect(),
            next_up: next_up.into(),
            images: images.iter().map(|s| s.to_string()).collect(),
            ..Entry::default()
        }
    }

    fn session(sid: &str, started: &str, ended: &str, dur: u64, rec: Option<&str>) -> (String, Session) {
        (
            sid.into(),
            Session {
                session: sid.into(),
                game: "sample".into(),
                started_at: started.into(),
                ended_at: ended.into(),
                duration_s: dur,
                source: "import-journal".into(),
                recording: rec.map(Into::into),
                ..Default::default()
            },
        )
    }

    fn sample() -> (Vec<Entry>, HashMap<String, Session>) {
        let entries = vec![
            entry(
                "20260301-210000",
                "en",
                "Into the Dome",
                "import",
                &[
                    "Zachariah reached the Source after three failed runs.",
                    "- **Main quest:** Cleared the gate.",
                    "- **Side quest:** Talked to Amelia.",
                    "Then the patrol reset.",
                ],
                "Return to the Exchange and talk to Amelia.",
                &["20260301-211500.png", "attachments/20260301-210000-1.png", "attachments/frames/frame-20260301-210000-02.jpg"],
            ),
            entry(
                "20260215-183000",
                "fr",
                "Trois contrats et Port-péril",
                "import",
                &["Le duo a enchaîné les sauvetages.", "- **Boss :** Tu as vaincu Corbin Claquebec."],
                "Tu reprendras dans le Mausolée III.",
                &["attachments/frames/frame-20260215-183000-01.jpg"],
            ),
            entry("20260110-000500", "en", "", "import", &[], "", &[]),
            entry(
                "20251220-120000",
                "en",
                "",
                "none",
                &["This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize."],
                "",
                &[],
            ),
            entry("20251201-230000", "en", "First Glimpse", "import", &["You reached the title screen."], "Press any key.", &["20251201-230100.png"]),
        ];
        let sessions = HashMap::from([
            session(
                "20260301-210000",
                "2026-03-01T21:00:00+01:00",
                "2026-03-01T22:30:00+01:00",
                5400,
                Some("/mnt/recordings/games/sample/20260301-210000.mkv"),
            ),
            session(
                "20260215-183000",
                "2026-02-15T18:30:00+01:00",
                "2026-02-15T19:45:00+01:00",
                4500,
                Some("/mnt/recordings/games/sample/003-20260215-183000-1h15m.mkv"),
            ),
            session(
                "20260110-000500",
                "2026-01-10T00:05:00+01:00",
                "2026-01-10T00:07:00+01:00",
                120,
                Some("/mnt/recordings/games/sample/002-20260110-000500-2m.mkv"),
            ),
            session(
                "20251220-120000",
                "2025-12-20T12:00:00+01:00",
                "2025-12-20T12:01:00+01:00",
                60,
                Some("/mnt/recordings/games/sample/001-20251220-120000-1m.mkv"),
            ),
            session("20251201-230000", "2025-12-01T23:00:00+01:00", "2025-12-02T00:10:00+01:00", 4200, None),
        ]);
        (entries, sessions)
    }

    #[test]
    fn entry_timing_from_the_session_then_its_own_stamps() {
        let (mut entries, sessions) = sample();
        entries.push(Entry {
            session: "20260501-200000".into(),
            title: "Stamped".into(),
            started_at: "2026-05-01T20:00:00+02:00".into(),
            ended_at: "2026-05-01T20:45:00+02:00".into(),
            duration_s: 2700,
            ..Entry::default()
        });
        fill_timing(&mut entries, &sessions);
        assert_eq!(entries[0].started_at, "2026-03-01T21:00:00+01:00");
        assert_eq!(entries[0].ended_at, "2026-03-01T22:30:00+01:00");
        assert_eq!(entries[0].duration_s, 5400);
        assert_eq!(entries[5].duration_s, 2700, "an entry's own stamps stay");
    }

    #[test]
    fn state_files_list_as_entries() {
        let dir = tempfile::tempdir().unwrap();
        let journal_dir = dir.path().join("journal");
        std::fs::create_dir_all(&journal_dir).unwrap();
        write(&journal_dir, &Entry { session: "20260910-100000".into(), title: "Written".into(), ..Entry::default() }).unwrap();
        std::fs::write(
            journal_dir.join("20260910-110000.pending.json"),
            r#"{"session":"20260910-110000","game":"x","started_at":"2026-09-10T11:00:00+02:00","provider":"codex"}"#,
        )
        .unwrap();
        std::fs::write(
            journal_dir.join("20260910-120000.failed.json"),
            r#"{"session":"20260910-120000","game":"x","written_at":"2026-09-10T12:30:00+02:00","reason":"codex: rate limited"}"#,
        )
        .unwrap();
        std::fs::write(journal_dir.join("20260910-120000.pending.json"), r#"{"session":"20260910-120000","game":"x"}"#).unwrap();
        std::fs::write(
            journal_dir.join("20260910-130000.pending.json"),
            r#"{"session":"20260910-130000","game":"x","started_at":"2026-09-10T13:00:00+02:00"}"#,
        )
        .unwrap();
        let stale = std::fs::OpenOptions::new().write(true).open(journal_dir.join("20260910-130000.pending.json")).unwrap();
        stale.set_modified(std::time::SystemTime::now() - Duration::from_secs(31 * 60)).unwrap();
        let due = Local::now() - chrono::Duration::minutes(5);
        std::fs::write(
            journal_dir.join("20260910-140000.deferred.json"),
            format!(
                r#"{{"session":"20260910-140000","game":"x","provider":"codex","written_at":"2026-09-10T14:20:00+02:00","until":"{}","reason":"codex quota reached","attempts":0}}"#,
                due.to_rfc3339()
            ),
        )
        .unwrap();
        // The session that is being written again keeps its deferred file until the run ends: the pending one does not show through it.
        std::fs::write(journal_dir.join("20260910-140000.pending.json"), r#"{"session":"20260910-140000","game":"x","provider":"codex"}"#).unwrap();
        std::fs::write(journal_dir.join(".game-memory.json"), "{}").unwrap();
        std::fs::write(journal_dir.join("notes.txt"), "x").unwrap();
        let all = read_all(&journal_dir).unwrap();
        let states: Vec<(&str, &str)> = all.iter().map(|e| (e.session.as_str(), e.state.as_str())).collect();
        assert_eq!(
            states,
            vec![
                ("20260910-140000", "deferred"),
                ("20260910-130000", "failed"),
                ("20260910-120000", "failed"),
                ("20260910-110000", "pending"),
                ("20260910-100000", "written")
            ]
        );
        assert_eq!(all[0].paragraphs, vec!["codex quota reached"]);
        assert_eq!((all[0].retry_at.as_str(), all[0].provider.as_str()), (due.to_rfc3339().as_str(), "codex"));
        assert_eq!(all[1].paragraphs, vec!["timed out"]);
        assert_eq!(all[2].paragraphs, vec!["codex: rate limited"]);
        assert_eq!(all[2].written_at, "2026-09-10T12:30:00+02:00");
        assert_eq!((all[3].started_at.as_str(), all[3].provider.as_str(), all[3].title.as_str()), ("2026-09-10T11:00:00+02:00", "codex", ""));
        assert!(all[3].paragraphs.is_empty());
        assert_eq!(pending(&journal_dir).len(), 1, "only the session with nothing else to its name is pending");
        assert_eq!(deferrals(&journal_dir), vec![("20260910-140000".to_string(), due)]);
        assert_eq!(count_written(&journal_dir), 1);
    }

    #[test]
    fn write_read_entry() {
        let dir = tempfile::tempdir().unwrap();
        let journal_dir = dir.path().join("journal");
        let shots_dir = dir.path().join("screenshots");
        let e = Entry {
            session: "20260910-213045".into(),
            game: "x".into(),
            title: "Into the Dome".into(),
            paragraphs: vec!["A.".into(), "B.".into()],
            next_up: "Go.".into(),
            images: vec!["20260910-214000.png".into(), "attachments/a.png".into()],
            ..Default::default()
        };
        write(&journal_dir, &e).unwrap();
        assert_eq!(read_all(&journal_dir).unwrap(), vec![e.clone()]);
        assert_eq!(image_path(&journal_dir, &shots_dir, "20260910-214000.png"), shots_dir.join("20260910-214000.png"));
        assert_eq!(image_path(&journal_dir, &shots_dir, "attachments/a.png"), journal_dir.join("attachments/a.png"));
        assert!(!crate::screenshots::is_shot_name("attachments/20260301-210000-1.png"));
        let bad = Entry { images: vec!["/etc/passwd".into()], ..e.clone() };
        assert!(write(&journal_dir, &bad).is_err());
        for session in ["../x", "a/b", "..", ".hidden"] {
            let bad = Entry { session: session.into(), ..e.clone() };
            assert!(matches!(write(&journal_dir, &bad), Err(crate::Error::Invalid(_))), "{session}");
        }
    }
}
