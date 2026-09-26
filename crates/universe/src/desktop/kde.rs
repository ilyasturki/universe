use std::sync::atomic::{AtomicU32, Ordering};

use futures_util::StreamExt;

use super::{run, CursorUndo, Toplevel, CURSOR_IDLE_S};

// Plasma 6 names first; Plasma 5 has clientList and activeClient.
const WINDOWS_JS: &str = r#"
const windows = workspace.windowList ? workspace.windowList() : workspace.clientList();
const active = workspace.windowList ? workspace.activeWindow : workspace.activeClient;
function activate(w) {
    w.minimized = false;
    if (workspace.windowList) workspace.activeWindow = w; else workspace.activeClient = w;
}
"#;

const LIST_JS: &str = r#"
answer(JSON.stringify(windows.filter(w => w.normalWindow).map(w => ({
    id: String(w.internalId), pid: w.pid, wm_class: w.resourceClass, title: w.caption, focused: w === active,
    x: Math.round(w.x), y: Math.round(w.y), width: Math.round(w.width), height: Math.round(w.height), minimized: w.minimized,
}))));
"#;

/// A KWin script runs inside the compositor and calls back on this connection with `answer(text)`: KWin scripting is the one window list KDE lets another process read.
async fn script(body: &str) -> Result<String, String> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let me = conn.unique_name().ok_or("no unique name on the session bus")?.to_string();
    let plugin = format!("universe-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed));
    let file = crate::paths::runtime_dir().join(format!("{plugin}.js"));
    let source = format!(
        "{WINDOWS_JS}\nfunction answer(text) {{ callDBus({}, \"/\", \"\", \"answer\", text); }}\n{body}",
        serde_json::to_string(&me).unwrap_or_default()
    );
    std::fs::write(&file, source).map_err(|e| format!("{}: {e}", file.display()))?;
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::MethodCall).member("answer").map_err(|e| e.to_string())?.build();
    let mut answers = zbus::MessageStream::for_match_rule(rule, &conn, None).await.map_err(|e| e.to_string())?;
    let scripting = zbus::Proxy::new(&conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting").await.map_err(|e| e.to_string())?;
    let ran = async {
        let id: i32 = scripting.call("loadScript", &(file.to_string_lossy().as_ref(), plugin.as_str())).await.map_err(|e| format!("KWin loadScript: {e}"))?;
        if id < 0 {
            return Err(format!("KWin refused script {plugin}"));
        }
        // Plasma 6 exports the script under /Scripting, Plasma 5 at the root.
        let mut started = Err(String::new());
        for path in [format!("/Scripting/Script{id}"), format!("/{id}")] {
            let run = async { zbus::Proxy::new(&conn, "org.kde.KWin", path.as_str(), "org.kde.kwin.Script").await?.call::<_, _, ()>("run", &()).await };
            started = run.await.map_err(|e| format!("KWin script run: {e}"));
            if started.is_ok() {
                break;
            }
        }
        started?;
        while let Some(msg) = answers.next().await {
            let Ok(msg) = msg else { continue };
            let header = msg.header();
            if header.member().is_some_and(|m| m.as_str() == "answer") {
                let _ = conn.reply(&header, &()).await;
                return msg.body().deserialize::<String>().map_err(|e| e.to_string());
            }
        }
        Err("the session bus closed".to_string())
    };
    let answer = tokio::time::timeout(std::time::Duration::from_secs(5), ran).await.unwrap_or_else(|_| Err("KWin ran no script within 5 s".into()));
    let _ = scripting.call::<_, _, bool>("unloadScript", &(plugin.as_str(),)).await;
    let _ = std::fs::remove_file(&file);
    answer
}

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    serde_json::from_str(&script(LIST_JS).await?).map_err(|e| format!("KWin window list: {e}"))
}

