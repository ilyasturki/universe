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

/// Builds the umu-run invocation from the resolved game: WINEPREFIX always set, GAMEID/STORE resolved offline.
pub fn plan(r: &Resolved, config: &Config, session_id: &str, extra_env: &BTreeMap<String, String>) -> crate::Result<Plan> {
    let g = &r.game;
    if g.launch.exe.is_empty() {
        return Err(crate::Error::Invalid(format!("{}: no executable", g.id)));
    }
    let exe = g.exe_path();
    if !exe.exists() {
        return Err(crate::Error::NotFound(format!("{}: {} missing", g.id, exe.display())));
    }
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in extra_env {
        env.insert(k.clone(), v.clone());
    }
    for (k, v) in &config.launch.env {
        env.insert(k.clone(), v.clone());
    }
    let (program, args) = match g.launch.backend.as_str() {
        "proton" => {
            let prefix = if g.launch.prefix.is_empty() { config.prefixes_root().join(&g.id) } else { crate::paths::expand(&g.launch.prefix) };
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
            let mut args = vec![exe.to_string_lossy().to_string()];
            args.extend(g.launch.args.iter().cloned());
            (config.launch.umu_run.clone(), args)
        }
        "wine" => {
            let prefix = if g.launch.prefix.is_empty() { config.prefixes_root().join(&g.id) } else { crate::paths::expand(&g.launch.prefix) };
            env.insert("WINEPREFIX".into(), prefix.to_string_lossy().into());
            let mut args = vec![exe.to_string_lossy().to_string()];
            args.extend(g.launch.args.iter().cloned());
            ("wine".into(), args)
        }
        "native" => {
            (exe.to_string_lossy().to_string(), g.launch.args.clone())
        }
        other => return Err(crate::Error::Unavailable(format!("{}: backend '{other}' has no launcher yet", g.id))),
    };
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

/// A transient service, not a scope: env and cwd are passed explicitly, `ExitType=cgroup` ends it with the last game
/// process, and systemd then runs `stop_post` (`universe session-end …`) whatever happened to the launcher.
pub async fn spawn(plan: &Plan, stop_post: &[String], passthrough: &BTreeMap<String, String>, timeout_stop_s: u64) -> crate::Result<()> {
    let mut cmd = tokio::process::Command::new("systemd-run");
    cmd.args(["--user", "--collect", "--quiet"])
        .arg(format!("--unit={}", plan.unit))
        .arg("--property=ExitType=cgroup")
        .arg(format!("--property=TimeoutStopSec={timeout_stop_s}"))
        .arg(format!("--property=ExecStopPost={}", unit_quote(stop_post)));
    if plan.cwd.is_dir() {
        cmd.arg(format!("--working-directory={}", plan.cwd.display()));
    }
    for (k, v) in passthrough.iter().chain(plan.env.iter()) {
        cmd.arg(format!("--setenv={k}={v}"));
    }
    cmd.arg("--").arg(&plan.program).args(&plan.args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
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

pub async fn stop_unit(unit: &str) -> crate::Result<()> {
    let out = tokio::process::Command::new("systemctl").args(["--user", "stop", unit]).output().await?;
    let err = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() && !err.contains("not loaded") && !err.contains("could not be found") {
        return Err(crate::Error::Io(format!("systemctl stop {unit}: {}", err.trim())));
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
            effective: Effective { proton: "proton-ge".into(), proton_path: "/nix/store/proton".into(), esync: true, fsync: false, mangohud: true, hide_cursor: true, env: BTreeMap::new() },
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
}
