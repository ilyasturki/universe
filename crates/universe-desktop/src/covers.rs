use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::backend;

// About two screens of 4K covers at scale 2: a grid scrolled back and forth repaints from here.
const CAPACITY: usize = 240;

#[derive(Default)]
struct Cache {
    textures: HashMap<(String, u32, u32), (gdk::Texture, u64)>,
    tick: u64,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::default();
}

fn cached(key: &(String, u32, u32)) -> Option<gdk::Texture> {
    CACHE.with_borrow_mut(|c| {
        c.tick += 1;
        let tick = c.tick;
        c.textures.get_mut(key).map(|(t, used)| {
            *used = tick;
            t.clone()
        })
    })
}

fn keep(key: (String, u32, u32), texture: gdk::Texture) {
    CACHE.with_borrow_mut(|c| {
        c.tick += 1;
        let tick = c.tick;
        c.textures.insert(key, (texture, tick));
        if c.textures.len() > CAPACITY {
            let mut ages: Vec<u64> = c.textures.values().map(|(_, used)| *used).collect();
            ages.sort_unstable();
            let cut = ages[c.textures.len() - CAPACITY];
            c.textures.retain(|_, (_, used)| *used >= cut);
        }
    });
}

/// Drops a file's textures, at every size: the art under that path was replaced.
pub fn forget(path: &str) {
    CACHE.with_borrow_mut(|c| c.textures.retain(|(p, _, _), _| p != path));
}

/// The picture when it is decoded already at that size.
pub fn ready(path: &str, width: u32, height: u32) -> Option<gdk::Texture> {
    cached(&(path.to_string(), width, height))
}

/// The picture at `path`, a file or a web address, scaled down to cover `width`×`height` pixels, decoded off the main loop;
/// `None` when there is none.
pub async fn texture(path: &str, width: u32, height: u32) -> Option<gdk::Texture> {
    if path.is_empty() {
        return None;
    }
    let key = (path.to_string(), width, height);
    if let Some(t) = cached(&key) {
        return Some(t);
    }
    let file = if is_web(path) { fetched(path).await? } else { path.to_string() };
    let texture = backend::run(async move { tokio::task::spawn_blocking(move || decode(&file, width, height)).await.ok().flatten() }).await?;
    keep(key, texture.clone());
    Some(texture)
}

/// A picture at the size a viewer shows it, decoded off the main loop and kept out of the cache, which is for small ones.
pub async fn full(path: &str, width: u32, height: u32) -> Option<gdk::Texture> {
    let file = path.to_string();
    backend::run(async move { tokio::task::spawn_blocking(move || decode(&file, width, height)).await.ok().flatten() }).await
}

fn is_web(path: &str) -> bool {
    path.starts_with("https://") || path.starts_with("http://")
}

fn cache_path(url: &str) -> Option<std::path::PathBuf> {
    let name = glib::compute_checksum_for_string(glib::ChecksumType::Sha1, url)?;
    Some(universe::paths::cache_home().join("remote").join(name.as_str()))
}

/// The picture as a file on this machine: the path itself, a web picture's copy once it was downloaded.
pub fn on_disk(path: &str) -> Option<std::path::PathBuf> {
    let file = if is_web(path) { cache_path(path)? } else { std::path::PathBuf::from(path) };
    file.is_file().then_some(file)
}

/// A web picture's copy in the cache, downloaded the first time.
async fn fetched(url: &str) -> Option<String> {
    let path = cache_path(url)?;
    let file = path.to_string_lossy().into_owned();
    if path.is_file() {
        return Some(file);
    }
    let url = url.to_string();
    let result = backend::run(async move {
        let one = FETCHING.lock().ok()?.entry(url.clone()).or_default().clone();
        let _only = one.lock().await;
        if path.is_file() {
            return Some(());
        }
        let _turn = FETCHES.acquire().await.ok()?;
        let fetched = download(&url, &path).await.map_err(|e| tracing::debug!("{url}: {e}")).ok();
        FETCHING.lock().ok()?.remove(&url);
        fetched
    })
    .await;
    result.map(|_| file)
}

// A store listing asks for every row's picture at once; the rest wait their turn.
static FETCHES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// One download per address: a row built again while its picture comes waits for that one.
static FETCHING: LazyLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = LazyLock::new(Mutex::default);

async fn download(url: &str, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    let client = CLIENT.get_or_init(|| reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().unwrap_or_default());
    let bytes = client.get(url).send().await?.error_for_status()?.bytes().await?;
    let kept = tokio::task::spawn_blocking(move || shrink(&bytes)).await??;
    let dir = path.parent().ok_or("no folder")?;
    tokio::fs::create_dir_all(dir).await?;
    let part = path.with_extension("part");
    tokio::fs::write(&part, &kept).await?;
    tokio::fs::rename(&part, path).await?;
    Ok(())
}

// Web pictures show as thumbnails; a store's originals run to 1600 px and 400 KB each.
const KEPT_WIDTH: u32 = 480;

/// The picture no wider than `KEPT_WIDTH`, as a PNG when it has transparency, else a JPEG.
fn shrink(bytes: &[u8]) -> Result<Vec<u8>, image::ImageError> {
    let img = image::load_from_memory(bytes)?;
    if img.width() <= KEPT_WIDTH {
        return Ok(bytes.to_vec());
    }
    let img = img.resize(KEPT_WIDTH, u32::MAX, image::imageops::FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    if img.color().has_alpha() {
        img.write_to(&mut out, image::ImageFormat::Png)?;
    } else {
        image::DynamicImage::ImageRgb8(img.to_rgb8()).write_to(&mut out, image::ImageFormat::Jpeg)?;
    }
    Ok(out.into_inner())
}

fn decode(path: &str, width: u32, height: u32) -> Option<gdk::Texture> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().map_err(|e| tracing::debug!("{path}: {e}")).ok()?;
    let (w, h) = (img.width().max(1), img.height().max(1));
    let scale = (f64::from(width) / f64::from(w)).max(f64::from(height) / f64::from(h));
    let img = if scale < 1.0 {
        let (nw, nh) = (((f64::from(w) * scale).round() as u32).max(1), ((f64::from(h) * scale).round() as u32).max(1));
        img.resize_exact(nw, nh, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    let bytes = glib::Bytes::from_owned(rgba.into_raw());
    Some(gdk::MemoryTexture::new(w as i32, h as i32, gdk::MemoryFormat::R8g8b8a8, &bytes, w as usize * 4).upcast())
}
