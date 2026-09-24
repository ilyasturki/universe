use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The Universe GNOME Shell extension (`extension/`): window capture and the like.
pub const UNIVERSE_EXTENSION: &str = "universe@ilyasturki.github.io";

// ExtensionState.ACTIVE (js/misc/extensionUtils.js)
const EXTENSION_ACTIVE: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Gnome,
    None,
}

pub fn detect(config: &Config) -> Profile {
    match config.desktop.profile.as_str() {
        "gnome" => Profile::Gnome,
        "none" => Profile::None,
        _ => {
            let cur = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
            if cur.contains("gnome") {
                Profile::Gnome
            } else {
                Profile::None
            }
        }
    }
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
    let call = async {
        let conn = zbus::Connection::session().await.ok()?;
        let dbus = zbus::Proxy::new(&conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus").await.ok()?;
        let mut owned = Vec::new();
        for (name, _, _) in AWAKE_SERVICES {
            if dbus.call::<_, _, bool>("NameHasOwner", &(name,)).await.ok()? {
                owned.push(name);
            }
        }
        Some(owned)
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), call).await.ok().flatten().unwrap_or_default()
}

/// Returns whether the extension was already active. No `extension` is Universe's own, whose `HideCursor` hides the resting pointer.
pub async fn cursor_extension_enable(conn: &zbus::Connection, profile: Profile, extension: &str) -> bool {
    let Some(proxy) = extensions_proxy(conn, profile, extension).await else { return false };
    let was_active = extension_is_active(extension_state(&proxy, extension).await.flatten());
    if !was_active {
        call_bool(&proxy, "EnableExtension", extension).await;
    }
    was_active
}

/// `None` when the shell cannot be asked, `Some(None)` when it has not loaded the extension, else the ExtensionState.
pub async fn extension_state(proxy: &zbus::Proxy<'_>, extension: &str) -> Option<Option<f64>> {
    shell_call::<HashMap<String, zbus::zvariant::OwnedValue>>(proxy, "GetExtensionInfo", extension)
        .await
        .map(|info| info.get("state").and_then(|v| f64::try_from(v).ok()))
}

pub fn extension_is_active(state: Option<f64>) -> bool {
    state == Some(EXTENSION_ACTIVE)
}

pub async fn cursor_extension_restore(conn: &zbus::Connection, profile: Profile, extension: &str, was_active: bool) {
    if was_active {
        return;
    }
    if let Some(proxy) = extensions_proxy(conn, profile, extension).await {
        call_bool(&proxy, "DisableExtension", extension).await;
    }
}

pub async fn extensions_proxy(conn: &zbus::Connection, profile: Profile, extension: &str) -> Option<zbus::Proxy<'static>> {
    if profile != Profile::Gnome || extension.is_empty() {
        return None;
    }
    match zbus::Proxy::new(conn, "org.gnome.Shell", "/org/gnome/Shell", "org.gnome.Shell.Extensions").await {
        Ok(p) => Some(p),
        Err(e) => {
            tracing::warn!("gnome shell extensions proxy: {e}");
            None
        }
    }
}

async fn shell_call<T: serde::de::DeserializeOwned + zbus::zvariant::Type>(proxy: &zbus::Proxy<'_>, method: &str, extension: &str) -> Option<T> {
    match tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call::<_, _, T>(method, &(extension,))).await {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            tracing::warn!("{method}({extension}): {e}");
            None
        }
        Err(_) => {
            tracing::warn!("{method}({extension}): timeout");
            None
        }
    }
}

async fn call_bool(proxy: &zbus::Proxy<'_>, method: &str, extension: &str) -> bool {
    shell_call(proxy, method, extension).await.unwrap_or(false)
}

/// `id` is what the extension's `Activate` and Mutter's `RecordWindow` take.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Toplevel {
    pub id: u64,
    pub pid: i64,
    pub wm_class: Option<String>,
    pub title: Option<String>,
    pub focused: bool,
    pub width: i64,
    pub height: i64,
    pub hidden: bool,
    pub minimized: bool,
}

async fn windows_proxy() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    zbus::Proxy::new(&conn, "org.universe.Windows", "/org/universe/Windows", "org.universe.Windows").await.map_err(|e| e.to_string())
}

/// The shell's toplevels through the Universe extension; an error off GNOME or before the shell has loaded it.
pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    let proxy = windows_proxy().await?;
    let json: String = tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call("List", &()))
        .await
        .map_err(|_| "gnome-shell did not answer".to_string())?
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

pub async fn activate_window(id: u64) -> Result<bool, String> {
    let proxy = windows_proxy().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call("Activate", &(id,)))
        .await
        .map_err(|_| "gnome-shell did not answer".to_string())?
        .map_err(|e| e.to_string())
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

pub fn extension_installed(extension: &str) -> bool {
    let home = crate::paths::home().join(".local/share/gnome-shell/extensions").join(extension);
    if home.exists() {
        return true;
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/share".into());
    dirs.split(':').any(|d| std::path::Path::new(d).join("gnome-shell/extensions").join(extension).exists())
}

/// org.gnome.Shell.ShowOSD refuses callers other than gsd, so through the extension, enabled on the first call; `level` in [0, 1] shows the bar.
pub async fn show_osd(icon: &str, label: Option<&str>, level: Option<f64>) -> Result<(), String> {
    let args = (icon, label.unwrap_or(""), level.unwrap_or(-1.0));
    let call = async {
        let proxy = windows_proxy().await?;
        let Err(first) = proxy.call_method("ShowOSD", &args).await else { return Ok(()) };
        let Some(ext) = extensions_proxy(proxy.connection(), Profile::Gnome, UNIVERSE_EXTENSION).await else { return Err(first.to_string()) };
        if !call_bool(&ext, "EnableExtension", UNIVERSE_EXTENSION).await {
            return Err(format!("{first}; the shell has not loaded {UNIVERSE_EXTENSION}"));
        }
        proxy.call_method("ShowOSD", &args).await.map(|_| ()).map_err(|e| e.to_string())
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), call).await.unwrap_or_else(|_| Err("gnome-shell did not answer".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let w = |id, pid, width, hidden| Toplevel { id, pid, width, height: 100, hidden, ..Default::default() };
        let windows = vec![w(1, 10, 300, true), w(2, 10, 200, false), w(3, 11, 100, false), w(4, 99, 900, false), w(5, 0, 900, false)];
        assert_eq!(pick_window(&windows, |pid| pid == 10 || pid == 11).map(|w| w.id), Some(2));
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
}
