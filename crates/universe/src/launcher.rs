use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::config::Config;
use crate::library::Resolved;

#[derive(Debug, Clone)]
pub struct Plan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub pre_command: String,
    pub post_command: String,
    /// Written before the launch: the user's MangoHud.conf plus the limit.
    pub mangohud_conf: Option<(PathBuf, String)>,
}

impl Plan {
    pub fn command_line(&self) -> String {
        shell_words::join(std::iter::once(&self.program).chain(&self.args))
    }
}

pub fn unit_name(id: &str, session_id: &str) -> String {
    format!("universe-game-{id}-{session_id}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FpsLimit {
    Auto,
    None,
    Hz(u32),
}

pub fn parse_fps_limit(s: &str) -> crate::Result<FpsLimit> {
    match s.trim() {
        "" | "auto" => Ok(FpsLimit::Auto),
        "none" => Ok(FpsLimit::None),
        n => match n.parse::<u32>() {
            Ok(hz) if hz > 0 => Ok(FpsLimit::Hz(hz)),
            _ => Err(crate::Error::Invalid(format!("fps_limit must be auto, none or frames per second, not '{n}'"))),
        },
    }
}

/// `auto` is the refresh the game sees: its gamescope rate when set, else the screen's.
pub fn fps_limit_hz(e: &crate::library::Effective, screen: Option<crate::gamescope::Mode>) -> Option<u32> {
    match parse_fps_limit(&e.fps_limit).unwrap_or(FpsLimit::Auto) {
        FpsLimit::Hz(hz) => Some(hz),
        FpsLimit::None => None,
        FpsLimit::Auto => {
            let gamescope_hz = e.gamescope.then(|| crate::gamescope::parse_refresh(&e.gamescope_fields.refresh).unwrap_or(None)).flatten();
            gamescope_hz.or_else(|| screen.map(|s| s.refresh).filter(|hz| *hz > 0))
        }
    }
}

pub fn mangohud_conf_text(hz: u32, hidden: bool) -> String {
    let own = std::fs::read_to_string(crate::paths::xdg("XDG_CONFIG_HOME", ".config").join("MangoHud/MangoHud.conf")).unwrap_or_default();
    let key = |l: &str| l.split('=').next().unwrap_or("").trim().to_string();
    let mut out: String = own.lines().filter(|l| !matches!(key(l).as_str(), "fps_limit" | "no_display")).map(|l| format!("{l}\n")).collect();
    if hidden {
        out.push_str("no_display\n");
    }
    out.push_str(&format!("fps_limit={hz}\n"));
    out
}

fn prefix_of(g: &crate::game::Game, config: &Config) -> PathBuf {
    if g.launch.prefix.is_empty() { config.prefixes_root().join(&g.id) } else { crate::paths::expand(&g.launch.prefix) }
}

fn dll_overrides_env(g: &crate::game::Game, env: &mut BTreeMap<String, String>) {
    if !g.launch.dll_overrides.is_empty() {
        let s: Vec<String> = g.launch.dll_overrides.iter().map(|(k, v)| format!("{k}={v}")).collect();
        env.insert("WINEDLLOVERRIDES".into(), s.join(";"));
    }
}

pub fn proton_toggles(e: &crate::library::Effective) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for (on, key) in [(!e.esync, "PROTON_NO_ESYNC"), (!e.fsync, "PROTON_NO_FSYNC"), (!e.ntsync, "PROTON_NO_NTSYNC"), (e.wayland, "PROTON_ENABLE_WAYLAND"), (e.hdr, "PROTON_ENABLE_HDR"), (e.dlss_upgrade, "PROTON_DLSS_UPGRADE"), (e.fsr4_upgrade, "PROTON_FSR4_UPGRADE"), (e.xess_upgrade, "PROTON_XESS_UPGRADE"), (e.optiscaler, "PROTON_USE_OPTISCALER")] {
        if on {
            env.insert(key.into(), "1".into());
        }
    }
    env
}

