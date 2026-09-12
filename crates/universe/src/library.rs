use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::game::Game;
use crate::journal::Entry;
use crate::sessions::{Session, Stats};
use crate::{paths, sessions};

pub const MEDIA_SLOTS: [&str; 4] = ["box_front", "tile", "background", "logo"];

#[derive(Debug, Clone, Default, Serialize)]
pub struct Resolved {
    pub game: Game,
    pub stats: Stats,
    pub sessions: Vec<Session>,
    pub media: Vec<(String, String)>,
    pub screenshots: Vec<String>,
    pub journal: Vec<Entry>,
    pub modules: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    pub effective: Effective,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Effective {
    pub proton: String,
    pub proton_path: String,
    pub esync: bool,
    pub fsync: bool,
    pub mangohud: bool,
    pub hide_cursor: bool,
    pub env: BTreeMap<String, String>,
}

impl Resolved {
    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(&self.game).unwrap_or(serde_json::Value::Null);
        v["stats"] = serde_json::to_value(&self.stats).unwrap();
        let mut media = serde_json::Map::new();
        for slot in MEDIA_SLOTS {
            let p = self.media.iter().find(|(s, _)| s == slot).map(|(_, p)| serde_json::Value::String(p.clone())).unwrap_or(serde_json::Value::Null);
            media.insert(slot.into(), p);
        }
        media.insert("screenshots".into(), serde_json::json!(self.screenshots));
        v["media"] = serde_json::Value::Object(media);
        v["modules"] = serde_json::to_value(&self.modules).unwrap();
        v["effective"] = serde_json::to_value(&self.effective).unwrap();
        v["installed"] = serde_json::Value::Bool(self.game.is_installed());
        v["removed"] = serde_json::Value::Bool(!self.game.removed_at.is_empty());
        v["journal_count"] = serde_json::json!(self.journal.len());
        v["recording_count"] = serde_json::json!(self.sessions.iter().filter(|s| s.recording.is_some()).count());
        v["dir"] = serde_json::json!(self.game.dir());
        v
    }
}

pub fn media_of(game: &Game, overrides: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let mut media = Vec::new();
    let mut shots = Vec::new();
    let mut dirs = vec![overrides.join(&game.id)];
    if !game.source.lutris_slug.is_empty() && game.source.lutris_slug != game.id {
        dirs.push(overrides.join(&game.source.lutris_slug));
    }
    dirs.push(game.media_dir());
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().and_then(|s| s.to_str()) == Some("screenshots") {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        let mut list: Vec<String> = rd2.flatten().map(|e| e.path()).filter(|p| is_image(p)).map(|p| p.to_string_lossy().into()).collect();
                        list.sort();
                        for s in list {
                            if !shots.contains(&s) {
                                shots.push(s);
                            }
                        }
                    }
                }
                continue;
            }
            if !is_image(&p) {
                continue;
            }
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let slot = match stem {
                "box_front" | "boxFront" | "cover" | "boxart" => "box_front",
                "tile" | "banner" | "grid" => "tile",
                "background" | "hero" | "fanart" => "background",
                "logo" => "logo",
                _ => continue,
            };
            if !media.iter().any(|(s, _): &(String, String)| s == slot) {
                media.push((slot.to_string(), p.to_string_lossy().to_string()));
            }
        }
    }
    (media, shots)
}

