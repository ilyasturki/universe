use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::game::Game;
use crate::sessions::{Session, Stats};
use crate::{paths, sessions};

pub const MEDIA_SLOTS: [&str; 5] = ["box_front", "square", "banner", "background", "logo"];
pub const IMAGE_EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Default, Serialize)]
pub struct Resolved {
    pub game: Game,
    pub stats: Stats,
    pub sessions: Vec<Session>,
    pub media: Vec<(String, String)>,
    pub screenshots: Vec<String>,
    pub journal_count: usize,
    pub modules: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    pub effective: Effective,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Effective {
    pub runner: String,
    pub runner_name: String,
    pub runner_kind: String,
    pub runner_path: String,
    pub platform: String,
    pub options: serde_json::Map<String, serde_json::Value>,
    pub inputplumber: bool,
    pub proton: String,
    pub proton_path: String,
    pub esync: bool,
    pub fsync: bool,
    pub ntsync: bool,
    pub wayland: bool,
    pub hdr: bool,
    pub dlss_upgrade: bool,
    pub fsr4_upgrade: bool,
    pub xess_upgrade: bool,
    pub optiscaler: bool,
    pub mangohud: bool,
    pub gamescope: bool,
    pub gamescope_args: String,
    #[serde(flatten)]
    pub gamescope_fields: crate::gamescope::Fields,
    /// `auto` (the refresh the game sees), `none`, or frames per second.
    pub fps_limit: String,
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
        v["platform"] = serde_json::Value::String(self.effective.platform.clone());
        v["removed"] = serde_json::Value::Bool(!self.game.removed_at.is_empty());
        v["journal_count"] = serde_json::json!(self.journal_count);
        v["recording_count"] = serde_json::json!(self.sessions.iter().filter(|s| s.recording.is_some()).count());
        v["dir"] = serde_json::json!(self.game.dir());
        v
    }
}

/// Pegasus's `tile` is the 1:1 grid and its `steam` the 920×430 banner.
pub fn stems_of(slot: &str) -> &'static [&'static str] {
    match slot {
        "box_front" => &["box_front", "boxFront", "cover", "boxart"],
        "square" => &["square", "tile", "icon"],
        "banner" => &["banner", "steam", "grid"],
        "background" => &["background", "hero", "fanart"],
        "logo" => &["logo"],
        _ => &[],
    }
}

pub fn slot_of_stem(stem: &str) -> Option<&'static str> {
    MEDIA_SLOTS.into_iter().find(|s| stems_of(s).contains(&stem))
}

/// First match wins: the overrides by id, then by the Lutris slug, then the game's own media directory.
pub fn media_dirs(game: &Game, overrides: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![overrides.join(&game.id)];
    if !game.source.lutris_slug.is_empty() && game.source.lutris_slug != game.id {
        dirs.push(overrides.join(&game.source.lutris_slug));
    }
    dirs.push(game.media_dir());
    dirs
}

pub fn scan_media_dir(dir: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let mut media: Vec<(String, String, usize)> = Vec::new();
    let mut shots = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return (Vec::new(), shots) };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().and_then(|s| s.to_str()) == Some("screenshots") {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    let mut list: Vec<String> = rd2.flatten().map(|e| e.path()).filter(|p| is_image(p)).map(|p| p.to_string_lossy().into()).collect();
                    list.sort();
                    shots.extend(list);
                }
            }
            continue;
        }
        if !is_image(&p) {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let Some(slot) = slot_of_stem(stem) else { continue };
        let rank = stems_of(slot).iter().position(|s| *s == stem).unwrap_or(usize::MAX);
        match media.iter_mut().find(|(s, _, _)| s == slot) {
            Some(have) if have.2 > rank => *have = (slot.to_string(), p.to_string_lossy().to_string(), rank),
            Some(_) => {}
            None => media.push((slot.to_string(), p.to_string_lossy().to_string(), rank)),
        }
    }
    (media.into_iter().map(|(s, p, _)| (s, p)).collect(), shots)
}

pub fn media_of(game: &Game, overrides: &Path) -> (Vec<(String, String)>, Vec<String>) {
    let mut media: Vec<(String, String)> = Vec::new();
    let mut shots: Vec<String> = Vec::new();
    for dir in media_dirs(game, overrides) {
        let (m, s) = scan_media_dir(&dir);
        for (slot, path) in m {
            if !media.iter().any(|(have, _)| *have == slot) {
                media.push((slot, path));
            }
        }
        for shot in s {
            if !shots.contains(&shot) {
                shots.push(shot);
            }
        }
    }
    (media, shots)
}