pub async fn activate_window(id: &str) -> Result<bool, String> {
    let body = format!(
        "const w = windows.find(w => String(w.internalId) === {});\nif (w) activate(w);\nanswer(w ? \"true\" : \"false\");\n",
        serde_json::to_string(id).unwrap_or_default()
    );
    Ok(script(&body).await? == "true")
}

async fn osd_proxy() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    zbus::Proxy::new(&conn, "org.kde.plasmashell", "/org/kde/osdService", "org.kde.osdService").await.map_err(|e| e.to_string())
}

/// showProgress is private: mediaPlayerVolumeChanged is the public bar that takes a label and an icon.
pub async fn show_osd(icon: &str, label: &str, level: Option<f64>) -> Result<(), String> {
    let proxy = osd_proxy().await?;
    let shown = match level {
        Some(level) => proxy.call::<_, _, ()>("mediaPlayerVolumeChanged", &(percent(level), label, icon)).await,
        None => proxy.call::<_, _, ()>("showText", &(icon, label)).await,
    };
    shown.map_err(|e| format!("plasmashell OSD: {e}"))
}

fn percent(level: f64) -> i32 {
    (level.clamp(0.0, 1.0) * 100.0).round() as i32
}

pub async fn screenshot(path: &std::path::Path, window: bool, cursor: bool) -> Result<(), String> {
    let mut args: Vec<String> = vec!["-b".into(), "-n".into(), if window { "-a" } else { "-f" }.into()];
    if cursor {
        args.push("-p".into());
    }
    args.extend(["-o".into(), path.to_string_lossy().into_owned()]);
    run("spectacle", &args).await.map(drop)
}

async fn effects() -> Result<zbus::Proxy<'static>, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    zbus::Proxy::new(&conn, "org.kde.KWin", "/Effects", "org.kde.kwin.Effects").await.map_err(|e| e.to_string())
}

const KWINRC: [&str; 6] = ["--file", "kwinrc", "--group", "Effect-hidecursor", "--key", "InactivityDuration"];

fn kwinrc(extra: &[&str]) -> Vec<String> {
    KWINRC.iter().chain(extra).map(|s| s.to_string()).collect()
}

/// The hidecursor effect (Plasma 6.1) hides nothing at its default duration of 0: the duration is set for the session, the user's put back after.
pub async fn hide_cursor() -> Result<CursorUndo, String> {
    let effects = effects().await?;
    let known: Vec<String> = effects.get_property("listOfEffects").await.map_err(|e| format!("KWin effects: {e}"))?;
    if !known.iter().any(|e| e == "hidecursor") {
        return Err("KWin has no hidecursor effect before Plasma 6.1".into());
    }
    let loaded: bool = effects.call("isEffectLoaded", &("hidecursor",)).await.map_err(|e| e.to_string())?;
    let duration = run("kreadconfig6", &kwinrc(&[])).await?.trim().to_string();
    if loaded && duration.parse::<u32>().is_ok_and(|d| d > 0) {
        return Ok(CursorUndo::Nothing);
    }
    run("kwriteconfig6", &kwinrc(&[&CURSOR_IDLE_S.to_string()])).await?;
    let method = if loaded { "reconfigureEffect" } else { "loadEffect" };
    effects.call_method(method, &("hidecursor",)).await.map_err(|e| format!("KWin {method}: {e}"))?;
    Ok(CursorUndo::Kde { duration: Some(duration).filter(|d| !d.is_empty()), loaded })
}

pub async fn restore_cursor(duration: Option<&str>, loaded: bool) -> Result<(), String> {
    match duration {
        Some(d) => run("kwriteconfig6", &kwinrc(&[d])).await?,
        None => run("kwriteconfig6", &kwinrc(&["--delete"])).await?,
    };
    let effects = effects().await?;
    let method = if loaded { "reconfigureEffect" } else { "unloadEffect" };
    effects.call_method(method, &("hidecursor",)).await.map(drop).map_err(|e| format!("KWin {method}: {e}"))
}
