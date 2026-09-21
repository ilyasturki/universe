use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::{Error, Result};

/// gamescope's fixed screenshot path; a new mtime is a new shot.
const SCREENSHOT_PATH: &str = "/tmp/gamescope.png";
const SCREENSHOT_BASE_PLANE: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focusable {
    pub window: u32,
    pub app_id: u32,
    pub pid: u32,
}

pub fn parse_focusable(cards: &[u32]) -> Vec<Focusable> {
    cards.as_chunks::<3>().0.iter().map(|&[window, app_id, pid]| Focusable { window, app_id, pid }).collect()
}

pub fn inside() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
}

pub struct Nest {
    conn: RustConnection,
    root: u32,
    pub pid: u32,
}

impl Nest {
    pub fn open() -> Result<Nest> {
        if !inside() {
            return Err(Error::Unavailable("not inside gamescope".into()));
        }
        let (conn, screen) = x11rb::connect(None).map_err(|e| Error::Unavailable(format!("gamescope's display: {e}")))?;
        let root = conn.setup().roots[screen].root;
        let nest = Nest { conn, root, pid: 0 };
        let pid = nest.cards(root, "GAMESCOPE_PID")?.first().copied().ok_or_else(|| Error::Unavailable("the display is not gamescope's".into()))?;
        Ok(Nest { pid, ..nest })
    }

    fn atom(&self, name: &str) -> Result<u32> {
        Ok(self.conn.intern_atom(false, name.as_bytes()).map_err(x)?.reply().map_err(x)?.atom)
    }

    fn cards(&self, window: u32, name: &str) -> Result<Vec<u32>> {
        let atom = self.atom(name)?;
        let reply = self.conn.get_property(false, window, atom, AtomEnum::CARDINAL, 0, 4096).map_err(x)?.reply().map_err(x)?;
        Ok(reply.value32().map(|it| it.collect()).unwrap_or_default())
    }

    pub fn set_card(&self, window: u32, name: &str, value: u32) -> Result<()> {
        let atom = self.atom(name)?;
        self.conn.change_property32(PropMode::REPLACE, window, atom, AtomEnum::CARDINAL, &[value]).map_err(x)?;
        self.conn.flush().map_err(x)
    }

    fn delete_card(&self, window: u32, name: &str) -> Result<()> {
        let atom = self.atom(name)?;
        self.conn.delete_property(window, atom).map_err(x)?;
        self.conn.flush().map_err(x)
    }

    pub fn windows(&self) -> Result<Vec<Focusable>> {
        Ok(parse_focusable(&self.cards(self.root, "GAMESCOPE_FOCUSABLE_WINDOWS")?))
    }

    pub fn focused(&self) -> Result<Option<u32>> {
        Ok(self.cards(self.root, "GAMESCOPE_FOCUSED_WINDOW")?.first().copied().filter(|w| *w != 0))
    }

    pub fn foreign(&self, launcher: u32) -> Result<Vec<Focusable>> {
        Ok(self.windows()?.into_iter().filter(|w| w.pid != launcher).collect())
    }

    pub fn game_shown(&self, launcher: u32) -> Result<bool> {
        let Some(focused) = self.focused()? else { return Ok(false) };
        Ok(self.foreign(launcher)?.iter().any(|w| w.window == focused))
    }

    // Without --steam gamescope shows the newest mapped window whose STEAM_GAME is not 0; the BASELAYER atoms act only under --steam.
    pub fn show(&self, launcher: u32, game: bool) -> Result<()> {
        for w in self.foreign(launcher)? {
            if (w.app_id == 0) == game {
                self.set_card(w.window, "STEAM_GAME", if game { w.window } else { 0 })?;
            }
        }
        Ok(())
    }

    pub fn frame(&self, into: &Path, timeout: Duration) -> Result<Option<PathBuf>> {
        let src = Path::new(SCREENSHOT_PATH);
        let before = std::fs::metadata(src).and_then(|m| m.modified()).ok();
        self.set_card(self.root, "GAMESCOPECTRL_REQUEST_SCREENSHOT", SCREENSHOT_BASE_PLANE)?;
        let deadline = Instant::now() + timeout;
        loop {
            let now = std::fs::metadata(src).and_then(|m| m.modified()).ok();
            if now.is_some() && now != before {
                std::thread::sleep(Duration::from_millis(30));
                std::fs::copy(src, into)?;
                return Ok(Some(into.to_path_buf()));
            }
            if Instant::now() > deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn set_filter(&self, filter: &str, sharpness: Option<u32>) -> Result<()> {
        let mode = match filter {
            "" | "linear" => 0,
            "nearest" | "pixel" => 1,
            "integer" => 2,
            "fsr" => 3,
            "nis" => 4,
            other => return Err(Error::Invalid(format!("unknown filter {other}"))),
        };
        self.set_card(self.root, "GAMESCOPE_SCALING_FILTER", mode)?;
        match sharpness {
            Some(s) => self.set_card(self.root, "GAMESCOPE_FSR_SHARPNESS", s.min(crate::gamescope::SHARPNESS_MAX)),
            // gamescope reads its default (2) back on the delete notify.
            None => self.delete_card(self.root, "GAMESCOPE_FSR_SHARPNESS"),
        }
    }
}

fn x(e: impl std::fmt::Display) -> Error {
    Error::Io(format!("gamescope's display: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focusable_triplets_parse_and_a_short_tail_is_dropped() {
        let cards = [0xa00007, 0xa00007, 2913226, 0x600007, 0, 2913200, 7];
        assert_eq!(
            parse_focusable(&cards),
            [Focusable { window: 0xa00007, app_id: 0xa00007, pid: 2913226 }, Focusable { window: 0x600007, app_id: 0, pid: 2913200 }]
        );
        assert!(parse_focusable(&[]).is_empty());
    }
}
