use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

use super::Bus;

const MANAGER: &str = "/org/freedesktop/systemd1";
const UNIT: &str = "/org/freedesktop/systemd1/unit/game";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.systemd1")]
pub enum SdError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NoSuchUnit(String),
}

/// A cgroup of its own under the test process's: what the kernel does to a unit's processes (freezing, signals, emptying) happens for real.
pub struct Cgroup {
    pub path: String,
    dir: PathBuf,
}

impl Cgroup {
    pub fn new() -> Cgroup {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let own = std::fs::read_to_string("/proc/self/cgroup").unwrap();
        let own = own.lines().find_map(|l| l.strip_prefix("0::")).expect("a cgroup v2 hierarchy");
        let path = format!("{}/universe-test-{}-{}", own.trim_end_matches('/'), std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst));
        let dir = PathBuf::from("/sys/fs/cgroup").join(path.trim_start_matches('/'));
        std::fs::create_dir(&dir).unwrap_or_else(|e| panic!("{}: {e} (a stop test needs a writable cgroup; the flake's sandbox skips it)", dir.display()));
        Cgroup { path, dir }
    }

    /// Started inside the cgroup, before its first instruction.
    pub fn spawn(&self, cmd: &mut tokio::process::Command) -> tokio::process::Child {
        let procs = std::fs::OpenOptions::new().write(true).open(self.dir.join("cgroup.procs")).unwrap();
        let fd = procs.as_raw_fd();
        // SAFETY: write(2) is async-signal-safe; "0" moves the calling process, the forked child.
        unsafe {
            cmd.as_std_mut().pre_exec(move || match libc::write(fd, b"0".as_ptr().cast(), 1) {
                1 => Ok(()),
                _ => Err(std::io::Error::last_os_error()),
            });
        }
        cmd.kill_on_drop(true).spawn().unwrap()
    }

    pub fn pids(&self) -> Vec<u32> {
        std::fs::read_to_string(self.dir.join("cgroup.procs")).unwrap_or_default().lines().filter_map(|l| l.parse().ok()).collect()
    }

    fn event(&self, key: &str) -> bool {
        std::fs::read_to_string(self.dir.join("cgroup.events")).unwrap_or_default().lines().any(|l| l == format!("{key} 1"))
    }

    pub fn populated(&self) -> bool {
        self.event("populated")
    }

    /// Returns once the kernel says every process stands still, or runs again.
    pub async fn freeze(&self, on: bool) {
        std::fs::write(self.dir.join("cgroup.freeze"), if on { "1" } else { "0" }).unwrap();
        while self.event("frozen") != on {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

impl Drop for Cgroup {
    fn drop(&mut self) {
        let _ = std::fs::write(self.dir.join("cgroup.kill"), "1");
        for _ in 0..500 {
            if std::fs::remove_dir(&self.dir).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

struct Manager {
    unit: String,
    cgroup: Arc<Cgroup>,
    jobs: AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl Manager {
    fn subscribe(&self) {}

    fn get_unit(&self, name: String) -> Result<OwnedObjectPath, SdError> {
        if name == self.unit && self.cgroup.populated() {
            Ok(ObjectPath::try_from(UNIT).unwrap().into())
        } else {
            Err(SdError::NoSuchUnit(name))
        }
    }

    async fn freeze_unit(&self, _name: String) {
        self.cgroup.freeze(true).await;
    }

    async fn thaw_unit(&self, _name: String) {
        self.cgroup.freeze(false).await;
    }

    /// KillMode=control-group: SIGTERM to every process, the job done once none is left.
    async fn stop_unit(&self, #[zbus(connection)] conn: &zbus::Connection, name: String, _mode: String) -> Result<OwnedObjectPath, SdError> {
        if name != self.unit || !self.cgroup.populated() {
            return Err(SdError::NoSuchUnit(name));
        }
        let id = self.jobs.fetch_add(1, Ordering::SeqCst) + 1;
        let job: OwnedObjectPath = ObjectPath::try_from(format!("{MANAGER}/job/{id}")).unwrap().into();
        for pid in self.cgroup.pids() {
            // SAFETY: a plain kill(2) on a pid of the test's own cgroup.
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        let (conn, cgroup, done) = (conn.clone(), self.cgroup.clone(), job.clone());
        tokio::spawn(async move {
            while cgroup.populated() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            let emitter = SignalEmitter::new(&conn, MANAGER).unwrap();
            Manager::job_removed(&emitter, id, done.as_ref(), &name, "done").await.unwrap();
        });
        Ok(job)
    }

    #[zbus(signal)]
    async fn job_removed(emitter: &SignalEmitter<'_>, id: u32, job: ObjectPath<'_>, unit: &str, result: &str) -> zbus::Result<()>;
}

struct Unit {
    cgroup: Arc<Cgroup>,
}

#[zbus::interface(name = "org.freedesktop.systemd1.Unit")]
impl Unit {
    #[zbus(property)]
    fn active_state(&self) -> String {
        if self.cgroup.populated() { "active" } else { "inactive" }.into()
    }
}

struct Service {
    cgroup: Arc<Cgroup>,
}

#[zbus::interface(name = "org.freedesktop.systemd1.Service")]
impl Service {
    #[zbus(property)]
    fn control_group(&self) -> String {
        self.cgroup.path.clone()
    }
}

/// The user manager on the test bus with one service, `unit`, whose processes are `cgroup`'s.
pub struct Systemd {
    _conn: zbus::Connection,
}

impl Systemd {
    pub async fn serve(bus: &Bus, unit: &str, cgroup: Arc<Cgroup>) -> Systemd {
        let conn = bus.connect().await;
        let server = conn.object_server();
        server.at(MANAGER, Manager { unit: unit.into(), cgroup: cgroup.clone(), jobs: AtomicU32::new(0) }).await.unwrap();
        server.at(UNIT, Unit { cgroup: cgroup.clone() }).await.unwrap();
        server.at(UNIT, Service { cgroup }).await.unwrap();
        conn.request_name("org.freedesktop.systemd1").await.unwrap();
        Systemd { _conn: conn }
    }
}
