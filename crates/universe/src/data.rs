use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use crate::config::Config;
use crate::core::{blocking, Core};
use crate::game::Game;
use crate::launcher::prefix_of;
use crate::library::Resolved;
use crate::paths;
use crate::runners::{self, Kind};
use crate::saves;
use crate::{Error, Result};

pub const CLOUD_ACTIONS: [&str; 5] = ["status", "download", "upload", "keep-local", "keep-cloud"];

/// Bytes of the files under `path` (or of the file itself); a symlink counts for nothing and is never followed.
pub fn disk_usage(path: &Path) -> u64 {
    let Ok(meta) = path.symlink_metadata() else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    let mut total = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let Ok(m) = e.path().symlink_metadata() else { continue };
            if m.is_dir() {
                stack.push(e.path());
            } else if m.is_file() {
                total += m.len();
            }
        }
    }
    total
}

/// Free and total bytes of the filesystem `path` is on, or would be on: its nearest folder that exists.
pub fn free_space(path: &Path) -> Option<(u64, u64)> {
    let dir = path.ancestors().find(|p| p.is_dir())?;
    let st = rustix::fs::statvfs(dir).ok()?;
    Some((st.f_bavail.saturating_mul(st.f_frsize), st.f_blocks.saturating_mul(st.f_frsize)))
}

/// `universe` under prefixes_root, `steam` in Steam's compatdata and `wine` for Wine's default prefix (both never moved),
/// `lutris` for an import, else `elsewhere`.
pub fn prefix_owner(prefix: &Path, game: &Game, config: &Config) -> &'static str {
    let names: Vec<_> = prefix.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    // Plain wine, Lutris and other launchers all fall back on these two.
    let defaults = [paths::home().join(".wine"), paths::xdg("XDG_DATA_HOME", ".local/share").join("wine/default")];
    if prefix.starts_with(config.prefixes_root()) {
        "universe"
    } else if names.windows(2).any(|w| w[0] == "steamapps" && w[1] == "compatdata") {
        "steam"
    } else if defaults.iter().any(|d| d == prefix) {
        "wine"
    } else if game.source.kind == "lutris" || !game.source.lutris_slug.is_empty() {
        "lutris"
    } else {
        "elsewhere"
    }
}

fn has_prefix(game: &Game) -> bool {
    runners::spec(&game.runner_id()).is_some_and(|s| matches!(s.kind, Kind::Proton | Kind::Wine) || s.via_proton)
}

fn sharers(game: &Game, games: &[Resolved], config: &Config) -> Vec<String> {
    let prefix = prefix_of(game, config);
    games
        .iter()
        .map(|r| &r.game)
        .filter(|x| x.id != game.id && x.removed_at.is_empty() && has_prefix(x) && prefix_of(x, config) == prefix)
        .map(|x| x.id.clone())
        .collect()
}

fn place(path: &Path) -> Value {
    json!({"path": path, "bytes": disk_usage(path), "exists": path.exists()})
}

/// What a leftover is: `prefix` (no game's), `recordings` (archived at a remove), `game` (a removed game's folder), `logs` (a removed game's).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Leftover {
    pub kind: &'static str,
    pub path: String,
    pub id: String,
    pub title: String,
    pub bytes: u64,
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    out.sort();
    out
}

fn leftovers(games: &[Resolved], config: &Config, sized: bool) -> Vec<Leftover> {
    let live: Vec<&Game> = games.iter().map(|r| &r.game).filter(|g| g.removed_at.is_empty()).collect();
    let title = |id: &str| games.iter().find(|r| r.game.id == id).map(|r| r.game.title.clone()).unwrap_or_default();
    let held: Vec<PathBuf> = live.iter().filter(|g| has_prefix(g)).map(|g| prefix_of(g, config)).collect();
    let mut out = Vec::new();
    let mut push = |kind: &'static str, path: PathBuf| {
        let id = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        out.push(Leftover { kind, title: title(&id), id, bytes: if sized { disk_usage(&path) } else { 0 }, path: path.to_string_lossy().into() });
    };
    for dir in subdirs(&config.prefixes_root()) {
        if !held.iter().any(|h| h.starts_with(&dir) || dir.starts_with(h)) {
            push("prefix", dir);
        }
    }
    for dir in subdirs(&config.recordings_root().join(".archive")) {
        push("recordings", dir);
    }
    let removed = |id: &str| !live.iter().any(|g| g.id == id);
    for dir in subdirs(&paths::games_dir()) {
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if removed(&id) && games.iter().any(|r| r.game.id == id) {
            push("game", dir);
        }
    }
    for dir in subdirs(&paths::state_home().join("logs")) {
        let id = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if removed(&id) {
            push("logs", dir);
        }
    }
    out
}

/// Moves `from` to `to`: a rename, or a copy then the original to the trash across filesystems. `Ok(Some(from))` names an original the trash refused, left where it was.
fn move_dir(from: &Path, to: &Path) -> Result<(bool, Option<PathBuf>)> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => return Ok((false, None)),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {}
        Err(e) => return Err(Error::Io(format!("move {} to {}: {e}", from.display(), to.display()))),
    }
    if let Err(e) = copy_tree(from, to) {
        remove_copy(to);
        return Err(e);
    }
    let count = |p: &Path| (disk_usage(p), walk_count(p));
    if count(from) != count(to) {
        remove_copy(to);
        return Err(Error::Io(format!("the copy of {} into {} came out different; the original is untouched", from.display(), to.display())));
    }
    match trash::delete(from) {
        Ok(()) => Ok((true, None)),
        Err(e) => {
            tracing::warn!("trash {}: {e}; left in place", from.display());
            Ok((true, Some(from.to_path_buf())))
        }
    }
}

