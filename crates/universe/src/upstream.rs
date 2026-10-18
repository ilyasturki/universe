//! What Steam, titledb, Sony and a game's store say is the newest build, kept under the cache, and the rules that judge
//! a game's installed version by it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::game::Game;
use crate::versions::Installed;
use crate::{paths, Error, Result};

const STEAM: &str = "https://api.steamcmd.net/v1/info/";
const TITLEDB: &str = "https://raw.githubusercontent.com/blawar/titledb/master/versions.json";
const SONY: &str = "https://a0.ww.np.dl.playstation.net/tpl/np/";
/// Serves a certificate chained to a root no trust store holds: the one host asked without verifying it.
const SONY_HOST: &str = "a0.ww.np.dl.playstation.net";
/// A Steam or Sony answer, or a store's list, is asked for again after this.
const FRESH_S: i64 = 4 * 3600;
/// titledb's list is 1.7 MB.
const TITLEDB_FRESH_S: i64 = 24 * 3600;
const AT_ONCE: usize = 8;

/// Where the requests go: the network, or (tests) saved answers by URL, any other URL a 404.
pub(crate) enum Net {
    Live,
    #[cfg(test)]
    Saved {
        answers: BTreeMap<String, Vec<u8>>,
        asked: std::sync::Mutex<Vec<String>>,
    },
}

pub(crate) static LIVE: Net = Net::Live;

struct Clients {
    verified: reqwest::Client,
    unverified: reqwest::Client,
}

impl Clients {
    fn build(tweak: impl Fn(reqwest::ClientBuilder) -> reqwest::ClientBuilder) -> reqwest::Result<Clients> {
        let base = || tweak(reqwest::Client::builder().timeout(Duration::from_secs(30)).user_agent("universe/0.1"));
        Ok(Clients {
            verified: base().build()?,
            // A redirect would carry the unverified client to another host.
            unverified: base().tls_danger_accept_invalid_certs(true).redirect(reqwest::redirect::Policy::none()).build()?,
        })
    }

    fn of(&self, url: &str) -> &reqwest::Client {
        match reqwest::Url::parse(url) {
            Ok(u) if u.scheme() == "https" && u.host_str() == Some(SONY_HOST) => &self.unverified,
            _ => &self.verified,
        }
    }

    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        let failed = |e: reqwest::Error| Error::Io(format!("{url}: {e}"));
        let resp = self.of(url).get(url).send().await.map_err(failed)?;
        if !resp.status().is_success() {
            return Err(Error::Io(format!("{url}: {}", resp.status())));
        }
        Ok(resp.bytes().await.map_err(failed)?.to_vec())
    }
}

fn clients() -> Result<&'static Clients> {
    static CLIENTS: OnceLock<std::result::Result<Clients, String>> = OnceLock::new();
    CLIENTS.get_or_init(|| Clients::build(|b| b).map_err(|e| e.to_string())).as_ref().map_err(|e| Error::Io(e.clone()))
}

