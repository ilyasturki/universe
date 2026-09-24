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
    pub mangohud_conf: Option<(PathBuf, String)>,
    pub mangoapp_conf: Option<(PathBuf, String)>,
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

/// `auto` is the refresh the game sees: its gamescope rate when set, else the screen's; none for an emulator, which paces itself.
pub fn fps_limit_hz(e: &crate::library::Effective, screen: Option<crate::gamescope::Mode>) -> Option<u32> {
    match parse_fps_limit(&e.fps_limit).unwrap_or(FpsLimit::Auto) {
        FpsLimit::Hz(hz) => Some(hz),
        FpsLimit::None => None,
        FpsLimit::Auto if e.runner_kind == "emulator" => None,
        FpsLimit::Auto => {
            let gamescope_hz = e.gamescope.then(|| crate::gamescope::parse_refresh(&e.gamescope_fields.refresh).unwrap_or(None)).flatten();
            gamescope_hz.or_else(|| screen.map(|s| s.refresh).filter(|hz| *hz > 0))
        }
    }
}

/// A pre-launch hook's line in `UNIVERSE_ENV_FILE`: flags for the game's gamescope, never the game's environment.
pub const HOOK_GAMESCOPE_ARGS: &str = "UNIVERSE_GAMESCOPE_ARGS";

/// Some distros ship mangoapp apart from MangoHud (Debian's `mangoapp` package): without it gamescope gets no `--mangoapp` and the layer in the game draws the HUD.
pub fn mangoapp_installed() -> bool {
    crate::runners::on_path("mangoapp").is_some()
}

fn users_conf_lines() -> String {
    let own = std::fs::read_to_string(crate::paths::xdg("XDG_CONFIG_HOME", ".config").join("MangoHud/MangoHud.conf")).unwrap_or_default();
    let key = |l: &str| l.split('=').next().unwrap_or("").trim().to_string();
    own.lines().filter(|l| !matches!(key(l).as_str(), "fps_limit" | "no_display" | "control")).map(|l| format!("{l}\n")).collect()
}

pub fn layer_conf_text(hud_of: Option<&str>, hz: Option<u32>, hidden: bool) -> String {
    let mut out = users_conf_lines();
    if hidden {
        out.push_str("no_display\n");
    }
    if let Some(id) = hud_of {
        out.push_str(&format!("control={}\n", crate::mangoapp::layer_control(id)));
    }
    if let Some(hz) = hz {
        out.push_str(&format!("fps_limit={hz}\n"));
    }
    out
}

pub fn mangoapp_conf_text(shown: bool) -> String {
    let mut out = users_conf_lines();
    if !shown {
        out.push_str("no_display\n");
    }
    out
}

pub fn layer_conf_path() -> PathBuf {
    crate::paths::state_home().join("MangoHud.conf")
}

pub fn mangoapp_conf_path() -> PathBuf {
    crate::paths::state_home().join("mangoapp.conf")
}

fn env_bin() -> String {
    crate::runners::on_path("env").map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "env".into())
}

pub(crate) fn prefix_of(g: &crate::game::Game, config: &Config) -> PathBuf {
    if g.launch.prefix.is_empty() {
        config.prefixes_root().join(&g.id)
    } else {
        crate::paths::expand(&g.launch.prefix)
    }
}

fn dll_overrides_env(g: &crate::game::Game, env: &mut BTreeMap<String, String>) {
    if !g.launch.dll_overrides.is_empty() {
        let s: Vec<String> = g.launch.dll_overrides.iter().map(|(k, v)| format!("{k}={v}")).collect();
        env.insert("WINEDLLOVERRIDES".into(), s.join(";"));
    }
}

pub fn proton_toggles(e: &crate::library::Effective, rdna3: bool) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for (on, key) in [
        (!e.esync, "PROTON_NO_ESYNC"),
        (!e.fsync, "PROTON_NO_FSYNC"),
        (!e.ntsync, "PROTON_NO_NTSYNC"),
        (e.wayland, "PROTON_ENABLE_WAYLAND"),
        (e.hdr, "PROTON_ENABLE_HDR"),
        (e.dlss_upgrade, "PROTON_DLSS_UPGRADE"),
        (e.fsr4_upgrade && !rdna3, "PROTON_FSR4_UPGRADE"),
        (e.fsr4_upgrade && rdna3, "PROTON_FSR4_RDNA3_UPGRADE"),
        (e.xess_upgrade, "PROTON_XESS_UPGRADE"),
        (e.optiscaler, "PROTON_USE_OPTISCALER"),
    ] {
        if on {
            env.insert(key.into(), "1".into());
        }
    }
    env
}