fn proton_env(g: &crate::game::Game, r: &Resolved, config: &Config, env: &mut BTreeMap<String, String>) -> crate::Result<()> {
    let prefix = prefix_of(g, config);
    std::fs::create_dir_all(&prefix)?;
    env.insert("WINEPREFIX".into(), prefix.to_string_lossy().into());
    let proton = r.effective.proton_path.clone();
    if proton.is_empty() {
        return Err(crate::Error::Unavailable(format!("{}: Proton '{}' not found", g.id, r.effective.proton)));
    }
    env.insert("PROTONPATH".into(), proton);
    env.insert("GAMEID".into(), if g.launch.umu_id.is_empty() { "umu-default".into() } else { g.launch.umu_id.clone() });
    if !g.launch.store.is_empty() {
        env.insert("STORE".into(), g.launch.store.clone());
    }
    // A switch off removes what [launch.env] seeded, so the field decides.
    for key in ["PROTON_ENABLE_WAYLAND", "PROTON_ENABLE_HDR"] {
        env.remove(key);
    }
    env.extend(proton_toggles(&r.effective));
    dll_overrides_env(g, env);
    Ok(())
}

fn wine_env(g: &crate::game::Game, r: &Resolved, config: &Config, env: &mut BTreeMap<String, String>) {
    env.insert("WINEPREFIX".into(), prefix_of(g, config).to_string_lossy().into());
    if !g.launch.arch.is_empty() {
        env.insert("WINEARCH".into(), g.launch.arch.clone());
    }
    env.insert("WINEESYNC".into(), if r.effective.esync { "1" } else { "0" }.into());
    env.insert("WINEFSYNC".into(), if r.effective.fsync { "1" } else { "0" }.into());
    dll_overrides_env(g, env);
}

/// `launch.wrapper` in front of the program: the innermost layer, inside gamescope and setpriv.
fn wrap(wrapper: &str, program: String, args: Vec<String>) -> (String, Vec<String>) {
    let mut words = shell_words::split(wrapper).unwrap_or_else(|_| vec![wrapper.to_string()]);
    words.retain(|w| !w.is_empty());
    match words.split_first() {
        Some((first, rest)) => (first.clone(), rest.iter().cloned().chain(std::iter::once(program)).chain(args).collect()),
        None => (program, args),
    }
}

