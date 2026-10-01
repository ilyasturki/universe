use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;

mod cinnamon;
pub mod gnome;
mod hyprland;
mod kde;
mod niri;
mod notify;
mod sway;
mod x11;

pub use gnome::UNIVERSE_EXTENSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Gnome,
    Kde,
    Cinnamon,
    Sway,
    Hyprland,
    Niri,
    X11,
    None,
}

/// `desktop.profile`'s values: `auto` detects.
pub const PROFILES: [&str; 9] = ["auto", "gnome", "kde", "cinnamon", "sway", "hyprland", "niri", "x11", "none"];

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::Gnome => "GNOME",
            Profile::Kde => "KDE Plasma",
            Profile::Cinnamon => "Cinnamon",
            Profile::Sway => "Sway",
            Profile::Hyprland => "Hyprland",
            Profile::Niri => "niri",
            Profile::X11 => "X11",
            Profile::None => "none",
        }
    }
}

pub fn detect(config: &Config) -> Profile {
    match config.desktop.profile.as_str() {
        "gnome" => Profile::Gnome,
        "kde" => Profile::Kde,
        "cinnamon" => Profile::Cinnamon,
        "sway" => Profile::Sway,
        "hyprland" => Profile::Hyprland,
        "niri" => Profile::Niri,
        "x11" => Profile::X11,
        "none" => Profile::None,
        _ => from_env(&env),
    }
}

/// What gamescope sets for its children (`XDG_CURRENT_DESKTOP=gamescope`, `XDG_SESSION_TYPE=x11`, its Xwayland's `DISPLAY`, no
/// `WAYLAND_DISPLAY`): the launcher's own nested gamescope hands the desktop's on as `UNIVERSE_HOST_<VAR>`, empty where it had none.
pub const HOST_VARS: [&str; 4] = ["XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE", "DISPLAY", "WAYLAND_DISPLAY"];

pub fn host_var(key: &str) -> String {
    format!("UNIVERSE_HOST_{key}")
}

fn handed() -> bool {
    crate::nest::inside() && std::env::var_os(host_var("XDG_SESSION_TYPE")).is_some()
}

/// Inside a gamescope that handed the desktop's variables on, those, and no `GAMESCOPE_WAYLAND_DISPLAY`: that one is gamescope's.
fn env(key: &str) -> Option<String> {
    let name = match key {
        "GAMESCOPE_WAYLAND_DISPLAY" if handed() => return None,
        k if HOST_VARS.contains(&k) && handed() => host_var(k),
        k => k.to_string(),
    };
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The compositors' own sockets first: XDG_CURRENT_DESKTOP is whatever the session file claims.
fn from_env(var: &impl Fn(&str) -> Option<String>) -> Profile {
    if var("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return Profile::Hyprland;
    }
    if var("NIRI_SOCKET").is_some() {
        return Profile::Niri;
    }
    if var("SWAYSOCK").is_some() {
        return Profile::Sway;
    }
    let desktops = var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    if desktops.contains("gnome") {
        return Profile::Gnome;
    }
    if desktops.contains("kde") {
        return Profile::Kde;
    }
    if desktops.contains("cinnamon") {
        return Profile::Cinnamon;
    }
    if x11_session(var) {
        return Profile::X11;
    }
    Profile::None
}

/// Inside the launcher's gamescope `DISPLAY` is gamescope's own Xwayland and `WAYLAND_DISPLAY` unset: no sign of an X11 desktop.
fn x11_session(var: &impl Fn(&str) -> Option<String>) -> bool {
    var("XDG_SESSION_TYPE").as_deref() == Some("x11")
        || (var("GAMESCOPE_WAYLAND_DISPLAY").is_none() && var("DISPLAY").is_some() && var("WAYLAND_DISPLAY").is_none())
}

/// The desktop's X server, never the Xwayland of a gamescope that handed nothing on.
fn x11_display() -> Option<String> {
    if (crate::nest::inside() && !handed()) || !x11_session(&env) {
        return None;
    }
    env("DISPLAY")
}

fn x11_reachable() -> bool {
    x11_display().is_some()
}

const DRM_DIR: &str = "/sys/class/drm";

/// Every cabled connector, sorted, and whether the display server drives it: `card1-DP-1` is `DP-1`.
fn drm_outputs(dir: &str) -> Vec<(String, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut outputs: Vec<(String, bool)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let (_, connector) = name.split_once('-')?;
            let status = std::fs::read_to_string(e.path().join("status")).ok()?;
            if status.trim() != "connected" {
                return None;
            }
            // A driver that writes no `enabled` leaves the connector lit; "disabled" is a cable with nothing drawn on it.
            let lit = std::fs::read_to_string(e.path().join("enabled")).map_or(true, |s| s.trim() != "disabled");
            Some((connector.to_string(), lit))
        })
        .collect();
    outputs.sort();
    outputs
}

