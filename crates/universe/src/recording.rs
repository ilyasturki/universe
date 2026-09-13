use std::path::{Path, PathBuf};

use crate::game::Game;
use crate::sessions::{self, Session};

/// `NNN-YYYYMMDD-HHMMSS-<dur>.mkv` (process-game-recording.mjs) or `<session>.mkv`.
pub fn parse_name(name: &str) -> Option<(String, u64)> {
    let stem = name.strip_suffix(".mkv")?;
    let parts: Vec<&str> = stem.split('-').collect();
    match parts.len() {
        2 if parts[0].len() == 8 && parts[1].len() == 6 => Some((stem.to_string(), 0)),
        4 if parts[1].len() == 8 && parts[2].len() == 6 => Some((format!("{}-{}", parts[1], parts[2]), parse_duration(parts[3]))),
        _ => None,
    }
}

pub fn parse_duration(s: &str) -> u64 {
    let mut total = 0u64;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
        } else {
            let n: u64 = num.parse().unwrap_or(0);
            num.clear();
            total += match c {
                'h' => n * 3600,
                'm' => n * 60,
                's' => n,
                _ => 0,
            };
        }
    }
    total
}

pub fn probe_duration(path: &Path) -> Option<u64> {
    let out = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path)
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().ok().map(|f| f.round() as u64)
}

fn session_times(session: &str, duration_s: u64) -> (String, String) {
    let start = chrono::NaiveDateTime::parse_from_str(session, "%Y%m%d-%H%M%S").ok();
    match start.and_then(|s| s.and_local_timezone(chrono::Local).single()) {
        Some(s) => (s.to_rfc3339(), (s + chrono::Duration::seconds(duration_s as i64)).to_rfc3339()),
        None => (String::new(), String::new()),
    }
}

/// One `import-recording` session per mkv not yet referenced (plan §7); returns how many were added.
pub fn import_existing(game: &Game, recordings_root: &Path) -> crate::Result<usize> {
    let dir = recordings_root.join(&game.id);
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(0) };
    let existing = sessions::read(&game.sessions_path())?;
    let mut added = 0;
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|s| s.to_str()) == Some("mkv")).collect();
    files.sort();
    for p in files {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let Some((sid, dur_from_name)) = parse_name(name) else { continue };
        let ps = p.to_string_lossy().to_string();
        if existing.iter().any(|s| s.recording.as_deref() == Some(ps.as_str()) || s.session == sid) {
            continue;
        }
        let duration_s = if dur_from_name > 0 { dur_from_name } else { probe_duration(&p).unwrap_or(0) };
        let (started_at, ended_at) = session_times(&sid, duration_s);
        sessions::append(&game.sessions_path(), &Session { session: sid, game: game.id.clone(), started_at, ended_at, duration_s, source: "import-recording".into(), recording: Some(ps), ..Default::default() })?;
        added += 1;
    }
    Ok(added)
}

/// Moves a filed recording to `<recordings_root>/<id>/<session>.mkv` and attaches it to the session line.
pub fn file(game: &Game, session_id: &str, src: &Path, recordings_root: &Path) -> crate::Result<PathBuf> {
    if !src.is_file() {
        return Err(crate::Error::NotFound(src.display().to_string()));
    }
    let dir = recordings_root.join(&game.id);
    std::fs::create_dir_all(&dir)?;
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("mkv");
    let dest = dir.join(format!("{session_id}.{ext}"));
    if dest != src {
        if std::fs::rename(src, &dest).is_err() {
            std::fs::copy(src, &dest)?;
            std::fs::remove_file(src)?;
        }
    }
    let ds = dest.to_string_lossy().to_string();
    let found = sessions::update(&game.sessions_path(), session_id, |s| s.recording = Some(ds.clone()))?;
    if !found {
        let duration_s = probe_duration(&dest).unwrap_or(0);
        let (started_at, ended_at) = session_times(session_id, duration_s);
        sessions::append(&game.sessions_path(), &Session { session: session_id.into(), game: game.id.clone(), started_at, ended_at, duration_s, source: "import-recording".into(), recording: Some(ds), ..Default::default() })?;
    }
    Ok(dest)
}

pub fn list(game: &Game) -> crate::Result<Vec<serde_json::Value>> {
    let sessions = sessions::read(&game.sessions_path())?;
    let mut out = Vec::new();
    for s in sessions.iter().rev() {
        let Some(rec) = &s.recording else { continue };
        let md = std::fs::metadata(rec).ok();
        out.push(serde_json::json!({
            "session": s.session,
            "path": rec,
            "size": md.as_ref().map(|m| m.len()).unwrap_or(0),
            "exists": md.is_some(),
            "duration_s": s.duration_s,
            "created_at": s.ended_at,
        }));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_durations() {
        assert_eq!(parse_name("004-20241216-015858-1h5m.mkv"), Some(("20241216-015858".into(), 3900)));
        assert_eq!(parse_name("20260911-120000.mkv"), Some(("20260911-120000".into(), 0)));
        assert_eq!(parse_name("clip.mkv"), None);
        assert_eq!(parse_duration("12m30s"), 750);
    }

    #[test]
    fn import_and_file() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let data = tempfile::tempdir().unwrap();
        std::env::set_var("UNIVERSE_DATA_HOME", data.path());
        let rec = tempfile::tempdir().unwrap();
        let g = Game::new("Dead Cells");
        std::fs::create_dir_all(rec.path().join("dead-cells")).unwrap();
        std::fs::write(rec.path().join("dead-cells/001-20241211-012656-30m.mkv"), b"x").unwrap();
        assert_eq!(import_existing(&g, rec.path()).unwrap(), 1);
        assert_eq!(import_existing(&g, rec.path()).unwrap(), 0);
        let pending = data.path().join("p.mkv");
        std::fs::write(&pending, b"y").unwrap();
        let dest = file(&g, "20260911-120000", &pending, rec.path()).unwrap();
        assert!(dest.ends_with("dead-cells/20260911-120000.mkv"));
        let l = list(&g).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0]["session"], "20260911-120000");
    }
}
