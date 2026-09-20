use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::paths;

/// One launcher's games on this machine. `importable` names what Universe can launch itself; `via` is the importer (`lutris`, `gog`).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Launcher {
    pub id: String,
    pub name: String,
    pub found: bool,
    pub dir: String,
    pub games: usize,
    pub titles: Vec<String>,
    pub importable: bool,
    pub via: String,
    pub detail: String,
}

/// `gog_dirs`: folders holding GOG installs made by another launcher, for the gog source's `scan_dirs`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub launchers: Vec<Launcher>,
    pub gog_dirs: Vec<String>,
}

const TITLES: usize = 6;
const STEAM_TOOLS: &[&str] = &["Proton", "Steam Linux Runtime", "Steamworks Common Redistributables"];

pub fn run(config: &Config) -> Report {
    let home = paths::home();
    let heroic = first_dir(&[home.join(".config/heroic"), home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")]);
    let steam = first_dir(&[home.join(".local/share/Steam"), home.join(".steam/steam"), home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam")]);
    let mut gog_dirs = heroic.as_deref().map(heroic_gog_dirs).unwrap_or_default();
    gog_dirs.retain(|d| d != &config.games_root());
    let mut launchers = vec![lutris(config)];
    launchers.push(steam.as_deref().map(steam_launcher).unwrap_or_else(|| Launcher { id: "steam".into(), name: "Steam".into(), ..Default::default() }));
    launchers.extend(heroic_launchers(heroic.as_deref(), &gog_dirs, &config.games_root()));
    launchers.push(roms_launcher(config, library_files()));
    Report { launchers, gog_dirs: gog_dirs.iter().map(|d| d.to_string_lossy().into()).collect() }
}

/// Every library game's file, for the folder scan to leave out.
fn library_files() -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(paths::games_dir()) else { return Vec::new() };
    rd.flatten()
        .filter_map(|e| crate::game::Game::load(&e.path().join("game.toml")).ok())
        .filter(|g| !g.launch.exe.is_empty())
        .map(|g| paths::expand(&g.launch.exe))
        .collect()
}

/// The folders the installed emulators list themselves (`roms.rs`): `games` counts the files not in the library yet.
fn roms_launcher(config: &Config, existing: Vec<PathBuf>) -> Launcher {
    let report = crate::roms::scan(config, &existing);
    let mut seen = std::collections::BTreeSet::new();
    let dirs: Vec<&str> = report.folders.iter().map(|f| f.dir.as_str()).filter(|d| seen.insert(*d)).collect();
    Launcher {
        id: "roms".into(),
        name: "Emulator folders".into(),
        found: !report.folders.is_empty(),
        dir: dirs.join(", "),
        games: report.imported.len(),
        titles: report.imported.iter().take(TITLES).map(|f| f.title.clone()).collect(),
        importable: true,
        via: "roms".into(),
        detail: if report.folders.is_empty() { "No installed emulator lists a game folder yet.".into() } else { String::new() },
    }
}

fn first_dir(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_dir()).cloned()
}

fn lutris(config: &Config) -> Launcher {
    let pga = paths::expand(&config.lutris.pga_db);
    let mut l = Launcher {
        id: "lutris".into(),
        name: "Lutris".into(),
        found: pga.is_file(),
        dir: pga.to_string_lossy().into(),
        importable: true,
        via: "lutris".into(),
        ..Default::default()
    };
    if !l.found {
        return l;
    }
    match crate::lutris::pending(config) {
        Ok(mut titles) => {
            titles.sort();
            l.games = titles.len();
            l.titles = titles.into_iter().take(TITLES).collect();
        }
        Err(e) => l.detail = e.to_string(),
    }
    l
}

fn steam_launcher(root: &Path) -> Launcher {
    let mut libraries = vec![root.to_path_buf()];
    libraries.extend(vdf_paths(&root.join("steamapps/libraryfolders.vdf")));
    libraries.dedup();
    let mut titles = Vec::new();
    for lib in libraries {
        let Ok(rd) = std::fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Some(title) = std::fs::read_to_string(e.path()).ok().and_then(|s| vdf_value(&s, "name")) else { continue };
            if !STEAM_TOOLS.iter().any(|t| title.starts_with(t)) {
                titles.push(title);
            }
        }
    }
    titles.sort();
    Launcher {
        id: "steam".into(),
        name: "Steam".into(),
        found: true,
        dir: root.to_string_lossy().into(),
        games: titles.len(),
        titles: titles.into_iter().take(TITLES).collect(),
        detail: "Steam games launch through Steam; a Steam source comes later.".into(),
        ..Default::default()
    }
}

/// The quoted tokens of a VDF document, quotes and escapes dropped, braces skipped.
fn vdf_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut tok = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(n) = chars.next() {
                        tok.push(n);
                    }
                }
                c => tok.push(c),
            }
        }
        out.push(tok);
    }
    out
}

