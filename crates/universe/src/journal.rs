use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Entry {
    pub session: String,
    pub game: String,
    pub written_at: String,
    pub lang: String,
    pub title: String,
    pub provider: String,
    pub paragraphs: Vec<String>,
    pub next_up: String,
    pub images: Vec<String>,
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

pub fn read_all(journal_dir: &Path) -> crate::Result<Vec<Entry>> {
    let mut out = Vec::new();
    let rd = match std::fs::read_dir(journal_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&p).map_err(crate::Error::from).and_then(|s| serde_json::from_str::<Entry>(&s).map_err(Into::into)) {
            Ok(mut en) => {
                if en.session.is_empty() {
                    en.session = p.file_stem().map(|s| s.to_string_lossy().into()).unwrap_or_default();
                }
                out.push(en)
            }
            Err(err) => tracing::warn!("{}: {err}", p.display()),
        }
    }
    out.sort_by(|a, b| b.session.cmp(&a.session));
    Ok(out)
}

pub fn write(journal_dir: &Path, entry: &Entry) -> crate::Result<std::path::PathBuf> {
    entry.validate()?;
    std::fs::create_dir_all(journal_dir)?;
    let p = journal_dir.join(format!("{}.json", entry.session));
    crate::game::atomic_write(&p, serde_json::to_string_pretty(entry)?.as_bytes())?;
    Ok(p)
}

/// Markdown rendering of the entries (newest first), Obsidian-compatible with the existing notes.
pub fn render_markdown(title: &str, entries: &[Entry], cover: Option<&str>) -> String {
    let mut sessions: Vec<&str> = entries.iter().map(|e| e.session.as_str()).collect();
    sessions.sort();
    let first = sessions.first().map(|s| date_of(s)).unwrap_or_default();
    let last = sessions.last().map(|s| date_of(s)).unwrap_or_default();
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("game: \"{}\"\n", title.replace('"', "\\\"")));
    out.push_str(&format!("sessions: {}\n", entries.len()));
    out.push_str(&format!("first_played: {first}\n"));
    out.push_str(&format!("last_played: {last}\n"));
    if let Some(c) = cover {
        out.push_str(&format!("cover: \"{c}\"\n"));
    }
    out.push_str("---\n\n");
    out.push_str(&format!("# {title}\n\n"));
    let n = entries.len();
    for (i, e) in entries.iter().enumerate() {
        let num = n - i;
        out.push_str(&format!("<!-- session: {} -->\n", e.session));
        out.push_str(&format!("## #{num} — {}\n\n", if e.title.is_empty() { "Session" } else { &e.title }));
        out.push_str(&format!("*{} · {}*\n\n", date_fr(&e.session), time_fr(&e.session)));
        for p in &e.paragraphs {
            out.push_str(p);
            out.push_str("\n\n");
        }
        if !e.images.is_empty() {
            for img in &e.images {
                out.push_str(&format!("![[{}]]\n", img.rsplit('/').next().unwrap_or(img)));
            }
            out.push('\n');
        }
        if !e.next_up.is_empty() {
            out.push_str(&format!("**Next up:** {}\n\n", e.next_up));
        }
    }
    out
}

fn date_of(session: &str) -> String {
    if session.len() >= 8 {
        format!("{}-{}-{}", &session[0..4], &session[4..6], &session[6..8])
    } else {
        String::new()
    }
}
fn date_fr(session: &str) -> String {
    if session.len() >= 8 {
        format!("{}/{}/{}", &session[6..8], &session[4..6], &session[0..4])
    } else {
        session.into()
    }
}
fn time_fr(session: &str) -> String {
    if session.len() >= 13 {
        format!("{}h{}", &session[9..11], &session[11..13])
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_render() {
        let dir = tempfile::tempdir().unwrap();
        let e = Entry { session: "20260910-213045".into(), game: "x".into(), title: "Into the Dome".into(), paragraphs: vec!["A.".into(), "B.".into()], next_up: "Go.".into(), images: vec!["attachments/a.png".into()], ..Default::default() };
        write(dir.path(), &e).unwrap();
        let all = read_all(dir.path()).unwrap();
        assert_eq!(all, vec![e.clone()]);
        let md = render_markdown("X", &all, None);
        assert!(md.contains("<!-- session: 20260910-213045 -->"));
        assert!(md.contains("*10/09/2026 · 21h30*"));
        let bad = Entry { images: vec!["/etc/passwd".into()], ..e };
        assert!(write(dir.path(), &bad).is_err());
    }
}
