use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::{Error, Result};

/// gamescope's fixed screenshot path; a new mtime is a new shot.
const SCREENSHOT_PATH: &str = "/tmp/gamescope.png";

/// gamescope-control's `screenshot_type`: the game alone at its render size, or with the overlay layers (mangoapp's HUD, a visible dock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shot {
    BasePlane = 1,
    AllRealLayers = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focusable {
    pub window: u32,
    pub app_id: u32,
    pub pid: u32,
}

pub fn parse_focusable(cards: &[u32]) -> Vec<Focusable> {
    cards.as_chunks::<3>().0.iter().map(|&[window, app_id, pid]| Focusable { window, app_id, pid }).collect()
}

/// The window to show among one program's (named, else the newest: ids grow), and the STEAM_GAME each window needs for it.
fn presenting(theirs: &[(Focusable, String)], title: &str) -> (Option<u32>, Vec<(u32, u32)>) {
    let target =
        theirs.iter().find(|(_, name)| !title.is_empty() && name == title).or_else(|| theirs.iter().max_by_key(|(w, _)| w.window)).map(|(w, _)| w.window);
    let cards = theirs
        .iter()
        .filter_map(|(w, _)| match (Some(w.window) == target, w.app_id) {
            (true, 0) => Some((w.window, w.window)),
            (false, app) if app != 0 => Some((w.window, 0)),
            _ => None,
        })
        .collect();
    (target, cards)
}

pub fn inside() -> bool {
    std::env::var_os("GAMESCOPE_WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
}

/// Set by the launcher on the gamescope it starts for itself: `drm` straight on the screen, `nested` in a desktop's window.
pub const OWN_ENV: &str = "UNIVERSE_OWN_GAMESCOPE";

/// The desktop's X display, set by the launcher on its nested gamescope: inside, `DISPLAY` is gamescope's own Xwayland.
pub const HOST_DISPLAY_ENV: &str = "UNIVERSE_HOST_DISPLAY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Own {
    Drm,
    Nested,
}

pub fn own() -> Option<Own> {
    match std::env::var(OWN_ENV).as_deref() {
        Ok("drm") => Some(Own::Drm),
        Ok("nested") => Some(Own::Nested),
        _ => None,
    }
}

/// Steam drives a gamescope it runs with --steam (Game Mode): it sets the base layer's app id on the root, and hands its
/// games (this launcher among them, as a non-Steam shortcut) a SteamGameId.
pub fn steam_driven(own: Option<Own>, baselayer: bool, steam_game_id: bool) -> bool {
    own.is_none() && (baselayer || steam_game_id)
}

/// The app id gamescope gave the launcher's own window, else the one Steam handed the launcher.
fn launcher_app(windows: &[Focusable], launcher: u32) -> Option<u32> {
    windows
        .iter()
        .find(|w| w.pid == launcher && w.app_id != 0)
        .map(|w| w.app_id)
        .or_else(|| std::env::var("SteamGameId").ok()?.parse().ok().filter(|id| *id != 0))
}

/// Whether a `/proc/<pid>/cgroup` puts the process in systemd's `unit` (a service, `.service` optional).
pub fn in_unit(cgroup: &str, unit: &str) -> bool {
    let service = format!("{unit}.service");
    cgroup.lines().any(|line| line.rsplit(':').next().is_some_and(|path| path.split('/').any(|seg| seg == unit || seg == service)))
}

/// The game's windows: every one not the launcher's; under Steam (`steam_app`, the launcher's id) only those no app claims
/// yet or the launcher's id already stamped, since Steam's own UI is on the list too.
pub fn game_windows(windows: &[Focusable], launcher: u32, steam_app: Option<u32>) -> impl Iterator<Item = &Focusable> {
    windows.iter().filter(move |w| w.pid != launcher && steam_app.is_none_or(|app| w.app_id == 0 || w.app_id == app))
}

/// The STEAM_GAME each game window takes: the game shown, each its own id or, under Steam, the launcher's; the launcher
/// shown, 0.
pub fn stamps(windows: &[Focusable], launcher: u32, steam_app: Option<u32>, game: bool) -> Vec<(u32, u32)> {
    game_windows(windows, launcher, steam_app)
        .filter_map(|w| {
            let want = match (game, steam_app) {
                (false, _) => 0,
                (true, Some(app)) => app,
                // An id already set stays: the game may have set its own.
                (true, None) if w.app_id != 0 => w.app_id,
                (true, None) => w.window,
            };
            (w.app_id != want).then_some((w.window, want))
        })
        .collect()
}

pub struct Nest {
    conn: RustConnection,
    root: u32,
    pub pid: u32,
    /// Steam's gamescope, not the launcher's: see `steam_driven`.
    pub steam: bool,
}

