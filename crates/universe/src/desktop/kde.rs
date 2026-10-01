use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

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

// Plasma 6's signal first; Plasma 5 has clientActivated.
const FOCUS_JS: &str = r#"
function report(w) { answer(String(w ? w.pid : 0)); }
(workspace.windowActivated || workspace.clientActivated).connect(report);
report(active);
"#;

const FOCUS_PREFIX: &str = "universe-focus-";

fn source(me: &str, body: &str) -> String {
    format!("{WINDOWS_JS}\nfunction answer(text) {{ callDBus({}, \"/\", \"\", \"answer\", text); }}\n{body}", serde_json::to_string(me).unwrap_or_default())
}

async fn answers(conn: &zbus::Connection) -> Result<zbus::MessageStream, String> {
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::MethodCall).member("answer").map_err(|e| e.to_string())?.build();
    zbus::MessageStream::for_match_rule(rule, conn, None).await.map_err(|e| e.to_string())
}

/// KWin's callDBus waits on the reply: each answer is acknowledged.
async fn next_answer(conn: &zbus::Connection, answers: &mut zbus::MessageStream) -> Option<Result<String, String>> {
    while let Some(msg) = answers.next().await {
        let Ok(msg) = msg else { continue };
        let header = msg.header();
        if header.member().is_some_and(|m| m.as_str() == "answer") {
            let _ = conn.reply(&header, &()).await;
            return Some(msg.body().deserialize::<String>().map_err(|e| e.to_string()));
        }
    }
    None
}

async fn scripting(conn: &zbus::Connection) -> Result<zbus::Proxy<'static>, String> {
    zbus::Proxy::new(conn, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting").await.map_err(|e| e.to_string())
}

async fn load(conn: &zbus::Connection, scripting: &zbus::Proxy<'_>, file: &std::path::Path, plugin: &str) -> Result<(), String> {
    let id: i32 = scripting.call("loadScript", &(file.to_string_lossy().as_ref(), plugin)).await.map_err(|e| format!("KWin loadScript: {e}"))?;
    if id < 0 {
        return Err(format!("KWin refused script {plugin}"));
    }
    // Plasma 6 exports the script under /Scripting, Plasma 5 at the root.
    let mut started = Err(String::new());
    for path in [format!("/Scripting/Script{id}"), format!("/{id}")] {
        let run = async { zbus::Proxy::new(conn, "org.kde.KWin", path.as_str(), "org.kde.kwin.Script").await?.call::<_, _, ()>("run", &()).await };
        started = run.await.map_err(|e| format!("KWin script run: {e}"));
        if started.is_ok() {
            break;
        }
    }
    started
}

/// A KWin script runs inside the compositor and calls back on this connection with `answer(text)`: KWin scripting is the one window list KDE lets another process read.
async fn script(body: &str) -> Result<String, String> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let me = conn.unique_name().ok_or("no unique name on the session bus")?.to_string();
    let plugin = format!("universe-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed));
    let file = crate::paths::runtime_dir().join(format!("{plugin}.js"));
    std::fs::write(&file, source(&me, body)).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut answers = answers(&conn).await?;
    let scripting = scripting(&conn).await?;
    let ran = async {
        load(&conn, &scripting, &file, &plugin).await?;
        next_answer(&conn, &mut answers).await.unwrap_or_else(|| Err("the session bus closed".to_string()))
    };
    let answer = tokio::time::timeout(std::time::Duration::from_secs(5), ran).await.unwrap_or_else(|_| Err("KWin ran no script within 5 s".into()));
    let _ = scripting.call::<_, _, bool>("unloadScript", &(plugin.as_str(),)).await;
    let _ = std::fs::remove_file(&file);
    answer
}

/// The focus script this process left running in KWin, and the pid of the active window as it last pushed it.
struct Resident {
    conn: zbus::Connection,
    plugin: String,
    pid: std::sync::Arc<AtomicI64>,
    reader: tokio::task::JoinHandle<()>,
}

static RESIDENT: tokio::sync::Mutex<Option<Resident>> = tokio::sync::Mutex::const_new(None);

