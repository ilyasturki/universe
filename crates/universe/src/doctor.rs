use serde::Serialize;

use crate::config::Config;
use crate::modules::Module;
use crate::sources::Source;

/// `check` is the stable id the docs cite; `detail` is the problem when `ok` is false, and `fix` is empty when it is true.
/// `module` is the module or source the check belongs to, `core`, `runners`, `media` or `controller` otherwise.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub check: String,
    pub label: String,
    pub ok: bool,
    pub detail: String,
    pub fix: String,
    pub module: String,
}

fn which(bin: &str) -> Option<String> {
    crate::runners::on_path(bin).map(|p| p.to_string_lossy().into())
}

/// Eden's `[UI] confirmStop`: 0 (its default) asks before closing, so a stop from Universe shows a question instead of quitting; 2 never asks.
fn eden_quits_on_stop() -> (bool, String, String) {
    let ini = crate::roms::qt_config(&crate::paths::xdg("XDG_CONFIG_HOME", ".config"), crate::roms::EDEN_CONFIGS);
    let value = ini.as_deref().and_then(|p| crate::roms::ini_value(p, "UI", "confirmStop")).unwrap_or_else(|| "0".into());
    let file = ini.map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "~/.config/eden/qt-config.ini".into());
    if value == "2" {
        (true, format!("quits on stop without asking ({file})"), String::new())
    } else {
        (
            false,
            "Eden asks before closing: a stop from Universe shows its question, then kills it 10 s later".into(),
            format!("in Eden, set Configure › General › Confirm before stopping emulation to Never Ask ([UI] confirmStop=2 in {file})"),
        )
    }
}

