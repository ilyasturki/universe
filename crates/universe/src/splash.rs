//! gamescope unmaps its toplevel whenever no client window is focused; a disabled, skip-taskbar window ranks below any game window.

use std::io::Read;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::Command;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, PropMode, WindowClass};
use x11rb::protocol::Event;
use x11rb::wrapper::ConnectionExt as _;

const WS_DISABLED: u32 = 0x0800_0000;

/// `<width> <height>\n` then width×height×4 bytes of Qt's `Format_RGB32` (X's depth-24 ZPixmap, little-endian).
pub struct Poster {
    pub width: u32,
    pub height: u32,
    pub bgrx: Vec<u8>,
}

pub fn read_poster(path: &Path) -> std::io::Result<Poster> {
    let mut f = std::fs::File::open(path)?;
    let mut header = Vec::new();
    let mut b = [0u8; 1];
    while f.read(&mut b)? == 1 && b[0] != b'\n' && header.len() < 32 {
        header.push(b[0]);
    }
    let header = String::from_utf8_lossy(&header);
    let (w, h) = header.trim().split_once(' ').ok_or_else(|| std::io::Error::other("poster header"))?;
    let (width, height): (u32, u32) = (w.parse().map_err(std::io::Error::other)?, h.parse().map_err(std::io::Error::other)?);
    let mut bgrx = Vec::new();
    f.read_to_end(&mut bgrx)?;
    if bgrx.len() != (width * height * 4) as usize {
        return Err(std::io::Error::other(format!("poster is {} bytes, not {}x{}x4", bgrx.len(), width, height)));
    }
    Ok(Poster { width, height, bgrx })
}

/// Nearest neighbour onto the nested screen; the frontend grabs at the screen's size, so this is a no-op unless the game renders smaller.
pub fn scale(p: &Poster, width: u32, height: u32) -> Vec<u8> {
    if p.width == width && p.height == height {
        return p.bgrx.clone();
    }
    let mut out = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let sy = (y as u64 * p.height as u64 / height as u64) as usize;
        for x in 0..width {
            let sx = (x as u64 * p.width as u64 / width as u64) as usize;
            let s = (sy * p.width as usize + sx) * 4;
            let d = ((y * width + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&p.bgrx[s..s + 4]);
        }
    }
    out
}

struct Shown {
    conn: x11rb::rust_connection::RustConnection,
    win: u32,
    wm_state: u32,
    hwnd_style: u32,
}

fn show(poster: Option<&Poster>) -> Result<Shown, Box<dyn std::error::Error>> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let (width, height, depth) = (screen.width_in_pixels, screen.height_in_pixels, screen.root_depth);
    let win = conn.generate_id()?;
    let mut aux = CreateWindowAux::new().event_mask(EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE);
    let pixmap = match poster {
        Some(p) if depth == 24 || depth == 32 => {
            let pix = conn.generate_id()?;
            conn.create_pixmap(depth, pix, screen.root, width, height)?;
            let gc = conn.generate_id()?;
            conn.create_gc(gc, pix, &CreateGCAux::new())?;
            let data = scale(p, width as u32, height as u32);
            // One request holds so many rows: 4K×4 is past BIG-REQUESTS' limit.
            let row = width as usize * 4;
            let rows = ((conn.maximum_request_bytes() - 64) / row).max(1);
            let mut y = 0usize;
            while y < height as usize {
                let n = rows.min(height as usize - y);
                conn.put_image(ImageFormat::Z_PIXMAP, pix, gc, width, n as u16, 0, y as i16, 0, depth, &data[y * row..(y + n) * row])?;
                y += n;
            }
            conn.free_gc(gc)?;
            aux = aux.background_pixmap(pix);
            Some(pix)
        }
        _ => {
            aux = aux.background_pixel(screen.black_pixel);
            None
        }
    };
    conn.create_window(depth, win, screen.root, 0, 0, width, height, 0, WindowClass::INPUT_OUTPUT, screen.root_visual, &aux)?;
    let atom = |name: &str| -> Result<u32, Box<dyn std::error::Error>> { Ok(conn.intern_atom(false, name.as_bytes())?.reply()?.atom) };
    let (state, skip_taskbar, skip_pager, hwnd_style, wm_state, net_name, utf8) = (
        atom("_NET_WM_STATE")?,
        atom("_NET_WM_STATE_SKIP_TASKBAR")?,
        atom("_NET_WM_STATE_SKIP_PAGER")?,
        atom("_WINE_HWND_STYLE")?,
        atom("WM_STATE")?,
        atom("_NET_WM_NAME")?,
        atom("UTF8_STRING")?,
    );
    conn.change_property32(PropMode::REPLACE, win, state, AtomEnum::ATOM, &[skip_taskbar, skip_pager])?;
    conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"Universe")?;
    conn.change_property8(PropMode::REPLACE, win, net_name, utf8, b"Universe")?;
    conn.map_window(win)?;
    conn.flush()?;
    if let Some(pix) = pixmap {
        conn.free_pixmap(pix)?;
    }
    Ok(Shown { conn, win, wm_state, hwnd_style })
}

/// gamescope reads `_WINE_HWND_STYLE` only from a PropertyNotify it selects as it maps the window; its `WM_STATE` write is that moment.
fn serve(s: Shown) {
    let mut styled = false;
    while let Ok(event) = s.conn.wait_for_event() {
        match event {
            Event::PropertyNotify(e) if e.window == s.win && e.atom == s.wm_state && !styled => {
                styled =
                    s.conn.change_property32(PropMode::REPLACE, s.win, s.hwnd_style, AtomEnum::CARDINAL, &[WS_DISABLED]).and_then(|_| s.conn.flush()).is_ok();
            }
            Event::Error(e) => eprintln!("splash: X error {:?} (request {})", e.error_kind, e.major_opcode),
            _ => {}
        }
    }
}

pub fn run(image: Option<&Path>, cmd: &[String]) -> i32 {
    let Some((program, args)) = cmd.split_first() else {
        eprintln!("splash: no command");
        return 2;
    };
    let poster = image.and_then(|p| read_poster(p).inspect_err(|e| eprintln!("splash: {}: {e}", p.display())).ok());
    if let Some(p) = image {
        let _ = std::fs::remove_file(p);
    }
    if let Ok(s) = show(poster.as_ref()).inspect_err(|e| eprintln!("splash: no window: {e}")) {
        std::thread::spawn(move || serve(s));
    }
    match Command::new(program).args(args).status() {
        Ok(status) => status.code().unwrap_or_else(|| 128 + status.signal().unwrap_or(0)),
        Err(e) => {
            eprintln!("splash: {program}: {e}");
            127
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poster_round_trip_and_scale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.bgrx");
        let mut bytes = b"2 1\n".to_vec();
        bytes.extend([1, 2, 3, 0, 4, 5, 6, 0]);
        std::fs::write(&path, &bytes).unwrap();
        let p = read_poster(&path).unwrap();
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(scale(&p, 2, 1), p.bgrx);
        assert_eq!(scale(&p, 4, 2), [1, 2, 3, 0, 1, 2, 3, 0, 4, 5, 6, 0, 4, 5, 6, 0, 1, 2, 3, 0, 1, 2, 3, 0, 4, 5, 6, 0, 4, 5, 6, 0]);
        std::fs::write(&path, b"2 2\n1234").unwrap();
        assert!(read_poster(&path).is_err(), "a short body is refused");
    }

    #[test]
    fn no_command_is_refused_before_any_window() {
        assert_eq!(run(None, &[]), 2);
    }
}
