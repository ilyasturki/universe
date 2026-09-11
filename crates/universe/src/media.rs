use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::game::Game;
use crate::library::MEDIA_SLOTS;

const SGDB: &str = "https://www.steamgriddb.com/api/v2";
const RAWG: &str = "https://api.rawg.io/api";
const STEAM_APPDETAILS: &str = "https://store.steampowered.com/api/appdetails";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Candidate {
    pub provider: String,
    pub url: String,
    pub score: i64,
    pub slot: String,
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

/// SteamGridDB search: exact-name twins are disambiguated by release year (pegasus-sync rule).
pub fn sgdb_search(key: &str, title: &str, year: u32) -> crate::Result<Option<u64>> {
    let url = format!("{SGDB}/search/autocomplete/{}", urlencoding::encode(title));
    let v = get_json(&url, Some(key))?;
    let results = v["data"].as_array().cloned().unwrap_or_default();
    if results.is_empty() {
        return Ok(None);
    }
    let head = results[0]["name"].as_str().unwrap_or("").to_lowercase();
    let twins: Vec<&serde_json::Value> = results.iter().filter(|r| r["name"].as_str().unwrap_or("").to_lowercase() == head).collect();
    if year > 0 && twins.len() > 1 {
        for r in &twins {
            let rd = r["release_date"].as_i64().unwrap_or(0);
            let ry = chrono::DateTime::from_timestamp(rd, 0).map(|d| d.format("%Y").to_string()).and_then(|y| y.parse::<u32>().ok());
            if ry == Some(year) {
                return Ok(r["id"].as_u64());
            }
        }
    }
    Ok(results[0]["id"].as_u64())
}

fn sgdb_assets(key: &str, endpoint: &str, game_id: u64, dims: Option<&str>) -> crate::Result<Vec<Candidate>> {
    let mut url = format!("{SGDB}/{endpoint}/game/{game_id}?types=static,animated");
    if let Some(d) = dims {
        url.push_str(&format!("&dimensions={d}"));
    }
    let v = match get_json(&url, Some(key)) {
        Ok(v) => v,
        Err(e) if e.to_string().contains("HTTP 404") => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut rows: Vec<Candidate> = v["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|r| r["language"].as_str().unwrap_or("en") == "en" && !r["nsfw"].as_bool().unwrap_or(false))
        .map(|r| Candidate { provider: "sgdb".into(), url: r["url"].as_str().unwrap_or("").into(), score: r["upvotes"].as_i64().unwrap_or(0) * 1000 + r["score"].as_i64().unwrap_or(0), slot: String::new() })
        .filter(|c| !c.url.is_empty())
        .collect();
    rows.sort_by(|a, b| b.score.cmp(&a.score));
    Ok(rows)
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

fn cache_path(game: &Game) -> PathBuf {
    game.media_dir().join(".sync.json")
}

fn read_cache(game: &Game) -> SyncCache {
    std::fs::read_to_string(cache_path(game)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write_cache(game: &Game, c: &SyncCache) -> crate::Result<()> {
    std::fs::create_dir_all(game.media_dir())?;
    std::fs::write(cache_path(game), serde_json::to_string_pretty(c)?)?;
    Ok(())
}

fn slot_present(game: &Game, config: &Config, slot: &str) -> bool {
    let (media, _) = crate::library::media_of(game, &config.overrides_dir());
    media.iter().any(|(s, _)| s == slot)
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

/// Fills missing slots and metadata from SteamGridDB, RAWG and Steam; pins in game.toml win. Returns true if anything changed.
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
            let plan = [("box_front", "grids", Some("600x900")), ("tile", "grids", Some("920x430,460x215")), ("background", "heroes", None), ("logo", "logos", None)];
            for (slot, endpoint, dims) in plan {
                if !force && slot_present(&g, config, slot) {
                    continue;
                }
                let list = sgdb_assets(key, endpoint, sgdb_id, dims)?;
                if let Some(c) = list.first() {
                    let dest = g.media_dir().join(format!("{slot}.{}", ext_of(&c.url)));
                    for old in MEDIA_SLOTS.iter().filter(|s| **s == slot) {
                        for e in ["png", "jpg", "webp"] {
                            let _ = std::fs::remove_file(g.media_dir().join(format!("{old}.{e}")));
                        }
                    }
                    download(&c.url, &dest)?;
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

pub fn candidates(config: &Config, game: &Game, slot: &str) -> crate::Result<Vec<Candidate>> {
    let key = config.api_key("sgdb").ok_or_else(|| crate::Error::Unavailable("no SteamGridDB key".into()))?;
    let cache = read_cache(game);
    let id = if game.metadata.sgdb_id > 0 { game.metadata.sgdb_id } else if cache.sgdb_id > 0 { cache.sgdb_id } else { sgdb_search(&key, &game.title, game.release_year)?.unwrap_or(0) };
    if id == 0 {
        return Ok(vec![]);
    }
    let (endpoint, dims) = match slot {
        "box_front" => ("grids", Some("600x900")),
        "tile" => ("grids", Some("920x430,460x215")),
        "background" => ("heroes", None),
        "logo" => ("logos", None),
        _ => return Err(crate::Error::Invalid(format!("unknown slot {slot}"))),
    };
    let mut list = sgdb_assets(&key, endpoint, id, dims)?;
    for c in list.iter_mut() {
        c.slot = slot.into();
    }
    Ok(list)
}

pub fn set_slot(game: &Game, slot: &str, src: &Path) -> crate::Result<()> {
    if !MEDIA_SLOTS.contains(&slot) && slot != "screenshot" {
        return Err(crate::Error::Invalid(format!("unknown slot {slot}")));
    }
    if !src.is_file() {
        return Err(crate::Error::NotFound(src.display().to_string()));
    }
    let ext = src.extension().and_then(|s| s.to_str()).unwrap_or("png").to_lowercase();
    std::fs::create_dir_all(game.media_dir())?;
    if slot == "screenshot" {
        let dir = game.media_dir().join("screenshots");
        std::fs::create_dir_all(&dir)?;
        let name = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "shot.png".into());
        std::fs::copy(src, dir.join(name))?;
        return Ok(());
    }
    unset(game, slot)?;
    std::fs::copy(src, game.media_dir().join(format!("{slot}.{ext}")))?;
    Ok(())
}

pub fn unset(game: &Game, slot: &str) -> crate::Result<()> {
    for e in ["png", "jpg", "jpeg", "webp"] {
        let _ = std::fs::remove_file(game.media_dir().join(format!("{slot}.{e}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(name_key("Assassin's Creed: Odyssey"), "assassins creed odyssey");
        assert_eq!(strip_html("<p>Hello &amp; <b>bye</b></p>"), "Hello & bye");
        assert_eq!(ext_of("https://x/y.webp?z"), "webp");
        assert_eq!(released_year("2016-06-28"), Some(2016));
    }
}