impl Net {
    async fn get(&self, url: &str) -> Result<Vec<u8>> {
        match self {
            Net::Live => clients()?.get(url).await,
            #[cfg(test)]
            Net::Saved { answers, asked } => {
                asked.lock().unwrap().push(url.to_string());
                answers.get(url).cloned().ok_or_else(|| Error::Io(format!("{url}: 404 Not Found")))
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Steam {
    build: String,
    date: String,
    at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Sony {
    /// Empty when the title never got an update.
    version: String,
    at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Titles {
    /// Title version → the day it shipped; empty when titledb lists none.
    versions: BTreeMap<u64, String>,
    /// The time of the titledb copy it was read from.
    at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Store {
    /// The store's id of a game → its `update` row.
    rows: BTreeMap<String, serde_json::Map<String, Value>>,
    at: i64,
}

/// The answers, by Steam app id, Switch or PS3 title id, and store; kept in `upstream.json` under the cache.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Upstream {
    steam: BTreeMap<String, Steam>,
    switch: BTreeMap<String, Titles>,
    ps3: BTreeMap<String, Sony>,
    stores: BTreeMap<String, Store>,
    /// The sources on, whose games their store judges even before it answered.
    #[serde(skip)]
    sources: Vec<String>,
}

fn write_atomic(path: &Path, bytes: &[u8]) {
    // Two threads of one process may write at once.
    static WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), WRITES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let written = std::fs::create_dir_all(paths::cache_home()).and_then(|_| std::fs::write(&tmp, bytes));
    if written.and_then(|_| std::fs::rename(&tmp, path)).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

impl Upstream {
    fn file() -> PathBuf {
        paths::cache_home().join("upstream.json")
    }

    pub fn load() -> Upstream {
        std::fs::read(Self::file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        write_atomic(&Self::file(), &serde_json::to_vec(self).unwrap_or_default());
    }

    /// Whether `source`'s list is missing or older than a few hours.
    pub(crate) fn store_stale(&self, source: &str, now: i64) -> bool {
        self.stores.get(source).is_none_or(|s| now - s.at >= FRESH_S)
    }

    /// The `update` rows a store listed, by the store's id of each game.
    pub(crate) fn set_store(&mut self, source: &str, rows: impl IntoIterator<Item = serde_json::Map<String, Value>>, now: i64) {
        let rows = rows.into_iter().filter_map(|r| Some((r.get("id")?.as_str()?.to_string(), r))).collect();
        self.stores.insert(source.into(), Store { rows, at: now });
    }

    pub fn with_sources(mut self, active: &[String]) -> Upstream {
        self.stores.retain(|s, _| active.contains(s));
        self.sources = active.to_vec();
        self
    }

    /// Answers asked for since this was loaded, over it: the file may have changed in between.
    pub(crate) fn absorb(&mut self, fresh: Upstream) {
        self.steam.extend(fresh.steam);
        self.switch.extend(fresh.switch);
        self.ps3.extend(fresh.ps3);
        self.stores.extend(fresh.stores);
    }

    /// After the store updated `id`: its row says the published build is the one installed.
    pub(crate) fn applied(&mut self, source: &str, id: &str) -> bool {
        let Some(row) = self.stores.get_mut(source).and_then(|s| s.rows.get_mut(id)) else { return false };
        for (to, from) in [("local_build", "remote_build"), ("local_version", "version")] {
            if let Some(v) = row.get(from).cloned() {
                row.insert(to.into(), v);
            }
        }
        row.remove("skipped");
        row.insert("current".into(), Value::Bool(true));
        true
    }
}

#[derive(Debug, Default)]
pub(crate) struct Wants {
    steam: BTreeSet<u64>,
    switch: BTreeSet<String>,
    ps3: BTreeSet<String>,
    pub(crate) stores: BTreeSet<String>,
}

/// The store that judges a game: the one it came from, else GOG for an install its manifest names.
fn store_of<'a>(game: &'a Game, read: &'a Installed, active: &[String]) -> Option<(&'a str, &'a str)> {
    if !game.source.id.is_empty() && active.contains(&game.source.kind) {
        return Some((&game.source.kind, &game.source.id));
    }
    (read.channel == "gog" && !read.title_id.is_empty() && active.iter().any(|a| a == "gog")).then_some(("gog", read.title_id.as_str()))
}

pub(crate) fn wants<'a>(games: impl IntoIterator<Item = (&'a Game, &'a Installed)>, active: &[String]) -> Wants {
    let mut w = Wants::default();
    for (game, read) in games {
        if let Some((store, _)) = store_of(game, read, active) {
            w.stores.insert(store.into());
            continue;
        }
        match read.channel {
            "pc" if game.metadata.steam_appid != 0 => {
                w.steam.insert(game.metadata.steam_appid);
            }
            "switch" if !read.title_id.is_empty() => {
                w.switch.insert(read.title_id.clone());
            }
            "ps3" if !read.title_id.is_empty() => {
                w.ps3.insert(read.title_id.clone());
            }
            _ => {}
        }
    }
    w
}

/// Steam's public branch: its build id and the day its content last changed.
async fn steam_build(net: &Net, appid: u64) -> Option<Steam> {
    let body = net.get(&format!("{STEAM}{appid}")).await.map_err(|e| tracing::info!("steam: {e}")).ok()?;
    let v: Value = serde_json::from_slice(&body).ok()?;
    let public = &v["data"][appid.to_string()]["depots"]["branches"]["public"];
    let number = |k: &str| public[k].as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| public[k].as_i64());
    let build = public["buildid"].as_str().map(String::from).or_else(|| public["buildid"].as_u64().map(|b| b.to_string()))?;
    // `timeupdated` also moves for an edit of the store page alone.
    let date = number("timebuildupdated")
        .or_else(|| number("timeupdated"))
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    Some(Steam { build, date, at: 0 })
}

/// The highest `<package version="01.02">` of Sony's update feed for the title; an empty feed is no update ever shipped.
async fn sony_version(net: &Net, title_id: &str) -> Option<String> {
    static PACKAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<package\s[^>]*?\bversion="(\d+\.\d+)""#).unwrap());
    let body = net.get(&format!("{SONY}{title_id}/{title_id}-ver.xml")).await.map_err(|e| tracing::info!("sony: {e}")).ok()?;
    let text = String::from_utf8_lossy(&body);
    Some(PACKAGE.captures_iter(&text).map(|c| c[1].to_string()).max_by(|a, b| crate::versions::compare(a, b)).unwrap_or_default())
}

fn titledb_file() -> PathBuf {
    paths::cache_home().join("titledb-versions.json")
}

fn mtime_s(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}

/// titledb's versions of the wanted titles, from a copy downloaded again once a day old; `None` when the entries kept
/// already come from that copy, or there is no copy at all.
async fn titledb(net: &Net, kept: &BTreeMap<String, Titles>, wanted: &BTreeSet<String>, now: i64) -> Option<BTreeMap<String, Titles>> {
    if wanted.is_empty() {
        return None;
    }
    type List = BTreeMap<String, BTreeMap<String, String>>;
    let file = titledb_file();
    let mut downloaded: Option<List> = None;
    if mtime_s(&file).is_none_or(|at| now - at >= TITLEDB_FRESH_S) {
        match net.get(TITLEDB).await {
            Ok(body) => match serde_json::from_slice::<List>(&body) {
                Ok(list) => {
                    write_atomic(&file, &body);
                    downloaded = Some(list);
                }
                Err(e) => tracing::info!("titledb: {e}"),
            },
            Err(e) => tracing::info!("titledb: {e}"),
        }
    }
    let at = mtime_s(&file)?;
    if wanted.iter().all(|t| kept.get(t).is_some_and(|k| k.at == at)) {
        return None;
    }
    let list: List = match downloaded {
        Some(list) => list,
        None => serde_json::from_slice(&std::fs::read(&file).ok()?).ok()?,
    };
    Some(
        wanted
            .iter()
            .map(|t| {
                let versions = list.get(t).into_iter().flatten().filter_map(|(v, day)| Some((v.parse().ok()?, day.clone()))).collect();
                (t.clone(), Titles { versions, at })
            })
            .collect(),
    )
}

/// The answers missing from `kept` or older than their time, asked for again; one not given is left out.
pub(crate) async fn refresh(net: &Net, kept: &Upstream, wants: &Wants, now: i64) -> Upstream {
    let stale = |at: Option<i64>| at.is_none_or(|at| now - at >= FRESH_S);
    let steam: Vec<u64> = wants.steam.iter().copied().filter(|id| stale(kept.steam.get(&id.to_string()).map(|s| s.at))).collect();
    let ps3: Vec<&String> = wants.ps3.iter().filter(|id| stale(kept.ps3.get(*id).map(|s| s.at))).collect();
    let (steam, (ps3, switch)) = futures_util::future::join(
        stream::iter(steam).map(|id| async move { (id, steam_build(net, id).await) }).buffer_unordered(AT_ONCE).collect::<Vec<_>>(),
        futures_util::future::join(
            stream::iter(ps3).map(|id| async move { (id.clone(), sony_version(net, id).await) }).buffer_unordered(AT_ONCE).collect::<Vec<_>>(),
            titledb(net, &kept.switch, &wants.switch, now),
        ),
    )
    .await;
    Upstream {
        steam: steam.into_iter().filter_map(|(id, s)| Some((id.to_string(), Steam { at: now, ..s? }))).collect(),
        ps3: ps3.into_iter().filter_map(|(id, version)| Some((id, Sony { version: version?, at: now }))).collect(),
        switch: switch.unwrap_or_default(),
        ..Default::default()
    }
}

/// Upstream's newest build of a game updated by hand, asked for anew: Steam's public build id, titledb's newest title
/// version.
pub(crate) async fn newest(net: &Net, game: &Game, read: &Installed) -> Result<String> {
    let mut up = Upstream::load();
    let now = chrono::Utc::now().timestamp();
    let id = &game.id;
    let found = match read.channel {
        "pc" => {
            let appid = game.metadata.steam_appid;
            if appid == 0 {
                return Err(Error::Invalid(format!("{id} has no Steam app id: universe set {id} metadata.steam_appid=<id>")));
            }
            let s = steam_build(net, appid).await.ok_or_else(|| Error::Unavailable(format!("Steam gave no build of app {appid}")))?;
            let build = s.build.clone();
            up.steam.insert(appid.to_string(), Steam { at: now, ..s });
            build
        }
        "switch" => {
            let tid = &read.title_id;
            if tid.is_empty() {
                return Err(Error::Invalid(format!("{id} has no Switch title id: universe set {id} version.switch_title_id=<id>")));
            }
            if let Some(found) = titledb(net, &up.switch, &BTreeSet::from([tid.clone()]), now).await {
                up.switch.extend(found);
            }
            let titles = up.switch.get(tid).ok_or_else(|| Error::Unavailable("titledb could not be fetched".into()))?;
            titles.versions.keys().next_back().map(|v| v.to_string()).ok_or_else(|| Error::Invalid(format!("titledb lists no update of {tid}")))?
        }
        _ => return Err(Error::Invalid(format!("{id} has no upstream build to take"))),
    };
    up.save();
    Ok(found)
}

/// How a game's installed version stands against upstream's newest.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Check {
    /// `current`, `update`, `likely`, `unknown`, `final`, `untracked`; empty for a game no update channel reaches.
    pub status: &'static str,
    /// `no-answer`, `no-baseline`, `dates-rewritten`, `none-listed`, `not-owned`, or empty.
    pub reason: &'static str,
    /// `apply` when its store updates it, `manual` when it is updated by hand; empty when no update applies.
    pub how: &'static str,
    /// The source whose `update` judges it, and that store's id of the game.
    pub store: String,
    pub store_id: String,
    /// As upstream names it: the store's version name, Sony's version, titledb's title version, Steam's build id.
    pub latest: String,
    pub latest_date: String,
    /// Upstream's builds past the installed one: titledb's versions above it; 1 for a newer store, Steam or Sony build.
    pub newer: usize,
    /// The store's name of the installed build, which `installed` shows over what the files say.
    #[serde(skip)]
    pub version_name: String,
}

fn manual(status: &'static str, reason: &'static str) -> Check {
    Check { status, reason, how: "manual", ..Default::default() }
}

/// A game's status by the answers kept: its store's list, else its runner's channel; `version.final` holds it at final.
pub fn check(game: &Game, read: &Installed, up: &Upstream) -> Check {
    let mut c = match store_of(game, read, &up.sources) {
        Some((store, id)) => match up.stores.get(store) {
            Some(list) => by_store(game, read, list, store, id),
            None => Check { store: store.into(), store_id: id.into(), ..manual("unknown", "no-answer") },
        },
        None => match read.channel {
            "pc" => by_steam(game, read, up),
            "switch" => by_titledb(game, read, up),
            "ps3" => by_sony(read, up),
            "gog" => manual("unknown", "no-answer"),
            "final" => Check { status: "final", ..Default::default() },
            _ => Check::default(),
        },
    };
    if game.version.r#final {
        (c.status, c.reason, c.how) = ("final", "", "");
    }
    c
}

fn by_store(game: &Game, read: &Installed, list: &Store, store: &str, id: &str) -> Check {
    let (store, store_id) = (store.to_string(), id.to_string());
    let Some(row) = list.rows.get(id) else {
        // A store lists the installs it looks after; GOG may not reach an install it was not asked to look in.
        let own = game.source.kind == store;
        return Check { store, store_id, ..if own { Check { status: "current", how: "apply", ..Default::default() } } else { manual("unknown", "no-answer") } };
    };
    let text = |k: &str| row.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let local = if read.build.is_empty() { text("local_build") } else { read.build.clone() };
    // A store's build labels may match across an update (an itch upload changed in place): only its word or the files say current.
    let current = if read.build.is_empty() { row.get("current") == Some(&Value::Bool(true)) } else { read.build == text("remote_build") };
    let skipped = !text("skipped").is_empty();
    Check {
        status: if current { "current" } else { "update" },
        reason: if skipped && !current { "not-owned" } else { "" },
        how: if skipped { "manual" } else { "apply" },
        store,
        store_id,
        latest: text("version"),
        latest_date: text("date"),
        newer: usize::from(!current),
        version_name: if text("local_build") == local { text("local_version") } else { String::new() },
    }
}

fn by_steam(game: &Game, read: &Installed, up: &Upstream) -> Check {
    let appid = game.metadata.steam_appid;
    if appid == 0 {
        return manual("untracked", "");
    }
    let Some(s) = up.steam.get(&appid.to_string()) else { return manual("unknown", "no-answer") };
    let seen = &game.version.seen;
    let (status, reason) = if *seen == s.build {
        ("current", "")
    } else if !seen.is_empty() {
        ("update", "")
    } else if read.modified.is_empty() || s.date.is_empty() {
        ("unknown", "no-baseline")
    // Steam builds a branch after the program in it was compiled: a program dated after the build had its dates rewritten.
    } else if read.modified < s.date {
        ("likely", "")
    } else {
        ("unknown", "dates-rewritten")
    };
    Check { latest: s.build.clone(), latest_date: s.date.clone(), newer: usize::from(!seen.is_empty() && *seen != s.build), ..manual(status, reason) }
}

/// The title version the emulator or the ROM's name gives, else `seen`: a release's own version name and the title
/// version it carries are set by different people.
fn by_titledb(game: &Game, read: &Installed, up: &Upstream) -> Check {
    if read.title_id.is_empty() {
        return manual("untracked", "");
    }
    let have: Option<u64> = read.build.parse().ok().or_else(|| game.version.seen.parse().ok());
    let Some(titles) = up.switch.get(&read.title_id) else { return manual("unknown", "no-answer") };
    let Some((&latest, date)) = titles.versions.last_key_value() else { return manual("current", "none-listed") };
    let (status, reason) = match have {
        None => ("unknown", "no-baseline"),
        Some(h) if h >= latest => ("current", ""),
        Some(_) => ("update", ""),
    };
    let newer = have.map_or(0, |h| titles.versions.keys().filter(|&&v| v > h).count());
    Check { latest: latest.to_string(), latest_date: date.clone(), newer, ..manual(status, reason) }
}

fn by_sony(read: &Installed, up: &Upstream) -> Check {
    if read.title_id.is_empty() {
        return manual("untracked", "");
    }
    let Some(s) = up.ps3.get(&read.title_id) else { return manual("unknown", "no-answer") };
    if s.version.is_empty() {
        return manual("current", "none-listed");
    }
    if read.installed.is_empty() {
        return Check { latest: s.version.clone(), ..manual("unknown", "no-baseline") };
    }
    let newer = crate::versions::compare(&s.version, &read.installed).is_gt();
    Check { latest: s.version.clone(), newer: usize::from(newer), ..manual(if newer { "update" } else { "current" }, "") }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;
    use crate::core::Core;
    use crate::testpe::Exe;

    /// 2026-09-11 at noon UTC: the same day in any zone the tests run in.
    const SHIPPED: i64 = 1_789_128_000;

    fn saved(answers: Vec<(String, Vec<u8>)>) -> Net {
        Net::Saved { answers: answers.into_iter().collect(), asked: Default::default() }
    }

    fn asked(net: &Net) -> Vec<String> {
        match net {
            Net::Saved { asked, .. } => asked.lock().unwrap().clone(),
            Net::Live => unreachable!(),
        }
    }

    fn steam(appid: u64, build: &str) -> (String, Vec<u8>) {
        let public = serde_json::json!({"buildid": build, "timebuildupdated": SHIPPED.to_string(), "timeupdated": (SHIPPED + 86_400 * 3).to_string()});
        (
            format!("{STEAM}{appid}"),
            serde_json::json!({"data": {appid.to_string(): {"depots": {"branches": {"public": public}}}}, "status": "success"}).to_string().into_bytes(),
        )
    }

    fn file(path: &Path, bytes: &[u8]) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        path.to_path_buf()
    }

    fn program(path: &Path, day: &str) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        Exe { strings: &[("ProductVersion", "1.0.3")], ..Default::default() }.write(path);
        let t = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").unwrap().and_hms_opt(12, 0, 0).unwrap().and_local_timezone(chrono::Local).unwrap();
        std::fs::File::options().write(true).open(path).unwrap().set_modified(t.into()).unwrap();
        path.to_path_buf()
    }

