use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::game::Game;
use crate::library::{is_image, media_dirs, scan_media_dir, stems_of, MEDIA_SLOTS};

const SGDB: &str = "https://www.steamgriddb.com/api/v2";
const RAWG: &str = "https://api.rawg.io/api";
const STEAM_APPDETAILS: &str = "https://store.steampowered.com/api/appdetails";
const SGDB_PLAN: [(&str, &str, Option<&str>); 5] = [
    ("box_front", "grids", Some("600x900")),
    ("square", "grids", Some("1024x1024,512x512")),
    ("tile", "grids", Some("920x430,460x215")),
    ("background", "heroes", None),
    ("logo", "logos", None),
];
const IMAGE_EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Candidate {
    pub provider: String,
    pub id: u64,
    pub url: String,
    pub thumb: String,
    pub score: i64,
    pub slot: String,
}

/// One page of a slot's candidates; `more` says whether the provider has another page.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidatePage {
    pub items: Vec<Candidate>,
    pub page: u32,
    pub more: bool,
}

/// A provider's game, from a search by name: what a pin points at.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Hit {
    pub provider: String,
    pub id: u64,
    pub name: String,
    pub year: u32,
    pub verified: bool,
    pub current: bool,
}

/// Where a slot stands: the file the UI shows, the fetched default under it, the override over it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlotStatus {
    pub slot: String,
    pub path: String,
    pub default: String,
    #[serde(rename = "override")]
    pub override_path: String,
    /// `picked` for an override, else the provider that wrote the default (`sgdb`, `pegasus`), or empty.
    pub origin: String,
    /// The provider that wrote the default, whether or not an override sits over it.
    pub default_origin: String,
    /// picked | fetched | guessed | missing
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShotsStatus {
    pub count: usize,
    pub override_count: usize,
    pub origin: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaStatus {
    pub id: String,
    pub title: String,
    pub sgdb_id: u64,
    pub slots: Vec<SlotStatus>,
    pub screenshots: ShotsStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct SyncCache {
    sgdb_id: u64,
    rawg_id: u64,
    steam_appid: u64,
    rawg_miss: bool,
    sgdb_miss: bool,
    fetched_at: String,
    /// slot (or `screenshots`) → the provider that wrote it into media/.
    sources: BTreeMap<String, String>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(20)).user_agent("universe/0.1").build().expect("client")
}

fn get_json(url: &str, bearer: Option<&str>) -> crate::Result<serde_json::Value> {
    let c = client();
    let mut req = c.get(url);
    if let Some(b) = bearer {
        req = req.bearer_auth(b);
    }
    let resp = req.send().map_err(|e| crate::Error::Io(format!("{url}: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(crate::Error::Io(format!("{url}: HTTP {status}")));
    }
    resp.json().map_err(|e| crate::Error::Io(format!("{url}: {e}")))
}

fn download(url: &str, dest: &Path) -> crate::Result<()> {
    let bytes = client().get(url).send().and_then(|r| r.error_for_status()).and_then(|r| r.bytes()).map_err(|e| crate::Error::Io(format!("{url}: {e}")))?;
    if bytes.is_empty() {
        return Err(crate::Error::Io(format!("{url}: empty image")));
    }
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(dest, &bytes)?;
    Ok(())
}

fn name_key(name: &str) -> String {
    let s: String = crate::slug::slug(name).replace('-', " ");
    s
}

fn released_year(date: &str) -> Option<u32> {
    date.get(0..4).and_then(|y| y.parse().ok())
}

fn epoch_year(epoch: i64) -> u32 {
    chrono::DateTime::from_timestamp(epoch, 0).map(|d| d.format("%Y").to_string()).and_then(|y| y.parse::<u32>().ok()).unwrap_or(0)
}

/// SteamGridDB's autocomplete for a title: the hits as they come, with their release year.
pub fn sgdb_hits(key: &str, title: &str) -> crate::Result<Vec<Hit>> {
    let url = format!("{SGDB}/search/autocomplete/{}", urlencoding::encode(title));
    let v = get_json(&url, Some(key))?;
    Ok(v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            Some(Hit {
                provider: "sgdb".into(),
                id: r["id"].as_u64()?,
                name: r["name"].as_str().unwrap_or("").to_string(),
                year: r["release_date"].as_i64().map(epoch_year).unwrap_or(0),
                verified: r["verified"].as_bool().unwrap_or(false),
                current: false,
            })
        })
        .collect())
}

/// SteamGridDB search: exact-name twins are disambiguated by release year (pegasus-sync rule).
pub fn sgdb_search(key: &str, title: &str, year: u32) -> crate::Result<Option<u64>> {
    let hits = sgdb_hits(key, title)?;
    let Some(head) = hits.first() else { return Ok(None) };
    let name = head.name.to_lowercase();
    let twins: Vec<&Hit> = hits.iter().filter(|h| h.name.to_lowercase() == name).collect();
    if year > 0 && twins.len() > 1 {
        if let Some(h) = twins.iter().find(|h| h.year == year) {
            return Ok(Some(h.id));
        }
    }
    Ok(Some(head.id))
}

fn sgdb_assets(key: &str, endpoint: &str, game_id: u64, dims: Option<&str>, page: u32) -> crate::Result<(Vec<Candidate>, bool)> {
    let mut url = format!("{SGDB}/{endpoint}/game/{game_id}?types=static,animated&page={page}");
    if let Some(d) = dims {
        url.push_str(&format!("&dimensions={d}"));
    }
    let v = match get_json(&url, Some(key)) {
        Ok(v) => v,
        Err(e) if e.to_string().contains("HTTP 404") => return Ok((vec![], false)),
        Err(e) => return Err(e),
    };
    let mut rows: Vec<Candidate> = v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|r| r["language"].as_str().unwrap_or("en") == "en" && !r["nsfw"].as_bool().unwrap_or(false))
        .map(|r| Candidate {
            provider: "sgdb".into(),
            id: r["id"].as_u64().unwrap_or(0),
            url: r["url"].as_str().unwrap_or("").into(),
            thumb: r["thumb"].as_str().unwrap_or("").into(),
            score: r["upvotes"].as_i64().unwrap_or(0) * 1000 + r["score"].as_i64().unwrap_or(0),
            slot: String::new(),
        })
        .filter(|c| !c.url.is_empty())
        .collect();
    rows.sort_by_key(|c| std::cmp::Reverse(c.score));
    // The language filter thins a page unevenly, so "more" counts the provider's unfiltered rows.
    let seen = (v["page"].as_u64().unwrap_or(page as u64) + 1) * v["limit"].as_u64().unwrap_or(50);
    let more = seen < v["total"].as_u64().unwrap_or(0);
    Ok((rows, more))
}

