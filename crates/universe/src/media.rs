use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::game::Game;
use crate::library::{is_image, scan_media_dir, stems_of, IMAGE_EXTS, MEDIA_SLOTS};

const SGDB: &str = "https://www.steamgriddb.com/api/v2";
const STEAM_STORE: &str = "https://store.steampowered.com/";
const STEAM_APPDETAILS: &str = "https://store.steampowered.com/api/appdetails";
const STEAM_ITEMS: &str = "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/";
const STEAM_CDN: &str = "https://shared.fastly.steamstatic.com/store_item_assets/";
const GAMESDB: &str = "https://gamesdb.gog.com";
const LIBRETRO: &str = "https://thumbnails.libretro.com";
const SGDB_PLAN: [(&str, &str, Option<&str>); 5] = [
    ("box_front", "grids", Some("600x900")),
    ("square", "grids", Some("1024x1024,512x512")),
    ("banner", "grids", Some("920x430,460x215")),
    ("background", "heroes", None),
    ("logo", "logos", None),
];
const KEYLESS: [&str; 4] = ["source", "steam", "gamesdb", "libretro"];
const STORE_SLOTS: [&str; 4] = ["box_front", "banner", "background", "logo"];
pub const GENERATED: &str = "generated";
const SCREENSHOTS: usize = 8;
const SQUARE_SIDE: u32 = 512;
/// Steam's store answers 429 past about 200 requests in 5 minutes from one address.
const STORE_BUDGET: usize = 200;
const STORE_WINDOW: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Candidate {
    pub provider: String,
    pub id: u64,
    pub url: String,
    pub thumb: String,
    pub score: i64,
    pub slot: String,
}

/// `more`: SteamGridDB has another page; `entry`: the catalogue game the items belong to; `sgdb_key`: the user's SteamGridDB key is set.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CandidatePage {
    pub items: Vec<Candidate>,
    pub page: u32,
    pub more: bool,
    pub entry: Option<Hit>,
    pub sgdb_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Hit {
    pub provider: String,
    pub id: u64,
    pub name: String,
    pub year: u32,
    pub verified: bool,
    pub current: bool,
}

/// The file the UI shows, the fetched default under it, the pick over it.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SlotStatus {
    pub slot: String,
    pub path: String,
    pub default: String,
    #[serde(rename = "override")]
    pub override_path: String,
    /// `picked` for a pick, else the provider that wrote the default, or empty.
    pub origin: String,
    /// The provider that wrote the default, whether or not an override sits over it.
    pub default_origin: String,
    /// picked | default | missing
    pub kind: String,
}

/// `entry`: SteamGridDB's game with the user's key, else GOG GamesDB's.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaStatus {
    pub id: String,
    pub title: String,
    pub entry: Option<Hit>,
    pub sgdb_key: bool,
    pub slots: Vec<SlotStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct SyncCache {
    sgdb_id: u64,
    sgdb_name: String,
    sgdb_year: u32,
    sgdb_miss: bool,
    gamesdb_id: u64,
    gamesdb_name: String,
    gamesdb_year: u32,
    gamesdb_miss: bool,
    /// Found through GamesDB or SteamGridDB; `source_appid` is the store's word and wins.
    steam_appid: u64,
    source_appid: u64,
    /// slot → URL, from the source's `game` event.
    source_art: BTreeMap<String, String>,
    fetched_at: String,
    /// slot, `screenshots` or `description` → the provider that wrote it.
    sources: BTreeMap<String, String>,
}

/// Where the requests go: the network, or (tests) saved answers by URL, any other URL a 404.
pub(crate) enum Net {
    Live,
    #[cfg(test)]
    Saved {
        answers: BTreeMap<String, Vec<u8>>,
        asked: Mutex<Vec<String>>,
    },
}

fn client() -> crate::Result<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(20)).user_agent("universe/0.1").build().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| crate::Error::Io(e.clone()))
}

static STORE_SENT: Mutex<VecDeque<Instant>> = Mutex::new(VecDeque::new());
static LIVE: Net = Net::Live;

/// `None` counts a request sent now; else how long until the window has room.
fn store_slot(sent: &mut VecDeque<Instant>, now: Instant) -> Option<Duration> {
    while sent.front().is_some_and(|t| now.duration_since(*t) >= STORE_WINDOW) {
        sent.pop_front();
    }
    if sent.len() < STORE_BUDGET {
        sent.push_back(now);
        return None;
    }
    sent.front().map(|t| STORE_WINDOW - now.duration_since(*t))
}

async fn store_turn() {
    loop {
        let wait = store_slot(&mut STORE_SENT.lock().unwrap_or_else(|e| e.into_inner()), Instant::now());
        match wait {
            None => return,
            Some(d) => tokio::time::sleep(d).await,
        }
    }
}

fn not_found(e: &crate::Error) -> bool {
    e.to_string().contains("404 Not Found")
}

impl Net {
    async fn get(&self, url: &str, bearer: Option<&str>) -> crate::Result<Vec<u8>> {
        match self {
            Net::Live => {
                if url.starts_with(STEAM_STORE) {
                    store_turn().await;
                }
                let mut req = client()?.get(url);
                if let Some(b) = bearer {
                    req = req.bearer_auth(b);
                }
                let resp = req.send().await.and_then(|r| r.error_for_status()).map_err(|e| crate::Error::Io(format!("{url}: {e}")))?;
                Ok(resp.bytes().await.map_err(|e| crate::Error::Io(format!("{url}: {e}")))?.to_vec())
            }
            #[cfg(test)]
            Net::Saved { answers, asked } => {
                asked.lock().unwrap().push(url.to_string());
                answers.get(url).cloned().ok_or_else(|| crate::Error::Io(format!("{url}: 404 Not Found")))
            }
        }
    }

    async fn exists(&self, url: &str) -> bool {
        match self {
            Net::Live => match client() {
                Ok(c) => c.head(url).send().await.is_ok_and(|r| r.status().is_success()),
                Err(_) => false,
            },
            #[cfg(test)]
            Net::Saved { answers, asked } => {
                asked.lock().unwrap().push(url.to_string());
                answers.contains_key(url)
            }
        }
    }

    async fn json(&self, url: &str, bearer: Option<&str>) -> crate::Result<Value> {
        serde_json::from_slice(&self.get(url, bearer).await?).map_err(|e| crate::Error::Io(format!("{url}: {e}")))
    }

    async fn download(&self, url: &str, dest: &Path) -> crate::Result<()> {
        let bytes = self.get(url, None).await?;
        if bytes.is_empty() {
            return Err(crate::Error::Io(format!("{url}: empty image")));
        }
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(dest, &bytes)?;
        Ok(())
    }
}

/// urlencoding's set: everything but unreserved characters.
const URL_COMPONENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

fn encode(s: &str) -> percent_encoding::PercentEncode<'_> {
    percent_encoding::utf8_percent_encode(s, URL_COMPONENT)
}

fn name_key(name: &str) -> String {
    crate::slug::slug(name).replace('-', " ")
}

/// No-Intro's "Legend of Zelda, The - A Link to the Past" read as "The Legend of Zelda - A Link to the Past".
fn article_first(title: &str) -> String {
    for article in ["The", "A", "An"] {
        let tail = format!(", {article}");
        if let Some(i) = title.find(&tail) {
            let rest = &title[i + tail.len()..];
            if rest.is_empty() || rest.starts_with(" -") || rest.starts_with(':') {
                return format!("{article} {}{rest}", &title[..i]);
            }
        }
    }
    title.to_string()
}

fn released_year(date: &str) -> Option<u32> {
    date.get(0..4).and_then(|y| y.parse().ok())
}

/// Steam writes "24 Feb, 2017" (or "Feb 2017", "Q3 2026").
fn first_year(text: &str) -> Option<u32> {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let run = b[i..i + 4].iter().all(u8::is_ascii_digit) && (i == 0 || !b[i - 1].is_ascii_digit()) && b.get(i + 4).is_none_or(|c| !c.is_ascii_digit());
        run.then(|| text[i..i + 4].parse().ok()).flatten()
    })
}

