use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::desktop::Profile;
use crate::distro::Family;
use crate::modules::{Hooker, Module};
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
    pub component: String,
}

/// A line of a module's or a source's `check` hook.
#[derive(Deserialize)]
struct Reported {
    check: String,
    #[serde(default)]
    label: String,
    ok: bool,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    fix: String,
    #[serde(default)]
    component: String,
}

/// What a module's or a source's `check` hook finds, under its global settings; a hook that prints none and fails is one failed check.
async fn hook_checks(hooker: Hooker, config: &Config) -> Vec<Check> {
    let Some(exe) = hooker.hook("check") else { return Vec::new() };
    let mut env = crate::modules::HookEnv::default();
    for (k, v) in crate::core::passthrough_env() {
        env.set(&k, v);
    }
    env.set("UNIVERSE_DISTRO", crate::distro::detect().as_str());
    let (settings, name) = match &hooker {
        Hooker::Module(m) => (("MODULE_SETTINGS_JSON", m.merged_settings(config, None)), if m.manifest.name.is_empty() { m.id() } else { &m.manifest.name }),
        Hooker::Source(s) => (("SOURCE_SETTINGS_JSON", s.merged_settings(config, None)), s.name()),
    };
    env.set(settings.0, serde_json::Value::Object(settings.1).to_string());
    let check = |r: Reported| Check {
        label: if r.label.is_empty() { r.check.clone() } else { r.label },
        check: r.check,
        ok: r.ok,
        detail: r.detail,
        fix: if r.ok { String::new() } else { r.fix },
        module: hooker.id().into(),
        component: r.component,
    };
    let failed = |detail: String| {
        let fix = format!("run {} by hand to see why", exe.display());
        vec![check(Reported { check: "check".into(), label: format!("{name} check"), ok: false, detail, fix, component: String::new() })]
    };
    let out = match crate::modules::run_blocking(&hooker, "check", &env).await {
        Ok(out) => out,
        Err(e) => return failed(e.to_string()),
    };
    let found: Vec<Check> = out.stdout.lines().filter_map(|line| serde_json::from_str::<Reported>(line).ok()).map(check).collect();
    if found.is_empty() && out.status != 0 {
        return failed(format!("exited {}: {}", out.status, out.stderr.trim().lines().last().unwrap_or("")));
    }
    found
}

