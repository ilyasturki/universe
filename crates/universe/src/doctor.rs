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
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push("/run/wrappers/bin".into());
    dirs.iter().map(|d| d.join(bin)).find(|p| p.is_file()).map(|p| p.to_string_lossy().into())
}

pub fn run(config: &Config, modules: &[Module]) -> Vec<Check> {
    let mut out = Vec::new();
    let mut push = |check: &str, ok: bool, detail: String, module: &str| out.push(Check { check: check.into(), ok, detail, module: module.into() });

    push("config", true, crate::paths::config_file().to_string_lossy().into(), "core");
    push("data", crate::paths::games_dir().is_dir(), crate::paths::games_dir().to_string_lossy().into(), "core");
    for bin in ["umu-run", "systemd-run", "systemctl"] {
        push(bin, which(bin).is_some(), which(bin).unwrap_or_else(|| "missing".into()), "core");
    }
    let proton = config.proton_path(&config.launch.proton);
    push("proton", proton.is_some(), proton.map(|p| p.to_string_lossy().into()).unwrap_or_else(|| format!("{} not found", config.launch.proton)), "core");
    let ext_ok = crate::desktop::extension_installed(&config.desktop.cursor_extension);
    push("cursor-extension", ext_ok || !config.desktop.hide_cursor, format!("{} {}", config.desktop.cursor_extension, if ext_ok { "installed" } else { "missing" }), "core");
    push("recordings_root", config.recordings_root().is_dir(), config.recordings_root().to_string_lossy().into(), "core");
    push("journal_root", config.journal_root().is_dir(), config.journal_root().to_string_lossy().into(), "core");
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
            let auth = crate::paths::expand(&m.merged_settings(config, None).get("auth_path").and_then(|v| v.as_str()).unwrap_or("~/.config/gogdl/auth.json").to_string());
            let logged = std::fs::read_to_string(&auth).map(|s| s.contains("refresh_token")).unwrap_or(false);
            push("gog-auth", logged, if logged { auth.to_string_lossy().into() } else { "not logged in (universe login gog)".into() }, "gog");
        }
    }
    if config.controller.enabled {
        let uinput = std::fs::OpenOptions::new().write(true).open("/dev/uinput").is_ok();
        push("uinput", uinput, if uinput { "/dev/uinput writable".into() } else { "/dev/uinput not writable: hardware.uinput.enable and the uinput group".into() }, "controller");
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
