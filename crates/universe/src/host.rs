use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
#[cfg(test)]
use std::sync::Arc;

use crate::config::Config;
use crate::desktop::Profile;
use crate::{Error, Result};

pub struct Host {
    pub units: Units,
    pub shell: Shell,
    pub pads: Pads,
}

impl Host {
    pub fn live(cfg: &Config) -> Host {
        Host { units: Units::Systemd, shell: Shell::Live { profile: crate::desktop::detect(cfg), extension: cfg.desktop.cursor_extension.clone() }, pads: Pads::Inputplumber }
    }

    #[cfg(test)]
    pub fn memory() -> (Host, Arc<Memory>) {
        let m = Arc::new(Memory::default());
        (Host { units: Units::Memory(m.clone()), shell: Shell::Memory(m.clone()), pads: Pads::Memory(m.clone()) }, m)
    }
}

#[derive(Debug, Clone, Default)]
pub struct UnitSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub unset_env: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub properties: Vec<(String, String)>,
    pub bind_to: Option<String>,
    pub stop_post: Vec<String>,
}

pub enum Units {
    Systemd,
    #[cfg(test)]
    Memory(Arc<Memory>),
}

impl Units {
    pub async fn start(&self, spec: &UnitSpec) -> Result<()> {
        match self {
            Units::Systemd => {
                let out = tokio::process::Command::new("systemd-run").args(systemd_run_args(spec)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).output().await.map_err(|e| Error::Io(format!("systemd-run: {e}")))?;
                if !out.status.success() {
                    return Err(Error::Io(format!("systemd-run {}: {}", spec.name, String::from_utf8_lossy(&out.stderr).trim())));
                }
                Ok(())
            }
            #[cfg(test)]
            Units::Memory(m) => m.start(spec),
        }
    }

    /// `deactivating` counts: `ExecStopPost` is still running the session's end.
    pub async fn is_active(&self, unit: &str) -> bool {
        match self {
            Units::Systemd => {
                let out = tokio::process::Command::new("systemctl").args(["--user", "is-active", unit]).output().await;
                match out {
                    Ok(o) => matches!(String::from_utf8_lossy(&o.stdout).trim(), "active" | "activating" | "deactivating" | "reloading"),
                    Err(_) => false,
                }
            }
            #[cfg(test)]
            Units::Memory(m) => m.units.lock().unwrap().get(unit).map(|u| u.active).unwrap_or(false),
        }
    }

