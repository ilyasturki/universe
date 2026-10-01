use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What the window remembers between runs, beside the core's state: `$XDG_STATE_HOME/universe/desktop.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub width: i32,
    pub height: i32,
    pub maximized: bool,
    /// The sidebar's pick: `all`, `media`, `store`, `source:<kind>` or `platform:<name>`.
    pub view: String,
    pub sort: String,
    pub show_hidden: bool,
    /// Only read: the first-run flag from before the core kept one for every frontend, carried over to it.
    pub onboarded: bool,
    /// The pad whose buttons Preferences › Controller shows while none is connected.
    pub controller_family: String,
}

impl Default for State {
    fn default() -> State {
        State {
            width: 1180,
            height: 760,
            maximized: false,
            view: "all".into(),
            sort: "last-played".into(),
            show_hidden: false,
            onboarded: false,
            controller_family: String::new(),
        }
    }
}

fn path() -> PathBuf {
    universe::paths::state_home().join("desktop.json")
}

impl State {
    pub fn load() -> State {
        std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let file = path();
        let tmp = file.with_extension("json.tmp");
        let written = file.parent().map(std::fs::create_dir_all).transpose().and_then(|_| {
            std::fs::write(&tmp, serde_json::to_string_pretty(self).unwrap_or_default())?;
            std::fs::rename(&tmp, &file)
        });
        if let Err(e) = written {
            tracing::warn!("{}: {e}", file.display());
        }
    }
}
