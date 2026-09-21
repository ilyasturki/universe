use serde::Serialize;

use crate::config::Config;
use crate::modules::Module;
use crate::sources::Source;

/// `module` is the module or source the check belongs to, `core`, `runners`, `media` or `controller` otherwise.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub check: String,
    pub ok: bool,
    pub detail: String,
    pub module: String,
}

fn which(bin: &str) -> Option<String> {
    crate::runners::on_path(bin).map(|p| p.to_string_lossy().into())
}

/// Eden's `[UI] confirmStop`: 0 (its default) asks before closing, so a stop from Universe shows a question instead of quitting; 2 never asks.
fn eden_quits_on_stop() -> (bool, String) {
    let ini = crate::roms::qt_config(&crate::paths::xdg("XDG_CONFIG_HOME", ".config"), crate::roms::EDEN_CONFIGS);
    let value = ini.as_deref().and_then(|p| crate::roms::ini_value(p, "UI", "confirmStop")).unwrap_or_else(|| "0".into());
    let file = ini.map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "~/.config/eden/qt-config.ini".into());
    if value == "2" {
        (true, format!("quits on stop without asking ({file})"))
    } else {
        (
            false,
            format!("asks before closing: a stop from Universe shows the question and kills it 10 s later; set Configure › General › Confirm before stopping emulation to Never Ask ([UI] confirmStop=2 in {file})"),
        )
    }
}

