use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};

use crate::config::Config;
use crate::game::Game;
use crate::paths;
use crate::runners::{self, Kind};
use crate::{Error, Result};

pub const BIN: &str = "ludusavi";
/// Where a backup records the prefix's files: the same path wherever the prefix sits, so a backup outlives a move or a reset.
const PREFIX_IN_BACKUPS: &str = "/universe/prefix";
// A first run downloads ludusavi's manifest (17 MB) before it scans.
const TIMEOUT: Duration = Duration::from_secs(600);

/// ludusavi's config, manifest and cache, apart from the player's own ludusavi's.
pub fn config_dir() -> PathBuf {
    paths::data_home().join("ludusavi")
}

pub fn dir(config: &Config, id: &str) -> PathBuf {
    config.saves_root().join(id)
}

/// The Wine prefix in a game's prefix folder: Steam's compatdata keeps it under `pfx/`.
pub fn wine_prefix(prefix: &Path) -> PathBuf {
    let pfx = prefix.join("pfx");
    if pfx.join("drive_c").is_dir() {
        pfx
    } else {
        prefix.to_path_buf()
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct EmuSaves {
    pub folder: String,
    pub title_id: String,
    /// The title's own saves as ludusavi globs, never a key, firmware, BIOS or system title; empty when only the folder is known.
    pub files: Vec<String>,
}

pub fn emulator(game: &Game, config: &Config) -> Option<EmuSaves> {
    emulator_under(game, config, &paths::xdg("XDG_CONFIG_HOME", ".config"), &paths::xdg("XDG_DATA_HOME", ".local/share"))
}

/// A literal path in a glob: ludusavi reads its paths as patterns.
fn literal(p: &Path) -> String {
    p.to_string_lossy().chars().map(|c| if matches!(c, '[' | ']' | '*' | '?' | '{' | '}') { format!("[{c}]") } else { c.to_string() }).collect()
}

fn emulator_under(game: &Game, config: &Config, cfg: &Path, data: &Path) -> Option<EmuSaves> {
    let spec = runners::spec(&game.runner_id())?;
    if spec.kind != Kind::Emulator || spec.via_proton {
        return None;
    }
    let exe = game.exe_path();
    let options = spec.merged_options(config, Some(game));
    let option = |k: &str| options.get(k).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(paths::expand);
    let folder_only = |folder: PathBuf| (folder, String::new(), Vec::new());
    let (folder, title_id, files): (PathBuf, String, Vec<String>) = match spec.id {
        "rpcs3" => {
            let home = cfg.join("rpcs3/dev_hdd0/home");
            let id = exe.parent().and_then(Path::parent).and_then(|d| crate::roms::sfo_value(&d.join("PARAM.SFO"), "TITLE_ID")).unwrap_or_default();
            let files = if id.is_empty() { vec![] } else { vec![format!("{}/*/savedata/{}*", literal(&home), literal(Path::new(&id)))] };
            (home.join("00000001/savedata"), id, files)
        }
        "shadps4" => {
            let root = data.join("shadPS4");
            let id = exe.parent().and_then(|d| crate::roms::sfo_value(&d.join("sce_sys/param.sfo"), "TITLE_ID")).unwrap_or_default();
            let folder = [root.join("home/1000/savedata"), root.join("user/savedata")]
                .into_iter()
                .find(|p| p.is_dir())
                .unwrap_or_else(|| root.join("home/1000/savedata"));
            let files = if id.is_empty() {
                vec![]
            } else {
                let (root, id) = (literal(&root), literal(Path::new(&id)));
                vec![format!("{root}/home/*/savedata/{id}"), format!("{root}/user/savedata/*/{id}")]
            };
            (folder, id, files)
        }
        "vita3k" => {
            let pref = crate::roms::read(&cfg.join("Vita3K/config.yml"))
                .lines()
                .find_map(|l| l.strip_prefix("pref-path:").map(|v| v.trim().trim_matches(['"', '\'']).to_string()))
                .filter(|v| !v.is_empty())
                .map(|v| paths::expand(&v))
                .unwrap_or_else(|| data.join("Vita3K/Vita3K"));
            let folder = pref.join("ux0/user/00/savedata");
            let id = game.launch.exe.trim().to_string();
            let is_id = id.len() == 9 && id[..4].chars().all(|c| c.is_ascii_uppercase()) && id[4..].chars().all(|c| c.is_ascii_digit());
            let files = if is_id { vec![literal(&folder.join(&id))] } else { vec![] };
            (folder, if is_id { id } else { String::new() }, files)
        }
        "cemu" => {
            let mlc = option("mlc")
                .or_else(|| {
                    crate::roms::xml_text(&crate::roms::read(&cfg.join("Cemu/settings.xml")), "mlc_path")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                })
                .unwrap_or_else(|| data.join("Cemu/mlc01"));
            let folder = mlc.join("usr/save");
            let meta = exe.parent().and_then(Path::parent).map(|t| crate::roms::read(&t.join("meta/meta.xml"))).unwrap_or_default();
            let id = crate::roms::xml_text(&meta, "title_id")
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit()))
                .unwrap_or_default();
            let files = if id.is_empty() { vec![] } else { vec![literal(&folder.join(&id[..8]).join(&id[8..]))] };
            (folder, id, files)
        }
        "dolphin" => {
            let user = option("user_directory").unwrap_or_else(|| data.join("dolphin-emu"));
            match disc_id(&exe) {
                Some(disc) if disc.wii => {
                    let folder = user.join("Wii/title/00010000");
                    let hex: String = disc.code.bytes().map(|b| format!("{b:02x}")).collect();
                    let files = vec![literal(&folder.join(hex).join("data"))];
                    (folder, disc.code, files)
                }
                Some(disc) => {
                    let folder = user.join("GC");
                    let files = vec![format!("{}/*/Card */{}-{}-*.gci", literal(&folder), literal(Path::new(&disc.maker)), literal(Path::new(&disc.code)))];
                    (folder, disc.code, files)
                }
                None if game.platform.contains("Wii") => folder_only(user.join("Wii/title/00010000")),
                None => folder_only(user.join("GC")),
            }
        }
        // They write a `.sav` beside the ROM.
        "melonds" | "mgba" => (exe.parent().map(Path::to_path_buf).unwrap_or_default(), String::new(), vec![literal(&exe.with_extension("sav"))]),
        "eden" => folder_only(
            crate::roms::EDEN_CONFIGS
                .iter()
                .map(|n| data.join(n).join("nand/user/save"))
                .find(|p| p.is_dir())
                .unwrap_or_else(|| data.join("eden/nand/user/save")),
        ),
        "ryujinx" => folder_only(cfg.join("Ryujinx/bis/user/save")),
        "pcsx2" => folder_only(cfg.join("PCSX2/memcards")),
        "duckstation" => folder_only(data.join("duckstation/memcards")),
        "ppsspp" => folder_only(cfg.join("ppsspp/PSP/SAVEDATA")),
        "azahar" => folder_only(data.join("azahar-emu/sdmc/Nintendo 3DS")),
        "mupen64plus" => folder_only(data.join("mupen64plus/save")),
        "scummvm" => folder_only(data.join("scummvm/saves")),
        "xemu" => folder_only(data.join("xemu/xemu")),
        _ => return None,
    };
    Some(EmuSaves { folder: folder.to_string_lossy().into(), title_id, files })
}