fn sgdb_steam_appid(key: &str, game_id: u64) -> Option<u64> {
    let v = get_json(&format!("{SGDB}/games/id/{game_id}?platformdata=steam"), Some(key)).ok()?;
    v["data"]["external_platform_data"]["steam"].as_array()?.first()?["id"].as_str().and_then(|s| s.parse().ok()).or_else(|| v["data"]["external_platform_data"]["steam"][0]["id"].as_u64())
}

pub fn rawg_search(key: &str, title: &str, year: u32) -> crate::Result<Option<u64>> {
    let url = format!("{RAWG}/games?search={}&search_precise=true&page_size=5&key={key}", urlencoding::encode(title));
    let v = get_json(&url, None)?;
    let want = name_key(title);
    let mut loose = None;
    for hit in v["results"].as_array().cloned().unwrap_or_default() {
        let got = name_key(hit["name"].as_str().unwrap_or(""));
        let hy = hit["released"].as_str().and_then(released_year);
        let same_year = year == 0 || hy.map(|y| (y as i64 - year as i64).abs() <= 1).unwrap_or(true);
        if got == want && same_year {
            return Ok(hit["id"].as_u64());
        }
        if loose.is_none() && (got.contains(&want) || want.contains(&got)) {
            loose = hit["id"].as_u64();
        }
    }
    Ok(loose)
}

fn strip_html(s: &str) -> String {
    let re = regex::Regex::new(r"(?i)</p>|<br\s*/?>").unwrap();
    let t = re.replace_all(s, "\n");
    let re2 = regex::Regex::new(r"<[^>]+>").unwrap();
    let t = re2.replace_all(&t, "");
    t.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").trim().to_string()
}

pub fn rawg_details(key: &str, id: u64) -> crate::Result<serde_json::Value> {
    get_json(&format!("{RAWG}/games/{id}?key={key}"), None)
}

pub fn steam_screenshots(appid: u64) -> crate::Result<Vec<String>> {
    let v = get_json(&format!("{STEAM_APPDETAILS}?appids={appid}&l=english"), None)?;
    let data = &v[appid.to_string()]["data"];
    Ok(data["screenshots"].as_array().cloned().unwrap_or_default().iter().filter_map(|s| s["path_full"].as_str().map(|s| s.to_string())).collect())
}

