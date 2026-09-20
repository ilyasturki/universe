//! Thumbnails of the player's screenshots and the journal's pictures: 960 px wide JPEGs made once, in the background, under `$XDG_CACHE_HOME/universe/thumbs/`.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use crate::paths;

/// A four-wide grid on a 4K screen.
pub const WIDTH: u32 = 960;
const JPEG_QUALITY: u8 = 85;
/// Decodes at a time: a 4K png costs about 150 ms of one core.
const WORKERS: usize = 3;

pub fn dir() -> PathBuf {
    paths::cache_home().join("thumbs")
}

/// Where `src`'s thumbnail lives, made or not: one flat directory, the source's mtime in the name, `None` when `src` is not there.
pub fn path_for(game: &str, src: &Path) -> Option<PathBuf> {
    path_in(&dir(), game, src)
}

fn path_in(root: &Path, game: &str, src: &Path) -> Option<PathBuf> {
    let mtime = std::fs::metadata(src).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let stem = src.file_stem()?.to_str()?;
    Some(root.join(format!("{game}--{stem}-{mtime}.jpg")))
}

/// Decode `src`, scale it to `WIDTH` and write `dest` in one move (a `.part` file renamed into place).
pub fn make(src: &Path, dest: &Path) -> crate::Result<()> {
    let img = image::open(src).map_err(|e| crate::Error::Invalid(format!("{}: {e}", src.display())))?;
    let (w, h) = (img.width(), img.height());
    let img = if w > WIDTH { img.thumbnail(WIDTH, ((u64::from(h) * u64::from(WIDTH)) / u64::from(w)).max(1) as u32) } else { img };
    let rgb = img.into_rgb8();
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension("part");
    {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&part)?);
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
            .encode_image(&rgb)
            .map_err(|e| crate::Error::Io(format!("{}: {e}", dest.display())))?;
        out.flush()?;
    }
    std::fs::rename(&part, dest)?;
    Ok(())
}

/// The thumbnails on their way: a second listing does not queue them again. Runs on the caller's tokio runtime.
#[derive(Debug)]
pub struct Maker {
    pending: Arc<Mutex<HashSet<PathBuf>>>,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Default for Maker {
    fn default() -> Self {
        Maker { pending: Arc::new(Mutex::new(HashSet::new())), permits: Arc::new(tokio::sync::Semaphore::new(WORKERS)) }
    }
}

impl Maker {
    /// The path `src`'s thumbnail has, and whether it is there; queues it when it is not.
    pub fn thumb(&self, game: &str, src: &Path) -> (String, bool) {
        let Some(dest) = path_for(game, src) else { return (String::new(), false) };
        let ready = dest.is_file();
        if !ready {
            self.want(src.to_path_buf(), dest.clone());
        }
        (dest.to_string_lossy().into_owned(), ready)
    }

    /// Queue `dest` from `src`; the callers' order is the queue's, so a listing queues newest first.
    pub fn want(&self, src: PathBuf, dest: PathBuf) {
        if !self.pending.lock().map(|mut p| p.insert(dest.clone())).unwrap_or(false) {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.pending.lock().map(|mut p| p.remove(&dest)).ok();
            return;
        };
        let permits = self.permits.clone();
        let pending = self.pending.clone();
        handle.spawn(async move {
            let _permit = permits.acquire_owned().await;
            let (s, d) = (src.clone(), dest.clone());
            match tokio::task::spawn_blocking(move || make(&s, &d)).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!("thumbnail: {e}"),
                Err(e) => tracing::warn!("thumbnail {}: {e}", src.display()),
            }
            pending.lock().map(|mut p| p.remove(&dest)).ok();
        });
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(path: &Path, w: u32, h: u32) {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        img.save(path).unwrap();
    }

    #[test]
    fn make_scales_wide_pictures_down_and_leaves_small_ones() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("20251219-215949.png");
        png(&big, 1920, 1080);
        let dest = dir.path().join("out.jpg");
        make(&big, &dest).unwrap();
        let out = image::open(&dest).unwrap();
        assert_eq!((out.width(), out.height()), (960, 540));
        assert!(!dir.path().join("out.part").exists(), "renamed into place");

        let small = dir.path().join("small.png");
        png(&small, 320, 200);
        let dest = dir.path().join("small.jpg");
        make(&small, &dest).unwrap();
        let out = image::open(&dest).unwrap();
        assert_eq!((out.width(), out.height()), (320, 200));
    }

    #[test]
    fn path_carries_the_game_and_the_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("20251219-215949.png");
        png(&src, 4, 4);
        let p = path_in(&dir.path().join("thumbs"), "mario", &src).unwrap();
        assert!(p.starts_with(dir.path().join("thumbs")), "{}", p.display());
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("mario--20251219-215949-") && name.ends_with(".jpg"), "{name}");
        assert!(path_in(dir.path(), "mario", &dir.path().join("missing.png")).is_none());
    }

    #[test]
    fn maker_queues_once_and_lands_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("20251219-215949.png");
        png(&src, 1200, 600);
        let dest = dir.path().join("thumbs").join("zelda--20251219-215949-1.jpg");
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        rt.block_on(async {
            let maker = Maker::default();
            maker.want(src.clone(), dest.clone());
            maker.want(src.clone(), dest.clone());
            assert!(maker.pending() <= 1, "queued once");
            for _ in 0..200 {
                if dest.is_file() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            assert!(dest.is_file(), "the thumbnail landed");
            for _ in 0..200 {
                if maker.pending() == 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            assert_eq!(maker.pending(), 0, "done, so it can be asked again");
        });
        let out = image::open(&dest).unwrap();
        assert_eq!((out.width(), out.height()), (960, 480));
    }
}