fn epoch_year(epoch: i64) -> u32 {
    chrono::DateTime::from_timestamp(epoch, 0).map(|d| d.format("%Y").to_string()).and_then(|y| y.parse::<u32>().ok()).unwrap_or(0)
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let c = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if name.starts_with("#x") || name.starts_with("#X") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
            _ if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Steam's store HTML as paragraphs: blocks break lines, the rest of the markup and its pictures go.
fn html_text(html: &str) -> String {
    let mut flat = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        flat.push_str(&rest[..i]);
        let Some(j) = rest[i..].find('>') else {
            rest = "";
            break;
        };
        let inner = &rest[i + 1..i + j];
        let closing = inner.starts_with('/');
        let tag = inner.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_ascii_lowercase();
        match tag.as_str() {
            "br" => flat.push('\n'),
            "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" => flat.push_str("\n\n"),
            "li" if !closing => flat.push_str("\n• "),
            _ => {}
        }
        rest = &rest[i + j + 1..];
    }
    flat.push_str(rest);
    let mut lines: Vec<String> = Vec::new();
    for line in decode_entities(&flat).lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !line.is_empty() || lines.last().is_some_and(|l| !l.is_empty()) {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

/// A GamesDB name or genre: a plain string, or by language with `*` the default.
fn localized(v: &Value) -> String {
    v.as_str().or_else(|| v["*"].as_str()).or_else(|| v["en-US"].as_str()).unwrap_or("").to_string()
}

fn gamesdb_title(g: &Value) -> String {
    localized(&g["title"])
}

fn gamesdb_year(g: &Value) -> u32 {
    g["first_release_date"].as_str().and_then(released_year).unwrap_or(0)
}

fn gamesdb_id_of(g: &Value) -> u64 {
    g["id"].as_str().and_then(|s| s.parse().ok()).or_else(|| g["id"].as_u64()).unwrap_or(0)
}

fn releases(g: &Value) -> impl Iterator<Item = (&str, &str)> + '_ {
    g["releases"].as_array().into_iter().flatten().filter_map(|r| Some((r["platform_id"].as_str()?, r["external_id"].as_str()?)))
}

/// GamesDB lists stray ids under steam (Hollow Knight's `100006`): the first is the store's.
fn steam_release(g: &Value) -> Option<u64> {
    releases(g).filter(|(p, _)| *p == "steam").find_map(|(_, id)| id.parse().ok())
}

fn gamesdb_image(g: &Value, slot: &str) -> Option<String> {
    let (key, formatter, ext) = match slot {
        "box_front" => ("vertical_cover", "", "jpg"),
        "background" => ("background", "", "jpg"),
        "logo" => ("logo", "", "png"),
        // A 796×364 crop of the wide artwork, the shape of Steam's header.
        "banner" => ("horizontal_artwork", "_product_tile_398_2x", "jpg"),
        _ => return None,
    };
    let fmt = g[key]["url_format"].as_str().or_else(|| if slot == "box_front" { g["cover"]["url_format"].as_str() } else { None })?;
    Some(fmt.replace("{formatter}", formatter).replace("{ext}", ext))
}

fn gamesdb_urls(g: &Value, key: &str) -> Vec<String> {
    g[key].as_array().into_iter().flatten().filter_map(|a| a["url_format"].as_str()).map(|f| f.replace("{formatter}", "").replace("{ext}", "jpg")).collect()
}

/// GamesDB's platform ids for a game of this platform: a PC game is a store's.
fn gamesdb_platforms(platform: &str) -> &'static [&'static str] {
    match platform {
        "" | "windows" | "linux" => &["steam", "gog", "epic"],
        "Nintendo SNES" => &["snes"],
        "Nintendo 64" => &["n64"],
        "Nintendo GameCube" => &["ncube"],
        "Nintendo Wii" => &["nwii"],
        "Nintendo Wii U" => &["nwiiu"],
        "Nintendo Switch" => &["nswitch"],
        "Nintendo 3DS" => &["3ds"],
        "Nintendo DS" => &["nds"],
        "Nintendo Game Boy Advance" | "Nintendo Game Boy" => &["ngameboy"],
        "Sony PlayStation" => &["psx"],
        "Sony PlayStation 2" => &["ps2"],
        "Sony PlayStation 3" | "Sony PlayStation 4" => &["psn"],
        "Sony PlayStation Portable" => &["psp"],
        "Sony PlayStation Vita" => &["psvita"],
        "Sega Dreamcast" => &["dc"],
        "Microsoft Xbox" | "Microsoft Xbox 360" => &["xboxog"],
        _ => &[],
    }
}

/// GamesDB's title search is fuzzy ("hollow knight" brings Silksong): only a hit named like the title counts, one on the game's
/// platform first, twins told apart by year. `true` with it when it was found on that platform.
fn gamesdb_match(items: &Value, title: &str, year: u32, platforms: &[&str]) -> Option<(Value, bool)> {
    let want = name_key(&article_first(title));
    let same: Vec<&Value> =
        items.as_array().into_iter().flatten().filter(|i| i["type"].as_str().is_none_or(|t| t == "game") && name_key(&gamesdb_title(i)) == want).collect();
    let on_platform = |g: &Value| releases(g).any(|(p, _)| platforms.contains(&p));
    let local: Vec<&Value> = same.iter().copied().filter(|g| on_platform(g)).collect();
    let pool = if local.is_empty() { same } else { local };
    let pick = (year > 0).then(|| pool.iter().find(|g| gamesdb_year(g) == year)).flatten().or(pool.first())?;
    Some(((*pick).clone(), on_platform(pick)))
}

fn libretro_system(platform: &str, ext: &str) -> Option<&'static str> {
    Some(match (platform, ext) {
        (_, "gb") => "Nintendo - Game Boy",
        (_, "gbc") => "Nintendo - Game Boy Color",
        ("Nintendo Game Boy Advance", _) => "Nintendo - Game Boy Advance",
        ("Nintendo Game Boy", _) => "Nintendo - Game Boy",
        ("Nintendo SNES", _) => "Nintendo - Super Nintendo Entertainment System",
        ("Nintendo 64", _) => "Nintendo - Nintendo 64",
        ("Nintendo DS", _) => "Nintendo - Nintendo DS",
        ("Nintendo 3DS", _) => "Nintendo - Nintendo 3DS",
        ("Nintendo GameCube", _) => "Nintendo - GameCube",
        ("Nintendo Wii", _) => "Nintendo - Wii",
        ("Nintendo Wii U", _) => "Nintendo - Wii U",
        ("Sony PlayStation", _) => "Sony - PlayStation",
        ("Sony PlayStation 2", _) => "Sony - PlayStation 2",
        ("Sony PlayStation 3", _) => "Sony - PlayStation 3",
        ("Sony PlayStation 4", _) => "Sony - PlayStation 4",
        ("Sony PlayStation Portable", _) => "Sony - PlayStation Portable",
        ("Sony PlayStation Vita", _) => "Sony - PlayStation Vita",
        ("Sega Dreamcast", _) => "Sega - Dreamcast",
        ("Microsoft Xbox", _) => "Microsoft - Xbox",
        ("Microsoft Xbox 360", _) => "Microsoft - Xbox 360",
        ("ScummVM", _) => "ScummVM",
        ("MS-DOS", _) => "DOS",
        ("Arcade", _) => "MAME",
        _ => return None,
    })
}

fn emulated(game: &Game) -> bool {
    crate::runners::spec(&game.launch.runner).is_some_and(|s| matches!(s.kind, crate::runners::Kind::Emulator))
}

fn platform_of(game: &Game) -> String {
    if !game.platform.is_empty() {
        return game.platform.clone();
    }
    crate::runners::spec(&game.launch.runner).map(|s| s.default_platform().to_string()).unwrap_or_default()
}

/// libretro names a picture after the ROM's own file, its `&*/:`<>?\|"` turned to `_`.
fn libretro_url(game: &Game, slot: &str) -> Option<String> {
    let folder = match slot {
        "box_front" => "Named_Boxarts",
        "logo" => "Named_Logos",
        _ => return None,
    };
    if !emulated(game) || game.launch.exe.is_empty() {
        return None;
    }
    let exe = game.exe_path();
    let ext = exe.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let system = libretro_system(&platform_of(game), &ext)?;
    let stem = exe.file_stem()?.to_str()?;
    let name: String = stem.chars().map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c }).collect();
    Some(format!("{LIBRETRO}/{}/{folder}/{}.png", encode(system), encode(&name)))
}

fn steam_items_url(appid: u64) -> String {
    let input =
        serde_json::json!({"ids": [{"appid": appid}], "context": {"language": "english", "country_code": "US"}, "data_request": {"include_assets": true}});
    format!("{STEAM_ITEMS}?input_json={}", encode(&input.to_string()))
}

fn steam_details_url(appid: u64) -> String {
    format!("{STEAM_APPDETAILS}?appids={appid}&l=english")
}

fn steam_file(appid: u64, file: &str) -> String {
    format!("{STEAM_CDN}steam/apps/{appid}/{file}")
}