pub fn connected_outputs() -> Vec<String> {
    drm_outputs(DRM_DIR).into_iter().map(|(name, _)| name).collect()
}

/// The connectors something is drawn on — what a recorder or a screenshot can name. Cabled ones when the sysfs
/// flag leaves none, so a driver that lies still gets a screen rather than nothing.
pub fn active_outputs() -> Vec<String> {
    lit_or_cabled(drm_outputs(DRM_DIR))
}

fn lit_or_cabled(outputs: Vec<(String, bool)>) -> Vec<String> {
    let lit: Vec<String> = outputs.iter().filter(|(_, lit)| *lit).map(|(name, _)| name.clone()).collect();
    if lit.is_empty() {
        outputs.into_iter().map(|(name, _)| name).collect()
    } else {
        lit
    }
}

/// Qt/Mutter names HDMI outputs "HDMI-1"; DRM says "HDMI-A-1". gsr wants the DRM name.
fn normalize_connector(name: &str) -> String {
    let connected = connected_outputs();
    if connected.iter().any(|c| c == name) {
        return name.to_string();
    }
    if let Some(rest) = name.strip_prefix("HDMI-") {
        let cand = format!("HDMI-A-{rest}");
        if connected.iter().any(|c| c == &cand) {
            return cand;
        }
    }
    name.to_string()
}

pub fn pick_screen(requested: &str) -> String {
    if !requested.is_empty() {
        return normalize_connector(requested);
    }
    active_outputs().into_iter().next().unwrap_or_default()
}

/// Mutter's DisplayConfig on GNOME, else the connector's DRM mode; `None` when the connector is not there.
pub async fn screen_mode(screen: &str) -> Option<crate::gamescope::Mode> {
    if screen.is_empty() {
        return None;
    }
    match tokio::time::timeout(std::time::Duration::from_secs(5), mutter_current_mode(screen)).await {
        Ok(Ok(Some(mode))) => return Some(mode),
        Ok(Ok(None)) => {}
        Ok(Err(e)) => tracing::debug!("DisplayConfig.GetCurrentState: {e}"),
        Err(_) => tracing::warn!("DisplayConfig.GetCurrentState: timeout"),
    }
    drm_mode(screen)
}

/// The `is-current` mode of the monitor on `screen`; Mutter spells HDMI outputs without the `-A`, so both spellings match.
async fn mutter_current_mode(screen: &str) -> zbus::Result<Option<crate::gamescope::Mode>> {
    type Props = HashMap<String, zbus::zvariant::OwnedValue>;
    type Mode = (String, i32, i32, f64, f64, Vec<f64>, Props);
    type Monitor = ((String, String, String, String), Vec<Mode>, Props);
    type Logical = (i32, i32, f64, u32, bool, Vec<(String, String, String, String)>, Props);
    let conn = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(&conn, "org.gnome.Mutter.DisplayConfig", "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig").await?;
    let (_serial, monitors, _logical, _props): (u32, Vec<Monitor>, Vec<Logical>, Props) = proxy.call("GetCurrentState", &()).await?;
    let wanted = screen.replace("-A-", "-");
    for (info, modes, _) in monitors {
        if info.0 != screen && info.0.replace("-A-", "-") != wanted {
            continue;
        }
        // Mutter marks the modes a VRR screen can run variably with `refresh-rate-mode = "variable"`.
        let vrr = modes.iter().any(|(_, _, _, _, _, _, props)| props.get("refresh-rate-mode").and_then(|v| <&str>::try_from(v).ok()) == Some("variable"));
        for (_, w, h, hz, _, _, props) in modes {
            let current = props.get("is-current").and_then(|v| bool::try_from(v).ok()).unwrap_or(false);
            if current && w > 0 && h > 0 {
                return Ok(Some(crate::gamescope::Mode { width: w as u32, height: h as u32, refresh: hz.round() as u32, vrr }));
            }
        }
    }
    Ok(None)
}

