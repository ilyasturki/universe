//! The controller watcher's volume macros on a private PipeWire with two null sinks; the test plays WirePlumber's part.
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{json, Value};

const STEP: u8 = 2;
// Past the watcher's 1 s burst, within which a press reuses the output name it last read.
const PAUSE: Duration = Duration::from_millis(1200);
const WAIT: Duration = Duration::from_secs(5);

struct PipeWire {
    daemon: Child,
    runtime: std::path::PathBuf,
}

impl PipeWire {
    fn start(dir: &Path) -> PipeWire {
        let runtime = dir.join("pipewire");
        std::fs::create_dir_all(&runtime).unwrap();
        let sink = |name: &str, label: &str| {
            format!("{{ factory = adapter, args = {{ factory.name = support.null-audio-sink, node.name = {name}, node.description = \"{label}\", media.class = Audio/Sink, audio.position = [ FL FR ], node.group = test }} }}")
        };
        let conf = format!(
            r#"context.properties = {{ core.daemon = true, core.name = universe-test, support.dbus = false }}
context.spa-libs = {{ audio.convert.* = audioconvert/libspa-audioconvert, support.* = support/libspa-support }}
context.modules = [
    {{ name = libpipewire-module-protocol-native }}
    {{ name = libpipewire-module-metadata }}
    {{ name = libpipewire-module-client-node }}
    {{ name = libpipewire-module-adapter }}
    {{ name = libpipewire-module-spa-node-factory }}
    {{ name = libpipewire-module-access }}
]
context.objects = [
    {{ factory = spa-node-factory, args = {{ factory.name = support.node.driver, node.name = test-driver, node.group = test, priority.driver = 8000 }} }}
    {speakers}
    {headphones}
    {{ factory = metadata, args = {{ metadata.name = default, metadata.values = [ {{ key = default.audio.sink, type = "Spa:String:JSON", value = "{{ \"name\": \"speakers\" }}" }} ] }} }}
]
"#,
            speakers = sink("speakers", "Speakers"),
            headphones = sink("headphones", "Headphones"),
        );
        let path = dir.join("pipewire.conf");
        std::fs::write(&path, conf).unwrap();
        // A dev shell's LD_LIBRARY_PATH carries its own libpipewire, whose modules another build of the daemon cannot load.
        let daemon = Command::new("pipewire")
            .arg("-c")
            .arg(&path)
            .env_remove("LD_LIBRARY_PATH")
            .env("PIPEWIRE_RUNTIME_DIR", &runtime)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("pipewire runs the test's private sound server: put it on PATH (the pipewire package)");
        let pw = PipeWire { daemon, runtime };
        let up = std::time::Instant::now();
        while pw.try_wpctl(&["get-volume", "@DEFAULT_AUDIO_SINK@"]).is_err() {
            assert!(up.elapsed() < WAIT, "the private PipeWire never came up");
            std::thread::sleep(Duration::from_millis(10));
        }
        pw
    }

    fn env<'a>(&'a self, cmd: &'a mut Command) -> &'a mut Command {
        cmd.env("PIPEWIRE_RUNTIME_DIR", &self.runtime).env("PIPEWIRE_REMOTE", "universe-test")
    }

    fn try_wpctl(&self, args: &[&str]) -> Result<String, String> {
        let out = self.env(&mut Command::new("wpctl")).args(args).output().expect("wpctl drives the private sinks: put it on PATH (the wireplumber package)");
        match out.status.success() {
            true => Ok(String::from_utf8_lossy(&out.stdout).trim().to_string()),
            false => Err(String::from_utf8_lossy(&out.stderr).into_owned()),
        }
    }

    fn wpctl(&self, args: &[&str]) -> String {
        self.try_wpctl(args).unwrap_or_else(|e| panic!("wpctl {args:?}: {e}"))
    }

    /// What WirePlumber writes once the user picks another output.
    fn make_default(&self, sink: &str) {
        let out = self
            .env(&mut Command::new("pw-metadata"))
            .args(["-n", "default", "0", "default.audio.sink", &format!("{{ \"name\": \"{sink}\" }}"), "Spa:String:JSON"])
            .output()
            .unwrap();
        assert!(out.status.success(), "pw-metadata: {}", String::from_utf8_lossy(&out.stderr));
    }

    fn level(&self) -> String {
        self.wpctl(&["get-volume", "@DEFAULT_AUDIO_SINK@"])
    }
}

