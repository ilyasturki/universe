use std::cell::RefCell;
use std::collections::HashMap;

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

/// The picture at `path` scaled down to cover `width`×`height` pixels, decoded off the main loop; `None` when there is none.
pub async fn texture(path: &str, width: u32, height: u32) -> Option<gdk::Texture> {
    if path.is_empty() {
        return None;
    }
    let key = (path.to_string(), width, height);
    if let Some(t) = cached(&key) {
        return Some(t);
    }
    let file = path.to_string();
    let texture = backend::run(async move { tokio::task::spawn_blocking(move || decode(&file, width, height)).await.ok().flatten() }).await?;
    keep(key, texture.clone());
    Some(texture)
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
