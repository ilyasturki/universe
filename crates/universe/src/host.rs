use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
#[cfg(test)]
use std::sync::Arc;
use zbus::zvariant::{OwnedObjectPath, Value};

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
        Host {
            units: Units::Systemd(Systemd::default()),
            shell: Shell::Live { profile: crate::desktop::detect(cfg), extension: cfg.desktop.cursor_extension.clone() },
            pads: Pads::Inputplumber,
        }
    }

    #[cfg(test)]
    pub fn memory() -> (Host, Arc<Memory>) {
        let m = Arc::new(Memory::default());
        (Host { units: Units::Memory(m.clone()), shell: Shell::Memory(m.clone()), pads: Pads::Memory(m.clone()) }, m)
    }
}

/// A transient unit's property, typed as systemd's bus API takes it.
#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Str(String),
    U64(u64),
}

#[derive(Debug, Clone, Default)]
pub struct UnitSpec {
    pub name: String,
    pub description: String,
    pub program: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub unset_env: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub properties: Vec<(String, Prop)>,
    pub bind_to: Option<String>,
    pub stop_post: Vec<String>,
}

pub enum Units {
    Systemd(Systemd),
    #[cfg(test)]
    Memory(Arc<Memory>),
}

const SYSTEMD: &str = "org.freedesktop.systemd1";
const MANAGER_PATH: &str = "/org/freedesktop/systemd1";
const MANAGER_IFACE: &str = "org.freedesktop.systemd1.Manager";
const NO_SUCH_UNIT: &str = "org.freedesktop.systemd1.NoSuchUnit";
const UNIT_EXISTS: &str = "org.freedesktop.systemd1.UnitExists";
/// A start job ends once the main process is forked; a stop job only after `ExecStopPost`, which the launcher's Stop must not wait through.
const START_WAIT: Duration = Duration::from_secs(30);
const STOP_WAIT: Duration = Duration::from_secs(10);

/// The user manager over the session bus, one connection subscribed to its job signals.
#[derive(Default)]
pub struct Systemd {
    conn: tokio::sync::OnceCell<zbus::Connection>,
}

type JobResult = (u32, OwnedObjectPath, String, String);

impl Systemd {
    async fn manager(&self) -> Result<zbus::Proxy<'static>> {
        let conn = self
            .conn
            .get_or_try_init(|| async {
                let conn = zbus::Connection::session().await?;
                // JobRemoved reaches subscribed clients only.
                manager_proxy(&conn).await?.call::<_, _, ()>("Subscribe", &()).await?;
                Ok::<_, zbus::Error>(conn)
            })
            .await
            .map_err(|e| Error::Unavailable(format!("user systemd: {e}")))?;
        manager_proxy(conn).await.map_err(|e| Error::Unavailable(format!("user systemd: {e}")))
    }

    async fn unit_proxy(&self, unit: &str, iface: &str) -> Result<Option<zbus::Proxy<'static>>> {
        let manager = self.manager().await?;
        let path = match manager.call::<_, _, OwnedObjectPath>("GetUnit", &(unit,)).await {
            Ok(p) => p,
            Err(e) if is_dbus_error(&e, NO_SUCH_UNIT) => return Ok(None),
            Err(e) => return Err(Error::Io(format!("GetUnit({unit}): {e}"))),
        };
        let iface = zbus::names::InterfaceName::try_from(iface.to_string()).map_err(|e| Error::Io(format!("{unit}: {e}")))?;
        let build = || {
            zbus::proxy::Builder::new(manager.connection())
                .destination(SYSTEMD)?
                .path(path.clone())?
                .interface(iface)
                .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No).build())
        };
        match build() {
            Ok(fut) => fut.await.map(Some).map_err(|e| Error::Io(format!("{unit}: {e}"))),
            Err(e) => Err(Error::Io(format!("{unit}: {e}"))),
        }
    }
}