fn read_cache_in(media_dir: &Path) -> SyncCache {
    std::fs::read_to_string(media_dir.join(".sync.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write_cache_in(media_dir: &Path, c: &SyncCache) -> crate::Result<()> {
    std::fs::create_dir_all(media_dir)?;
    std::fs::write(media_dir.join(".sync.json"), serde_json::to_string_pretty(c)?)?;
    Ok(())
}

fn read_cache(game: &Game) -> SyncCache {
    read_cache_in(&game.media_dir())
}

fn write_cache(game: &Game, c: &SyncCache) -> crate::Result<()> {
    write_cache_in(&game.media_dir(), c)
}

/// Records who wrote `<media_dir>/<slot>` (the migration's Pegasus import).
pub fn note_source(media_dir: &Path, slot: &str, provider: &str) -> crate::Result<()> {
    let mut cache = read_cache_in(media_dir);
    cache.sources.insert(slot.into(), provider.into());
    write_cache_in(media_dir, &cache)
}

fn ext_of(url: &str) -> &'static str {
    let lower = url.to_lowercase();
    if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".jpg") || lower.contains(".jpeg") {
        "jpg"
    } else {
        "png"
    }
}

/// Removes every file a slot could be read from in `dir`, under any of its stems and extensions.
fn clear_slot(dir: &Path, slot: &str) -> bool {
    let mut gone = false;
    for stem in stems_of(slot) {
        for e in IMAGE_EXTS {
            if std::fs::remove_file(dir.join(format!("{stem}.{e}"))).is_ok() {
                gone = true;
            }
        }
    }
    gone
}

fn override_dir(config: &Config, game: &Game) -> PathBuf {
    config.overrides_dir().join(&game.id)
}

/// The override directories a game reads: its own and, for a migrated game, its Lutris slug's.
fn override_dirs(config: &Config, game: &Game) -> Vec<PathBuf> {
    let mut dirs = media_dirs(game, &config.overrides_dir());
    dirs.pop();
    dirs
}

/// Fills missing slots and metadata from SteamGridDB, RAWG and Steam into media/; an override
/// over a slot does not stop its default from being fetched. Pins in game.toml win. Returns true
/// if anything changed.
pub fn refresh(config: &Config, game: &Game, force: bool) -> crate::Result<bool> {
    let mut changed = false;
    let mut cache = read_cache(game);
    let mut g = Game::load(&game.toml_path())?;
    let sgdb_key = config.api_key("sgdb");
    let rawg_key = config.api_key("rawg");

    if let Some(key) = &sgdb_key {
        let mut sgdb_id = g.metadata.sgdb_id;
        if sgdb_id == 0 && !cache.sgdb_miss {
            sgdb_id = if cache.sgdb_id > 0 { cache.sgdb_id } else { sgdb_search(key, &g.title, g.release_year)?.unwrap_or(0) };
            if sgdb_id == 0 {
                cache.sgdb_miss = true;
            }
        }
        if sgdb_id > 0 {
            cache.sgdb_id = sgdb_id;
            let (have, _) = scan_media_dir(&g.media_dir());
            for (slot, endpoint, dims) in SGDB_PLAN {
                if !force && have.iter().any(|(s, _)| s == slot) {
                    continue;
                }
                let (list, _) = sgdb_assets(key, endpoint, sgdb_id, dims, 0)?;
                if let Some(c) = list.first() {
                    let dest = g.media_dir().join(format!("{slot}.{}", ext_of(&c.url)));
                    clear_slot(&g.media_dir(), slot);
                    download(&c.url, &dest)?;
                    cache.sources.insert(slot.into(), "sgdb".into());
                    changed = true;
                }
            }
            if g.metadata.steam_appid == 0 && cache.steam_appid == 0 {
                cache.steam_appid = sgdb_steam_appid(key, sgdb_id).unwrap_or(0);
            }
        }
    }

    if let Some(key) = &rawg_key {
        let mut rawg_id = g.metadata.rawg_id;
        if rawg_id == 0 && !cache.rawg_miss {
            rawg_id = if cache.rawg_id > 0 { cache.rawg_id } else { rawg_search(key, &g.title, g.release_year)?.unwrap_or(0) };
            if rawg_id == 0 {
                cache.rawg_miss = true;
            }
        }
        if rawg_id > 0 && (force || g.metadata.description.is_empty() || g.metadata.genres.is_empty()) {
            cache.rawg_id = rawg_id;
            let d = rawg_details(key, rawg_id)?;
            let desc = d["description_raw"].as_str().map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| strip_html(d["description"].as_str().unwrap_or("")));
            let names = |k: &str| -> Vec<String> { d[k].as_array().cloned().unwrap_or_default().iter().filter_map(|x| x["name"].as_str().map(|s| s.to_string())).collect() };
            g.metadata.description = desc.clone();
            if g.metadata.summary.is_empty() {
                g.metadata.summary = desc.split("\n\n").next().unwrap_or("").chars().take(400).collect();
            }
            g.metadata.developers = names("developers");
            g.metadata.publishers = names("publishers");
            g.metadata.genres = names("genres");
            g.metadata.metacritic = d["metacritic"].as_u64().unwrap_or(0) as u32;
            if g.release_year == 0 {
                g.release_year = d["released"].as_str().and_then(released_year).unwrap_or(0);
            }
            changed = true;
        }
    }

    let appid = if g.metadata.steam_appid > 0 { g.metadata.steam_appid } else { cache.steam_appid };
    if appid > 0 {
        let shots_dir = g.media_dir().join("screenshots");
        let have = std::fs::read_dir(&shots_dir).map(|r| r.count()).unwrap_or(0);
        if force || have == 0 {
            let urls = steam_screenshots(appid).unwrap_or_default();
            for (i, u) in urls.iter().take(8).enumerate() {
                let dest = shots_dir.join(format!("steam-{:02}.{}", i + 1, ext_of(u)));
                if download(u, &dest).is_ok() {
                    cache.sources.insert("screenshots".into(), "steam".into());
                    changed = true;
                }
            }
        }
    }

    cache.fetched_at = chrono::Local::now().to_rfc3339();
    write_cache(&g, &cache)?;
    if changed {
        g.save()?;
    }
    Ok(changed)
}