pub fn is_image(p: &Path) -> bool {
    p.extension().and_then(|s| s.to_str()).is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The game's gamescope fields over the global ones, a field left empty taking `[launch]`'s.
fn gamescope_fields_of(game: &Game, config: &Config) -> crate::gamescope::Fields {
    let l = &game.launch;
    let d = &config.launch;
    let pick = |own: &str, global: &str| if own.is_empty() { global.to_string() } else { own.to_string() };
    let or_default = |global: &str, key: &str| if global.is_empty() { crate::launch_keys::default_of(key).to_string() } else { global.to_string() };
    crate::gamescope::Fields {
        resolution: pick(&l.gamescope_resolution, &or_default(&d.gamescope_resolution, "gamescope_resolution")),
        refresh: pick(&l.gamescope_refresh, &or_default(&d.gamescope_refresh, "gamescope_refresh")),
        scaler: pick(&l.gamescope_scaler, &d.gamescope_scaler),
        filter: pick(&l.gamescope_filter, &d.gamescope_filter),
        sharpness: l.gamescope_sharpness.or(d.gamescope_sharpness),
        adaptive_sync: l.gamescope_adaptive_sync.unwrap_or(d.gamescope_adaptive_sync),
    }
}

pub fn resolve(game: Game, config: &Config, modules: &[crate::modules::Module]) -> Resolved {
    resolve_with(game, config, modules, &mut HashMap::new())
}

/// `located` memoises each runner's program and each Proton's path across the games of one load.
pub fn resolve_with(game: Game, config: &Config, modules: &[crate::modules::Module], located: &mut HashMap<String, String>) -> Resolved {
    let sessions = sessions::read(&game.sessions_path()).unwrap_or_default();
    let stats = sessions::stats(&sessions);
    let (media, screenshots) = media_of(&game, &config.overrides_dir());
    let journal_count = crate::journal::count_written(&game.journal_dir());
    let mut mods = BTreeMap::new();
    for m in modules.iter().filter(|m| m.active() && m.is_hooks()) {
        mods.insert(m.id().to_string(), m.merged_settings(config, Some(&game)));
    }
    let proton = if game.launch.proton.is_empty() { config.launch.proton.clone() } else { game.launch.proton.clone() };
    let mut env = config.launch.env.clone();
    env.extend(game.launch.env.clone());
    let runner = game.runner_id();
    let spec = crate::runners::spec(&runner);
    let runner_path = if !game.launch.runner_exe.is_empty() {
        paths::expand(&game.launch.runner_exe).to_string_lossy().into()
    } else {
        spec.map(|s| located.entry(s.id.into()).or_insert_with(|| crate::runners::locate(s, config).program).clone()).unwrap_or_default()
    };
    let options = spec.map(|s| s.merged_options(config, Some(&game))).unwrap_or_default();
    let gamescope = game.launch.gamescope.or_else(|| config.runners.get(&runner).and_then(|t| t.get("gamescope")).and_then(|v| v.as_bool())).unwrap_or(config.launch.gamescope);
    let effective = Effective {
        runner_name: spec.map(|s| s.name.to_string()).unwrap_or_else(|| runner.clone()),
        runner_kind: spec.map(|s| s.kind.as_str().to_string()).unwrap_or_default(),
        runner_path,
        platform: if game.platform.is_empty() { spec.map(|s| s.default_platform().to_string()).unwrap_or_default() } else { game.platform.clone() },
        inputplumber: options.get("inputplumber").and_then(|v| v.as_bool()).unwrap_or(false),
        options,
        runner,
        proton_path: located.entry(format!("proton:{proton}")).or_insert_with(|| config.proton_path(&proton).map(|p| p.to_string_lossy().into()).unwrap_or_default()).clone(),
        proton,
        esync: game.launch.esync.unwrap_or(config.launch.esync),
        fsync: game.launch.fsync.unwrap_or(config.launch.fsync),
        ntsync: game.launch.ntsync.unwrap_or(config.launch.ntsync),
        wayland: game.launch.wayland.unwrap_or(config.launch.wayland),
        hdr: game.launch.hdr.unwrap_or(config.launch.hdr),
        dlss_upgrade: game.launch.dlss_upgrade.unwrap_or(config.launch.dlss_upgrade),
        fsr4_upgrade: game.launch.fsr4_upgrade.unwrap_or(config.launch.fsr4_upgrade),
        xess_upgrade: game.launch.xess_upgrade.unwrap_or(config.launch.xess_upgrade),
        optiscaler: game.launch.optiscaler.unwrap_or(config.launch.optiscaler),
        mangohud: game.launch.mangohud.unwrap_or(config.launch.mangohud),
        gamescope,
        gamescope_args: game.launch.gamescope_args.clone(),
        gamescope_fields: gamescope_fields_of(&game, config),
        fps_limit: [&game.launch.fps_limit, &config.launch.fps_limit].into_iter().find(|s| !s.is_empty()).cloned().unwrap_or_else(|| crate::launch_keys::default_of("fps_limit").into()),
        hide_cursor: game.desktop.hide_cursor.unwrap_or(config.desktop.hide_cursor),
        env,
    };
    Resolved { game, stats, sessions, media, screenshots, journal_count, modules: mods, effective }
}

pub fn load_all(config: &Config, modules: &[crate::modules::Module]) -> Vec<Resolved> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(paths::games_dir()) else { return out };
    let mut located = HashMap::new();
    for e in rd.flatten() {
        let p = e.path().join("game.toml");
        if !p.is_file() {
            continue;
        }
        match Game::load(&p) {
            Ok(g) => out.push(resolve_with(g, config, modules, &mut located)),
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

/// exact › whole word › substring › path on id, title and exe, then every query word a prefix of a title word or genre.
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
    let by_path: Vec<&Resolved> = games.iter().filter(|g| !g.game.launch.exe.is_empty() && (g.game.exe_path() == qp || g.game.exe_path().starts_with(&qp) || g.game.game_root() == qp)).collect();
    if !by_path.is_empty() {
        return by_path;
    }
    let words: Vec<&str> = ql.split_whitespace().collect();
    games.iter().filter(|g| words.iter().all(|w| g.game.title.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|t| t.starts_with(w)) || g.game.metadata.genres.iter().any(|x| x.to_lowercase().starts_with(w)))).collect()
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
        assert_eq!(resolve_query(&games, "mot mini")[0].game.id, "mini-motorways");
        assert!(resolve_query(&games, "zzz").is_empty());
    }
}