/// The copy keeps the original's read-only folders, which `remove_dir_all` can't empty.
fn remove_copy(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            if e.path().symlink_metadata().is_ok_and(|m| m.is_dir()) {
                stack.push(e.path());
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn walk_count(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            n += 1;
            if e.path().symlink_metadata().is_ok_and(|m| m.is_dir()) {
                stack.push(e.path());
            }
        }
    }
    n
}

/// A copy that keeps symlinks as symlinks: a prefix's `dosdevices/z:` points at `/`.
pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    let failed = |p: &Path, e: std::io::Error| Error::Io(format!("copy {}: {e}", p.display()));
    std::fs::create_dir(to).map_err(|e| failed(to, e))?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (src, dest) = (e.path(), to.join(e.file_name()));
        let meta = src.symlink_metadata().map_err(|e| failed(&src, e))?;
        if meta.file_type().is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&src)?, &dest).map_err(|e| failed(&dest, e))?;
        } else if meta.is_dir() {
            copy_tree(&src, &dest)?;
        } else if meta.is_file() {
            std::fs::copy(&src, &dest).map_err(|e| failed(&src, e))?;
            if let Ok(t) = meta.modified() {
                let _ = std::fs::File::options().write(true).open(&dest).and_then(|f| f.set_modified(t));
            }
        }
    }
    std::fs::set_permissions(to, std::fs::metadata(from)?.permissions()).map_err(|e| failed(to, e))
}

impl Core {
    /// The library takes in the title ludusavi found, which the ask wrote to game.toml.
    async fn saves_ask(&self, game: &Game, config: &Config, fetch: bool) -> Result<saves::Ask> {
        let ask = saves::ask(game, config, fetch).await?;
        if ask.files.is_empty() && game.saves.name != ask.name {
            self.reload_game(&game.id).await?;
        }
        Ok(ask)
    }

    async fn running(&self, ids: &[String]) -> Result<()> {
        match self.current().await {
            Some(c) if ids.contains(&c.id) => Err(Error::Busy(format!("{} is running", c.id))),
            _ => Ok(()),
        }
    }

