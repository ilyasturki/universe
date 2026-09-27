use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};
use serde_json::Value;

/// `universe controller watch --json --wait` as a child of this process: its events land on the main loop, commands go
/// down its stdin. Dropping it closes that stdin, which ends the watcher.
#[derive(Debug)]
pub struct Watcher {
    process: gio::Subprocess,
    stdin: gio::OutputStream,
    stopped: Rc<Cell<bool>>,
}

impl Watcher {
    /// `on_event` hears every line, and `{"event": "off"}` when the watcher ends on its own.
    pub fn start(on_event: impl Fn(Value) + 'static) -> Option<Watcher> {
        let program =
            std::env::var_os("UNIVERSE_BIN").filter(|p| !p.is_empty()).map(std::path::PathBuf::from).or_else(|| glib::find_program_in_path("universe"))?;
        let argv: [&std::ffi::OsStr; 5] = [program.as_os_str(), "controller".as_ref(), "watch".as_ref(), "--json".as_ref(), "--wait".as_ref()];
        let process = gio::Subprocess::newv(&argv, gio::SubprocessFlags::STDIN_PIPE | gio::SubprocessFlags::STDOUT_PIPE)
            .map_err(|e| tracing::warn!("controller watcher: {e}"))
            .ok()?;
        let (stdin, stdout) = (process.stdin_pipe()?, process.stdout_pipe()?);
        let reader = gio::DataInputStream::new(&stdout);
        let stopped = Rc::new(Cell::new(false));
        let quiet = stopped.clone();
        glib::spawn_future_local(async move {
            while let Ok(Some(line)) = reader.read_line_utf8_future(glib::Priority::DEFAULT).await {
                if let Ok(event) = serde_json::from_str::<Value>(&line) {
                    on_event(event);
                }
            }
            if !quiet.get() {
                on_event(serde_json::json!({"event": "off"}));
            }
        });
        Some(Watcher { process, stdin, stopped })
    }

    pub fn send(&self, command: Value) {
        let line = format!("{command}\n");
        if let Err(e) = self.stdin.write_all(line.as_bytes(), gio::Cancellable::NONE) {
            tracing::debug!("controller watcher: {e}");
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stopped.set(true);
        let _ = self.stdin.close(gio::Cancellable::NONE);
        let process = self.process.clone();
        glib::spawn_future_local(async move {
            let ended = glib::future_with_timeout(Duration::from_secs(2), process.wait_future()).await;
            if !matches!(ended, Ok(Ok(()))) {
                process.force_exit();
            }
        });
    }
}
