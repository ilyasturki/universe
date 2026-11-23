pub mod bluez;
pub mod nm;
pub mod systemd;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

/// A server the test starts for itself would not start: the test skips, saying why, unless UNIVERSE_TEST_DISPLAY requires every such server (the dev shell, the Nix check, CI's distros).
pub fn server_missing<T>(why: &str) -> Option<T> {
    assert!(std::env::var_os("UNIVERSE_TEST_DISPLAY").is_none(), "{why}, and UNIVERSE_TEST_DISPLAY requires it");
    eprintln!("skipped: {why}");
    None
}

/// A test racing processes (signals, a terminal, a deadline) on a packager's build (`UNIVERSE_TEST_PACKAGER`): it skips, named, for tools/race-check to replay under load.
pub fn races_processes(why: &str) -> bool {
    let packager = std::env::var_os("UNIVERSE_TEST_PACKAGER").is_some();
    if packager {
        eprintln!("skipped: {why}, left out of a packager's build: {}", std::thread::current().name().unwrap_or("?"));
    }
    packager
}

/// A test whose verdict is a time measured to the moment, which a loaded machine moves: it skips on a packager's build, and tools/race-check leaves it alone.
pub fn timed_to_the_moment(why: &str) -> bool {
    let packager = std::env::var_os("UNIVERSE_TEST_PACKAGER").is_some();
    if packager {
        eprintln!("skipped: {why}, timed too finely for a packager's build");
    }
    packager
}

pub struct Bus {
    daemon: Child,
    pub address: String,
    _dir: tempfile::TempDir,
}

const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=@SOCKET@</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

impl Bus {
    pub fn start() -> Option<Bus> {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        let mut f = std::fs::File::create(&config).unwrap();
        f.write_all(CONFIG.replace("@SOCKET@", &dir.path().join("bus").display().to_string()).as_bytes()).unwrap();
        let spawned = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut daemon = match spawned {
            Ok(daemon) => daemon,
            Err(e) => return server_missing(&format!("dbus-daemon runs the tests' private bus and is not on PATH (the dbus package): {e}")),
        };
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address).unwrap();
        if address.trim().is_empty() {
            let _ = daemon.wait();
            return server_missing("dbus-daemon exited before naming the tests' private bus");
        }
        Some(Bus { daemon, address: address.trim().to_string(), _dir: dir })
    }

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str()).build().await.unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
