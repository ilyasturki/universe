use serde::Serialize;

use crate::config::Config;
use crate::distro::Family;
use crate::modules::Module;
use crate::sources::Source;

/// ExitType=cgroup, which ends a game's unit with its last process rather than the one it started.
const SYSTEMD_MIN: u32 = 250;

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

/// MangoHud with its 32-bit layer, which 32-bit games (Proton's included) load.
fn mangohud_install(family: Family) -> &'static str {
    match family {
        Family::NixOs => "add pkgs.mangohud to your packages",
        Family::Arch => "install mangohud and lib32-mangohud",
        Family::Fedora => "install mangohud and mangohud.i686",
        Family::Debian => "install mangohud and mangohud:i386",
        Family::Other => "install MangoHud and its 32-bit build",
    }
}

/// Native gpu-screen-recorder 6.1+: its Flatpak cannot be driven from outside, and gsr-kms-server needs `cap_sys_admin` (Arch's package sets it).
fn gsr_fix(family: Family) -> &'static str {
    match family {
        Family::NixOs => "set programs.gpu-screen-recorder.enable",
        Family::Arch => "install gpu-screen-recorder",
        _ => "install gpu-screen-recorder 6.1 or later natively (not its Flatpak), then sudo setcap cap_sys_admin+ep on its gsr-kms-server",
    }
}