    fn game(title: &str, runner: &str, exe: &Path, edit: impl FnOnce(&mut Game)) {
        let mut g = Game::new(title);
        g.launch.runner = runner.into();
        g.launch.exe = exe.to_string_lossy().into();
        edit(&mut g);
        g.save().unwrap();
    }

    async fn open() -> Core {
        Core::open_with(crate::config::Config::load().unwrap(), crate::host::Host::memory().0).await.unwrap()
    }

    /// Each game's `version` by id.
    async fn judged(core: &Core) -> BTreeMap<String, Value> {
        core.list().await.into_iter().map(|g| (g["id"].as_str().unwrap().to_string(), g["version"].clone())).collect()
    }

    fn of<'a>(v: &'a BTreeMap<String, Value>, id: &str) -> (&'a str, &'a str, &'a str, &'a str) {
        let f = |k: &str| v[id][k].as_str().unwrap_or_else(|| panic!("{id}.{k}: {}", v[id]));
        (f("status"), f("reason"), f("how"), f("latest"))
    }

    #[tokio::test]
    async fn a_pc_game_stands_by_steams_build_its_seen_build_else_its_programs_date() {
        let env = paths::test_env();
        let games = env.path().join("games");
        let old = program(&games.join("Old/old.exe"), "2020-01-01");
        let new = program(&games.join("New/new.exe"), "2026-10-01");
        game("Untracked", "proton", &old, |_| {});
        game("Current", "proton", &old, |g| (g.metadata.steam_appid, g.version.seen) = (1, "100".into()));
        game("Behind", "wine", &old, |g| (g.metadata.steam_appid, g.version.seen) = (2, "99".into()));
        game("Held", "proton", &old, |g| (g.metadata.steam_appid, g.version.seen, g.version.r#final) = (2, "99".into(), true));
        game("Likely", "proton", &old, |g| g.metadata.steam_appid = 3);
        game("Rewritten", "proton", &new, |g| g.metadata.steam_appid = 3);
        game("Missing", "proton", &games.join("Gone/gone.exe"), |g| g.metadata.steam_appid = 3);
        game("Silent", "proton", &old, |g| g.metadata.steam_appid = 4);
        let net = saved(vec![steam(1, "100"), steam(2, "200"), steam(3, "300")]);
        let core = open().await;
        assert_eq!(of(&judged(&core).await, "behind"), ("unknown", "no-answer", "manual", ""), "nothing asked at startup");
        core.check_versions_with(&net, "").await.unwrap();
        let v = judged(&core).await;
        assert_eq!(of(&v, "untracked"), ("untracked", "", "manual", ""));
        assert_eq!(of(&v, "current"), ("current", "", "manual", "100"));
        assert_eq!(of(&v, "behind"), ("update", "", "manual", "200"));
        assert_eq!(
            (v["behind"]["latest_date"].as_str(), v["behind"]["installed"].as_str()),
            (Some("2026-09-11"), Some("1.0.3")),
            "the build's day, not the store page's"
        );
        assert_eq!((of(&v, "held"), v["held"]["newer"].as_u64()), (("final", "", "", "200"), Some(1)));
        assert_eq!(of(&v, "likely"), ("likely", "", "manual", "300"), "a program older than Steam's build");
        assert_eq!(of(&v, "rewritten"), ("unknown", "dates-rewritten", "manual", "300"));
        assert_eq!(of(&v, "missing"), ("unknown", "no-baseline", "manual", "300"));
        assert_eq!(of(&v, "silent"), ("unknown", "no-answer", "manual", ""));
        let urls = [1, 2, 3, 4].map(|id| format!("{STEAM}{id}"));
        assert_eq!(asked(&net), urls, "an app id is asked once");

        core.check_versions_with(&net, "").await.unwrap();
        assert_eq!(asked(&net)[4..], [format!("{STEAM}4")], "a few hours old, an answer is kept; a missing one is asked again");
        assert_eq!(judged(&open().await).await, v, "a new process judges by the answers kept");

        assert_eq!(core.mark_current_with(&net, "behind", "").await.unwrap(), "200", "Steam asked anew");
        assert_eq!(asked(&net).last().unwrap(), &format!("{STEAM}2"));
        assert_eq!(core.mark_current_with(&net, "likely", "300").await.unwrap(), "300");
        let v = judged(&core).await;
        assert_eq!((of(&v, "behind").0, of(&v, "likely").0, v["likely"]["seen"].as_str()), ("current", "current", Some("300")));
        assert!(core.mark_current_with(&net, "untracked", "").await.is_err(), "no app id to ask Steam about");
        assert!(core.mark_current_with(&net, "likely", "v1.2").await.is_err());
    }

    #[tokio::test]
    async fn a_switch_game_stands_by_titledbs_versions_and_its_emulators_title_version_else_seen() {
        let env = paths::test_env();
        let roms = env.path().join("roms");
        let rom = |name: &str| file(&roms.join(name), b"");
        game("Galaxy", "eden", &rom("Super Mario Galaxy 2 [010000000000a000][v131072].nsp"), |_| {});
        game("Held", "eden", &rom("Galaxy Again [010000000000a000][v65536].nsp"), |g| g.version.r#final = true);
        game("Assumed", "ryujinx", &rom("Xenoblade.xci"), |g| (g.version.switch_title_id, g.version.seen) = ("010000000000b000".into(), "262144".into()));
        game("Baseline", "eden", &rom("Wonder [010000000000c000].nsp"), |_| {});
        game("Unlisted", "eden", &rom("Prime [010000000000d000].nsp"), |_| {});
        game("Untracked", "eden", &rom("Strikers.nsp"), |_| {});
        let titledb = serde_json::json!({
            "010000000000a000": {"65536": "2017-10-26", "131072": "2017-11-30", "196608": "2018-02-22", "393216": "2026-05-13"},
            "010000000000b000": {"131072": "2020-05-26", "262144": "2026-06-09"},
            "010000000000c000": {"65536": "2023-11-01"},
        });
        let net = saved(vec![(TITLEDB.into(), titledb.to_string().into_bytes())]);
        let core = open().await;
        core.check_versions_with(&net, "").await.unwrap();
        let v = judged(&core).await;
        assert_eq!(of(&v, "galaxy"), ("update", "", "manual", "393216"));
        assert_eq!((v["galaxy"]["latest_date"].as_str(), v["galaxy"]["newer"].as_u64()), (Some("2026-05-13"), Some(2)));
        assert_eq!((of(&v, "held").0, v["held"]["newer"].as_u64()), ("final", Some(3)), "final, and still counting what shipped since");
        assert_eq!(of(&v, "assumed"), ("current", "", "manual", "262144"), "no title version from the emulator: seen stands for it");
        assert_eq!(of(&v, "baseline"), ("unknown", "no-baseline", "manual", "65536"));
        assert_eq!(of(&v, "unlisted"), ("current", "none-listed", "manual", ""));
        assert_eq!(of(&v, "untracked"), ("untracked", "", "manual", ""));

        core.check_versions_with(&net, "").await.unwrap();
        assert_eq!(asked(&net), [TITLEDB], "titledb's list is downloaded once a day");
        assert_eq!(core.mark_current_with(&net, "baseline", "").await.unwrap(), "65536", "titledb's newest version of it");
        assert_eq!(of(&judged(&core).await, "baseline").0, "current");
        assert_eq!(asked(&net), [TITLEDB], "from the day's copy");
        assert!(core.mark_current_with(&net, "galaxy", "").await.is_err(), "its ROM's name gives its title version");
    }

    #[tokio::test]
    async fn a_ps3_game_stands_by_sonys_update_feed() {
        let env = paths::test_env();
        let disc = |name: &str, title_id: &str, version: &str| {
            let dir = env.path().join("ps3").join(name).join("PS3_GAME");
            file(&dir.join("PARAM.SFO"), &crate::roms::tests::sfo_of(&[("TITLE_ID", title_id), ("APP_VER", version)]));
            file(&dir.join("USRDIR/EBOOT.BIN"), b"")
        };
        game("Bolt", "rpcs3", &disc("Bolt", "BLES00416", "01.00"), |_| {});
        game("Metal Gear", "rpcs3", &disc("MGS4", "BLUS30109", "01.00"), |_| {});
        game("Last of Us", "rpcs3", &disc("TLOU", "BCUS98174", "01.11"), |_| {});
        game("Simpsons", "rpcs3", &disc("Simpsons", "BLES00142", "01.00"), |_| {});
        let feed = |packages: &str| {
            format!("<?xml version='1.0' encoding='UTF-8'?>\n<titlepatch status=\"alive\"><tag name=\"T0\">{packages}</tag></titlepatch>").into_bytes()
        };
        let url = |tid: &str| format!("https://a0.ww.np.dl.playstation.net/tpl/np/{tid}/{tid}-ver.xml");
        let net = saved(vec![
            (url("BLES00416"), Vec::new()),
            (url("BLUS30109"), feed(r#"<package version="01.02" size="1"/><package version="02.00" size="2" ps3_system_ver="04.0000"/>"#)),
            (url("BCUS98174"), feed(r#"<package version="01.11" size="3"/>"#)),
        ]);
        let core = open().await;
        core.check_versions_with(&net, "").await.unwrap();
        let v = judged(&core).await;
        assert_eq!(of(&v, "bolt"), ("current", "none-listed", "manual", ""), "an empty feed: no update ever shipped");
        assert_eq!(of(&v, "metal-gear"), ("update", "", "manual", "02.00"));
        assert_eq!(of(&v, "last-of-us"), ("current", "", "manual", "01.11"));
        assert_eq!(of(&v, "simpsons"), ("unknown", "no-answer", "manual", ""));
        assert!(core.mark_current_with(&net, "bolt", "1").await.is_err(), "its files say which update it has");
    }

    #[tokio::test]
    async fn a_store_judges_its_games_and_a_gog_install_its_manifest_names() {
        let env = paths::test_env();
        let games = env.path().join("games");
        let manifest = |dir: &str, id: &str, build: &str| {
            file(&games.join(dir).join(format!("goggame-{id}.info")), format!(r#"{{"gameId":"{id}","rootGameId":"{id}","buildId":"{build}"}}"#).as_bytes())
        };
        let exe = |dir: &str| program(&games.join(dir).join("game.exe"), "2024-01-01");
        manifest("Dead Cells", "1237807960", "B2");
        game("Dead Cells", "proton", &exe("Dead Cells"), |g| g.source.kind = "lutris".into());
        manifest("Cuphead", "1963513391", "C1");
        game("Cuphead", "proton", &exe("Cuphead"), |g| g.source.kind = "lutris".into());
        manifest("Control", "2049187585", "K1");
        game("Control", "proton", &exe("Control"), |g| g.source.kind = "lutris".into());
        let mini = manifest("Mini Metro", "1434554947", "M1");
        game("Mini Metro", "proton", &exe("Mini Metro"), |g| {
            (g.source.kind, g.source.id, g.source.dir) = ("gog".into(), "1434554947".into(), games.join("Mini Metro").to_string_lossy().into());
        });
        game("Witcher", "proton", &exe("Witcher"), |g| (g.source.kind, g.source.id) = ("gog".into(), "20".into()));
        game("Hades", "proton", &exe("Hades"), |g| (g.source.kind, g.source.id) = ("gog".into(), "30".into()));
        let row = |id: &str, local: &str, remote: &str, version: &str, local_version: &str, extra: &str| {
            format!(
                r#"echo '{{"event":"update","id":"{id}","title":"x","local_build":"{local}","remote_build":"{remote}","version":"{version}","date":"2025-10-03","local_version":"{local_version}"{extra}}}'"#
            )
        };
        let listed = [
            row("1237807960", "B2", "B2", "1.25.3", "1.25.3", r#","current":true"#),
            row("1963513391", "C1", "C2", "1.3.9", "", r#","skipped":"not owned""#),
            row("1434554947", "M1", "M2", "1.1", "1.0", ""),
            row("30", "upload 7", "upload 7", "1.1", "", ""),
        ]
        .join("; ");
        let event = format!(
            r#"{{"event":"game","id":"1434554947","title":"Mini Metro","owned":true,"installed":true,"dir":"{}","exe":"game.exe","build":"M2"}}"#,
            games.join("Mini Metro").display()
        );
        let script = format!(
            "#!/bin/sh\ncase \"$1\" in\nupdate) if [ -n \"$2\" ]; then sed -i s/M1/M2/ '{}'; echo '{event}'; else echo \"$1\" >> \"$SOURCE_DATA_DIR/asked\"; {listed}; fi ;;\nesac\necho '{{\"event\":\"done\"}}'\n",
            mini.display()
        );
        let src = env.path().join("sources/gog");
        file(&src.join("source.toml"), b"api = 2\nid = \"gog\"\nname = \"GOG\"\nexe = \"run\"\n");
        file(&src.join("run"), script.as_bytes());
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(src.join("run"), std::fs::Permissions::from_mode(0o755)).unwrap();
        file(&env.path().join("config/config.toml"), b"[modules]\nenabled = []\n[sources]\nenabled = [\"gog\"]\n");
        let net = saved(Vec::new());
        let core = open().await;
        let before = judged(&core).await;
        assert_eq!(
            (of(&before, "witcher"), before["witcher"]["store"].as_str()),
            (("unknown", "no-answer", "manual", ""), Some("gog")),
            "its store on, not asked yet"
        );
        core.check_versions_with(&net, "").await.unwrap();
        let v = judged(&core).await;
        assert_eq!(of(&v, "dead-cells"), ("current", "", "apply", "1.25.3"));
        assert_eq!(of(&v, "hades"), ("update", "", "apply", "1.1"), "a store's build labels may match across an update: only its word says current");
        assert_eq!((v["dead-cells"]["installed"].as_str(), v["dead-cells"]["from"].as_str()), (Some("1.25.3"), Some("gog")), "the store's name of the build");
        assert_eq!((v["dead-cells"]["store"].as_str(), v["dead-cells"]["store_id"].as_str()), (Some("gog"), Some("1237807960")));
        assert_eq!(of(&v, "cuphead"), ("update", "not-owned", "manual", "1.3.9"));
        assert_eq!(of(&v, "mini-metro"), ("update", "", "apply", "1.1"));
        assert_eq!(v["mini-metro"]["installed"], "1.0");
        assert_eq!(of(&v, "witcher"), ("current", "", "apply", ""), "its own store lists only what it would update");
        assert_eq!(of(&v, "control"), ("unknown", "no-answer", "manual", ""), "an install the store did not see");
        assert!(asked(&net).is_empty());

        core.source_update("gog", "1434554947", None).await.unwrap();
        let v = judged(&core).await;
        assert_eq!((of(&v, "mini-metro").0, v["mini-metro"]["installed"].as_str()), ("current", Some("1.1")), "updated, it is at the build listed");
        core.check_versions_with(&net, "").await.unwrap();
        let lists = std::fs::read_to_string(paths::sources_data_dir("gog").join("asked")).unwrap();
        assert_eq!(lists.lines().count(), 1, "a store's list is kept a few hours too");
        assert!(core.mark_current_with(&net, "cuphead", "1").await.is_err(), "its manifest says its build");
    }

    #[tokio::test]
    async fn only_sonys_host_is_asked_without_verifying_its_certificate() {
        use tokio_rustls::rustls;
        let cert = rustls::pki_types::CertificateDer::from(include_bytes!("../tests/fixtures/tls/cert.der").to_vec());
        let key = rustls::pki_types::PrivateKeyDer::Pkcs8(include_bytes!("../tests/fixtures/tls/key.der").to_vec().into());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let Ok(mut tls) = acceptor.accept(tcp).await else { return };
                    let mut request = vec![0; 4096];
                    let n = tls.read(&mut request).await.unwrap_or(0);
                    let moved = String::from_utf8_lossy(&request[..n]).starts_with("GET /moved");
                    let answer = if moved {
                        format!("HTTP/1.1 302 Found\r\nLocation: https://api.steamcmd.net:{port}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    } else {
                        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string()
                    };
                    let _ = tls.write_all(answer.as_bytes()).await;
                    let _ = tls.shutdown().await;
                });
            }
        });
        let lookalike = format!("{SONY_HOST}.example");
        let clients = Clients::build(|b| b.resolve(SONY_HOST, addr).resolve("api.steamcmd.net", addr).resolve(&lookalike, addr)).unwrap();
        assert_eq!(clients.get(&format!("https://{SONY_HOST}:{port}/tpl/np/BLES00416/BLES00416-ver.xml")).await.unwrap(), b"ok");
        for url in [
            format!("https://api.steamcmd.net:{port}/v1/info/271590"),
            format!("https://{lookalike}:{port}/"),
            format!("https://{SONY_HOST}:{port}@api.steamcmd.net:{port}/"),
            format!("https://{SONY_HOST}:{port}/moved"),
        ] {
            assert!(clients.get(&url).await.is_err(), "{url} is verified, or not followed");
        }
    }
}
