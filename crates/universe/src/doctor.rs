use serde::Serialize;

use crate::config::Config;
use crate::modules::Module;

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

pub async fn run(config: &Config, modules: &[Module], shell: Option<&zbus::Connection>, game_runners: &[String]) -> Vec<Check> {
    let mut out = Vec::new();
    let mut push = |check: &str, ok: bool, detail: String, module: &str| out.push(Check { check: check.into(), ok, detail, module: module.into() });

    push("config", true, crate::paths::config_file().to_string_lossy().into(), "core");
    push("data", crate::paths::games_dir().is_dir(), crate::paths::games_dir().to_string_lossy().into(), "core");
    for bin in ["umu-run", "systemd-run", "systemctl"] {
        push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| "missing".into()), "core");
    }
    if config.launch.gamescope {
        let bin = &config.launch.gamescope_bin;
        push("gamescope", which(bin).is_some(), which(bin).unwrap_or_else(|| format!("{bin} not found: games launch on the desktop (launch.gamescope = false to stop asking)")), "core");
        if config.launch.mangohud {
            push("mangoapp", which("mangoapp").is_some(), which("mangoapp").unwrap_or_else(|| "missing: no HUD inside gamescope (the mangohud package ships it)".into()), "core");
        }
    }
    let proton = config.proton_path(&config.launch.proton);
    push("proton", proton.is_some(), proton.map(|p| p.to_string_lossy().into()).unwrap_or_else(|| format!("{} not found", config.launch.proton)), "core");
    let ext_ok = crate::desktop::extension_installed(&config.desktop.cursor_extension);
    push("cursor-extension", ext_ok || !config.desktop.hide_cursor, format!("{} {}", config.desktop.cursor_extension, if ext_ok { "installed" } else { "missing" }), "core");
    if crate::desktop::detect(config) == crate::desktop::Profile::Gnome {
        let uuid = crate::desktop::UNIVERSE_EXTENSION;
        let (ok, detail) = if !crate::desktop::extension_installed(uuid) {
            (false, "window capture falls back to the screen: the home-manager module installs the universe shell extension; log out to load it".to_string())
        } else {
            let proxy = match shell {
                Some(conn) => crate::desktop::extensions_proxy(conn, crate::desktop::Profile::Gnome, uuid).await,
                None => None,
            };
            let state = match &proxy {
                Some(p) => crate::desktop::extension_state(p, uuid).await,
                None => None,
            };
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
        push(&format!("runner-{}", spec.id), ok, if ok { format!("{} ({})", located.program, located.source) } else { format!("{} not found: install it or set runners.{}.exe", spec.name, spec.id) }, "runners");
        inputplumber_wanted |= spec.kind == crate::runners::Kind::Emulator && spec.merged_options(config, None).get("inputplumber").and_then(|v| v.as_bool()).unwrap_or(false);
    }
    if inputplumber_wanted {
        let installed = crate::inputplumber::installed();
        let reachable = installed && crate::inputplumber::reachable();
        push("inputplumber", reachable, if reachable { "daemon reachable".into() } else if installed { "daemon not reachable on the system bus (services.inputplumber)".into() } else { "inputplumber not on PATH; emulators run on the raw pads (runners.<id>.inputplumber = false to stop asking)".into() }, "runners");
    }
    push("key-sgdb", config.api_key("sgdb").is_some(), if config.api_key("sgdb").is_some() { "present".into() } else { format!("missing ({})", config.keys.sgdb_file) }, "media");
    push("key-rawg", config.api_key("rawg").is_some(), if config.api_key("rawg").is_some() { "present".into() } else { format!("missing ({})", config.keys.rawg_file) }, "media");
    for m in modules {
        if !m.enabled {
            continue;
        }
        for bin in &m.manifest.requires.bins {
            push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| "missing".into()), m.id());
        }
        if m.id() == "capture" {
            push("gsr-kms-server", which("gsr-kms-server").is_some(), which("gsr-kms-server").unwrap_or_else(|| "missing (programs.gpu-screen-recorder.enable)".into()), "capture");
        }
        if m.is_source() && m.id() == "gog" {
            let auth = crate::paths::expand(m.merged_settings(config, None).get("auth_path").and_then(|v| v.as_str()).unwrap_or("~/.config/gogdl/auth.json"));
            let logged = std::fs::read_to_string(&auth).map(|s| s.contains("refresh_token")).unwrap_or(false);
            push("gog-auth", logged, if logged { auth.to_string_lossy().into() } else { "not logged in (universe login gog)".into() }, "gog");
        }
    }
    if config.controller.enabled {
        let uinput = std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok();
        push("uinput", uinput, if uinput { "/dev/uinput writable".into() } else { "/dev/uinput not writable (key and MangoHud macros): hardware.uinput.enable and the uinput group".into() }, "controller");
        let pads = crate::controller::watch::enumerate_json(&config.controller);
        let detail = if pads.is_empty() {
            "no pad connected".to_string()
        } else {
            pads.iter().map(|p| format!("{} ({}, {})", p["name"].as_str().unwrap_or(""), p["family_name"].as_str().unwrap_or(""), p["bus"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("; ")
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
        push("pad-buttons", unbound.is_empty(), if unbound.is_empty() { "every button of every pad answers".into() } else { format!("not seen on this connection, learn them: {}", unbound.join(", ")) }, "controller");
    }
    let enabled_missing: Vec<&str> = config.modules.enabled.iter().filter(|e| !modules.iter().any(|m| m.id() == e.as_str())).map(|s| s.as_str()).collect();
    push("modules", enabled_missing.is_empty(), if enabled_missing.is_empty() { format!("{} found", modules.len()) } else { format!("enabled but not found: {}", enabled_missing.join(", ")) }, "core");
    out
}
