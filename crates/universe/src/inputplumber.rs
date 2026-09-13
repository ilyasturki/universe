use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

const DAEMON: &str = "inputplumber";
const BUS_NAME: &str = "org.shadowblip.InputPlumber";

fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn wait_for(tries: u32, every: Duration, mut ready: impl FnMut() -> bool) -> bool {
    for _ in 0..tries {
        if ready() {
            return true;
        }
        std::thread::sleep(every);
    }
    ready()
}

pub fn installed() -> bool {
    crate::runners::on_path(DAEMON).is_some()
}

pub fn reachable() -> bool {
    run("busctl", &["--system", "--no-pager", "status", BUS_NAME])
}

fn composite_present() -> bool {
    output("busctl", &["--system", "--no-pager", "tree", BUS_NAME]).map(|t| t.contains("CompositeDevice")).unwrap_or(false)
}

fn hidden_present() -> bool {
    std::fs::read_dir(Path::new("/dev/inputplumber/by-hidden")).map(|rd| rd.flatten().next().is_some()).unwrap_or(false)
}

pub fn engage() -> bool {
    if !installed() {
        return false;
    }
    // A second manage-all --enable in one daemon lifetime builds a duplicate composite.
    run("systemctl", &["restart", "--no-ask-password", "inputplumber"]);
    if !wait_for(20, Duration::from_millis(250), || run(DAEMON, &["devices", "list"])) {
        tracing::warn!("inputplumber: the daemon did not come up");
        return false;
    }
    if !run(DAEMON, &["devices", "manage-all", "--enable"]) {
        tracing::warn!("inputplumber: manage-all --enable failed");
        return false;
    }
    // The first composite takes 7-9 s on a fresh daemon; the emulator's startup pad scan misses a later one.
    if !wait_for(30, Duration::from_millis(500), composite_present) {
        tracing::warn!("inputplumber: no composite device appeared; the pads may still be hidden");
    }
    true
}

// No restart: killed before udev restores permissions, a pad stays mode 000 until power-cycled.
// by-hidden empties only once udev has run, so that is the "usable again" probe.
pub fn release() {
    if !installed() {
        return;
    }
    run(DAEMON, &["devices", "manage-all"]);
    if !wait_for(20, Duration::from_millis(250), || !hidden_present()) {
        tracing::warn!("inputplumber: pads still hidden after release");
    }
}