    /// The game first, then the unit: `systemctl stop` SIGTERMs the whole cgroup at once, and gamescope dies before the game, which loses its X server and can save nothing.
    // A second SIGTERM after ~3 s: Dolphin takes the first as a "quit?" prompt and only exits on the second.
    pub async fn stop(&self, unit: &str) -> Result<()> {
        match self {
            Units::Systemd => {
                // systemd 260 drops the stop job of a frozen unit ("Cannot stop frozen unit") and reports success.
                let _ = tokio::process::Command::new("systemctl").args(["--user", "thaw", unit]).output().await;
                if let Some(cg) = self.cgroup(unit).await {
                    let mut game = game_pids(&cgroup_procs(&cg));
                    for round in 0..20 {
                        if game.is_empty() {
                            break;
                        }
                        if round % 6 == 0 {
                            for pid in &game {
                                // SAFETY: kill(2) with a pid read from the unit's own cgroup.
                                unsafe { libc::kill(*pid as libc::pid_t, libc::SIGTERM) };
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        game = game_pids(&cgroup_procs(&cg));
                    }
                }
                let out = tokio::process::Command::new("systemctl").args(["--user", "stop", "--no-block", unit]).output().await?;
                let err = String::from_utf8_lossy(&out.stderr);
                if !out.status.success() {
                    if err.contains("not loaded") || err.contains("could not be found") {
                        return Ok(());
                    }
                    return Err(Error::Io(format!("systemctl stop {unit}: {}", err.trim())));
                }
                for _ in 0..20 {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if !self.is_active(unit).await {
                        return Ok(());
                    }
                }
                Ok(())
            }
            #[cfg(test)]
            Units::Memory(m) => {
                m.record(format!("unit:stop {unit}"));
                m.finish(unit, 143);
                Ok(())
            }
        }
    }

    pub async fn freeze(&self, unit: &str, on: bool) -> Result<()> {
        match self {
            Units::Systemd => {
                let verb = if on { "freeze" } else { "thaw" };
                let out = tokio::process::Command::new("systemctl").args(["--user", verb, unit]).output().await?;
                if !out.status.success() {
                    return Err(Error::Io(format!("systemctl {verb} {unit}: {}", String::from_utf8_lossy(&out.stderr).trim())));
                }
                Ok(())
            }
            #[cfg(test)]
            Units::Memory(m) => {
                m.record(format!("unit:{} {unit}", if on { "freeze" } else { "thaw" }));
                Ok(())
            }
        }
    }

    pub async fn log(&self, unit: &str) -> UnitLog {
        match self {
            Units::Systemd => {
                let out = tokio::process::Command::new("journalctl").args(["--user", "-u", unit, "-o", "json", "--no-pager", "-q"]).output().await;
                match out {
                    Ok(o) => parse_unit_log(&String::from_utf8_lossy(&o.stdout)),
                    Err(_) => UnitLog::default(),
                }
            }
            #[cfg(test)]
            Units::Memory(m) => m.units.lock().unwrap().get(unit).map(|u| u.log.clone()).unwrap_or_default(),
        }
    }

    /// The cgroup path the manager reports for a unit, `None` while it is not loaded.
    pub async fn cgroup(&self, unit: &str) -> Option<String> {
        match self {
            Units::Systemd => {
                let out = tokio::process::Command::new("systemctl").args(["--user", "show", "-p", "ControlGroup", "--value", unit]).output().await.ok()?;
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (!s.is_empty()).then_some(s)
            }
            #[cfg(test)]
            Units::Memory(m) => m.units.lock().unwrap().contains_key(unit).then(|| format!("/memory/{unit}")),
        }
    }

    pub async fn adopt_scope(&self, pid: u32) -> Result<String> {
        let name = format!("universe-launcher-{pid}.scope");
        match self {
            Units::Systemd => {
                let conn = zbus::Connection::session().await.map_err(|e| Error::Unavailable(format!("session bus: {e}")))?;
                let proxy = zbus::Proxy::new(&conn, "org.freedesktop.systemd1", "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager").await.map_err(|e| Error::Unavailable(format!("systemd: {e}")))?;
                let props: Vec<(&str, zbus::zvariant::Value)> = vec![("PIDs", vec![pid].into()), ("Description", "Universe launcher".into())];
                let aux: Vec<(String, Vec<(String, zbus::zvariant::Value)>)> = vec![];
                match proxy.call::<_, _, zbus::zvariant::OwnedObjectPath>("StartTransientUnit", &(name.as_str(), "fail", props, aux)).await {
                    Ok(_) => {}
                    // A scope left by an earlier core of this process counts only if we are in it.
                    Err(e) if e.to_string().contains("UnitExists") && in_cgroup_of(&name) => {}
                    Err(e) => return Err(Error::Unavailable(format!("StartTransientUnit({name}): {e}"))),
                }
                // The reply only queues the start job; the move into the scope's cgroup lands when it runs.
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                while !in_cgroup_of(&name) {
                    if std::time::Instant::now() > deadline {
                        return Err(Error::Unavailable(format!("{name}: this process was not moved into it")));
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Ok(name)
            }
            #[cfg(test)]
            Units::Memory(m) => {
                m.record(format!("unit:adopt_scope {name}"));
                Ok(name)
            }
        }
    }
}

fn in_cgroup_of(unit: &str) -> bool {
    std::fs::read_to_string("/proc/self/cgroup").map(|s| s.lines().any(|l| l.rsplit('/').next() == Some(unit))).unwrap_or(false)
}

/// `BindsTo=` plus `After=`: the unit goes down with `bind_to` (the launcher's scope, or a hook's game unit) from the start.
fn systemd_run_args(spec: &UnitSpec) -> Vec<String> {
    let mut args: Vec<String> = vec!["--user".into(), "--collect".into(), "--quiet".into(), format!("--unit={}", spec.name)];
    for (k, v) in &spec.properties {
        args.push(format!("--property={k}={v}"));
    }
    if !spec.stop_post.is_empty() {
        args.push(format!("--property=ExecStopPost={}", unit_quote(&spec.stop_post)));
    }
    if let Some(bound) = &spec.bind_to {
        args.push(format!("--property=BindsTo={bound}"));
        args.push(format!("--property=After={bound}"));
    }
    if let Some(cwd) = spec.cwd.as_ref().filter(|d| d.is_dir()) {
        args.push(format!("--working-directory={}", cwd.display()));
    }
    for (k, v) in &spec.env {
        args.push(format!("--setenv={k}={v}"));
    }
    for k in &spec.unset_env {
        args.push(format!("--property=UnsetEnvironment={k}"));
    }
    args.push("--".into());
    args.push(spec.program.clone());
    args.extend(spec.args.iter().cloned());
    args
}

/// Unit-file quoting for an Exec= line: double quotes, backslash escapes.
fn unit_quote(parts: &[String]) -> String {
    parts.iter().map(|p| format!("\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\""))).collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone)]
struct Proc {
    pid: u32,
    ppid: u32,
    argv: Vec<String>,
}

fn cgroup_procs(cgroup: &str) -> Vec<Proc> {
    let path = PathBuf::from("/sys/fs/cgroup").join(cgroup.trim_start_matches('/')).join("cgroup.procs");
    let pids = std::fs::read_to_string(path).unwrap_or_default();
    pids.lines()
        .filter_map(|l| l.trim().parse::<u32>().ok())
        .filter_map(|pid| {
            let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
            let ppid = stat.rsplit(')').next()?.split_whitespace().nth(1)?.parse().ok()?;
            let argv = std::fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default().split(|b| *b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
            Some(Proc { pid, ppid, argv })
        })
        .collect()
}

/// The processes under `universe splash` (gamescope's primary child, the game its child); every process when there is no splash.
fn game_pids(procs: &[Proc]) -> Vec<u32> {
    let is_splash = |p: &Proc| p.argv.first().and_then(|a| a.rsplit('/').next()) == Some("universe") && p.argv.get(1).map(String::as_str) == Some("splash");
    let splashes: Vec<u32> = procs.iter().filter(|p| is_splash(p)).map(|p| p.pid).collect();
    if splashes.is_empty() {
        return procs.iter().map(|p| p.pid).collect();
    }
    let parent = |pid: u32| procs.iter().find(|p| p.pid == pid).map(|p| p.ppid);
    let under_splash = |mut pid: u32| {
        for _ in 0..64 {
            match parent(pid) {
                Some(pp) if splashes.contains(&pp) => return true,
                Some(pp) if pp > 1 => pid = pp,
                _ => return false,
            }
        }
        false
    };
    procs.iter().filter(|p| !splashes.contains(&p.pid) && under_splash(p.pid)).map(|p| p.pid).collect()
}

#[derive(Debug, Default, Clone)]
pub struct UnitLog {
    pub started: Option<chrono::DateTime<chrono::Local>>,
    pub ended: Option<chrono::DateTime<chrono::Local>>,
    pub exit: Option<i32>,
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

pub enum Shell {
    Live { profile: Profile, extension: String },
    #[cfg(test)]
    Memory(Arc<Memory>),
}

impl Shell {
    /// Returns whether the extension was already active.
    pub async fn cursor_enable(&self) -> bool {
        match self {
            Shell::Live { profile, extension } => match session_bus().await {
                Some(conn) => crate::desktop::cursor_extension_enable(&conn, *profile, extension).await,
                None => false,
            },
            #[cfg(test)]
            Shell::Memory(m) => {
                m.record("cursor:enable".into());
                false
            }
        }
    }

    pub async fn cursor_restore(&self, was_active: bool) {
        match self {
            Shell::Live { profile, extension } => {
                if let Some(conn) = session_bus().await {
                    crate::desktop::cursor_extension_restore(&conn, *profile, extension, was_active).await;
                }
            }
            #[cfg(test)]
            Shell::Memory(m) => m.record(format!("cursor:restore({was_active})")),
        }
    }
}

pub(crate) async fn session_bus() -> Option<zbus::Connection> {
    zbus::Connection::session().await.inspect_err(|e| tracing::warn!("session bus: {e}")).ok()
}

pub enum Pads {
    Inputplumber,
    #[cfg(test)]
    Memory(Arc<Memory>),
}

impl Pads {
    /// Returns whether the pads were taken, so only then are they given back.
    pub async fn engage(&self) -> bool {
        match self {
            Pads::Inputplumber => tokio::task::spawn_blocking(crate::inputplumber::engage).await.unwrap_or(false),
            #[cfg(test)]
            Pads::Memory(m) => {
                m.record("pads:engage".into());
                true
            }
        }
    }

    pub async fn release(&self) {
        match self {
            Pads::Inputplumber => {
                let _ = tokio::task::spawn_blocking(crate::inputplumber::release).await;
            }
            #[cfg(test)]
            Pads::Memory(m) => m.record("pads:release".into()),
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct Memory {
    units: std::sync::Mutex<BTreeMap<String, UnitState>>,
    calls: std::sync::Mutex<Vec<String>>,
    start_fails: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
struct UnitState {
    spec: UnitSpec,
    active: bool,
    log: UnitLog,
}

#[cfg(test)]
impl Memory {
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    fn start(&self, spec: &UnitSpec) -> Result<()> {
        self.record(format!("unit:start {}", spec.name));
        if self.start_fails.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(Error::Io(format!("systemd-run {}: refused", spec.name)));
        }
        let log = UnitLog { started: Some(chrono::Local::now()), ..Default::default() };
        self.units.lock().unwrap().insert(spec.name.clone(), UnitState { spec: spec.clone(), active: true, log });
        Ok(())
    }

    pub fn refuse_starts(&self, refuse: bool) {
        self.start_fails.store(refuse, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn finish(&self, unit: &str, exit: i32) {
        if let Some(u) = self.units.lock().unwrap().get_mut(unit) {
            u.active = false;
            u.log.ended = Some(chrono::Local::now());
            u.log.exit = Some(exit);
        }
    }

    /// Gone as if the machine rebooted: no journal, no unit.
    pub fn drop_unit(&self, unit: &str) {
        self.units.lock().unwrap().remove(unit);
    }

    pub fn spec(&self, unit: &str) -> Option<UnitSpec> {
        self.units.lock().unwrap().get(unit).map(|u| u.spec.clone())
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, ppid: u32, argv: &[&str]) -> Proc {
        Proc { pid, ppid, argv: argv.iter().map(|a| a.to_string()).collect() }
    }

    #[test]
    fn game_pids_are_the_splash_subtree_or_everything() {
        let tree = [
            proc(10, 1, &["/run/wrappers/bin/gamescope", "-f"]),
            proc(11, 10, &["Xwayland", ":2"]),
            proc(12, 10, &["gamescopereaper"]),
            proc(13, 10, &["mangoapp"]),
            proc(20, 10, &["/nix/store/x/bin/universe", "splash", "--image", "/p", "--", "eden"]),
            proc(21, 20, &["/nix/store/y/bin/.eden-wrapped", "-f"]),
            proc(22, 21, &["wineserver"]),
        ];
        assert_eq!(game_pids(&tree), [21, 22]);
        let bare = [proc(30, 1, &["umu-run", "game.exe"]), proc(31, 30, &["wine", "game.exe"])];
        assert_eq!(game_pids(&bare), [30, 31]);
        assert!(game_pids(&[]).is_empty());
    }

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
        let spec = UnitSpec {
            name: "universe-game-x-20260911-120000.service".into(),
            program: "umu-run".into(),
            args: vec!["/g/x.exe".into(), "-w".into()],
            env: BTreeMap::from([("PATH".to_string(), "/bin".to_string()), ("WINEPREFIX".to_string(), "/p".to_string())]),
            unset_env: vec!["WAYLAND_DISPLAY".into()],
            cwd: Some("/nonexistent".into()),
            properties: vec![("ExitType".into(), "cgroup".into()), ("TimeoutStopSec".into(), "80".into())],
            bind_to: None,
            stop_post: vec!["/usr/bin/universe".into(), "session-end".into(), "x".into(), "20260911-120000".into()],
        };
        let plain = systemd_run_args(&spec);
        assert_eq!(&plain[..3], &["--user", "--collect", "--quiet"]);
        assert!(plain.contains(&"--unit=universe-game-x-20260911-120000.service".to_string()));
        assert!(plain.contains(&"--property=ExitType=cgroup".to_string()));
        assert!(plain.contains(&"--property=TimeoutStopSec=80".to_string()));
        assert!(plain.contains(&"--property=ExecStopPost=\"/usr/bin/universe\" \"session-end\" \"x\" \"20260911-120000\"".to_string()));
        assert!(!plain.iter().any(|a| a.starts_with("--property=BindsTo=") || a.starts_with("--property=After=") || a.starts_with("--working-directory=")));
        assert!(plain.contains(&"--setenv=PATH=/bin".to_string()) && plain.contains(&"--setenv=WINEPREFIX=/p".to_string()));
        assert!(plain.contains(&"--property=UnsetEnvironment=WAYLAND_DISPLAY".to_string()));
        assert_eq!(&plain[plain.len() - 4..], &["--", "umu-run", "/g/x.exe", "-w"]);
        let bound = systemd_run_args(&UnitSpec { bind_to: Some("universe-launcher-4242.scope".into()), ..spec });
        assert!(bound.contains(&"--property=BindsTo=universe-launcher-4242.scope".to_string()));
        assert!(bound.contains(&"--property=After=universe-launcher-4242.scope".to_string()));
        assert_eq!(bound.len(), plain.len() + 2);
    }
}