impl Nest {
    pub fn open() -> Result<Nest> {
        if !inside() {
            return Err(Error::Unavailable("not inside gamescope".into()));
        }
        let (conn, screen) = x11rb::connect(None).map_err(|e| Error::Unavailable(format!("gamescope's display: {e}")))?;
        let root = conn.setup().roots[screen].root;
        let nest = Nest { conn, root, pid: 0, steam: false };
        let pid = nest.cards(root, "GAMESCOPE_PID")?.first().copied().ok_or_else(|| Error::Unavailable("the display is not gamescope's".into()))?;
        let baselayer = !nest.cards(root, "GAMESCOPECTRL_BASELAYER_APPID")?.is_empty();
        let steam = steam_driven(own(), baselayer, std::env::var_os("SteamGameId").is_some_and(|v| !v.is_empty()));
        Ok(Nest { pid, steam, ..nest })
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

    /// The mapped top-level windows as the X tree has them, with `_NET_WM_PID` and `STEAM_GAME` (0 where unset).
    fn tree(&self) -> Result<Vec<Focusable>> {
        let children = self.conn.query_tree(self.root).map_err(x)?.reply().map_err(x)?.children;
        let mut out = vec![];
        for window in children {
            let Ok(attrs) = self.conn.get_window_attributes(window).map_err(x)?.reply() else { continue };
            if attrs.map_state != MapState::VIEWABLE || attrs.override_redirect {
                continue;
            }
            let pid = self.cards(window, "_NET_WM_PID").ok().and_then(|c| c.first().copied()).unwrap_or(0);
            let app_id = self.cards(window, "STEAM_GAME").ok().and_then(|c| c.first().copied()).unwrap_or(0);
            out.push(Focusable { window, app_id, pid });
        }
        Ok(out)
    }

    /// The windows the game may have, and the launcher's app id under Steam. Steam's gamescope lists no window without an app
    /// id, and a game started as a unit has none (no Steam reaper above it): there they come off the X tree, the ones whose
    /// process runs in `game_unit`. On the launcher's own gamescope a store client's windows are not the game's either
    /// (`game`), yet stand aside with the game's when the launcher shows.
    fn candidates(&self, launcher: u32, game_unit: Option<&str>, game: bool) -> Result<(Vec<Focusable>, Option<u32>)> {
        let owned = |pid: u32, unit: &str| std::fs::read_to_string(format!("/proc/{pid}/cgroup")).is_ok_and(|text| in_unit(&text, unit));
        if !self.steam {
            let windows = self.windows()?;
            return Ok(match game_unit.filter(|_| game) {
                Some(unit) => (windows.into_iter().filter(|w| owned(w.pid, unit)).collect(), None),
                None => (windows, None),
            });
        }
        let app = launcher_app(&self.windows()?, launcher).ok_or_else(|| Error::Unavailable("Steam gave the launcher no app id".into()))?;
        let Some(unit) = game_unit else { return Ok((vec![], Some(app))) };
        let windows = self.tree()?.into_iter().filter(|w| w.pid != 0 && owned(w.pid, unit)).collect();
        Ok((windows, Some(app)))
    }

    pub fn game_shown(&self, launcher: u32, game_unit: Option<&str>) -> Result<bool> {
        let Some(focused) = self.focused()? else { return Ok(false) };
        let (windows, steam_app) = self.candidates(launcher, game_unit, true)?;
        let shown = game_windows(&windows, launcher, steam_app).any(|w| w.window == focused);
        Ok(shown)
    }

    // Without --steam gamescope shows the newest mapped window whose STEAM_GAME is not 0; the BASELAYER atoms act only under --steam.
    // Under --steam the game's windows take the launcher's app id, and the newer window shows.
    pub fn show(&self, launcher: u32, game: bool, game_unit: Option<&str>) -> Result<()> {
        let (windows, steam_app) = self.candidates(launcher, game_unit, game)?;
        for (window, value) in stamps(&windows, launcher, steam_app, game) {
            self.set_card(window, "STEAM_GAME", value)?;
        }
        Ok(())
    }

    /// Keeps the window of `class` titled `title` (else the class's newest) the one gamescope may show among that class's
    /// windows; Steam's own gamescope shows what Steam picks.
    pub fn present(&self, launcher: u32, class: &str, title: &str) -> Result<()> {
        if self.steam {
            return Ok(());
        }
        let mut theirs = Vec::new();
        for w in self.windows()?.into_iter().filter(|w| w.pid != launcher) {
            if self.class_of(w.window)? == class {
                theirs.push((w, self.name_of(w.window)?));
            }
        }
        for (window, value) in presenting(&theirs, title).1 {
            self.set_card(window, "STEAM_GAME", value)?;
        }
        Ok(())
    }

    fn class_of(&self, window: u32) -> Result<String> {
        let reply = self.conn.get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256).map_err(x)?.reply().map_err(x)?;
        Ok(reply.value.split(|b| *b == 0).nth(1).map(|c| String::from_utf8_lossy(c).into_owned()).unwrap_or_default())
    }

