use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, ImageFormat, ImageOrder, Window};
use x11rb::rust_connection::RustConnection;

use super::Toplevel;

fn xerr(e: impl std::fmt::Display) -> String {
    format!("X11: {e}")
}

struct Display {
    conn: RustConnection,
    root: Window,
}

impl Display {
    fn open() -> Result<Display, String> {
        let (conn, screen) = x11rb::connect(None).map_err(xerr)?;
        let root = conn.setup().roots[screen].root;
        Ok(Display { conn, root })
    }

    fn atom(&self, name: &str) -> Result<u32, String> {
        Ok(self.conn.intern_atom(false, name.as_bytes()).map_err(xerr)?.reply().map_err(xerr)?.atom)
    }

    fn words(&self, window: Window, name: &str, kind: impl Into<u32>) -> Result<Vec<u32>, String> {
        let reply = self.conn.get_property(false, window, self.atom(name)?, kind, 0, u32::MAX).map_err(xerr)?.reply().map_err(xerr)?;
        Ok(reply.value32().map(|it| it.collect()).unwrap_or_default())
    }

    fn text(&self, window: Window, name: &str, kind: impl Into<u32>) -> Result<Vec<u8>, String> {
        Ok(self.conn.get_property(false, window, self.atom(name)?, kind, 0, u32::MAX).map_err(xerr)?.reply().map_err(xerr)?.value)
    }

    fn clients(&self) -> Result<Vec<Window>, String> {
        self.words(self.root, "_NET_CLIENT_LIST", AtomEnum::WINDOW)
    }

    fn active(&self) -> Result<Option<Window>, String> {
        Ok(self.words(self.root, "_NET_ACTIVE_WINDOW", AtomEnum::WINDOW)?.first().copied().filter(|w| *w != 0))
    }

    /// The window's frame-less rectangle in root coordinates.
    fn rect(&self, window: Window) -> Result<(i64, i64, i64, i64), String> {
        let geometry = self.conn.get_geometry(window).map_err(xerr)?.reply().map_err(xerr)?;
        let at = self.conn.translate_coordinates(window, self.root, 0, 0).map_err(xerr)?.reply().map_err(xerr)?;
        Ok((at.dst_x.into(), at.dst_y.into(), geometry.width.into(), geometry.height.into()))
    }

    fn toplevel(&self, window: Window, active: Option<Window>, hidden_state: u32) -> Result<Toplevel, String> {
        let utf8 = self.atom("UTF8_STRING")?;
        let name = self.text(window, "_NET_WM_NAME", utf8)?;
        let name = if name.is_empty() { self.text(window, "WM_NAME", AtomEnum::STRING)? } else { name };
        let class = self.text(window, "WM_CLASS", AtomEnum::STRING)?;
        let (x, y, width, height) = self.rect(window)?;
        let minimized = self.words(window, "_NET_WM_STATE", AtomEnum::ATOM)?.contains(&hidden_state);
        Ok(Toplevel {
            id: window.to_string(),
            pid: self.words(window, "_NET_WM_PID", AtomEnum::CARDINAL)?.first().map(|p| i64::from(*p)).unwrap_or(0),
            wm_class: class.split(|b| *b == 0).filter(|s| !s.is_empty()).nth(1).map(|s| String::from_utf8_lossy(s).into_owned()),
            title: Some(String::from_utf8_lossy(&name).into_owned()).filter(|t| !t.is_empty()),
            focused: active == Some(window),
            x,
            y,
            width,
            height,
            hidden: false,
            minimized,
        })
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    blocking(|| {
        let d = Display::open()?;
        let active = d.active()?;
        let hidden = d.atom("_NET_WM_STATE_HIDDEN")?;
        let clients = d.clients()?;
        Ok(clients.into_iter().filter_map(|w| d.toplevel(w, active, hidden).ok()).collect())
    })
    .await
}

/// Source indication 2, a pager's: KWin and Mutter pass it over their focus-stealing prevention, which may leave an application's request as a blinking taskbar entry.
pub async fn activate_window(id: &str) -> Result<bool, String> {
    let window: Window = id.parse().map_err(|_| format!("not an X11 window id: {id}"))?;
    blocking(move || {
        let d = Display::open()?;
        if !d.clients()?.contains(&window) {
            return Ok(false);
        }
        let event = ClientMessageEvent::new(32, window, d.atom("_NET_ACTIVE_WINDOW")?, [2u32, x11rb::CURRENT_TIME, 0, 0, 0]);
        d.conn.send_event(false, d.root, EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY, event).map_err(xerr)?;
        d.conn.flush().map_err(xerr)?;
        Ok(true)
    })
    .await
}

pub async fn screenshot(path: &std::path::Path, window: bool) -> Result<(), String> {
    let path = path.to_path_buf();
    blocking(move || {
        let d = Display::open()?;
        let screen = d.conn.setup().roots.iter().find(|s| s.root == d.root).ok_or("no screen")?;
        let (sw, sh) = (i64::from(screen.width_in_pixels), i64::from(screen.height_in_pixels));
        let (mut x, mut y, mut w, mut h) = (0, 0, sw, sh);
        if window {
            if let Some(active) = d.active()? {
                (x, y, w, h) = d.rect(active)?;
            }
        }
        let (x0, y0) = (x.clamp(0, sw), y.clamp(0, sh));
        let (w, h) = ((x + w).clamp(0, sw) - x0, (y + h).clamp(0, sh) - y0);
        if w == 0 || h == 0 {
            return Err("the window is off screen".into());
        }
        let image = d.conn.get_image(ImageFormat::Z_PIXMAP, d.root, x0 as i16, y0 as i16, w as u16, h as u16, !0).map_err(xerr)?.reply().map_err(xerr)?;
        let setup = d.conn.setup();
        let bpp = setup.pixmap_formats.iter().find(|f| f.depth == image.depth).map(|f| f.bits_per_pixel).unwrap_or(0);
        if bpp != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
            return Err(format!("X11: a {bpp}-bit {:?} root image is not one Universe reads", setup.image_byte_order));
        }
        let rgb: Vec<u8> = image.data.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0]]).collect();
        let buffer = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("X11: short image")?;
        buffer.save_with_format(&path, image::ImageFormat::Png).map_err(|e| format!("{}: {e}", path.display()))
    })
    .await
}