    /// Where a game's data lives, each part with its size: install, prefix, saves, Universe's own files, recordings, logs.
    pub async fn game_data(&self, id: &str) -> Result<Value> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        let games = self.games.read().await.clone();
        let uninstaller = self.uninstall_via(id).await.ok().flatten();
        let game = r.game.clone();
        let mut data = {
            let config = config.clone();
            blocking(move || Ok(index(&game, &games, &config, uninstaller))).await?
        };
        let (saves, cloud) = tokio::join!(self.saves_status(&r.game, &config), self.saves_cloud_of(&r, "status"));
        data["saves"] = saves;
        data["saves"]["cloud"] = match cloud {
            Ok(cloud) => cloud,
            Err(e) => {
                if !matches!(e, Error::Unavailable(_)) {
                    tracing::warn!("{id}: cloud saves: {e}");
                }
                Value::Null
            }
        };
        let total: u64 = ["install", "prefix", "universe", "recordings", "logs"].iter().map(|k| data[k]["bytes"].as_u64().unwrap_or(0)).sum::<u64>()
            + data["saves"]["backups_bytes"].as_u64().unwrap_or(0);
        data["total"] = total.into();
        Ok(data)
    }

    async fn saves_status(&self, game: &Game, config: &Config) -> Value {
        let dir = saves::dir(config, &game.id);
        let backups = {
            let dir = dir.clone();
            blocking(move || Ok(saves::backups(&dir))).await.unwrap_or_default()
        };
        let emu = saves::emulator(game, config);
        let mut out = json!({
            "engine": if emu.as_ref().is_some_and(|e| e.files.is_empty()) { "" } else if emu.is_some() { "emulator" } else { "ludusavi" },
            "name": game.saves.name, "files": [], "bytes": 0, "error": "",
            "folder": emu.as_ref().map(|e| e.folder.clone()).unwrap_or_default(),
            "title_id": emu.as_ref().map(|e| e.title_id.clone()).unwrap_or_default(),
            "dir": dir, "backups_bytes": backups.iter().map(|b| b.bytes).sum::<u64>(), "backups": backups,
            "auto": config.saves.auto_backup, "keep": config.saves.keep,
        });
        if out["engine"] == "" {
            return out;
        }
        match self.saves_ask(game, config, false).await {
            Ok(ask) => {
                out["name"] = ask.name.clone().into();
                match saves::locate(&ask, config, &game.id).await {
                    Ok(files) => {
                        out["bytes"] = files.iter().map(|f| f.bytes).sum::<u64>().into();
                        out["files"] = serde_json::to_value(files).unwrap_or_default();
                    }
                    Err(e) => out["error"] = e.to_string().into(),
                }
            }
            Err(e) => out["error"] = e.to_string().into(),
        }
        out
    }

    /// Every root Universe writes under with its size and free space, the games by size, and what no game holds any more.
    pub async fn storage(&self) -> Result<Value> {
        let config = self.config.read().await.clone();
        let games = self.games.read().await.clone();
        blocking(move || Ok(storage(&games, &config))).await
    }

    /// `storage()`'s roots with their free space and no size: no folder walked, cheap enough for a home screen.
    pub async fn disk_free(&self) -> Value {
        let config = self.config.read().await.clone();
        roots(&config).iter().map(|(id, path)| root_json(id, path, None)).collect()
    }

    /// One of `storage()`'s leftovers to the trash, named by its path: nothing else is taken.
    pub async fn trash_leftover(&self, path: &str) -> Result<()> {
        let config = self.config.read().await.clone();
        let games = self.games.read().await.clone();
        let listed = leftovers(&games, &config, false);
        let Some(item) = listed.iter().find(|l| l.path == path) else {
            return Err(Error::Invalid(format!("{path} is no leftover: only what the Storage view lists goes to the trash")));
        };
        trash::delete(&item.path).map_err(|e| Error::Io(format!("trash {}: {e}", item.path)))?;
        if item.kind == "game" {
            self.reload_all().await;
        }
        Ok(())
    }

    /// The prefix moved to `prefixes_root/<id>`, every game using it pointed there; Steam's compatdata and Universe's own stay.
    pub async fn move_prefix(&self, id: &str) -> Result<Value> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        if !has_prefix(&r.game) {
            return Err(Error::Invalid(format!("{id} has no Wine prefix")));
        }
        let from = prefix_of(&r.game, &config);
        match prefix_owner(&from, &r.game, &config) {
            "universe" => return Err(Error::Invalid(format!("{} is already under {}", from.display(), config.prefixes_root().display()))),
            "steam" => return Err(Error::Invalid(format!("{} is Steam's: it stays where Steam keeps it", from.display()))),
            "wine" => return Err(Error::Invalid(format!("{} is Wine's default prefix: it stays where other launchers look for it", from.display()))),
            _ => {}
        }
        if !from.is_dir() {
            return Err(Error::NotFound(format!("{} does not exist", from.display())));
        }
        let home = paths::home();
        let unsafe_root = from.parent().is_none() || from == home || home.starts_with(&from) || from == config.games_root();
        if unsafe_root || !saves::wine_prefix(&from).join("drive_c").is_dir() {
            return Err(Error::Invalid(format!("refusing to move {}: it is no Wine prefix", from.display())));
        }
        let mut moved: Vec<String> = vec![id.to_string()];
        moved.extend(sharers(&r.game, &self.games.read().await, &config));
        self.running(&moved).await?;
        let to = config.prefixes_root().join(id);
        if to.exists() {
            return Err(Error::Invalid(format!("{} already exists", to.display())));
        }
        let (copied, left) = {
            let (from, to) = (from.clone(), to.clone());
            blocking(move || move_dir(&from, &to)).await?
        };
        for g in &moved {
            let toml = paths::game_dir(g).join("game.toml");
            crate::game::set_key(&toml, "launch.prefix", &to.to_string_lossy())?;
            self.reload_game(g).await?;
        }
        let owner = prefix_owner(&from, &r.game, &config);
        Ok(json!({"from": from, "to": to, "copied": copied, "left": left, "games": moved, "owner": owner}))
    }

    /// The game's saves backed up before its prefix goes: `Ok(None)` when there is nothing to back up, an error when the backup failed.
    pub(crate) async fn backup_before(&self, game: &Game) -> Result<Option<saves::Outcome>> {
        let config = self.config.read().await.clone();
        let empty = has_prefix(game) && !saves::wine_prefix(&prefix_of(game, &config)).join("drive_c").is_dir();
        if !saves::backs_up(game, &config) || empty {
            return Ok(None);
        }
        let refused = |e: Error| Error::Invalid(format!("refusing to touch {}'s prefix: its saves could not be backed up: {e}", game.id));
        let ask = match self.saves_ask(game, &config, true).await {
            Ok(ask) => ask,
            Err(Error::NotFound(_)) => return Ok(None),
            Err(e) => return Err(refused(e)),
        };
        saves::backup(&ask, &config, &game.id).await.map(Some).map_err(refused)
    }

    /// Universe's prefix of the game to the trash, its saves backed up first: the next launch makes a fresh one.
    pub async fn reset_prefix(&self, id: &str) -> Result<Value> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        if !has_prefix(&r.game) {
            return Err(Error::Invalid(format!("{id} has no Wine prefix")));
        }
        let prefix = prefix_of(&r.game, &config);
        if prefix_owner(&prefix, &r.game, &config) != "universe" {
            return Err(Error::Invalid(format!("{} is not Universe's: move it into {} first", prefix.display(), config.prefixes_root().display())));
        }
        if let Some(other) = sharers(&r.game, &self.games.read().await, &config).first() {
            return Err(Error::Invalid(format!("refusing to trash {}: {other} uses it too", prefix.display())));
        }
        if !prefix.is_dir() {
            return Err(Error::NotFound(format!("{} does not exist", prefix.display())));
        }
        self.running(&[id.to_string()]).await?;
        let backup = self.backup_before(&r.game).await?;
        trash::delete(&prefix).map_err(|e| Error::Io(format!("trash {}: {e}", prefix.display())))?;
        Ok(json!({"trashed": prefix, "backup": backup}))
    }

    pub async fn saves_backup(&self, id: &str) -> Result<saves::Outcome> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        let ask = self.saves_ask(&r.game, &config, true).await?;
        saves::backup(&ask, &config, id).await
    }

    /// An empty `backup` is the latest.
    pub async fn saves_restore(&self, id: &str, backup: &str) -> Result<saves::Outcome> {
        let r = self.get(id).await?;
        self.running(&[id.to_string()]).await?;
        let config = self.config.read().await.clone();
        let ask = self.saves_ask(&r.game, &config, true).await?;
        saves::restore(&ask, &config, id, backup).await
    }

    /// The game's cloud saves through its source's `cloud-saves` verb: `status`, `download`, `upload`, `keep-local` or `keep-cloud`.
    pub async fn saves_cloud(&self, id: &str, action: &str) -> Result<Value> {
        let r = self.get(id).await?;
        if !CLOUD_ACTIONS.contains(&action) {
            return Err(Error::Invalid(format!("{action}: not one of {}", CLOUD_ACTIONS.join(", "))));
        }
        if action != "status" {
            self.running(&[id.to_string()]).await?;
        }
        self.saves_cloud_of(&r, action).await
    }

    async fn saves_cloud_of(&self, r: &Resolved, action: &str) -> Result<Value> {
        let Some(m) = self.game_source(&r.game).await.filter(|s| s.can("cloud-saves")) else {
            return Err(Error::Unavailable(format!("{}: its store keeps no cloud saves Universe syncs", r.game.title)));
        };
        if r.game.source.id.is_empty() {
            return Err(Error::Unavailable(format!("{}: {} does not know its id", r.game.title, m.name())));
        }
        let args = [r.game.source.id.clone(), action.to_string()];
        let events = self.run_verb_for(&m, "cloud-saves", &args, r).await?;
        events
            .into_iter()
            .find_map(|e| if let crate::sources::SourceEvent::Cloud(c) = e { Some(Value::Object(c)) } else { None })
            .ok_or_else(|| Error::Io(format!("{} cloud-saves printed no cloud event", m.id())))
    }

    /// Every backup of the game zipped into the folder `to`; returns the zip.
    pub async fn saves_export(&self, id: &str, to: &str) -> Result<PathBuf> {
        self.get(id).await?;
        let config = self.config.read().await.clone();
        let (id, to) = (id.to_string(), paths::expand(to));
        blocking(move || saves::export(&config, &id, &to)).await
    }

    /// winecfg, winetricks or an .exe in the game's prefix as a unit of its own (`{"tool", "unit"}`), or `kill`: the prefix's Wine stopped (`{"tool", "stopped"}`, its wineservers).
    pub async fn prefix_tool(&self, id: &str, tool: &str, args: &[String]) -> Result<Value> {
        let r = self.get(id).await?;
        let config = self.config.read().await.clone();
        if tool == "kill" {
            if !has_prefix(&r.game) {
                return Err(Error::Invalid(format!("{id}: {} keeps no Wine prefix", r.game.runner_id())));
            }
            let stopped = stop_wine(&prefix_of(&r.game, &config)).await?;
            return Ok(json!({"tool": tool, "stopped": stopped}));
        }
        let (program, argv, env) = crate::launcher::prefix_command(&r, &config, tool, args)?;
        let program = tool_program(&program).await?;
        let mut unit_env = crate::core::passthrough_env();
        unit_env.extend(env);
        let spec = crate::host::UnitSpec {
            name: format!("universe-prefix-{id}-{tool}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S")),
            description: format!("Universe {tool} in {}'s prefix", r.game.title),
            program,
            args: argv,
            env: unit_env,
            cwd: Some(r.game.working_dir()).filter(|d| d.is_dir()),
            ..Default::default()
        };
        self.host.units.start(&spec).await?;
        let shows_as = match tool {
            "winecfg" => "winecfg.exe".to_string(),
            "run" => args.first().and_then(|exe| Path::new(exe).file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            _ => tool.to_string(),
        };
        self.tool_up(&spec.name, &shows_as).await?;
        Ok(json!({"tool": tool, "unit": spec.name}))
    }

    /// umu sets its container up first: a failure can come 20 s in, and one past `TOOL_UP_WAIT` goes unreported.
    async fn tool_up(&self, unit: &str, program: &str) -> Result<()> {
        let units = &self.host.units;
        let deadline = tokio::time::Instant::now() + TOOL_UP_WAIT;
        while tokio::time::Instant::now() < deadline {
            if !units.is_active(unit).await {
                let mut log = units.log(unit).await;
                for _ in 0..8 {
                    if log.ended.is_some() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    log = units.log(unit).await;
                }
                let Some(code) = log.exit.filter(|c| *c != 0) else { return Ok(()) };
                let said = units.journal(unit, 50).await.into_iter().rev().find(|l| l.source != "systemd").map(|l| format!(": {}", l.message));
                return Err(Error::Io(format!("{program} failed with exit status {code}{}", said.unwrap_or_default())));
            }
            if units.holds(unit, program).await {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        Ok(())
    }
}

const TOOL_UP_WAIT: Duration = Duration::from_secs(60);

/// Wine names a prefix's server dir `server-<dev>-<ino>` after the prefix, in whichever `/tmp` the server sees (umu's sandbox keeps its own).
fn server_dir_name(prefix: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(prefix).ok().map(|m| format!("server-{:x}-{:x}", m.dev(), m.ino()))
}

fn wineservers(proc: &Path, dirs: &[String]) -> Vec<i32> {
    let serves = |pid: &Path| {
        let comm = std::fs::read_to_string(pid.join("comm")).unwrap_or_default();
        let cwd = std::fs::read_link(pid.join("cwd")).unwrap_or_default();
        comm.starts_with("wineserver") && cwd.file_name().and_then(|n| n.to_str()).is_some_and(|n| dirs.iter().any(|d| d == n))
    };
    let mut pids: Vec<i32> =
        std::fs::read_dir(proc).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str()?.parse().ok().filter(|_| serves(&e.path()))).collect();
    pids.sort();
    pids
}