pub async fn run(config: &Config, modules: &[Module], sources: &[Source], shell: Option<&zbus::Connection>, game_runners: &[String]) -> Vec<Check> {
    let mut out = Vec::new();
    let mut push = |check: &str, label: &str, ok: bool, detail: String, fix: String, module: &str| {
        out.push(Check { check: check.into(), label: label.into(), ok, detail, fix: if ok { String::new() } else { fix }, module: module.into() })
    };
    let bin = |bin: &str| (which(bin).is_some(), which(bin).unwrap_or_else(|| format!("{bin} is not on PATH")));

    let config_file = crate::paths::config_file();
    let config_state = if !config_file.exists() {
        " (absent: defaults)"
    } else if Config::writable(&config_file) {
        ""
    } else {
        " read-only: home-manager's programs.universe.settings"
    };
    push("config", "Config file", true, format!("{}{config_state}", config_file.display()), String::new(), "core");
    let games = crate::paths::games_dir();
    push(
        "data",
        "Games folder",
        games.is_dir(),
        if games.is_dir() { games.to_string_lossy().into() } else { format!("{} does not exist", games.display()) },
        "restart Universe: it creates the folder on start".into(),
        "core",
    );
    let (ok, detail) = bin("umu-run");
    push("umu-run", "Proton launcher (umu-run)", ok, detail, "install umu-launcher".into(), "core");
    let (ok, detail) = bin("journalctl");
    push("journalctl", "Game logs (journalctl)", ok, detail, "install systemd's journalctl".into(), "core");
    // journald keeps the games' output across reboots only with /var/log/journal on disk (Storage=persistent, or auto with the directory made).
    let persistent = std::path::Path::new("/var/log/journal").is_dir();
    push(
        "journal-persistent",
        "Game logs kept across reboots",
        persistent,
        if persistent { "/var/log/journal: the games' logs survive a reboot".into() } else { "no /var/log/journal: a reboot drops the games' logs".into() },
        "set Storage=persistent in journald.conf".into(),
        "core",
    );
    if config.launch.fps_limit != "none" {
        push(
            "mangohud",
            "Frame rate limit (MangoHud)",
            which("mangohud").is_some(),
            which("mangohud").unwrap_or_else(|| "mangohud is not on PATH: games run without a frame rate limit".into()),
            "install MangoHud, or set launch.fps_limit = \"none\" to stop asking".into(),
            "core",
        );
    }
    if config.launch.gamescope {
        let bin_name = &config.launch.gamescope_bin;
        push(
            "gamescope",
            "Gamescope",
            which(bin_name).is_some(),
            which(bin_name).unwrap_or_else(|| format!("{bin_name} not found: games launch on the desktop")),
            "install gamescope, or set launch.gamescope = false to stop asking".into(),
            "core",
        );
        push(
            "mangoapp",
            "HUD inside Gamescope (mangoapp)",
            which("mangoapp").is_some(),
            which("mangoapp").unwrap_or_else(|| "mangoapp is not on PATH: no HUD inside gamescope".into()),
            "install MangoHud: its package ships mangoapp".into(),
            "core",
        );
        let screen = crate::desktop::pick_screen("");
        let mode = crate::desktop::screen_mode(&screen).await;
        push(
            "screen",
            "Screen resolution",
            mode.is_some(),
            match mode {
                Some(m) => format!("{screen} {}×{} @ {} Hz: what gamescope's resolution follows on auto", m.width, m.height, m.refresh),
                None => "no connected output found: gamescope keeps its own 1280×720".into(),
            },
            "set launch.gamescope_resolution".into(),
            "core",
        );
    }
    let proton = config.proton_path(&config.launch.proton);
    push(
        "proton",
        &format!("Proton ({})", config.launch.proton),
        proton.is_some(),
        proton.map(|p| p.to_string_lossy().into()).unwrap_or_else(|| format!("{} not found", config.launch.proton)),
        format!("install {0}, or point [proton] {0} at it in config.toml", config.launch.proton),
        "core",
    );
    let ext_ok = crate::desktop::extension_installed(&config.desktop.cursor_extension);
    push(
        "cursor-extension",
        "Cursor hiding extension",
        ext_ok || !config.desktop.hide_cursor,
        format!("{} {}", config.desktop.cursor_extension, if ext_ok { "installed" } else { "missing" }),
        format!("install the {} shell extension, or set desktop.hide_cursor = false to stop asking", config.desktop.cursor_extension),
        "core",
    );
    if config.desktop.keep_awake {
        let ok = crate::desktop::screensaver_available().await;
        push(
            "keep-awake",
            "Desktop stays awake in game",
            ok,
            if ok {
                "org.freedesktop.ScreenSaver: the desktop stays awake while a game runs".into()
            } else {
                "no org.freedesktop.ScreenSaver on the session bus: the desktop may blank or suspend mid-game".into()
            },
            "run a desktop that offers org.freedesktop.ScreenSaver, or set desktop.keep_awake = false to stop asking".into(),
            "core",
        );
    }
    if crate::desktop::detect(config) == crate::desktop::Profile::Gnome {
        let uuid = crate::desktop::UNIVERSE_EXTENSION;
        let (ok, detail, fix) = if !crate::desktop::extension_installed(uuid) {
            (
                false,
                "not installed: window capture falls back to the whole screen".to_string(),
                "the home-manager module installs the universe shell extension; log out and back in to load it".to_string(),
            )
        } else {
            let mut state = None;
            if let Some(conn) = shell {
                if let Some(p) = crate::desktop::extensions_proxy(conn, crate::desktop::Profile::Gnome, uuid).await {
                    state = crate::desktop::extension_state(&p, uuid).await;
                }
            }
            match state {
                Some(s) if crate::desktop::extension_is_active(s) => (true, format!("{uuid} active"), String::new()),
                Some(Some(_)) => (false, "installed but not enabled".to_string(), format!("gnome-extensions enable {uuid}")),
                Some(None) => (false, "installed but not loaded yet".to_string(), "log out and back in to load it".to_string()),
                None => (false, "installed; GNOME Shell did not answer".to_string(), "check that GNOME Shell is running, then refresh".to_string()),
            }
        };
        push("universe-extension", "Universe shell extension", ok, detail, fix, "core");
    }
    for (check, label, dir, key) in [
        ("recordings_root", "Recordings folder", config.recordings_root(), "recordings_root"),
        ("journal_root", "Journal folder", config.journal_root(), "journal_root"),
    ] {
        push(
            check,
            label,
            dir.is_dir(),
            if dir.is_dir() { dir.to_string_lossy().into() } else { format!("{} does not exist", dir.display()) },
            format!("create it, or set paths.{key} to a folder that exists"),
            "core",
        );
    }
    let used: std::collections::BTreeSet<&String> = game_runners.iter().chain(config.runners.keys()).collect();
    let mut inputplumber_wanted = false;
    for id in used {
        let Some(spec) = crate::runners::spec(id) else {
            push(
                &format!("runner-{id}"),
                &format!("Runner {id}"),
                false,
                format!("{id} is not a runner Universe ships"),
                format!("pick another runner for the games that name {id}, or remove [runners.{id}] from config.toml"),
                "runners",
            );
            continue;
        };
        if spec.kind == crate::runners::Kind::Linux || spec.kind == crate::runners::Kind::Proton {
            continue;
        }
        let located = crate::runners::locate(spec, config);
        let ok = !located.program.is_empty() && std::path::Path::new(&located.program).is_file();
        push(
            &format!("runner-{}", spec.id),
            spec.name,
            ok,
            if ok { format!("{} ({})", located.program, located.source) } else { format!("{} not found", spec.name) },
            format!("install it, or set runners.{}.exe", spec.id),
            "runners",
        );
        inputplumber_wanted |=
            spec.kind == crate::runners::Kind::Emulator && spec.merged_options(config, None).get("inputplumber").and_then(|v| v.as_bool()).unwrap_or(false);
        if spec.id == "eden" && ok {
            let (quits, detail, fix) = eden_quits_on_stop();
            push("runner-eden-stop", "Eden quits on stop", quits, detail, fix, "runners");
        }
    }
    if inputplumber_wanted {
        let reachable = crate::inputplumber::reachable().await;
        push(
            "inputplumber",
            "InputPlumber",
            reachable,
            if reachable { "daemon reachable".into() } else { "daemon not on the system bus: emulators get the raw pads".into() },
            "enable services.inputplumber, or set runners.<id>.inputplumber = false to stop asking".into(),
            "runners",
        );
    }
    for (check, label, which_key, file) in
        [("key-sgdb", "SteamGridDB API key", "sgdb", &config.keys.sgdb_file), ("key-rawg", "RAWG API key", "rawg", &config.keys.rawg_file)]
    {
        let present = config.api_key(which_key).is_some();
        push(
            check,
            label,
            present,
            if present { "present".into() } else { format!("missing: no key in keys.{which_key} nor in {file}") },
            format!("put your key in {file}"),
            "media",
        );
    }
    for m in modules {
        if !m.enabled {
            continue;
        }
        for b in m.manifest.requires.bins.iter().chain(m.missing.iter()).collect::<std::collections::BTreeSet<_>>() {
            let (ok, detail) = bin(b);
            let fix = if b == "gsr-cli" {
                "install gpu-screen-recorder 6.1 or later: it ships gsr-cli".to_string()
            } else {
                format!("install {b}, or turn the {} module off", m.manifest.name)
            };
            push(b, b, ok, detail, fix, m.id());
        }
        for key in &m.unset {
            let label = m.manifest.settings.iter().find(|s| &s.key == key).map(|s| s.label.clone()).unwrap_or_else(|| key.clone());
            push(
                key,
                &label,
                false,
                format!("not chosen yet: the {} module does nothing until it is", m.manifest.name),
                format!("choose it in Settings › Modules, or run universe module set {} {key}=…", m.id()),
                m.id(),
            );
        }
        if m.id() == "capture" {
            let (ok, detail) = bin("gsr-kms-server");
            push("gsr-kms-server", "gsr-kms-server", ok, detail, "set programs.gpu-screen-recorder.enable".into(), "capture");
        }
    }
    for m in sources.iter().filter(|m| m.enabled) {
        for b in &m.manifest.requires.bins {
            let (ok, detail) = bin(b);
            push(b, b, ok, detail, format!("install {b}, or turn the {} source off", m.manifest.name), m.id());
        }
        if m.id() == "gog" {
            let auth = crate::paths::expand(m.merged_settings(config).get("auth_path").and_then(|v| v.as_str()).unwrap_or("~/.config/gogdl/auth.json"));
            let logged = std::fs::read_to_string(&auth).map(|s| s.contains("refresh_token")).unwrap_or(false);
            push(
                "gog-auth",
                "GOG login",
                logged,
                if logged { auth.to_string_lossy().into() } else { "not logged in".into() },
                "run universe login gog".into(),
                "gog",
            );
        }
    }
    if config.controller.enabled {
        let uinput = std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok();
        push(
            "uinput",
            "Key and MangoHud macros (uinput)",
            uinput,
            if uinput { "/dev/uinput writable".into() } else { "/dev/uinput is not writable: key and MangoHud macros do nothing".into() },
            "set hardware.uinput.enable and add yourself to the uinput group".into(),
            "controller",
        );
        let pads = crate::controller::watch::enumerate_json(&config.controller);
        let detail = if pads.is_empty() {
            "no pad connected".to_string()
        } else {
            pads.iter()
                .map(|p| format!("{} ({}, {})", p["name"].as_str().unwrap_or(""), p["family_name"].as_str().unwrap_or(""), p["bus"].as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("; ")
        };
        push("pads", "Controllers", true, detail, String::new(), "controller");
        let mut unbound: Vec<String> = Vec::new();
        for p in &pads {
            let name = p["family_name"].as_str().unwrap_or("");
            for (k, v) in p["slots"].as_object().into_iter().flatten() {
                if v["bound"] == false {
                    unbound.push(format!("{name} {k}"));
                }
            }
        }
        push(
            "pad-buttons",
            "Controller buttons",
            unbound.is_empty(),
            if unbound.is_empty() { "every button of every pad answers".into() } else { format!("not seen on this connection: {}", unbound.join(", ")) },
            "learn them in Settings › Controller".into(),
            "controller",
        );
    }
    let enabled_missing: Vec<&str> = config.modules.enabled.iter().filter(|e| !modules.iter().any(|m| m.id() == e.as_str())).map(|s| s.as_str()).collect();
    let fix = match enabled_missing.iter().find(|e| sources.iter().any(|s| s.id() == **e)) {
        Some(source) => format!("{source} is a source: move it to [sources] enabled in config.toml (universe source enable {source})"),
        None => "remove them from [modules] enabled in config.toml, or install them".into(),
    };
    push(
        "modules",
        "Enabled modules",
        enabled_missing.is_empty(),
        if enabled_missing.is_empty() { format!("{} found", modules.len()) } else { format!("enabled but not found: {}", enabled_missing.join(", ")) },
        fix,
        "core",
    );
    let sources_missing: Vec<&str> = config.sources.enabled.iter().filter(|e| !sources.iter().any(|s| s.id() == e.as_str())).map(|s| s.as_str()).collect();
    push(
        "sources",
        "Enabled sources",
        sources_missing.is_empty(),
        if sources_missing.is_empty() { format!("{} found", sources.len()) } else { format!("enabled but not found: {}", sources_missing.join(", ")) },
        "remove them from [sources] enabled in config.toml, or install them".into(),
        "core",
    );
    out
}