/// `screen` `None` leaves gamescope's own size; `splash` `None` keeps the keep-alive window black.
pub fn plan(r: &Resolved, config: &Config, extra_env: &BTreeMap<String, String>, screen: Option<crate::gamescope::Mode>, splash: Option<&Path>, nested: bool) -> crate::Result<Plan> {
    use crate::runners::{self, Kind};
    let g = &r.game;
    if g.launch.exe.is_empty() {
        return Err(crate::Error::Invalid(format!("{}: no executable", g.id)));
    }
    let spec = runners::spec(&r.effective.runner).ok_or_else(|| crate::Error::Unavailable(format!("{}: runner '{}' is not one Universe ships", g.id, r.effective.runner)))?;
    let exe = g.exe_path();
    if spec.file_required && !exe.exists() {
        return Err(crate::Error::NotFound(format!("{}: {} missing", g.id, exe.display())));
    }
    let mut env = extra_env.clone();
    env.extend(config.launch.env.clone());
    let file = exe.to_string_lossy().to_string();
    let (program, mut args) = match spec.kind {
        Kind::Proton => {
            proton_env(g, r, config, &mut env)?;
            (if g.launch.runner_exe.is_empty() { config.launch.umu_run.clone() } else { r.effective.runner_path.clone() }, vec![file])
        }
        Kind::Wine => {
            wine_env(g, r, config, &mut env);
            let program = if r.effective.runner_path.is_empty() { "wine".to_string() } else { r.effective.runner_path.clone() };
            (program, vec![file])
        }
        Kind::Linux => (file, vec![]),
        Kind::Emulator => {
            let mut args = runners::global_args(spec, config);
            args.extend(spec.option_args(&r.effective.options));
            args.extend(runners::file_args(spec, &exe));
            if r.effective.runner_path.is_empty() {
                return Err(crate::Error::Unavailable(format!("{}: {} not found (install it or set runners.{}.exe)", g.id, spec.name, spec.id)));
            }
            if spec.via_proton {
                proton_env(g, r, config, &mut env)?;
                args.insert(0, r.effective.runner_path.clone());
                (config.launch.umu_run.clone(), args)
            } else {
                (r.effective.runner_path.clone(), args)
            }
        }
    };
    args.extend(g.launch.args.iter().cloned());
    env.extend(g.launch.env.clone());
    let (program, args) = wrap(&g.launch.wrapper, program, args);
    let gamescope = (!nested && r.effective.gamescope).then(|| runners::on_path(&config.launch.gamescope_bin)).flatten();
    if !nested && r.effective.gamescope && gamescope.is_none() {
        tracing::warn!("{}: {} not found, launching on the desktop", g.id, config.launch.gamescope_bin);
    }
    let inside = nested || gamescope.is_some();
    // Native and emulator programs run through MangoHud's wrapper (OpenGL too), Proton and Wine on its Vulkan layer alone.
    let limit = fps_limit_hz(&r.effective, screen);
    let mangohud = limit.and_then(|_| runners::on_path("mangohud"));
    if limit.is_some() && mangohud.is_none() {
        tracing::warn!("{}: mangohud not found, no frame rate limit", g.id);
    }
    let mut mangohud_conf = None;
    let (program, args) = match (limit, &mangohud) {
        (Some(hz), Some(bin)) => {
            let path = crate::paths::state_home().join("MangoHud.conf");
            mangohud_conf = Some((path.clone(), mangohud_conf_text(hz, inside || !r.effective.mangohud)));
            // On the program, not on the unit: gamescope is a Vulkan client too and would read the variables.
            let env_bin = runners::on_path("env").map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "env".into());
            let mut line = vec!["MANGOHUD=1".to_string(), format!("MANGOHUD_CONFIGFILE={}", path.display())];
            if matches!(spec.kind, Kind::Linux | Kind::Emulator) && !spec.via_proton {
                line.push(bin.to_string_lossy().to_string());
            }
            line.push(program);
            line.extend(args);
            (env_bin, line)
        }
        _ => (program, args),
    };
    let (program, args) = match &gamescope {
        Some(bin) => {
            let mut wrap = gamescope_args(&r.effective.gamescope_fields, [&config.launch.gamescope_args, &r.effective.gamescope_args], r.effective.mangohud, r.effective.hdr, screen);
            // gamescope hosts X11 clients through its own Xwayland; a Wayland Proton finds no xdg-shell there.
            if !wrap.iter().any(|a| a == "--expose-wayland") {
                env.remove("PROTON_ENABLE_WAYLAND");
            }
            wrap.push("--".into());
            // gamescope's primary child is the keep-alive window; the game is its child, still under setpriv.
            wrap.extend([crate::paths::self_exe().to_string_lossy().to_string(), "splash".into()]);
            if let Some(p) = splash {
                wrap.extend(["--image".into(), p.to_string_lossy().to_string()]);
            }
            wrap.push("--".into());
            // A capSysNice wrapper on gamescope hands CAP_SYS_NICE down, and bwrap refuses to start holding one.
            if let Some(setpriv) = runners::on_path("setpriv") {
                wrap.extend([setpriv.to_string_lossy().to_string(), "--ambient-caps=-all".into(), "--inh-caps=-all".into(), "--".into()]);
            }
            wrap.push(program);
            wrap.extend(args);
            (bin.to_string_lossy().to_string(), wrap)
        }
        // On the launcher's gamescope: mangoapp is its HUD, Proton goes X11 through its Xwayland.
        None if nested => {
            env.remove("PROTON_ENABLE_WAYLAND");
            (program, args)
        }
        None => {
            if r.effective.mangohud {
                env.insert("MANGOHUD".into(), "1".into());
            }
            (program, args)
        }
    };
    Ok(Plan { program, args, cwd: g.working_dir(), env, pre_command: g.launch.pre_command.clone(), post_command: g.launch.post_command.clone(), mangohud_conf })
}

/// `--force-composition`: a buffer scanned out straight records as one flat colour in Mutter's window screencast.
fn gamescope_args(fields: &crate::gamescope::Fields, extras: [&str; 2], mangohud: bool, hdr: bool, screen: Option<crate::gamescope::Mode>) -> Vec<String> {
    let mut args: Vec<String> = vec!["-f".into(), "--force-composition".into()];
    args.extend(crate::gamescope::args(fields, screen));
    for extra in extras {
        args.extend(shell_words::split(extra).unwrap_or_else(|_| vec![extra.to_string()]).into_iter().filter(|a| !a.is_empty()));
    }
    if mangohud {
        args.push("--mangoapp".into());
    }
    if hdr && !args.iter().any(|a| a == "--hdr-enabled") {
        args.push("--hdr-enabled".into());
    }
    args
}