async fn manager_proxy(conn: &zbus::Connection) -> zbus::Result<zbus::Proxy<'static>> {
    zbus::proxy::Builder::new(conn)
        .destination(SYSTEMD)?
        .path(MANAGER_PATH)?
        .interface(MANAGER_IFACE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

fn is_dbus_error(e: &zbus::Error, name: &str) -> bool {
    matches!(e, zbus::Error::MethodError(n, _, _) if n.as_str() == name)
}

/// `JobRemoved` for `job`: its result, `None` past `timeout`.
async fn wait_job(jobs: &mut zbus::proxy::SignalStream<'_>, job: &OwnedObjectPath, timeout: Duration) -> Option<String> {
    tokio::time::timeout(timeout, async {
        while let Some(msg) = jobs.next().await {
            if let Ok((_, path, _, result)) = msg.body().deserialize::<JobResult>() {
                if &path == job {
                    return Some(result);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

/// systemctl's rule: a name without a type is a service.
fn qualified(unit: &str) -> String {
    if unit.rsplit('.').next().is_some_and(|t| matches!(t, "service" | "scope")) {
        unit.to_string()
    } else {
        format!("{unit}.service")
    }
}

/// An `Exec*=` command: the program's path, its argv, failure not ignored.
fn exec_value(argv: Vec<String>) -> Value<'static> {
    let path = argv.first().cloned().unwrap_or_default();
    vec![(path, argv, false)].into()
}

/// `BindsTo=` plus `After=`: the unit goes down with `bind_to` (the launcher's scope, or a hook's game unit) from the start.
fn unit_properties(spec: &UnitSpec, program: &Path) -> Vec<(String, Value<'static>)> {
    let mut props: Vec<(String, Value<'static>)> =
        vec![("Description".into(), spec.description.clone().into()), ("CollectMode".into(), "inactive-or-failed".into())];
    let mut argv = vec![program.to_string_lossy().into_owned()];
    argv.extend(spec.args.iter().cloned());
    props.push(("ExecStart".into(), exec_value(argv)));
    if !spec.stop_post.is_empty() {
        props.push(("ExecStopPost".into(), exec_value(spec.stop_post.clone())));
    }
    if let Some(bound) = &spec.bind_to {
        props.push(("BindsTo".into(), vec![bound.clone()].into()));
        props.push(("After".into(), vec![bound.clone()].into()));
    }
    if let Some(cwd) = spec.cwd.as_ref().filter(|d| d.is_dir()) {
        props.push(("WorkingDirectory".into(), cwd.to_string_lossy().into_owned().into()));
    }
    props.push(("Environment".into(), spec.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().into()));
    if !spec.unset_env.is_empty() {
        props.push(("UnsetEnvironment".into(), spec.unset_env.clone().into()));
    }
    for (k, v) in &spec.properties {
        let v: Value<'static> = match v {
            Prop::Str(s) => s.clone().into(),
            Prop::U64(n) => (*n).into(),
        };
        props.push((k.clone(), v));
    }
    props
}

impl Units {
    pub async fn start(&self, spec: &UnitSpec) -> Result<()> {
        match self {
            Units::Systemd(sd) => {
                // The manager looks a bare name up on its own compile-time path only.
                let program = crate::runners::on_path(&spec.program).ok_or_else(|| Error::NotFound(format!("{}: not on PATH", spec.program)))?;
                let name = qualified(&spec.name);
                let props = unit_properties(spec, &program);
                let manager = sd.manager().await?;
                let mut jobs = manager.receive_signal("JobRemoved").await.map_err(|e| Error::Io(format!("JobRemoved: {e}")))?;
                let aux: Vec<(String, Vec<(String, Value)>)> = vec![];
                let job = match manager.call::<_, _, OwnedObjectPath>("StartTransientUnit", &(name.as_str(), "fail", &props, &aux)).await {
                    Ok(job) => job,
                    Err(e) if is_dbus_error(&e, UNIT_EXISTS) => return Err(Error::Busy(format!("{name} is already running"))),
                    Err(e) => return Err(Error::Io(format!("StartTransientUnit({name}): {e}"))),
                };
                match wait_job(&mut jobs, &job, START_WAIT).await.as_deref() {
                    Some("done") => Ok(()),
                    Some(result) => Err(Error::Io(format!("{name}: start job {result}"))),
                    None => Err(Error::Io(format!("{name}: start job still queued after {}s", START_WAIT.as_secs()))),
                }
            }
            #[cfg(test)]
            Units::Memory(m) => m.start(spec),
        }
    }

    /// `deactivating` counts: `ExecStopPost` is still running the session's end.
    pub async fn is_active(&self, unit: &str) -> bool {
        match self {
            Units::Systemd(sd) => match sd.unit_proxy(&qualified(unit), "org.freedesktop.systemd1.Unit").await {
                Ok(Some(p)) => {
                    p.get_property::<String>("ActiveState").await.is_ok_and(|s| matches!(s.as_str(), "active" | "activating" | "deactivating" | "reloading"))
                }
                _ => false,
            },
            #[cfg(test)]
            Units::Memory(m) => m.units.lock().unwrap().get(unit).map(|u| u.active).unwrap_or(false),
        }
    }

    /// The game first, then the unit: a stop job SIGTERMs the whole cgroup at once, and gamescope dies before the game, which loses its X server and can save nothing.
    /// `term_twice`: SIGTERM again every ~3 s (Dolphin takes the first as a "quit?" prompt); off, the first is the only one until the stop job.
    pub async fn stop(&self, unit: &str, term_twice: bool) -> Result<()> {
        match self {
            Units::Systemd(sd) => {
                // systemd 260 drops the stop job of a frozen unit ("Cannot stop frozen unit") and reports success.
                let _ = sd.manager().await?.call::<_, _, ()>("ThawUnit", &(qualified(unit).as_str(),)).await;
                if let Some(cg) = self.cgroup(unit).await {
                    let mut game = game_pids(&cgroup_procs(&cg));
                    for round in 0..20 {
                        if game.is_empty() {
                            break;
                        }
                        if round == 0 || (term_twice && round % 6 == 0) {
                            for pid in game.iter().filter_map(|p| rustix::process::Pid::from_raw(*p as i32)) {
                                let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        game = game_pids(&cgroup_procs(&cg));
                    }
                }
                self.stop_unit(unit).await
            }
            #[cfg(test)]
            Units::Memory(m) => {
                m.record(format!("unit:stop {unit}"));
                m.finish(unit, 143);
                Ok(())
            }
        }
    }

    /// The stop job alone, waited for `STOP_WAIT` at most; an unloaded unit is already stopped.
    pub async fn stop_unit(&self, unit: &str) -> Result<()> {
        match self {
            Units::Systemd(sd) => {
                let name = qualified(unit);
                let manager = sd.manager().await?;
                let mut jobs = manager.receive_signal("JobRemoved").await.map_err(|e| Error::Io(format!("JobRemoved: {e}")))?;
                let job = match manager.call::<_, _, OwnedObjectPath>("StopUnit", &(name.as_str(), "replace")).await {
                    Ok(job) => job,
                    Err(e) if is_dbus_error(&e, NO_SUCH_UNIT) => return Ok(()),
                    Err(e) => return Err(Error::Io(format!("StopUnit({name}): {e}"))),
                };
                let _ = wait_job(&mut jobs, &job, STOP_WAIT).await;
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
            Units::Systemd(sd) => {
                let method = if on { "FreezeUnit" } else { "ThawUnit" };
                sd.manager().await?.call::<_, _, ()>(method, &(qualified(unit).as_str(),)).await.map_err(|e| Error::Io(format!("{method}({unit}): {e}")))
            }
            #[cfg(test)]
            Units::Memory(m) => {
                m.record(format!("unit:{} {unit}", if on { "freeze" } else { "thaw" }));
                Ok(())
            }
        }
    }

    /// From the journal, the one source left once the unit is collected (a reboot, `reconcile`).
    pub async fn log(&self, unit: &str) -> UnitLog {
        match self {
            Units::Systemd(_) => {
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
            Units::Systemd(sd) => {
                let name = qualified(unit);
                let iface = if name.ends_with(".scope") { "org.freedesktop.systemd1.Scope" } else { "org.freedesktop.systemd1.Service" };
                let proxy = sd.unit_proxy(&name, iface).await.ok().flatten()?;
                proxy.get_property::<String>("ControlGroup").await.ok().filter(|s| !s.is_empty())
            }
            #[cfg(test)]
            Units::Memory(m) => m.units.lock().unwrap().contains_key(unit).then(|| format!("/memory/{unit}")),
        }
    }

    pub async fn adopt_scope(&self, pid: u32) -> Result<String> {
        let name = format!("universe-launcher-{pid}.scope");
        match self {
            Units::Systemd(sd) => {
                let manager = sd.manager().await?;
                let mut jobs = manager.receive_signal("JobRemoved").await.map_err(|e| Error::Unavailable(format!("JobRemoved: {e}")))?;
                let props: Vec<(&str, Value)> = vec![("PIDs", vec![pid].into()), ("Description", "Universe launcher".into())];
                let aux: Vec<(String, Vec<(String, Value)>)> = vec![];
                match manager.call::<_, _, OwnedObjectPath>("StartTransientUnit", &(name.as_str(), "fail", props, aux)).await {
                    Ok(job) => match wait_job(&mut jobs, &job, START_WAIT).await.as_deref() {
                        Some("done") => {}
                        Some(result) => return Err(Error::Unavailable(format!("{name}: start job {result}"))),
                        None => return Err(Error::Unavailable(format!("{name}: start job still queued"))),
                    },
                    // A scope left by an earlier core of this process counts only if we are in it.
                    Err(e) if is_dbus_error(&e, UNIT_EXISTS) && in_cgroup_of(&name) => {}
                    Err(e) => return Err(Error::Unavailable(format!("StartTransientUnit({name}): {e}"))),
                }
                if !in_cgroup_of(&name) {
                    return Err(Error::Unavailable(format!("{name}: this process was not moved into it")));
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
            let argv = std::fs::read(format!("/proc/{pid}/cmdline"))
                .unwrap_or_default()
                .split(|b| *b == 0)
                .filter(|a| !a.is_empty())
                .map(|a| String::from_utf8_lossy(a).into_owned())
                .collect();
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
        let ts = v["__REALTIME_TIMESTAMP"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(chrono::DateTime::from_timestamp_micros)
            .map(|t| t.with_timezone(&chrono::Local));
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
    Live {
        profile: Profile,
        extension: String,
    },
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
            Pads::Inputplumber => crate::inputplumber::engage().await,
            #[cfg(test)]
            Pads::Memory(m) => {
                m.record("pads:engage".into());
                true
            }
        }
    }

    pub async fn release(&self) {
        match self {
            Pads::Inputplumber => crate::inputplumber::release().await,
            #[cfg(test)]
            Pads::Memory(m) => m.record("pads:release".into()),
        }
    }

    /// Hand the raw pads back before a game that reads them directly, in case a prior session left them held.
    pub async fn ensure_free(&self) {
        match self {
            Pads::Inputplumber => crate::inputplumber::ensure_free().await,
            #[cfg(test)]
            Pads::Memory(m) => m.record("pads:ensure_free".into()),
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
            r#"{"MESSAGE":"Started [systemd-run] umu-run","__REALTIME_TIMESTAMP":"1789147978000000"}"#,
            "\n",
            r#"{"MESSAGE":"universe-game-x.service: Main process exited, code=exited, status=3/NOTIMPLEMENTED","__REALTIME_TIMESTAMP":"1789147979000000"}"#,
            "\n",
            r#"{"MESSAGE":"universe-game-x.service: Failed with result 'exit-code'.","__REALTIME_TIMESTAMP":"1789147980000000"}"#,
            "\n",
            r#"{"MESSAGE":"universe-game-x.service: Consumed 17.224s CPU time over 54.416s wall clock time, 1.6G memory peak.","__REALTIME_TIMESTAMP":"1789147981000000"}"#,
            "\n",
        );
        let log = parse_unit_log(lines);
        assert_eq!(log.exit, Some(3));
        assert_eq!((log.ended.unwrap() - log.started.unwrap()).num_seconds(), 3);
    }

    fn prop<'a>(props: &'a [(String, Value<'static>)], name: &str) -> Option<&'a Value<'static>> {
        props.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    #[test]
    fn transient_unit_binds_to_the_launcher_scope_only_when_asked() {
        let spec = UnitSpec {
            name: "universe-game-x-20260911-120000.service".into(),
            description: "Universe: X".into(),
            program: "umu-run".into(),
            args: vec!["/g/x.exe".into(), "-w".into()],
            env: BTreeMap::from([("PATH".to_string(), "/bin".to_string()), ("WINEPREFIX".to_string(), "/p".to_string())]),
            unset_env: vec!["WAYLAND_DISPLAY".into()],
            cwd: Some("/nonexistent".into()),
            properties: vec![("ExitType".into(), Prop::Str("cgroup".into())), ("TimeoutStopUSec".into(), Prop::U64(80_000_000))],
            bind_to: None,
            stop_post: vec!["/usr/bin/universe".into(), "session-end".into(), "x".into(), "20260911-120000".into()],
        };
        let plain = unit_properties(&spec, Path::new("/nix/store/u/bin/umu-run"));
        let exec = |v: &Value<'static>| -> Vec<(String, Vec<String>, bool)> { v.try_clone().unwrap().downcast().unwrap() };
        assert_eq!(
            exec(prop(&plain, "ExecStart").unwrap()),
            [("/nix/store/u/bin/umu-run".to_string(), vec!["/nix/store/u/bin/umu-run".to_string(), "/g/x.exe".into(), "-w".into()], false)]
        );
        assert_eq!(
            exec(prop(&plain, "ExecStopPost").unwrap()),
            [("/usr/bin/universe".to_string(), vec!["/usr/bin/universe".to_string(), "session-end".into(), "x".into(), "20260911-120000".into()], false)]
        );
        assert_eq!(prop(&plain, "Description").unwrap(), &Value::from("Universe: X"));
        assert_eq!(prop(&plain, "CollectMode").unwrap(), &Value::from("inactive-or-failed"));
        assert_eq!(prop(&plain, "ExitType").unwrap(), &Value::from("cgroup"));
        assert_eq!(prop(&plain, "TimeoutStopUSec").unwrap(), &Value::from(80_000_000u64));
        assert_eq!(prop(&plain, "Environment").unwrap(), &Value::from(vec!["PATH=/bin".to_string(), "WINEPREFIX=/p".into()]));
        assert_eq!(prop(&plain, "UnsetEnvironment").unwrap(), &Value::from(vec!["WAYLAND_DISPLAY".to_string()]));
        assert!(prop(&plain, "BindsTo").is_none() && prop(&plain, "After").is_none() && prop(&plain, "WorkingDirectory").is_none());
        let bound = unit_properties(&UnitSpec { bind_to: Some("universe-launcher-4242.scope".into()), ..spec }, Path::new("/nix/store/u/bin/umu-run"));
        assert_eq!(prop(&bound, "BindsTo").unwrap(), &Value::from(vec!["universe-launcher-4242.scope".to_string()]));
        assert_eq!(prop(&bound, "After").unwrap(), &Value::from(vec!["universe-launcher-4242.scope".to_string()]));
        assert_eq!(bound.len(), plain.len() + 2);
    }

    #[test]
    fn a_bare_unit_name_is_a_service() {
        assert_eq!(qualified("universe-capture-start-1"), "universe-capture-start-1.service");
        assert_eq!(qualified("universe-game-x-1.service"), "universe-game-x-1.service");
        assert_eq!(qualified("universe-launcher-42.scope"), "universe-launcher-42.scope");
        assert_eq!(qualified("universe-game-x.y-1"), "universe-game-x.y-1.service");
    }
}

/// Against the user manager on the session bus: `just test-live`.
#[cfg(test)]
mod live {
    use super::*;

    fn units() -> Units {
        Units::Systemd(Systemd::default())
    }

    async fn freezer_state(units: &Units, unit: &str) -> String {
        let Units::Systemd(sd) = units else { unreachable!() };
        sd.unit_proxy(unit, "org.freedesktop.systemd1.Unit").await.unwrap().unwrap().get_property::<String>("FreezerState").await.unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn a_transient_unit_starts_freezes_thaws_and_stops() {
        let units = units();
        let name = format!("universe-test-{}", std::process::id());
        let spec =
            UnitSpec { name: name.clone(), description: "Universe live test".into(), program: "sleep".into(), args: vec!["300".into()], ..Default::default() };
        units.start(&spec).await.unwrap();
        assert!(units.is_active(&name).await);
        assert!(units.cgroup(&name).await.is_some_and(|cg| cg.ends_with(&format!("{name}.service"))));
        units.freeze(&name, true).await.unwrap();
        assert_eq!(freezer_state(&units, &format!("{name}.service")).await, "frozen");
        units.freeze(&name, false).await.unwrap();
        assert_eq!(freezer_state(&units, &format!("{name}.service")).await, "running");
        // A frozen unit stops all the same
        units.freeze(&name, true).await.unwrap();
        units.stop(&name, true).await.unwrap();
        assert!(!units.is_active(&name).await);
        assert!(units.cgroup(&name).await.is_none(), "collected once inactive");
        assert!(units.stop_unit(&name).await.is_ok(), "stopping an unloaded unit is fine");
    }

    #[tokio::test]
    #[ignore]
    async fn a_program_off_path_is_refused_before_systemd_sees_it() {
        let spec = UnitSpec { name: "universe-test-nowhere".into(), program: "universe-no-such-program".into(), ..Default::default() };
        assert!(matches!(units().start(&spec).await, Err(Error::NotFound(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn adopt_scope_moves_this_process_into_it() {
        let name = units().adopt_scope(std::process::id()).await.unwrap();
        assert!(in_cgroup_of(&name));
        assert_eq!(units().adopt_scope(std::process::id()).await.unwrap(), name, "idempotent");
    }
}
