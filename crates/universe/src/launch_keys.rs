//! The launch keys: what `game.toml` and `config.toml`'s `[launch]` take, what the CLI and the settings rows derive from.

use serde::Serialize;

use crate::gamescope::{self, Mode};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int { max: Option<u32> },
    Str,
    Path,
    List,
    Enum(&'static [&'static str]),
    /// `auto` or `WxH`.
    Resolution,
    /// `auto` or Hz.
    Refresh,
    /// `auto`, `none` or frames per second.
    Fps,
    /// A `[proton]` name or a path; the frontend lists the names.
    Proton,
    /// Written as `<key>.<name> = value`; never a row.
    Map,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Game,
    Global,
    Both,
}

impl Scope {
    pub fn parse(s: &str) -> crate::Result<Scope> {
        match s {
            "game" => Ok(Scope::Game),
            "global" => Ok(Scope::Global),
            "both" => Ok(Scope::Both),
            other => Err(Error::Invalid(format!("scope must be game, global or both, not '{other}'"))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Scope::Game => "game",
            Scope::Global => "global",
            Scope::Both => "both",
        }
    }

    /// `Both` takes a key of either scope.
    fn takes(self, key: Scope) -> bool {
        self == key || self == Scope::Both || key == Scope::Both
    }
}

pub struct LaunchKey {
    /// Without the `launch.` prefix.
    pub key: &'static str,
    pub kind: Kind,
    /// The global default; a game's key left empty takes it.
    pub default: &'static str,
    pub label: &'static str,
    /// The settings card the row sits in; empty: the key is settable but has no row.
    pub section: &'static str,
    pub scope: Scope,
    /// Empty: every runner; else the runner kinds the key applies to.
    pub runners: &'static [&'static str],
    pub description: &'static str,
}

const WINE: &[&str] = &["proton", "wine"];
const PROTON: &[&str] = &["proton"];

macro_rules! key {
    ($key:literal, $kind:expr, $default:literal, $label:literal, $section:literal, $scope:ident, $runners:expr, $description:literal) => {
        LaunchKey { key: $key, kind: $kind, default: $default, label: $label, section: $section, scope: Scope::$scope, runners: $runners, description: $description }
    };
}

