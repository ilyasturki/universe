use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::config::Config;
use crate::game::Game;
use crate::paths;
use crate::sessions::{self, Session};

#[derive(Debug, Clone)]
pub struct PgaGame {
    pub name: String,
    pub slug: String,
    pub runner: String,
    pub platform: String,
    pub hidden: bool,
    pub playtime_h: f64,
    pub lastplayed: i64,
    pub service: String,
    pub service_id: String,
    pub configpath: String,
    pub directory: String,
    pub year: i64,
}

pub fn read_pga(pga: &Path) -> crate::Result<Vec<PgaGame>> {
    let conn = rusqlite::Connection::open_with_flags(pga, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut st = conn.prepare("SELECT name, slug, runner, platform, hidden, playtime, lastplayed, service, service_id, configpath, directory, year FROM games WHERE installed = 1")?;
    let rows = st.query_map([], |r| {
        Ok(PgaGame {
            name: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            slug: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            runner: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            platform: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            hidden: r.get::<_, Option<i64>>(4)?.unwrap_or(0) == 1,
            playtime_h: r.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
            lastplayed: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
            service: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
            service_id: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
            configpath: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
            directory: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
            year: r.get::<_, Option<i64>>(11)?.unwrap_or(0),
        })
    })?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn yaml_str(v: &serde_yaml::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    match cur {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn yaml_bool(v: &serde_yaml::Value, path: &[&str]) -> Option<bool> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    cur.as_bool()
}

fn yaml_map(v: &serde_yaml::Value, path: &[&str]) -> BTreeMap<String, String> {
    let mut cur = v;
    for p in path {
        match cur.get(*p) {
            Some(c) => cur = c,
            None => return BTreeMap::new(),
        }
    }
    cur.as_mapping()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let key = k.as_str()?.to_string();
                    let val = match v {
                        serde_yaml::Value::String(s) => s.clone(),
                        serde_yaml::Value::Number(n) => n.to_string(),
                        serde_yaml::Value::Bool(b) => b.to_string(),
                        serde_yaml::Value::Null => String::new(),
                        _ => return None,
                    };
                    Some((key, val))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct RunnerHint {
    pub runner: String,
    pub lutris_runner: String,
    pub executable: String,
    pub program: String,
    pub args: Vec<String>,
    pub wrapped: bool,
}

pub fn runner_hints(lutris_dir: &Path) -> Vec<RunnerHint> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(lutris_dir.join("runners")) else { return out };
    let mut files: Vec<std::path::PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yml")).collect();
    files.sort();
    for f in files {
        let lutris_runner = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let Some(spec) = crate::runners::spec(&lutris_runner) else { continue };
        if spec.kind != crate::runners::Kind::Emulator {
            continue;
        }
        let yml: serde_yaml::Value = std::fs::read_to_string(&f).ok().and_then(|s| serde_yaml::from_str(&s).ok()).unwrap_or(serde_yaml::Value::Null);
        let Some(executable) = yaml_str(&yml, &[&lutris_runner, "runner_executable"]) else { continue };
        let (program, args, wrapped) = see_through(Path::new(&executable));
        out.push(RunnerHint { runner: spec.id.into(), lutris_runner, executable, program, args, wrapped });
    }
    out
}

fn see_through(exe: &Path) -> (String, Vec<String>, bool) {
    let itself = (exe.to_string_lossy().to_string(), vec![], false);
    let mut magic = [0u8; 2];
    if std::fs::File::open(exe).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut magic)).is_err() || &magic != b"#!" {
        return itself;
    }
    let Ok(text) = std::fs::read_to_string(exe) else { return itself };
    let Some(line) = text.lines().find(|l| l.trim_start().starts_with("exec ")) else { return itself };
    let Ok(words) = shell_words::split(line.trim_start().trim_start_matches("exec ")) else { return itself };
    let mut words: Vec<String> = words.into_iter().filter(|w| w != "\"$@\"" && w != "$@").collect();
    // emu-pad: the user's InputPlumber wrapper; the core does that itself now.
    let wrapped = words.first().map(|w| w.ends_with("emu-pad")).unwrap_or(false);
    if wrapped {
        words.remove(0);
    }
    match words.split_first() {
        Some((program, rest)) => (program.clone(), rest.to_vec(), wrapped),
        None => itself,
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GogManifest {
    pub game_id: String,
    pub root_game_id: String,
    pub build_id: String,
    pub name: String,
    pub primary_exe: String,
}

/// Reads the base game's goggame-<id>.info in `dir` (gameId == rootGameId).
pub fn read_gog_manifest(dir: &Path) -> Option<GogManifest> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best: Option<GogManifest> = None;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !(name.starts_with("goggame-") && name.ends_with(".info")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
        let gid = v["gameId"].as_str().unwrap_or("").to_string();
        let root = v["rootGameId"].as_str().unwrap_or(&gid).to_string();
        if gid != root {
            continue;
        }
        let exe = v["playTasks"]
            .as_array()
            .and_then(|t| t.iter().find(|t| t["isPrimary"].as_bool() == Some(true)).or_else(|| t.first()))
            .and_then(|t| t["path"].as_str())
            .unwrap_or("")
            .to_string();
        best = Some(GogManifest { game_id: gid, root_game_id: root, build_id: v["buildId"].as_str().unwrap_or("").to_string(), name: v["name"].as_str().unwrap_or("").to_string(), primary_exe: exe });
        break;
    }
    best
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EnvDiff {
    pub id: String,
    pub title: String,
    pub lutris_env: BTreeMap<String, String>,
    pub universe_env: BTreeMap<String, String>,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub runners_promoted: Vec<String>,
    pub runners: Vec<RunnerHint>,
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub updated: Vec<String>,
    pub hours_imported: BTreeMap<String, f64>,
    pub media_imported: Vec<String>,
    pub env_diffs: Vec<EnvDiff>,
    pub applied: bool,
}

pub struct Imported {
    pub game: Game,
    pub lutris_env: BTreeMap<String, String>,
    pub playtime_h: f64,
    pub lastplayed: i64,
}

/// Builds a game.toml from one Lutris entry; runner ≠ wine → backend "emulator", parked (plan §13).
pub fn convert(p: &PgaGame, lutris_dir: &Path, global_env: &BTreeMap<String, String>) -> Imported {
    let mut g = Game::new(&p.name);
    g.hidden = p.hidden;
    g.release_year = p.year.max(0) as u32;
    g.source.lutris_slug = p.slug.clone();
    let yml_path = lutris_dir.join("games").join(format!("{}.yml", p.configpath));
    let yml: serde_yaml::Value = std::fs::read_to_string(&yml_path).ok().and_then(|s| serde_yaml::from_str(&s).ok()).unwrap_or(serde_yaml::Value::Null);
    let mut lutris_env = global_env.clone();
    lutris_env.extend(yaml_map(&yml, &["system", "env"]));
    let mut parked = toml::Table::new();
    parked.insert("slug".into(), toml::Value::String(p.slug.clone()));
    parked.insert("runner".into(), toml::Value::String(p.runner.clone()));
    parked.insert("configpath".into(), toml::Value::String(p.configpath.clone()));
    parked.insert("playtime_h".into(), toml::Value::Float(p.playtime_h));
    if p.runner == "wine" {
        g.platform = "windows".into();
        g.launch.backend = "proton".into();
        g.launch.exe = yaml_str(&yml, &["game", "exe"]).unwrap_or_default();
        g.launch.prefix = yaml_str(&yml, &["game", "prefix"]).unwrap_or_default();
        g.launch.working_dir = yaml_str(&yml, &["game", "working_dir"]).unwrap_or_default();
        g.launch.arch = yaml_str(&yml, &["game", "arch"]).unwrap_or_else(|| "win64".into());
        if let Some(a) = yaml_str(&yml, &["game", "args"]) {
            g.launch.args = shell_words::split(&a).unwrap_or_else(|_| vec![a]);
        }
        g.launch.env = yaml_map(&yml, &["system", "env"]);
        g.launch.pre_command = yaml_str(&yml, &["system", "prelaunch_command"]).unwrap_or_default();
        g.launch.post_command = yaml_str(&yml, &["system", "postexit_command"]).unwrap_or_default();
        g.launch.esync = yaml_bool(&yml, &["wine", "esync"]);
        g.launch.fsync = yaml_bool(&yml, &["wine", "fsync"]);
        g.launch.dll_overrides = yaml_map(&yml, &["wine", "overrides"]);
        if let Some(v) = yaml_str(&yml, &["wine", "version"]) {
            if v == "system" {
                g.launch.backend = "wine".into();
            } else {
                g.launch.proton = v;
            }
        }
        if let Some(m) = yaml_bool(&yml, &["system", "mangohud"]) {
            g.launch.mangohud = Some(m);
        }
        if let Some(pc) = yaml_str(&yml, &["system", "prefix_command"]) {
            parked.insert("prefix_command".into(), toml::Value::String(pc));
        }
        if let Some(fps) = yaml_str(&yml, &["system", "fps_limit"]) {
            parked.insert("fps_limit".into(), toml::Value::String(fps));
        }
        let exe_dir = g.exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_default();
        if let Some(m) = read_gog_manifest(&exe_dir) {
            let confirmed = p.service == "gog" && (p.service_id.is_empty() || p.service_id == m.game_id);
            if confirmed {
                g.source.kind = "gog".into();
                g.source.gog_id = m.game_id.clone();
                g.source.build_id = m.build_id.clone();
            } else {
                g.source.kind = "lutris".into();
                parked.insert("gog_manifest".into(), toml::Value::String(m.game_id.clone()));
                parked.insert("gog_build".into(), toml::Value::String(m.build_id.clone()));
            }
            g.source.dir = exe_dir.to_string_lossy().into();
        } else {
            g.source.kind = "lutris".into();
            g.source.dir = exe_dir.to_string_lossy().into();
        }
    } else {
        g.platform = if p.platform.is_empty() { p.runner.clone() } else { p.platform.clone() };
        g.launch.runner = crate::runners::canonical(&p.runner);
        g.launch.arch = String::new();
        g.launch.exe = yaml_str(&yml, &["game", "main_file"]).unwrap_or_default();
        g.launch.working_dir = yaml_str(&yml, &["game", "working_dir"]).unwrap_or_default();
        g.launch.env = yaml_map(&yml, &["system", "env"]);
        g.source.kind = "lutris".into();
        if let Some(core) = yaml_str(&yml, &["game", "core"]) {
            parked.insert("core".into(), toml::Value::String(core));
        }
        if let Some(rexe) = yaml_str(&yml, &[&p.runner, "runner_executable"]) {
            parked.insert("runner_executable".into(), toml::Value::String(rexe));
        }
    }
    g.extra.insert("lutris".into(), toml::Value::Table(parked));
    Imported { game: g, lutris_env, playtime_h: p.playtime_h, lastplayed: p.lastplayed }
}

fn lutris_global_env(lutris_dir: &Path) -> BTreeMap<String, String> {
    let yml: serde_yaml::Value = std::fs::read_to_string(lutris_dir.join("system.yml")).ok().and_then(|s| serde_yaml::from_str(&s).ok()).unwrap_or(serde_yaml::Value::Null);
    yaml_map(&yml, &["system", "env"])
}

fn diff(id: &str, title: &str, lutris_env: &BTreeMap<String, String>, universe_env: &BTreeMap<String, String>) -> EnvDiff {
    let skip = ["WINEPREFIX", "PROTONPATH", "GAMEID", "STORE", "MANGOHUD"];
    let uni: BTreeMap<String, String> = universe_env.iter().filter(|(k, _)| !skip.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    let mut d = EnvDiff { id: id.into(), title: title.into(), lutris_env: lutris_env.clone(), universe_env: uni.clone(), ..Default::default() };
    for (k, v) in &uni {
        match lutris_env.get(k) {
            None => d.added.push(k.clone()),
            Some(lv) if lv != v => d.changed.push(k.clone()),
            _ => {}
        }
    }
    for k in lutris_env.keys() {
        if !uni.contains_key(k) {
            d.removed.push(k.clone());
        }
    }
    d
}

/// Imports installed Lutris games into games/<id>/game.toml without overwriting existing files; hours become one
/// `import-lutris` session covering what recordings do not (never negative). `apply=false` only reports.
pub fn import(config: &Config, apply: bool) -> crate::Result<Report> {
    let lutris_dir = paths::expand(&config.lutris.config_dir);
    let pga = paths::expand(&config.lutris.pga_db);
    if !pga.exists() {
        return Err(crate::Error::NotFound(format!("{}", pga.display())));
    }
    let global_env = lutris_global_env(&lutris_dir);
    let mut report = Report { applied: apply, ..Default::default() };
    let modules: Vec<crate::modules::Module> = vec![];
    let mut located = std::collections::HashMap::new();
    for p in read_pga(&pga)? {
        if p.name.trim().is_empty() {
            continue;
        }
        let imp = convert(&p, &lutris_dir, &global_env);
        let toml_path = imp.game.toml_path();
        let existed = toml_path.exists();
        let game = if existed {
            match Game::load(&toml_path) {
                Ok(g) => {
                    if g.launch.runner.is_empty() && g.launch.backend == "emulator" && !imp.game.launch.runner.is_empty() {
                        report.runners_promoted.push(g.id.clone());
                        if apply {
                            crate::game::set_key(&toml_path, "launch.runner", &imp.game.launch.runner)?;
                        }
                    }
                    report.skipped.push(g.id.clone());
                    Game::load(&toml_path).unwrap_or(g)
                }
                Err(_) => imp.game.clone(),
            }
        } else {
            report.imported.push(imp.game.id.clone());
            imp.game.clone()
        };
        let r = crate::library::resolve_with(game.clone(), config, &modules, &mut located);
        let mut env_for_diff = r.effective.env.clone();
        if r.effective.mangohud {
            env_for_diff.insert("MANGOHUD".into(), "1".into());
        }
        report.env_diffs.push(diff(&game.id, &game.title, &imp.lutris_env, &env_for_diff));
        if !apply {
            continue;
        }
        if !existed {
            game.save()?;
        }
        crate::recording::import_existing(&game, &config.recordings_root())?;
        if import_pegasus_media(&game.media_dir(), &game, &paths::expand(&config.lutris.pegasus_library))? {
            report.media_imported.push(game.id.clone());
        }
        let sessions = sessions::read(&game.sessions_path())?;
        if !sessions.iter().any(|s| s.source == "import-lutris") && imp.playtime_h > 0.0 {
            let covered: u64 = sessions.iter().filter(|s| s.source == "import-recording").map(|s| s.duration_s).sum();
            let total = (imp.playtime_h * 3600.0).round() as u64;
            let remainder = total.saturating_sub(covered);
            let ended_at = if imp.lastplayed > 0 {
                chrono::DateTime::from_timestamp(imp.lastplayed, 0).map(|d| d.with_timezone(&chrono::Local).to_rfc3339()).unwrap_or_default()
            } else {
                String::new()
            };
            sessions::append(&game.sessions_path(), &Session { session: "lutris".into(), game: game.id.clone(), started_at: String::new(), ended_at, duration_s: remainder, source: "import-lutris".into(), ..Default::default() })?;
            report.hours_imported.insert(game.id.clone(), (remainder as f64 / 36.0).round() / 100.0);
            if existed {
                report.updated.push(game.id.clone());
            }
        }
    }
    report.runners = runner_hints(&lutris_dir);
    if apply {
        for h in &report.runners {
            let spec = crate::runners::spec(&h.runner).expect("hints only name shipped runners");
            let located = crate::runners::locate(spec, config);
            let known = config.runners.get(spec.id);
            if located.program.is_empty() && known.and_then(|t| t.get("exe")).is_none() && Path::new(&h.program).is_file() {
                Config::set_key(&paths::config_file(), &format!("runners.{}.exe", spec.id), &h.program)?;
            }
            let fullscreen: Vec<&str> = spec.options.iter().filter(|o| o.key == "fullscreen").flat_map(|o| o.argument.split_whitespace()).collect();
            let args: Vec<String> = h.args.iter().filter(|a| !fullscreen.contains(&a.as_str())).cloned().collect();
            if !args.is_empty() && known.and_then(|t| t.get("args")).is_none() {
                Config::set_key(&paths::config_file(), &format!("runners.{}.args", spec.id), &shell_words::join(&args))?;
            }
        }
    }
    Ok(report)
}

/// Copies the art pegasus-sync fetched (`<root>/<platform>/media/<slug>/`) into `games/<id>/media/`, once: an
/// existing media dir is left alone. Only the slots the core reads; flat `screenshotNN.*` land in `screenshots/`.
fn import_pegasus_media(dest: &Path, game: &Game, root: &Path) -> crate::Result<bool> {
    if dest.exists() {
        return Ok(false);
    }
    let mut slugs = vec![game.id.as_str()];
    if !game.source.lutris_slug.is_empty() && game.source.lutris_slug != game.id {
        slugs.push(game.source.lutris_slug.as_str());
    }
    let Ok(platforms) = std::fs::read_dir(root) else { return Ok(false) };
    let Some(src) = platforms.flatten().map(|e| e.path().join("media")).flat_map(|m| slugs.iter().map(move |s| m.join(s)).collect::<Vec<_>>()).find(|p| p.is_dir()) else {
        return Ok(false);
    };
    let mut copied = false;
    for e in std::fs::read_dir(&src)?.flatten() {
        let p = e.path();
        let (Some(stem), Some(ext)) = (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|s| s.to_str())) else { continue };
        if !matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp") {
            continue;
        }
        let target = match stem {
            "boxFront" | "tile" | "background" | "logo" => dest.join(e.file_name()),
            s if s.starts_with("screenshot") => dest.join("screenshots").join(e.file_name()),
            _ => continue,
        };
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&p, &target)?;
        copied = true;
    }
    Ok(copied)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_wine_game_with_gog_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let gdir = dir.path().join("The Technomancer");
        std::fs::create_dir_all(gdir.join("games")).unwrap();
        std::fs::write(gdir.join("goggame-1972906591.info"), r#"{"buildId":"52654527801265271","gameId":"1972906591","rootGameId":"1972906591","name":"The Technomancer","playTasks":[{"isPrimary":true,"path":"TheTechnomancer.exe"}]}"#).unwrap();
        let lutris = dir.path().join("lutris");
        std::fs::create_dir_all(lutris.join("games")).unwrap();
        std::fs::write(lutris.join("games/the-technomancer-1.yml"), format!("game:\n  exe: {}/TheTechnomancer.exe\n  prefix: /mnt/games/gog/the-technomancer\nsystem:\n  env:\n    WINE_CPU_TOPOLOGY: 4:0,1,2,3\nwine:\n  version: proton-ge\n  esync: false\n", gdir.display())).unwrap();
        let p = PgaGame { name: "The Technomancer".into(), slug: "the-technomancer".into(), runner: "wine".into(), platform: "Windows".into(), hidden: false, playtime_h: 0.8, lastplayed: 0, service: "gog".into(), service_id: "1972906591".into(), configpath: "the-technomancer-1".into(), directory: String::new(), year: 2016 };
        let imp = convert(&p, &lutris, &BTreeMap::from([("PROTON_ENABLE_WAYLAND".to_string(), "1".to_string())]));
        let g = imp.game;
        assert_eq!(g.id, "the-technomancer");
        assert_eq!(g.source.kind, "gog");
        assert_eq!(g.source.gog_id, "1972906591");
        assert_eq!(g.source.build_id, "52654527801265271");
        assert_eq!(g.launch.proton, "proton-ge");
        assert_eq!(g.launch.esync, Some(false));
        assert_eq!(g.launch.env["WINE_CPU_TOPOLOGY"], "4:0,1,2,3");
        assert_eq!(imp.lutris_env["PROTON_ENABLE_WAYLAND"], "1");
        let text = toml::to_string_pretty(&g).unwrap();
        assert!(text.contains("[lutris]"));
        let d = diff("x", "X", &imp.lutris_env, &BTreeMap::from([("WINE_CPU_TOPOLOGY".to_string(), "4:0,1,2,3".to_string()), ("NEW".to_string(), "1".to_string())]));
        assert_eq!(d.added, vec!["NEW"]);
        assert_eq!(d.removed, vec!["PROTON_ENABLE_WAYLAND"]);
    }

    #[test]
    fn pegasus_media_lands_in_slots_and_screenshots() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("pegasus/windows/media/old-slug");
        std::fs::create_dir_all(&src).unwrap();
        for f in ["boxFront.png", "tile.jpg", "steam.png", "marquee.png", "screenshot01.png", "video.mp4"] {
            std::fs::write(src.join(f), b"x").unwrap();
        }
        let mut g = Game::new("New");
        g.source.lutris_slug = "old-slug".into();
        let media = dir.path().join("games/new/media");
        assert!(import_pegasus_media(&media, &g, &dir.path().join("pegasus")).unwrap());
        assert!(media.join("boxFront.png").exists());
        assert!(media.join("tile.jpg").exists());
        assert!(media.join("screenshots/screenshot01.png").exists());
        assert!(!media.join("steam.png").exists());
        assert!(!media.join("marquee.png").exists());
        assert!(!import_pegasus_media(&media, &g, &dir.path().join("pegasus")).unwrap());
    }

    #[test]
    fn convert_emulator_is_parked() {
        let dir = tempfile::tempdir().unwrap();
        let p = PgaGame { name: "F-Zero GX".into(), slug: "f-zero-gx".into(), runner: "dolphin".into(), platform: "Nintendo GameCube".into(), hidden: false, playtime_h: 4.3, lastplayed: 0, service: String::new(), service_id: String::new(), configpath: "none".into(), directory: String::new(), year: 0 };
        let g = convert(&p, dir.path(), &BTreeMap::new()).game;
        assert_eq!(g.launch.runner, "dolphin");
        assert!(g.launch.backend.is_empty());
        assert_eq!(g.platform, "Nintendo GameCube");
    }

    #[test]
    fn wrapper_scripts_are_seen_through() {
        let dir = tempfile::tempdir().unwrap();
        let w = dir.path().join("ryujinx-yasso");
        std::fs::write(&w, "#!/bin/bash\nexec /nix/store/x-emu-pad /nix/store/y/bin/Ryujinx --fullscreen --profile \"Yasso\" \"$@\"\n").unwrap();
        let (program, args, wrapped) = see_through(&w);
        assert_eq!(program, "/nix/store/y/bin/Ryujinx");
        assert_eq!(args, vec!["--fullscreen", "--profile", "Yasso"]);
        assert!(wrapped);
        let plain = dir.path().join("dolphin-emu");
        std::fs::write(&plain, b"\x7fELF").unwrap();
        assert_eq!(see_through(&plain), (plain.to_string_lossy().to_string(), vec![], false));
        let runners = dir.path().join("runners");
        std::fs::create_dir_all(&runners).unwrap();
        std::fs::write(runners.join("yuzu.yml"), format!("yuzu:\n  runner_executable: {}\n", w.display())).unwrap();
        std::fs::write(runners.join("wine.yml"), "wine:\n  version: x\n").unwrap();
        let hints = runner_hints(dir.path());
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].runner, "eden");
        assert_eq!(hints[0].lutris_runner, "yuzu");
        assert_eq!(hints[0].args, vec!["--fullscreen", "--profile", "Yasso"]);
    }
}
