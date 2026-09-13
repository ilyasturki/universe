use std::collections::HashMap;

use crate::config::Config;

/// The Universe GNOME Shell extension: window capture and the like; the home-manager module installs it.
pub const UNIVERSE_EXTENSION: &str = "universe@ilyasturki.github.io";

// ExtensionState.ACTIVE (js/misc/extensionUtils.js)
const EXTENSION_ACTIVE: f64 = 1.0;

/// The four desktop-dependent operations (plan §6). GNOME is the only profile in the MVP.
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

/// Connected DRM connectors, e.g. ["DP-1"]; the first one is the fallback screen.
pub fn connected_outputs() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return out };
    let mut names: Vec<String> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let status = std::fs::read_to_string(e.path().join("status")).ok()?;
            if status.trim() != "connected" {
                return None;
            }
            let (_, conn) = name.split_once('-')?;
            Some(conn.to_string())
        })
        .collect();
    names.sort();
    out.extend(names);
    out
}

/// Qt/Mutter names HDMI outputs "HDMI-1"; DRM says "HDMI-A-1". gsr wants the DRM name.
pub fn normalize_connector(name: &str) -> String {
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

/// Enables the cursor-hiding GNOME Shell extension for the session (plan §6); returns whether it was already active.
pub async fn cursor_extension_enable(conn: &zbus::Connection, profile: Profile, extension: &str) -> bool {
    let Some(proxy) = extensions_proxy(conn, profile, extension).await else { return false };
    let was_active = extension_is_active(extension_state(&proxy, extension).await.flatten());
    if !was_active {
        call_bool(&proxy, "EnableExtension", extension).await;
    }
    was_active
}

/// What the shell knows of an extension: `None` when it cannot be asked, `Some(None)` when it has not
/// loaded it (`GetExtensionInfo` answers an empty dict), else the ExtensionState.
pub async fn extension_state(proxy: &zbus::Proxy<'_>, extension: &str) -> Option<Option<f64>> {
    match tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call::<_, _, HashMap<String, zbus::zvariant::OwnedValue>>("GetExtensionInfo", &(extension,))).await {
        Ok(Ok(info)) => Some(info.get("state").and_then(|v| f64::try_from(v).ok())),
        Ok(Err(e)) => {
            tracing::warn!("GetExtensionInfo({extension}): {e}");
            None
        }
        Err(_) => {
            tracing::warn!("GetExtensionInfo({extension}): timeout");
            None
        }
    }
}

pub fn extension_is_active(state: Option<f64>) -> bool {
    state == Some(EXTENSION_ACTIVE)
}

/// Disables the extension again unless it was active before the session.
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

async fn call_bool(proxy: &zbus::Proxy<'_>, method: &str, extension: &str) -> bool {
    match tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call::<_, _, bool>(method, &(extension,))).await {
        Ok(Ok(ok)) => ok,
        Ok(Err(e)) => {
            tracing::warn!("{method}({extension}): {e}");
            false
        }
        Err(_) => {
            tracing::warn!("{method}({extension}): timeout");
            false
        }
    }
}

pub fn extension_installed(extension: &str) -> bool {
    let home = crate::paths::home().join(".local/share/gnome-shell/extensions").join(extension);
    if home.exists() {
        return true;
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/share".into());
    dirs.split(':').any(|d| std::path::Path::new(d).join("gnome-shell/extensions").join(extension).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_keeps_unknown() {
        assert_eq!(normalize_connector("DP-9"), "DP-9");
    }
}