pub static LAUNCH_KEYS: &[LaunchKey] = &[
    key!("runner", Kind::Str, "", "Runner", "", Game, &[], "What starts the game: proton, wine, linux or an emulator."),
    key!("runner_exe", Kind::Path, "", "Runner program", "", Game, &[], "The runner's program, when not the detected one."),
    key!("exe", Kind::Path, "", "Program", "", Game, &[], "The program, or for an emulator the ROM, image or folder."),
    key!("proton", Kind::Proton, "proton-ge", "Proton", "Proton", Both, PROTON, "The Proton build umu-run starts the game with."),
    key!("esync", Kind::Bool, "true", "Esync", "Proton", Both, WINE, "Wine's eventfd synchronisation; faster, on for most games."),
    key!("fsync", Kind::Bool, "true", "Fsync", "Proton", Both, WINE, "Futex synchronisation; needs a kernel with futex2, faster than esync."),
    key!("ntsync", Kind::Bool, "true", "NTSync", "Proton", Both, PROTON, "The kernel's NT synchronisation driver; needs Linux 6.14 and a Proton built for it."),
    key!("wayland", Kind::Bool, "true", "Wayland", "Proton", Both, PROTON, "Proton's own Wayland driver instead of Xwayland; dropped inside gamescope unless it exposes Wayland."),
    key!("hdr", Kind::Bool, "false", "HDR", "Proton", Both, PROTON, "HDR output: Proton's PROTON_ENABLE_HDR, and --hdr-enabled on gamescope."),
    key!("dlss_upgrade", Kind::Bool, "false", "DLSS upgrade", "Proton", Both, PROTON, "Swap the game's DLSS for the latest the driver ships."),
    key!("fsr4_upgrade", Kind::Bool, "false", "FSR 4 upgrade", "Proton", Both, PROTON, "Swap the game's FSR for FSR 4; RDNA 4, or RDNA 3 with Proton's override."),
    key!("xess_upgrade", Kind::Bool, "false", "XeSS upgrade", "Proton", Both, PROTON, "Swap the game's XeSS for the latest."),
    key!("optiscaler", Kind::Bool, "false", "OptiScaler", "Proton", Both, PROTON, "Run OptiScaler: FSR 4 or XeSS where the game only offers DLSS."),
    key!("prefix", Kind::Path, "", "Wine prefix", "Proton", Game, WINE, "The Wine prefix the game runs in; empty: <prefixes_root>/<id>."),
    key!("arch", Kind::Enum(&["win64", "win32"]), "win64", "Architecture", "", Game, &["wine"], "WINEARCH for a plain Wine prefix."),
    key!("umu_id", Kind::Str, "", "umu id", "", Game, PROTON, "GAMEID for umu-run's protonfixes; empty: umu-default."),
    key!("store", Kind::Str, "", "Store", "", Game, PROTON, "STORE for umu-run's protonfixes."),
    key!("dll_overrides", Kind::Map, "", "DLL overrides", "", Game, WINE, "WINEDLLOVERRIDES, one key per DLL without .dll: d3d11 = \"n,b\"."),
    key!("env", Kind::Map, "", "Environment", "", Both, &[], "Environment variables for the game; a game's win over the global ones."),
    key!("mangohud", Kind::Bool, "true", "MangoHud", "Overlay and cursor", Both, &[], "Show MangoHud's overlay in the game."),
    key!("fps_limit", Kind::Fps, "auto", "Frame rate limit", "Overlay and cursor", Both, &[], "MangoHud holds the game to this many frames per second, overlay or not; auto is the refresh rate the game sees."),
    key!("pause_on_home", Kind::Bool, "false", "Pause on HOME", "Overlay and cursor", Both, &[], "Freeze the game while the home menu is up; it runs again when the menu closes."),
    key!("wrapper", Kind::Str, "", "Wrapper command", "Launch", Game, &[], "A command the game runs through, innermost: gamemoderun, taskset -c 0-7…"),
    key!("args", Kind::List, "", "Arguments", "Launch", Game, &[], "Arguments appended to the game's command line."),
    key!("working_dir", Kind::Path, "", "Working directory", "Launch", Game, &[], "The folder the game starts in."),
    key!("pre_command", Kind::Str, "", "Before the game", "", Game, &[], "A shell line run before the game starts."),
    key!("post_command", Kind::Str, "", "After the game", "", Game, &[], "A shell line run once the game has ended."),
    key!("gamescope", Kind::Bool, "true", "Gamescope", "Gamescope", Both, &[], "Run the game inside a gamescope of its own; a launcher already inside gamescope puts it on that one instead."),
    key!("gamescope_resolution", Kind::Resolution, "auto", "Resolution", "Gamescope", Both, &[], "What the game renders at; auto is the screen's own. Lower is upscaled to the screen."),
    key!("gamescope_refresh", Kind::Refresh, "auto", "Refresh rate", "Gamescope", Both, &[], "The refresh rate the game sees; auto is the screen's own."),
    key!("gamescope_scaler", Kind::Enum(&gamescope::SCALERS), "", "Scaler", "Gamescope", Both, &[], "How a smaller picture fills the screen: integer keeps pixels whole, fit keeps the aspect, fill and stretch do not."),
    key!("gamescope_filter", Kind::Enum(&gamescope::FILTERS), "", "Filter", "Gamescope", Both, &[], "The upscaling filter: fsr and nis sharpen, nearest and pixel keep pixel art crisp."),
    key!("gamescope_sharpness", Kind::Int { max: Some(gamescope::SHARPNESS_MAX) }, "", "Sharpness", "Gamescope", Both, &[], "For fsr and nis: 0 is sharpest, 20 softest."),
    key!("gamescope_adaptive_sync", Kind::Bool, "false", "Adaptive sync", "Gamescope", Both, &[], "Variable refresh rate when the screen supports it."),
    key!("gamescope_args", Kind::Str, "", "Arguments", "Gamescope", Both, &[], "Extra gamescope flags, after and over the fields above."),
    key!("gamescope_bin", Kind::Path, "gamescope", "Gamescope program", "", Global, &[], "The gamescope binary: a name on PATH or a path."),
    key!("umu_run", Kind::Path, "umu-run", "umu-run program", "", Global, &[], "The umu-run binary: a name on PATH or a path."),
    key!("options", Kind::Map, "", "Runner options", "", Game, &[], "The runner's options, validated against `universe runner options <id>`."),
];

pub fn find(key: &str) -> Option<&'static LaunchKey> {
    LAUNCH_KEYS.iter().find(|k| k.key == key)
}

pub fn default_of(key: &str) -> &'static str {
    find(key).map(|k| k.default).unwrap_or("")
}

