//! A game session in the launcher's gamescope, its systemd, gamescope root, game and HUD layer faked; nothing changed in game is a setting.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use universe::core::Core;
use universe::session::Runtime;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode};
use x11rb::wrapper::ConnectionExt as _;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

#[path = "../src/testbus/mod.rs"]
#[allow(dead_code)]
mod testbus;

const MANAGER: &str = "/org/freedesktop/systemd1";
const WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.systemd1")]
enum SdError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NoSuchUnit(String),
}

#[derive(Default)]
struct Loaded {
    path: OwnedObjectPath,
    pid: Option<u32>,
    stopping: Option<OwnedObjectPath>,
}

#[derive(Default)]
struct Units {
    loaded: BTreeMap<String, Loaded>,
    started: Vec<(String, Vec<String>)>,
    jobs: u32,
}

type Shared = Arc<Mutex<Units>>;

struct Manager {
    units: Shared,
}

fn object(path: String) -> OwnedObjectPath {
    ObjectPath::try_from(path).unwrap().into()
}

impl Manager {
    fn job(&self) -> (u32, OwnedObjectPath) {
        let mut u = self.units.lock().unwrap();
        u.jobs += 1;
        (u.jobs, object(format!("{MANAGER}/job/{}", u.jobs)))
    }
}

async fn job_done(conn: zbus::Connection, id: u32, job: OwnedObjectPath, unit: String) {
    tokio::time::sleep(Duration::from_millis(20)).await;
    let emitter = SignalEmitter::new(&conn, MANAGER).unwrap();
    Manager::job_removed(&emitter, id, job.as_ref(), &unit, "done").await.unwrap();
}

fn prop<T: TryFrom<OwnedValue>>(props: &[(String, OwnedValue)], name: &str) -> Option<T> {
    props.iter().find(|(k, _)| k == name).and_then(|(_, v)| T::try_from(v.try_clone().ok()?).ok())
}

async fn run_game(conn: zbus::Connection, units: Shared, name: String, props: Vec<(String, OwnedValue)>) {
    let env: Vec<String> = prop(&props, "Environment").unwrap_or_default();
    let env: Vec<(String, String)> = env.iter().filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.into(), v.into())).collect();
    let cwd: Option<String> = prop(&props, "WorkingDirectory");
    let exec = |key| prop::<Vec<(String, Vec<String>, bool)>>(&props, key).and_then(|e| e.into_iter().next()).map(|(_, argv, _)| argv);
    let argv = exec("ExecStart").unwrap();
    let mut cmd = tokio::process::Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .env_clear()
        .envs(env.iter().cloned())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd.spawn().unwrap();
    units.lock().unwrap().loaded.get_mut(&name).unwrap().pid = child.id();
    let status = child.wait().await.unwrap();
    let (code, why) = match std::os::unix::process::ExitStatusExt::signal(&status) {
        Some(15) => ("killed".to_string(), "TERM".to_string()),
        Some(s) => ("killed".to_string(), s.to_string()),
        None => ("exited".to_string(), status.code().unwrap_or(0).to_string()),
    };
    if let Some(post) = exec("ExecStopPost") {
        let out = tokio::process::Command::new(&post[0])
            .args(&post[1..])
            .env_clear()
            .envs(env.iter().cloned())
            .env("EXIT_CODE", code)
            .env("EXIT_STATUS", why)
            .output()
            .await
            .unwrap();
        assert!(out.status.success(), "session-end: {}", String::from_utf8_lossy(&out.stderr));
    }
    let gone = units.lock().unwrap().loaded.remove(&name).unwrap();
    conn.object_server().remove::<Unit, _>(&gone.path).await.unwrap();
    conn.object_server().remove::<Service, _>(&gone.path).await.unwrap();
    if let Some(job) = gone.stopping {
        let id = job.as_str().rsplit('/').next().unwrap().parse().unwrap();
        job_done(conn, id, job, name).await;
    }
}