/// The Vulkan loader's implicit layer directories: `$XDG_CONFIG_HOME`, `$XDG_CONFIG_DIRS`, `/etc`, `$XDG_DATA_HOME`, `$XDG_DATA_DIRS`, the system data dirs.
fn vulkan_layer_dirs() -> Vec<std::path::PathBuf> {
    let list = |var: &str, default: &str| -> Vec<std::path::PathBuf> {
        std::env::var(var).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| default.into()).split(':').filter(|d| !d.is_empty()).map(Into::into).collect()
    };
    let mut roots = vec![crate::paths::xdg("XDG_CONFIG_HOME", ".config")];
    roots.extend(list("XDG_CONFIG_DIRS", "/etc/xdg"));
    roots.push("/etc".into());
    roots.push(crate::paths::xdg("XDG_DATA_HOME", ".local/share"));
    roots.extend(list("XDG_DATA_DIRS", ""));
    roots.extend(["/usr/local/share".into(), "/usr/share".into()]);
    roots.into_iter().map(|r| r.join("vulkan/implicit_layer.d")).collect()
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
    let family = crate::distro::detect();
    let nixos = family == Family::NixOs;

    let config_file = crate::paths::config_file();
    let config_state = if !config_file.exists() {
        " (absent: defaults)"
    } else if Config::writable(&config_file) {
        ""
    } else if nixos {
        " read-only: home-manager's programs.universe.settings"
    } else {
        " read-only"
    };
    push("config", "Config file", true, format!("{}{config_state}", config_file.display()), String::new(), "core");
    let bus = match shell {
        Some(conn) => Some(conn.clone()),
        None => crate::host::session_bus().await,
    };
    let version = match &bus {
        Some(conn) => crate::host::systemd_version(conn).await,
        None => None,
    };
    push(
        "systemd",
        "systemd user manager",
        version.is_some_and(|v| v >= SYSTEMD_MIN),
        match version {
            Some(v) if v >= SYSTEMD_MIN => format!("systemd {v}"),
            Some(v) => format!("systemd {v}: games run as user units that need {SYSTEMD_MIN} or later"),
            None => "no systemd user manager on the session bus: games cannot start".into(),
        },
        format!("run Universe in a session with a systemd {SYSTEMD_MIN}+ user manager (systemctl --user)"),
        "core",
    );
    let cgroup2 = std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists();
    push(
        "cgroup2",
        "Unified cgroups (v2)",
        cgroup2,
        if cgroup2 { "/sys/fs/cgroup is cgroup v2".into() } else { "/sys/fs/cgroup is not cgroup v2: a game cannot be paused, nor its end told".into() },
        "boot with the unified hierarchy (systemd.unified_cgroup_hierarchy=1, the default since systemd 247)".into(),
        "core",
    );
    let games = crate::paths::games_dir();
    push(
        "data",
        "Games folder",
        games.is_dir(),
        if games.is_dir() { games.to_string_lossy().into() } else { format!("{} does not exist", games.display()) },
        "restart Universe: it creates the folder on start".into(),
        "core",
    );
    match (which("umu-run"), crate::tools::find("umu-run")) {
        (Some(path), _) => push("umu-run", "Proton launcher (umu-run)", true, path, String::new(), "core"),
        (None, Some(t)) => {
            let python = which("python3").is_some();
            push(
                "umu-run",
                "Proton launcher (umu-run)",
                python,
                if python {
                    format!("not installed: Universe fetches {} {} into {} at the first Proton launch", t.package, t.version, crate::tools::dir().display())
                } else {
                    format!("not installed, and the {} {} Universe would fetch is a Python zipapp: no python3 on PATH", t.package, t.version)
                },
                "install python3 (3.10 or later), or umu-launcher".into(),
                "core",
            );
        }
        (None, None) => push("umu-run", "Proton launcher (umu-run)", false, "umu-run is not on PATH".into(), "install umu-launcher".into(), "core"),
    }
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
            format!("{}, or set launch.fps_limit = \"none\" to stop asking", mangohud_install(family)),
            "core",
        );
    }
    if which("mangohud").is_some() {
        let dirs = vulkan_layer_dirs();
        let layer = dirs.iter().map(|d| d.join("MangoHud.x86.json")).find(|p| p.is_file());
        push(
            "mangohud-32bit",
            "MangoHud in 32-bit games",
            layer.is_some(),
            layer
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "no 32-bit MangoHud layer: a 32-bit game gets neither the frame rate limit nor the HUD".into()),
            mangohud_install(family).into(),
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
            if nixos {
                "set programs.gamescope.enable, or set launch.gamescope = false to stop asking"
            } else {
                "install gamescope, or set launch.gamescope = false to stop asking"
            }
            .into(),
            "core",
        );
        push(
            "mangoapp",
            "HUD inside Gamescope (mangoapp)",
            which("mangoapp").is_some(),
            which("mangoapp")
                .unwrap_or_else(|| "mangoapp is not on PATH: inside gamescope the MangoHud layer in the game draws the HUD, Vulkan games only".into()),
            match family {
                Family::Debian => "install the mangoapp package".into(),
                Family::NixOs => "add pkgs.mangohud to your packages: it ships mangoapp".into(),
                _ => "install MangoHud: its package ships mangoapp".into(),
            },
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
    let name = &config.launch.proton;
    let (ok, detail) = match (config.proton_path(name), crate::config::umu_codename(name)) {
        (Some(p), _) => (true, p.to_string_lossy().into_owned()),
        (None, Some(codename)) => (true, format!("not installed: umu-run downloads the latest {codename} at the first launch")),
        (None, None) => (false, format!("{name} not found: umu-run runs games on its own UMU-Proton instead")),
    };
    push("proton", &format!("Proton ({name})"), ok, detail, format!("install {name}, or point [proton] {name} at it in config.toml"), "core");
    let gnome = crate::desktop::detect(config) == crate::desktop::Profile::Gnome;
    if gnome {
        let ext_ok = crate::desktop::extension_installed(&config.desktop.cursor_extension);
        push(
            "cursor-extension",
            "Cursor hiding extension",
            ext_ok || !config.desktop.hide_cursor,
            format!("{} {}", config.desktop.cursor_extension, if ext_ok { "installed" } else { "missing" }),
            format!("install the {} shell extension, or set desktop.hide_cursor = false to stop asking", config.desktop.cursor_extension),
            "core",
        );
    }
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
    if gnome {
        let uuid = crate::desktop::UNIVERSE_EXTENSION;
        let (ok, detail, fix) = if !crate::desktop::extension_installed(uuid) {
            (
                false,
                "not installed: window capture falls back to the whole screen".to_string(),
                if nixos {
                    "the home-manager module installs the universe shell extension; log out and back in to load it".to_string()
                } else {
                    format!("copy the extension/ folder of Universe to ~/.local/share/gnome-shell/extensions/{uuid}, then log out and back in")
                },
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
            match family {
                Family::NixOs => "enable services.inputplumber, or set runners.<id>.inputplumber = false to stop asking".into(),
                Family::Arch => "install inputplumber and run systemctl enable --now inputplumber, or set runners.<id>.inputplumber = false to stop asking".into(),
                _ => "install InputPlumber (packaged on Arch, SteamOS and Bazzite) and enable its service, or set runners.<id>.inputplumber = false to stop asking".into(),
            },
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
            let fix = if b == "gsr-cli" || b == "gpu-screen-recorder" {
                gsr_fix(family).to_string()
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
            push("gsr-kms-server", "gsr-kms-server", ok, detail, gsr_fix(family).into(), "capture");
        }
    }
    for m in sources.iter().filter(|m| m.enabled) {
        for b in &m.manifest.requires.bins {
            let (ok, detail) = match (which(b), crate::tools::find(b)) {
                (Some(path), _) => (true, path),
                (None, Some(t)) => {
                    (true, format!("not installed: Universe fetches {} {} into {} on first use", t.package, t.version, crate::tools::dir().display()))
                }
                (None, None) => (false, format!("{b} is not on PATH")),
            };
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
            "Key macros (uinput)",
            uinput,
            if uinput { "/dev/uinput writable".into() } else { "/dev/uinput is not writable: key macros do nothing".into() },
            match family {
                Family::NixOs => "set hardware.uinput.enable and add yourself to the uinput group".into(),
                Family::Arch => "install game-devices-udev (AUR), whose rules give your session /dev/uinput, then reboot".into(),
                Family::Debian => "install steam-devices, whose rules give your session /dev/uinput, then reboot".into(),
                Family::Fedora => "install steam-devices (RPM Fusion), whose rules give your session /dev/uinput, then reboot".into(),
                Family::Other => {
                    "add a udev rule giving your session /dev/uinput (KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\"), then reboot".into()
                }
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