struct Card(std::fs::File);
impl std::os::fd::AsFd for Card {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl drm::control::Device for Card {}

/// The connector's mode with its rate, asked of the card that owns it (`card1-DP-1` in sysfs): any compositor's, X11's too.
fn drm_mode(screen: &str) -> Option<crate::gamescope::Mode> {
    let rd = std::fs::read_dir(DRM_DIR).ok()?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((card, connector)) = name.split_once('-') else { continue };
        if connector != screen {
            continue;
        }
        if let Some(mode) = card_mode(card, screen) {
            return Some(mode);
        }
        // Without the device (no seat, no video group): the first line of `modes` is the preferred one, `WxH`, no rate.
        let modes = std::fs::read_to_string(e.path().join("modes")).ok()?;
        let first = modes.lines().next()?.trim();
        if let Ok(Some((w, h))) = crate::gamescope::parse_resolution(first) {
            return Some(crate::gamescope::Mode { width: w, height: h, refresh: 60, vrr: false });
        }
    }
    None
}

/// The mode the connector's CRTC scans out, else the preferred one of a connector nothing drives.
fn card_mode(card: &str, screen: &str) -> Option<crate::gamescope::Mode> {
    use drm::control::{Device as _, ModeTypeFlags};
    let dev = Card(std::fs::OpenOptions::new().read(true).write(true).open(format!("/dev/dri/{card}")).ok()?);
    let handles = dev.resource_handles().ok()?;
    let info = handles
        .connectors()
        .iter()
        .filter_map(|h| dev.get_connector(*h, false).ok())
        .find(|c| format!("{}-{}", c.interface().as_str(), c.interface_id()) == screen)?;
    let running = info.current_encoder().and_then(|h| dev.get_encoder(h).ok()).and_then(|e| e.crtc()).and_then(|h| dev.get_crtc(h).ok()).and_then(|c| c.mode());
    let mode = running.or_else(|| info.modes().iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).or_else(|| info.modes().first()).copied())?;
    let (w, h) = mode.size();
    let vrr = dev
        .get_properties(info.handle())
        .ok()
        .is_some_and(|props| props.iter().any(|(id, value)| dev.get_property(*id).is_ok_and(|p| p.name().to_bytes() == b"vrr_capable") && *value != 0));
    Some(crate::gamescope::Mode { width: w.into(), height: h.into(), refresh: mode.vrefresh(), vrr })
}

/// `(name, path, interface)`, each `Inhibit(app, reason) → cookie`. ScreenSaver holds off the blank (and GNOME's suspend); KDE's and Xfce's power managers suspend on their own, which only PowerManagement holds off.
const AWAKE_SERVICES: [(&str, &str, &str); 2] = [
    ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver", "org.freedesktop.ScreenSaver"),
    ("org.freedesktop.PowerManagement", "/org/freedesktop/PowerManagement/Inhibit", "org.freedesktop.PowerManagement.Inhibit"),
];

pub struct Inhibitor {
    conn: zbus::Connection,
    cookies: Vec<(usize, u32)>,
    idle_lock: Option<zbus::zvariant::OwnedFd>,
}

impl Inhibitor {
    pub async fn release(self) {
        for (i, cookie) in &self.cookies {
            let (name, path, interface) = AWAKE_SERVICES[*i];
            let released = async { zbus::Proxy::new(&self.conn, name, path, interface).await?.call::<_, _, ()>("UnInhibit", &(*cookie,)).await };
            if let Err(e) = released.await {
                tracing::warn!("{name} UnInhibit({cookie}): {e}");
            }
        }
    }

    pub fn held(&self) -> Vec<&'static str> {
        let services = self.cookies.iter().map(|(i, _)| AWAKE_SERVICES[*i].0);
        services.chain(self.idle_lock.is_some().then_some("logind")).collect()
    }
}

