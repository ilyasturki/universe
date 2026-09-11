use std::time::Duration;

use zbus::proxy;

use crate::{BUS_NAME, OBJECT_PATH};

#[proxy(interface = "io.github.ilyasturki.Universe.Library1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Library1 {
    fn list(&self) -> zbus::Result<String>;
    fn get(&self, id: &str) -> zbus::Result<String>;
    fn resolve(&self, query: &str) -> zbus::Result<Vec<String>>;
    fn set(&self, id: &str, key: &str, value: &str) -> zbus::Result<()>;
    fn remove(&self, id: &str, purge: bool) -> zbus::Result<()>;
    fn rescan(&self) -> zbus::Result<()>;
    fn import_lutris(&self, apply: bool) -> zbus::Result<String>;
    #[zbus(signal)]
    fn library_changed(&self, ids: Vec<String>) -> zbus::Result<()>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Session1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Session1 {
    fn launch(&self, id: &str, screen: &str) -> zbus::Result<String>;
    fn stop(&self, session_id: &str) -> zbus::Result<()>;
    fn screenshot(&self) -> zbus::Result<String>;
    fn sessions(&self, id: &str) -> zbus::Result<String>;
    #[zbus(property)]
    fn current(&self) -> zbus::Result<String>;
    #[zbus(signal)]
    fn session_started(&self, session_id: &str, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn session_ended(&self, session_id: &str, id: &str, duration_s: u32) -> zbus::Result<()>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Sources1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Sources1 {
    fn list(&self) -> zbus::Result<String>;
    fn login_url(&self, source: &str) -> zbus::Result<String>;
    fn login(&self, source: &str, code: &str) -> zbus::Result<String>;
    fn library(&self, source: &str) -> zbus::Result<String>;
    fn refresh_library(&self, source: &str) -> zbus::Result<String>;
    fn search(&self, source: &str, query: &str) -> zbus::Result<String>;
    fn info(&self, source: &str, game_id: &str) -> zbus::Result<String>;
    fn install(&self, source: &str, game_id: &str) -> zbus::Result<String>;
    fn update(&self, source: &str, game_id: &str) -> zbus::Result<String>;
    fn updates(&self) -> zbus::Result<String>;
    fn scan(&self, source: &str) -> zbus::Result<String>;
    fn jobs(&self) -> zbus::Result<String>;
    #[zbus(signal)]
    fn progress(&self, job_id: &str, done: u64, total: u64, message: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn job_finished(&self, job_id: &str, ok: bool, message: &str) -> zbus::Result<()>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Media1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Media1 {
    fn refresh(&self, id: &str, force: bool) -> zbus::Result<String>;
    fn set_slot(&self, id: &str, slot: &str, path: &str) -> zbus::Result<()>;
    fn unset(&self, id: &str, slot: &str) -> zbus::Result<()>;
    fn candidates(&self, id: &str, slot: &str) -> zbus::Result<String>;
    fn pin(&self, id: &str, provider: &str, provider_id: &str) -> zbus::Result<()>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Recording1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Recording1 {
    fn file(&self, session_id: &str, path: &str) -> zbus::Result<String>;
    fn list(&self, id: &str) -> zbus::Result<String>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Journal1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Journal1 {
    fn add_entry(&self, session_id: &str, json: &str) -> zbus::Result<()>;
    fn list(&self, id: &str) -> zbus::Result<String>;
    fn render(&self, id: &str) -> zbus::Result<String>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Modules1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Modules1 {
    fn list(&self) -> zbus::Result<String>;
    fn enable(&self, id: &str, enabled: bool) -> zbus::Result<()>;
    fn get_settings(&self, module: &str, game_id: &str) -> zbus::Result<String>;
    fn set_setting(&self, module: &str, game_id: &str, key: &str, value: &str) -> zbus::Result<()>;
    fn doctor(&self) -> zbus::Result<String>;
}

#[proxy(interface = "io.github.ilyasturki.Universe.Settings1", default_service = "io.github.ilyasturki.Universe", default_path = "/io/github/ilyasturki/Universe")]
pub trait Settings1 {
    fn get(&self) -> zbus::Result<String>;
    fn set(&self, key: &str, value: &str) -> zbus::Result<()>;
    fn reload(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}

pub struct Client {
    pub conn: zbus::Connection,
}

impl Client {
    /// Connects to the daemon, starting `universed` detached if nobody owns the name yet.
    pub async fn connect() -> anyhow::Result<Client> {
        let conn = zbus::Connection::session().await?;
        if !name_owned(&conn).await? {
            spawn_daemon()?;
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if name_owned(&conn).await? {
                    break;
                }
            }
            if !name_owned(&conn).await? {
                anyhow::bail!("universed did not come up on {BUS_NAME}");
            }
        }
        Ok(Client { conn })
    }

    pub async fn library(&self) -> zbus::Result<Library1Proxy<'_>> {
        Library1Proxy::new(&self.conn).await
    }
    pub async fn session(&self) -> zbus::Result<Session1Proxy<'_>> {
        Session1Proxy::new(&self.conn).await
    }
    pub async fn sources(&self) -> zbus::Result<Sources1Proxy<'_>> {
        Sources1Proxy::new(&self.conn).await
    }
    pub async fn media(&self) -> zbus::Result<Media1Proxy<'_>> {
        Media1Proxy::new(&self.conn).await
    }
    pub async fn recording(&self) -> zbus::Result<Recording1Proxy<'_>> {
        Recording1Proxy::new(&self.conn).await
    }
    pub async fn journal(&self) -> zbus::Result<Journal1Proxy<'_>> {
        Journal1Proxy::new(&self.conn).await
    }
    pub async fn modules(&self) -> zbus::Result<Modules1Proxy<'_>> {
        Modules1Proxy::new(&self.conn).await
    }
    pub async fn settings(&self) -> zbus::Result<Settings1Proxy<'_>> {
        Settings1Proxy::new(&self.conn).await
    }
}

async fn name_owned(conn: &zbus::Connection) -> anyhow::Result<bool> {
    let dbus = zbus::fdo::DBusProxy::new(conn).await?;
    Ok(dbus.name_has_owner(BUS_NAME.try_into()?).await?)
}

fn daemon_binary() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.with_file_name("universed");
        if sibling.is_file() {
            return sibling;
        }
    }
    std::path::PathBuf::from("universed")
}

pub fn spawn_daemon() -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;
    let log_dir = crate::paths::state_home();
    std::fs::create_dir_all(&log_dir)?;
    let log = std::fs::OpenOptions::new().create(true).append(true).open(log_dir.join("universed.log"))?;
    let err = log.try_clone()?;
    let mut cmd = std::process::Command::new(daemon_binary());
    cmd.stdin(std::process::Stdio::null()).stdout(log).stderr(err);
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map_err(|e| anyhow::anyhow!("cannot start universed: {e}"))?;
    Ok(())
}

pub fn object_path() -> &'static str {
    OBJECT_PATH
}