#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl Manager {
    fn subscribe(&self) {}

    async fn start_transient_unit(
        &self,
        #[zbus(connection)] conn: &zbus::Connection,
        name: String,
        _mode: String,
        props: Vec<(String, OwnedValue)>,
        _aux: Vec<(String, Vec<(String, OwnedValue)>)>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let (id, job) = self.job();
        let argv = prop::<Vec<(String, Vec<String>, bool)>>(&props, "ExecStart").and_then(|e| e.into_iter().next()).map(|(_, a, _)| a).unwrap_or_default();
        let game = name.starts_with("universe-game-");
        {
            let mut u = self.units.lock().unwrap();
            u.started.push((name.clone(), argv));
            if game {
                let path = object(format!("{MANAGER}/unit/{id}"));
                u.loaded.insert(name.clone(), Loaded { path, ..Loaded::default() });
            }
        }
        if game {
            let path = self.units.lock().unwrap().loaded[&name].path.clone();
            conn.object_server().at(&path, Unit).await?;
            conn.object_server().at(&path, Service).await?;
            tokio::spawn(run_game(conn.clone(), self.units.clone(), name.clone(), props));
        }
        tokio::spawn(job_done(conn.clone(), id, job.clone(), name));
        Ok(job)
    }

    fn get_unit(&self, name: String) -> Result<OwnedObjectPath, SdError> {
        self.units.lock().unwrap().loaded.get(&name).map(|u| u.path.clone()).ok_or(SdError::NoSuchUnit(name))
    }

    fn stop_unit(&self, name: String, _mode: String) -> Result<OwnedObjectPath, SdError> {
        let (_, job) = self.job();
        let mut u = self.units.lock().unwrap();
        let unit = u.loaded.get_mut(&name).ok_or(SdError::NoSuchUnit(name))?;
        unit.stopping = Some(job.clone());
        if let Some(pid) = unit.pid {
            // SAFETY: the game's process is this manager's child, still unreaped.
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        Ok(job)
    }

    fn freeze_unit(&self, _name: String) {}

    fn thaw_unit(&self, _name: String) {}

    #[zbus(property)]
    fn version(&self) -> String {
        "258".into()
    }

    #[zbus(signal)]
    async fn job_removed(emitter: &SignalEmitter<'_>, id: u32, job: ObjectPath<'_>, unit: &str, result: &str) -> zbus::Result<()>;
}

struct Unit;

#[zbus::interface(name = "org.freedesktop.systemd1.Unit")]
impl Unit {
    #[zbus(property)]
    fn active_state(&self) -> String {
        "active".into()
    }
}

struct Service;

#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl Service {
    #[zbus(property)]
    fn control_group(&self) -> String {
        String::new()
    }
}

/// The launcher's gamescope as the core reads it: a display whose root names gamescope's pid.
struct Gamescope {
    server: std::process::Child,
    conn: x11rb::rust_connection::RustConnection,
    root: u32,
    display: String,
}

impl Gamescope {
    fn start() -> Gamescope {
        let mut fds = [0; 2];
        // SAFETY: `fds` has room for the two ends.
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
        let server = std::process::Command::new("Xvfb")
            .args(["-displayfd", &fds[1].to_string(), "-nolisten", "tcp", "-screen", "0", "64x64x24"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Xvfb stands in for the launcher's gamescope: put it on PATH (the xvfb package)");
        // SAFETY: the write end is the server's now; the read end is ours alone.
        let mut number = String::new();
        unsafe {
            libc::close(fds[1]);
            std::io::Read::read_to_string(&mut <std::fs::File as std::os::fd::FromRawFd>::from_raw_fd(fds[0]), &mut number).unwrap();
        }
        assert!(!number.trim().is_empty(), "Xvfb exited before naming its display");
        let display = format!(":{}", number.trim());
        let (conn, screen) = x11rb::connect(Some(&display)).unwrap();
        let root = conn.setup().roots[screen].root;
        let gamescope = Gamescope { server, conn, root, display };
        gamescope.set("GAMESCOPE_PID", std::process::id());
        gamescope
    }

    fn atom(&self, name: &str) -> u32 {
        self.conn.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom
    }

    fn set(&self, name: &str, value: u32) {
        self.conn.change_property32(PropMode::REPLACE, self.root, self.atom(name), AtomEnum::CARDINAL, &[value]).unwrap();
        self.conn.flush().unwrap();
    }

    fn card(&self, name: &str) -> Option<u32> {
        let reply = self.conn.get_property(false, self.root, self.atom(name), AtomEnum::CARDINAL, 0, 1).unwrap().reply().unwrap();
        reply.value32().and_then(|mut v| v.next())
    }

    fn filter(&self) -> (Option<u32>, Option<u32>) {
        (self.card("GAMESCOPE_SCALING_FILTER"), self.card("GAMESCOPE_FSR_SHARPNESS"))
    }
}

impl Drop for Gamescope {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}

fn layer(game: &str) -> Arc<Mutex<Vec<String>>> {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(universe::mangoapp::layer_control(game)).unwrap();
    let listener = std::os::unix::net::UnixListener::bind_addr(&addr).unwrap();
    let got = Arc::new(Mutex::new(vec![]));
    let seen = got.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut stream.unwrap(), &mut text).unwrap();
            seen.lock().unwrap().push(text);
        }
    });
    got
}