fn alive(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| s.rsplit(')').next().and_then(|r| r.split_whitespace().next()) != Some("Z"))
}

/// What `wineserver -k` does, from outside any sandbox: SIGINT has the server kill its programs, save the registry and exit; SIGKILL if it hangs on.
async fn stop_wine(prefix: &Path) -> Result<usize> {
    use rustix::process::{kill_process, Pid, Signal};
    let mut dirs: Vec<String> = [prefix.to_path_buf(), prefix.join("pfx")].iter().filter_map(|p| server_dir_name(p)).collect();
    dirs.dedup();
    let servers = wineservers(Path::new("/proc"), &dirs);
    let signal = |sig: Signal| {
        for pid in servers.iter().filter_map(|p| Pid::from_raw(*p)) {
            let _ = kill_process(pid, sig);
        }
    };
    let gone = || async {
        for _ in 0..40 {
            if !servers.iter().any(|p| alive(*p)) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(125)).await;
        }
        false
    };
    signal(Signal::INT);
    signal(Signal::CONT);
    if !gone().await {
        tracing::warn!("{}: the wineserver outlived SIGINT, killed", prefix.display());
        signal(Signal::KILL);
        if !gone().await {
            return Err(Error::Busy(format!("{}: its wineserver is still running", prefix.display())));
        }
    }
    Ok(servers.len())
}