fn resolved_sgdb_id(config: &Config, game: &Game, cache: &SyncCache) -> crate::Result<u64> {
    if game.metadata.sgdb_id > 0 {
        return Ok(game.metadata.sgdb_id);
    }
    if cache.sgdb_id > 0 {
        return Ok(cache.sgdb_id);
    }
    let key = config.api_key("sgdb").ok_or_else(|| crate::Error::Unavailable("no SteamGridDB key".into()))?;
    Ok(sgdb_search(&key, &game.title, game.release_year)?.unwrap_or(0))
}

/// One page of SteamGridDB's art for a slot, best first.
pub fn candidates(config: &Config, game: &Game, slot: &str, page: u32) -> crate::Result<CandidatePage> {
    let key = config.api_key("sgdb").ok_or_else(|| crate::Error::Unavailable("no SteamGridDB key".into()))?;
    let Some(&(_, endpoint, dims)) = SGDB_PLAN.iter().find(|(s, _, _)| *s == slot) else {
        return Err(crate::Error::Invalid(format!("unknown slot {slot}")));
    };
    let mut cache = read_cache(game);
    let id = resolved_sgdb_id(config, game, &cache)?;
    if id == 0 {
        return Ok(CandidatePage { items: vec![], page, more: false });
    }
    if cache.sgdb_id != id {
        cache.sgdb_id = id;
        cache.sgdb_miss = false;
        let _ = write_cache(game, &cache);
    }
    let (mut items, more) = sgdb_assets(&key, endpoint, id, dims, page)?;
    for c in items.iter_mut() {
        c.slot = slot.into();
    }
    Ok(CandidatePage { items, page, more })
}

/// SteamGridDB's games for a query (the title when empty), the pinned or resolved one marked.
pub fn search(config: &Config, game: &Game, query: &str) -> crate::Result<Vec<Hit>> {
    let key = config.api_key("sgdb").ok_or_else(|| crate::Error::Unavailable("no SteamGridDB key".into()))?;
    let query = if query.trim().is_empty() { game.title.as_str() } else { query.trim() };
    let current = if game.metadata.sgdb_id > 0 { game.metadata.sgdb_id } else { read_cache(game).sgdb_id };
    let mut hits = sgdb_hits(&key, query)?;
    for h in hits.iter_mut() {
        h.current = h.id == current;
    }
    Ok(hits)
}

fn check_slot(slot: &str) -> crate::Result<()> {
    if !MEDIA_SLOTS.contains(&slot) && slot != "screenshot" {
        return Err(crate::Error::Invalid(format!("unknown slot {slot}")));
    }
    Ok(())
}