/// GetItems hands the hashed paths: `header_2x` (920×430) lives only there.
fn steam_asset(items: &Value, key: &str) -> Option<String> {
    let file = items[key].as_str()?;
    Some(format!("{STEAM_CDN}{}", items["asset_url_format"].as_str()?.replace("${FILENAME}", file)))
}

fn sgdb_hit(r: &Value, current: bool) -> Option<Hit> {
    Some(Hit {
        provider: "sgdb".into(),
        id: r["id"].as_u64()?,
        name: r["name"].as_str().unwrap_or("").to_string(),
        year: r["release_date"].as_i64().map(epoch_year).unwrap_or(0),
        verified: r["verified"].as_bool().unwrap_or(false),
        current,
    })
}

async fn sgdb_hits(net: &Net, key: &str, title: &str) -> crate::Result<Vec<Hit>> {
    let url = format!("{SGDB}/search/autocomplete/{}", encode(title));
    Ok(net.json(&url, Some(key)).await?["data"].as_array().into_iter().flatten().filter_map(|r| sgdb_hit(r, false)).collect())
}

/// The hit named like the title wins over autocomplete's first; exact-name twins are told apart by year.
async fn sgdb_match(net: &Net, key: &str, title: &str, year: u32) -> crate::Result<Option<Hit>> {
    let hits = sgdb_hits(net, key, title).await?;
    let want = name_key(title);
    let same: Vec<&Hit> = hits.iter().filter(|h| name_key(&h.name) == want).collect();
    let pool: Vec<&Hit> = if same.is_empty() { hits.iter().collect() } else { same };
    let Some(head) = pool.first() else { return Ok(None) };
    let name = head.name.to_lowercase();
    let twins: Vec<&&Hit> = pool.iter().filter(|h| h.name.to_lowercase() == name).collect();
    if year > 0 && twins.len() > 1 {
        if let Some(h) = twins.iter().find(|h| h.year == year) {
            return Ok(Some((**h).clone()));
        }
    }
    Ok(Some((*head).clone()))
}

async fn sgdb_game(net: &Net, key: &str, id: u64) -> Option<Hit> {
    net.json(&format!("{SGDB}/games/id/{id}"), Some(key)).await.ok().and_then(|v| sgdb_hit(&v["data"], true))
}

