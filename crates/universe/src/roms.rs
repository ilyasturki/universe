//! Games under the folders the emulators themselves are configured with.
use std::collections::{BTreeSet, HashSet};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::Config;
use crate::game::Game;
use crate::paths;
use crate::runners::{self, Kind, RunnerSpec};

/// Folder names that hold an emulator's patches, saves and system files rather than games, at any depth.
const NOT_GAMES: &[&str] = &["updates", "dlc", "mods", "firmware", "amiibo", "backups", "downloads", "prefixes", "saves", "textures", "shaders"];
/// Eden and the forks sharing its CLI keep `qt-config.ini` under their own name.
pub const EDEN_CONFIGS: &[&str] = &["eden", "citron", "sudachi", "suyu", "yuzu"];
/// Eden's and Azahar's virtual entries in `Paths\gamedirs`.
const VIRTUAL_DIRS: &[&str] = &["SDMC", "UserNAND", "SysNAND", "INSTALLED", "SYSTEM"];
/// A `.bin` / `.img` / `.raw` beside one of these is a track of it, not a game.
const CUE_SHEETS: &[&str] = &["cue", "gdi", "m3u"];
const TRACKS: &[&str] = &["bin", "img", "raw"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Folder {
    pub runner: String,
    pub dir: String,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Found {
    pub id: String,
    pub title: String,
    pub runner: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Skipped {
    pub path: String,
    pub reason: String,
}

/// `imported`: what `import` adds, or added; `skipped`: files it left out and why (a title already in the library under another file, a twin).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Report {
    pub folders: Vec<Folder>,
    pub imported: Vec<Found>,
    pub skipped: Vec<Skipped>,
    pub applied: bool,
}

/// The folders each installed emulator lists in its own configuration.
pub fn folders(config: &Config) -> Vec<Folder> {
    folders_under(config, &paths::xdg("XDG_CONFIG_HOME", ".config"), &paths::xdg("XDG_DATA_HOME", ".local/share"))
}

fn folders_under(config: &Config, cfg: &Path, data: &Path) -> Vec<Folder> {
    let mut out = Vec::new();
    for spec in runners::RUNNERS.iter().filter(|s| s.kind == Kind::Emulator) {
        if runners::locate(spec, config).program.is_empty() {
            continue;
        }
        let dirs: Vec<(PathBuf, bool)> = match spec.id {
            "eden" => qt_config(cfg, EDEN_CONFIGS).map(|p| qt_gamedirs(&p)).unwrap_or_default(),
            "azahar" => qt_config(cfg, &["azahar", "citra-emu"]).map(|p| qt_gamedirs(&p)).unwrap_or_default(),
            "dolphin" => dolphin_paths(&cfg.join("dolphin-emu/Dolphin.ini")),
            "ryujinx" => json_dirs(&cfg.join("Ryujinx/Config.json"), "game_dirs").into_iter().map(|d| (d, true)).collect(),
            "rpcs3" => rpcs3_dirs(&cfg.join("rpcs3")),
            "pcsx2" => ini_game_list(&cfg.join("PCSX2/inis/PCSX2.ini")),
            "duckstation" => {
                ini_game_list(&data.join("duckstation/settings.ini")).into_iter().chain(ini_game_list(&cfg.join("duckstation/settings.ini"))).collect()
            }
            "cemu" => xml_entries(&cfg.join("Cemu/settings.xml"), "GamePaths").into_iter().map(|d| (d, true)).collect(),
            "shadps4" => toml_dirs(&cfg.join("shadPS4/config.toml"), "GUI", "installDirs").into_iter().map(|d| (d, false)).collect(),
            "flycast" => ini_value(&cfg.join("flycast/emu.cfg"), "config", "Dreamcast.ContentPath")
                .map(|v| v.split(';').filter(|s| !s.trim().is_empty()).map(|s| (PathBuf::from(s.trim()), true)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let mut seen = BTreeSet::new();
        for (dir, recursive) in dirs {
            let dir = paths::expand(&dir.to_string_lossy());
            if dir.is_dir() && seen.insert(dir.clone()) {
                out.push(Folder { runner: spec.id.into(), dir: dir.to_string_lossy().into(), recursive });
            }
        }
    }
    out
}

/// The games under the folders that are not in the library yet: `existing` is every game's file. A file is offered
/// to the first runner whose folder holds it; a title already taken by another file is skipped, and so is a twin.
pub fn scan(config: &Config, existing: &[PathBuf]) -> Report {
    scan_under(config, existing, &paths::xdg("XDG_CONFIG_HOME", ".config"), &paths::xdg("XDG_DATA_HOME", ".local/share"))
}

fn scan_under(config: &Config, existing: &[PathBuf], cfg: &Path, data: &Path) -> Report {
    let folders = folders_under(config, cfg, data);
    let mut known: HashSet<FileKey> = existing.iter().filter_map(|p| FileKey::of(p)).collect();
    let mut report = Report { folders: folders.clone(), ..Default::default() };
    let mut titles: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for folder in &folders {
        let Some(spec) = runners::spec(&folder.runner) else { continue };
        let mut games = Vec::new();
        collect(spec, Path::new(&folder.dir), folder.recursive, &mut games);
        // A plain name first: the base game before its tagged update or patch of the same title.
        games.sort_by_cached_key(|p| (tagged(p), p.clone()));
        for path in games {
            let Some(key) = FileKey::of(&path) else { continue };
            if !known.insert(key) {
                continue;
            }
            let title = title_for(spec, &path);
            let g = Game::new(&title);
            let shown = path.to_string_lossy().to_string();
            if g.id.is_empty() || title.is_empty() {
                report.skipped.push(Skipped { path: shown, reason: "no title could be made from the name".into() });
            } else if g.toml_path().exists() {
                report.skipped.push(Skipped { path: shown, reason: format!("{} is already in the library under another file", g.id) });
            } else if let Some(first) = titles.get(&g.id) {
                report.skipped.push(Skipped { path: shown, reason: format!("same title as {first}") });
            } else {
                titles.insert(g.id.clone(), shown.clone());
                report.imported.push(Found { id: g.id, title, runner: spec.id.into(), path: shown });
            }
        }
    }
    report
}

/// Device and inode: the same file under a bind mount or a symlink counts once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FileKey(u64, u64);

impl FileKey {
    fn of(p: &Path) -> Option<FileKey> {
        let m = std::fs::metadata(p).ok()?;
        Some(FileKey(m.dev(), m.ino()))
    }
}

fn tagged(p: &Path) -> bool {
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    crate::core::title_of(p) != stem
}

fn skipped_dir(name: &str) -> bool {
    name.starts_with('.') || NOT_GAMES.contains(&name.to_ascii_lowercase().as_str())
}

fn extension(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// The game files of `spec` under `dir`: folder games for RPCS3, shadPS4 and Cemu's `code/*.rpx`, files by extension otherwise.
fn collect(spec: &RunnerSpec, dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    match spec.id {
        "rpcs3" => {
            if let Some(eboot) = ps3_eboot(dir) {
                out.push(eboot);
                return;
            }
            for sub in entries(dir).into_iter().filter(|p| p.is_dir()) {
                if let Some(eboot) = ps3_eboot(&sub) {
                    out.push(eboot);
                }
            }
        }
        "shadps4" => {
            for sub in entries(dir).into_iter().filter(|p| p.is_dir()) {
                let name = sub.file_name().map(|n| n.to_string_lossy().to_ascii_uppercase()).unwrap_or_default();
                let eboot = sub.join("eboot.bin");
                if !name.ends_with("-UPDATE") && !name.ends_with("-PATCH") && eboot.is_file() {
                    out.push(eboot);
                }
            }
        }
        _ => collect_files(spec, dir, recursive, out),
    }
}

fn collect_files(spec: &RunnerSpec, dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let items = entries(dir);
    let has_sheet = items.iter().any(|p| p.is_file() && CUE_SHEETS.contains(&extension(p).as_str()));
    for p in items {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if p.is_dir() {
            if recursive && !skipped_dir(&name) {
                collect_files(spec, &p, true, out);
            }
            continue;
        }
        let ext = extension(&p);
        if !spec.extensions.contains(&ext.as_str()) || (has_sheet && TRACKS.contains(&ext.as_str())) {
            continue;
        }
        // A Wii U title is its code/*.rpx; an .rpx elsewhere is a system title or a tool.
        if spec.id == "cemu" && ext == "rpx" && dir.file_name().is_none_or(|n| n != "code") {
            continue;
        }
        out.push(p);
    }
}

fn ps3_eboot(dir: &Path) -> Option<PathBuf> {
    [dir.join("PS3_GAME/USRDIR/EBOOT.BIN"), dir.join("USRDIR/EBOOT.BIN")].into_iter().find(|p| p.is_file())
}

fn title_for(spec: &RunnerSpec, path: &Path) -> String {
    let named = match spec.id {
        "rpcs3" => path.parent().and_then(|p| p.parent()).and_then(|p| {
            sfo_title(&p.join("PARAM.SFO")).or_else(|| {
                let root = if p.file_name().is_some_and(|n| n == "PS3_GAME") { p.parent()? } else { p };
                Some(crate::core::title_of(root))
            })
        }),
        "shadps4" => path.parent().and_then(|p| sfo_title(&p.join("sce_sys/param.sfo")).or_else(|| Some(crate::core::title_of(p)))),
        "cemu" if extension(path) == "rpx" => path.parent().and_then(|p| p.parent()).and_then(|title| {
            xml_text(&std::fs::read_to_string(title.join("meta/meta.xml")).unwrap_or_default(), "longname_en")
                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                .filter(|s| !s.is_empty())
                .or_else(|| Some(crate::core::title_of(title)))
        }),
        _ => None,
    };
    named.unwrap_or_else(|| crate::core::title_of(path))
}

/// `TITLE` of a PARAM.SFO (PS3 and PS4 alike): a key table, a data table and an index of `(key, format, len, max, data)` entries.
fn sfo_title(path: &Path) -> Option<String> {
    let b = std::fs::read(path).ok()?;
    let u32_at = |o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as usize);
    let u16_at = |o: usize| b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as usize);
    if b.get(0..4) != Some(b"\0PSF") {
        return None;
    }
    let (keys, data, n) = (u32_at(8)?, u32_at(12)?, u32_at(16)?);
    for i in 0..n.min(64) {
        let e = 20 + i * 16;
        let (key_off, len, data_off) = (u16_at(e)?, u32_at(e + 4)?, u32_at(e + 12)?);
        let key = b.get(keys + key_off..)?.split(|c| *c == 0).next()?;
        if key == b"TITLE" {
            let raw = b.get(data + data_off..data + data_off + len)?;
            let s = String::from_utf8_lossy(raw.split(|c| *c == 0).next()?);
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            return if s.is_empty() { None } else { Some(s) };
        }
    }
    None
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// A Qt `.ini`'s `[section]` `key=value` pairs, the values unquoted.
fn ini_pairs(text: &str) -> Vec<(String, String, String)> {
    let mut section = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.replace("%20", " ");
        } else if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
            out.push((section.clone(), k.trim().to_string(), v.to_string()));
        }
    }
    out
}

pub(crate) fn ini_value(path: &Path, section: &str, key: &str) -> Option<String> {
    ini_pairs(&read(path)).into_iter().find(|(s, k, _)| s == section && k == key).map(|(_, _, v)| v)
}

/// The first `<config home>/<name>/qt-config.ini` that exists.
pub(crate) fn qt_config(cfg: &Path, names: &[&str]) -> Option<PathBuf> {
    names.iter().map(|n| cfg.join(n).join("qt-config.ini")).find(|p| p.is_file())
}

/// yuzu's `[UI] Paths\gamedirs\N\path` with its `deep_scan`.
fn qt_gamedirs(path: &Path) -> Vec<(PathBuf, bool)> {
    let pairs = ini_pairs(&read(path));
    let mut out = Vec::new();
    for (s, k, v) in &pairs {
        if s != "UI" || !k.starts_with("Paths\\gamedirs\\") || !k.ends_with("\\path") || VIRTUAL_DIRS.contains(&v.as_str()) {
            continue;
        }
        let deep_key = k.replace("\\path", "\\deep_scan");
        let deep = pairs.iter().find(|(s, kk, _)| s == "UI" && *kk == deep_key).map(|(_, _, v)| v == "true").unwrap_or(false);
        out.push((PathBuf::from(v), deep));
    }
    out
}

/// Dolphin's `[General] ISOPathN`, all recursive when `RecursiveISOPaths` is.
fn dolphin_paths(path: &Path) -> Vec<(PathBuf, bool)> {
    let pairs = ini_pairs(&read(path));
    let recursive = pairs.iter().any(|(s, k, v)| s == "General" && k == "RecursiveISOPaths" && v.eq_ignore_ascii_case("true"));
    pairs
        .iter()
        .filter(|(s, k, _)| s == "General" && k.starts_with("ISOPath") && k[7..].chars().all(|c| c.is_ascii_digit()) && k.len() > 7)
        .map(|(_, _, v)| (PathBuf::from(v), recursive))
        .collect()
}

/// PCSX2's and DuckStation's `[GameList]`: `RecursivePaths` and `Paths`, one line per folder.
fn ini_game_list(path: &Path) -> Vec<(PathBuf, bool)> {
    ini_pairs(&read(path))
        .into_iter()
        .filter(|(s, k, _)| s == "GameList" && (k == "RecursivePaths" || k == "Paths"))
        .map(|(_, k, v)| (PathBuf::from(v), k == "RecursivePaths"))
        .collect()
}

fn json_dirs(path: &Path, key: &str) -> Vec<PathBuf> {
    serde_json::from_str::<serde_json::Value>(&read(path))
        .ok()
        .and_then(|v| v.get(key).and_then(|a| a.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect()
}

fn toml_dirs(path: &Path, table: &str, key: &str) -> Vec<PathBuf> {
    read(path)
        .parse::<toml::Table>()
        .ok()
        .and_then(|t| t.get(table)?.get(key)?.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect()
}

fn xml_text(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = text.find(&open)?;
    let body = &text[start + open.len()..];
    let body = &body[body.find('>')? + 1..];
    let end = body.find(&format!("</{tag}>"))?;
    Some(body[..end].replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&apos;", "'").replace("&quot;", "\""))
}

/// Cemu's `<GamePaths><Entry>…</Entry></GamePaths>`.
fn xml_entries(path: &Path, tag: &str) -> Vec<PathBuf> {
    let Some(block) = xml_text(&read(path), tag) else { return Vec::new() };
    let mut out = Vec::new();
    let mut rest = block.as_str();
    while let Some(entry) = xml_text(rest, "Entry") {
        out.push(PathBuf::from(entry.trim()));
        let Some(end) = rest.find("</Entry>") else { break };
        rest = &rest[end + "</Entry>".len()..];
    }
    out
}

/// RPCS3's `games.yml` (`SERIAL: folder`, disc games added to it) and the HDD games under `dev_hdd0/game`.
fn rpcs3_dirs(rpcs3: &Path) -> Vec<(PathBuf, bool)> {
    let mut out: Vec<(PathBuf, bool)> = read(&rpcs3.join("games.yml"))
        .lines()
        .filter_map(|l| l.split_once(": "))
        .map(|(_, dir)| (PathBuf::from(dir.trim().trim_end_matches('/')), false))
        .collect();
    let hdd = rpcs3.join("dev_hdd0/game");
    if hdd.is_dir() {
        out.push((hdd, false));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"").unwrap();
    }

    fn sfo(title: &str) -> Vec<u8> {
        let key = b"TITLE\0";
        let data = format!("{title}\0").into_bytes();
        let (keys_at, data_at) = (20 + 16, 20 + 16 + key.len());
        let mut b = Vec::new();
        b.extend_from_slice(b"\0PSF");
        b.extend_from_slice(&0x0101u32.to_le_bytes());
        b.extend_from_slice(&(keys_at as u32).to_le_bytes());
        b.extend_from_slice(&(data_at as u32).to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&0u16.to_le_bytes());
        b.extend_from_slice(&0x0204u16.to_le_bytes());
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(key);
        b.extend_from_slice(&data);
        b
    }

    #[test]
    fn qt_gamedirs_skip_the_virtual_ones_and_read_deep_scan() {
        let dir = tempfile::tempdir().unwrap();
        let ini = dir.path().join("qt-config.ini");
        std::fs::write(
            &ini,
            "[UI]\nPaths\\gamedirs\\1\\path=SDMC\nPaths\\gamedirs\\2\\path=/roms/switch\nPaths\\gamedirs\\2\\deep_scan=false\nPaths\\gamedirs\\3\\path=/roms/deep\nPaths\\gamedirs\\3\\deep_scan=true\n",
        )
        .unwrap();
        assert_eq!(qt_gamedirs(&ini), vec![(PathBuf::from("/roms/switch"), false), (PathBuf::from("/roms/deep"), true)]);
        assert_eq!(qt_config(dir.path(), &["missing"]), None);
    }

    #[test]
    fn dolphin_pcsx2_cemu_ryujinx_shadps4_lists() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::fs::write(d.join("Dolphin.ini"), "[General]\nISOPath0 = /roms/wii\nISOPaths = 2\nISOPath1 = /roms/gc\nRecursiveISOPaths = True\n").unwrap();
        assert_eq!(dolphin_paths(&d.join("Dolphin.ini")), vec![(PathBuf::from("/roms/wii"), true), (PathBuf::from("/roms/gc"), true)]);
        std::fs::write(d.join("PCSX2.ini"), "[GameList]\nRecursivePaths = /roms/ps2\nPaths = /roms/flat\n").unwrap();
        assert_eq!(ini_game_list(&d.join("PCSX2.ini")), vec![(PathBuf::from("/roms/ps2"), true), (PathBuf::from("/roms/flat"), false)]);
        std::fs::write(
            d.join("settings.xml"),
            "<content>\n  <GamePaths>\n    <Entry>/roms/wiiu</Entry>\n    <Entry>/roms/wiiu2</Entry>\n  </GamePaths>\n</content>",
        )
        .unwrap();
        assert_eq!(xml_entries(&d.join("settings.xml"), "GamePaths"), vec![PathBuf::from("/roms/wiiu"), PathBuf::from("/roms/wiiu2")]);
        std::fs::write(d.join("Config.json"), r#"{"game_dirs": ["/roms/switch"], "autoload_dirs": []}"#).unwrap();
        assert_eq!(json_dirs(&d.join("Config.json"), "game_dirs"), vec![PathBuf::from("/roms/switch")]);
        std::fs::write(d.join("config.toml"), "[GUI]\ninstallDirs = [\"/roms/ps4\"]\n").unwrap();
        assert_eq!(toml_dirs(&d.join("config.toml"), "GUI", "installDirs"), vec![PathBuf::from("/roms/ps4")]);
        std::fs::write(d.join("games.yml"), "BLES00142: /roms/ps3/Simpsons/\nBLES00416: /roms/ps3/Bolt/").unwrap();
        assert_eq!(rpcs3_dirs(d), vec![(PathBuf::from("/roms/ps3/Simpsons"), false), (PathBuf::from("/roms/ps3/Bolt"), false)]);
    }

    #[test]
    fn files_by_extension_skip_patch_folders_and_tracks() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        for f in ["Game A.xci", "updates/Game A [v1.1].nsp", "notes.txt", "sub/Game B.nsp", "sub/.hidden/Game C.nsp"] {
            touch(&d.join(f));
        }
        let mut out = Vec::new();
        collect(runners::spec("eden").unwrap(), d, true, &mut out);
        assert_eq!(out, vec![d.join("Game A.xci"), d.join("sub/Game B.nsp")]);
        let mut flat = Vec::new();
        collect(runners::spec("eden").unwrap(), d, false, &mut flat);
        assert_eq!(flat, vec![d.join("Game A.xci")]);
        for f in ["ps1/Crash.cue", "ps1/Crash (Track 1).bin", "ps1/Crash (Track 2).bin", "ps1/Spyro.chd"] {
            touch(&d.join(f));
        }
        let mut ps1 = Vec::new();
        collect(runners::spec("duckstation").unwrap(), &d.join("ps1"), false, &mut ps1);
        assert_eq!(ps1, vec![d.join("ps1/Crash.cue"), d.join("ps1/Spyro.chd")]);
    }

    #[test]
    fn folder_games_and_their_titles() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        touch(&d.join("ps3/Simpsons BLES00142/PS3_GAME/USRDIR/EBOOT.BIN"));
        std::fs::write(d.join("ps3/Simpsons BLES00142/PS3_GAME/PARAM.SFO"), sfo("The Simpsons Game")).unwrap();
        touch(&d.join("ps3/Bolt/PS3_GAME/USRDIR/EBOOT.BIN"));
        touch(&d.join("ps3/Bolt/Bolt.iso"));
        touch(&d.join("ps3/nothing/README"));
        let rpcs3 = runners::spec("rpcs3").unwrap();
        let mut out = Vec::new();
        collect(rpcs3, &d.join("ps3"), false, &mut out);
        assert_eq!(out, vec![d.join("ps3/Bolt/PS3_GAME/USRDIR/EBOOT.BIN"), d.join("ps3/Simpsons BLES00142/PS3_GAME/USRDIR/EBOOT.BIN")]);
        assert_eq!(title_for(rpcs3, &out[0]), "Bolt");
        assert_eq!(title_for(rpcs3, &out[1]), "The Simpsons Game");
        let mut one = Vec::new();
        collect(rpcs3, &d.join("ps3/Bolt"), false, &mut one);
        assert_eq!(one, vec![d.join("ps3/Bolt/PS3_GAME/USRDIR/EBOOT.BIN")], "a games.yml entry is the game itself");

        touch(&d.join("ps4/CUSA00001/eboot.bin"));
        touch(&d.join("ps4/CUSA00001/sce_sys/param.sfo"));
        std::fs::write(d.join("ps4/CUSA00001/sce_sys/param.sfo"), sfo("Bloodborne")).unwrap();
        touch(&d.join("ps4/CUSA00001-UPDATE/eboot.bin"));
        let shadps4 = runners::spec("shadps4").unwrap();
        let mut ps4 = Vec::new();
        collect(shadps4, &d.join("ps4"), false, &mut ps4);
        assert_eq!(ps4, vec![d.join("ps4/CUSA00001/eboot.bin")]);
        assert_eq!(title_for(shadps4, &ps4[0]), "Bloodborne");

        touch(&d.join("wiiu/Wind Waker HD [ARZP01]/code/Turbo.rpx"));
        std::fs::create_dir_all(d.join("wiiu/Wind Waker HD [ARZP01]/meta")).unwrap();
        std::fs::write(
            d.join("wiiu/Wind Waker HD [ARZP01]/meta/meta.xml"),
            "<menu>\n<longname_en type=\"string\">The Legend of Zelda\nThe Wind Waker HD</longname_en>\n</menu>",
        )
        .unwrap();
        touch(&d.join("wiiu/tool/other.rpx"));
        touch(&d.join("wiiu/Bayonetta.wud"));
        let cemu = runners::spec("cemu").unwrap();
        let mut wiiu = Vec::new();
        collect(cemu, &d.join("wiiu"), true, &mut wiiu);
        assert_eq!(wiiu, vec![d.join("wiiu/Bayonetta.wud"), d.join("wiiu/Wind Waker HD [ARZP01]/code/Turbo.rpx")]);
        assert_eq!(title_for(cemu, &wiiu[1]), "The Legend of Zelda The Wind Waker HD");
        assert_eq!(title_for(cemu, &wiiu[0]), "Bayonetta");
    }

    #[test]
    fn scan_offers_a_file_once_and_keeps_the_library_out() {
        let _guard = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::env::set_var("UNIVERSE_DATA_HOME", d.join("universe"));
        let emu = d.join("bin/emu");
        touch(&emu);
        std::fs::create_dir_all(d.join("cfg/eden")).unwrap();
        std::fs::create_dir_all(d.join("cfg/Ryujinx")).unwrap();
        std::fs::write(
            d.join("cfg/eden/qt-config.ini"),
            format!("[UI]\nPaths\\gamedirs\\1\\path={}\nPaths\\gamedirs\\1\\deep_scan=false\n", d.join("switch").display()),
        )
        .unwrap();
        std::fs::write(d.join("cfg/Ryujinx/Config.json"), format!(r#"{{"game_dirs": ["{}"]}}"#, d.join("switch").display())).unwrap();
        for f in ["switch/Zelda.xci", "switch/Zelda [v2].nsp", "switch/Owned.xci", "switch/Mario.nsp", "switch/updates/Mario [v1].nsp"] {
            touch(&d.join(f));
        }
        let mut config = Config::default();
        for id in ["eden", "ryujinx"] {
            config.runners.insert(id.into(), [("exe".to_string(), toml::Value::String(emu.to_string_lossy().into()))].into_iter().collect());
        }
        let folders = folders_under(&config, &d.join("cfg"), &d.join("data"));
        assert_eq!(folders.iter().map(|f| (f.runner.as_str(), f.recursive)).collect::<Vec<_>>(), vec![("eden", false), ("ryujinx", true)]);
        let mut owned = Game::new("Owned");
        owned.launch.runner = "eden".into();
        owned.launch.exe = d.join("switch/Owned.xci").to_string_lossy().into();
        owned.save().unwrap();
        let mut taken = Game::new("Mario");
        taken.launch.exe = "/elsewhere/Mario.nsp".into();
        taken.save().unwrap();
        let report = scan_under(&config, &[d.join("switch/Owned.xci")], &d.join("cfg"), &d.join("data"));
        assert_eq!(report.imported.iter().map(|f| (f.id.as_str(), f.runner.as_str())).collect::<Vec<_>>(), vec![("zelda", "eden")], "{:?}", report.imported);
        assert_eq!(
            report.skipped.iter().map(|s| s.reason.as_str()).collect::<Vec<_>>(),
            vec!["mario is already in the library under another file", &format!("same title as {}", d.join("switch/Zelda.xci").display())]
        );
        std::env::remove_var("UNIVERSE_DATA_HOME");
    }

    #[test]
    fn sfo_without_a_title_reads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.sfo"), b"not an sfo").unwrap();
        assert_eq!(sfo_title(&dir.path().join("x.sfo")), None);
        assert_eq!(sfo_title(&dir.path().join("missing.sfo")), None);
    }
}