/// Places `src` (a file the caller owns) as the override of a slot: `<overrides>/<id>/<slot>.<ext>`,
/// any other file of the slot there gone; a screenshot joins `<overrides>/<id>/screenshots/`.
fn place_override(config: &Config, game: &Game, slot: &str, src: &Path, name: &str) -> crate::Result<PathBuf> {
    let dir = override_dir(config, game);
    if slot == "screenshot" {
        let dir = dir.join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(name);
        std::fs::copy(src, &dest)?;
        return Ok(dest);
    }
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("png").to_lowercase();
    let ext = if ext == "jpeg" { "jpg".to_string() } else { ext };
    std::fs::create_dir_all(&dir)?;
    for d in override_dirs(config, game) {
        clear_slot(&d, slot);
    }
    let dest = dir.join(format!("{slot}.{ext}"));
    std::fs::copy(src, &dest)?;
    Ok(dest)
}

/// Copies a local image over a slot, as its override. Returns where it landed.
pub fn set_slot(config: &Config, game: &Game, slot: &str, src: &Path) -> crate::Result<PathBuf> {
    check_slot(slot)?;
    if !src.is_file() || !is_image(src) {
        return Err(crate::Error::NotFound(src.display().to_string()));
    }
    let name = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "shot.png".into());
    place_override(config, game, slot, src, &name)
}

/// Downloads an image (a candidate's URL) over a slot, as its override. Returns where it landed.
pub fn set_slot_url(config: &Config, game: &Game, slot: &str, url: &str) -> crate::Result<PathBuf> {
    check_slot(slot)?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return set_slot(config, game, slot, Path::new(url.strip_prefix("file://").unwrap_or(url)));
    }
    let ext = ext_of(url);
    let tmp = std::env::temp_dir().join(format!("universe-{}-{}-{}.{ext}", game.id, slot, std::process::id()));
    download(url, &tmp)?;
    let name = url.rsplit('/').next().and_then(|n| n.split('?').next()).filter(|n| !n.is_empty()).map(|n| n.to_string()).unwrap_or_else(|| format!("shot.{ext}"));
    let placed = place_override(config, game, slot, &tmp, &name);
    let _ = std::fs::remove_file(&tmp);
    placed
}

/// Removes a slot's override, so the slot shows its fetched default again. Returns whether one was there.
pub fn unset(config: &Config, game: &Game, slot: &str) -> crate::Result<bool> {
    check_slot(slot)?;
    let mut gone = false;
    for dir in override_dirs(config, game) {
        if slot == "screenshot" {
            let shots = dir.join("screenshots");
            if shots.is_dir() {
                std::fs::remove_dir_all(&shots)?;
                gone = true;
            }
        } else if clear_slot(&dir, slot) {
            gone = true;
        }
    }
    Ok(gone)
}