async fn sgdb_assets(net: &Net, key: &str, endpoint: &str, game_id: u64, dims: Option<&str>, page: u32) -> crate::Result<(Vec<Candidate>, bool)> {
    let mut url = format!("{SGDB}/{endpoint}/game/{game_id}?types=static,animated&page={page}");
    if let Some(d) = dims {
        url.push_str(&format!("&dimensions={d}"));
    }
    let v = match net.json(&url, Some(key)).await {
        Ok(v) => v,
        Err(e) if not_found(&e) => return Ok((vec![], false)),
        Err(e) => return Err(e),
    };
    let mut rows: Vec<Candidate> = v["data"]
        .as_array()
        .into_iter()
        .flatten()
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

async fn sgdb_steam_appid(net: &Net, key: &str, game_id: u64) -> Option<u64> {
    let v = net.json(&format!("{SGDB}/games/id/{game_id}?platformdata=steam"), Some(key)).await.ok()?;
    let id = &v["data"]["external_platform_data"]["steam"][0]["id"];
    id.as_u64().or_else(|| id.as_str()?.parse().ok())
}

fn read_cache_in(media_dir: &Path) -> SyncCache {
    std::fs::read_to_string(media_dir.join(".sync.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn write_cache_in(media_dir: &Path, c: &SyncCache) -> crate::Result<()> {
    std::fs::create_dir_all(media_dir)?;
    std::fs::write(media_dir.join(".sync.json"), serde_json::to_string_pretty(c)?)?;
    Ok(())
}

pub fn note_source(media_dir: &Path, slot: &str, provider: &str) -> crate::Result<()> {
    let mut cache = read_cache_in(media_dir);
    cache.sources.insert(slot.into(), provider.into());
    write_cache_in(media_dir, &cache)
}

/// What a source's `game` event says of the game's art: the next refresh takes it first.
pub fn note_source_art(media_dir: &Path, art: &BTreeMap<String, String>, steam_appid: u64) -> crate::Result<()> {
    let mut cache = read_cache_in(media_dir);
    let art: BTreeMap<String, String> =
        art.iter().filter(|(slot, url)| MEDIA_SLOTS.contains(&slot.as_str()) && !url.is_empty()).map(|(s, u)| (s.clone(), u.clone())).collect();
    if (art.is_empty() || art == cache.source_art) && (steam_appid == 0 || steam_appid == cache.source_appid) {
        return Ok(());
    }
    if !art.is_empty() {
        cache.source_art = art;
    }
    if steam_appid > 0 {
        cache.source_appid = steam_appid;
    }
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

/// A logo sits over the background: GamesDB's is at times an opaque poster. A file the decoder cannot read passes.
fn see_through(path: &Path) -> bool {
    match image::ImageReader::open(path).ok().and_then(|r| r.with_guessed_format().ok()).map(|r| r.decode()) {
        Some(Ok(img)) => img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255),
        _ => true,
    }
}

/// A square from the cover: the cover whole over a blurred, darkened fill of itself.
fn compose_square(cover: &Path, dest: &Path) -> crate::Result<()> {
    use image::imageops::{self, FilterType};
    let bad = |e: &dyn std::fmt::Display| crate::Error::Invalid(format!("{}: {e}", cover.display()));
    // A CDN may serve a png under a .jpg name: the bytes say which.
    let img = image::ImageReader::open(cover)?.with_guessed_format()?.decode().map_err(|e| bad(&e))?.into_rgb8();
    let fill = imageops::blur(&imageops::resize(&img, 24, 24, FilterType::Triangle), 1.5);
    let mut canvas = imageops::resize(&fill, SQUARE_SIDE, SQUARE_SIDE, FilterType::Triangle);
    for px in canvas.pixels_mut() {
        for c in px.0.iter_mut() {
            *c = (f32::from(*c) * 0.55) as u8;
        }
    }
    let (w, h) = img.dimensions();
    let scale = SQUARE_SIDE as f32 / w.max(h) as f32;
    let (fw, fh) = (((w as f32 * scale).round() as u32).clamp(1, SQUARE_SIDE), ((h as f32 * scale).round() as u32).clamp(1, SQUARE_SIDE));
    let front = imageops::resize(&img, fw, fh, FilterType::Lanczos3);
    imageops::overlay(&mut canvas, &front, i64::from((SQUARE_SIDE - fw) / 2), i64::from((SQUARE_SIDE - fh) / 2));
    let part = dest.with_extension("part");
    canvas.save_with_format(&part, image::ImageFormat::Jpeg).map_err(|e| crate::Error::Io(format!("{}: {e}", dest.display())))?;
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// One game's providers, each asked at most once.
struct Lookup<'a> {
    net: &'a Net,
    game: Game,
    key: Option<String>,
    prefer_sgdb: bool,
    cache: SyncCache,
    gamesdb: Option<Option<Value>>,
    items: Option<Option<Value>>,
    details: Option<Option<Value>>,
    sgdb: Option<u64>,
}

impl<'a> Lookup<'a> {
    fn new(net: &'a Net, config: &Config, game: Game, force: bool) -> Lookup<'a> {
        let mut cache = read_cache_in(&game.media_dir());
        if force {
            cache.gamesdb_miss = false;
            cache.sgdb_miss = false;
        }
        Lookup { net, game, key: config.sgdb_key(), prefer_sgdb: config.keys.prefer_sgdb, cache, gamesdb: None, items: None, details: None, sgdb: None }
    }

    fn order(&self) -> Vec<&'static str> {
        let mut list = KEYLESS.to_vec();
        match (&self.key, self.prefer_sgdb) {
            (Some(_), true) => list.insert(0, "sgdb"),
            (Some(_), false) => list.push("sgdb"),
            (None, _) => {}
        }
        list
    }

    /// `source` is recorded under the source's own id.
    fn named(&self, provider: &str) -> String {
        if provider == "source" {
            self.game.source.kind.clone()
        } else {
            provider.to_string()
        }
    }

    fn store_ids(&self) -> Vec<(&'static str, String)> {
        let mut ids = Vec::new();
        let source = &self.game.source;
        for store in ["steam", "gog", "epic"] {
            if source.kind == store && !source.id.is_empty() {
                ids.push((store, source.id.clone()));
            }
        }
        let appid = [self.game.metadata.steam_appid, self.cache.source_appid].into_iter().find(|a| *a > 0);
        if let Some(a) = appid.filter(|a| !ids.contains(&("steam", a.to_string()))) {
            ids.push(("steam", a.to_string()));
        }
        ids
    }

    async fn find_gamesdb(&mut self) -> crate::Result<Option<Value>> {
        let by_id = |id: u64| format!("{GAMESDB}/games/{id}");
        let pinned = self.game.metadata.gamesdb_id;
        if pinned > 0 {
            return self.net.json(&by_id(pinned), None).await.map(Some);
        }
        if self.cache.gamesdb_id > 0 {
            match self.net.json(&by_id(self.cache.gamesdb_id), None).await {
                Ok(g) => return Ok(Some(g)),
                Err(e) if !not_found(&e) => return Err(e),
                Err(_) => {}
            }
        }
        for (store, id) in self.store_ids() {
            match self.net.json(&format!("{GAMESDB}/platforms/{store}/external_releases/{}", encode(&id)), None).await {
                Ok(r) if r["game"].is_object() => return Ok(Some(r["game"].clone())),
                Ok(_) => {}
                Err(e) if !not_found(&e) => return Err(e),
                Err(_) => {}
            }
        }
        if self.cache.gamesdb_miss {
            return Ok(None);
        }
        let title = article_first(&self.game.title);
        let hits = self.net.json(&format!("{GAMESDB}/games?title={}", encode(&title)), None).await?;
        Ok(gamesdb_match(&hits["items"], &title, self.game.release_year, gamesdb_platforms(&platform_of(&self.game))).map(|(g, _)| g))
    }

    async fn gamesdb(&mut self) -> Option<Value> {
        if let Some(found) = &self.gamesdb {
            return found.clone();
        }
        let found = match self.find_gamesdb().await {
            Ok(found) => found,
            Err(e) => {
                tracing::warn!("gamesdb {}: {e}", self.game.id);
                self.gamesdb = Some(None);
                return None;
            }
        };
        let id = found.as_ref().map_or(0, gamesdb_id_of);
        if id != self.cache.gamesdb_id && self.cache.gamesdb_id > 0 {
            self.cache.steam_appid = 0;
        }
        self.cache.gamesdb_id = id;
        self.cache.gamesdb_name = found.as_ref().map(gamesdb_title).unwrap_or_default();
        self.cache.gamesdb_year = found.as_ref().map_or(0, gamesdb_year);
        self.cache.gamesdb_miss = found.is_none();
        if let Some(appid) = found.as_ref().and_then(steam_release).filter(|_| self.cache.steam_appid == 0) {
            self.cache.steam_appid = appid;
        }
        self.gamesdb = Some(found.clone());
        found
    }

    async fn steam_appid(&mut self) -> u64 {
        for known in [self.game.metadata.steam_appid, self.cache.source_appid, self.cache.steam_appid] {
            if known > 0 {
                return known;
            }
        }
        if self.gamesdb().await.is_some() && self.cache.steam_appid > 0 {
            return self.cache.steam_appid;
        }
        if let Some(key) = self.key.clone() {
            let id = self.sgdb_id().await;
            if id > 0 {
                self.cache.steam_appid = sgdb_steam_appid(self.net, &key, id).await.unwrap_or(0);
            }
        }
        self.cache.steam_appid
    }

    async fn items(&mut self, appid: u64) -> Option<Value> {
        if self.items.is_none() {
            let v = self.net.json(&steam_items_url(appid), None).await.ok();
            self.items = Some(v.map(|v| v["response"]["store_items"][0]["assets"].clone()).filter(Value::is_object));
        }
        self.items.clone().flatten()
    }

    async fn details(&mut self, appid: u64) -> Option<Value> {
        if self.details.is_none() {
            let v = self.net.json(&steam_details_url(appid), None).await.ok();
            self.details = Some(v.map(|v| v[appid.to_string()]["data"].clone()).filter(Value::is_object));
        }
        self.details.clone().flatten()
    }

    /// The pin, else the cached or searched match; zero when SteamGridDB has nothing under the title.
    async fn resolve_sgdb(&mut self) -> crate::Result<u64> {
        let Some(key) = self.key.clone() else { return Ok(0) };
        let pinned = self.game.metadata.sgdb_id;
        let mut found = None;
        let cache = &mut self.cache;
        let id = if pinned > 0 {
            pinned
        } else if cache.sgdb_id > 0 {
            cache.sgdb_id
        } else if cache.sgdb_miss {
            0
        } else {
            found = sgdb_match(self.net, &key, &self.game.title, self.game.release_year).await?;
            cache.sgdb_miss = found.is_none();
            found.as_ref().map_or(0, |h| h.id)
        };
        if id == 0 {
            return Ok(0);
        }
        if cache.sgdb_id != id {
            cache.sgdb_name.clear();
            cache.sgdb_year = 0;
        }
        cache.sgdb_id = id;
        cache.sgdb_miss = false;
        let h = match found {
            Some(h) => Some(h),
            None if cache.sgdb_name.is_empty() => sgdb_game(self.net, &key, id).await,
            None => None,
        };
        if let Some(h) = h {
            self.cache.sgdb_name = h.name;
            self.cache.sgdb_year = h.year;
        }
        Ok(id)
    }

    async fn sgdb_id(&mut self) -> u64 {
        if let Some(id) = self.sgdb {
            return id;
        }
        let id = self.resolve_sgdb().await.unwrap_or_else(|e| {
            tracing::warn!("sgdb {}: {e}", self.game.id);
            0
        });
        self.sgdb = Some(id);
        id
    }

    async fn sgdb_page(&mut self, slot: &str, page: u32) -> crate::Result<(Vec<Candidate>, bool)> {
        let (Some(key), Some(&(_, endpoint, dims))) = (self.key.clone(), SGDB_PLAN.iter().find(|(s, _, _)| *s == slot)) else {
            return Ok((vec![], false));
        };
        let id = self.resolve_sgdb().await?;
        self.sgdb = Some(id);
        if id == 0 {
            return Ok((vec![], false));
        }
        sgdb_assets(self.net, &key, endpoint, id, dims, page).await
    }

    /// The provider's pictures for the slot, best first; `all` lists the alternatives a refresh would not take.
    async fn urls(&mut self, provider: &str, slot: &str, all: bool) -> Vec<String> {
        match provider {
            "source" => self.cache.source_art.get(slot).cloned().into_iter().collect(),
            "steam" if STORE_SLOTS.contains(&slot) => {
                let appid = self.steam_appid().await;
                if appid == 0 {
                    return vec![];
                }
                // The unhashed library_600x900.jpg is 300×450: its _2x is the 600×900.
                let (keys, files): (&[&str], &[&str]) = match slot {
                    "box_front" => (&["library_capsule_2x", "library_capsule"], &["library_600x900_2x.jpg", "library_600x900.jpg"]),
                    "background" => (&["library_hero"], &["library_hero.jpg"]),
                    "banner" => (&["header_2x", "header"], &["header.jpg"]),
                    _ => (&[], &["logo.png"]),
                };
                let mut list = Vec::new();
                if slot == "banner" || (all && !keys.is_empty()) {
                    if let Some(items) = self.items(appid).await {
                        list.extend(keys.iter().find_map(|k| steam_asset(&items, k)));
                    }
                }
                let unhashed = files.iter().map(|f| steam_file(appid, f));
                if !all {
                    list.extend(unhashed);
                } else if list.is_empty() {
                    list.extend(unhashed.take(1));
                }
                list
            }
            "gamesdb" if STORE_SLOTS.contains(&slot) => {
                let Some(g) = self.gamesdb().await else { return vec![] };
                let mut list: Vec<String> = gamesdb_image(&g, slot).into_iter().collect();
                if all && slot == "background" {
                    list.extend(gamesdb_urls(&g, "artworks"));
                }
                list
            }
            "libretro" => libretro_url(&self.game, slot).into_iter().collect(),
            "sgdb" => match self.sgdb_page(slot, 0).await {
                Ok((list, _)) => list.into_iter().take(1).map(|c| c.url).collect(),
                Err(e) => {
                    tracing::warn!("sgdb {}: {e}", self.game.id);
                    vec![]
                }
            },
            _ => vec![],
        }
    }

    /// The slot's first picture the chain can download, swapped in for what the slot held; the provider that gave it.
    async fn fill(&mut self, slot: &str, dir: &Path) -> Option<String> {
        for provider in self.order() {
            for url in self.urls(provider, slot, false).await {
                let part = dir.join(format!(".{slot}.part"));
                match self.net.download(&url, &part).await {
                    Ok(()) if slot == "logo" && !see_through(&part) => {
                        let _ = std::fs::remove_file(&part);
                        tracing::debug!("{} logo from {provider}: opaque, a picture rather than a logo", self.game.id);
                    }
                    Ok(()) => {
                        clear_slot(dir, slot);
                        if std::fs::rename(&part, dir.join(format!("{slot}.{}", ext_of(&url)))).is_ok() {
                            return Some(self.named(provider));
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&part);
                        tracing::debug!("{} {slot} from {provider}: {e}", self.game.id);
                    }
                }
            }
        }
        None
    }

    fn entry(&self) -> Option<Hit> {
        let (provider, pinned, cached, name, year) = if self.key.is_some() {
            ("sgdb", self.game.metadata.sgdb_id, self.cache.sgdb_id, &self.cache.sgdb_name, self.cache.sgdb_year)
        } else {
            ("gamesdb", self.game.metadata.gamesdb_id, self.cache.gamesdb_id, &self.cache.gamesdb_name, self.cache.gamesdb_year)
        };
        let id = if pinned > 0 { pinned } else { cached };
        let known = id > 0 && cached == id;
        (id > 0).then(|| Hit {
            provider: provider.into(),
            id,
            name: if known { name.clone() } else { String::new() },
            year: if known { year } else { 0 },
            verified: false,
            current: true,
        })
    }
}

fn take_text(field: &mut String, value: String, over: bool) -> bool {
    let fill = !value.is_empty() && (over || field.is_empty()) && *field != value;
    if fill {
        *field = value;
    }
    fill
}

fn take_list(field: &mut Vec<String>, value: Vec<String>, over: bool) -> bool {
    let fill = !value.is_empty() && (over || field.is_empty()) && *field != value;
    if fill {
        *field = value;
    }
    fill
}

fn summary_of(text: &str) -> String {
    text.split("\n\n").next().unwrap_or("").chars().take(400).collect()
}

/// Steam's store page: what it has replaces the fields on `over`, else fills the empty ones; `true` when the description is its.
fn fill_from_steam(g: &mut Game, d: &Value, over: bool) -> (bool, bool) {
    let names = |k: &str| -> Vec<String> { d[k].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_string)).collect() };
    let m = &mut g.metadata;
    let about = d["about_the_game"].as_str().or(d["detailed_description"].as_str()).map(html_text).unwrap_or_default();
    let described = take_text(&mut m.description, about, over);
    let mut changed = described;
    changed |= take_text(&mut m.summary, html_text(d["short_description"].as_str().unwrap_or("")), false);
    changed |= take_list(&mut m.developers, names("developers"), over);
    changed |= take_list(&mut m.publishers, names("publishers"), over);
    let genres = d["genres"].as_array().into_iter().flatten().filter_map(|x| x["description"].as_str().map(str::to_string)).collect();
    changed |= take_list(&mut m.genres, genres, over);
    if let Some(score) = d["metacritic"]["score"].as_u64().filter(|s| *s > 0 && (over || m.metacritic == 0) && *s as u32 != m.metacritic) {
        m.metacritic = score as u32;
        changed = true;
    }
    if g.release_year == 0 {
        if let Some(y) = d["release_date"]["date"].as_str().and_then(first_year) {
            g.release_year = y;
            changed = true;
        }
    }
    (changed, described)
}

/// GamesDB fills what Steam left empty.
fn fill_from_gamesdb(g: &mut Game, item: &Value) -> (bool, bool) {
    let names = |k: &str| -> Vec<String> { item[k].as_array().into_iter().flatten().map(|x| localized(&x["name"])).filter(|s| !s.is_empty()).collect() };
    let m = &mut g.metadata;
    let summary = localized(&item["summary"]);
    let described = take_text(&mut m.description, summary.clone(), false);
    let mut changed = described;
    changed |= take_text(&mut m.summary, summary_of(&summary), false);
    changed |= take_list(&mut m.developers, names("developers"), false);
    changed |= take_list(&mut m.publishers, names("publishers"), false);
    changed |= take_list(&mut m.genres, names("genres"), false);
    if g.release_year == 0 && gamesdb_year(item) > 0 {
        g.release_year = gamesdb_year(item);
        changed = true;
    }
    (changed, described)
}

fn wants_details(g: &Game) -> bool {
    let m = &g.metadata;
    m.description.is_empty() || m.genres.is_empty() || m.developers.is_empty()
}

/// A pick over a slot does not stop its default from being fetched; pins in game.toml win.
pub async fn refresh(config: &Config, game: &Game, force: bool) -> crate::Result<bool> {
    refresh_with(&LIVE, config, game, force).await
}

async fn refresh_with(net: &Net, config: &Config, game: &Game, force: bool) -> crate::Result<bool> {
    let mut g = Game::load(&game.toml_path())?;
    let mut look = Lookup::new(net, config, g.clone(), force);
    let mut changed = false;
    let dir = g.media_dir();
    std::fs::create_dir_all(&dir)?;

    let (have, _) = scan_media_dir(&dir);
    for slot in MEDIA_SLOTS {
        let generated = look.cache.sources.get(slot).is_some_and(|p| p == GENERATED);
        if !force && !generated && have.iter().any(|(s, _)| s == slot) {
            continue;
        }
        if let Some(provider) = look.fill(slot, &dir).await {
            look.cache.sources.insert(slot.into(), provider);
            changed = true;
        }
    }
    let (have, _) = scan_media_dir(&dir);
    let square = have.iter().any(|(s, _)| s == "square");
    let generated = look.cache.sources.get("square").is_some_and(|p| p == GENERATED);
    if !square || (force && generated) {
        let (shown, _) = crate::library::media_of(&g);
        let cover = shown.into_iter().find(|(s, p)| s == "box_front" && ["png", "jpg", "jpeg"].iter().any(|e| p.to_lowercase().ends_with(e)));
        if let Some((_, cover)) = cover {
            let dest = dir.join("square.jpg");
            match compose_square(Path::new(&cover), &dest) {
                Ok(()) => {
                    look.cache.sources.insert("square".into(), GENERATED.into());
                    changed = true;
                }
                Err(e) => tracing::warn!("square {}: {e}", g.id),
            }
        }
    }

    if force || wants_details(&g) {
        let appid = look.steam_appid().await;
        let mut described_by = None;
        if appid > 0 {
            if let Some(d) = look.details(appid).await {
                let (filled, described) = fill_from_steam(&mut g, &d, force);
                changed |= filled;
                described_by = described.then_some("steam");
            }
        }
        if wants_details(&g) {
            if let Some(item) = look.gamesdb().await {
                let (filled, described) = fill_from_gamesdb(&mut g, &item);
                changed |= filled;
                described_by = described_by.or(described.then_some("gamesdb"));
            }
        }
        if let Some(p) = described_by {
            look.cache.sources.insert("description".into(), p.into());
        }
    }

    let shots_dir = dir.join("screenshots");
    let shots = std::fs::read_dir(&shots_dir).map(|r| r.count()).unwrap_or(0);
    if force || shots == 0 {
        let appid = look.steam_appid().await;
        let mut urls: Vec<(&str, String)> = Vec::new();
        if appid > 0 {
            if let Some(d) = look.details(appid).await {
                urls = d["screenshots"].as_array().into_iter().flatten().filter_map(|s| s["path_full"].as_str()).map(|u| ("steam", u.to_string())).collect();
            }
        }
        if urls.is_empty() {
            if let Some(item) = look.gamesdb().await {
                urls = gamesdb_urls(&item, "screenshots").into_iter().map(|u| ("gamesdb", u)).collect();
            }
        }
        for (i, (provider, u)) in urls.iter().take(SCREENSHOTS).enumerate() {
            let dest = shots_dir.join(format!("{provider}-{:02}.{}", i + 1, ext_of(u)));
            if net.download(u, &dest).await.is_ok() {
                look.cache.sources.insert("screenshots".into(), (*provider).into());
                changed = true;
            }
        }
    }

    if emulated(&g) && !g.launch.exe.is_empty() && g.title == crate::core::title_of(&g.exe_path()) {
        if let Some(name) = look.gamesdb().await.and_then(|item| sure_name(&g, &item)) {
            g.title = name;
            changed = true;
        }
    }

    look.cache.fetched_at = chrono::Local::now().to_rfc3339();
    write_cache_in(&dir, &look.cache)?;
    if changed {
        g.save()?;
    }
    Ok(changed)
}

/// GamesDB's name for a ROM titled after its file: pinned, or named alike and released on the game's platform.
fn sure_name(g: &Game, item: &Value) -> Option<String> {
    let name = gamesdb_title(item);
    let pinned = g.metadata.gamesdb_id > 0 && g.metadata.gamesdb_id == gamesdb_id_of(item);
    let alike = name_key(&article_first(&g.title)) == name_key(&name);
    let platforms = gamesdb_platforms(&platform_of(g));
    let local = releases(item).any(|(p, _)| platforms.contains(&p));
    (!name.is_empty() && name != g.title && (pinned || (alike && local))).then_some(name)
}

pub async fn candidates(config: &Config, game: &Game, slot: &str, page: u32) -> crate::Result<CandidatePage> {
    candidates_with(&LIVE, config, game, slot, page).await
}

async fn candidates_with(net: &Net, config: &Config, game: &Game, slot: &str, page: u32) -> crate::Result<CandidatePage> {
    if !MEDIA_SLOTS.contains(&slot) {
        return Err(crate::Error::Invalid(format!("unknown slot {slot}")));
    }
    let mut look = Lookup::new(net, config, game.clone(), false);
    let before = serde_json::to_string(&look.cache).unwrap_or_default();
    let mut keyless: Vec<Candidate> = Vec::new();
    if page == 0 {
        for provider in KEYLESS {
            for url in look.urls(provider, slot, true).await {
                let unsure = (provider == "libretro" || (provider == "steam" && slot == "logo")) && !net.exists(&url).await;
                if unsure || keyless.iter().any(|c| c.url == url) {
                    continue;
                }
                let provider = look.named(provider);
                keyless.push(Candidate { provider, id: keyless.len() as u64, thumb: url.clone(), url, score: 0, slot: slot.into() });
            }
        }
    }
    let (mut sgdb, more) = look.sgdb_page(slot, page).await?;
    for c in sgdb.iter_mut() {
        c.slot = slot.into();
    }
    let items = if look.prefer_sgdb { sgdb.into_iter().chain(keyless).collect() } else { keyless.into_iter().chain(sgdb).collect() };
    if look.key.is_none() && look.gamesdb.is_none() {
        look.gamesdb().await;
    }
    if serde_json::to_string(&look.cache).unwrap_or_default() != before {
        let _ = write_cache_in(&game.media_dir(), &look.cache);
    }
    Ok(CandidatePage { items, page, more, entry: look.entry(), sgdb_key: look.key.is_some() })
}

/// SteamGridDB's games with the user's key, else GOG GamesDB's: the ids `media_pin` takes under the hit's provider.
pub async fn search(config: &Config, game: &Game, query: &str) -> crate::Result<Vec<Hit>> {
    search_with(&LIVE, config, game, query).await
}

async fn search_with(net: &Net, config: &Config, game: &Game, query: &str) -> crate::Result<Vec<Hit>> {
    let query = if query.trim().is_empty() { game.title.as_str() } else { query.trim() };
    let cache = read_cache_in(&game.media_dir());
    if let Some(key) = config.sgdb_key() {
        let current = if game.metadata.sgdb_id > 0 { game.metadata.sgdb_id } else { cache.sgdb_id };
        let mut hits = sgdb_hits(net, &key, query).await?;
        for h in hits.iter_mut() {
            h.current = h.id == current;
        }
        return Ok(hits);
    }
    let current = if game.metadata.gamesdb_id > 0 { game.metadata.gamesdb_id } else { cache.gamesdb_id };
    let found = net.json(&format!("{GAMESDB}/games?title={}", encode(&article_first(query))), None).await?;
    Ok(found["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| g["type"].as_str().is_none_or(|t| t == "game"))
        .map(|g| Hit {
            provider: "gamesdb".into(),
            id: gamesdb_id_of(g),
            name: gamesdb_title(g),
            year: gamesdb_year(g),
            verified: false,
            current: gamesdb_id_of(g) == current,
        })
        .filter(|h| h.id > 0)
        .collect())
}

fn check_slot(slot: &str) -> crate::Result<()> {
    if !MEDIA_SLOTS.contains(&slot) && slot != "screenshot" {
        return Err(crate::Error::Invalid(format!("unknown slot {slot}")));
    }
    Ok(())
}

/// `media/picked/<slot>.<ext>`, any other file of the slot there gone; a screenshot joins `screenshots/`.
fn place_pick(game: &Game, slot: &str, src: &Path, name: &str) -> crate::Result<PathBuf> {
    let dir = game.picked_dir();
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
    clear_slot(&dir, slot);
    let dest = dir.join(format!("{slot}.{ext}"));
    std::fs::copy(src, &dest)?;
    Ok(dest)
}

pub fn set_slot(game: &Game, slot: &str, src: &Path) -> crate::Result<PathBuf> {
    check_slot(slot)?;
    if !src.is_file() || !is_image(src) {
        return Err(crate::Error::NotFound(src.display().to_string()));
    }
    let name = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "shot.png".into());
    place_pick(game, slot, src, &name)
}

pub async fn set_slot_url(game: &Game, slot: &str, url: &str) -> crate::Result<PathBuf> {
    check_slot(slot)?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return set_slot(game, slot, Path::new(url.strip_prefix("file://").unwrap_or(url)));
    }
    let ext = ext_of(url);
    let tmp = std::env::temp_dir().join(format!("universe-{}-{}-{}.{ext}", game.id, slot, std::process::id()));
    LIVE.download(url, &tmp).await?;
    let name =
        url.rsplit('/').next().and_then(|n| n.split('?').next()).filter(|n| !n.is_empty()).map(|n| n.to_string()).unwrap_or_else(|| format!("shot.{ext}"));
    let placed = place_pick(game, slot, &tmp, &name);
    let _ = std::fs::remove_file(&tmp);
    placed
}