/// A unit started on a missing program fails where nobody looks: the program is found, or fetched (umu-run), first.
async fn tool_program(program: &str) -> Result<String> {
    if runners::on_path(program).is_none() && !program.contains('/') {
        let name = program.to_string();
        blocking(move || tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(crate::tools::ensure(&name))).await?;
    }
    runners::on_path(program)
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Unavailable(format!("{program} is not installed: install it to use it in a game's prefix")))
}

fn index(game: &Game, games: &[Resolved], config: &Config, uninstaller: Option<String>) -> Value {
    let install = (!game.source.dir.is_empty()).then(|| paths::expand(&game.source.dir));
    let install = install.map(|dir| {
        let mut v = place(&dir);
        v["owner"] = uninstaller.unwrap_or_else(|| "universe".into()).into();
        v
    });
    let prefix = has_prefix(game).then(|| {
        let path = prefix_of(game, config);
        let owner = prefix_owner(&path, game, config);
        let target = config.prefixes_root().join(&game.id);
        let mut v = place(&path);
        v["owner"] = owner.into();
        v["shared_with"] = sharers(game, games, config).into();
        v["movable"] = (matches!(owner, "lutris" | "elsewhere") && path.is_dir() && !target.exists()).into();
        v["target"] = target.to_string_lossy().into_owned().into();
        v
    });
    let dir = game.dir();
    let parts: BTreeMap<&str, u64> =
        [("media", game.media_dir()), ("screenshots", game.screenshots_dir()), ("journal", game.journal_dir()), ("sessions", game.sessions_path())]
            .into_iter()
            .map(|(k, p)| (k, disk_usage(&p)))
            .collect();
    let root = config.recordings_root();
    let archived = root.join(".archive").join(&game.id);
    let recordings = if archived.is_dir() && !root.join(&game.id).is_dir() { archived.clone() } else { root.join(&game.id) };
    let mut recordings = place(&recordings);
    recordings["archived"] = (recordings["path"].as_str() == Some(&*archived.to_string_lossy())).into();
    json!({
        "id": game.id, "title": game.title,
        "runner_kind": runners::spec(&game.runner_id()).map(|s| s.kind.as_str()).unwrap_or(""),
        "install": install, "prefix": prefix,
        "universe": {"path": dir, "bytes": disk_usage(&dir), "parts": parts},
        "recordings": recordings,
        "logs": place(&paths::game_logs_dir(&game.id)),
    })
}

