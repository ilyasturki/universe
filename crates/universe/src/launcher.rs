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

pub struct Running {
    pub child: tokio::process::Child,
    pub unit: String,
}

/// `systemd-run --user --scope`: the command runs from this process's context, so env and cwd are set on the child (T1).
pub fn spawn(plan: &Plan) -> crate::Result<Running> {
    let mut cmd = tokio::process::Command::new("systemd-run");
    cmd.arg("--user").arg("--scope").arg("--collect").arg("--quiet").arg(format!("--unit={}", plan.unit))
        .arg("--")
        .arg(&plan.program)
        .args(&plan.args)
        .envs(plan.env.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if plan.cwd.is_dir() {
        cmd.current_dir(&plan.cwd);
    }
    let child = cmd.spawn().map_err(|e| crate::Error::Io(format!("systemd-run: {e}")))?;
    Ok(Running { child, unit: plan.unit.clone() })
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

pub fn cgroup_root() -> PathBuf {
    PathBuf::from("/sys/fs/cgroup")
}

/// Resolves the scope's cgroup dir via `systemctl --user show -p ControlGroup`; retries while the unit settles.
pub async fn cgroup_dir(unit: &str) -> Option<PathBuf> {
    for _ in 0..20 {
        let out = tokio::process::Command::new("systemctl")
            .args(["--user", "show", "-p", "ControlGroup", "--value", &format!("{unit}.scope")])
            .output()
            .await
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(cgroup_root().join(s.trim_start_matches('/')));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    None
}

pub fn populated(cgroup: &Path) -> Option<bool> {
    let s = std::fs::read_to_string(cgroup.join("cgroup.events")).ok()?;
    Some(s.lines().any(|l| l.trim() == "populated 1"))
}

/// Waits until the scope's cgroup is empty (or gone), polling cgroup.events (T1: the file disappears right after populated 0).
pub async fn wait_empty(cgroup: Option<&Path>, child: &mut tokio::process::Child) -> i32 {
    let mut exit = -1;
    let mut child_done = false;
    loop {
        if !child_done {
            match child.try_wait() {
                Ok(Some(st)) => {
                    exit = st.code().unwrap_or(-1);
                    child_done = true;
                }
                Ok(None) => {}
                Err(_) => child_done = true,
            }
        }
        let alive = match cgroup {
            Some(cg) => populated(cg).unwrap_or(false),
            None => !child_done,
        };
        if !alive && (child_done || cgroup.is_some()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    if !child_done {
        if let Ok(Some(st)) = child.try_wait() {
            exit = st.code().unwrap_or(-1);
        }
    }
    exit
}

pub async fn stop_unit(unit: &str) -> crate::Result<()> {
    let out = tokio::process::Command::new("systemctl").args(["--user", "stop", &format!("{unit}.scope")]).output().await?;
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
