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
    /// `written` | `pending` | `failed`: the file's kind, never read from its contents
    pub state: String,
}

impl Default for Entry {
    fn default() -> Self {
        Entry { session: String::new(), game: String::new(), written_at: String::new(), started_at: String::new(), ended_at: String::new(), duration_s: 0, lang: String::new(), title: String::new(), provider: String::new(), paragraphs: vec![], next_up: String::new(), images: vec![], state: "written".into() }
    }
}

impl Entry {
    pub fn validate(&self) -> crate::Result<()> {
        if self.session.is_empty() {
            return Err(crate::Error::Invalid("entry.session missing".into()));
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
    Ok(Entry { session: sid.into(), game: field(&v, "game"), written_at: field(&v, "written_at"), paragraphs: if reason.is_empty() { vec![] } else { vec![reason] }, state: "failed".into(), ..Entry::default() })
}

/// A session with a written entry hides its failed one, a failed one its pending one.
pub fn read_all(journal_dir: &Path) -> crate::Result<Vec<Entry>> {
    let (mut written, mut failed, mut pending) = (Vec::new(), Vec::new(), Vec::new());
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
    for en in failed.into_iter().chain(pending) {
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
            if journal_dir.join(format!("{sid}.json")).exists() || journal_dir.join(format!("{sid}.failed.json")).exists() {
                return None;
            }
            pending_entry(&e.path(), &sid).ok().filter(|en| en.state == "pending")
        })
        .collect()
}

pub fn count_written(journal_dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(journal_dir) else { return 0 };
    rd.flatten().filter(|e| matches!(e.file_name().to_str(), Some(n) if !n.starts_with('.') && n.ends_with(".json") && !n.ends_with(".pending.json") && !n.ends_with(".failed.json"))).count()
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

struct Labels {
    journal: &'static str,
    recording: &'static str,
    next: &'static str,
    frames: &'static str,
    colon: &'static str,
}

// Mirror of LABELS in modules/journal/bin/_common.py; keep the two in step.
const LABELS: &[(&str, Labels)] = &[
    ("fr", Labels { journal: "Journal", recording: "Enregistrement", next: "Reprise", frames: "Images extraites de l'enregistrement", colon: " :" }),
    ("en", Labels { journal: "Journal", recording: "Recording", next: "Next up", frames: "Frames from the recording", colon: ":" }),
    ("es", Labels { journal: "Diario", recording: "Grabación", next: "Retomar", frames: "Imágenes extraídas de la grabación", colon: ":" }),
    ("de", Labels { journal: "Journal", recording: "Aufnahme", next: "Weiter", frames: "Bilder aus der Aufnahme", colon: ":" }),
    ("it", Labels { journal: "Diario", recording: "Registrazione", next: "Ripresa", frames: "Immagini estratte dalla registrazione", colon: ":" }),
    ("pt", Labels { journal: "Diário", recording: "Gravação", next: "Retomar", frames: "Imagens extraídas da gravação", colon: ":" }),
    ("ja", Labels { journal: "日誌", recording: "録画", next: "次回", frames: "録画から抽出した画像", colon: "：" }),
];

fn labels(lang: &str) -> &'static Labels {
    let code = lang.trim().to_lowercase();
    LABELS.iter().find(|(c, _)| *c == code).or_else(|| LABELS.iter().find(|(c, _)| *c == "en")).map(|(_, l)| l).unwrap()
}

pub struct Locale(libc::locale_t);

impl Locale {
    pub fn from_env() -> Self {
        Self::new(c"")
    }

