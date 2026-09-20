use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// The Universe GNOME Shell extension: window capture and the like; the home-manager module installs it.
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

pub fn connected_outputs() -> Vec<String> {
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return vec![] };
    let mut names: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let (_, connector) = name.split_once('-')?;
            let status = std::fs::read_to_string(e.path().join("status")).ok()?;
            (status.trim() == "connected").then(|| connector.to_string())
        })
        .collect();
    names.sort();
    names
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
    connected_outputs().into_iter().next().unwrap_or_default()
}

/// Mutter's DisplayConfig on GNOME, else the connector's preferred DRM mode; `None` when the connector is not there.
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
    drm_preferred_mode(screen)
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

/// The connector's preferred mode with its rate, asked of the card that owns it (`card1-DP-1` in sysfs).
fn drm_preferred_mode(screen: &str) -> Option<crate::gamescope::Mode> {
    let rd = std::fs::read_dir("/sys/class/drm").ok()?;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((card, connector)) = name.split_once('-') else { continue };
        if connector != screen {
            continue;
        }
        if let Some(mode) = card_preferred_mode(card, screen) {
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

fn card_preferred_mode(card: &str, screen: &str) -> Option<crate::gamescope::Mode> {
    use drm::control::{Device as _, ModeTypeFlags};
    let dev = Card(std::fs::OpenOptions::new().read(true).write(true).open(format!("/dev/dri/{card}")).ok()?);
    let handles = dev.resource_handles().ok()?;
    let info = handles
        .connectors()
        .iter()
        .filter_map(|h| dev.get_connector(*h, false).ok())
        .find(|c| format!("{}-{}", c.interface().as_str(), c.interface_id()) == screen)?;
    let mode = info.modes().iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).or_else(|| info.modes().first())?;
    let (w, h) = mode.size();
    let vrr = dev
        .get_properties(info.handle())
        .ok()
        .is_some_and(|props| props.iter().any(|(id, value)| dev.get_property(*id).is_ok_and(|p| p.name().to_bytes() == b"vrr_capable") && *value != 0));
    Some(crate::gamescope::Mode { width: w.into(), height: h.into(), refresh: mode.vrefresh(), vrr })
}

/// Returns whether the extension was already active.
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
    fn the_preferred_mode_of_every_connected_output_carries_a_rate() {
        let outputs = super::connected_outputs();
        assert!(!outputs.is_empty(), "a connected output");
        for name in outputs {
            let mode = super::drm_preferred_mode(&name).unwrap_or_else(|| panic!("{name}: no mode"));
            assert!(mode.width > 0 && mode.height > 0 && mode.refresh > 0, "{name}: {mode:?}");
            let modes = std::fs::read_to_string(format!("/sys/class/drm/card1-{name}/modes"))
                .or_else(|_| std::fs::read_to_string(format!("/sys/class/drm/card0-{name}/modes")))
                .unwrap_or_default();
            assert_eq!(
                modes.lines().next().map(str::trim),
                Some(format!("{}x{}", mode.width, mode.height).as_str()),
                "{name}: the card's preferred mode is sysfs's first"
            );
        }
    }
}