fn is_image(p: &Path) -> bool {
    matches!(p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp"))
}

pub fn resolve(game: Game, config: &Config, modules: &[crate::modules::Module]) -> Resolved {
    let sessions = sessions::read(&game.sessions_path()).unwrap_or_default();
    let stats = sessions::stats(&sessions);
    let (media, screenshots) = media_of(&game, &config.overrides_dir());
    let journal = crate::journal::read_all(&game.journal_dir()).unwrap_or_default();
    let mut mods = BTreeMap::new();
    for m in modules.iter().filter(|m| m.active() && m.is_hooks()) {
        mods.insert(m.id().to_string(), m.merged_settings(config, Some(&game)));
    }
    let proton = if game.launch.proton.is_empty() { config.launch.proton.clone() } else { game.launch.proton.clone() };
    let mut env = config.launch.env.clone();
    env.extend(game.launch.env.clone());
    let effective = Effective {
        proton_path: config.proton_path(&proton).map(|p| p.to_string_lossy().into()).unwrap_or_default(),
        proton,
        esync: game.launch.esync.unwrap_or(config.launch.esync),
        fsync: game.launch.fsync.unwrap_or(config.launch.fsync),
        mangohud: game.launch.mangohud.unwrap_or(config.launch.mangohud),
        hide_cursor: game.desktop.hide_cursor.unwrap_or(config.desktop.hide_cursor),
        env,
    };
    Resolved { game, stats, sessions, media, screenshots, journal, modules: mods, effective }
}

pub fn load_all(config: &Config, modules: &[crate::modules::Module]) -> Vec<Resolved> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(paths::games_dir()) else { return out };
    for e in rd.flatten() {
        let p = e.path().join("game.toml");
        if !p.is_file() {
            continue;
        }
        match Game::load(&p) {
            Ok(g) => out.push(resolve(g, config, modules)),
            Err(err) => tracing::warn!("{}: {err}", p.display()),
        }
    }
    sort_default(&mut out);
    out
}

/// Not hidden first, then most recently played, then title.
pub fn sort_default(list: &mut [Resolved]) {
    list.sort_by(|a, b| {
        let ra = (!a.game.removed_at.is_empty(), a.game.hidden);
        let rb = (!b.game.removed_at.is_empty(), b.game.hidden);
        ra.cmp(&rb)
            .then_with(|| b.stats.last_played.cmp(&a.stats.last_played))
            .then_with(|| a.game.title.to_lowercase().cmp(&b.game.title.to_lowercase()))
    });
}

/// exact › whole word › substring › path, on id, title and exe (the game script's four passes).
pub fn resolve_query<'a>(games: &'a [Resolved], query: &str) -> Vec<&'a Resolved> {
    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }
    let ql = q.to_lowercase();
    let qs = crate::slug::slug(q);
    let exact: Vec<&Resolved> = games.iter().filter(|g| g.game.id == qs || g.game.title.to_lowercase() == ql).collect();
    if !exact.is_empty() {
        return exact;
    }
    let word: Vec<&Resolved> = games
        .iter()
        .filter(|g| g.game.id.split('-').any(|w| w == qs) || g.game.title.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w == ql))
        .collect();
    if !word.is_empty() {
        return word;
    }
    let sub: Vec<&Resolved> = games.iter().filter(|g| g.game.id.contains(&qs) || g.game.title.to_lowercase().contains(&ql)).collect();
    if !sub.is_empty() {
        return sub;
    }
    let qp = PathBuf::from(q);
    games.iter().filter(|g| !g.game.launch.exe.is_empty() && (g.game.exe_path() == qp || g.game.exe_path().starts_with(&qp) || g.game.game_root() == qp)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(title: &str, exe: &str) -> Resolved {
        let mut g = Game::new(title);
        g.launch.exe = exe.into();
        Resolved { game: g, ..Default::default() }
    }

    #[test]
    fn four_passes() {
        let games = vec![r("Dead Cells", "/g/Dead Cells/deadcells.exe"), r("Mini Metro", "/g/Mini Metro/m.exe"), r("Mini Motorways", "/g/Mini Motorways/m.exe")];
        assert_eq!(resolve_query(&games, "dead cells").len(), 1);
        assert_eq!(resolve_query(&games, "metro")[0].game.id, "mini-metro");
        assert_eq!(resolve_query(&games, "mini").len(), 2);
        assert_eq!(resolve_query(&games, "otorw")[0].game.id, "mini-motorways");
        assert_eq!(resolve_query(&games, "/g/Dead Cells")[0].game.id, "dead-cells");
        assert!(resolve_query(&games, "zzz").is_empty());
    }
}