/// A `launch.*` key's value (`env.FOO` and `options.<key>` reach their map); empty always passes, it clears the key.
pub fn validate(scope: Scope, key: &str, value: &str) -> crate::Result<()> {
    let head = key.split('.').next().unwrap_or(key);
    let k = find(head).ok_or_else(|| Error::Invalid(format!("unknown launch key {key}")))?;
    if !scope.takes(k.scope) {
        return Err(Error::Invalid(format!("launch.{key} is a {} key", k.scope.as_str())));
    }
    if head != key && k.kind != Kind::Map {
        return Err(Error::Invalid(format!("unknown launch key {key}")));
    }
    if value.is_empty() {
        return Ok(());
    }
    match k.kind {
        Kind::Bool if !matches!(value, "true" | "false") => Err(Error::Invalid(format!("{key} must be true or false"))),
        Kind::Int { max } => match value.parse::<u32>() {
            Ok(n) if max.is_none_or(|m| n <= m) => Ok(()),
            _ => Err(Error::Invalid(match max {
                Some(m) => format!("{key} must be 0 to {m}"),
                None => format!("{key} must be a whole number"),
            })),
        },
        Kind::Enum(choices) if !choices.contains(&value) => Err(Error::Invalid(format!("{key} must be one of {}", choices.join(", ")))),
        Kind::Resolution => gamescope::parse_resolution(value).map(|_| ()),
        Kind::Refresh => gamescope::parse_refresh(value).map(|_| ()),
        Kind::Fps => crate::launcher::parse_fps_limit(value).map(|_| ()),
        _ => Ok(()),
    }
}

/// A settings row as the frontends read it; `default` typed as the key is.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Row {
    pub key: &'static str,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub default: serde_json::Value,
    pub choices: Vec<String>,
    pub label: &'static str,
    pub section: &'static str,
    pub scope: &'static str,
    pub runners: &'static [&'static str],
    pub description: &'static str,
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Bool => "bool",
        Kind::Int { .. } => "int",
        Kind::Str => "string",
        Kind::Path => "path",
        Kind::List => "list",
        Kind::Enum(_) => "enum",
        Kind::Resolution => "resolution",
        Kind::Refresh => "refresh",
        Kind::Fps => "fps",
        Kind::Proton => "proton",
        Kind::Map => "map",
    }
}

fn typed_default(k: &LaunchKey) -> serde_json::Value {
    match k.kind {
        Kind::Bool => serde_json::Value::Bool(k.default == "true"),
        Kind::Int { .. } => k.default.parse::<u32>().map(serde_json::Value::from).unwrap_or(serde_json::Value::Null),
        _ => serde_json::Value::String(k.default.into()),
    }
}

/// Six steps over 0..=max: what a picker offers before a typed value.
fn int_steps(max: u32) -> Vec<String> {
    [0, max / 10, max / 4, max / 2, max * 3 / 4, max].iter().map(|n| n.to_string()).collect()
}

fn choices_of(k: &LaunchKey, screen: Option<Mode>) -> Vec<String> {
    match k.kind {
        Kind::Enum(choices) => choices.iter().map(|s| s.to_string()).collect(),
        Kind::Int { max: Some(max) } => int_steps(max),
        Kind::Resolution => gamescope::resolution_choices(screen),
        Kind::Refresh => gamescope::refresh_choices(screen),
        Kind::Fps => gamescope::fps_limit_choices(screen),
        _ => Vec::new(),
    }
}