/// One script per call costs a file, a load and up to 5 s: the focus comes from a script left running, loaded again once KWin lost it (a restart).
pub async fn focused_pid() -> Result<i64, String> {
    let mut resident = RESIDENT.lock().await;
    if let Some(r) = resident.as_ref().filter(|r| !r.reader.is_finished()) {
        let loaded = async { scripting(&r.conn).await?.call::<_, _, bool>("isScriptLoaded", &(r.plugin.as_str(),)).await.map_err(|e| e.to_string()) };
        if tokio::time::timeout(std::time::Duration::from_secs(2), loaded).await.is_ok_and(|l| l == Ok(true)) {
            return Ok(r.pid.load(Ordering::Relaxed));
        }
    }
    if let Some(stale) = resident.take() {
        stale.reader.abort();
    }
    let started = tokio::time::timeout(std::time::Duration::from_secs(5), resident_script()).await;
    let r = started.unwrap_or_else(|_| Err("KWin ran no focus script within 5 s".into()))?;
    let pid = r.pid.load(Ordering::Relaxed);
    *resident = Some(r);
    Ok(pid)
}

async fn resident_script() -> Result<Resident, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let me = conn.unique_name().ok_or("no unique name on the session bus")?.to_string();
    let plugin = format!("{FOCUS_PREFIX}{}", std::process::id());
    let scripting = scripting(&conn).await?;
    sweep(&scripting, &plugin).await;
    let file = crate::paths::runtime_dir().join(format!("{plugin}.js"));
    std::fs::write(&file, source(&me, FOCUS_JS)).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut answers = answers(&conn).await?;
    let first = match load(&conn, &scripting, &file, &plugin).await {
        Ok(()) => next_answer(&conn, &mut answers).await.unwrap_or_else(|| Err("the session bus closed".to_string())),
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_file(&file);
    let pid = std::sync::Arc::new(AtomicI64::new(first?.parse().unwrap_or(0)));
    let (seen, replies) = (pid.clone(), conn.clone());
    let reader = tokio::spawn(async move {
        while let Some(text) = next_answer(&replies, &mut answers).await {
            if let Ok(text) = text {
                seen.store(text.parse().unwrap_or(0), Ordering::Relaxed);
            }
        }
    });
    Ok(Resident { conn, plugin, pid, reader })
}

/// A process gone without unloading its focus script leaves it pushing to nobody: each one loaded is listed under the runtime dir, the dead ones unloaded.
async fn sweep(scripting: &zbus::Proxy<'_>, ours: &str) {
    let list = crate::paths::runtime_dir().join("kwin-focus-scripts");
    let (dead, alive) = stale_scripts(&std::fs::read_to_string(&list).unwrap_or_default(), |pid| std::path::Path::new(&format!("/proc/{pid}")).exists());
    for name in dead.iter().chain([&ours.to_string()]) {
        let _ = scripting.call::<_, _, bool>("unloadScript", &(name.as_str(),)).await;
    }
    let kept: Vec<String> = alive.into_iter().filter(|n| n != ours).chain([ours.to_string()]).collect();
    let _ = std::fs::write(&list, kept.join("\n") + "\n");
}

/// The listed focus scripts whose process is gone, and the rest.
fn stale_scripts(names: &str, alive: impl Fn(u32) -> bool) -> (Vec<String>, Vec<String>) {
    names
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .partition(|n| n.strip_prefix(FOCUS_PREFIX).and_then(|p| p.parse().ok()).is_none_or(|pid| !alive(pid)))
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

#[cfg(test)]
mod tests {
    #[test]
    fn the_focus_scripts_of_gone_processes_are_the_stale_ones() {
        let listed = "universe-focus-10\n\nuniverse-focus-20\nuniverse-focus-x\n";
        let (dead, alive) = super::stale_scripts(listed, |pid| pid == 20);
        assert_eq!(dead, ["universe-focus-10", "universe-focus-x"]);
        assert_eq!(alive, ["universe-focus-20"]);
    }
}
