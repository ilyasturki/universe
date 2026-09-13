use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::config::Config;
use crate::library::Resolved;

#[derive(Debug, Clone)]
pub struct Plan {
    pub unit: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
    pub pre_command: String,
    pub post_command: String,
}

impl Plan {
    pub fn command_line(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.iter().cloned());
        shell_words::join(parts)
    }
}

pub fn unit_name(id: &str, session_id: &str) -> String {
    format!("universe-game-{id}-{session_id}")
}

fn prefix_of(g: &crate::game::Game, config: &Config) -> PathBuf {
    if g.launch.prefix.is_empty() { config.prefixes_root().join(&g.id) } else { crate::paths::expand(&g.launch.prefix) }
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
    if !r.effective.esync {
        env.insert("PROTON_NO_ESYNC".into(), "1".into());
    }
    if !r.effective.fsync {
        env.insert("PROTON_NO_FSYNC".into(), "1".into());
    }
    if !g.launch.dll_overrides.is_empty() {
        let s: Vec<String> = g.launch.dll_overrides.iter().map(|(k, v)| format!("{k}={v}")).collect();
        env.insert("WINEDLLOVERRIDES".into(), s.join(";"));
    }
    Ok(())
}

pub fn plan(r: &Resolved, config: &Config, session_id: &str, extra_env: &BTreeMap<String, String>) -> crate::Result<Plan> {
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
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in extra_env {
        env.insert(k.clone(), v.clone());
    }
    for (k, v) in &config.launch.env {
        env.insert(k.clone(), v.clone());
    }
    let file = exe.to_string_lossy().to_string();
    let (program, mut args) = match spec.kind {
        Kind::Proton => {
            proton_env(g, r, config, &mut env)?;
            (if g.launch.runner_exe.is_empty() { config.launch.umu_run.clone() } else { r.effective.runner_path.clone() }, vec![file])
        }
        Kind::Wine => {
            env.insert("WINEPREFIX".into(), prefix_of(g, config).to_string_lossy().into());
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
    if r.effective.mangohud {
        env.insert("MANGOHUD".into(), "1".into());
    }
    for (k, v) in &g.launch.env {
        env.insert(k.clone(), v.clone());
    }
    Ok(Plan {
        unit: unit_name(&g.id, session_id),
        program,
        args,
        cwd: g.working_dir(),
        env,
        pre_command: g.launch.pre_command.clone(),
        post_command: g.launch.post_command.clone(),
    })
}

/// The `systemd-run` invocation: `ExitType=cgroup` ends the unit with the last game process; `bind_to` (the
/// launcher's scope) takes the game down with the launcher, `BindsTo=` plus `After=` so the bond holds from the start.
pub fn systemd_run_args(plan: &Plan, stop_post: &[String], passthrough: &BTreeMap<String, String>, timeout_stop_s: u64, bind_to: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec!["--user".into(), "--collect".into(), "--quiet".into(), format!("--unit={}", plan.unit), "--property=ExitType=cgroup".into(), format!("--property=TimeoutStopSec={timeout_stop_s}"), format!("--property=ExecStopPost={}", unit_quote(stop_post))];
    if let Some(scope) = bind_to {
        args.push(format!("--property=BindsTo={scope}"));
        args.push(format!("--property=After={scope}"));
    }
    if plan.cwd.is_dir() {
        args.push(format!("--working-directory={}", plan.cwd.display()));
    }
    for (k, v) in passthrough.iter().chain(plan.env.iter()) {
        args.push(format!("--setenv={k}={v}"));
    }
    args.push("--".into());
    args.push(plan.program.clone());
    args.extend(plan.args.iter().cloned());
    args
}

/// A transient service, not a scope: env and cwd are passed explicitly, and systemd runs `stop_post`
/// (`universe session-end …`) when the cgroup empties, whatever happened to the launcher.
pub async fn spawn(plan: &Plan, stop_post: &[String], passthrough: &BTreeMap<String, String>, timeout_stop_s: u64, bind_to: Option<&str>) -> crate::Result<()> {
    let mut cmd = tokio::process::Command::new("systemd-run");
    cmd.args(systemd_run_args(plan, stop_post, passthrough, timeout_stop_s, bind_to)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let out = cmd.output().await.map_err(|e| crate::Error::Io(format!("systemd-run: {e}")))?;
    if !out.status.success() {
        return Err(crate::Error::Io(format!("systemd-run: {}", String::from_utf8_lossy(&out.stderr).trim())));
    }
    Ok(())
}

/// Unit-file quoting for an Exec= line: double quotes, backslash escapes.
fn unit_quote(parts: &[String]) -> String {
    parts.iter().map(|p| format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\""))).collect::<Vec<_>>().join(" ")
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

/// `deactivating` counts: `ExecStopPost` is still running the session's end.
pub async fn is_active(unit: &str) -> bool {
    let out = tokio::process::Command::new("systemctl").args(["--user", "is-active", unit]).output().await;
    match out {
        Ok(o) => matches!(String::from_utf8_lossy(&o.stdout).trim(), "active" | "activating" | "deactivating" | "reloading"),
        Err(_) => false,
    }
}

#[derive(Debug, Default, Clone)]
pub struct UnitLog {
    pub started: Option<chrono::DateTime<chrono::Local>>,
    pub ended: Option<chrono::DateTime<chrono::Local>>,
    pub exit: Option<i32>,
}

/// What the journal remembers of a unit: the start, the end and the main process's exit status.
pub async fn unit_log(unit: &str) -> UnitLog {
    let out = tokio::process::Command::new("journalctl").args(["--user", "-u", unit, "-o", "json", "--no-pager", "-q"]).output().await;
    match out {
        Ok(o) => parse_unit_log(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => UnitLog::default(),
    }
}

pub fn parse_unit_log(json_lines: &str) -> UnitLog {
    let mut log = UnitLog::default();
    for line in json_lines.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let Some(msg) = v["MESSAGE"].as_str() else { continue };
        let ts = v["__REALTIME_TIMESTAMP"].as_str().and_then(|s| s.parse::<i64>().ok()).and_then(chrono::DateTime::from_timestamp_micros).map(|t| t.with_timezone(&chrono::Local));
        if msg.starts_with("Started ") && log.started.is_none() {
            log.started = ts;
        } else if msg.contains("Deactivated successfully") || msg.contains("Failed with result") || msg.starts_with("Stopped ") || msg.contains("Consumed ") {
            log.ended = ts;
        } else if let Some(rest) = msg.split("status=").nth(1) {
            if msg.contains("Main process exited") {
                log.exit = rest.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse().ok());
            }
        }
    }
    log
}

// --no-block, then a second SIGTERM after ~3 s: Dolphin takes the first as a "quit?" prompt and
// only exits on the second.
pub async fn stop_unit(unit: &str) -> crate::Result<()> {
    let out = tokio::process::Command::new("systemctl").args(["--user", "stop", "--no-block", unit]).output().await?;
    let err = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        if err.contains("not loaded") || err.contains("could not be found") {
            return Ok(());
        }
        return Err(crate::Error::Io(format!("systemctl stop {unit}: {}", err.trim())));
    }
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        if !is_active(unit).await {
            return Ok(());
        }
    }
    let _ = tokio::process::Command::new("systemctl").args(["--user", "kill", "--signal=SIGTERM", "--kill-whom=main", unit]).output().await;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !is_active(unit).await {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::library::Effective;

    #[test]
    fn unit_log_from_journal_lines() {
        let lines = concat!(
            r#"{"MESSAGE":"Started [systemd-run] umu-run","__REALTIME_TIMESTAMP":"1789147978000000"}"#, "\n",
            r#"{"MESSAGE":"universe-game-x.service: Main process exited, code=exited, status=3/NOTIMPLEMENTED","__REALTIME_TIMESTAMP":"1789147979000000"}"#, "\n",
            r#"{"MESSAGE":"universe-game-x.service: Failed with result 'exit-code'.","__REALTIME_TIMESTAMP":"1789147980000000"}"#, "\n",
            r#"{"MESSAGE":"universe-game-x.service: Consumed 17.224s CPU time over 54.416s wall clock time, 1.6G memory peak.","__REALTIME_TIMESTAMP":"1789147981000000"}"#, "\n",
        );
        let log = parse_unit_log(lines);
        assert_eq!(log.exit, Some(3));
        assert_eq!((log.ended.unwrap() - log.started.unwrap()).num_seconds(), 3);
    }

    #[test]
    fn systemd_run_binds_to_the_launcher_scope_only_when_asked() {
        let plan = Plan { unit: "universe-game-x-20260911-120000".into(), program: "umu-run".into(), args: vec!["/g/x.exe".into(), "-w".into()], cwd: "/nonexistent".into(), env: BTreeMap::from([("WINEPREFIX".to_string(), "/p".to_string())]), pre_command: String::new(), post_command: String::new() };
        let stop_post = vec!["/usr/bin/universe".to_string(), "session-end".into(), "x".into(), "20260911-120000".into()];
        let passthrough = BTreeMap::from([("PATH".to_string(), "/bin".to_string())]);
        let plain = systemd_run_args(&plan, &stop_post, &passthrough, 80, None);
        assert_eq!(&plain[..3], &["--user", "--collect", "--quiet"]);
        assert!(plain.contains(&"--unit=universe-game-x-20260911-120000".to_string()));
        assert!(plain.contains(&"--property=ExitType=cgroup".to_string()));
        assert!(plain.contains(&"--property=TimeoutStopSec=80".to_string()));
        assert!(plain.contains(&"--property=ExecStopPost=\"/usr/bin/universe\" \"session-end\" \"x\" \"20260911-120000\"".to_string()));
        assert!(!plain.iter().any(|a| a.starts_with("--property=BindsTo=") || a.starts_with("--property=After=") || a.starts_with("--working-directory=")));
        assert!(plain.contains(&"--setenv=PATH=/bin".to_string()) && plain.contains(&"--setenv=WINEPREFIX=/p".to_string()));
        assert_eq!(&plain[plain.len() - 4..], &["--", "umu-run", "/g/x.exe", "-w"]);
        let bound = systemd_run_args(&plan, &stop_post, &passthrough, 80, Some("universe-launcher-4242.scope"));
        assert!(bound.contains(&"--property=BindsTo=universe-launcher-4242.scope".to_string()));
        assert!(bound.contains(&"--property=After=universe-launcher-4242.scope".to_string()));
        assert_eq!(bound.len(), plain.len() + 2);
    }

    #[test]
    fn plan_sets_umu_env() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Game.exe");
        std::fs::write(&exe, b"").unwrap();
        let mut g = Game::new("Sample");
        g.launch.exe = exe.to_string_lossy().into();
        g.launch.prefix = dir.path().join("pfx").to_string_lossy().into();
        g.launch.env.insert("WINE_CPU_TOPOLOGY".into(), "4:0,1,2,3".into());
        g.launch.dll_overrides.insert("d3d11".into(), "n,b".into());
        let r = Resolved {
            game: g,
            effective: Effective { runner: "proton".into(), proton: "proton-ge".into(), proton_path: "/nix/store/proton".into(), esync: true, fsync: false, mangohud: true, hide_cursor: true, ..Default::default() },
            ..Default::default()
        };
        let cfg = Config::default();
        let p = plan(&r, &cfg, "20260911-120000", &BTreeMap::from([("FROM_HOOK".to_string(), "1".to_string())])).unwrap();
        assert_eq!(p.unit, "universe-game-sample-20260911-120000");
        assert_eq!(p.program, "umu-run");
        assert_eq!(p.args[0], exe.to_string_lossy());
        assert_eq!(p.env["GAMEID"], "umu-default");
        assert_eq!(p.env["PROTONPATH"], "/nix/store/proton");
        assert_eq!(p.env["PROTON_NO_FSYNC"], "1");
        assert!(!p.env.contains_key("PROTON_NO_ESYNC"));
        assert_eq!(p.env["MANGOHUD"], "1");
        assert_eq!(p.env["WINEDLLOVERRIDES"], "d3d11=n,b");
        assert_eq!(p.env["WINE_CPU_TOPOLOGY"], "4:0,1,2,3");
        assert_eq!(p.env["FROM_HOOK"], "1");
        assert_eq!(p.env["PROTON_ENABLE_WAYLAND"], "1");
        assert_eq!(p.cwd, dir.path());
        assert!(dir.path().join("pfx").is_dir());
    }

    #[test]
    fn plan_for_an_emulator() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("F-Zero GX.iso");
        std::fs::write(&rom, b"").unwrap();
        let emu = dir.path().join("dolphin-emu");
        std::fs::write(&emu, b"#!/bin/sh\n").unwrap();
        let mut g = Game::new("F-Zero GX");
        g.launch.runner = "dolphin".into();
        g.launch.exe = rom.to_string_lossy().into();
        g.launch.args = vec!["--extra".into()];
        let mut cfg = Config::default();
        let mut t = toml::Table::new();
        t.insert("exe".into(), toml::Value::String(emu.to_string_lossy().into()));
        t.insert("args".into(), toml::Value::String("--config Dolphin.Display.Fullscreen=True".into()));
        cfg.runners.insert("dolphin".into(), t);
        let r = crate::library::resolve(g, &cfg, &[]);
        assert_eq!(r.effective.runner, "dolphin");
        assert_eq!(r.effective.runner_path, emu.to_string_lossy());
        assert_eq!(r.effective.platform, "Nintendo GameCube");
        assert!(r.effective.inputplumber);
        let p = plan(&r, &cfg, "20260913-120000", &BTreeMap::new()).unwrap();
        assert_eq!(p.program, emu.to_string_lossy());
        assert_eq!(p.args, vec!["--config", "Dolphin.Display.Fullscreen=True", "--batch", "-e", &rom.to_string_lossy().to_string(), "--extra"]);
        assert_eq!(p.env["MANGOHUD"], "1");
        assert!(!p.env.contains_key("WINEPREFIX"));
        assert_eq!(p.cwd, dir.path());
    }

    #[test]
    fn plan_runs_xenia_through_umu() {
        let dir = tempfile::tempdir().unwrap();
        let iso = dir.path().join("a.iso");
        std::fs::write(&iso, b"").unwrap();
        let mut g = Game::new("A");
        g.launch.runner = "xenia".into();
        g.launch.exe = iso.to_string_lossy().into();
        g.launch.runner_exe = "/x/xenia_canary.exe".into();
        g.launch.prefix = dir.path().join("pfx").to_string_lossy().into();
        let cfg = Config::default();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.proton_path = "/p".into();
        let p = plan(&r, &cfg, "s", &BTreeMap::new()).unwrap();
        assert_eq!(p.program, cfg.launch.umu_run);
        assert_eq!(p.args[..2], ["/x/xenia_canary.exe", "--fullscreen"]);
        assert_eq!(p.env["PROTONPATH"], "/p");
    }

    #[test]
    fn plan_refuses_a_missing_runner() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("a.nsp");
        std::fs::write(&rom, b"").unwrap();
        let mut g = Game::new("A");
        g.launch.runner = "eden".into();
        g.launch.exe = rom.to_string_lossy().into();
        let cfg = Config::default();
        let mut r = crate::library::resolve(g, &cfg, &[]);
        r.effective.runner_path.clear();
        let err = plan(&r, &cfg, "s", &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, crate::Error::Unavailable(_)), "{err}");
    }
}