impl Drop for PipeWire {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

struct Watcher {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<Value>,
}

impl Watcher {
    fn start(dir: &Path, pw: &PipeWire) -> Watcher {
        for sub in ["data", "config", "state", "cache", "run"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        // No macro of its own: a real pad pressed meanwhile fires nothing, the test's `run` lines alone do.
        std::fs::write(dir.join("config/config.toml"), "schema = 1\n[desktop]\nprofile = \"none\"\n[controller]\nmacros = []\n").unwrap();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_universe"));
        for (var, sub) in
            [("UNIVERSE_DATA_HOME", "data"), ("UNIVERSE_CONFIG_HOME", "config"), ("UNIVERSE_STATE_HOME", "state"), ("UNIVERSE_CACHE_HOME", "cache")]
        {
            cmd.env(var, dir.join(sub));
        }
        let mut child = pw
            .env(&mut cmd)
            .args(["controller", "watch", "--json"])
            .env("HOME", dir)
            .env("XDG_RUNTIME_DIR", dir.join("run"))
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null/bus")
            .env_remove("GAMESCOPE_WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(serde_json::from_str(&line).unwrap_or(Value::String(line))).is_err() {
                    return;
                }
            }
        });
        let mut w = Watcher { child, stdin, lines };
        w.until(|l| l["event"] == "ready").expect("the watcher never got ready");
        w
    }

    fn press(&mut self, action: &str) {
        writeln!(self.stdin, "{}", json!({"cmd": "run", "action": action})).unwrap();
        self.stdin.flush().unwrap();
    }

    fn until(&mut self, want: impl Fn(&Value) -> bool) -> Option<Value> {
        loop {
            let line = self.lines.recv_timeout(WAIT).ok()?;
            if want(&line) {
                return Some(line);
            }
        }
    }

    fn volume(&mut self) -> Value {
        self.until(|l| l["event"] == "volume").expect("no volume event")
    }

    /// Each volume event up to one at `percent`, in the order they came.
    fn volumes_until(&mut self, percent: u64) -> Vec<Value> {
        let mut seen = vec![];
        while let Some(level) = self.until(|l| l["event"] == "volume") {
            let done = level["percent"] == percent;
            seen.push(level);
            if done {
                return seen;
            }
        }
        panic!("never at {percent} %, after {:?}", percents(&seen));
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn level(percent: u8, muted: bool, output: &str) -> Value {
    json!({"event": "volume", "percent": percent, "muted": muted, "output": output})
}

fn percents(levels: &[Value]) -> Vec<u64> {
    levels.iter().map(|l| l["percent"].as_u64().unwrap()).collect()
}

#[test]
fn volume_keys_step_under_the_mute_unmute_going_up_and_name_the_output_picked() {
    let dir = tempfile::tempdir().unwrap();
    let pw = PipeWire::start(dir.path());
    pw.wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", "0.4"]);
    let mut w = Watcher::start(dir.path(), &pw);

    w.press("mute");
    assert_eq!(w.volume(), level(40, true, "Speakers"), "the level stays as the sound stops");
    assert_eq!(pw.level(), "Volume: 0.40 [MUTED]");
    for want in [38, 36, 34] {
        w.press("volume_down");
        assert_eq!(w.volume(), level(want, true, "Speakers"), "down moves the level and keeps the sound off");
    }
    assert_eq!(pw.level(), "Volume: 0.34 [MUTED]");
    w.press("volume_up");
    assert_eq!(w.volume(), level(36, false, "Speakers"), "up brings the sound back a step higher");
    assert_eq!(pw.level(), "Volume: 0.36");

    w.press("mute");
    assert_eq!(w.volume(), level(36, true, "Speakers"));
    let held = 12;
    for _ in 0..held {
        w.press("volume_up");
        std::thread::sleep(Duration::from_millis(10));
    }
    let climb = w.volumes_until(36 + u64::from(STEP) * held);
    assert!(climb.iter().all(|l| l["muted"] == false), "the first repeat unmutes: {climb:?}");
    assert!(percents(&climb).windows(2).all(|p| p[0] < p[1]), "a held key's levels only climb: {:?}", percents(&climb));
    assert_eq!(pw.level(), "Volume: 0.60", "every repeat landed");

    for _ in 0..40 {
        w.press("volume_down");
    }
    let fall = w.volumes_until(0);
    assert!(percents(&fall).windows(2).all(|p| p[0] > p[1]), "{:?}", percents(&fall));
    assert_eq!(fall.last().unwrap(), &level(0, false, "Speakers"), "down to nothing is not a mute");
    assert_eq!(pw.level(), "Volume: 0.00");

    pw.make_default("headphones");
    pw.wpctl(&["set-volume", "@DEFAULT_AUDIO_SINK@", "0.5"]);
    std::thread::sleep(PAUSE);
    w.press("volume_up");
    assert_eq!(w.volume(), level(52, false, "Headphones"), "the first press after a switch names the new output");
}