/// Every slot of a game: what shows, what was fetched, what was picked, and where each came from.
pub fn status(config: &Config, game: &Game) -> MediaStatus {
    let cache = read_cache(game);
    let (default, default_shots) = scan_media_dir(&game.media_dir());
    let mut picked: Vec<(String, String)> = Vec::new();
    let mut picked_shots: Vec<String> = Vec::new();
    for dir in override_dirs(config, game) {
        let (m, s) = scan_media_dir(&dir);
        for (slot, path) in m {
            if !picked.iter().any(|(have, _)| *have == slot) {
                picked.push((slot, path));
            }
        }
        picked_shots.extend(s);
    }
    let slots = MEDIA_SLOTS
        .iter()
        .map(|slot| {
            let default = default.iter().find(|(s, _)| s == slot).map(|(_, p)| p.clone()).unwrap_or_default();
            let over = picked.iter().find(|(s, _)| s == slot).map(|(_, p)| p.clone()).unwrap_or_default();
            let source = cache.sources.get(*slot).cloned().unwrap_or_default();
            let (origin, kind) = if !over.is_empty() {
                ("picked".to_string(), "picked")
            } else if default.is_empty() {
                (String::new(), "missing")
            } else if source.is_empty() {
                (String::new(), "guessed")
            } else {
                (source.clone(), "fetched")
            };
            SlotStatus { slot: slot.to_string(), path: if over.is_empty() { default.clone() } else { over.clone() }, default, override_path: over, origin, default_origin: source, kind: kind.into() }
        })
        .collect();
    let shots_source = cache.sources.get("screenshots").cloned().unwrap_or_default();
    let (origin, kind) = if !picked_shots.is_empty() {
        ("picked".to_string(), "picked")
    } else if default_shots.is_empty() {
        (String::new(), "missing")
    } else if shots_source.is_empty() {
        (String::new(), "guessed")
    } else {
        (shots_source, "fetched")
    };
    MediaStatus {
        id: game.id.clone(),
        title: game.title.clone(),
        sgdb_id: if game.metadata.sgdb_id > 0 { game.metadata.sgdb_id } else { cache.sgdb_id },
        slots,
        screenshots: ShotsStatus { count: picked_shots.len() + default_shots.len(), override_count: picked_shots.len(), origin, kind: kind.into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_in(dir: &Path, id: &str) -> Game {
        std::env::set_var("UNIVERSE_DATA_HOME", dir.join("data"));
        let mut g = Game::new(id);
        g.source.lutris_slug = format!("{id}-lutris");
        g
    }

    fn config_in(dir: &Path) -> Config {
        let mut c = Config::default();
        c.paths.overrides = dir.join("overrides").to_string_lossy().into();
        c
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"\x89PNG").unwrap();
    }

    #[test]
    fn helpers() {
        assert_eq!(name_key("Assassin's Creed: Odyssey"), "assassins creed odyssey");
        assert_eq!(strip_html("<p>Hello &amp; <b>bye</b></p>"), "Hello & bye");
        assert_eq!(ext_of("https://x/y.webp?z"), "webp");
        assert_eq!(released_year("2016-06-28"), Some(2016));
    }

    #[test]
    fn overrides_sit_over_defaults_and_come_off() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = config_in(dir.path());
        let game = game_in(dir.path(), "g");
        touch(&game.media_dir().join("boxFront.png"));
        note_source(&game.media_dir(), "box_front", "pegasus").unwrap();
        touch(&game.media_dir().join("logo.png"));

        let st = status(&config, &game);
        let slot = |s: &str| st.slots.iter().find(|x| x.slot == s).unwrap().clone();
        assert_eq!(slot("box_front").kind, "fetched");
        assert_eq!(slot("box_front").origin, "pegasus");
        assert_eq!(slot("logo").kind, "guessed");
        assert_eq!(slot("tile").kind, "missing");

        let src = dir.path().join("pick.jpg");
        touch(&src);
        let placed = set_slot(&config, &game, "box_front", &src).unwrap();
        assert_eq!(placed, dir.path().join("overrides/g/box_front.jpg"));
        let st = status(&config, &game);
        let bf = st.slots.iter().find(|x| x.slot == "box_front").unwrap();
        assert_eq!(bf.kind, "picked");
        assert_eq!(bf.path, placed.to_string_lossy());
        assert!(bf.default.ends_with("boxFront.png"));

        // A second pick replaces the first whatever its extension; the old Lutris-slug dir is cleared too.
        touch(&dir.path().join("overrides/g-lutris/cover.png"));
        let src2 = dir.path().join("pick2.png");
        touch(&src2);
        set_slot(&config, &game, "box_front", &src2).unwrap();
        assert!(!placed.exists());
        assert!(!dir.path().join("overrides/g-lutris/cover.png").exists());
        assert!(dir.path().join("overrides/g/box_front.png").exists());

        assert!(unset(&config, &game, "box_front").unwrap());
        assert!(!unset(&config, &game, "box_front").unwrap());
        let st = status(&config, &game);
        let bf = st.slots.iter().find(|x| x.slot == "box_front").unwrap();
        assert_eq!(bf.kind, "fetched");
        assert!(bf.path.ends_with("boxFront.png"));
        assert!(game.media_dir().join("boxFront.png").exists());
    }

    #[test]
    fn screenshots_join_the_override_dir() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = config_in(dir.path());
        let game = game_in(dir.path(), "s");
        let src = dir.path().join("mine.png");
        touch(&src);
        set_slot(&config, &game, "screenshot", &src).unwrap();
        assert!(dir.path().join("overrides/s/screenshots/mine.png").exists());
        let st = status(&config, &game);
        assert_eq!(st.screenshots.kind, "picked");
        assert_eq!(st.screenshots.override_count, 1);
        assert!(unset(&config, &game, "screenshot").unwrap());
        assert_eq!(status(&config, &game).screenshots.kind, "missing");
        assert!(set_slot(&config, &game, "cover", &src).is_err());
    }
}
