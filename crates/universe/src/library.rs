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
    pub achievements: crate::achievements::Summary,
    pub effective: Effective,
    pub version: crate::versions::Installed,
    pub check: crate::upstream::Check,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Effective {
    pub runner: String,
    pub runner_name: String,
    pub runner_kind: String,
    pub runner_path: String,
    pub platform: String,
    pub options: serde_json::Map<String, serde_json::Value>,
    pub proton: String,
    pub proton_path: String,
    /// umu-run's `GAMEID`: `launch.umu_id`, else `umu-<metadata.steam_appid>` for a `proton` game, else `umu-default`.
    pub umu_id: String,
    pub esync: bool,
    pub fsync: bool,
    pub ntsync: bool,
    pub wayland: bool,
    pub hdr: bool,
    pub discrete_gpu: bool,
    pub dlss_upgrade: bool,
    pub fsr4_upgrade: bool,
    pub xess_upgrade: bool,
    pub optiscaler: bool,
    pub debug_log: bool,
    /// What a PlayStation pad is to a Proton game (`native`, `dualshock4`, `xinput`), `auto` resolved; empty when a `PROTON_SONY_*` of `env` decides.
    pub sony_pad: String,
    pub mangohud: bool,
    pub pause_on_home: bool,
    pub gamescope: bool,
    pub gamescope_args: String,
    #[serde(flatten)]
    pub gamescope_fields: crate::gamescope::Fields,
    pub fps_limit: String,
    pub hide_cursor: bool,
    pub env: BTreeMap<String, String>,
    /// The folder the program starts in and the Wine prefix, the game's own or the ones the launch would make.
    pub working_dir: String,
    pub prefix: String,
    /// Each active module's settings for this game: its defaults, then the config's, then the game's own.
    pub modules: BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
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
        v["effective"] = serde_json::to_value(&self.effective).unwrap();
        v["installed"] = serde_json::Value::Bool(self.game.is_installed());
        v["platform"] = serde_json::Value::String(self.effective.platform.clone());
        v["removed"] = serde_json::Value::Bool(!self.game.removed_at.is_empty());
        v["journal_count"] = serde_json::json!(self.journal_count);
        v["achievements"] = serde_json::to_value(self.achievements).unwrap();
        v["recording_count"] = serde_json::json!(self.sessions.iter().filter(|s| s.recording.is_some()).count());
        v["dir"] = serde_json::json!(self.game.dir());
        let mut version = serde_json::to_value(&self.game.version).unwrap();
        for part in [serde_json::to_value(&self.version).unwrap(), serde_json::to_value(&self.check).unwrap()] {
            if let (Some(own), serde_json::Value::Object(part)) = (version.as_object_mut(), part) {
                own.extend(part);
            }
        }
        let (installed, from) = self.installed();
        (version["installed"], version["from"]) = (serde_json::json!(installed), serde_json::json!(from));
        v["version"] = version;
        v
    }

    /// The installed version and where it was read: the store's name of the build over what the files say, unless pinned.
    pub fn installed(&self) -> (&str, &str) {
        if !self.check.version_name.is_empty() && self.version.from != "pin" {
            (&self.check.version_name, &self.check.store)
        } else {
            (&self.version.installed, &self.version.from)
        }
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

/// First match wins: the player's picks, then the fetched media.
pub fn media_dirs(game: &Game) -> [PathBuf; 2] {
    [game.picked_dir(), game.media_dir()]
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

pub fn media_of(game: &Game) -> (Vec<(String, String)>, Vec<String>) {
    let mut media: Vec<(String, String)> = Vec::new();
    let mut shots: Vec<String> = Vec::new();
    for dir in media_dirs(game) {
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

/// A game's switch over the global one; `auto` is what the GPU wants (off when none is known).
fn upgrade(key: &str, own: Option<crate::config::Toggle>, global: crate::config::Toggle) -> bool {
    own.unwrap_or(global).or(|| crate::gpu::detected().and_then(|g| g.wants(key)).unwrap_or(false))
}

pub(crate) fn gamescope_fields_of(game: &Game, config: &Config) -> crate::gamescope::Fields {
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

/// A GE-Proton PlayStation pad switch already in the environment: set by hand, it takes over from `launch.sony_pad`.
pub fn sony_by_hand<'a>(keys: impl IntoIterator<Item = &'a String>) -> Option<&'a String> {
    keys.into_iter().find(|k| k.starts_with("PROTON_SONY_"))
}

pub fn sony_pad_auto(playstation: &str) -> &'static str {
    match playstation {
        "dualsense" => "native",
        "dualshock" => "dualshock4",
        _ => "xinput",
    }
}

pub fn resolve(game: Game, config: &Config, modules: &[crate::modules::Module]) -> Resolved {
    resolve_with(game, config, modules, &mut HashMap::new())
}

/// `located` memoises each runner's program and each Proton's path across the games of one load.
pub fn resolve_with(game: Game, config: &Config, modules: &[crate::modules::Module], located: &mut HashMap<String, String>) -> Resolved {
    let sessions = sessions::read(&game.sessions_path()).unwrap_or_default();
    let stats = sessions::stats(&sessions);
    let (media, screenshots) = media_of(&game);
    let journal_count = crate::journal::count_written(&game.journal_dir());
    let achievements = crate::achievements::read(&crate::achievements::path(&game)).map(|c| c.summary()).unwrap_or_default();
    let mut mods = BTreeMap::new();
    let kind = game.runner_kind();
    for m in modules.iter().filter(|m| m.active() && m.manifest.applies.takes(kind)) {
        mods.insert(m.id().to_string(), m.merged_settings(config, Some(&game)));
    }
    let proton = if game.launch.proton.is_empty() { config.launch.proton.clone() } else { game.launch.proton.clone() };
    let mut env = config.launch.env.clone();
    env.extend(game.launch.env.clone());
    let runner = game.runner_id();
    let spec = crate::runners::spec(&runner);
    let runner_path = runner_path(&game, spec, |s| located.entry(s.id.into()).or_insert_with(|| crate::runners::locate(s, config).program).clone());
    let home = spec.and_then(|s| crate::runners::home(s, Path::new(&runner_path), &paths::xdg("XDG_DATA_HOME", ".local/share")));
    let working_dir = crate::launcher::working_dir(&game, home.as_ref()).to_string_lossy().into_owned();
    let options = spec.map(|s| s.merged_options(config, Some(&game))).unwrap_or_default();
    let gamescope = game
        .launch
        .gamescope
        .or_else(|| config.runners.get(&runner).and_then(|t| t.get("gamescope")).and_then(|v| v.as_bool()))
        .unwrap_or(config.launch.gamescope);
    let effective = Effective {
        runner_name: spec.map(|s| s.name.to_string()).unwrap_or_else(|| runner.clone()),
        runner_kind: spec.map(|s| s.kind.as_str().to_string()).unwrap_or_default(),
        runner_path,
        platform: if game.platform.is_empty() { spec.map(|s| s.default_platform().to_string()).unwrap_or_default() } else { game.platform.clone() },
        options,
        runner,
        proton_path: located
            .entry(format!("proton:{proton}"))
            .or_insert_with(|| config.proton_path(&proton).map(|p| p.to_string_lossy().into()).unwrap_or_default())
            .clone(),
        proton,
        umu_id: match (game.launch.umu_id.as_str(), game.metadata.steam_appid) {
            ("", appid) if appid > 0 && spec.is_some_and(|s| s.kind == crate::runners::Kind::Proton) => format!("umu-{appid}"),
            ("", _) => "umu-default".into(),
            (own, _) => own.into(),
        },
        esync: game.launch.esync.unwrap_or(config.launch.esync),
        fsync: game.launch.fsync.unwrap_or(config.launch.fsync),
        ntsync: game.launch.ntsync.unwrap_or(config.launch.ntsync),
        wayland: game.launch.wayland.unwrap_or(config.launch.wayland),
        hdr: game.launch.hdr.unwrap_or(config.launch.hdr),
        discrete_gpu: game.launch.discrete_gpu.unwrap_or(config.launch.discrete_gpu),
        dlss_upgrade: upgrade("dlss_upgrade", game.launch.dlss_upgrade, config.launch.dlss_upgrade),
        fsr4_upgrade: upgrade("fsr4_upgrade", game.launch.fsr4_upgrade, config.launch.fsr4_upgrade),
        xess_upgrade: upgrade("xess_upgrade", game.launch.xess_upgrade, config.launch.xess_upgrade),
        optiscaler: game.launch.optiscaler.unwrap_or(config.launch.optiscaler),
        debug_log: game.launch.debug_log.unwrap_or(config.launch.debug_log),
        sony_pad: if sony_by_hand(env.keys()).is_some() {
            String::new()
        } else {
            match [&game.launch.sony_pad, &config.launch.sony_pad].into_iter().find(|s| !s.is_empty()).map_or("auto", String::as_str) {
                "auto" => sony_pad_auto(&game.metadata.playstation).into(),
                set => set.into(),
            }
        },
        mangohud: game.launch.mangohud.unwrap_or(config.launch.mangohud),
        pause_on_home: game.launch.pause_on_home.unwrap_or(config.launch.pause_on_home),
        gamescope,
        gamescope_args: game.launch.gamescope_args.clone(),
        gamescope_fields: gamescope_fields_of(&game, config),
        fps_limit: [&game.launch.fps_limit, &config.launch.fps_limit]
            .into_iter()
            .find(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| crate::launch_keys::default_of("fps_limit").into()),
        hide_cursor: game.desktop.hide_cursor.unwrap_or(config.desktop.hide_cursor),
        env,
        working_dir,
        prefix: if spec.is_some_and(|s| matches!(s.kind, crate::runners::Kind::Proton | crate::runners::Kind::Wine)) {
            crate::launcher::prefix_of(&game, config).to_string_lossy().into()
        } else {
            String::new()
        },
        modules: mods,
    };
    Resolved { game, stats, sessions, media, screenshots, journal_count, achievements, effective, version: Default::default(), check: Default::default() }
}

pub fn runner_path(game: &Game, spec: Option<&crate::runners::RunnerSpec>, locate: impl FnOnce(&crate::runners::RunnerSpec) -> String) -> String {
    let own_build = spec
        .filter(|_| !game.launch.runner_build.is_empty())
        .and_then(|s| crate::components::find_build(s.id, &game.launch.runner_build).filter(|b| b.version == game.launch.runner_build));
    if !game.launch.runner_exe.is_empty() {
        paths::expand(&game.launch.runner_exe).to_string_lossy().into()
    } else if let Some(b) = own_build {
        b.program_path().to_string_lossy().into()
    } else {
        spec.map(locate).unwrap_or_default()
    }
}

/// `resolve`, the version the game's files say and how it stands by the answers kept: what `load_all` makes of each game.
pub fn load_one(game: Game, config: &Config, modules: &[crate::modules::Module]) -> Resolved {
    let mut r = resolve(game, config, modules);
    r.version = crate::versions::read_one(&r.game);
    r.check = crate::upstream::check(&r.game, &r.version, &crate::upstream::Upstream::load().with_sources(&config.sources.enabled));
    r
}

pub fn load_all(config: &Config, modules: &[crate::modules::Module]) -> Vec<Resolved> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(paths::games_dir()) else { return out };
    let mut located = HashMap::new();
    let mut probes = crate::versions::Probes::default();
    let upstream = crate::upstream::Upstream::load().with_sources(&config.sources.enabled);
    for e in rd.flatten() {
        let p = e.path().join("game.toml");
        if !p.is_file() {
            continue;
        }
        match Game::load(&p) {
            Ok(g) => {
                let mut r = resolve_with(g, config, modules, &mut located);
                r.version = crate::versions::read(&r.game, &mut probes);
                r.check = crate::upstream::check(&r.game, &r.version, &upstream);
                out.push(r);
            }
            Err(err) => tracing::warn!("{}: {err}", p.display()),
        }
    }
    probes.save();
    sort_default(&mut out);
    out
}

pub fn sort_default(list: &mut [Resolved]) {
    list.sort_by(|a, b| {
        let ra = (!a.game.removed_at.is_empty(), a.game.hidden);
        let rb = (!b.game.removed_at.is_empty(), b.game.hidden);
        ra.cmp(&rb).then_with(|| b.stats.last_played.cmp(&a.stats.last_played)).then_with(|| a.game.title.to_lowercase().cmp(&b.game.title.to_lowercase()))
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
    let by_path: Vec<&Resolved> = games
        .iter()
        .filter(|g| !g.game.launch.exe.is_empty() && (g.game.exe_path() == qp || g.game.exe_path().starts_with(&qp) || g.game.game_root() == qp))
        .collect();
    if !by_path.is_empty() {
        return by_path;
    }
    let words: Vec<&str> = ql.split_whitespace().collect();
    games
        .iter()
        .filter(|g| {
            words.iter().all(|w| {
                g.game.title.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|t| t.starts_with(w))
                    || g.game.metadata.genres.iter().any(|x| x.to_lowercase().starts_with(w))
            })
        })
        .collect()
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
        let games =
            vec![r("Dead Cells", "/g/Dead Cells/deadcells.exe"), r("Mini Metro", "/g/Mini Metro/m.exe"), r("Mini Motorways", "/g/Mini Motorways/m.exe")];
        assert_eq!(resolve_query(&games, "dead cells").len(), 1);
        assert_eq!(resolve_query(&games, "metro")[0].game.id, "mini-metro");
        assert_eq!(resolve_query(&games, "mini").len(), 2);
        assert_eq!(resolve_query(&games, "otorw")[0].game.id, "mini-motorways");
        assert_eq!(resolve_query(&games, "/g/Dead Cells")[0].game.id, "dead-cells");
        assert_eq!(resolve_query(&games, "mot mini")[0].game.id, "mini-motorways");
        assert!(resolve_query(&games, "zzz").is_empty());
    }
}