/// logind's IdleAction and hypridle wait on an `idle` lock. Not `sleep`: since systemd 257 a block lock refuses the user's own Suspend too.
async fn logind_idle_lock(reason: &str) -> zbus::Result<zbus::zvariant::OwnedFd> {
    let conn = zbus::Connection::system().await?;
    let proxy = zbus::Proxy::new(&conn, "org.freedesktop.login1", "/org/freedesktop/login1", "org.freedesktop.login1.Manager").await?;
    proxy.call("Inhibit", &("idle", "Universe", reason, "block")).await
}

/// GNOME counts keyboard and mouse alone as activity, and gamescope forwards no inhibitor of its own: a pad-played game idles the desktop.
pub async fn inhibit_idle(reason: &str) -> Result<Inhibitor, String> {
    let call = async {
        let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
        let mut cookies = Vec::new();
        let mut errors = Vec::new();
        for (i, (name, path, interface)) in AWAKE_SERVICES.into_iter().enumerate() {
            let taken = async { zbus::Proxy::new(&conn, name, path, interface).await?.call::<_, _, u32>("Inhibit", &("universe", reason)).await };
            match taken.await {
                Ok(cookie) => cookies.push((i, cookie)),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        let idle_lock = logind_idle_lock(reason).await.map_err(|e| errors.push(format!("logind: {e}"))).ok();
        let inhibitor = Inhibitor { conn, cookies, idle_lock };
        if inhibitor.held().is_empty() {
            return Err(errors.join("; "));
        }
        for e in errors {
            tracing::debug!("keep awake: {e}");
        }
        Ok(inhibitor)
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), call).await.unwrap_or_else(|_| Err("the session bus did not answer".into()))
}

pub async fn awake_services() -> Vec<&'static str> {
    let Ok(conn) = zbus::Connection::session().await else { return vec![] };
    let mut owned = Vec::new();
    for (name, _, _) in AWAKE_SERVICES {
        if name_owned(&conn, name).await {
            owned.push(name);
        }
    }
    owned
}

/// `id` is the desktop's own handle, which `activate_window` takes back: a number, a KWin uuid, a Hyprland address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Toplevel {
    pub id: String,
    pub pid: i64,
    pub wm_class: Option<String>,
    pub title: Option<String>,
    pub focused: bool,
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
    pub hidden: bool,
    pub minimized: bool,
}

/// What the desktop's focused toplevel is to a launcher: its own window (gamescope's, when it runs nested) or its running game's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct HostFocus {
    pub launcher: bool,
    pub session: bool,
}

impl HostFocus {
    /// `pid` is the focused toplevel's, 0 when none has the focus.
    pub fn of(pid: i64, launcher: &[i64], in_session: impl Fn(i64) -> bool) -> HostFocus {
        HostFocus { launcher: pid > 0 && launcher.contains(&pid), session: pid > 0 && in_session(pid) }
    }
}

fn unsupported(profile: Profile, what: &str) -> String {
    match profile {
        Profile::None => format!("no desktop profile ({what} needs one: desktop.profile)"),
        p => format!("{} has no {what} Universe can reach", p.name()),
    }
}

pub async fn list_windows(profile: Profile) -> Result<Vec<Toplevel>, String> {
    match profile {
        Profile::Gnome => gnome::list_windows().await,
        Profile::Kde => kde::list_windows().await,
        Profile::Sway => sway::list_windows().await,
        Profile::Hyprland => hyprland::list_windows().await,
        Profile::Niri => niri::list_windows().await,
        Profile::X11 | Profile::Cinnamon if x11_reachable() => x11::list_windows().await,
        p => Err(unsupported(p, "window list")),
    }
}

pub async fn activate_window(profile: Profile, id: &str) -> Result<bool, String> {
    match profile {
        Profile::Gnome => gnome::activate_window(id).await,
        Profile::Kde => kde::activate_window(id).await,
        Profile::Sway => sway::activate_window(id).await,
        Profile::Hyprland => hyprland::activate_window(id).await,
        Profile::Niri => niri::activate_window(id).await,
        Profile::X11 | Profile::Cinnamon if x11_reachable() => x11::activate_window(id).await,
        p => Err(unsupported(p, "window list")),
    }
}

