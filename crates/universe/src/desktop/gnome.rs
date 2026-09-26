use std::collections::HashMap;

use super::{CursorUndo, Toplevel};

pub const UNIVERSE_EXTENSION: &str = "universe@ilyasturki.github.io";

// ExtensionState.ACTIVE (js/misc/extensionUtils.js)
const EXTENSION_ACTIVE: f64 = 1.0;

/// No `extension` is Universe's own, whose `HideCursor` hides the resting pointer; another is enabled for the session when it was off.
pub async fn hide_cursor(extension: &str) -> CursorUndo {
    if extension.is_empty() {
        if let Err(e) = universe_extension_call::<_, ()>("HideCursor", &(true,)).await {
            tracing::warn!("cursor hiding: {e}");
        }
        return CursorUndo::Universe;
    }
    let Some(conn) = crate::host::session_bus().await else { return CursorUndo::Nothing };
    let Some(proxy) = extensions_proxy(&conn).await else { return CursorUndo::Nothing };
    if extension_is_active(extension_state(&proxy, extension).await.flatten()) {
        return CursorUndo::Nothing;
    }
    call_bool(&proxy, "EnableExtension", extension).await;
    CursorUndo::Extension { uuid: extension.to_string() }
}

pub async fn restore_cursor(undo: &CursorUndo) {
    match undo {
        CursorUndo::Universe => {
            if let Err(e) = universe_extension_call::<_, ()>("HideCursor", &(false,)).await {
                tracing::warn!("cursor hiding: {e}");
            }
        }
        CursorUndo::Extension { uuid } => {
            let Some(conn) = crate::host::session_bus().await else { return };
            if let Some(proxy) = extensions_proxy(&conn).await {
                call_bool(&proxy, "DisableExtension", uuid).await;
            }
        }
        _ => {}
    }
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

pub async fn extensions_proxy(conn: &zbus::Connection) -> Option<zbus::Proxy<'static>> {
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

async fn windows_proxy() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    zbus::Proxy::new(&conn, "org.universe.Windows", "/org/universe/Windows", "org.universe.Windows").await.map_err(|e| e.to_string())
}

/// `id` is what the extension's `Activate` and Mutter's `RecordWindow` take, a number.
#[derive(Default, serde::Deserialize)]
#[serde(default)]
struct Window {
    id: u64,
    pid: i64,
    wm_class: Option<String>,
    title: Option<String>,
    focused: bool,
    width: i64,
    height: i64,
    hidden: bool,
    minimized: bool,
}

pub fn parse_windows(json: &str) -> Result<Vec<Toplevel>, String> {
    let windows: Vec<Window> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(windows
        .into_iter()
        .map(|w| Toplevel {
            id: w.id.to_string(),
            pid: w.pid,
            wm_class: w.wm_class,
            title: w.title,
            focused: w.focused,
            width: w.width,
            height: w.height,
            hidden: w.hidden,
            minimized: w.minimized,
            ..Default::default()
        })
        .collect())
}

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    let proxy = windows_proxy().await?;
    let json: String = tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call("List", &()))
        .await
        .map_err(|_| "gnome-shell did not answer".to_string())?
        .map_err(|e| e.to_string())?;
    parse_windows(&json)
}

pub async fn activate_window(id: &str) -> Result<bool, String> {
    let id: u64 = id.parse().map_err(|_| format!("not a shell window id: {id}"))?;
    let proxy = windows_proxy().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call("Activate", &(id,)))
        .await
        .map_err(|_| "gnome-shell did not answer".to_string())?
        .map_err(|e| e.to_string())
}

pub fn extension_installed(extension: &str) -> bool {
    let home = crate::paths::home().join(".local/share/gnome-shell/extensions").join(extension);
    if home.exists() {
        return true;
    }
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/share".into());
    dirs.split(':').any(|d| std::path::Path::new(d).join("gnome-shell/extensions").join(extension).exists())
}

/// org.gnome.Shell.ShowOSD refuses callers other than gsd, so through the extension; `level` in [0, 1] shows the bar.
pub async fn show_osd(icon: &str, label: &str, level: Option<f64>) -> Result<(), String> {
    universe_extension_call("ShowOSD", &(icon, label, level.unwrap_or(-1.0))).await
}

/// The extension answers once the pixels are grabbed; the PNG lands a moment later.
pub async fn screenshot(path: &std::path::Path, window: bool, cursor: bool) -> Result<(), String> {
    let taken: bool = universe_extension_call("Screenshot", &(path.to_string_lossy().as_ref(), window, cursor)).await?;
    if taken {
        Ok(())
    } else {
        Err("gnome-shell refused the screenshot".into())
    }
}

/// A method of the Universe extension, which the shell is asked to enable when nobody answers the first call.
async fn universe_extension_call<B, R>(method: &str, body: &B) -> Result<R, String>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: serde::de::DeserializeOwned + zbus::zvariant::Type,
{
    let call = async {
        let proxy = windows_proxy().await?;
        let first = match proxy.call::<_, B, R>(method, body).await {
            Ok(r) => return Ok(r),
            Err(e) => e,
        };
        let Some(ext) = extensions_proxy(proxy.connection()).await else { return Err(first.to_string()) };
        if !call_bool(&ext, "EnableExtension", UNIVERSE_EXTENSION).await {
            return Err(format!("{first}; the shell has not loaded {UNIVERSE_EXTENSION}"));
        }
        // EnableExtension answers before enable() has taken the bus name.
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Ok(r) = proxy.call::<_, B, R>(method, body).await {
                return Ok(r);
            }
        }
    };
    let timeout = if method == "Screenshot" { 15 } else { 5 };
    tokio::time::timeout(std::time::Duration::from_secs(timeout), call).await.unwrap_or_else(|_| Err(format!("{UNIVERSE_EXTENSION} did not answer {method}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_extensions_numeric_ids_come_back_as_handles() {
        let windows = super::parse_windows(r#"[{"id":123456789012,"pid":42,"wm_class":"steam_app_1","focused":true,"width":10,"height":20}]"#).unwrap();
        assert_eq!((windows[0].id.as_str(), windows[0].pid, windows[0].focused), ("123456789012", 42, true));
    }
}