/// A check's component: the one its hook names, a fetched tool or a system tool by its program, the default Proton's family, a runner.
fn attach_components(out: &mut [Check], config: &Config, packagekit: bool) {
    let catalogue = crate::components::cached(config);
    let installable = |id: &str| catalogue.components.get(id).and_then(|e| e.latest()).is_some();
    let proton = catalogue
        .components
        .iter()
        .find(|(_, e)| e.kind == crate::components::Kind::Proton && crate::config::of_family(&config.launch.proton, &e.family))
        .map(|(id, _)| id.clone());
    let family = crate::distro::detect();
    // A system tool, by id or program, that PackageKit can install here.
    let system = |name: &str| {
        crate::components::SYSTEM
            .iter()
            .find(|t| t.id == name || t.bin == name)
            .filter(|t| packagekit && !t.packages(family).is_empty())
            .map(|t| t.id.to_string())
            .unwrap_or_default()
    };
    for c in out.iter_mut() {
        let named = std::mem::take(&mut c.component);
        let missing = !c.ok || c.detail.starts_with("not installed");
        let id = match c.check.as_str() {
            _ if !named.is_empty() && c.ok => String::new(),
            _ if !named.is_empty() && crate::components::system_tool(&named).is_some() => system(&named),
            _ if !named.is_empty() => named,
            "proton" if missing => proton.clone().unwrap_or_default(),
            "mangohud-32bit" if !c.ok => system("mangohud"),
            check if missing && catalogue.tool(check, &c.module).is_some() => catalogue.tool(check, &c.module).map(|(id, _)| id.clone()).unwrap_or_default(),
            check if !c.ok && crate::components::SYSTEM.iter().any(|t| t.bin == check) => system(check),
            check => check.strip_prefix("runner-").filter(|_| !c.ok).unwrap_or_default().to_string(),
        };
        if !id.is_empty() && (installable(&id) || crate::components::system_tool(&id).is_some()) {
            if !c.ok {
                let notice = catalogue.components.get(&id).map(|e| e.notice.as_str()).filter(|n| !n.is_empty()).map(|n| format!(". {n}")).unwrap_or_default();
                c.fix = format!("universe component install {id} (Settings › Runners), or {}{notice}", c.fix);
            }
            c.component = id;
        }
    }
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
        out.push(Check {
            check: check.into(),
            label: label.into(),
            ok,
            detail,
            fix: if ok { String::new() } else { fix },
            module: module.into(),
            component: String::new(),
        })
    };
    let bin = |bin: &str| (which(bin).is_some(), which(bin).unwrap_or_else(|| format!("{bin} is not on PATH")));
    let family = crate::distro::detect();
    let nixos = family == Family::NixOs;

    let config_file = crate::paths::config_file();
    let config_state = if !config_file.exists() {
        " (absent: defaults)"
    } else if Config::writable(&config_file) {
        ""
    } else if Config::owner(&config_file) == crate::config::HOME_MANAGER {
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
    match (which("umu-run"), crate::tools::find("umu-run", "")) {
        (Some(path), _) => push("umu-run", "Proton launcher (umu-run)", true, path, String::new(), "core"),
        (None, Some((package, version))) => {
            let python = which("python3").is_some();
            push(
                "umu-run",
                "Proton launcher (umu-run)",
                python,
                if python {
                    format!("not installed: Universe fetches {package} {version} into {} at the first Proton launch", crate::components::root().display())
                } else {
                    format!("not installed, and the {package} {version} Universe would fetch is a Python zipapp: no python3 on PATH")
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
    let desktop = crate::desktop::detect(config);
    let gnome = desktop == Profile::Gnome;
    let session = crate::nest::session();
    let (ok, detail) = desktop_state(desktop, &config.desktop.profile, session);
    push(
        "desktop",
        "Desktop",
        ok,
        detail,
        format!("set desktop.profile to one of {}, or to none to stop asking", crate::desktop::PROFILES[1..crate::desktop::PROFILES.len() - 1].join(", ")),
        "core",
    );
    for (tool, package, purpose) in crate::desktop::tools(desktop) {
        let found = which(tool);
        push(
            &format!("desktop-{tool}"),
            tool,
            found.is_some(),
            found.map(|p| format!("{p}: {purpose}")).unwrap_or_else(|| format!("not on PATH: no {purpose}")),
            if nixos { format!("add pkgs.{package} to your packages") } else { format!("install {package}") },
            "core",
        );
    }
    if crate::desktop::osd_by_notification(desktop) {
        let owned = match &bus {
            Some(conn) => crate::desktop::name_owned(conn, "org.freedesktop.Notifications").await,
            None => false,
        };
        push(
            "desktop-notifications",
            "OSD notifications",
            owned,
            if owned { "the volume and recording OSD show as notifications".into() } else { "no notification daemon: no volume or recording OSD".into() },
            "run a notification daemon: mako, dunst, swaync or fnott draw the level as a bar".into(),
            "core",
        );
    }
    if config.desktop.hide_cursor && !session {
        if let Err(why) = crate::desktop::cursor_route(desktop) {
            push("cursor", "Cursor hiding", false, why.to_string(), "set desktop.hide_cursor = false to stop asking".into(), "core");
        }
    }
    if gnome && !config.desktop.cursor_extension.is_empty() {
        let ext_ok = crate::desktop::gnome::extension_installed(&config.desktop.cursor_extension);
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
        let services = crate::desktop::awake_services().await;
        push(
            "keep-awake",
            "Desktop stays awake in game",
            !services.is_empty() || desktop == Profile::Sway,
            if services.is_empty() && desktop == Profile::Sway {
                "logind's idle lock held while a game runs: swayidle waits on it".into()
            } else if services.is_empty() {
                "neither org.freedesktop.ScreenSaver nor org.freedesktop.PowerManagement on the session bus: only logind's idle lock is held, and the screen may blank mid-game".into()
            } else {
                format!("{} and logind's idle lock held while a game runs", services.join(", "))
            },
            "run an idle daemon that offers org.freedesktop.ScreenSaver (hypridle does), or set desktop.keep_awake = false to stop asking".into(),
            "core",
        );
    }
    if gnome {
        let uuid = crate::desktop::UNIVERSE_EXTENSION;
        let (ok, detail, fix) = if !crate::desktop::gnome::extension_installed(uuid) {
            (
                false,
                "not installed: window capture falls back to the whole screen, and the cursor stays shown in game".to_string(),
                if nixos {
                    "the home-manager module installs the universe shell extension; log out and back in to load it".to_string()
                } else {
                    "run universe setup, then log out and back in".to_string()
                },
            )
        } else {
            let mut state = None;
            if let Some(conn) = shell {
                if let Some(p) = crate::desktop::gnome::extensions_proxy(conn).await {
                    state = crate::desktop::gnome::extension_state(&p, uuid).await;
                }
            }
            match state {
                Some(s) if crate::desktop::gnome::extension_is_active(s) => (true, format!("{uuid} active"), String::new()),
                Some(Some(_)) => (false, "installed but not enabled".to_string(), format!("gnome-extensions enable {uuid}")),
                Some(None) => (false, "installed but not loaded yet".to_string(), "log out and back in to load it".to_string()),
                None => (false, "installed; GNOME Shell did not answer".to_string(), "check that GNOME Shell is running, then refresh".to_string()),
            }
        };
        push("universe-extension", "Universe shell extension", ok, detail, fix, "core");
    }
    let recordings = config.recordings_root();
    push(
        "recordings_root",
        "Recordings folder",
        recordings.is_dir(),
        if recordings.is_dir() { recordings.to_string_lossy().into() } else { format!("{} does not exist", recordings.display()) },
        "create it, or set paths.recordings_root to a folder that exists".into(),
        "core",
    );
    if nixos
        && crate::components::ids()
            .iter()
            .any(|id| crate::components::installed(id).iter().any(|b| matches!(b.kind, crate::components::Kind::Emulator | crate::components::Kind::Wine)))
    {
        let fhs = crate::components::fhs();
        push(
            "components-fhs",
            "Downloaded runners on NixOS (universe-fhs)",
            fhs.is_some(),
            fhs.map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "universe-fhs is not on PATH: the builds Universe downloaded cannot find their libraries".into()),
            "install Universe from its flake, which puts universe-fhs next to it".into(),
            "runners",
        );
    }
    let used: std::collections::BTreeSet<&String> = game_runners.iter().chain(config.runners.keys()).collect();
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
        if spec.id == "eden" && ok {
            let (quits, detail, fix) = eden_quits_on_stop();
            push("runner-eden-stop", "Eden quits on stop", quits, detail, fix, "runners");
        }
    }
    let present = config.sgdb_key().is_some();
    push(
        "key-sgdb",
        "SteamGridDB API key",
        true,
        if present {
            "present: SteamGridDB's art joins the stores', GamesDB's and libretro's".into()
        } else {
            "optional: art comes from the stores, GOG GamesDB and libretro without it".into()
        },
        "a key from steamgriddb.com/profile/preferences/api in keys.sgdb adds its community art".into(),
        "media",
    );
    let apis = modules
        .iter()
        .map(|m| (m.id(), &m.manifest.name, m.manifest.api, "module"))
        .chain(sources.iter().map(|s| (s.id(), &s.manifest.name, s.manifest.api, "source")));
    for (id, name, api, kind) in apis {
        if let Some(why) = crate::extensions::unsupported(api) {
            push("extension-api", "Extension API", false, why, format!("update the {name} {kind} to one this Universe reads, or remove it"), id);
        }
    }
    let mut reported = Vec::new();
    for m in modules {
        if !m.enabled || crate::extensions::unsupported(m.manifest.api).is_some() {
            continue;
        }
        if !m.incompatible.is_empty() {
            let fix = format!("update Universe or the {} module, or turn it off", m.manifest.name);
            push("requires-core", "Universe version", false, m.incompatible.clone(), fix, m.id());
            continue;
        }
        if !m.needs_setup() {
            reported.extend(hook_checks(Hooker::Module(m.clone()), config).await);
        }
        for b in m.manifest.requires.bins.iter().chain(m.missing.iter()).collect::<std::collections::BTreeSet<_>>() {
            let (ok, detail) = bin(b);
            push(b, b, ok, detail, format!("install {b}, or turn the {} module off", m.manifest.name), m.id());
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
    }
    for m in sources.iter().filter(|m| m.enabled && crate::extensions::unsupported(m.manifest.api).is_none()) {
        if !m.incompatible.is_empty() {
            let fix = format!("update Universe or the {} source, or turn it off", m.manifest.name);
            push("requires-core", "Universe version", false, m.incompatible.clone(), fix, m.id());
            continue;
        }
        for b in &m.manifest.requires.bins {
            let (ok, detail) = match (which(b), crate::tools::find(b, m.id())) {
                (Some(path), _) => (true, path),
                (None, Some((package, version))) => {
                    (true, format!("not installed: Universe fetches {package} {version} into {} on first use", crate::components::root().display()))
                }
                (None, None) => (false, format!("{b} is not on PATH")),
            };
            push(b, b, ok, detail, format!("install {b}, or turn the {} source off", m.manifest.name), m.id());
        }
        reported.extend(hook_checks(Hooker::Source(m.clone()), config).await);
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
    // A hook's line replaces the core's of the same id for its module, a required binary's: the module words its own fix.
    out.retain(|c| !reported.iter().any(|r| r.module == c.module && r.check == c.check));
    out.extend(reported);
    attach_components(&mut out, config, crate::system_install::available().await);
    out
}

/// In the Universe session gamescope has the screen to itself: no desktop to find, and it hides the cursor itself.
fn desktop_state(desktop: Profile, configured: &str, session: bool) -> (bool, String) {
    match (desktop, configured) {
        (Profile::None, _) if session => (true, "none: the Universe session, gamescope alone on the screen".into()),
        (Profile::None, "none") => (true, "none (desktop.profile): no window focus, OSD or screenshots from the desktop".into()),
        (Profile::None, _) => (false, "none detected: the launcher cannot focus a game's window, draw the OSD or take screenshots on this desktop".into()),
        (d, "auto") => (true, format!("{} (detected)", d.name())),
        (d, _) => (true, format!("{} (desktop.profile)", d.name())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn module_with_check(dir: &std::path::Path, script: &str) -> Module {
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let exe = dir.join("bin/check");
        std::fs::write(&exe, script).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let manifest = toml::from_str(
            r#"
id = "journal"
name = "Play journal"
[hooks]
check = "bin/check"
timeout_s = 5
[[settings]]
key = "provider"
type = "enum"
default = "codex"
choices = ["codex"]
"#,
        )
        .unwrap();
        Module { available: true, missing: vec![], incompatible: String::new(), unset: vec![], enabled: true, dir: dir.to_path_buf(), manifest }
    }

    #[tokio::test]
    async fn a_modules_check_hook_reports_its_lines_or_its_failure() {
        let env = crate::paths::test_env();
        let config: Config = toml::from_str("").unwrap();
        let m = module_with_check(
            env.path(),
            r#"#!/bin/sh
echo probing >&2
printf '%s' "$MODULE_SETTINGS_JSON" | grep -q '"provider":"codex"' || exit 3
echo '{"check":"codex-signin","label":"Codex sign-in","ok":false,"detail":"refused","fix":"run codex login"}'
echo 'not a check'
echo '{"check":"quota","ok":true,"detail":"28 % used","fix":"never shown"}'
exit 1
"#,
        );
        let checks = hook_checks(Hooker::Module(m), &config).await;
        let seen: Vec<_> = checks.iter().map(|c| (c.check.as_str(), c.label.as_str(), c.ok, c.detail.as_str(), c.fix.as_str(), c.module.as_str())).collect();
        assert_eq!(
            seen,
            [("codex-signin", "Codex sign-in", false, "refused", "run codex login", "journal"), ("quota", "quota", true, "28 % used", "", "journal"),],
            "each JSON line is a check of the module's, under its global settings; its exit status goes with lines printed"
        );

        let m = module_with_check(env.path(), "#!/bin/sh\necho 'codex went away' >&2\nexit 2\n");
        let checks = hook_checks(Hooker::Module(m), &config).await;
        assert_eq!(checks.len(), 1);
        assert_eq!((checks[0].check.as_str(), checks[0].label.as_str(), checks[0].ok), ("check", "Play journal check", false));
        assert!(checks[0].detail.contains("codex went away") && checks[0].fix.contains("bin/check"), "{:?}", checks[0]);
    }

    #[tokio::test]
    async fn a_sources_check_hook_reports_under_its_settings_and_names_a_component() {
        let env = crate::paths::test_env();
        std::fs::create_dir_all(env.path().join("bin")).unwrap();
        let exe = env.path().join("bin/check");
        std::fs::write(
            &exe,
            r#"#!/bin/sh
printf '%s' "$SOURCE_SETTINGS_JSON" | grep -q '"achievements":true' || exit 3
[ -n "$UNIVERSE_DISTRO" ] || exit 4
echo '{"check":"gog-comet","label":"GOG achievements (comet)","ok":false,"detail":"comet is not on PATH","fix":"install comet-gog","component":"comet"}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let manifest = toml::from_str(
            "id = \"gog\"\nname = \"GOG\"\nexe = \"bin/source\"\n[hooks]\ncheck = \"bin/check\"\n[[settings]]\nkey = \"achievements\"\ntype = \"bool\"\ndefault = true\n",
        )
        .unwrap();
        let source = Source { available: true, missing: vec![], incompatible: String::new(), enabled: true, dir: env.path().to_path_buf(), manifest };
        let checks = hook_checks(Hooker::Source(source), &toml::from_str("").unwrap()).await;
        let seen: Vec<_> = checks.iter().map(|c| (c.check.as_str(), c.ok, c.module.as_str(), c.component.as_str())).collect();
        assert_eq!(seen, [("gog-comet", false, "gog", "comet")]);
    }

    #[test]
    fn no_desktop_passes_in_the_universe_session_or_once_configured() {
        let ok = |desktop, configured, session| desktop_state(desktop, configured, session).0;
        assert!(ok(Profile::None, "auto", true));
        assert!(ok(Profile::None, "none", false));
        assert!(!ok(Profile::None, "auto", false), "a desktop the launcher cannot drive");
        assert!(ok(Profile::Gnome, "auto", false));
    }

    #[test]
    fn a_checks_component_is_the_one_its_hook_names_else_its_program() {
        let _env = crate::paths::test_env();
        let config: Config = toml::from_str("").unwrap();
        let check = |check: &str, ok: bool, component: &str| Check {
            check: check.into(),
            label: check.into(),
            ok,
            detail: if ok { "found".into() } else { "missing".into() },
            fix: "install it".into(),
            module: "m".into(),
            component: component.into(),
        };
        let mut out = vec![check("umu-run", false, ""), check("helper", false, "umu-run"), check("helper-ok", true, "umu-run"), check("comet", false, "nope")];
        attach_components(&mut out, &config, false);
        let components: Vec<_> = out.iter().map(|c| c.component.as_str()).collect();
        assert_eq!(components, ["umu-run", "umu-run", "", ""], "a fetched tool by its program, a hook's own, none once fine or unknown");
        assert!(out[1].fix.starts_with("universe component install umu-run"), "{}", out[1].fix);
    }
}