/// The pid of the toplevel that has the focus, 0 when none has; KDE's comes from a script left running in KWin, which pushes each change.
pub async fn focused_pid(profile: Profile) -> Result<i64, String> {
    match profile {
        Profile::Kde => kde::focused_pid().await,
        p => Ok(list_windows(p).await?.iter().find(|w| w.focused).map_or(0, |w| w.pid)),
    }
}

/// `level` in [0, 1] draws a bar under `label`.
pub async fn show_osd(profile: Profile, icon: &str, label: &str, level: Option<f64>) -> Result<(), String> {
    match profile {
        Profile::Gnome => gnome::show_osd(icon, label, level).await,
        Profile::Kde => kde::show_osd(icon, label, level).await,
        Profile::Cinnamon => cinnamon::show_osd(icon, label, level).await,
        Profile::None => Err(unsupported(profile, "OSD")),
        _ => notify::show_osd(icon, label, level).await,
    }
}

/// `screen` is the DRM connector a whole-screen shot takes; `window` the focused window alone.
pub async fn screenshot(profile: Profile, path: &std::path::Path, screen: &str, window: bool, cursor: bool) -> Result<(), String> {
    let _ = std::fs::remove_file(path);
    match profile {
        Profile::Gnome => gnome::screenshot(path, window, cursor).await?,
        Profile::Kde => kde::screenshot(path, window, cursor).await?,
        Profile::Sway | Profile::Hyprland | Profile::Niri => {
            let region = if window { focused_region(profile).await } else { None };
            grim(path, screen, region, cursor).await?;
        }
        Profile::X11 | Profile::Cinnamon if x11_reachable() => x11::screenshot(path, window).await?,
        p => return Err(unsupported(p, "screenshot")),
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !std::fs::metadata(path).is_ok_and(|m| m.len() > 0) {
        if std::time::Instant::now() > deadline {
            return Err(format!("no screenshot at {}", path.display()));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Ok(())
}

/// niri lists no window's place on the output: its shot is the output's.
async fn focused_region(profile: Profile) -> Option<(i64, i64, i64, i64)> {
    if profile == Profile::Niri {
        return None;
    }
    let windows = list_windows(profile).await.ok()?;
    let w = windows.iter().find(|w| w.focused && w.width > 0 && w.height > 0)?;
    Some((w.x, w.y, w.width, w.height))
}

async fn grim(path: &std::path::Path, screen: &str, region: Option<(i64, i64, i64, i64)>, cursor: bool) -> Result<(), String> {
    let mut args: Vec<String> = Vec::new();
    match region {
        Some((x, y, w, h)) => args.extend(["-g".into(), format!("{x},{y} {w}x{h}")]),
        None if !screen.is_empty() => args.extend(["-o".into(), screen.to_string()]),
        None => {}
    }
    if cursor {
        args.push("-c".into());
    }
    args.push(path.to_string_lossy().into_owned());
    run("grim", &args).await.map(drop)
}

/// A desktop's own CLI, its stdout; the error names the program and what it printed.
pub(crate) async fn run(program: &str, args: &[String]) -> Result<String, String> {
    let out = tokio::time::timeout(std::time::Duration::from_secs(10), tokio::process::Command::new(program).args(args).kill_on_drop(true).output())
        .await
        .map_err(|_| format!("{program} did not answer"))?
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let said = if err.trim().is_empty() { String::from_utf8_lossy(&out.stdout).into_owned() } else { err.into_owned() };
        return Err(format!("{program} {}: {}", args.first().map(String::as_str).unwrap_or(""), said.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// What a session's cursor hiding changed: the stop, maybe from another process, puts it back from the marker.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "via", rename_all = "snake_case")]
pub enum CursorUndo {
    #[default]
    Nothing,
    Universe,
    Extension {
        uuid: String,
    },
    Kde {
        duration: Option<String>,
        loaded: bool,
    },
    Hyprland {
        seconds: f64,
    },
    Sway {
        ms: u32,
    },
}

/// Hides the pointer after `CURSOR_IDLE_S` at rest; `cursor_extension` is GNOME's alone.
pub async fn hide_cursor(profile: Profile, cursor_extension: &str) -> CursorUndo {
    let undo = match profile {
        Profile::Gnome => return gnome::hide_cursor(cursor_extension).await,
        Profile::Kde => kde::hide_cursor().await,
        Profile::Hyprland => hyprland::hide_cursor().await,
        Profile::Sway => sway::hide_cursor().await,
        _ => return CursorUndo::Nothing,
    };
    undo.unwrap_or_else(|e| {
        tracing::warn!("cursor hiding: {e}");
        CursorUndo::Nothing
    })
}

pub async fn restore_cursor(undo: &CursorUndo) {
    let restored = match undo {
        CursorUndo::Nothing => Ok(()),
        CursorUndo::Universe | CursorUndo::Extension { .. } => {
            gnome::restore_cursor(undo).await;
            Ok(())
        }
        CursorUndo::Kde { duration, loaded } => kde::restore_cursor(duration.as_deref(), *loaded).await,
        CursorUndo::Hyprland { seconds } => hyprland::restore_cursor(*seconds).await,
        CursorUndo::Sway { ms } => sway::restore_cursor(*ms).await,
    };
    if let Err(e) = restored {
        tracing::warn!("cursor restore: {e}");
    }
}

pub const CURSOR_IDLE_S: u32 = 5;

/// `(program, package, what it does)`: the programs a profile drives.
pub fn tools(profile: Profile) -> Vec<(&'static str, &'static str, &'static str)> {
    match profile {
        Profile::Sway => vec![("swaymsg", "sway", "window focus and cursor hiding"), ("grim", "grim", "screenshots")],
        Profile::Hyprland => vec![("hyprctl", "hyprland", "window focus and cursor hiding"), ("grim", "grim", "screenshots")],
        Profile::Niri => vec![("niri", "niri", "window focus"), ("grim", "grim", "screenshots")],
        Profile::Kde => vec![("spectacle", "spectacle", "screenshots"), ("kwriteconfig6", "kconfig", "cursor hiding")],
        _ => vec![],
    }
}

pub fn osd_by_notification(profile: Profile) -> bool {
    matches!(profile, Profile::Sway | Profile::Hyprland | Profile::Niri | Profile::X11)
}

pub async fn name_owned(conn: &zbus::Connection, name: &str) -> bool {
    let asked = async {
        zbus::Proxy::new(conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus")
            .await?
            .call::<_, _, bool>("NameHasOwner", &(name,))
            .await
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), asked).await.ok().and_then(|r| r.ok()).unwrap_or(false)
}

/// How the profile hides the resting cursor, or why it cannot: for doctor.
pub fn cursor_route(profile: Profile) -> Result<&'static str, &'static str> {
    match profile {
        Profile::Gnome => Ok("the Universe extension"),
        Profile::Kde => Ok("KWin's hidecursor effect (Plasma 6.1 and later)"),
        Profile::Hyprland => Ok("cursor:inactive_timeout"),
        Profile::Sway => Ok("seat * hide_cursor"),
        Profile::Niri => Err("niri hides the cursor from its own config only: cursor { hide-after-inactive-ms 5000; }"),
        Profile::Cinnamon | Profile::X11 => Err("no cursor hiding on this desktop: unclutter does it for any X11 session"),
        Profile::None => Err("no desktop profile"),
    }
}

pub fn pid_in_cgroup(pid: i64, cgroup: &str) -> bool {
    let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/cgroup")) else { return false };
    cgroup_matches(&text, cgroup)
}

pub fn cgroup_matches(proc_cgroup: &str, cgroup: &str) -> bool {
    let base = cgroup.trim_end_matches('/');
    proc_cgroup.lines().any(|l| {
        let path = l.rsplit(':').next().unwrap_or("");
        path == base || path.starts_with(&format!("{base}/"))
    })
}

/// The largest visible toplevel whose pid `in_unit` claims: gamescope's when the game runs inside it.
pub fn pick_window(windows: &[Toplevel], in_unit: impl Fn(i64) -> bool) -> Option<Toplevel> {
    windows
        .iter()
        .filter(|w| !w.hidden && !w.minimized && w.width > 0 && w.height > 0 && w.pid > 0 && in_unit(w.pid))
        .max_by_key(|w| w.width * w.height)
        .cloned()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compositors_socket_names_the_desktop_before_the_session_file() {
        let env = |vars: &'static [(&'static str, &'static str)]| move |k: &str| vars.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string());
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "Hyprland"), ("HYPRLAND_INSTANCE_SIGNATURE", "abc")])), Profile::Hyprland);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "sway"), ("SWAYSOCK", "/run/user/1000/sway-ipc.sock")])), Profile::Sway);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "niri"), ("NIRI_SOCKET", "/run/user/1000/niri.sock")])), Profile::Niri);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")])), Profile::Gnome);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "KDE")])), Profile::Kde);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "X-Cinnamon")])), Profile::Cinnamon);
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "XFCE"), ("XDG_SESSION_TYPE", "x11")])), Profile::X11);
        assert_eq!(from_env(&env(&[("DISPLAY", ":0")])), Profile::X11, "an X server and no Wayland one");
        assert_eq!(from_env(&env(&[("DISPLAY", ":2"), ("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0")])), Profile::None, "gamescope's own Xwayland");
        assert_eq!(from_env(&env(&[("XDG_CURRENT_DESKTOP", "COSMIC"), ("WAYLAND_DISPLAY", "wayland-1"), ("DISPLAY", ":1")])), Profile::None);
    }

    #[test]
    fn the_focused_window_is_the_launchers_its_games_or_another_apps() {
        let (launcher, game) = ([10, 20], |pid| pid == 30);
        assert_eq!(HostFocus::of(20, &launcher, game), HostFocus { launcher: true, session: false }, "the toplevel of the gamescope it runs nested in");
        assert_eq!(HostFocus::of(30, &launcher, game), HostFocus { launcher: false, session: true });
        assert_eq!(HostFocus::of(40, &launcher, game), HostFocus::default());
        assert_eq!(HostFocus::of(0, &[0], |_| true), HostFocus::default(), "nothing has the focus");
    }

    #[test]
    fn inside_the_nested_gamescope_the_desktop_is_the_one_it_started_on() {
        let _env = crate::paths::test_env();
        for var in ["GAMESCOPE_WAYLAND_DISPLAY", "WAYLAND_DISPLAY", "SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET"] {
            std::env::remove_var(var);
        }
        let set = |pairs: &[(&str, &str)]| pairs.iter().for_each(|(k, v)| std::env::set_var(k, v));
        set(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0")]);
        assert_eq!(x11_display().as_deref(), Some(":0"));
        set(&[("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0"), ("XDG_CURRENT_DESKTOP", "gamescope"), ("DISPLAY", ":1")]);
        assert_eq!(x11_display(), None, "a gamescope that handed nothing on: its Xwayland is no desktop");
        let host = |pairs: &[(&str, &str)]| pairs.iter().for_each(|(k, v)| std::env::set_var(host_var(k), v));
        host(&[("XDG_CURRENT_DESKTOP", ""), ("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "")]);
        assert_eq!((from_env(&env), x11_display().as_deref()), (Profile::X11, Some(":0")));
        host(&[("XDG_CURRENT_DESKTOP", "GNOME"), ("XDG_SESSION_TYPE", "wayland"), ("WAYLAND_DISPLAY", "wayland-0")]);
        assert_eq!((from_env(&env), x11_display()), (Profile::Gnome, None), "GNOME's Xwayland is no X11 desktop");
    }

    #[test]
    fn a_process_is_in_its_unit_or_under_it() {
        let cg = "/user.slice/user-1000.slice/user@1000.service/app.slice/universe-game-x-s.service";
        assert!(cgroup_matches(&format!("0::{cg}\n"), cg));
        assert!(cgroup_matches(&format!("0::{cg}/child\n"), &format!("{cg}/")));
        assert!(!cgroup_matches("0::/user.slice/user-1000.slice/user@1000.service/app.slice/other.service\n", cg));
    }

    #[test]
    fn a_cabled_but_dark_connector_is_not_one_to_record() {
        let dir = tempfile::tempdir().unwrap();
        let connector = |name: &str, status: &str, enabled: Option<&str>| {
            let path = dir.path().join(name);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("status"), format!("{status}\n")).unwrap();
            if let Some(enabled) = enabled {
                std::fs::write(path.join("enabled"), format!("{enabled}\n")).unwrap();
            }
        };
        connector("card1-DP-1", "connected", Some("disabled"));
        connector("card1-HDMI-A-1", "connected", Some("enabled"));
        connector("card1-DP-2", "disconnected", Some("disabled"));
        connector("card0-DP-3", "connected", None);
        let outputs = drm_outputs(dir.path().to_str().unwrap());
        assert_eq!(outputs, vec![("DP-1".into(), false), ("DP-3".into(), true), ("HDMI-A-1".into(), true)]);
        assert_eq!(lit_or_cabled(outputs), vec!["DP-3", "HDMI-A-1"], "the dark cable is left out");
        assert_eq!(lit_or_cabled(vec![("DP-1".into(), false)]), vec!["DP-1"], "nothing lit: the cabled one stands");
        assert!(drm_outputs("/nope/at/all").is_empty());
    }

    #[test]
    fn the_session_window_is_the_largest_visible_one_of_the_unit() {
        let w = |id: u32, pid, width, hidden| Toplevel { id: id.to_string(), pid, width, height: 100, hidden, ..Default::default() };
        let windows = vec![w(1, 10, 300, true), w(2, 10, 200, false), w(3, 11, 100, false), w(4, 99, 900, false), w(5, 0, 900, false)];
        assert_eq!(pick_window(&windows, |pid| pid == 10 || pid == 11).map(|w| w.id), Some("2".into()));
        assert!(pick_window(&windows, |_| false).is_none());
    }
}

/// Reads the card of every connected output: `just test-live`.
#[cfg(test)]
mod live {
    #[test]
    #[ignore]
    fn the_mode_of_every_lit_output_carries_a_rate_and_matches_the_desktops() {
        let outputs = super::active_outputs();
        assert!(!outputs.is_empty(), "a lit output");
        let rt = tokio::runtime::Runtime::new().unwrap();
        for name in outputs {
            let mode = super::drm_mode(&name).unwrap_or_else(|| panic!("{name}: no mode"));
            assert!(mode.width > 0 && mode.height > 0 && mode.refresh > 0, "{name}: {mode:?}");
            if let Ok(Some(shell)) = rt.block_on(super::mutter_current_mode(&name)) {
                assert_eq!((mode.width, mode.height, mode.refresh), (shell.width, shell.height, shell.refresh), "{name}: the CRTC runs what Mutter set");
            }
        }
    }

    /// Against the desktop this runs in, with a window open on it.
    #[test]
    #[ignore]
    fn the_desktops_windows_are_listed_and_focused_and_its_cursor_put_back() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let profile = super::from_env(&super::env);
        let windows = rt.block_on(super::list_windows(profile)).unwrap();
        eprintln!("{profile:?}: {windows:#?}");
        let with_pid: Vec<_> = windows.iter().filter(|w| w.pid > 0).collect();
        assert!(!with_pid.is_empty(), "a window with a pid");
        // Each in turn, so a desktop that pushes the focus (KWin's script) is seen to follow it.
        for w in with_pid.iter().rev().chain(with_pid.last()) {
            assert!(rt.block_on(super::activate_window(profile, &w.id)).unwrap(), "{w:?}");
            let focused = (0..30).any(|_| {
                std::thread::sleep(std::time::Duration::from_millis(100));
                rt.block_on(super::focused_pid(profile)) == Ok(w.pid)
            });
            assert!(focused, "{profile:?}: the focus never named pid {}: {:?}", w.pid, rt.block_on(super::focused_pid(profile)));
        }
        let gone = if profile == super::Profile::Hyprland { "0xdead" } else { "4000000000" };
        assert!(!rt.block_on(super::activate_window(profile, gone)).unwrap());
        let undo = rt.block_on(super::hide_cursor(profile, ""));
        eprintln!("cursor: {undo:?}");
        rt.block_on(super::restore_cursor(&undo));
    }
}