    fn name_of(&self, window: u32) -> Result<String> {
        let (net, utf8) = (self.atom("_NET_WM_NAME")?, self.atom("UTF8_STRING")?);
        let mut value = self.conn.get_property(false, window, net, utf8, 0, 1024).map_err(x)?.reply().map_err(x)?.value;
        if value.is_empty() {
            value = self.conn.get_property(false, window, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024).map_err(x)?.reply().map_err(x)?.value;
        }
        Ok(String::from_utf8_lossy(&value).into_owned())
    }

    /// gamescope takes one shot at a time: a request made while another is in flight is dropped, and waits out `timeout`.
    pub fn frame(&self, into: &Path, shot: Shot, timeout: Duration) -> Result<Option<PathBuf>> {
        let src = Path::new(SCREENSHOT_PATH);
        let before = std::fs::metadata(src).and_then(|m| m.modified()).ok();
        self.set_card(self.root, "GAMESCOPECTRL_REQUEST_SCREENSHOT", shot as u32)?;
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

    /// The panel's refresh as gamescope drives it now.
    pub fn refresh(&self) -> Result<Option<u32>> {
        Ok(self.cards(self.root, "GAMESCOPE_DISPLAY_REFRESH_RATE_FEEDBACK")?.first().copied().filter(|hz| *hz != 0))
    }

    /// The internal panel's target refresh, within the range gamescope's display script gives it (as Steam's slider sets it).
    pub fn set_refresh(&self, hz: u32) -> Result<()> {
        self.set_card(self.root, "GAMESCOPE_DYNAMIC_REFRESH", hz)
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

    #[test]
    fn a_game_window_takes_its_own_id_on_the_launchers_gamescope_and_the_launchers_under_steam() {
        let launcher = Focusable { window: 0x100, app_id: 7_000_000, pid: 10 };
        let fresh = Focusable { window: 0x200, app_id: 0, pid: 20 };
        let tagged = Focusable { window: 0x300, app_id: 0x300, pid: 20 };
        let windows = [launcher, fresh, tagged];
        assert_eq!(stamps(&windows, 10, None, true), [(0x200, 0x200)], "a tagged window stays as it is");
        assert_eq!(stamps(&windows, 10, None, false), [(0x300, 0)]);
        assert_eq!(launcher_app(&windows, 10), Some(7_000_000));
        let stamped = Focusable { window: 0x300, app_id: 7_000_000, pid: 20 };
        let steam_ui = Focusable { window: 0x400, app_id: 769, pid: 30 };
        let game_mode = [launcher, fresh, stamped, steam_ui];
        assert_eq!(stamps(&game_mode, 10, Some(7_000_000), true), [(0x200, 7_000_000)], "Steam's own UI keeps its id");
        assert_eq!(stamps(&game_mode, 10, Some(7_000_000), false), [(0x300, 0)], "nor is it hidden, nor the launcher's window touched");
        let shown: Vec<u32> = game_windows(&game_mode, 10, Some(7_000_000)).map(|w| w.window).collect();
        assert_eq!(shown, [0x200, 0x300], "Steam's UI focused is no game shown");
    }

    #[test]
    fn a_process_is_the_games_by_its_cgroup() {
        let game = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/universe-game-dead-cells-20260927-173200.service\n";
        assert!(in_unit(game, "universe-game-dead-cells-20260927-173200.service"));
        assert!(in_unit(game, "universe-game-dead-cells-20260927-173200"), "the suffix is optional");
        assert!(!in_unit(game, "universe-game-dead-cells-20260927-170805.service"), "another session of the same game");
        let steam = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-steam@autostart.service\n";
        assert!(!in_unit(steam, "universe-game-dead-cells-20260927-173200.service"));
    }

    #[test]
    fn steam_drives_a_gamescope_the_launcher_did_not_start() {
        assert!(steam_driven(None, true, false), "Game Mode: the base layer is set");
        assert!(steam_driven(None, false, true), "a shortcut before Steam set the base layer");
        assert!(!steam_driven(None, false, false), "a bare gamescope someone else started");
        assert!(!steam_driven(Some(Own::Nested), false, true), "the launcher's own gamescope, started from Steam's desktop client");
    }

    #[test]
    fn the_named_window_is_shown_and_its_siblings_are_not() {
        let window = |window, app_id| Focusable { window, app_id, pid: 7 };
        let steam = [(window(0x220003f, 0x220003f), "Steam".to_string()), (window(0x220009f, 0x220009f), "Install".to_string())];
        assert_eq!(presenting(&steam, "Install"), (Some(0x220009f), vec![(0x220003f, 0)]), "the main window stands aside");
        assert_eq!(presenting(&steam, "Installer"), (Some(0x220009f), vec![(0x220003f, 0)]), "another language: the newest window");
        let hidden = [(window(0x220003f, 0), "Steam".to_string()), (window(0x220009f, 0), "Install".to_string())];
        assert_eq!(presenting(&hidden, "Install"), (Some(0x220009f), vec![(0x220009f, 0x220009f)]), "a window hidden before comes back");
        assert_eq!(presenting(&[], "Install"), (None, vec![]));
    }
}