struct Disc {
    code: String,
    maker: String,
    wii: bool,
}

fn disc_id(path: &Path) -> Option<Disc> {
    use std::io::{Read, Seek, SeekFrom};
    let at = match path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).as_deref() {
        Some("iso" | "gcm") => 0,
        Some("ciso") => 0x8000,
        // A WIA or RVZ file keeps the disc's header whole in its own, after 0x48 bytes of file header and 16 of disc fields.
        Some("wia" | "rvz") => 0x58,
        _ => return None,
    };
    let mut head = [0u8; 0x20];
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(at)).ok()?;
    f.read_exact(&mut head).ok()?;
    let word = |o: usize| u32::from_be_bytes([head[o], head[o + 1], head[o + 2], head[o + 3]]);
    let wii = word(0x18) == 0x5D1C_9EA3;
    if !wii && word(0x1C) != 0xC233_9F3D {
        return None;
    }
    let text = |b: &[u8]| b.iter().all(u8::is_ascii_alphanumeric).then(|| String::from_utf8_lossy(b).into_owned());
    Some(Disc { code: text(&head[0..4])?, maker: text(&head[4..6])?, wii })
}

#[derive(Debug, Clone, Default)]
pub struct Ask {
    pub bin: PathBuf,
    pub name: String,
    pub prefix: Option<PathBuf>,
    /// A custom game's files, an emulated title's saves: ludusavi's manifest knows none.
    pub files: Vec<String>,
    /// The install folder's parent: ludusavi finds `<base>` saves under a root.
    pub root: Option<PathBuf>,
}