pub async fn run(config: &Config, modules: &[Module], sources: &[Source], shell: Option<&zbus::Connection>, game_runners: &[String]) -> Vec<Check> {
    let mut out = Vec::new();
    let mut push = |check: &str, ok: bool, detail: String, module: &str| out.push(Check { check: check.into(), ok, detail, module: module.into() });

    let config_file = crate::paths::config_file();
    let config_state = if !config_file.exists() {
        " (absent: defaults)"
    } else if Config::writable(&config_file) {
        ""
    } else {
        " read-only: home-manager's programs.universe.settings"
    };
    push("config", true, format!("{}{config_state}", config_file.display()), "core");
    push("data", crate::paths::games_dir().is_dir(), crate::paths::games_dir().to_string_lossy().into(), "core");
    for bin in ["umu-run", "journalctl"] {
        push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| "missing".into()), "core");
    }
    // journald keeps the games' output across reboots only with /var/log/journal on disk (Storage=persistent, or auto with the directory made).
    let persistent = std::path::Path::new("/var/log/journal").is_dir();
    push(
        "journal-persistent",
        persistent,
        if persistent {
            "/var/log/journal: the games' logs survive a reboot".into()
        } else {
            "no /var/log/journal: a reboot drops the games' logs (journald Storage=persistent)".into()
        },
        "core",
    );
    if config.launch.fps_limit != "none" {
        push(
            "mangohud",
            which("mangohud").is_some(),
            which("mangohud").unwrap_or_else(|| "missing: no frame rate limit (launch.fps_limit = \"none\" to stop asking)".into()),
            "core",
        );
    }
    if config.launch.gamescope {
        let bin = &config.launch.gamescope_bin;
        push(
            "gamescope",
            which(bin).is_some(),
            which(bin).unwrap_or_else(|| format!("{bin} not found: games launch on the desktop (launch.gamescope = false to stop asking)")),
            "core",
        );
        push(
            "mangoapp",
            which("mangoapp").is_some(),
            which("mangoapp").unwrap_or_else(|| "missing: no HUD inside gamescope (the mangohud package ships it)".into()),
            "core",
        );
        let screen = crate::desktop::pick_screen("");
        let mode = crate::desktop::screen_mode(&screen).await;
        push(
            "screen",
            mode.is_some(),
            match mode {
                Some(m) => format!("{screen} {}×{} @ {} Hz: what gamescope's resolution follows on auto", m.width, m.height, m.refresh),
                None => "no connected output found: gamescope keeps its own 1280×720 unless launch.gamescope_resolution is set".into(),
            },
            "core",
        );
    }
    let proton = config.proton_path(&config.launch.proton);
    push("proton", proton.is_some(), proton.map(|p| p.to_string_lossy().into()).unwrap_or_else(|| format!("{} not found", config.launch.proton)), "core");
    let ext_ok = crate::desktop::extension_installed(&config.desktop.cursor_extension);
    push(
        "cursor-extension",
        ext_ok || !config.desktop.hide_cursor,
        format!("{} {}", config.desktop.cursor_extension, if ext_ok { "installed" } else { "missing" }),
        "core",
    );
    if crate::desktop::detect(config) == crate::desktop::Profile::Gnome {
        let uuid = crate::desktop::UNIVERSE_EXTENSION;
        let (ok, detail) = if !crate::desktop::extension_installed(uuid) {
            (false, "window capture falls back to the screen: the home-manager module installs the universe shell extension; log out to load it".to_string())
        } else {
            let mut state = None;
            if let Some(conn) = shell {
                if let Some(p) = crate::desktop::extensions_proxy(conn, crate::desktop::Profile::Gnome, uuid).await {
                    state = crate::desktop::extension_state(&p, uuid).await;
                }
            }
            match state {
                Some(s) if crate::desktop::extension_is_active(s) => (true, format!("{uuid} active")),
                Some(Some(_)) => (false, format!("installed but not enabled: gnome-extensions enable {uuid}")),
                Some(None) => (false, "installed, log out to load it".to_string()),
                None => (false, "installed; gnome shell unreachable".to_string()),
            }
        };
        push("universe-extension", ok, detail, "core");
    }
    push("recordings_root", config.recordings_root().is_dir(), config.recordings_root().to_string_lossy().into(), "core");
    push("journal_root", config.journal_root().is_dir(), config.journal_root().to_string_lossy().into(), "core");
    let used: std::collections::BTreeSet<&String> = game_runners.iter().chain(config.runners.keys()).collect();
    let mut inputplumber_wanted = false;
    for id in used {
        let Some(spec) = crate::runners::spec(id) else {
            push(&format!("runner-{id}"), false, "not a runner Universe ships".into(), "runners");
            continue;
        };
        if spec.kind == crate::runners::Kind::Linux || spec.kind == crate::runners::Kind::Proton {
            continue;
        }
        let located = crate::runners::locate(spec, config);
        let ok = !located.program.is_empty() && std::path::Path::new(&located.program).is_file();
        push(
            &format!("runner-{}", spec.id),
            ok,
            if ok {
                format!("{} ({})", located.program, located.source)
            } else {
                format!("{} not found: install it or set runners.{}.exe", spec.name, spec.id)
            },
            "runners",
        );
        inputplumber_wanted |=
            spec.kind == crate::runners::Kind::Emulator && spec.merged_options(config, None).get("inputplumber").and_then(|v| v.as_bool()).unwrap_or(false);
        if spec.id == "eden" && ok {
            let (quits, detail) = eden_quits_on_stop();
            push("runner-eden-stop", quits, detail, "runners");
        }
    }
    if inputplumber_wanted {
        let reachable = crate::inputplumber::reachable().await;
        push(
            "inputplumber",
            reachable,
            if reachable {
                "daemon reachable".into()
            } else {
                "daemon not on the system bus (services.inputplumber); emulators run on the raw pads (runners.<id>.inputplumber = false to stop asking)".into()
            },
            "runners",
        );
    }
    push(
        "key-sgdb",
        config.api_key("sgdb").is_some(),
        if config.api_key("sgdb").is_some() { "present".into() } else { format!("missing ({})", config.keys.sgdb_file) },
        "media",
    );
    push(
        "key-rawg",
        config.api_key("rawg").is_some(),
        if config.api_key("rawg").is_some() { "present".into() } else { format!("missing ({})", config.keys.rawg_file) },
        "media",
    );
    for m in modules {
        if !m.enabled {
            continue;
        }
        for bin in &m.manifest.requires.bins {
            let hint = if bin == "gsr-cli" { "missing (ships with gpu-screen-recorder 6.1 or later)" } else { "missing" };
            push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| hint.into()), m.id());
        }
        if m.id() == "capture" {
            push(
                "gsr-kms-server",
                which("gsr-kms-server").is_some(),
                which("gsr-kms-server").unwrap_or_else(|| "missing (programs.gpu-screen-recorder.enable)".into()),
                "capture",
            );
        }
    }
    for m in sources.iter().filter(|m| m.enabled) {
        for bin in &m.manifest.requires.bins {
            push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| "missing".into()), m.id());
        }
        if m.id() == "gog" {
            let auth = crate::paths::expand(m.merged_settings(config).get("auth_path").and_then(|v| v.as_str()).unwrap_or("~/.config/gogdl/auth.json"));
            let logged = std::fs::read_to_string(&auth).map(|s| s.contains("refresh_token")).unwrap_or(false);
            push("gog-auth", logged, if logged { auth.to_string_lossy().into() } else { "not logged in (universe login gog)".into() }, "gog");
        }
    }
    if config.controller.enabled {
        let uinput = std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok();
        push(
            "uinput",
            uinput,
            if uinput {
                "/dev/uinput writable".into()
            } else {
                "/dev/uinput not writable (key and MangoHud macros): hardware.uinput.enable and the uinput group".into()
            },
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
        push("pads", true, detail, "controller");
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
            unbound.is_empty(),
            if unbound.is_empty() {
                "every button of every pad answers".into()
            } else {
                format!("not seen on this connection, learn them: {}", unbound.join(", "))
            },
            "controller",
        );
    }
    let enabled_missing: Vec<&str> = config.modules.enabled.iter().filter(|e| !modules.iter().any(|m| m.id() == e.as_str())).map(|s| s.as_str()).collect();
    let hint = if enabled_missing.iter().any(|e| sources.iter().any(|s| s.id() == *e)) {
        " (a source: [sources] enabled in config.toml, `universe source enable`)"
    } else {
        ""
    };
    push(
        "modules",
        enabled_missing.is_empty(),
        if enabled_missing.is_empty() { format!("{} found", modules.len()) } else { format!("enabled but not found: {}{hint}", enabled_missing.join(", ")) },
        "core",
    );
    let sources_missing: Vec<&str> = config.sources.enabled.iter().filter(|e| !sources.iter().any(|s| s.id() == e.as_str())).map(|s| s.as_str()).collect();
    push(
        "sources",
        sources_missing.is_empty(),
        if sources_missing.is_empty() { format!("{} found", sources.len()) } else { format!("enabled but not found: {}", sources_missing.join(", ")) },
        "core",
    );
    out
}