fn roots(config: &Config) -> [(&'static str, PathBuf); 7] {
    [
        ("games", config.games_root()),
        ("prefixes", config.prefixes_root()),
        ("saves", config.saves_root()),
        ("recordings", config.recordings_root()),
        ("library", paths::games_dir()),
        ("components", crate::components::root()),
        ("logs", paths::state_home().join("logs")),
    ]
}

fn root_json(id: &str, path: &Path, bytes: Option<u64>) -> Value {
    let (free, total) = free_space(path).unwrap_or((0, 0));
    let mut v = json!({"id": id, "path": path, "free": free, "size": total, "exists": path.is_dir()});
    if let Some(b) = bytes {
        v["bytes"] = b.into();
    }
    v
}

fn storage(games: &[Resolved], config: &Config) -> Value {
    let roots: Vec<Value> = roots(config).iter().map(|(id, path)| root_json(id, path, Some(disk_usage(path)))).collect();
    let mut sized: Vec<Value> = games
        .iter()
        .filter(|r| r.game.removed_at.is_empty())
        .map(|r| {
            let g = &r.game;
            let part = |p: Option<PathBuf>| p.map(|p| disk_usage(&p)).unwrap_or(0);
            let install = part((!g.source.dir.is_empty()).then(|| paths::expand(&g.source.dir)));
            let prefix = part(has_prefix(g).then(|| prefix_of(g, config)));
            let universe = disk_usage(&g.dir());
            let recordings = disk_usage(&config.recordings_root().join(&g.id));
            let saves = disk_usage(&saves::dir(config, &g.id));
            let logs = disk_usage(&paths::game_logs_dir(&g.id));
            json!({
                "id": g.id, "title": g.title, "bytes": install + prefix + universe + recordings + saves + logs,
                "install": install, "prefix": prefix, "universe": universe, "recordings": recordings, "saves": saves, "logs": logs,
            })
        })
        .collect();
    sized.sort_by_key(|g| std::cmp::Reverse(g["bytes"].as_u64().unwrap_or(0)));
    let leftovers = leftovers(games, config, true);
    let leftover_bytes: u64 = leftovers.iter().map(|l| l.bytes).sum();
    json!({"roots": roots, "games": sized, "leftovers": leftovers, "leftover_bytes": leftover_bytes})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;

    fn library(env: &paths::TestEnv) -> PathBuf {
        let prefixes = env.path().join("prefixes");
        let umu = env.path().join("bin/umu-run");
        std::fs::create_dir_all(umu.parent().unwrap()).unwrap();
        std::fs::write(&umu, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&umu, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        std::fs::write(
            env.path().join("config/config.toml"),
            format!(
                "[paths]\nprefixes_root = \"{}\"\nrecordings_root = \"{}\"\n[launch]\ngamescope = false\nmangohud = false\nfps_limit = \"none\"\numu_run = \"{}\"\n[modules]\nenabled = []\n[sources]\nenabled = []\n",
                prefixes.display(),
                env.path().join("recordings").display(),
                umu.display()
            ),
        )
        .unwrap();
        prefixes
    }

    fn proton_game(env: &paths::TestEnv, title: &str, prefix: &Path) -> Game {
        let dir = env.path().join("games").join(title);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Game.exe"), b"MZ").unwrap();
        let mut g = Game::new(title);
        g.launch.runner = "proton".into();
        g.launch.exe = dir.join("Game.exe").to_string_lossy().into();
        g.launch.prefix = prefix.to_string_lossy().into();
        g.source.dir = dir.to_string_lossy().into();
        g.save().unwrap();
        g
    }

    async fn open() -> (Core, std::sync::Arc<crate::host::Memory>) {
        let (host, memory) = Host::memory();
        (Core::open_with(Config::load().unwrap(), host).await.unwrap(), memory)
    }

    #[tokio::test]
    async fn a_prefix_moved_into_prefixes_root_keeps_its_games_launchable() {
        let env = paths::test_env();
        let prefixes = library(&env);
        saves::tests::offline_manifest("{}\n");
        let elsewhere = env.path().join("lutris/prefixes/sample");
        std::fs::create_dir_all(elsewhere.join("drive_c/users/steamuser")).unwrap();
        std::fs::write(elsewhere.join("drive_c/users/steamuser/save.dat"), b"slot one").unwrap();
        proton_game(&env, "Sample", &elsewhere);
        proton_game(&env, "Sample Twin", &elsewhere);
        let (core, _) = open().await;
        let data = core.game_data("sample").await.unwrap();
        assert_eq!((data["prefix"]["owner"].as_str(), data["prefix"]["movable"].as_bool()), (Some("elsewhere"), Some(true)));
        assert_eq!(data["prefix"]["shared_with"], json!(["sample-twin"]));

        let moved = core.move_prefix("sample").await.unwrap();
        assert_eq!(moved["owner"], "elsewhere", "who may still point at the old path");
        let to = prefixes.join("sample");
        assert_eq!(moved["games"], json!(["sample", "sample-twin"]), "every game on the prefix follows it");
        assert_eq!(std::fs::read(to.join("drive_c/users/steamuser/save.dat")).unwrap(), b"slot one");
        assert!(!elsewhere.exists());
        for id in ["sample", "sample-twin"] {
            let r = core.get(id).await.unwrap();
            assert_eq!(Path::new(&r.game.launch.prefix), to);
            let config = core.config.read().await.clone();
            let plan = crate::launcher::plan(&r, &config, &BTreeMap::new(), None, None, false, false).unwrap();
            assert_eq!(Path::new(&plan.env["WINEPREFIX"]), to, "{id} launches on the moved prefix");
        }
        assert!(matches!(core.move_prefix("sample").await, Err(Error::Invalid(_))), "Universe's own stays");
        let steam = env.path().join("SteamLibrary/steamapps/compatdata/620");
        std::fs::create_dir_all(&steam).unwrap();
        proton_game(&env, "Portal 2", &steam);
        core.reload_all().await;
        assert!(matches!(core.move_prefix("portal-2").await, Err(Error::Invalid(_))), "Steam's compatdata never moves");
        assert!(steam.is_dir());
        let home = paths::home();
        std::fs::write(home.join("notes.txt"), b"kept").unwrap();
        proton_game(&env, "Home Pointer", &home);
        core.reload_all().await;
        assert!(matches!(core.move_prefix("home-pointer").await, Err(Error::Invalid(_))), "a home is no prefix to move");
        assert!(home.join("notes.txt").is_file());
        let plain = env.path().join("not-a-prefix");
        std::fs::create_dir_all(&plain).unwrap();
        proton_game(&env, "Plain Folder", &plain);
        core.reload_all().await;
        assert!(matches!(core.move_prefix("plain-folder").await, Err(Error::Invalid(_))), "no drive_c, no Wine prefix");
        assert!(plain.is_dir());
        for (title, default) in [("Dot Wine", home.join(".wine")), ("Shared Default", paths::xdg("XDG_DATA_HOME", ".local/share").join("wine/default"))] {
            std::fs::create_dir_all(default.join("drive_c")).unwrap();
            let g = proton_game(&env, title, &default);
            core.reload_all().await;
            let data = core.game_data(&g.id).await.unwrap();
            assert_eq!((data["prefix"]["owner"].as_str(), data["prefix"]["movable"].as_bool()), (Some("wine"), Some(false)), "{title}");
            assert!(matches!(core.move_prefix(&g.id).await, Err(Error::Invalid(_))), "Wine's default prefix never moves");
            assert!(default.join("drive_c").is_dir());
        }
    }

    #[tokio::test]
    async fn the_storage_view_sizes_every_root_and_lists_only_what_no_game_holds() {
        let env = paths::test_env();
        let prefixes = library(&env);
        saves::tests::offline_manifest("{}\n");
        let held = prefixes.join("one");
        std::fs::create_dir_all(&held).unwrap();
        std::fs::write(held.join("system.reg"), vec![0u8; 3000]).unwrap();
        let one = proton_game(&env, "One", &held);
        std::fs::write(env.path().join("games/One/data.pak"), vec![0u8; 50_000]).unwrap();
        std::fs::create_dir_all(one.media_dir()).unwrap();
        std::fs::write(one.media_dir().join("box_front.jpg"), vec![0u8; 700]).unwrap();
        std::fs::create_dir_all(env.path().join("recordings/one")).unwrap();
        std::fs::write(env.path().join("recordings/one/a.mkv"), vec![0u8; 9000]).unwrap();
        let stale = prefixes.join("one-bak");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("user.reg"), vec![0u8; 100]).unwrap();
        std::fs::create_dir_all(env.path().join("recordings/.archive/gone")).unwrap();
        let mut gone = Game::new("Gone");
        gone.removed_at = "2026-01-01T00:00:00+01:00".into();
        gone.save().unwrap();
        let (core, memory) = open().await;

        let data = core.game_data("one").await.unwrap();
        assert_eq!(data["install"]["bytes"], 50_002);
        assert_eq!((data["prefix"]["owner"].as_str(), data["prefix"]["bytes"].as_u64()), (Some("universe"), Some(3000)));
        assert_eq!(data["universe"]["parts"]["media"], 700);
        assert_eq!(data["recordings"]["bytes"], 9000);
        assert!(data["total"].as_u64().unwrap() >= 50_002 + 3000 + 9000);

        let storage = core.storage().await.unwrap();
        let kinds: Vec<(String, String)> =
            storage["leftovers"].as_array().unwrap().iter().map(|l| (l["kind"].as_str().unwrap().to_string(), l["id"].as_str().unwrap().to_string())).collect();
        assert_eq!(kinds, [("prefix".to_string(), "one-bak".to_string()), ("recordings".into(), "gone".into()), ("game".into(), "gone".into())]);
        assert_eq!(storage["games"][0]["id"], "one");
        let root = storage["roots"].as_array().unwrap().iter().find(|r| r["id"] == "prefixes").unwrap().clone();
        assert_eq!(root["bytes"], 3100);
        assert!(root["free"].as_u64().unwrap() > 0);
        let free = core.disk_free().await;
        assert_eq!((&free[1]["path"], &free[1]["size"]), (&root["path"], &root["size"]), "the same root, its disk's size alone");
        assert!(free[1]["free"].as_u64().unwrap() > 0);
        assert!(free[1].get("bytes").is_none(), "no folder walked");

        assert!(matches!(core.trash_leftover(&held.to_string_lossy()).await, Err(Error::Invalid(_))), "a held prefix is no leftover");
        assert!(matches!(core.trash_leftover("/etc").await, Err(Error::Invalid(_))));
        core.trash_leftover(&stale.to_string_lossy()).await.unwrap();
        assert!(!stale.exists() && held.is_dir());

        let started = core.prefix_tool("one", "winecfg", &[]).await.unwrap();
        let spec = memory.spec(started["unit"].as_str().unwrap()).unwrap();
        assert_eq!((Path::new(&spec.program), spec.args.as_slice()), (env.path().join("bin/umu-run").as_path(), ["winecfg".to_string()].as_slice()));
        assert_eq!(Path::new(&spec.env["WINEPREFIX"]), held);
        assert!(matches!(core.prefix_tool("one", "run", &["/nowhere.exe".into()]).await, Err(Error::NotFound(_))));
        memory.exit_next_start(127, "/usr/bin/env: 'bash': No such file or directory");
        let failed = core.prefix_tool("one", "winetricks", &[]).await;
        assert!(matches!(&failed, Err(Error::Io(m)) if m.contains("127") && m.contains("No such file")), "{failed:?}: reported with its reason, not started");
        let units = memory.calls().len();
        assert_eq!(core.prefix_tool("one", "kill", &[]).await.unwrap()["stopped"], 0, "no Wine runs in the prefix");
        assert_eq!(memory.calls().len(), units, "kill starts no unit");
        let mut wine = proton_game(&env, "Wine Game", &held);
        wine.launch.runner = "wine".into();
        wine.launch.runner_exe = env.path().join("no-wine/wine").to_string_lossy().into();
        wine.save().unwrap();
        core.reload_all().await;
        let missing = core.prefix_tool("wine-game", "winecfg", &[]).await;
        assert!(matches!(&missing, Err(Error::Unavailable(m)) if m.contains("no-wine/wine")), "{missing:?}: a missing program is named, nothing started");
        assert!(matches!(tool_program("universe-no-such-tool").await, Err(Error::Unavailable(m)) if m.contains("universe-no-such-tool")));
    }

    #[tokio::test]
    async fn saves_are_backed_up_and_restored_through_ludusavi_into_the_prefix_the_game_has_now() {
        let env = paths::test_env();
        if saves::installed().is_none() {
            eprintln!("ludusavi is not on PATH: the dev shell and the Nix check have it");
            return;
        }
        let prefixes = library(&env);
        saves::tests::offline_manifest("Fixture Game:\n  files:\n    \"<winAppData>/Fixture\":\n      when:\n        - os: windows\n");
        let elsewhere = env.path().join("elsewhere/fixture");
        let save = |prefix: &Path| prefix.join("drive_c/users/steamuser/AppData/Roaming/Fixture/slot1.sav");
        std::fs::create_dir_all(save(&elsewhere).parent().unwrap()).unwrap();
        std::fs::write(save(&elsewhere), b"first").unwrap();
        proton_game(&env, "Fixture Game", &elsewhere);
        let (core, _) = open().await;

        let done = core.saves_backup("fixture-game").await.unwrap();
        assert_eq!((done.change.as_str(), done.files.len()), ("new", 1));
        assert_eq!(core.get("fixture-game").await.unwrap().game.saves.name, "Fixture Game", "the title ludusavi found is kept");
        let data = core.game_data("fixture-game").await.unwrap();
        assert_eq!(data["saves"]["engine"], "ludusavi");
        assert_eq!(data["saves"]["files"][0]["bytes"], 5);
        assert_eq!(data["saves"]["backups"].as_array().unwrap().len(), 1);

        std::fs::write(save(&elsewhere), b"second").unwrap();
        core.move_prefix("fixture-game").await.unwrap();
        let moved = prefixes.join("fixture-game");
        core.saves_restore("fixture-game", "").await.unwrap();
        assert_eq!(std::fs::read(save(&moved)).unwrap(), b"first", "the backup lands in the moved prefix");
        assert!(!save(&elsewhere).exists());

        let zip = core.saves_export("fixture-game", &env.path().join("exports").to_string_lossy()).await.unwrap();
        assert!(zip.is_file() && zip.starts_with(env.path().join("exports")));

        let reset = core.reset_prefix("fixture-game").await.unwrap();
        assert_eq!(reset["backup"]["change"], "same", "the saves are backed up before the prefix goes");
        assert!(!moved.exists());
    }

    #[test]
    fn a_copy_keeps_symlinks_as_symlinks_and_never_follows_one() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("pfx");
        std::fs::create_dir_all(from.join("drive_c/users/steamuser")).unwrap();
        std::fs::create_dir_all(from.join("dosdevices")).unwrap();
        std::fs::write(from.join("drive_c/users/steamuser/save.dat"), b"slot one").unwrap();
        std::os::unix::fs::symlink("/", from.join("dosdevices/z:")).unwrap();
        std::os::unix::fs::symlink("../drive_c", from.join("dosdevices/c:")).unwrap();
        let to = dir.path().join("moved");
        copy_tree(&from, &to).unwrap();
        assert_eq!(std::fs::read(to.join("drive_c/users/steamuser/save.dat")).unwrap(), b"slot one");
        assert_eq!(std::fs::read_link(to.join("dosdevices/z:")).unwrap(), Path::new("/"), "z: still points at / and the root was not copied");
        assert_eq!(std::fs::read_link(to.join("dosdevices/c:")).unwrap(), Path::new("../drive_c"));
        assert_eq!((disk_usage(&from), walk_count(&from)), (disk_usage(&to), walk_count(&to)));
    }

    #[test]
    fn a_read_only_folder_is_copied_with_its_mode_and_a_failed_copy_still_goes() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("pfx");
        let locked = from.join("drive_c/locked");
        std::fs::create_dir_all(locked.join("inner")).unwrap();
        std::fs::write(locked.join("inner/data.bin"), b"kept").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let to = dir.path().join("moved");
        let copied = copy_tree(&from, &to);
        let mode = std::fs::metadata(to.join("drive_c/locked")).map(|m| m.permissions().mode() & 0o777);
        let same = (disk_usage(&from), walk_count(&from)) == (disk_usage(&to), walk_count(&to));
        let content = std::fs::read(to.join("drive_c/locked/inner/data.bin"));
        remove_copy(&to);
        let removed = !to.exists();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        copied.unwrap();
        assert_eq!(mode.unwrap(), 0o555);
        assert_eq!(content.unwrap(), b"kept");
        assert!(same, "the copy-equals-original check holds");
        assert!(removed, "a copy holding a read-only folder can still be removed");
    }

    #[test]
    fn kill_picks_only_the_wineserver_of_the_target_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let (target, other) = (dir.path().join("target"), dir.path().join("other"));
        std::fs::create_dir_all(&target).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let (mine, theirs) = (server_dir_name(&target).unwrap(), server_dir_name(&other).unwrap());
        let proc = dir.path().join("proc");
        let fake = |pid: u32, comm: &str, cwd: &str| {
            let at = proc.join(pid.to_string());
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("comm"), format!("{comm}\n")).unwrap();
            std::os::unix::fs::symlink(cwd, at.join("cwd")).unwrap();
        };
        fake(410, "wineserver", &format!("/tmp/.wine-1000/{mine}"));
        fake(420, "wineserver", &format!("/tmp/.wine-1000/{theirs}"));
        fake(430, "bash", &format!("/tmp/.wine-1000/{mine}"));
        fake(440, "winecfg.exe", "/mnt/games/Dead Cells");
        fake(450, "wineserver", &format!("/run/user/1000/sandbox/tmp/.wine-1000/{mine}"));
        std::fs::create_dir_all(proc.join("self")).unwrap();
        assert_eq!(wineservers(&proc, &[mine]), [410, 450], "the target's servers, in any sandbox's /tmp, and no shell sitting in their dir");
        assert_eq!(wineservers(&proc, &[theirs]), [420]);
    }

    #[test]
    fn a_prefix_is_owned_by_where_it_sits() {
        let mut config = Config::default();
        config.paths.prefixes_root = "/data/prefixes".into();
        let mut g = Game::new("x");
        assert_eq!(prefix_owner(Path::new("/data/prefixes/x"), &g, &config), "universe");
        assert_eq!(prefix_owner(Path::new("/games/SteamLibrary/steamapps/compatdata/620"), &g, &config), "steam");
        assert_eq!(prefix_owner(Path::new("/home/me/Games/x"), &g, &config), "elsewhere");
        g.source.kind = "lutris".into();
        assert_eq!(prefix_owner(Path::new("/home/me/Games/x"), &g, &config), "lutris");
    }
}