pub fn backs_up(game: &Game, config: &Config) -> bool {
    match runners::spec(&game.runner_id()) {
        Some(spec) if spec.kind == Kind::Emulator && !spec.via_proton => emulator(game, config).is_some_and(|e| !e.files.is_empty()),
        Some(_) => true,
        None => false,
    }
}

pub fn installed() -> Option<PathBuf> {
    runners::on_path(BIN)
}

pub async fn program() -> Result<PathBuf> {
    if let Some(bin) = installed() {
        return Ok(bin);
    }
    // The fetch's future is not Send (its progress callback): on a thread and a runtime of its own, a caller's future still is.
    crate::core::blocking(|| tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(crate::tools::ensure(BIN))).await?;
    runners::on_path(BIN).ok_or_else(|| Error::Unavailable("ludusavi is not installed and Universe has no build of it to fetch".into()))
}

fn command(bin: &Path) -> tokio::process::Command {
    // A fetched ludusavi is built for a usual distro: on NixOS it runs in Universe's FHS env, as a fetched emulator does.
    let real = std::fs::canonicalize(bin).unwrap_or_else(|_| bin.to_path_buf());
    match real.starts_with(crate::components::root()).then(crate::components::fhs).flatten() {
        Some(fhs) => {
            let mut cmd = tokio::process::Command::new(fhs);
            cmd.arg(real);
            cmd
        }
        None => tokio::process::Command::new(bin),
    }
}

fn config_yaml(ask: Option<&Ask>, dir: &Path, keep: u32) -> Value {
    let ask = ask.cloned().unwrap_or_default();
    let roots: Vec<Value> = ask.root.iter().map(|r| json!({"store": "other", "path": r})).collect();
    let redirects: Vec<Value> = ask.prefix.iter().map(|p| json!({"kind": "bidirectional", "source": p, "target": PREFIX_IN_BACKUPS})).collect();
    let custom: Vec<Value> = if ask.files.is_empty() { vec![] } else { vec![json!({"name": ask.name, "files": ask.files, "integration": "override"})] };
    // JSON is YAML: the file is written whole on every call, so nothing a player set in ludusavi's own GUI leaks in.
    json!({
        "manifest": {"enable": true},
        "release": {"check": false},
        "roots": roots,
        "redirects": redirects,
        "backup": {"path": dir, "retention": {"full": keep.clamp(1, 255), "differential": 0}, "format": {"chosen": "simple"}},
        "restore": {"path": dir},
        "customGames": custom,
    })
}

/// The config is rewritten for each call, so calls from every process wait on one lock.
async fn run(bin: &Path, ask: Option<&Ask>, dir: &Path, keep: u32, args: Vec<std::ffi::OsString>) -> Result<Value> {
    let cfg_dir = config_dir();
    std::fs::create_dir_all(&cfg_dir)?;
    let lock = std::fs::File::create(cfg_dir.join("universe.lock"))?;
    let lock = crate::core::blocking(move || {
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive).map_err(|e| Error::Io(format!("ludusavi lock: {e}")))?;
        Ok(lock)
    })
    .await?;
    std::fs::write(cfg_dir.join("config.yaml"), serde_json::to_vec_pretty(&config_yaml(ask, dir, keep))?)?;
    let mut cmd = command(bin);
    cmd.arg("--config")
        .arg(&cfg_dir)
        .arg("--try-manifest-update")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let out = tokio::time::timeout(TIMEOUT, cmd.output()).await.map_err(|_| Error::Io("ludusavi did not finish in 10 minutes".into()))??;
    drop(lock);
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    if let Some(unknown) = reply["errors"]["unknownGames"].as_array().filter(|u| !u.is_empty()) {
        let names: Vec<&str> = unknown.iter().filter_map(Value::as_str).collect();
        return Err(Error::NotFound(format!("ludusavi knows no game named {}", names.join(", "))));
    }
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
        return Err(Error::Io(format!("ludusavi failed ({}): {why}", out.status)));
    }
    Ok(reply)
}