pub fn rows(scope: Scope, screen: Option<Mode>) -> Vec<Row> {
    LAUNCH_KEYS
        .iter()
        .filter(|k| !k.section.is_empty() && k.kind != Kind::Map && scope.takes(k.scope))
        .map(|k| Row {
            key: k.key,
            kind: kind_name(k.kind),
            default: typed_default(k),
            choices: choices_of(k, screen),
            label: k.label,
            section: k.section,
            scope: k.scope.as_str(),
            runners: k.runners,
            description: k.description,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(v: &serde_json::Value) -> String {
        match v {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(a) if a.is_empty() => String::new(),
            serde_json::Value::Object(o) if o.is_empty() => String::new(),
            other => other.to_string(),
        }
    }

    #[test]
    fn the_table_and_the_two_structs_agree() {
        let game = serde_json::to_value(crate::game::Launch::default()).unwrap();
        let global = serde_json::to_value(crate::config::LaunchDefaults::default()).unwrap();
        for (name, value) in game.as_object().unwrap() {
            let k = find(name).unwrap_or_else(|| panic!("game::Launch::{name} is not in the table"));
            assert!(Scope::Game.takes(k.scope), "{name} is not a game key");
            if k.scope == Scope::Game {
                assert_eq!(k.default, rendered(value), "{name}: the table's default is not game::Launch's");
            }
        }
        for (name, value) in global.as_object().unwrap() {
            let k = find(name).unwrap_or_else(|| panic!("config::LaunchDefaults::{name} is not in the table"));
            assert!(Scope::Global.takes(k.scope), "{name} is not a global key");
            assert_eq!(k.default, rendered(value), "{name}: the table's default is not config::LaunchDefaults's");
        }
        for k in LAUNCH_KEYS {
            let in_game = game.get(k.key).is_some();
            let in_global = global.get(k.key).is_some();
            let expected = match k.scope {
                Scope::Game => (true, false),
                Scope::Global => (false, true),
                Scope::Both => (true, true),
            };
            assert_eq!((in_game, in_global), expected, "{}: scope {:?} does not match the structs", k.key, k.scope);
        }
        let keys: Vec<&str> = rows(Scope::Both, None).iter().map(|r| r.key).collect();
        assert!(!keys.contains(&"env") && !keys.contains(&"options") && !keys.contains(&"runner"), "maps and rowless keys stay out of rows()");
        assert!(rows(Scope::Game, None).iter().all(|r| r.scope != "global") && rows(Scope::Global, None).iter().all(|r| r.scope != "game"));
    }

    #[test]
    fn keys_validate() {
        assert!(validate(Scope::Game, "gamescope_scaler", "integer").is_ok());
        assert!(validate(Scope::Game, "gamescope_scaler", "bilinear").is_err());
        assert!(validate(Scope::Game, "gamescope_filter", "fsr").is_ok());
        assert!(validate(Scope::Game, "gamescope_filter", "").is_ok());
        assert!(validate(Scope::Global, "gamescope_sharpness", "20").is_ok());
        assert!(validate(Scope::Global, "gamescope_sharpness", "21").is_err());
        assert!(validate(Scope::Game, "fps_limit", "60").is_ok() && validate(Scope::Game, "fps_limit", "auto").is_ok() && validate(Scope::Game, "fps_limit", "none").is_ok());
        assert!(validate(Scope::Game, "fps_limit", "sixty").is_err() && validate(Scope::Game, "fps_limit", "0").is_err());
        assert!(validate(Scope::Game, "gamescope_resolution", "1920x1080").is_ok() && validate(Scope::Game, "gamescope_resolution", "1080p").is_err());
        assert!(validate(Scope::Game, "gamescope_adaptive_sync", "yes").is_err());
        assert!(validate(Scope::Game, "gamescope_args", "anything -r 120").is_ok());
        assert!(validate(Scope::Game, "arch", "win32").is_ok() && validate(Scope::Game, "arch", "arm64").is_err());
        assert!(validate(Scope::Game, "env.FOO", "bar").is_ok() && validate(Scope::Game, "options.batch", "false").is_ok());
        assert!(validate(Scope::Game, "proton.x", "y").is_err(), "only a map takes a dotted key");
        assert!(validate(Scope::Global, "protn", "x").is_err(), "an unknown key is refused");
        assert!(validate(Scope::Game, "umu_run", "").is_err(), "a global-only key is refused on a game");
        assert!(validate(Scope::Global, "prefix", "/p").is_err(), "a game-only key is refused globally");
        assert!(validate(Scope::Both, "umu_run", "umu-run").is_ok() && validate(Scope::Both, "prefix", "/p").is_ok());
    }

    #[test]
    fn rows_carry_the_screen() {
        let screen = Some(Mode { width: 2560, height: 1440, refresh: 144 });
        let by_key = |scope, screen| -> std::collections::BTreeMap<&str, Row> { rows(scope, screen).into_iter().map(|r| (r.key, r)).collect() };
        let with = by_key(Scope::Global, screen);
        assert_eq!(with["gamescope_resolution"].choices, ["auto", "2560x1440", "1920x1080", "1280x720"]);
        assert_eq!(with["gamescope_refresh"].choices[..3], ["auto", "144", "120"]);
        assert_eq!(with["fps_limit"].choices[..3], ["auto", "none", "144"]);
        assert_eq!(with["gamescope_sharpness"].choices, ["0", "2", "5", "10", "15", "20"]);
        assert_eq!((with["gamescope"].kind, with["gamescope"].default.clone()), ("bool", serde_json::Value::Bool(true)));
        assert_eq!(with["gamescope_sharpness"].default, serde_json::Value::Null);
        let without = by_key(Scope::Game, None);
        assert_eq!(without["gamescope_resolution"].choices, ["auto", "1920x1080", "1280x720"]);
        assert!(without.contains_key("prefix") && !without.contains_key("gamescope_bin") && !with.contains_key("prefix"));
        assert_eq!(without["esync"].runners, ["proton", "wine"]);
    }
}