/// `launch.debug_log`: Proton's log (`steam-<GAMEID>.log`) and DXVK's (`<exe>_d3d11.log`…) into `dir`; Wine's and umu's own go to stderr, the journal. Nothing for a Linux program or an emulator.
pub fn debug_env(kind: crate::runners::Kind, dir: &Path) -> BTreeMap<String, String> {
    use crate::runners::Kind;
    let dir = dir.to_string_lossy().into_owned();
    let mut env = BTreeMap::new();
    match kind {
        Kind::Proton => {
            env.insert("PROTON_LOG".into(), "1".into());
            env.insert("PROTON_LOG_DIR".into(), dir.clone());
            env.insert("UMU_LOG".into(), "debug".into());
        }
        Kind::Wine => {
            env.insert("WINEDEBUG".into(), "+timestamp,+pid,+tid,+seh,+debugstr,+loaddll,+mscoree".into());
            env.insert("DXVK_LOG_LEVEL".into(), "info".into());
            env.insert("VKD3D_DEBUG".into(), "warn".into());
        }
        Kind::Linux | Kind::Emulator => return env,
    }
    env.insert("DXVK_LOG_PATH".into(), dir);
    env
}

fn proton_env(g: &crate::game::Game, r: &Resolved, config: &Config, env: &mut BTreeMap<String, String>) -> crate::Result<()> {
    let prefix = prefix_of(g, config);
    std::fs::create_dir_all(&prefix)?;
    env.insert("WINEPREFIX".into(), prefix.to_string_lossy().into());
    let proton = r.effective.proton_path.clone();
    if !proton.is_empty() {
        env.insert("PROTONPATH".into(), proton);
    } else if let Some(codename) = crate::config::umu_codename(&r.effective.proton) {
        tracing::info!("{}: Proton '{}' not installed: umu-run downloads {codename}", g.id, r.effective.proton);
        env.insert("PROTONPATH".into(), codename.into());
    } else if !env.contains_key("PROTONPATH") {
        tracing::warn!("{}: Proton '{}' not installed: umu-run downloads its own UMU-Proton", g.id, r.effective.proton);
    }
    env.insert("GAMEID".into(), if g.launch.umu_id.is_empty() { "umu-default".into() } else { g.launch.umu_id.clone() });
    if !g.launch.store.is_empty() {
        env.insert("STORE".into(), g.launch.store.clone());
    }
    // A switch off removes what [launch.env] seeded, so the field decides.
    for key in ["PROTON_ENABLE_WAYLAND", "PROTON_ENABLE_HDR"] {
        env.remove(key);
    }
    env.extend(proton_toggles(&r.effective, crate::gpu::detected().is_some_and(|g| g.needs_fsr4_rdna3())));
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

/// `screen` `None` leaves gamescope's own size; `splash` `None` keeps the keep-alive window black; `mangoapp` is whether it is installed (`mangoapp_installed`).
pub fn plan(
    r: &Resolved,
    config: &Config,
    extra_env: &BTreeMap<String, String>,
    screen: Option<crate::gamescope::Mode>,
    splash: Option<&Path>,
    nested: bool,
    mangoapp: bool,
) -> crate::Result<Plan> {
    use crate::runners::{self, Kind};
    let g = &r.game;
    if g.launch.exe.is_empty() {
        return Err(crate::Error::Invalid(format!("{}: no executable", g.id)));
    }
    let spec = runners::spec(&r.effective.runner)
        .ok_or_else(|| crate::Error::Unavailable(format!("{}: runner '{}' is not one Universe ships", g.id, r.effective.runner)))?;
    let exe = g.exe_path();
    if spec.file_required && !exe.exists() {
        return Err(crate::Error::NotFound(format!("{}: {} missing", g.id, exe.display())));
    }
    let mut env = extra_env.clone();
    let hook_gamescope_args = env.remove(HOOK_GAMESCOPE_ARGS).unwrap_or_default();
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
    let hud_in_mangoapp = (nested || gamescope.is_some()) && mangoapp;
    // Where no mangoapp draws, the layer loads hidden too: the HUD is flipped mid-game.
    // Native and emulator programs run through MangoHud's wrapper (OpenGL too), Proton and Wine on its Vulkan layer alone.
    let limit = fps_limit_hz(&r.effective, screen);
    let mangohud = runners::on_path("mangohud");
    if limit.is_some() && mangohud.is_none() {
        tracing::warn!("{}: mangohud not found, no frame rate limit", g.id);
    }
    let mut mangohud_conf = None;
    let mut layer_env = vec![];
    let (program, args) = match &mangohud {
        Some(bin) if limit.is_some() || !hud_in_mangoapp => {
            let path = layer_conf_path();
            mangohud_conf = Some((path.clone(), layer_conf_text((!hud_in_mangoapp).then_some(g.id.as_str()), limit, hud_in_mangoapp || !r.effective.mangohud)));
            layer_env = vec![("MANGOHUD".to_string(), "1".to_string()), ("MANGOHUD_CONFIGFILE".to_string(), path.to_string_lossy().to_string())];
            if limit.is_some() && matches!(spec.kind, Kind::Linux | Kind::Emulator) && !spec.via_proton {
                (bin.to_string_lossy().to_string(), std::iter::once(program).chain(args).collect())
            } else {
                (program, args)
            }
        }
        _ => (program, args),
    };
    let mut mangoapp_conf = None;
    let (program, args) = match &gamescope {
        Some(bin) => {
            let mut wrap = gamescope_args(
                false,
                &r.effective.gamescope_fields,
                [&config.launch.gamescope_args, &r.effective.gamescope_args, &hook_gamescope_args],
                r.effective.hdr,
                screen,
                mangoapp,
            );
            // Its mangoapp draws the HUD from Universe's conf; the file alone on the unit loads no layer, MANGOHUD=1 would.
            if mangoapp {
                let path = mangoapp_conf_path();
                mangoapp_conf = Some((path.clone(), mangoapp_conf_text(r.effective.mangohud)));
                env.insert("MANGOHUD_CONFIGFILE".into(), path.to_string_lossy().to_string());
            }
            env.extend(crate::keyboard::probe().env());
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
            // The layer's variables on the game alone: gamescope is a Vulkan client too and would draw it.
            if !layer_env.is_empty() {
                wrap.push(env_bin());
                wrap.extend(layer_env.iter().map(|(k, v)| format!("{k}={v}")));
            }
            wrap.push(program);
            wrap.extend(args);
            (bin.to_string_lossy().to_string(), wrap)
        }
        None => {
            env.extend(layer_env);
            // On the launcher's gamescope Proton goes X11 through its Xwayland.
            if nested {
                env.remove("PROTON_ENABLE_WAYLAND");
            }
            (program, args)
        }
    };
    Ok(Plan {
        program,
        args,
        cwd: g.working_dir(),
        env,
        pre_command: g.launch.pre_command.clone(),
        post_command: g.launch.post_command.clone(),
        mangohud_conf,
        mangoapp_conf,
    })
}

/// `--force-composition` costs a composite per frame and is needed only under Mutter's window screencast, which records a buffer scanned out straight as one flat colour: a
/// pre-launch hook asks for it through `UNIVERSE_GAMESCOPE_ARGS`; the launcher's own gamescope, up before any game is known, keeps it.
/// `--mangoapp` whenever it is installed, the HUD on or off: it is shown and hidden while the game runs, so its drawer has to be there.
fn gamescope_args(
    force_composition: bool,
    fields: &crate::gamescope::Fields,
    extras: [&str; 3],
    hdr: bool,
    screen: Option<crate::gamescope::Mode>,
    mangoapp: bool,
) -> Vec<String> {
    let mut args: Vec<String> = vec!["-f".into()];
    if force_composition {
        args.push("--force-composition".into());
    }
    args.extend(crate::gamescope::args(fields, screen));
    for extra in extras {
        args.extend(shell_words::split(extra).unwrap_or_else(|_| vec![extra.to_string()]).into_iter().filter(|a| !a.is_empty()));
    }
    if mangoapp {
        args.push("--mangoapp".into());
    }
    if hdr && !args.iter().any(|a| a == "--hdr-enabled") {
        args.push("--hdr-enabled".into());
    }
    args
}

/// The launcher's own gamescope for `screen` (the profile's default when empty), from the config alone: a host re-execs before it opens the library.
pub async fn host_gamescope_for(config: &Config, screen: &str) -> Option<(String, Vec<String>)> {
    let screen = crate::desktop::pick_screen(screen);
    let mode = crate::desktop::screen_mode(&screen).await;
    let command = host_gamescope(config, mode, mangoapp_installed())?;
    let _ = std::fs::create_dir_all(crate::paths::state_home());
    if let Err(e) = std::fs::write(mangoapp_conf_path(), mangoapp_conf_text(false)) {
        tracing::warn!("mangoapp.conf: {e}");
    }
    Some(command)
}

pub fn host_gamescope(config: &Config, screen: Option<crate::gamescope::Mode>, mangoapp: bool) -> Option<(String, Vec<String>)> {
    let bin = crate::runners::on_path(&config.launch.gamescope_bin)?;
    let fields = crate::library::gamescope_fields_of(&crate::game::Game::default(), config);
    let mut args = vec![format!("MANGOHUD_CONFIGFILE={}", mangoapp_conf_path().display())];
    args.extend(crate::keyboard::probe().env().iter().map(|(k, v)| format!("{k}={v}")));
    args.push(bin.to_string_lossy().to_string());
    args.extend(gamescope_args(true, &fields, [&config.launch.gamescope_args, "", ""], config.launch.hdr, screen, mangoapp));
    Some((env_bin(), args))
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

    #[test]
    fn debug_env_sends_proton_and_dxvk_to_the_dir_and_leaves_native_games_alone() {
        let dir = Path::new("/tmp/logs/g/s");
        let proton = debug_env(crate::runners::Kind::Proton, dir);
        assert_eq!(
            (proton["PROTON_LOG"].as_str(), proton["PROTON_LOG_DIR"].as_str(), proton["DXVK_LOG_PATH"].as_str(), proton["UMU_LOG"].as_str()),
            ("1", "/tmp/logs/g/s", "/tmp/logs/g/s", "debug")
        );
        let wine = debug_env(crate::runners::Kind::Wine, dir);
        assert!(wine["WINEDEBUG"].contains("+seh") && wine["DXVK_LOG_PATH"] == "/tmp/logs/g/s" && !wine.contains_key("PROTON_LOG"));
        assert!(debug_env(crate::runners::Kind::Linux, dir).is_empty() && debug_env(crate::runners::Kind::Emulator, dir).is_empty());
    }

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
        let p = plan(&r, &cfg, &BTreeMap::from([("FROM_HOOK".to_string(), "1".to_string())]), None, None, false, true).unwrap();
        assert_eq!(p.program, "umu-run");
        assert_eq!(p.args[0], exe);
        assert_eq!(p.env["GAMEID"], "umu-default");
        assert_eq!(p.env["PROTONPATH"], "/nix/store/proton");
        assert_eq!(p.env["PROTON_NO_FSYNC"], "1");
        assert!(!p.env.contains_key("PROTON_NO_ESYNC"));
        assert_eq!(
            p.env.get("MANGOHUD").map(String::as_str),
            crate::runners::on_path("mangohud").map(|_| "1"),
            "the layer rides on the unit when its binary is around"
        );
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
        g.launch.dlss_upgrade = Some(crate::config::Toggle::On);
        g.launch.wrapper = "gamemoderun taskset -c '0-7'".into();
        let mut cfg = Config::default();
        cfg.launch.gamescope = false;
        cfg.launch.env.insert("PROTON_ENABLE_WAYLAND".into(), "1".into());
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert_eq!(p.program, "gamemoderun");
        assert_eq!(p.args, vec!["taskset", "-c", "0-7", "umu-run", &exe]);
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "the field off beats the seed");
        assert_eq!(p.env["PROTON_NO_NTSYNC"], "1");
        assert_eq!(p.env["PROTON_ENABLE_HDR"], "1");
        assert_eq!(p.env["PROTON_DLSS_UPGRADE"], "1");
        assert!(!p.env.contains_key("PROTON_FSR4_UPGRADE"));
    }

    #[test]
    fn fsr4_on_rdna3_takes_protons_rdna3_variant() {
        let dir = tempfile::tempdir().unwrap();
        let mut e = crate::library::resolve(game(dir.path(), "Game.exe", ""), &Config::default(), &[]).effective;
        e.fsr4_upgrade = true;
        let rdna3 = proton_toggles(&e, true);
        assert_eq!(rdna3["PROTON_FSR4_RDNA3_UPGRADE"], "1");
        assert!(!rdna3.contains_key("PROTON_FSR4_UPGRADE"), "one variant or the other, never both");
        let other = proton_toggles(&e, false);
        assert_eq!(other["PROTON_FSR4_UPGRADE"], "1");
        assert!(!other.contains_key("PROTON_FSR4_RDNA3_UPGRADE"));
        e.fsr4_upgrade = false;
        assert!(!proton_toggles(&e, true).contains_key("PROTON_FSR4_RDNA3_UPGRADE"), "the variant rides on the switch");
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
        assert_eq!(r.effective.prefix, dir.path().join("pfx").to_string_lossy(), "the game's own prefix");
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert!(p.program.ends_with("wine"));
        assert_eq!(p.env["WINEARCH"], "win64");
        assert_eq!(p.env["WINEESYNC"], "0");
        assert_eq!(p.env["WINEFSYNC"], "1");
        assert_eq!(p.env["WINEDLLOVERRIDES"], "amd_ags_x64=n,b");
        assert!(!p.env.contains_key("PROTONPATH") && !p.env.contains_key("PROTON_ENABLE_WAYLAND"));
    }

    #[test]
    fn effective_prefix_is_the_one_the_launch_makes() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "proton");
        g.launch.prefix.clear();
        let cfg = Config::default();
        let r = crate::library::resolve(g, &cfg, &[]);
        assert_eq!(r.effective.prefix, cfg.prefixes_root().join(&r.game.id).to_string_lossy());
        assert_eq!(r.effective.prefix, prefix_of(&r.game, &cfg).to_string_lossy());
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
        assert_eq!(r.effective.working_dir, dir.path().to_string_lossy(), "an empty working directory is the program's folder");
        assert!(r.effective.prefix.is_empty(), "no prefix outside Proton and Wine");
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert_eq!(p.program, emu.to_string_lossy());
        assert_eq!(p.args, vec!["--config", "Dolphin.Display.Fullscreen=True", "--batch", "-e", &rom, "--extra"]);
        assert_eq!(
            p.env.get("MANGOHUD").map(String::as_str),
            crate::runners::on_path("mangohud").map(|_| "1"),
            "the layer rides on the unit when its binary is around"
        );
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
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
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
        cfg.launch.mangohud = true;
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        assert!(r.effective.gamescope);
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60, vrr: false });
        let hook_env = BTreeMap::from([(HOOK_GAMESCOPE_ARGS.to_string(), "--force-composition".to_string())]);
        let p = plan(&r, &cfg, &hook_env, screen, Some(Path::new("/run/user/1000/universe/splash-x.bgrx")), false, true).unwrap();
        assert_eq!(p.program, bin.to_string_lossy());
        assert!(!p.env.contains_key(HOOK_GAMESCOPE_ARGS), "the hook's flags go to gamescope, not the game");
        let mut want: Vec<String> = [
            "-f",
            "-W",
            "3840",
            "-H",
            "2160",
            "-w",
            "3840",
            "-h",
            "2160",
            "-r",
            "60",
            "--adaptive-sync",
            "-r",
            "120",
            "--force-composition",
            "--mangoapp",
            "--",
        ]
        .map(String::from)
        .into();
        want.extend([
            crate::paths::self_exe().to_string_lossy().to_string(),
            "splash".into(),
            "--image".into(),
            "/run/user/1000/universe/splash-x.bgrx".into(),
            "--".into(),
        ]);
        if let Some(setpriv) = crate::runners::on_path("setpriv") {
            want.extend([setpriv.to_string_lossy().to_string(), "--ambient-caps=-all".into(), "--inh-caps=-all".into(), "--".into()]);
        }
        want.extend(["umu-run".to_string(), exe, "-skipintro".into()]);
        assert_eq!(p.args, want);
        assert!(!p.env.contains_key("MANGOHUD"), "mangoapp draws the HUD inside gamescope");
        let (path, text) = p.mangoapp_conf.as_ref().expect("its mangoapp reads a conf of ours");
        assert_eq!(p.env["MANGOHUD_CONFIGFILE"], path.to_string_lossy());
        assert!(!text.contains("no_display"), "the HUD is on: {text}");
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "an X11 Proton under gamescope's Xwayland");
        assert_eq!(p.env["PROTONPATH"], "/p");
        assert!(p.env.contains_key("XKB_DEFAULT_LAYOUT") && p.env.contains_key("XKB_DEFAULT_VARIANT"), "gamescope is told the keyboard layout");

        r.effective.gamescope_args = "--expose-wayland".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert_eq!(p.env["PROTON_ENABLE_WAYLAND"], "1");
        assert_eq!(p.args[1], "--adaptive-sync", "no screen known: gamescope keeps its own size");
        let splash = p.args.iter().position(|a| a == "splash").unwrap();
        assert_eq!(p.args[splash + 1], "--", "no poster: the keep-alive window stays black");

        r.game.launch.gamescope_resolution = "1920x1080".into();
        r.game.launch.gamescope_scaler = "integer".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        assert_eq!(
            p.args[1..13],
            ["-W", "3840", "-H", "2160", "-w", "1920", "-h", "1080", "-r", "60", "-S", "integer"],
            "the game's fields over the global ones, the output the screen"
        );

        r.effective.hdr = true;
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
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
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60, vrr: false });
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, Some(Path::new("/run/user/1000/universe/splash-x.bgrx")), true, true).unwrap();
        assert_eq!(p.program, "umu-run", "no gamescope, no splash, no setpriv of its own");
        assert_eq!(p.args, vec![exe]);
        assert!(!p.env.contains_key("MANGOHUD"), "mangoapp on the launcher's gamescope draws the HUD");
        assert!(!p.env.contains_key("PROTON_ENABLE_WAYLAND"), "X11 through the launcher's Xwayland");

        cfg.launch.fps_limit = "auto".into();
        if crate::runners::on_path("mangohud").is_some() {
            let mut r = crate::library::resolve(r.game.clone(), &cfg, &[]);
            r.effective.proton_path = "/p".into();
            let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, true, true).unwrap();
            let (_, text) = p.mangohud_conf.as_ref().expect("the limit still goes through the layer");
            assert!(text.contains("no_display\nfps_limit=60\n"), "{text}");
            assert!(p.mangoapp_conf.is_none(), "the launcher's mangoapp is told, not configured");
            assert_eq!((p.program.as_str(), p.env["MANGOHUD"].as_str()), ("umu-run", "1"), "no gamescope on the unit: the layer's variables ride on it");
        }
    }

    #[test]
    fn host_gamescope_takes_the_global_fields() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = dir.path().join("nope").to_string_lossy().into();
        assert!(host_gamescope(&cfg, None, true).is_none());
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.gamescope_args = "--adaptive-sync".into();
        cfg.launch.gamescope_filter = "fsr".into();
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60, vrr: false });
        std::env::set_var("XKB_DEFAULT_LAYOUT", "fr");
        std::env::set_var("XKB_DEFAULT_VARIANT", "bepo");
        let (program, args) = host_gamescope(&cfg, screen, true).unwrap();
        assert!(program.ends_with("env"), "{program}");
        let conf = format!("MANGOHUD_CONFIGFILE={}", mangoapp_conf_path().display());
        assert_eq!(args[..4], [conf.clone(), "XKB_DEFAULT_LAYOUT=fr".into(), "XKB_DEFAULT_VARIANT=bepo".into(), bin.to_string_lossy().to_string()]);
        assert_eq!(
            args[4..],
            ["-f", "--force-composition", "-W", "3840", "-H", "2160", "-w", "3840", "-h", "2160", "-r", "60", "-F", "fsr", "--adaptive-sync", "--mangoapp"]
        );
        cfg.launch.mangohud = false;
        cfg.launch.hdr = true;
        let (_, args) = host_gamescope(&cfg, None, true).unwrap();
        assert_eq!(
            args[4..],
            ["-f", "--force-composition", "-F", "fsr", "--adaptive-sync", "--mangoapp", "--hdr-enabled"],
            "mangoapp is there whatever the HUD's state: a game shows it"
        );
        let (_, args) = host_gamescope(&cfg, None, false).unwrap();
        assert!(!args.contains(&"--mangoapp".to_string()), "no mangoapp installed: gamescope would respawn it forever");
    }

    #[test]
    fn plan_without_mangoapp_leaves_the_hud_to_the_layer() {
        let Some(_) = crate::runners::on_path("mangohud") else { return };
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "Game.exe", "");
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.fps_limit = "none".into();
        cfg.launch.mangohud = true;
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, false).unwrap();
        assert!(!p.args.contains(&"--mangoapp".to_string()) && p.mangoapp_conf.is_none());
        assert!(!p.env.contains_key("MANGOHUD_CONFIGFILE"), "the unit's conf was mangoapp's");
        let (_, text) = p.mangohud_conf.as_ref().expect("the layer is loaded for the HUD alone, no limit asked");
        assert!(!text.contains("no_display") && text.contains("control=universe-mangohud-"), "{text}");
        assert!(p.args.iter().any(|a| a == "MANGOHUD=1"), "inside the game's gamescope, on the game alone");

        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, true, false).unwrap();
        assert_eq!(p.env["MANGOHUD"], "1", "on the launcher's gamescope too");
        assert!(p.mangohud_conf.as_ref().unwrap().1.contains("control=universe-mangohud-"));
    }

    #[test]
    fn fps_limit_parses_and_follows_the_refresh() {
        assert_eq!(parse_fps_limit("auto").unwrap(), FpsLimit::Auto);
        assert_eq!(parse_fps_limit("").unwrap(), FpsLimit::Auto);
        assert_eq!(parse_fps_limit("none").unwrap(), FpsLimit::None);
        assert_eq!(parse_fps_limit("40").unwrap(), FpsLimit::Hz(40));
        assert!(parse_fps_limit("0").is_err() && parse_fps_limit("fast").is_err());
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60, vrr: false });
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
        e.runner_kind = "emulator".into();
        assert_eq!(fps_limit_hz(&e, screen), Some(45), "a rate set is a rate set, emulator or not");
        e.fps_limit = "auto".into();
        assert_eq!(fps_limit_hz(&e, screen), None, "an emulator paces itself: auto adds no limiter");
    }

    #[test]
    fn plan_limits_the_frame_rate_through_mangohud() {
        let Some(mangohud) = crate::runners::on_path("mangohud") else { return };
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "Game.exe", "");
        let exe = g.launch.exe.clone();
        let bin = dir.path().join("gamescope");
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = bin.to_string_lossy().into();
        cfg.launch.mangohud = true;
        let screen = Some(crate::gamescope::Mode { width: 3840, height: 2160, refresh: 60, vrr: false });
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        let at = p.args.iter().position(|a| a == "MANGOHUD=1").expect("the game's MangoHud env");
        assert!(p.args[at - 1].ends_with("/env") && p.args[at - 2] == "--", "right after setpriv");
        let (path, text) = p.mangohud_conf.as_ref().expect("a config to write");
        assert_eq!(p.args[at + 1..at + 3], [format!("MANGOHUD_CONFIGFILE={}", path.display()), "umu-run".to_string()]);
        assert!(text.contains("no_display\nfps_limit=60\n"), "the layer draws nothing, mangoapp does: {text}");
        assert!(
            !p.env.contains_key("MANGOHUD") && p.env.contains_key("MANGOHUD_CONFIGFILE"),
            "mangoapp's conf on the unit, never MANGOHUD=1: gamescope is a Vulkan app too"
        );
        assert!(p.args.contains(&"--mangoapp".to_string()));

        r.game.launch.gamescope_refresh = "30".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        assert!(p.mangohud_conf.as_ref().unwrap().1.contains("fps_limit=30\n"), "auto follows the game's gamescope rate");

        r.game.launch.fps_limit = "none".into();
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        assert!(!p.args.iter().any(|a| a.starts_with("MANGOHUD")) && p.mangohud_conf.is_none());

        r.game.launch.fps_limit.clear();
        r.game.launch.gamescope = Some(false);
        let mut r2 = crate::library::resolve(r.game.clone(), &cfg, &[]);
        r2.effective.proton_path = "/p".into();
        let p = plan(&r2, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        let (path, text) = p.mangohud_conf.as_ref().expect("a config to write");
        assert!(text.contains("fps_limit=60\n") && !text.contains("no_display"));
        assert!(p.mangoapp_conf.is_none(), "no gamescope, no mangoapp");
        assert_eq!((p.program.as_str(), &p.args[..1]), ("umu-run", &[exe.clone()][..]), "no gamescope on the unit: the variables ride on it");
        assert_eq!((p.env["MANGOHUD"].as_str(), p.env["MANGOHUD_CONFIGFILE"].as_str()), ("1", path.to_string_lossy().as_ref()));

        let mut n = game(dir.path(), "game", "linux");
        let native = n.launch.exe.clone();
        n.launch.mangohud = Some(false);
        n.launch.gamescope = Some(false);
        let r = crate::library::resolve(n.clone(), &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        let (path, text) = p.mangohud_conf.as_ref().unwrap();
        assert_eq!(
            (p.program.as_str(), p.args.clone()),
            (mangohud.to_string_lossy().as_ref(), vec![native.clone()]),
            "a native goes through the wrapper for OpenGL"
        );
        assert_eq!(p.env["MANGOHUD_CONFIGFILE"], path.to_string_lossy());
        assert!(text.contains("no_display\n"), "the HUD is off: the layer limits, draws nothing");

        n.launch.fps_limit = "none".into();
        let r = crate::library::resolve(n, &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), screen, None, false, true).unwrap();
        let (_, text) = p.mangohud_conf.as_ref().expect("on the desktop the layer is the HUD: loaded hidden, shown mid-game");
        assert_eq!((p.program.as_str(), p.env["MANGOHUD"].as_str()), (native.as_str(), "1"), "no limit: no wrapper");
        assert!(text.contains("no_display\n") && !text.lines().any(|l| l.starts_with("fps_limit=")), "{text}");
    }

    #[test]
    fn mangohud_confs_keep_the_users_layout_and_own_the_rest() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("MangoHud")).unwrap();
        std::fs::write(
            dir.path().join("MangoHud/MangoHud.conf"),
            "fps_limit=30\nfps_limit_method=early\nno_display\ntoggle_hud=F12\nreload_cfg=F9\ncontrol=mine\n",
        )
        .unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        assert_eq!(
            layer_conf_text(Some("g"), Some(60), false),
            "fps_limit_method=early\ntoggle_hud=F12\nreload_cfg=F9\ncontrol=universe-mangohud-g\nfps_limit=60\n",
            "the method and the keys survive; the limit, no_display and the socket are Universe's"
        );
        assert_eq!(
            layer_conf_text(Some("g"), None, true),
            "fps_limit_method=early\ntoggle_hud=F12\nreload_cfg=F9\nno_display\ncontrol=universe-mangohud-g\n",
            "the desktop's HUD, off: no limit, the layer only waits to be shown"
        );
        assert_eq!(
            layer_conf_text(None, Some(45), true),
            "fps_limit_method=early\ntoggle_hud=F12\nreload_cfg=F9\nno_display\nfps_limit=45\n",
            "under mangoapp it limits and listens to nobody"
        );
        assert_eq!(
            mangoapp_conf_text(true),
            "fps_limit_method=early\ntoggle_hud=F12\nreload_cfg=F9\n",
            "mangoapp never limits and starts as told, not as the user's no_display says"
        );
        assert_eq!(mangoapp_conf_text(false), "fps_limit_method=early\ntoggle_hud=F12\nreload_cfg=F9\nno_display\n");
    }

    #[test]
    fn plan_without_a_gamescope_binary_runs_the_game_plain() {
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "game", "linux");
        let exe = g.launch.exe.clone();
        let mut cfg = Config::default();
        cfg.launch.gamescope_bin = dir.path().join("nope/gamescope").to_string_lossy().into();
        let r = crate::library::resolve(g, &cfg, &[]);
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert_eq!(p.program, exe);
        assert_eq!(
            p.env.get("MANGOHUD").map(String::as_str),
            crate::runners::on_path("mangohud").map(|_| "1"),
            "the layer rides on the unit when its binary is around"
        );
    }

    #[test]
    fn plan_refuses_a_missing_runner() {
        let dir = tempfile::tempdir().unwrap();
        let g = game(dir.path(), "a.nsp", "eden");
        let cfg = Config::default();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.runner_path.clear();
        let err = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap_err();
        assert!(matches!(err, crate::Error::Unavailable(_)), "{err}");
    }

    #[test]
    fn plan_leaves_a_missing_proton_to_umu() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = game(dir.path(), "Game.exe", "");
        g.launch.gamescope = Some(false);
        let mut cfg = Config::default();
        cfg.launch.fps_limit = "none".into();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton = "proton-ge".into();
        r.effective.proton_path.clear();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert_eq!(p.env["PROTONPATH"], "GE-Proton", "umu fetches the latest GE-Proton");
        r.effective.proton = "proton-cachyos".into();
        let p = plan(&r, &cfg, &BTreeMap::new(), None, None, false, true).unwrap();
        assert!(!p.env.contains_key("PROTONPATH"), "no codename for it: umu's own UMU-Proton");
    }
}