    #[cfg(test)]
    pub fn posix() -> Self {
        Self::new(c"C")
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

fn session_span(session: Option<&Session>, entry: &Entry) -> (DateTime<Local>, DateTime<Local>, u64) {
    let start = session.and_then(|s| parse_rfc3339(&s.started_at)).or_else(|| parse_rfc3339(&entry.started_at)).or_else(|| crate::sessions::parse_session_id(&entry.session)).unwrap_or_else(Local::now);
    let duration = session.map(|s| s.duration_s).filter(|d| *d > 0).unwrap_or(entry.duration_s.max(0) as u64);
    let end = session.and_then(|s| parse_rfc3339(&s.ended_at)).or_else(|| parse_rfc3339(&entry.ended_at)).unwrap_or_else(|| start + chrono::Duration::seconds(duration as i64));
    (start, end, duration)
}

fn fmt_time(t: &DateTime<Local>) -> String {
    t.format("%H:%M").to_string()
}

fn fmt_duration(total_sec: u64) -> String {
    // Python's round() is half-to-even; the module rendered these first.
    let total_min = ((total_sec as f64) / 60.0).round_ties_even().max(1.0) as u64;
    let (h, m) = (total_min / 60, total_min % 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// The player's own shots are named bare and live in `screenshots/`; the rest is relative to the journal.
pub fn image_path(journal_dir: &Path, screenshots_dir: &Path, rel: &str) -> PathBuf {
    if crate::screenshots::is_shot_name(rel) {
        screenshots_dir.join(rel)
    } else {
        journal_dir.join(rel)
    }
}

/// Like Python's urllib quote with the note's safe set; parentheses are encoded for Markdown, `?` and `#` would start a query or a fragment.
fn file_uri(path: &str) -> String {
    const SAFE: &[u8] = b"/;,:@&=+$!~*'-_.";
    let mut out = String::from("file://");
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || SAFE.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A plain scalar YAML 1.1 (PyYAML) or 1.2 (js-yaml, Obsidian) would type as something other than a string.
fn yaml_typed(s: &str) -> bool {
    if matches!(s.to_lowercase().as_str(), "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "y" | "n" | "~" | ".inf" | "-.inf" | "+.inf" | ".nan") {
        return true;
    }
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    let b: Vec<u8> = t.bytes().collect();
    let radix = |digits: &[u8], radix: u32| digits.len() > 0 && digits.iter().all(|&c| c == b'_' || (c as char).is_digit(radix));
    if b.len() > 2 && b[0] == b'0' && matches!(b[1], b'x' | b'o' | b'b') && radix(&b[2..], match b[1] { b'x' => 16, b'o' => 8, _ => 2 }) {
        return true;
    }
    let digit_first = b.first().is_some_and(|c| c.is_ascii_digit() || *c == b'.');
    if digit_first && b.contains(&b':') && b.iter().all(|c| c.is_ascii_digit() || matches!(c, b':' | b'_' | b'.')) {
        return true;
    }
    if digit_first && s.replace('_', "").parse::<f64>().is_ok() {
        return true;
    }
    yaml_date(s.as_bytes())
}

/// `YYYY-M-D`, alone or followed by a time.
fn yaml_date(b: &[u8]) -> bool {
    if b.len() < 8 || !b[..4].iter().all(u8::is_ascii_digit) || b[4] != b'-' {
        return false;
    }
    let rest = &b[5..];
    let m = rest.iter().take_while(|c| c.is_ascii_digit()).count();
    if m == 0 || rest.get(m) != Some(&b'-') {
        return false;
    }
    let rest = &rest[m + 1..];
    let d = rest.iter().take_while(|c| c.is_ascii_digit()).count();
    (1..=2).contains(&d) && rest.get(d).is_none_or(|c| matches!(c, b'T' | b't' | b' '))
}

fn yaml_str(s: &str) -> String {
    let unsafe_start = |c: char| c.is_whitespace() || "\"'-*&![]{}|>%@`?#".contains(c);
    let chars: Vec<char> = s.chars().collect();
    let pair = |a: fn(char) -> bool, b: fn(char) -> bool| chars.windows(2).any(|w| a(w[0]) && b(w[1]));
    let needs_quotes = s.is_empty()
        || pair(|c| c == ':', char::is_whitespace)
        || pair(char::is_whitespace, |c| c == '#')
        || s.chars().next().is_some_and(unsafe_start)
        || s.chars().last().is_some_and(char::is_whitespace)
        || yaml_typed(s);
    if needs_quotes {
        serde_json::to_string(s).unwrap_or_default()
    } else {
        s.to_string()
    }
}

fn body_from_paragraphs(paragraphs: &[String], italic: bool) -> String {
    let mut blocks: Vec<String> = Vec::new();
    let mut bullets: Vec<&str> = Vec::new();
    for p in paragraphs {
        if p.starts_with("- ") {
            bullets.push(p);
            continue;
        }
        if !bullets.is_empty() {
            blocks.push(bullets.join("\n"));
            bullets.clear();
        }
        blocks.push(if italic { format!("*{p}*") } else { p.clone() });
    }
    if !bullets.is_empty() {
        blocks.push(bullets.join("\n"));
    }
    blocks.join("\n\n")
}

/// NNN from the recording name, else the rank among recorded sessions; none without footage.
fn entry_number(entry: &Entry, sessions: &HashMap<String, Session>, entries: &[Entry]) -> Option<usize> {
    let rec = sessions.get(&entry.session)?.recording.as_deref()?;
    let name = rec.rsplit('/').next().unwrap_or(rec).as_bytes();
    let n = name.iter().take_while(|c| c.is_ascii_digit()).count();
    let digits = |r: &[u8]| r.iter().all(u8::is_ascii_digit);
    if (1..=4).contains(&n) && name.len() >= n + 16 && name[n] == b'-' && name[n + 9] == b'-' && digits(&name[n + 1..n + 9]) && digits(&name[n + 10..n + 16]) {
        return std::str::from_utf8(&name[..n]).ok()?.parse().ok();
    }
    Some(1 + entries.iter().filter(|e| e.session < entry.session && sessions.get(&e.session).is_some_and(|s| s.recording.is_some())).count())
}

fn render_block(entry: &Entry, sessions: &HashMap<String, Session>, entries: &[Entry], loc: &Locale) -> String {
    let lab = labels(&entry.lang);
    let session = sessions.get(&entry.session);
    let (start, end, duration) = session_span(session, entry);
    let meta = format!("{} · {}–{} · {}", loc.date(&start), fmt_time(&start), fmt_time(&end), fmt_duration(duration));
    let prefix = entry_number(entry, sessions, entries).map(|n| format!("#{n} · ")).unwrap_or_default();
    let head = if entry.title.is_empty() { format!("## {prefix}{meta}") } else { format!("## {prefix}{}\n*{meta}*", entry.title) };
    let mut parts = vec![format!("{head}\n<!-- session: {} -->", entry.session)];
    let body = body_from_paragraphs(&entry.paragraphs, entry.provider == "none");
    if !body.is_empty() {
        parts.push(body);
    }
    if !entry.next_up.is_empty() {
        parts.push(format!("**{}{}** {}", lab.next, lab.colon, entry.next_up));
    }
    if let Some(rec) = session.and_then(|s| s.recording.as_deref()) {
        parts.push(format!("**{}{}** [{}]({})", lab.recording, lab.colon, rec.rsplit('/').next().unwrap_or(rec), file_uri(rec)));
    }
    let (shots, frames): (Vec<&String>, Vec<&String>) = entry.images.iter().partition(|i| crate::screenshots::is_shot_name(i));
    if !shots.is_empty() {
        parts.push(shots.iter().map(|i| format!("![]({i})")).collect::<Vec<_>>().join("\n"));
    }
    if !frames.is_empty() {
        parts.push(format!("*{}*\n\n{}", lab.frames, frames.iter().map(|i| format!("![]({i})")).collect::<Vec<_>>().join("\n")));
    }
    parts.join("\n\n")
}

fn frontmatter(title: &str, body: &str) -> String {
    let mut sids: Vec<&str> = body
        .match_indices("<!-- session: ")
        .filter_map(|(i, m)| body[i + m.len()..].split_once(" -->").map(|(s, _)| s))
        .filter(|s| s.len() == 15 && s.is_ascii())
        .collect();
    sids.sort();
    let cover = body.lines().find_map(|l| l.strip_prefix("![](").and_then(|r| r.split_once(')')).map(|(p, _)| p));
    let mut lines = vec![format!("game: {}", yaml_str(title)), format!("sessions: {}", body.lines().filter(|l| l.starts_with("## ")).count())];
    if let (Some(first), Some(last)) = (sids.first(), sids.last()) {
        lines.push(format!("first_played: {}-{}-{}", &first[0..4], &first[4..6], &first[6..8]));
        lines.push(format!("last_played: {}-{}-{}", &last[0..4], &last[4..6], &last[6..8]));
    }
    if let Some(c) = cover {
        lines.push(format!("cover: {c}"));
    }
    format!("---\n{}\n---\n\n", lines.join("\n"))
}

/// Byte for byte what the journal module renders; pending and failed entries stay out of it.
pub fn render_note(title: &str, entries: &[Entry], sessions: &HashMap<String, Session>, loc: &Locale) -> String {
    let mut entries: Vec<Entry> = entries.iter().filter(|e| e.state == "written").cloned().collect();
    entries.sort_by(|a, b| b.session.cmp(&a.session));
    let lab = labels(entries.first().map(|e| e.lang.as_str()).unwrap_or("en"));
    let blocks: Vec<String> = entries.iter().map(|e| render_block(e, sessions, &entries, loc)).collect();
    let mut body = format!("# {}{} {title}\n\n{}\n", lab.journal, lab.colon, blocks.join("\n\n"));
    while body.contains("\n\n\n") {
        body = body.replace("\n\n\n", "\n\n");
    }
    format!("{}{body}", frontmatter(title, &body))
}

fn is_titled_note(text: &str) -> bool {
    text.lines().any(|l| {
        let Some(rest) = l.strip_prefix('#') else { return false };
        let rest = rest.trim_start();
        LABELS.iter().any(|(_, lab)| rest.strip_prefix(lab.journal).is_some_and(|r| r.trim_start().starts_with([':', '：'])))
    })
}

/// `<Title>.md`, or the folder's single marked note so a renamed game does not fork its history.
fn resolve_note_path(note_dir: &Path, title: &str) -> PathBuf {
    let preferred = note_dir.join(format!("{}.md", crate::slug::note_name(title)));
    let Ok(rd) = std::fs::read_dir(note_dir) else { return preferred };
    let names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    if names.iter().any(|p| p.file_name() == preferred.file_name()) {
        return preferred;
    }
    let (mut marked, mut headed) = (Vec::new(), Vec::new());
    for p in names {
        if !p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("md")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        if text.contains("<!-- session:") {
            marked.push(p);
        } else if is_titled_note(&text) {
            headed.push(p);
        }
    }
    let notes = if marked.is_empty() { headed } else { marked };
    if notes.len() == 1 { notes.into_iter().next().unwrap() } else { preferred }
}

/// Obsidian only follows links inside the vault, so referenced images are copied beside the note.
fn mirror_images(entries: &[Entry], journal_dir: &Path, screenshots_dir: &Path, note_dir: &Path) -> crate::Result<()> {
    if std::path::absolute(journal_dir)? == std::path::absolute(note_dir)? {
        return Ok(());
    }
    for rel in entries.iter().flat_map(|e| e.images.iter()) {
        if rel.starts_with('/') || rel.split('/').any(|seg| seg == "..") {
            continue;
        }
        let (src, dst) = (image_path(journal_dir, screenshots_dir, rel), note_dir.join(rel));
        let Ok(meta) = std::fs::metadata(&src) else { continue };
        if !meta.is_file() || std::fs::metadata(&dst).is_ok_and(|d| d.is_file() && d.len() == meta.len()) {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)?;
    }
    Ok(())
}

pub fn write_note(title: &str, entries: &[Entry], sessions: &HashMap<String, Session>, journal_dir: &Path, screenshots_dir: &Path, note_dir: &Path, loc: &Locale) -> crate::Result<PathBuf> {
    std::fs::create_dir_all(note_dir)?;
    let path = resolve_note_path(note_dir, title);
    let text = render_note(title, entries, sessions, loc);
    mirror_images(entries, journal_dir, screenshots_dir, note_dir)?;
    if std::fs::read_to_string(&path).is_ok_and(|old| old == text) {
        return Ok(path);
    }
    crate::game::atomic_write(&path, text.as_bytes())?;
    Ok(path)
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
        (sid.into(), Session { session: sid.into(), game: "sample".into(), started_at: started.into(), ended_at: ended.into(), duration_s: dur, source: "import-journal".into(), recording: rec.map(Into::into), ..Default::default() })
    }

    // The same sample as sample_entries() in modules/journal/tests/test_journal.py.
    fn sample() -> (Vec<Entry>, HashMap<String, Session>) {
        let entries = vec![
            entry("20260301-210000", "en", "Into the Dome", "import", &["Zachariah reached the Source after three failed runs.", "- **Main quest:** Cleared the gate.", "- **Side quest:** Talked to Amelia.", "Then the patrol reset."], "Return to the Exchange and talk to Amelia.", &["20260301-211500.png", "attachments/20260301-210000-1.png", "attachments/frames/frame-20260301-210000-02.jpg"]),
            entry("20260215-183000", "fr", "Trois contrats et Port-péril", "import", &["Le duo a enchaîné les sauvetages.", "- **Boss :** Tu as vaincu Corbin Claquebec."], "Tu reprendras dans le Mausolée III.", &["attachments/frames/frame-20260215-183000-01.jpg"]),
            entry("20260110-000500", "en", "", "import", &[], "", &[]),
            entry("20251220-120000", "en", "", "none", &["This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize."], "", &[]),
            entry("20251201-230000", "en", "First Glimpse", "import", &["You reached the title screen."], "Press any key.", &["20251201-230100.png"]),
        ];
        let sessions = HashMap::from([
            session("20260301-210000", "2026-03-01T21:00:00+01:00", "2026-03-01T22:30:00+01:00", 5400, Some("/mnt/recordings/games/sample/20260301-210000.mkv")),
            session("20260215-183000", "2026-02-15T18:30:00+01:00", "2026-02-15T19:45:00+01:00", 4500, Some("/mnt/recordings/games/sample/003-20260215-183000-1h15m.mkv")),
            session("20260110-000500", "2026-01-10T00:05:00+01:00", "2026-01-10T00:07:00+01:00", 120, Some("/mnt/recordings/games/sample/002-20260110-000500-2m.mkv")),
            session("20251220-120000", "2025-12-20T12:00:00+01:00", "2025-12-20T12:01:00+01:00", 60, Some("/mnt/recordings/games/sample/001-20251220-120000-1m.mkv")),
            session("20251201-230000", "2025-12-01T23:00:00+01:00", "2025-12-02T00:10:00+01:00", 4200, None),
        ]);
        (entries, sessions)
    }

    // What modules/journal/bin/note.py renders for sample() under LC_ALL=C.UTF-8 and TZ=Europe/Paris.
    const MODULE_NOTE: &str = r#"---
game: "Sample: The Game"
sessions: 5
first_played: 2025-12-01
last_played: 2026-03-01
cover: 20260301-211500.png
---

# Journal: Sample: The Game

## #4 · Into the Dome
*03/01/26 · 21:00–22:30 · 1 h 30 min*
<!-- session: 20260301-210000 -->

Zachariah reached the Source after three failed runs.

- **Main quest:** Cleared the gate.
- **Side quest:** Talked to Amelia.

Then the patrol reset.

**Next up:** Return to the Exchange and talk to Amelia.

**Recording:** [20260301-210000.mkv](file:///mnt/recordings/games/sample/20260301-210000.mkv)

![](20260301-211500.png)

*Frames from the recording*

![](attachments/20260301-210000-1.png)
![](attachments/frames/frame-20260301-210000-02.jpg)

## #3 · Trois contrats et Port-péril
*02/15/26 · 18:30–19:45 · 1 h 15 min*
<!-- session: 20260215-183000 -->

Le duo a enchaîné les sauvetages.

- **Boss :** Tu as vaincu Corbin Claquebec.

**Reprise :** Tu reprendras dans le Mausolée III.

**Enregistrement :** [003-20260215-183000-1h15m.mkv](file:///mnt/recordings/games/sample/003-20260215-183000-1h15m.mkv)

*Images extraites de l'enregistrement*

![](attachments/frames/frame-20260215-183000-01.jpg)

## #2 · 01/10/26 · 00:05–00:07 · 2 min
<!-- session: 20260110-000500 -->

**Recording:** [002-20260110-000500-2m.mkv](file:///mnt/recordings/games/sample/002-20260110-000500-2m.mkv)

## #1 · 12/20/25 · 12:00–12:01 · 1 min
<!-- session: 20251220-120000 -->

*This session’s recording holds no picture and no screenshot covers it, so there is nothing to summarize.*

**Recording:** [001-20251220-120000-1m.mkv](file:///mnt/recordings/games/sample/001-20251220-120000-1m.mkv)

## First Glimpse
*12/01/25 · 23:00–00:10 · 1 h 10 min*
<!-- session: 20251201-230000 -->

You reached the title screen.

**Next up:** Press any key.

![](20251201-230100.png)
"#;

    #[test]
    fn note_matches_the_module_renderer() {
        let (mut entries, sessions) = sample();
        assert_eq!(render_note("Sample: The Game", &entries, &sessions, &Locale::posix()), MODULE_NOTE);
        entries.push(Entry { session: "20260401-100000".into(), state: "pending".into(), ..Entry::default() });
        entries.push(Entry { session: "20260402-100000".into(), state: "failed".into(), paragraphs: vec!["timed out".into()], ..Entry::default() });
        assert_eq!(render_note("Sample: The Game", &entries, &sessions, &Locale::posix()), MODULE_NOTE, "state files never reach the note");
    }

    #[test]
    fn entry_timing_from_the_session_then_its_own_stamps() {
        let (mut entries, sessions) = sample();
        entries.push(Entry { session: "20260501-200000".into(), title: "Stamped".into(), started_at: "2026-05-01T20:00:00+02:00".into(), ended_at: "2026-05-01T20:45:00+02:00".into(), duration_s: 2700, ..Entry::default() });
        fill_timing(&mut entries, &sessions);
        assert_eq!(entries[0].started_at, "2026-03-01T21:00:00+01:00");
        assert_eq!(entries[0].ended_at, "2026-03-01T22:30:00+01:00");
        assert_eq!(entries[0].duration_s, 5400);
        assert_eq!(entries[5].duration_s, 2700, "an entry's own stamps stay");
        let md = render_note("Sample", &entries, &sessions, &Locale::posix());
        assert!(md.contains("## Stamped\n*05/01/26 · 20:00–20:45 · 45 min*"), "{md}");
    }

    #[test]
    fn state_files_list_as_entries() {
        let dir = tempfile::tempdir().unwrap();
        let journal_dir = dir.path().join("journal");
        std::fs::create_dir_all(&journal_dir).unwrap();
        write(&journal_dir, &Entry { session: "20260910-100000".into(), title: "Written".into(), ..Entry::default() }).unwrap();
        std::fs::write(journal_dir.join("20260910-110000.pending.json"), r#"{"session":"20260910-110000","game":"x","started_at":"2026-09-10T11:00:00+02:00","provider":"codex"}"#).unwrap();
        std::fs::write(journal_dir.join("20260910-120000.failed.json"), r#"{"session":"20260910-120000","game":"x","written_at":"2026-09-10T12:30:00+02:00","reason":"codex: rate limited"}"#).unwrap();
        std::fs::write(journal_dir.join("20260910-120000.pending.json"), r#"{"session":"20260910-120000","game":"x"}"#).unwrap();
        std::fs::write(journal_dir.join("20260910-130000.pending.json"), r#"{"session":"20260910-130000","game":"x","started_at":"2026-09-10T13:00:00+02:00"}"#).unwrap();
        let stale = std::fs::OpenOptions::new().write(true).open(journal_dir.join("20260910-130000.pending.json")).unwrap();
        stale.set_modified(std::time::SystemTime::now() - Duration::from_secs(31 * 60)).unwrap();
        std::fs::write(journal_dir.join(".game-memory.json"), "{}").unwrap();
        std::fs::write(journal_dir.join("notes.txt"), "x").unwrap();
        let all = read_all(&journal_dir).unwrap();
        let states: Vec<(&str, &str)> = all.iter().map(|e| (e.session.as_str(), e.state.as_str())).collect();
        assert_eq!(states, vec![("20260910-130000", "failed"), ("20260910-120000", "failed"), ("20260910-110000", "pending"), ("20260910-100000", "written")]);
        assert_eq!(all[0].paragraphs, vec!["timed out"]);
        assert_eq!(all[1].paragraphs, vec!["codex: rate limited"]);
        assert_eq!(all[1].written_at, "2026-09-10T12:30:00+02:00");
        assert_eq!((all[2].started_at.as_str(), all[2].provider.as_str(), all[2].title.as_str()), ("2026-09-10T11:00:00+02:00", "codex", ""));
        assert!(all[2].paragraphs.is_empty());
    }

    #[test]
    fn write_read_note() {
        let dir = tempfile::tempdir().unwrap();
        let journal_dir = dir.path().join("journal");
        let shots_dir = dir.path().join("screenshots");
        std::fs::create_dir_all(journal_dir.join("attachments")).unwrap();
        std::fs::create_dir_all(&shots_dir).unwrap();
        std::fs::write(journal_dir.join("attachments/a.png"), b"png").unwrap();
        std::fs::write(shots_dir.join("20260910-214000.png"), b"shot").unwrap();
        let e = Entry { session: "20260910-213045".into(), game: "x".into(), title: "Into the Dome".into(), paragraphs: vec!["A.".into(), "B.".into()], next_up: "Go.".into(), images: vec!["20260910-214000.png".into(), "attachments/a.png".into()], ..Default::default() };
        write(&journal_dir, &e).unwrap();
        let all = read_all(&journal_dir).unwrap();
        assert_eq!(all, vec![e.clone()]);
        let sessions = sessions_by_id(&[Session { session: "20260910-213045".into(), game: "x".into(), started_at: "2026-09-10T21:30:45+02:00".into(), ended_at: "2026-09-10T22:00:00+02:00".into(), duration_s: 1755, ..Default::default() }]);
        let note_dir = dir.path().join("vault").join("x");
        let path = write_note("X", &all, &sessions, &journal_dir, &shots_dir, &note_dir, &Locale::posix()).unwrap();
        assert_eq!(path, note_dir.join("X.md"));
        let md = std::fs::read_to_string(&path).unwrap();
        assert!(md.contains("## Into the Dome\n*09/10/26 · 21:30–22:00 · 29 min*\n<!-- session: 20260910-213045 -->\n\nA.\n\nB.\n\n**Next up:** Go.\n\n![](20260910-214000.png)\n\n*Frames from the recording*\n\n![](attachments/a.png)\n"), "{md}");
        assert_eq!(std::fs::read(note_dir.join("attachments/a.png")).unwrap(), b"png");
        assert_eq!(std::fs::read(note_dir.join("20260910-214000.png")).unwrap(), b"shot");
        assert_eq!(image_path(&journal_dir, &shots_dir, "20260910-214000.png"), shots_dir.join("20260910-214000.png"));
        assert_eq!(image_path(&journal_dir, &shots_dir, "attachments/a.png"), journal_dir.join("attachments/a.png"));
        let bad = Entry { images: vec!["/etc/passwd".into()], ..e };
        assert!(write(&journal_dir, &bad).is_err());
    }

    #[test]
    fn helpers_match_python() {
        assert_eq!(fmt_duration(90), "2 min");
        assert_eq!(fmt_duration(150), "2 min");
        assert_eq!(fmt_duration(3600), "1 h");
        assert_eq!(fmt_duration(5400), "1 h 30 min");
        assert_eq!(fmt_duration(0), "1 min");
        assert_eq!(yaml_str("Cuphead"), "Cuphead");
        assert_eq!(yaml_str("Sample: The Game"), "\"Sample: The Game\"");
        assert_eq!(yaml_str("1979"), "\"1979\"");
        assert_eq!(yaml_str("- x"), "\"- x\"");
        for typed in ["#DRIVE", "0x1F", "0o17", "0b101", "1_000", ".5", "1e3", "1:30", "2024-05-01", "2024-5-1 10:00", ".inf", ".NaN", "On", "y", "N"] {
            assert_eq!(yaml_str(typed), serde_json::to_string(typed).unwrap(), "{typed}");
        }
        for plain in ["Cuphead 2", "Half-Life 2", "1979 Revolution", "F.E.A.R.", "v1.0", "2024 Game", "Portal 2", "2001-a-space"] {
            assert_eq!(yaml_str(plain), plain);
        }
        assert_eq!(file_uri("/mnt/rec (1)/é.mkv"), "file:///mnt/rec%20%281%29/%C3%A9.mkv");
        assert_eq!(file_uri("/mnt/#DRIVE/What? A Game/x.mkv"), "file:///mnt/%23DRIVE/What%3F%20A%20Game/x.mkv");
        assert!(crate::screenshots::is_shot_name("20260301-211500.png") && !crate::screenshots::is_shot_name("attachments/20260301-211500.png") && !crate::screenshots::is_shot_name("attachments/20260301-210000-1.png"));
    }
}