pub fn host_gamescope(config: &Config, screen: Option<crate::gamescope::Mode>) -> Option<(String, Vec<String>)> {
    let bin = crate::runners::on_path(&config.launch.gamescope_bin)?;
    let fields = crate::library::gamescope_fields_of(&crate::game::Game::default(), config);
    Some((bin.to_string_lossy().to_string(), gamescope_args(&fields, [&config.launch.gamescope_args, ""], config.launch.mangohud, config.launch.hdr, screen)))
}

pub async fn run_shell(command: &str, env: &BTreeMap<String, String>, cwd: &Path) -> crate::Result<i32> {
    if command.trim().is_empty() {
        return Ok(0);
    }
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command).envs(env.iter()).stdin(Stdio::null());
    if cwd.is_dir() {
        cmd.current_dir(cwd);
    }
    let st = tokio::time::timeout(Duration::from_secs(120), cmd.status()).await.map_err(|_| crate::Error::Io(format!("command timed out: {command}")))??;
    Ok(st.code().unwrap_or(-1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::library::Effective;

    fn game(dir: &Path, file: &str, runner: &str) -> Game {
        let exe = dir.join(file);
        std::fs::write(&exe, b"").unwrap();
        let mut g = Game::new("Sample");
        g.launch.runner = runner.into();
        g.launch.exe = exe.to_string_lossy().into();
        g.launch.prefix = dir.join("pfx").to_string_lossy().into();
        g
    }

    #[test]
    fn plan_sets_umu_env() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "");
        let exe = g.launch.exe.clone();
        g.launch.fsync = Some(false);
        g.launch.env.insert("WINE_CPU_TOPOLOGY".into(), "4:0,1,2,3".into());
        g.launch.dll_overrides.insert("d3d11".into(), "n,b".into());
        let mut cfg = Config::default();
        cfg.launch.gamescope = false;
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/nix/store/proton".into();
        let p = plan(&r, &cfg, &BTreeMap::from([("FROM_HOOK".to_string(), "1".to_string())]), None, None, false).unwrap();
        assert_eq!(p.program, "umu-run");
        assert_eq!(p.args[0], exe);
        assert_eq!(p.env["GAMEID"], "umu-default");
        assert_eq!(p.env["PROTONPATH"], "/nix/store/proton");
        assert_eq!(p.env["PROTON_NO_FSYNC"], "1");
        assert!(!p.env.contains_key("PROTON_NO_ESYNC"));
        assert_eq!(p.env["MANGOHUD"], "1");
        assert_eq!(p.env["WINEDLLOVERRIDES"], "d3d11=n,b");
        assert_eq!(p.env["WINE_CPU_TOPOLOGY"], "4:0,1,2,3");
        assert_eq!(p.env["FROM_HOOK"], "1");
        assert_eq!(p.env["PROTON_ENABLE_WAYLAND"], "1");
        assert!(!p.env.contains_key("PROTON_NO_NTSYNC") && !p.env.contains_key("PROTON_ENABLE_HDR"));
        assert_eq!(p.cwd, dir.path());
        assert!(dir.path().join("pfx").is_dir());
    }

    #[test]
    fn plan_switches_proton_features_and_wraps_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "");
        let exe = g.launch.exe.clone();
        g.launch.wayland = Some(false);
        g.launch.ntsync = Some(false);
        g.launch.hdr = Some(true);
        g.launch.dlss_upgrade = Some(true);
        g.launch.wrapper = "gamemoderun taskset -c '0-7'".into();
        let mut cfg = Config::default();
        cfg.launch.gamescope = false;
        cfg.launch.env.insert("PROTON_ENABLE_WAYLAND".into(), "1".into());
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert_eq!(p.program, "gamemoderun");
        assert_eq!(p.args, vec!["taskset", "-c", "0-7", "umu-run", &exe]);
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "the field off beats the seed");
        assert_eq!(p.env["PROTON_NO_NTSYNC"], "1");
        assert_eq!(p.env["PROTON_ENABLE_HDR"], "1");
        assert_eq!(p.env["PROTON_DLSS_UPGRADE"], "1");
        assert!(!p.env.contains_key("PROTON_FSR4_UPGRADE"));
    }

    #[test]
    fn plan_for_plain_wine_sets_wine_env() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "wine");
        g.launch.esync = Some(false);
        g.launch.dll_overrides.insert("amd_ags_x64".into(), "n,b".into());
        let mut cfg = Config::default();
        cfg.launch.gamescope = false;
        let r = crate::library::resolve(g, &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert!(p.program.ends_with("wine"));
        assert_eq!(p.env["WINEARCH"], "win64");
        assert_eq!(p.env["WINEESYNC"], "0");
        assert_eq!(p.env["WINEFSYNC"], "1");
        assert_eq!(p.env["WINEDLLOVERRIDES"], "amd_ags_x64=n,b");
        assert!(!p.env.contains_key("PROTONPATH") && !p.env.contains_key("PROTON_ENABLE_WAYLAND"));
    }

    #[test]
    fn plan_for_an_emulator() {
        let dir = tempfile::tempdir().unwrap();
        let emu = dir.path().join("dolphin-emu");
        std::fs::write(&emu, b"#!/bin/sh\n").unwrap();
        let mut g = game(dir.path(), "F-Zero GX.iso", "dolphin");
        let rom = g.launch.exe.clone();
        g.launch.args = vec!["--extra".into()];
        let mut cfg = Config::default();
        let mut t = toml::Table::new();
        t.insert("exe".into(), toml::Value::String(emu.to_string_lossy().into()));
        t.insert("args".into(), toml::Value::String("--config Dolphin.Display.Fullscreen=True".into()));
        t.insert("gamescope".into(), toml::Value::Boolean(false));
        cfg.runners.insert("dolphin".into(), t);
        let r = crate::library::resolve(g, &cfg, &[]);
        assert!(!r.effective.gamescope, "a runner's own gamescope switch wins over the default");
        assert_eq!(r.effective.runner, "dolphin");
        assert_eq!(r.effective.runner_path, emu.to_string_lossy());
        assert_eq!(r.effective.platform, "Nintendo GameCube");
        assert!(r.effective.inputplumber);
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert_eq!(p.program, emu.to_string_lossy());
        assert_eq!(p.args, vec!["--config", "Dolphin.Display.Fullscreen=True", "--batch", "-e", &rom, "--extra"]);
        assert_eq!(p.env["MANGOHUD"], "1");
        assert!(!p.env.contains_key("WINEPREFIX"));
        assert_eq!(p.cwd, dir.path());
    }

    #[test]
    fn plan_runs_xenia_through_umu() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "a.iso", "xenia");
        g.launch.runner_exe = "/x/xenia_canary.exe".into();
        let mut cfg = Config::default();
        cfg.launch.gamescope = false;
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert_eq!(p.program, cfg.launch.umu_run);
        assert_eq!(p.args[..2], ["/x/xenia_canary.exe", "--fullscreen"]);
        assert_eq!(p.env["PROTONPATH"], "/p");
    }

    #[test]
    fn plan_wraps_the_game_in_gamescope() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "");
        let exe = g.launch.exe.clone();
        g.launch.args = vec!["-skipintro".into()];
        g.launch.gamescope_args = "-r 120".into();
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_args = "--adaptive-sync".into();
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.fps_limit = "none".into();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        assert!(r.effective.gamescope);
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60 });
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, Some(Path::new("/run/user/1000/universe/splash-x.bgrx")), false).unwrap();
        assert_eq!(p.program, bin.to_string_lossy());
        let mut want: Vec<String> = ["-f", "--force-composition", "-W", "3840", "-H", "2160", "-w", "3840", "-h", "2160", "-r", "60", "--adaptive-sync", "-r", "120", "--mangoapp", "--"].map(String::from).into();
        want.extend([crate::paths::self_exe().to_string_lossy().to_string(), "splash".into(), "--image".into(), "/run/user/1000/universe/splash-x.bgrx".into(), "--".into()]);
        if let Some(setpriv) = crate::runners::on_path("setpriv") {
            want.extend([setpriv.to_string_lossy().to_string(), "--ambient-caps=-all".into(), "--inh-caps=-all".into(), "--".into()]);
        }
        want.extend(["umu-run".to_string(), exe, "-skipintro".into()]);
        assert_eq!(p.args, want);
        assert!(!p.env.contains_key("MANGOHUD"), "mangoapp draws the HUD inside gamescope");
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "an X11 Proton under gamescope's Xwayland");
        assert_eq!(p.env["PROTONPATH"], "/p");

        r.effective.gamescope_args = "--expose-wayland".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert_eq!(p.env["PROTON_ENABLE_WAYLAND"], "1");
        assert_eq!(p.args[2], "--adaptive-sync", "no screen known: gamescope keeps its own size");
        let splash = p.args.iter().position(|a| a == "splash").unwrap();
        assert_eq!(p.args[splash + 1], "--", "no poster: the keep-alive window stays black");

        r.game.launch.gamescope_resolution = "1920x1080".into();
        r.game.launch.gamescope_scaler = "integer".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        assert_eq!(p.args[2..14], ["-W", "3840", "-H", "2160", "-w", "1920", "-h", "1080", "-r", "60", "-S", "integer"], "the game's fields over the global ones, the output the screen");

        r.effective.hdr = true;
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert!(p.args.contains(&"--hdr-enabled".to_string()) && p.env["PROTON_ENABLE_HDR"] == "1");

        r.game.launch.gamescope = Some(false);
        let r = crate::library::resolve(r.game, &cfg, &[]);
        assert!(!r.effective.gamescope, "the game's own switch wins");
    }

    #[test]
    fn plan_nested_runs_the_game_plain_on_the_launchers_gamescope() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "");
        let exe = g.launch.exe.clone();
        g.launch.wayland = Some(true);
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.fps_limit = "none".into();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60 });
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, Some(Path::new("/run/user/1000/universe/splash-x.bgrx")), true).unwrap();
        assert_eq!(p.program, "umu-run", "no gamescope, no splash, no setpriv of its own");
        assert_eq!(p.args, vec![exe]);
        assert!(!p.env.contains_key("MANGOHUD"), "mangoapp on the launcher's gamescope draws the HUD");
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "X11 through the launcher's Xwayland");

        cfg.launch.fps_limit = "auto".into();
        if crate::runners::on_path("mangohud").is_some() {
            let mut r = crate::library::resolve(r.game.clone(), &cfg, &[]);
            r.effective.proton_path = "/p".into();
            let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, true).unwrap();
            let (_, text) = p.mangohud_conf.as_ref().expect("the limit still goes through the layer");
            assert!(text.ends_with("no_display\nfps_limit=60\n"), "{text}");
        }
    }

    #[test]
    fn host_gamescope_takes_the_global_fields() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = dir.path().join("nope").to_string_lossy().into();
        assert!(host_gamescope(&cfg, None).is_none());
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.gamescope_args = "--adaptive-sync".into();
        cfg.launch.gamescope_filter = "fsr".into();
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60 });
        let (program, args) = host_gamescope(&cfg, screen).unwrap();
        assert_eq!(program, bin.to_string_lossy());
        assert_eq!(args, ["-f", "--force-composition", "-W", "3840", "-H", "2160", "-w", "3840", "-h", "2160", "-r", "60", "-F", "fsr", "--adaptive-sync", "--mangoapp"]);
        cfg.launch.mangohud = false;
        cfg.launch.hdr = true;
        let (_, args) = host_gamescope(&cfg, None).unwrap();
        assert_eq!(args, ["-f", "--force-composition", "-F", "fsr", "--adaptive-sync", "--hdr-enabled"]);
    }

    #[test]
    fn fps_limit_parses_and_follows_the_refresh() {
        assert_eq!(parse_fps_limit("auto").unwrap(), FpsLimit::Auto);
        assert_eq!(parse_fps_limit("").unwrap(), FpsLimit::Auto);
        assert_eq!(parse_fps_limit("none").unwrap(), FpsLimit::None);
        assert_eq!(parse_fps_limit("40").unwrap(), FpsLimit::Hz(40));
        assert!(parse_fps_limit("0").is_err() && parse_fps_limit("fast").is_err());
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60 });
        let mut e = Effective { gamescope: true, fps_limit: "auto".into(), ..Default::default() };
        assert_eq!(fps_limit_hz(&e, screen), Some(60), "auto is the screen's rate");
        assert_eq!(fps_limit_hz(&e, None), None, "no screen known: nothing to follow");
        e.gamescope_fields.refresh = "30".into();
        assert_eq!(fps_limit_hz(&e, screen), Some(30), "the gamescope rate is what the game sees");
        e.gamescope = false;
        assert_eq!(fps_limit_hz(&e, screen), Some(60), "on the desktop the gamescope rate means nothing");
        e.fps_limit = "none".into();
        assert_eq!(fps_limit_hz(&e, screen), None);
        e.fps_limit = "45".into();
        assert_eq!(fps_limit_hz(&e, None), Some(45));
    }

    #[test]
    fn plan_limits_the_frame_rate_through_mangohud() {
        let Some(mangohud) = crate::runners::on_path("mangohud") else { return };
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "Game.exe", "");
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60 });
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        let at = p.args.iter().position(|a| a == "MANGOHUD=1").expect("the game's MangoHud env");
        assert!(p.args[at - 1].ends_with("/env") && p.args[at - 2] == "--", "right after setpriv");
        let (path, text) = p.mangohud_conf.as_ref().expect("a config to write");
        assert_eq!(p.args[at + 1..at + 3], [format!("MANGOHUD_CONFIGFILE={}", path.display()), "umu-run".to_string()]);
        assert!(text.ends_with("no_display\nfps_limit=60\n"), "the layer draws nothing, mangoapp does: {text}");
        assert!(!p.env.contains_key("MANGOHUD") && !p.env.contains_key("MANGOHUD_CONFIGFILE"), "nothing MangoHud on the unit: gamescope is a Vulkan app too");
        assert!(p.args.contains(&"--mangoapp".to_string()));

        r.game.launch.gamescope_refresh = "30".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        assert!(p.mangohud_conf.as_ref().unwrap().1.ends_with("fps_limit=30\n"), "auto follows the game's gamescope rate");

        r.game.launch.fps_limit = "none".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        assert!(!p.args.iter().any(|a| a.starts_with("MANGOHUD")) && p.mangohud_conf.is_none());

        r.game.launch.fps_limit.clear();
        r.game.launch.gamescope = Some(false);
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        let (path, text) = p.mangohud_conf.as_ref().expect("a config to write");
        assert!(text.ends_with("fps_limit=60\n") && !text.contains("no_display"));
        assert!(p.program.ends_with("env"));
        assert_eq!(p.args[..3], ["MANGOHUD=1".to_string(), format!("MANGOHUD_CONFIGFILE={}", path.display()), "umu-run".to_string()]);
        assert_eq!(p.env["MANGOHUD"], "1");

        let mut n = game(dir.path(), "game", "linux");
        let native = n.launch.exe.clone();
        n.launch.mangohud = Some(false);
        n.launch.gamescope = Some(false);
        let r = crate::library::resolve(n, &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, false).unwrap();
        assert!(p.program.ends_with("env"));
        let (path, text) = p.mangohud_conf.as_ref().unwrap();
        assert_eq!(p.args, ["MANGOHUD=1".to_string(), format!("MANGOHUD_CONFIGFILE={}", path.display()), mangohud.to_string_lossy().to_string(), native]);
        assert!(!p.env.contains_key("MANGOHUD") && text.contains("no_display\n"), "the HUD is off: the layer limits, draws nothing");
    }

    #[test]
    fn mangohud_conf_keeps_the_users_lines() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("MangoHud")).unwrap();
        std::fs::write(dir.path().join("MangoHud/MangoHud.conf"), "fps_limit=30\nfps_limit_method=early\nno_display\ntoggle_hud=F12\n").unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        let shown = mangohud_conf_text(60, false);
        let hidden = mangohud_conf_text(45, true);
        assert_eq!(shown, "fps_limit_method=early\ntoggle_hud=F12\nfps_limit=60\n", "the method survives, the old limit and no_display go");
        assert_eq!(hidden, "fps_limit_method=early\ntoggle_hud=F12\nno_display\nfps_limit=45\n");
    }

    #[test]
    fn plan_without_a_gamescope_binary_runs_the_game_plain() {
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "game", "linux");
        let exe = g.launch.exe.clone();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = dir.path().join("nope/gamescope").to_string_lossy().into();
        let r = crate::library::resolve(g, &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap();
        assert_eq!(p.program, exe);
        assert_eq!(p.env["MANGOHUD"], "1");
    }

    #[test]
    fn plan_refuses_a_missing_runner() {
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "a.nsp", "eden");
        let cfg = Config::default();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.runner_path.clear();
        let err = plan(&r, &cfg, &BTreeMap::new(), None, None, false).unwrap_err();
        assert!(matches!(err, crate::Error::Unavailable(_)), "{err}");
    }
}
