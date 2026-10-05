pub mod bluez;
pub mod nm;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

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
    pub fn start() -> Bus {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        let mut f = std::fs::File::create(&config).unwrap();
        f.write_all(CONFIG.replace("@SOCKET@", &dir.path().join("bus").display().to_string()).as_bytes()).unwrap();
        let mut daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon runs the tests' private bus: put it on PATH (the dbus package)");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address).unwrap();
        Bus { daemon, address: address.trim().to_string(), _dir: dir }
    }

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str()).unwrap().build().await.unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}