pub fn unset(game: &Game, slot: &str) -> crate::Result<bool> {
    check_slot(slot)?;
    if slot != "screenshot" {
        return Ok(clear_slot(&game.picked_dir(), slot));
    }
    let shots = game.picked_dir().join("screenshots");
    if !shots.is_dir() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&shots)?;
    Ok(true)
}

pub fn status(config: &Config, game: &Game) -> MediaStatus {
    let look = Lookup::new(&LIVE, config, game.clone(), false);
    let (default, _) = scan_media_dir(&game.media_dir());
    let (picked, _) = scan_media_dir(&game.picked_dir());
    let slots = MEDIA_SLOTS
        .iter()
        .map(|slot| {
            let default = default.iter().find(|(s, _)| s == slot).map(|(_, p)| p.clone()).unwrap_or_default();
            let over = picked.iter().find(|(s, _)| s == slot).map(|(_, p)| p.clone()).unwrap_or_default();
            let source = look.cache.sources.get(*slot).cloned().unwrap_or_default();
            let (origin, kind) = if !over.is_empty() {
                ("picked".to_string(), "picked")
            } else if default.is_empty() {
                (String::new(), "missing")
            } else {
                (source.clone(), "default")
            };
            SlotStatus {
                slot: slot.to_string(),
                path: if over.is_empty() { default.clone() } else { over.clone() },
                default,
                override_path: over,
                origin,
                default_origin: source,
                kind: kind.into(),
            }
        })
        .collect();
    MediaStatus { id: game.id.clone(), title: game.title.clone(), entry: look.entry(), sgdb_key: look.key.is_some(), slots }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/media");

    fn setup(id: &str) -> (crate::paths::TestEnv, Game) {
        (crate::paths::test_env(), Game::new(id))
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"\x89PNG").unwrap();
    }

    fn logo() -> Vec<u8> {
        let img = image::RgbaImage::from_fn(64, 30, |x, _| image::Rgba([200, 40, 40, if x < 8 { 0 } else { 255 }]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 251) as u8, (y % 251) as u8, 90]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(Path::new(FIXTURES).join(name)).unwrap()
    }

    fn json_of(name: &str) -> Value {
        serde_json::from_slice(&fixture(name)).unwrap()
    }

    fn saved(answers: Vec<(String, Vec<u8>)>) -> Net {
        Net::Saved { answers: answers.into_iter().collect(), asked: Mutex::new(Vec::new()) }
    }

    fn asked(net: &Net) -> Vec<String> {
        match net {
            Net::Saved { asked, .. } => asked.lock().unwrap().clone(),
            Net::Live => unreachable!(),
        }
    }

    fn keyless() -> Config {
        let mut c = Config::default();
        c.keys.sgdb_file = String::new();
        c
    }

    fn sources(game: &Game) -> BTreeMap<String, String> {
        read_cache_in(&game.media_dir()).sources
    }

    fn steam_answers(appid: u64) -> Vec<(String, Vec<u8>)> {
        let items = json_of(&format!("steam-getitems-{appid}.json"));
        let assets = &items["response"]["store_items"][0]["assets"];
        let header = steam_asset(assets, "header_2x").or_else(|| steam_asset(assets, "header")).unwrap();
        vec![
            (steam_items_url(appid), fixture(&format!("steam-getitems-{appid}.json"))),
            (steam_details_url(appid), fixture(&format!("steam-appdetails-{appid}.json"))),
            (steam_file(appid, "library_600x900_2x.jpg"), png(60, 90)),
            (steam_file(appid, "library_hero.jpg"), png(192, 62)),
            (steam_file(appid, "logo.png"), logo()),
            (header, png(92, 43)),
        ]
    }

    fn gamesdb_answers(g: &Value) -> Vec<(String, Vec<u8>)> {
        let mut list: Vec<(String, Vec<u8>)> = STORE_SLOTS.iter().filter_map(|s| gamesdb_image(g, s)).map(|u| (u, png(34, 48))).collect();
        list.extend(gamesdb_urls(g, "screenshots").into_iter().map(|u| (u, png(128, 72))));
        list
    }

    #[test]
    fn helpers() {
        assert_eq!(name_key("Assassin's Creed: Odyssey"), "assassins creed odyssey");
        assert_eq!(ext_of("https://x/y.webp?z"), "webp");
        assert_eq!(ext_of("https://images.gog.com/abc.png?namespace=gamesdb"), "png");
        assert_eq!(released_year("2016-06-28"), Some(2016));
        assert_eq!(first_year("24 Feb, 2017"), Some(2017));
        assert_eq!(first_year("Coming soon"), None);
        assert_eq!(article_first("Legend of Zelda, The - A Link to the Past"), "The Legend of Zelda - A Link to the Past");
        assert_eq!(article_first("Bard's Tale, The"), "The Bard's Tale");
        assert_eq!(article_first("Up, Up and Away"), "Up, Up and Away");
        assert_eq!(name_key(&article_first("Legend of Zelda, The - A Link to the Past")), name_key("The Legend of Zelda: A Link to the Past"));
    }

    #[test]
    fn steam_html_reads_as_paragraphs() {
        let html =
            "<h2 class=\"bb_tag\">About</h2><p>Hollow&nbsp;Knight &amp; friends<br>second line</p><ul><li>one</li><li>two</li></ul><img src=\"x\"/>tail <b";
        assert_eq!(html_text(html), "About\n\nHollow Knight & friends\nsecond line\n\n• one\n• two\n\ntail");
        assert_eq!(decode_entities("&#233;t&#xE9; &bogus; & done"), "été &bogus; & done");
    }

    #[test]
    fn the_store_window_holds_its_budget_and_frees_with_time() {
        let mut sent = VecDeque::new();
        let start = Instant::now();
        for _ in 0..STORE_BUDGET {
            assert_eq!(store_slot(&mut sent, start), None);
        }
        let wait = store_slot(&mut sent, start + Duration::from_secs(60)).expect("the 201st waits");
        assert_eq!(wait, Duration::from_secs(240));
        assert_eq!(store_slot(&mut sent, start + STORE_WINDOW), None, "the window slid past the first ones");
    }

    #[test]
    fn a_title_search_takes_the_same_name_on_the_games_platform() {
        let items = json_of("gamesdb-search-super-mario-world.json")["items"].clone();
        let (g, local) = gamesdb_match(&items, "Super Mario World", 0, &["snes"]).unwrap();
        assert_eq!((gamesdb_year(&g), local), (1990, true));
        let (g, local) = gamesdb_match(&items, "Super Mario World", 2001, &[]).unwrap();
        assert_eq!((gamesdb_year(&g), local), (2001, false));
        assert!(gamesdb_match(&items, "Super Mario", 0, &["snes"]).is_none(), "a fuzzy hit is no match");
    }

    #[tokio::test]
    async fn a_steam_game_gets_every_slot_its_details_and_shots_with_no_key() {
        let (_env, mut game) = setup("hollow-knight");
        game.title = "Hollow Knight".into();
        game.source.kind = "steam".into();
        game.source.id = "367520".into();
        game.save().unwrap();
        note_source_art(&game.media_dir(), &BTreeMap::new(), 367520).unwrap();
        let mut answers = steam_answers(367520);
        let d = json_of("steam-appdetails-367520.json");
        answers.extend(d["367520"]["data"]["screenshots"].as_array().unwrap().iter().map(|s| (s["path_full"].as_str().unwrap().to_string(), png(128, 72))));
        let net = saved(answers);

        assert!(refresh_with(&net, &keyless(), &game, false).await.unwrap());
        let got = sources(&game);
        for slot in ["box_front", "banner", "background", "logo", "description", "screenshots"] {
            assert_eq!(got.get(slot).map(String::as_str), Some("steam"), "{slot}");
        }
        assert_eq!(got["square"], GENERATED);
        let g = Game::load(&game.toml_path()).unwrap();
        assert_eq!(g.metadata.developers, ["Team Cherry"]);
        assert_eq!(g.metadata.genres, ["Action", "Adventure", "Indie"]);
        assert_eq!((g.metadata.metacritic, g.release_year), (87, 2017));
        assert!(g.metadata.description.starts_with("Hollow Knight Expands with Free Content\n\nGodmaster - Take your place"));
        assert!(g.metadata.summary.starts_with("Forge your own path"));
        assert!(asked(&net).iter().any(|u| u.contains("header_2x.jpg")), "the banner is the 920×430 header");
        assert!(!asked(&net).iter().any(|u| u.starts_with(GAMESDB)), "Steam had it all");
        let square = image::open(game.media_dir().join("square.jpg")).unwrap();
        assert_eq!((square.width(), square.height()), (SQUARE_SIDE, SQUARE_SIDE));

        let again = saved(vec![]);
        assert!(!refresh_with(&again, &keyless(), &game, false).await.unwrap());
        assert!(asked(&again).is_empty(), "a full game asks nothing");
    }

    #[tokio::test]
    async fn a_gog_game_reaches_steams_art_through_its_gamesdb_release() {
        let (_env, mut game) = setup("hollow-knight");
        game.title = "Hollow Knight".into();
        game.source.kind = "gog".into();
        game.source.id = "1308320804".into();
        game.save().unwrap();
        let release = json_of("gamesdb-gog-1308320804.json");
        let mut answers = steam_answers(367520);
        answers.push((format!("{GAMESDB}/platforms/gog/external_releases/1308320804"), fixture("gamesdb-gog-1308320804.json")));
        answers.extend(gamesdb_answers(&release["game"]));
        let net = saved(answers);

        refresh_with(&net, &keyless(), &game, false).await.unwrap();
        let cache = read_cache_in(&game.media_dir());
        assert_eq!(cache.steam_appid, 367520, "the first steam release, not the stray one");
        assert_eq!(cache.gamesdb_name, "Hollow Knight");
        assert_eq!(cache.sources["box_front"], "steam");
        assert_eq!(cache.sources["description"], "steam");
    }

    #[tokio::test]
    async fn an_epic_game_takes_its_stores_art_then_steams_banner() {
        let (_env, mut game) = setup("hades");
        game.title = "Hades".into();
        game.source.kind = "epic".into();
        game.source.id = "Min".into();
        game.save().unwrap();
        let art = BTreeMap::from([
            ("box_front".to_string(), "https://cdn1.epicgames.com/tall.jpg".to_string()),
            ("logo".into(), "https://cdn1.epicgames.com/logo.png".into()),
        ]);
        note_source_art(&game.media_dir(), &art, 0).unwrap();
        let mut answers = steam_answers(1145360);
        answers.push((format!("{GAMESDB}/platforms/epic/external_releases/Min"), fixture("gamesdb-epic-Min.json")));
        answers.push((art["box_front"].clone(), png(40, 60)));
        answers.push((art["logo"].clone(), logo()));
        let net = saved(answers);

        refresh_with(&net, &keyless(), &game, false).await.unwrap();
        let got = sources(&game);
        assert_eq!((got["box_front"].as_str(), got["logo"].as_str()), ("epic", "epic"));
        assert_eq!((got["banner"].as_str(), got["background"].as_str()), ("steam", "steam"));
        assert!(game.media_dir().join("box_front.jpg").is_file());
        assert!(!asked(&net).iter().any(|u| u.contains("library_600x900")), "the store's cover came first");
    }

    #[tokio::test]
    async fn a_rom_takes_gamesdb_then_libretro_and_its_gamesdb_name() {
        let (_env, mut game) = setup("legend-of-zelda-the-a-link-to-the-past");
        game.launch.runner = "snes9x".into();
        game.platform = "Nintendo SNES".into();
        game.launch.exe = "/roms/snes/Legend of Zelda, The - A Link to the Past (USA).sfc".into();
        game.title = crate::core::title_of(&game.exe_path());
        game.save().unwrap();
        let search = format!("{GAMESDB}/games?title={}", encode("The Legend of Zelda - A Link to the Past"));
        let mut item = json_of("gamesdb-search-super-mario-world.json")["items"][1].clone();
        item["title"] = serde_json::json!({"*": "The Legend of Zelda: A Link to the Past"});
        let mut answers = gamesdb_answers(&item);
        answers.push((search, serde_json::to_vec(&serde_json::json!({"items": [item]})).unwrap()));
        let retro_logo = libretro_url(&game, "logo").unwrap();
        assert_eq!(
            retro_logo,
            "https://thumbnails.libretro.com/Nintendo%20-%20Super%20Nintendo%20Entertainment%20System/Named_Logos/Legend%20of%20Zelda%2C%20The%20-%20A%20Link%20to%20the%20Past%20%28USA%29.png"
        );
        answers.push((retro_logo, logo()));
        let net = saved(answers);

        refresh_with(&net, &keyless(), &game, false).await.unwrap();
        let got = sources(&game);
        for slot in ["box_front", "banner", "background", "description", "screenshots"] {
            assert_eq!(got.get(slot).map(String::as_str), Some("gamesdb"), "{slot}");
        }
        assert_eq!(got["logo"], "libretro", "GamesDB's opaque poster is no logo");
        assert_eq!(Game::load(&game.toml_path()).unwrap().title, "The Legend of Zelda: A Link to the Past");
    }

    #[tokio::test]
    async fn a_title_nothing_names_is_a_miss_asked_once() {
        let (_env, mut game) = setup("nothing");
        game.title = "Nothing Like It".into();
        game.save().unwrap();
        let net = saved(vec![(format!("{GAMESDB}/games?title=Nothing%20Like%20It"), br#"{"items":[]}"#.to_vec())]);
        assert!(!refresh_with(&net, &keyless(), &game, false).await.unwrap());
        assert!(read_cache_in(&game.media_dir()).gamesdb_miss);
        let again = saved(vec![]);
        refresh_with(&again, &keyless(), &game, false).await.unwrap();
        assert!(asked(&again).is_empty(), "a miss is not asked again: {:?}", asked(&again));
    }

    #[tokio::test]
    async fn the_picker_lists_keyless_art_and_says_there_is_no_key() {
        let (_env, mut game) = setup("hollow-knight");
        game.title = "Hollow Knight".into();
        game.source.kind = "gog".into();
        game.source.id = "1308320804".into();
        game.save().unwrap();
        let release = json_of("gamesdb-gog-1308320804.json");
        let mut answers = steam_answers(367520);
        answers.push((format!("{GAMESDB}/platforms/gog/external_releases/1308320804"), fixture("gamesdb-gog-1308320804.json")));
        let net = saved(answers);

        let page = candidates_with(&net, &keyless(), &game, "box_front", 0).await.unwrap();
        assert!(!page.sgdb_key && !page.more);
        let providers: Vec<&str> = page.items.iter().map(|c| c.provider.as_str()).collect();
        assert_eq!(providers, ["steam", "gamesdb"]);
        assert_eq!(page.items[1].url, gamesdb_image(&release["game"], "box_front").unwrap());
        let entry = page.entry.unwrap();
        assert_eq!((entry.provider.as_str(), entry.name.as_str(), entry.year), ("gamesdb", "Hollow Knight", 2017));
        let logos = candidates_with(&net, &keyless(), &game, "logo", 0).await.unwrap();
        assert_eq!(logos.items.iter().map(|c| c.provider.as_str()).collect::<Vec<_>>(), ["steam", "gamesdb"]);
        assert!(candidates_with(&net, &keyless(), &game, "cover", 0).await.is_err());
    }

    #[tokio::test]
    async fn search_without_a_key_lists_gamesdb_games() {
        let (_env, mut game) = setup("super-mario-world");
        game.title = "Super Mario World".into();
        game.metadata.gamesdb_id = 51154291278329341;
        game.save().unwrap();
        let net = saved(vec![(format!("{GAMESDB}/games?title=Super%20Mario%20World"), fixture("gamesdb-search-super-mario-world.json"))]);
        let hits = search_with(&net, &keyless(), &game, "").await.unwrap();
        assert_eq!(hits.len(), 3);
        assert!(hits.iter().all(|h| h.provider == "gamesdb"));
        assert_eq!(hits.iter().filter(|h| h.current).map(|h| h.year).collect::<Vec<_>>(), [1990]);
    }

    #[tokio::test]
    async fn a_made_square_gives_way_to_a_real_one() {
        let (_env, mut game) = setup("g");
        game.title = "G".into();
        game.save().unwrap();
        std::fs::create_dir_all(game.media_dir()).unwrap();
        std::fs::write(game.media_dir().join("box_front.png"), png(60, 90)).unwrap();
        note_source(&game.media_dir(), "box_front", "gog").unwrap();
        let mut g = game.clone();
        g.metadata.description = "d".into();
        g.metadata.genres = vec!["x".into()];
        g.metadata.developers = vec!["y".into()];
        g.save().unwrap();
        let shot = game.media_dir().join("screenshots/s.png");
        touch(&shot);
        let none = saved(vec![(format!("{GAMESDB}/games?title=G"), br#"{"items":[]}"#.to_vec())]);
        assert!(refresh_with(&none, &keyless(), &g, false).await.unwrap());
        assert_eq!(sources(&g)["square"], GENERATED);

        let art = BTreeMap::from([("square".to_string(), "https://x/sq.png".to_string())]);
        note_source_art(&g.media_dir(), &art, 0).unwrap();
        g.source.kind = "itch".into();
        g.save().unwrap();
        let real = saved(vec![("https://x/sq.png".into(), png(50, 50))]);
        assert!(refresh_with(&real, &keyless(), &g, false).await.unwrap());
        assert_eq!(sources(&g)["square"], "itch");
        assert!(g.media_dir().join("square.png").is_file() && !g.media_dir().join("square.jpg").exists());
    }

    #[test]
    fn picks_sit_over_defaults_and_come_off() {
        let (env, game) = setup("g");
        touch(&game.media_dir().join("boxFront.png"));
        note_source(&game.media_dir(), "box_front", "steam").unwrap();
        touch(&game.media_dir().join("logo.png"));

        let st = status(&keyless(), &game);
        let slot = |s: &str| st.slots.iter().find(|x| x.slot == s).unwrap().clone();
        assert_eq!(slot("box_front").kind, "default");
        assert_eq!(slot("box_front").origin, "steam");
        assert_eq!(slot("logo").kind, "default");
        assert_eq!(slot("logo").origin, "");
        assert_eq!(slot("banner").kind, "missing");
        assert!(st.entry.is_none() && !st.sgdb_key);

        let src = env.path().join("pick.jpg");
        touch(&src);
        let placed = set_slot(&game, "box_front", &src).unwrap();
        assert_eq!(placed, game.media_dir().join("picked/box_front.jpg"));
        let st = status(&keyless(), &game);
        let bf = st.slots.iter().find(|x| x.slot == "box_front").unwrap();
        assert_eq!(bf.kind, "picked");
        assert_eq!(bf.path, placed.to_string_lossy());
        assert!(bf.default.ends_with("media/boxFront.png"));
        let shown = crate::library::media_of(&game).0;
        assert_eq!(shown.iter().find(|(s, _)| s == "box_front").unwrap().1, placed.to_string_lossy());

        touch(&game.picked_dir().join("cover.png"));
        let src2 = env.path().join("pick2.png");
        touch(&src2);
        set_slot(&game, "box_front", &src2).unwrap();
        assert!(!placed.exists());
        assert!(!game.picked_dir().join("cover.png").exists());
        assert!(game.picked_dir().join("box_front.png").exists());

        assert!(unset(&game, "box_front").unwrap());
        assert!(!unset(&game, "box_front").unwrap());
        let st = status(&keyless(), &game);
        let bf = st.slots.iter().find(|x| x.slot == "box_front").unwrap();
        assert_eq!(bf.kind, "default");
        assert!(bf.path.ends_with("media/boxFront.png"));
        assert!(set_slot(&game, "cover", &src).is_err());
    }

    #[test]
    fn screenshots_join_the_pick_dir() {
        let (env, game) = setup("s");
        touch(&game.media_dir().join("screenshots/steam-01.png"));
        let src = env.path().join("mine.png");
        touch(&src);
        set_slot(&game, "screenshot", &src).unwrap();
        assert!(game.picked_dir().join("screenshots/mine.png").exists());
        assert_eq!(crate::library::media_of(&game).1.len(), 2);
        assert!(unset(&game, "screenshot").unwrap());
        assert!(!game.picked_dir().join("screenshots").exists());
        assert!(game.media_dir().join("screenshots/steam-01.png").exists(), "the fetched shots stay");
    }
}