/// ludusavi's precedence: the Steam id, the GOG id, then the title give or take an edition.
pub async fn find(bin: &Path, game: &Game) -> Result<Option<String>> {
    let mut args: Vec<std::ffi::OsString> = vec!["find".into(), "--api".into()];
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let steam = if game.metadata.steam_appid > 0 {
        game.metadata.steam_appid.to_string()
    } else if game.source.kind == "steam" && numeric(&game.source.id) {
        game.source.id.clone()
    } else {
        String::new()
    };
    if !steam.is_empty() {
        args.extend(["--steam-id".into(), steam.into()]);
    }
    if game.source.kind == "gog" && numeric(&game.source.id) {
        args.extend(["--gog-id".into(), game.source.id.clone().into()]);
    }
    args.extend(["--normalized".into(), game.title.clone().into()]);
    match run(bin, None, &config_dir(), 1, args).await {
        Ok(reply) => Ok(reply["games"].as_object().and_then(|g| g.keys().next().cloned())),
        Err(Error::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The first call writes the title found to game.toml's `saves.name`.
/// `fetch` false asks only an installed ludusavi: a read never downloads it.
pub async fn ask(game: &Game, config: &Config, fetch: bool) -> Result<Ask> {
    let bin = if fetch {
        program().await?
    } else {
        installed().ok_or_else(|| Error::Unavailable("ludusavi is not installed yet: the first backup fetches it".into()))?
    };
    let spec = runners::spec(&game.runner_id()).ok_or_else(|| Error::Invalid(format!("{} has no known runner", game.id)))?;
    if spec.kind == Kind::Emulator && !spec.via_proton {
        let emu = emulator(game, config).unwrap_or_default();
        if emu.files.is_empty() {
            let folder = if emu.folder.is_empty() { String::new() } else { format!(": every game's are in {}", emu.folder) };
            return Err(Error::Unavailable(format!("{} keeps no saves of one game apart{folder}", spec.name)));
        }
        return Ok(Ask { bin, name: game.title.clone(), files: emu.files, ..Default::default() });
    }
    let name = if game.saves.name.is_empty() {
        let found = find(&bin, game).await?.ok_or_else(|| {
            Error::NotFound(format!("ludusavi knows no game by the title '{}': set saves.name to the title it uses (`ludusavi find --fuzzy`)", game.title))
        })?;
        crate::game::set_key(&game.toml_path(), "saves.name", &found)?;
        found
    } else {
        game.saves.name.clone()
    };
    let prefix = matches!(spec.kind, Kind::Proton | Kind::Wine) || spec.via_proton;
    let prefix = prefix.then(|| wine_prefix(&crate::launcher::prefix_of(game, config))).filter(|p| p.is_dir());
    let root = (!game.source.dir.is_empty()).then(|| paths::expand(&game.source.dir)).and_then(|d| d.parent().map(Path::to_path_buf));
    Ok(Ask { bin, name, prefix, files: vec![], root })
}

fn backup_args(verb: &str, ask: &Ask, dir: &Path, extra: &[&str]) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![verb.into()];
    args.extend(extra.iter().map(|a| a.into()));
    args.extend(["--api".into(), "--force".into(), "--path".into(), dir.as_os_str().to_owned()]);
    if verb == "backup" {
        if let Some(p) = &ask.prefix {
            args.extend(["--wine-prefix".into(), p.as_os_str().to_owned()]);
        }
    }
    args
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Found {
    pub path: String,
    pub bytes: u64,
}

fn files_of(reply: &Value, name: &str) -> Vec<Found> {
    let mut out: Vec<Found> = reply["games"][name]["files"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(path, f)| Found { path: path.clone(), bytes: f["bytes"].as_u64().unwrap_or(0) })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub async fn locate(ask: &Ask, config: &Config, id: &str) -> Result<Vec<Found>> {
    let dir = dir(config, id);
    let mut args = backup_args("backup", ask, &dir, &["--preview"]);
    args.push(ask.name.clone().into());
    let reply = run(&ask.bin, Some(ask), &dir, config.saves.keep, args).await?;
    Ok(files_of(&reply, &ask.name))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Outcome {
    /// ludusavi's `new`, `different` or `same`; `none` when it found nothing.
    pub change: String,
    pub files: Vec<Found>,
    pub bytes: u64,
}

fn outcome(reply: &Value, name: &str) -> Outcome {
    let files = files_of(reply, name);
    let change = reply["games"][name]["change"].as_str().map(str::to_ascii_lowercase).unwrap_or_else(|| "none".into());
    Outcome { bytes: files.iter().map(|f| f.bytes).sum(), change, files }
}

/// Unchanged saves make no new backup; the oldest beyond `saves.keep` goes.
pub async fn backup(ask: &Ask, config: &Config, id: &str) -> Result<Outcome> {
    let dir = dir(config, id);
    let keep = config.saves.keep.clamp(1, 255).to_string();
    let mut args = backup_args("backup", ask, &dir, &["--full-limit", &keep, "--differential-limit", "0"]);
    args.push(ask.name.clone().into());
    let reply = run(&ask.bin, Some(ask), &dir, config.saves.keep, args).await?;
    Ok(outcome(&reply, &ask.name))
}

/// An empty `backup` is the latest; a prefix's files land in the prefix the game has now.
pub async fn restore(ask: &Ask, config: &Config, id: &str, backup: &str) -> Result<Outcome> {
    let dir = dir(config, id);
    let listed = backups(&dir);
    let picked = if backup.is_empty() { listed.iter().find(|b| b.name == ask.name).or(listed.first()) } else { listed.iter().find(|b| b.id == backup) };
    let name = picked.map(|b| b.name.clone()).ok_or_else(|| Error::NotFound(format!("{id} has no backup {backup}").trim_end().to_string()))?;
    let mut args = backup_args("restore", ask, &dir, &[]);
    if !backup.is_empty() {
        args.extend(["--backup".into(), backup.into()]);
    }
    args.push(name.clone().into());
    let reply = run(&ask.bin, Some(ask), &dir, config.saves.keep, args).await?;
    Ok(outcome(&reply, &name))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Backup {
    /// What `restore` takes.
    pub id: String,
    /// ludusavi's title of the game at the time.
    pub name: String,
    pub when: String,
    pub bytes: u64,
    pub path: String,
}

/// Newest first.
pub fn backups(dir: &Path) -> Vec<Backup> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let Some(doc) = std::fs::read_to_string(entry.path().join("mapping.yaml"))
            .ok()
            .and_then(|t| yaml_rust2::YamlLoader::load_from_str(&t).ok())
            .and_then(|d| d.into_iter().next())
        else {
            continue;
        };
        let name = doc["name"].as_str().unwrap_or_default().to_string();
        for b in doc["backups"].as_vec().into_iter().flatten() {
            let Some(id) = b["name"].as_str() else { continue };
            let path = entry.path().join(id);
            out.push(Backup {
                id: id.to_string(),
                name: name.clone(),
                when: b["when"].as_str().unwrap_or_default().to_string(),
                bytes: crate::data::disk_usage(&path),
                path: path.to_string_lossy().into(),
            });
        }
    }
    out.sort_by(|a, b| b.when.cmp(&a.when));
    out
}

/// Laid out as ludusavi lays its backups out: unpacked, any ludusavi restores from it.
pub fn export(config: &Config, id: &str, to: &Path) -> Result<PathBuf> {
    use std::io::Write;
    let dir = dir(config, id);
    if backups(&dir).is_empty() {
        return Err(Error::NotFound(format!("{id} has no backup to export")));
    }
    std::fs::create_dir_all(to)?;
    let file = to.join(format!("{id}-saves-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let zipped = |e: zip::result::ZipError| Error::Io(format!("{}: {e}", file.display()));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&file)?);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            let path = e.path();
            let meta = path.symlink_metadata()?;
            let rel = Path::new(id).join(path.strip_prefix(&dir).unwrap_or(&path)).to_string_lossy().into_owned();
            if meta.is_dir() {
                zip.add_directory(rel, options).map_err(zipped)?;
                stack.push(path);
            } else if meta.is_file() {
                zip.start_file(rel, options).map_err(zipped)?;
                zip.write_all(&std::fs::read(&path)?)?;
            }
        }
    }
    zip.finish().map_err(zipped)?;
    Ok(file)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// ludusavi's manifest as `yaml`, checked a moment ago: no call reaches the network.
    pub(crate) fn offline_manifest(yaml: &str) {
        let dir = config_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("manifest.yaml"), yaml).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        std::fs::write(
            dir.join("cache.yaml"),
            format!("manifests:\n  \"https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml\":\n    checked: \"{now}\"\n    updated: \"{now}\"\n"),
        )
        .unwrap();
    }

    fn game(runner: &str, exe: &Path) -> Game {
        let mut g = Game::new("Sample Title");
        g.launch.runner = runner.into();
        g.launch.exe = exe.to_string_lossy().into();
        g
    }

    #[test]
    fn an_emulated_title_maps_to_its_own_saves_by_its_id() {
        let env = paths::test_env();
        let (cfg, data) = (env.path().join("home/.config"), env.path().join("home/.local/share"));
        let disc = env.path().join("games/Persona BLUS31176");
        std::fs::create_dir_all(disc.join("PS3_GAME/USRDIR")).unwrap();
        std::fs::write(disc.join("PS3_GAME/USRDIR/EBOOT.BIN"), b"").unwrap();
        std::fs::write(disc.join("PS3_GAME/PARAM.SFO"), crate::roms::tests::sfo_of(&[("TITLE", "Persona"), ("TITLE_ID", "BLUS31176")])).unwrap();
        let config = Config::default();
        let saves = emulator_under(&game("rpcs3", &disc.join("PS3_GAME/USRDIR/EBOOT.BIN")), &config, &cfg, &data).unwrap();
        assert_eq!(saves.title_id, "BLUS31176");
        assert_eq!(saves.folder, cfg.join("rpcs3/dev_hdd0/home/00000001/savedata").to_string_lossy());
        assert_eq!(saves.files, [format!("{}/rpcs3/dev_hdd0/home/*/savedata/BLUS31176*", cfg.display())]);

        let ps4 = env.path().join("games/CUSA00001");
        std::fs::create_dir_all(ps4.join("sce_sys")).unwrap();
        std::fs::write(ps4.join("sce_sys/param.sfo"), crate::roms::tests::sfo_of(&[("TITLE_ID", "CUSA00001")])).unwrap();
        let saves = emulator_under(&game("shadps4", &ps4.join("eboot.bin")), &config, &cfg, &data).unwrap();
        assert!(saves.files.iter().all(|f| f.ends_with("/CUSA00001")) && saves.files.len() == 2, "{saves:?}");

        let wiiu = env.path().join("games/Zelda [BotW]");
        std::fs::create_dir_all(wiiu.join("meta")).unwrap();
        std::fs::write(wiiu.join("meta/meta.xml"), "<menu><title_id type=\"hexBinary\" length=\"8\">00050000101C9500</title_id></menu>").unwrap();
        let saves = emulator_under(&game("cemu", &wiiu.join("code/U-King.rpx")), &config, &cfg, &data).unwrap();
        assert_eq!(saves.files, [format!("{}/Cemu/mlc01/usr/save/00050000/101c9500", data.display())]);

        let iso = env.path().join("games/F-Zero [!].iso");
        let mut head = vec![0u8; 0x20];
        head[..6].copy_from_slice(b"GFZP8P");
        head[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes());
        std::fs::write(&iso, &head).unwrap();
        let saves = emulator_under(&game("dolphin", &iso), &config, &cfg, &data).unwrap();
        assert_eq!(saves.files, [format!("{}/dolphin-emu/GC/*/Card */8P-GFZP-*.gci", data.display())]);
        let gba = emulator_under(&game("mgba", &env.path().join("games/Metroid [!].gba")), &config, &cfg, &data).unwrap();
        assert_eq!(gba.files, [format!("{}/games/Metroid [[]![]].sav", env.path().display())], "a bracket in a name is no pattern");

        let switch = emulator_under(&game("ryujinx", &env.path().join("games/a.nsp")), &config, &cfg, &data).unwrap();
        assert!(switch.files.is_empty() && switch.folder.ends_with("Ryujinx/bis/user/save"), "a Switch title's id needs its keys: the folder only");
        assert!(!backs_up(&game("ryujinx", &env.path().join("games/a.nsp")), &config));
        assert!(emulator_under(&game("proton", &env.path().join("g.exe")), &config, &cfg, &data).is_none());
    }

    #[test]
    fn a_prefix_in_steams_compatdata_keeps_its_wine_prefix_under_pfx() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("compat/pfx/drive_c")).unwrap();
        std::fs::create_dir_all(dir.path().join("own/drive_c")).unwrap();
        assert_eq!(wine_prefix(&dir.path().join("compat")), dir.path().join("compat/pfx"));
        assert_eq!(wine_prefix(&dir.path().join("own")), dir.path().join("own"));
    }
}