fn stub(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    path
}

fn which(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap()).map(|d| d.join(name)).find(|p| p.is_file()).unwrap_or_else(|| panic!("{name} on PATH"))
}

fn universe(args: &[&str]) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_universe")).args(args).output().unwrap();
    assert!(out.status.success(), "universe {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

async fn until<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        if let Some(v) = probe() {
            return v;
        }
        assert!(tokio::time::Instant::now() < deadline, "never: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

struct Watcher {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    lines: tokio::io::Lines<tokio::io::BufReader<tokio::process::ChildStdout>>,
}

impl Watcher {
    async fn start() -> Watcher {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_universe"))
            .args(["controller", "watch", "--json"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let lines = tokio::io::BufReader::new(child.stdout.take().unwrap()).lines();
        let mut w = Watcher { child, stdin, lines };
        w.next("ready").await;
        w
    }

    async fn next(&mut self, event: &str) -> serde_json::Value {
        loop {
            let line = tokio::time::timeout(WAIT, self.lines.next_line()).await.expect("the watcher went quiet").unwrap().expect("the watcher ended");
            let v: serde_json::Value = serde_json::from_str(&line).unwrap();
            if v["event"] == event {
                return v;
            }
        }
    }

    async fn macro_(&mut self, action: &str) -> serde_json::Value {
        self.stdin.write_all(format!("{}\n", serde_json::json!({"cmd": "run", "action": action})).as_bytes()).await.unwrap();
        self.next("hud").await
    }
}

fn runtime(mangohud: bool, fps_limit: &str, filter: &str, sharpness: Option<u32>) -> Runtime {
    Runtime { mangohud, fps_limit: fps_limit.into(), gamescope_filter: filter.into(), gamescope_sharpness: sharpness }
}

#[test]
fn hud_limit_and_filter_changed_in_game_last_the_session_and_leave_the_settings_alone() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let stubs = root.join("bin");
    std::fs::create_dir_all(&stubs).unwrap();
    for tool in ["env", "sh"] {
        std::os::unix::fs::symlink(which(tool), stubs.join(tool)).unwrap();
    }
    // No mangoapp on PATH: its control queue is the machine's, read by every mangoapp the user runs.
    let mangohud = stub(&stubs, "mangohud", "exec \"$@\"");
    stub(&stubs, "journalctl", "exit 0");
    let game = stub(root, "game.sh", &format!("exec {} 600", which("sleep").display()));
    let gamescope = Gamescope::start();
    let bus = testbus::Bus::start();
    for (var, sub) in [("UNIVERSE_DATA_HOME", "data"), ("UNIVERSE_CONFIG_HOME", "config"), ("UNIVERSE_STATE_HOME", "state"), ("UNIVERSE_CACHE_HOME", "cache")] {
        std::env::set_var(var, root.join(sub));
    }
    for (var, sub) in [("UNIVERSE_MODULES_PATH", "modules"), ("UNIVERSE_SOURCES_PATH", "sources"), ("XDG_RUNTIME_DIR", "run")] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
        std::env::set_var(var, root.join(sub));
    }
    for (var, value) in [
        ("HOME", root.to_str().unwrap()),
        ("XDG_CONFIG_HOME", root.join(".config").to_str().unwrap()),
        ("XDG_DATA_HOME", root.join(".local/share").to_str().unwrap()),
        ("PATH", stubs.to_str().unwrap()),
        ("UNIVERSE_BIN", env!("CARGO_BIN_EXE_universe")),
        ("DBUS_SESSION_BUS_ADDRESS", &bus.address),
        ("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus"),
        ("DISPLAY", &gamescope.display),
        ("GAMESCOPE_WAYLAND_DISPLAY", "gamescope-0"),
        ("UNIVERSE_OWN_GAMESCOPE", "nested"),
    ] {
        std::env::set_var(var, value);
    }
    for var in ["WAYLAND_DISPLAY", "SteamGameId", "HYPRLAND_INSTANCE_SIGNATURE", "UNIVERSE_SESSION"] {
        std::env::remove_var(var);
    }
    let config = root.join("config/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "schema = 1\n[desktop]\nprofile = \"none\"\nkeep_awake = false\n[controller]\nenabled = false\nmacros = []\n[saves]\nauto_backup = false\n\
         [modules]\nenabled = []\n[launch]\ngamescope = false\nhide_cursor = false\nmangohud = false\nfps_limit = \"none\"\n\
         gamescope_filter = \"nis\"\ngamescope_sharpness = 7\n",
    )
    .unwrap();
    // The id names the layer's abstract socket, which every run on the machine shares.
    let title = format!("Session Quest {}", std::process::id());
    let added: serde_json::Value = serde_json::from_str(&universe(&["--json", "add", game.to_str().unwrap(), "--runner", "linux", "--title", &title])).unwrap();
    let id = added["id"].as_str().unwrap().to_string();
    universe(&["set", &id, "mangohud=true", "fps_limit=60", "gamescope_filter=fsr", "gamescope_sharpness=3"]);
    let game_toml = root.join("data/games").join(&id).join("game.toml");
    let settings = || (std::fs::read_to_string(&game_toml).unwrap(), std::fs::read_to_string(&config).unwrap());
    let before = settings();

    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(async {
        let units = Shared::default();
        let manager = bus.connect().await;
        manager.object_server().at(MANAGER, Manager { units: units.clone() }).await.unwrap();
        manager.request_name("org.freedesktop.systemd1").await.unwrap();
        let toggles = layer(&id);
        let hud_conf = universe::launcher::layer_conf_path();
        let read_conf = || std::fs::read_to_string(&hud_conf).unwrap();

        let core = Core::open().await.unwrap();
        core.launch(&id, "", "").await.unwrap();
        let launched = read_conf();
        assert!(launched.contains("fps_limit=60\n") && !launched.contains("no_display"), "the game's own HUD and limit: {launched}");
        let (unit, argv) = units.lock().unwrap().started.last().cloned().unwrap();
        assert!(unit.starts_with(&format!("universe-game-{id}-")) && argv[0] == mangohud.to_str().unwrap(), "{unit}: {argv:?}");
        assert_eq!(core.runtime().await.unwrap(), runtime(true, "60", "nis", Some(7)), "the game's HUD and limit, the filter the launcher's gamescope holds");
        assert_eq!(gamescope.filter(), (None, None), "the launcher's gamescope keeps the filter it was started with");

        assert!(!core.set_mangohud(Some(false)).await.unwrap(), "the dock hides the HUD");
        let mut watcher = Watcher::start().await;
        assert_eq!(watcher.macro_("mangohud").await["shown"], true, "the pad's macro flips the HUD the dock left");
        assert_eq!(watcher.macro_("mangohud").await["shown"], false);
        core.set_fps_limit("30").await.unwrap();
        core.nest_filter("integer", None).await.unwrap();
        assert_eq!(core.runtime().await.unwrap(), runtime(false, "30", "integer", None));
        let changed = read_conf();
        assert!(changed.contains("no_display\n") && changed.contains("fps_limit=30\n"), "a new limit keeps the HUD the macro hid: {changed}");
        assert_eq!(until("three toggles reach the layer", || Some(toggles.lock().unwrap().clone()).filter(|t| t.len() == 3)).await, [":hud;"; 3]);
        assert_eq!(gamescope.filter(), (Some(2), None), "integer scaling, gamescope's own sharpness");
        assert_eq!(settings(), before, "nothing changed in game is a setting");

        core.stop("").await.unwrap();
        until("the session ends", || units.lock().unwrap().loaded.is_empty().then_some(())).await;
        assert!(core.current().await.is_none(), "the marker is gone");
        assert_eq!(gamescope.filter(), (Some(4), Some(7)), "the end puts the launcher's gamescope back on Settings › Launch's NIS at 7");
        assert_eq!(settings(), before, "game.toml and config.toml as they were");
        let rows = std::fs::read_to_string(root.join("data/games").join(&id).join("sessions.jsonl")).unwrap();
        assert_eq!(rows.lines().count(), 1, "{rows}");

        core.launch(&id, "", "").await.unwrap();
        assert_eq!(core.runtime().await.unwrap(), runtime(true, "60", "nis", Some(7)), "the relaunch starts from the settings");
        assert_eq!(read_conf(), launched, "the HUD shown and the game's limit again");
        core.stop("").await.unwrap();
        until("the second session ends", || units.lock().unwrap().loaded.is_empty().then_some(())).await;
        assert_eq!(settings(), before);
        drop(watcher.stdin);
        let _ = watcher.child.wait().await;
    });
}