fn vdf_value(text: &str, key: &str) -> Option<String> {
    let tokens = vdf_tokens(text);
    tokens.iter().position(|t| t.eq_ignore_ascii_case(key)).and_then(|i| tokens.get(i + 1)).cloned()
}

fn vdf_paths(file: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    let tokens = vdf_tokens(&text);
    tokens.windows(2).filter(|w| w[0] == "path").map(|w| PathBuf::from(&w[1])).collect()
}

fn json_file(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Heroic's GOG install roots: the parents of `gog_store/installed.json`'s paths and its default install folder.
fn heroic_gog_dirs(heroic: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(v) = json_file(&heroic.join("gog_store/installed.json")) {
        let installed = v.get("installed").and_then(|i| i.as_array()).cloned().or_else(|| v.as_array().cloned()).unwrap_or_default();
        for g in installed {
            if let Some(parent) = g.get("install_path").and_then(|p| p.as_str()).and_then(|p| Path::new(p).parent()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    if let Some(p) = json_file(&heroic.join("config.json")).and_then(|v| v["defaultSettings"]["defaultInstallPath"].as_str().map(PathBuf::from)) {
        dirs.push(p);
    }
    dirs.retain(|d| d.is_dir());
    let mut seen = std::collections::BTreeSet::new();
    dirs.retain(|d| seen.insert(d.clone()));
    dirs
}

fn gog_installs(dirs: &[&Path]) -> Vec<String> {
    let mut titles = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for game in rd.flatten().filter(|e| e.path().is_dir()) {
            let Ok(files) = std::fs::read_dir(game.path()) else { continue };
            let infos: Vec<serde_json::Value> = files
                .flatten()
                .filter(|f| {
                    let n = f.file_name().to_string_lossy().to_string();
                    n.starts_with("goggame-") && n.ends_with(".info")
                })
                .filter_map(|f| json_file(&f.path()))
                .collect();
            // DLCs carry their own info file beside the game's; the base game is the one whose gameId is its rootGameId.
            let base = infos.iter().find(|i| i["gameId"] == i["rootGameId"]).or(infos.first());
            if let Some(info) = base {
                titles.push(info["name"].as_str().map(String::from).unwrap_or_else(|| game.file_name().to_string_lossy().into()));
            }
        }
    }
    titles.sort();
    titles.dedup();
    titles
}

fn heroic_launchers(heroic: Option<&Path>, gog_dirs: &[PathBuf], games_root: &Path) -> Vec<Launcher> {
    let found = heroic.is_some();
    let dir = heroic.map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let scan: Vec<&Path> = gog_dirs.iter().map(|p| p.as_path()).chain(std::iter::once(games_root)).collect();
    let gog_titles = if found { gog_installs(&scan) } else { Vec::new() };
    let mut out = vec![Launcher {
        id: "heroic-gog".into(),
        name: "Heroic · GOG".into(),
        found,
        dir: dir.clone(),
        games: gog_titles.len(),
        titles: gog_titles.into_iter().take(TITLES).collect(),
        importable: true,
        via: "gog".into(),
        ..Default::default()
    }];
    let stores = [
        ("heroic-epic", "Heroic · Epic Games", "legendaryConfig/legendary/installed.json", "Epic games launch through Heroic; an Epic source comes later."),
        ("heroic-amazon", "Heroic · Amazon Games", "nile_store/installed.json", "Amazon games launch through Heroic; an Amazon source comes later."),
    ];
    for (id, name, file, detail) in stores {
        let mut titles: Vec<String> = heroic
            .and_then(|h| json_file(&h.join(file)))
            .map(|v| match v {
                serde_json::Value::Object(m) => m.values().filter_map(|g| g["title"].as_str().map(String::from)).collect(),
                serde_json::Value::Array(a) => a.iter().filter_map(|g| g["title"].as_str().map(String::from)).collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        titles.sort();
        out.push(Launcher {
            id: id.into(),
            name: name.into(),
            found,
            dir: dir.clone(),
            games: titles.len(),
            titles: titles.into_iter().take(TITLES).collect(),
            detail: detail.into(),
            ..Default::default()
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_manifests_without_tools() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Steam");
        let other = dir.path().join("Library");
        for lib in [&root, &other] {
            std::fs::create_dir_all(lib.join("steamapps")).unwrap();
        }
        std::fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
                root.display(),
                other.display()
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("steamapps/appmanifest_1493710.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"1493710\"\n\t\"name\"\t\t\"Proton Experimental\"\n}\n",
        )
        .unwrap();
        std::fs::write(root.join("steamapps/appmanifest_620.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n}\n").unwrap();
        std::fs::write(other.join("steamapps/appmanifest_413150.acf"), "\"AppState\"\n{\n\t\"name\"\t\t\"Stardew \\\"Valley\\\"\"\n}\n").unwrap();
        let l = steam_launcher(&root);
        assert!(l.found && !l.importable);
        assert_eq!(l.titles, vec!["Portal 2", "Stardew \"Valley\""]);
        assert_eq!(l.games, 2);
    }

    #[test]
    fn heroic_stores_and_gog_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let heroic = dir.path().join("heroic");
        let games = dir.path().join("games");
        std::fs::create_dir_all(heroic.join("gog_store")).unwrap();
        std::fs::create_dir_all(heroic.join("legendaryConfig/legendary")).unwrap();
        std::fs::create_dir_all(games.join("Mini Metro")).unwrap();
        std::fs::write(games.join("Mini Metro/goggame-1434554947.info"), r#"{"gameId":"1434554947","name":"Mini Metro"}"#).unwrap();
        std::fs::create_dir_all(games.join("Notes")).unwrap();
        std::fs::write(
            heroic.join("gog_store/installed.json"),
            format!(r#"{{"installed":[{{"appName":"1434554947","install_path":"{}/Mini Metro","platform":"windows"}}]}}"#, games.display()),
        )
        .unwrap();
        std::fs::write(heroic.join("config.json"), format!(r#"{{"defaultSettings":{{"defaultInstallPath":"{}"}}}}"#, games.display())).unwrap();
        std::fs::write(heroic.join("legendaryConfig/legendary/installed.json"), r#"{"abc":{"app_name":"abc","title":"Hades","install_path":"/x/Hades"}}"#)
            .unwrap();
        let dirs = heroic_gog_dirs(&heroic);
        assert_eq!(dirs, vec![games.clone()]);
        let l = heroic_launchers(Some(&heroic), &dirs, &dir.path().join("none"));
        assert_eq!(
            l.iter().map(|l| (l.id.as_str(), l.games, l.importable)).collect::<Vec<_>>(),
            vec![("heroic-gog", 1, true), ("heroic-epic", 1, false), ("heroic-amazon", 0, false)]
        );
        assert_eq!(l[0].titles, vec!["Mini Metro"]);
        assert_eq!(l[1].titles, vec!["Hades"]);
    }

    #[test]
    fn nothing_installed() {
        let l = heroic_launchers(None, &[], Path::new("/nonexistent"));
        assert!(l.iter().all(|l| !l.found && l.games == 0));
        assert!(vdf_paths(Path::new("/nonexistent/libraryfolders.vdf")).is_empty());
    }
}
