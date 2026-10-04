use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::core::Core;
use crate::gamescope::Mode;
use crate::launch_keys::{self, Scope};
use crate::modules::Applies;
use crate::runners::{self, Kind, RunnerSpec};
use crate::{Error, Result};

/// What gamescope does with a scaling key left unset.
const GAMESCOPE_DEFAULTS: [(&str, &str); 3] = [("gamescope_scaler", "auto"), ("gamescope_filter", "linear"), ("gamescope_sharpness", "2")];
/// Game keys whose empty value the launch fills in itself: they fall back to what `effective` says.
const COMPUTED: [&str; 2] = ["working_dir", "prefix"];
const NULL: Value = Value::Null;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Form {
    Launch,
    Runner(String),
    Game(String),
    Module(String),
    Source(String),
}

impl Form {
    pub fn parse(kind: &str, id: &str) -> Result<Form> {
        match kind {
            "launch" => Ok(Form::Launch),
            "runner" => Ok(Form::Runner(id.into())),
            "game" => Ok(Form::Game(id.into())),
            "module" => Ok(Form::Module(id.into())),
            "source" => Ok(Form::Source(id.into())),
            other => Err(Error::Invalid(format!("form: launch, runner, game, module or source, not '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Game,
    Runner,
    Global,
    Default,
    /// A runner's program found on PATH, set nowhere.
    Found,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

impl Choice {
    fn same(value: &str) -> Choice {
        Choice { value: value.into(), label: value.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub value: String,
    pub origin: Origin,
    pub resettable: bool,
    pub promotable: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Field {
    /// What `set_field` takes on this form; a map's entry is `<key>.<name>`.
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub section: String,
    pub advanced: bool,
    pub description: String,
    /// What the form's own scope sets, as `set_field` writes it; empty when it sets nothing.
    pub own: String,
    /// What applies: `own`, else `inherited`.
    pub value: String,
    pub inherited: String,
    /// Where `value` comes from; none when the key has nothing to fall back to.
    pub origin: Option<Origin>,
    /// Where `inherited` comes from, whether or not `own` hides it.
    pub fallback: Option<Origin>,
    pub resettable: bool,
    /// A game's own value with a global twin: `promote_field` makes it every game's.
    pub promotable: bool,
    pub choices: Vec<Choice>,
    /// A value outside `choices` may be typed.
    pub free: bool,
    /// The choices come from the module or source at run time: `module_setting_choices`, `source_setting_choices`.
    pub dynamic: bool,
    pub required: bool,
    /// What an `auto`, or a scaling key left unset, comes to on this machine: `144`, `2560x1440`, `on`, `linear`.
    pub resolved: String,
    /// The same for `inherited`: what a reset would come to.
    pub inherited_resolved: String,
    /// Whether this GPU gains anything from the upscaler the key turns on; none for other keys or an unknown GPU.
    pub fits: Option<bool>,
    pub entries: Vec<Entry>,
    /// What a search also finds a module's or a source's setting by.
    pub keywords: Vec<String>,
}

impl Field {
    fn new(key: impl Into<String>, label: impl Into<String>, kind: &str, section: &str) -> Field {
        Field { key: key.into(), label: label.into(), kind: kind.into(), section: section.into(), ..Field::default() }
    }

    fn about(mut self, description: &str, advanced: bool) -> Field {
        self.description = description.into();
        self.advanced = advanced;
        self
    }

    fn plain(mut self, value: String) -> Field {
        self.own = value.clone();
        self.value = value;
        self
    }

    fn inherits(mut self, own: String, own_origin: Origin, inherited: String, fallback: Origin) -> Field {
        self.origin = Some(if own.is_empty() { fallback } else { own_origin });
        self.fallback = Some(fallback);
        self.resettable = !own.is_empty();
        self.value = if own.is_empty() { inherited.clone() } else { own.clone() };
        self.own = own;
        self.inherited = inherited;
        self
    }
}

/// What a form is built from, in the shape of the views a frontend reads: `set` and `protons` as `settings()` has them,
/// `game` as `get()` (a game's form only), then `modules()`, `sources()`, `runners()` and `gpu()`. The core gathers its
/// own (`Core::form_inputs`); a stand-in core hands over its fixture's, so its forms are the core's.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Inputs {
    pub set: Value,
    pub protons: Vec<String>,
    pub game: Value,
    pub modules: Vec<Value>,
    pub sources: Vec<Value>,
    pub runners: Vec<Value>,
    pub gpu: Value,
    pub under_steam: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Owner {
    id: String,
    name: String,
    enabled: bool,
    applies: Applies,
    settings: Vec<Setting>,
}

impl Owner {
    fn name(&self) -> &str {
        if self.name.is_empty() {
            &self.id
        } else {
            &self.name
        }
    }
}

/// A setting as `modules::setting_json` lists it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Setting {
    key: String,
    #[serde(rename = "type")]
    kind: String,
    default: Value,
    label: String,
    description: String,
    scope: String,
    choices: Vec<String>,
    choice_labels: BTreeMap<String, String>,
    dynamic: bool,
    advanced: bool,
    required: bool,
    keywords: Vec<String>,
    runners: Vec<String>,
    platforms: Vec<String>,
}

impl Setting {
    fn applies_to(&self, runner: &str, platform: &str) -> bool {
        crate::modules::applies_to(&self.runners, &self.platforms, runner, platform)
    }
}

/// A view's nulls dropped: a field with a default takes a missing value, not a null one.
fn lenient<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T> {
    fn strip(v: &Value) -> Value {
        match v {
            Value::Object(o) => Value::Object(o.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k.clone(), strip(v))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(strip).collect()),
            other => other.clone(),
        }
    }
    Ok(serde_json::from_value(strip(v))?)
}

/// A map entry's key, the name cleaned to one table key; `None` when nothing is left of it.
pub fn entry_key(map: &str, name: &str) -> Option<String> {
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-')).collect();
    (!name.is_empty()).then(|| format!("{map}.{name}"))
}

fn render(v: &Value) -> String {
    match v {
        Value::Null | Value::Object(_) => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(render).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(","),
    }
}

fn dig<'a>(v: &'a Value, dotted: &str) -> &'a Value {
    dotted.split('.').fold(v, |node, part| node.get(part).unwrap_or(&NULL))
}

/// A toggle may be written as a bool.
fn as_kind(kind: &str, value: String) -> String {
    match (kind, value.as_str()) {
        ("toggle", "true") => "on".into(),
        ("toggle", "false") => "off".into(),
        _ => value,
    }
}

struct ConfigKey {
    key: &'static str,
    kind: &'static str,
    label: &'static str,
    section: &'static str,
    choices: &'static [&'static str],
    description: &'static str,
    advanced: bool,
}

const fn config_key(section: &'static str, key: &'static str, kind: &'static str, label: &'static str, description: &'static str) -> ConfigKey {
    ConfigKey { key, kind, label, section, choices: &[], description, advanced: true }
}

const CONFIG_KEYS: &[ConfigKey] = &[
    ConfigKey { advanced: false, ..config_key("Overlay", "desktop.hide_cursor", "bool", "Hide the cursor while playing", "Hide the desktop cursor while the game runs.") },
    ConfigKey {
        advanced: false,
        ..config_key(
            "Overlay",
            "desktop.keep_awake",
            "bool",
            "Keep the screen awake",
            "Hold off the desktop's blanking and automatic suspend while a game runs: a pad is no activity to it.",
        )
    },
    config_key("Folders", "paths.games_root", "path", "Games", "Where sources install games."),
    config_key("Folders", "paths.prefixes_root", "path", "Wine prefixes", "Where a game's prefix is made when it names none."),
    config_key("Folders", "paths.saves_root", "path", "Save backups", "Where each game's save backups are kept."),
    config_key("Folders", "paths.recordings_root", "path", "Recordings", "Where the capture module files its videos."),
    ConfigKey {
        advanced: false,
        ..config_key("Saves", "saves.auto_backup", "bool", "Back up after playing", "Back up a game's saves each time a session ends, through ludusavi.")
    },
    config_key("Saves", "saves.keep", "int", "Backups kept", "How many backups each game keeps, the oldest going first."),
    config_key(
        "API keys",
        "keys.sgdb",
        "secret",
        "SteamGridDB key (optional)",
        "Your own key from steamgriddb.com adds its community art to the picker, and square art. Without one, art comes from the stores, GOG GamesDB and libretro.",
    ),
    config_key("API keys", "keys.sgdb_file", "path", "SteamGridDB key file", "A file holding the key, read when the key above is empty."),
    config_key("API keys", "keys.prefer_sgdb", "bool", "Prefer SteamGridDB art", "With a key, SteamGridDB's art fills a slot before the stores' and GamesDB's."),
    ConfigKey {
        choices: &["auto", "gnome", "kde", "cinnamon", "sway", "hyprland", "niri", "x11", "none"],
        ..config_key(
            "Desktop",
            "desktop.profile",
            "enum",
            "Desktop",
            "The desktop the launcher focuses windows, shows its OSD, takes screenshots and hides the cursor on: auto detects it, none skips all of that.",
        )
    },
    config_key(
        "Desktop",
        "desktop.cursor_extension",
        "string",
        "Cursor extension",
        "Empty: the Universe extension hides the resting cursor. Else another GNOME Shell extension toggled to hide it, restored to its prior state after the session.",
    ),
    ConfigKey {
        advanced: false,
        ..config_key(
            "Updates",
            "desktop.whats_new",
            "bool",
            "Show what's new after an update",
            "The first start after an update opens the changes since the version you last ran.",
        )
    },
];

const PROTON_BUILDS: &str = "A Proton build by name, for the Proton choices: the folder holding its `proton` script.";

struct Machine<'a> {
    mode: Option<Mode>,
    /// `gpu()`: `auto` is what an upgrade left on auto turns into, `fits` whether it gains anything.
    gpu: &'a Value,
}

impl Machine<'_> {
    fn gpu_says(&self, table: &str, key: &str) -> Option<bool> {
        self.gpu[table][key].as_bool()
    }

    /// `gamescope` and `refresh` are what the game gets: an `auto` frame limit follows the refresh gamescope is given.
    fn resolve(&self, key: &str, kind: &str, value: &str, gamescope: bool, refresh: &str) -> String {
        let mode = self.mode.unwrap_or_default();
        let known = |n: u32| if n > 0 { n.to_string() } else { String::new() };
        let word = |on: bool| if on { "on" } else { "off" }.to_string();
        match (value, kind) {
            ("auto", "fps") if gamescope && refresh.parse::<u32>().is_ok() => refresh.into(),
            ("auto", "fps") | ("auto", "refresh") => known(mode.refresh),
            ("auto", "resolution") if mode.width > 0 => format!("{}x{}", mode.width, mode.height),
            ("auto", "toggle") if key == "gamescope_adaptive_sync" => self.mode.map(|m| word(m.vrr)).unwrap_or_default(),
            ("auto", "toggle") => self.gpu_says("auto", key).map(word).unwrap_or_default(),
            ("", _) => GAMESCOPE_DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string()).unwrap_or_default(),
            _ => String::new(),
        }
    }
}

fn entries(map: &Value, origin: impl Fn(&str) -> Origin) -> Vec<Entry> {
    map.as_object()
        .into_iter()
        .flatten()
        .map(|(name, v)| {
            let origin = origin(name);
            Entry { name: name.clone(), value: render(v), origin, resettable: origin != Origin::Global, promotable: false }
        })
        .collect()
}

fn setting_field(s: &Setting, key: String, section: &str) -> Field {
    let mut f = Field::new(key, if s.label.is_empty() { &s.key } else { &s.label }, &s.kind, section).about(&s.description, s.advanced || s.scope == "config");
    f.choices = s.choices.iter().map(|c| Choice { value: c.clone(), label: s.choice_labels.get(c).unwrap_or(c).clone() }).collect();
    f.dynamic = s.dynamic;
    f.required = s.required;
    f.keywords = s.keywords.clone();
    f
}

fn owners(list: &[Value]) -> Result<Vec<Owner>> {
    list.iter().map(lenient).collect()
}

/// Where a game's key is set for every game.
enum Twin<'a> {
    Config,
    Runner(&'a str),
    Module(&'a str, &'a str),
    Source(&'a str, &'a str),
}

fn twin(key: &str) -> Option<Twin<'_>> {
    match key.splitn(3, '.').collect::<Vec<_>>().as_slice() {
        ["desktop", "hide_cursor"] => Some(Twin::Config),
        ["launch", "options", option] => Some(Twin::Runner(option)),
        ["launch", head, ..] => launch_keys::find(head).filter(|k| k.scope == Scope::Both).map(|_| Twin::Config),
        // A game's `enabled` is not the module's own switch.
        ["modules", module, setting] if *setting != "enabled" => Some(Twin::Module(module, setting)),
        ["sources", source, setting] => Some(Twin::Source(source, setting)),
        _ => None,
    }
}

fn table_value(table: Option<&toml::Table>, key: &str) -> String {
    table.and_then(|t| t.get(key)).map(|v| render(&crate::modules::toml_to_json(v))).unwrap_or_default()
}

/// The fields of one settings form, in form order; `screen` sizes the resolution, refresh and frame rate choices and says what `auto` comes to.
pub fn fields(form: &Form, inputs: &Inputs, screen: Option<Mode>) -> Result<Vec<Field>> {
    let build = Build { inputs, config: lenient(&inputs.set)?, machine: Machine { mode: screen, gpu: &inputs.gpu } };
    match form {
        Form::Launch => Ok(build.launch_form()),
        Form::Runner(id) => build.runner_form(id),
        Form::Game(id) => build.game_form(id),
        Form::Module(id) => build.module_form(id),
        Form::Source(id) => build.source_form(id),
    }
}

/// What a field's change writes, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum Write {
    Config {
        key: String,
        value: String,
    },
    Runner {
        runner: String,
        key: String,
        value: String,
    },
    Game {
        game: String,
        key: String,
        value: String,
    },
    /// A module's global setting, config.toml's `[modules.<id>]`.
    Module {
        module: String,
        key: String,
        value: String,
    },
    Source {
        source: String,
        key: String,
        value: String,
    },
    EnableModule {
        module: String,
        on: bool,
    },
    EnableSource {
        source: String,
        on: bool,
    },
}

/// `value` on the form's own scope; an empty one clears it, so the field falls back to what it inherits.
pub fn set_writes(form: &Form, key: &str, value: &str) -> Vec<Write> {
    let (key, value) = (key.to_string(), value.to_string());
    vec![match form {
        Form::Launch => Write::Config { key, value },
        Form::Runner(_) if key.starts_with("launch.") => Write::Config { key, value },
        Form::Runner(id) => Write::Runner { runner: id.clone(), key, value },
        Form::Game(id) => Write::Game { game: id.clone(), key, value },
        Form::Module(id) if key == "enabled" => Write::EnableModule { module: id.clone(), on: value == "true" },
        Form::Module(id) => Write::Module { module: id.clone(), key, value },
        Form::Source(id) if key == "enabled" => Write::EnableSource { source: id.clone(), on: value == "true" },
        Form::Source(id) => Write::Source { source: id.clone(), key, value },
    }]
}

/// A game's own value of `key` becomes the global one, then leaves the game, which follows it; other games keep theirs.
/// Global first: a failed clear leaves the game a redundant value, never a lost one.
pub fn promote_writes(form: &Form, key: &str, inputs: &Inputs) -> Result<Vec<Write>> {
    let Form::Game(id) = form else { return Err(Error::Invalid(format!("{key} is not a game's"))) };
    let twin = twin(key).ok_or_else(|| Error::Invalid(format!("{key} has no global setting")))?;
    if inputs.game["id"].as_str() != Some(id) {
        return Err(Error::NotFound(id.clone()));
    }
    let value = render(dig(&inputs.game, key));
    if value.is_empty() {
        return Err(Error::Invalid(format!("{id} sets no {key} of its own")));
    }
    let global = match twin {
        Twin::Config => Write::Config { key: key.into(), value },
        Twin::Runner(option) => Write::Runner { runner: render(&inputs.game["effective"]["runner"]), key: option.into(), value },
        Twin::Module(module, setting) => Write::Module { module: module.into(), key: setting.into(), value },
        Twin::Source(source, setting) => Write::Source { source: source.into(), key: setting.into(), value },
    };
    Ok(vec![global, Write::Game { game: id.clone(), key: key.into(), value: String::new() }])
}

struct Build<'a> {
    inputs: &'a Inputs,
    config: Config,
    machine: Machine<'a>,
}

impl Build<'_> {
    fn set(&self) -> &Value {
        &self.inputs.set
    }

    fn launch_rows(&self, scope: Scope) -> Vec<launch_keys::Row> {
        let mut rows = launch_keys::rows(scope, self.machine.mode);
        if self.inputs.under_steam {
            rows.retain(|r| !launch_keys::STEAM_OWNS.contains(&r.key));
        }
        rows
    }

    /// The runner's program as `runners()` found it, and where from: `config`, `path`, `universe`.
    fn found(&self, spec: &RunnerSpec) -> (String, String) {
        let runner = self.inputs.runners.iter().find(|r| r["id"] == spec.id).unwrap_or(&NULL);
        (render(&runner["path"]), render(&runner["source"]))
    }

    fn source_default(&self, s: &Setting) -> Value {
        crate::sources::default_value(&s.key, &s.default, &self.config)
    }

    fn launch_field(&self, row: &launch_keys::Row) -> Field {
        let proton = &self.config.launch.proton;
        let mut f = Field::new(format!("launch.{}", row.key), row.label, row.kind, row.section).about(row.description, row.advanced);
        f.choices = if row.kind == "proton" {
            let mut names = self.inputs.protons.clone();
            if !proton.is_empty() && !names.contains(proton) {
                names.insert(0, proton.clone());
            }
            names.iter().map(|n| Choice::same(n)).collect()
        } else {
            row.choices.iter().map(|c| Choice::same(c)).collect()
        };
        f.free = matches!(row.kind, "resolution" | "refresh" | "fps" | "int" | "proton");
        f
    }

    fn global_launch_field(&self, row: &launch_keys::Row) -> Field {
        let key = format!("launch.{}", row.key);
        let mut f = self.launch_field(row);
        if row.kind == "map" {
            f.entries =
                entries(dig(self.set(), &key), |_| Origin::Default).into_iter().map(|e| Entry { origin: Origin::Global, resettable: true, ..e }).collect();
            return f;
        }
        let own = as_kind(row.kind, render(dig(self.set(), &key)));
        f = f.inherits(own, Origin::Global, render(&row.default), Origin::Default);
        let (gamescope, refresh) = (self.config.launch.gamescope, &self.config.launch.gamescope_refresh);
        f.resolved = self.machine.resolve(row.key, row.kind, &f.value, gamescope, refresh);
        f.inherited_resolved = self.machine.resolve(row.key, row.kind, &f.inherited, gamescope, refresh);
        f.fits = self.machine.gpu_says("fits", row.key);
        f
    }

    fn launch_form(&self) -> Vec<Field> {
        let mut out: Vec<Field> = self.launch_rows(Scope::Global).iter().filter(|r| r.runners.is_empty()).map(|r| self.global_launch_field(r)).collect();
        let defaults = serde_json::to_value(Config::default()).unwrap_or_default();
        for k in CONFIG_KEYS {
            let mut f = Field::new(k.key, k.label, k.kind, k.section).about(k.description, k.advanced);
            f.choices = k.choices.iter().map(|c| Choice::same(c)).collect();
            out.push(f.inherits(render(dig(self.set(), k.key)), Origin::Global, render(dig(&defaults, k.key)), Origin::Default));
        }
        let mut builds = Field::new("proton", "Proton builds", "map", "Proton builds").about(PROTON_BUILDS, true);
        builds.entries = entries(dig(self.set(), "proton"), |_| Origin::Global);
        out.push(builds);
        out
    }

    fn runner_form(&self, id: &str) -> Result<Vec<Field>> {
        let config = &self.config;
        let spec = runners::spec(id).ok_or_else(|| Error::NotFound(format!("runner {id}")))?;
        let own = config.runners.get(spec.id);
        let mut out = Vec::new();
        if spec.kind != Kind::Linux {
            let (program, source) = self.found(spec);
            let exe = table_value(own, "exe");
            let found = if source == "config" { String::new() } else { program };
            let f = Field::new("exe", "Program", "path", "Runner");
            out.push(if exe.is_empty() && found.is_empty() { f } else { f.inherits(exe, Origin::Runner, found, Origin::Found) });
            out.push(Field::new("args", "Arguments", "string", "Runner").plain(shell_words::join(runners::global_args(spec, config))));
        }
        let global = if dig(self.set(), "launch.gamescope").is_null() { Origin::Default } else { Origin::Global };
        out.push(Field::new("gamescope", "Gamescope", "bool", "Runner").inherits(
            table_value(own, "gamescope"),
            Origin::Runner,
            config.launch.gamescope.to_string(),
            global,
        ));
        let kind = spec.kind.as_str();
        out.extend(self.launch_rows(Scope::Global).iter().filter(|r| r.runners.contains(&kind)).map(|r| {
            let f = self.global_launch_field(r);
            if r.key == "proton" {
                Field { section: "Builds".into(), ..f }
            } else {
                f
            }
        }));
        for o in spec.options {
            let f = Field::new(o.key, o.label, o.kind, "Options");
            out.push(f.inherits(table_value(own, o.key), Origin::Runner, o.default.into(), Origin::Default));
        }
        Ok(out)
    }

    fn game_form(&self, id: &str) -> Result<Vec<Field>> {
        let (game, set, config, machine) = (&self.inputs.game, self.set(), &self.config, &self.machine);
        if game["id"].as_str() != Some(id) {
            return Err(Error::NotFound(id.into()));
        }
        let effective = &game["effective"];
        let global = serde_json::to_value(&config.launch)?;
        let runner = render(&effective["runner"]);
        let spec = runners::spec(&runner).or_else(|| runners::spec("proton")).ok_or_else(|| Error::NotFound(format!("runner {runner}")))?;
        let emulator = spec.kind == Kind::Emulator;
        let platform = render(&effective["platform"]);
        let mut out = Vec::new();

        let mut picker = Field::new("launch.runner", "Runner", "enum", "Launch").plain(spec.id.into());
        picker.choices = runners::RUNNERS.iter().map(|s| Choice { value: s.id.into(), label: s.name.into() }).collect();
        out.push(picker);
        out.push(Field::new("launch.exe", if emulator { "File" } else { "Program" }, "path", "Launch").plain(render(dig(game, "launch.exe"))));
        if emulator && spec.platforms.len() > 1 {
            let mut f = Field::new("platform", "Platform", "enum", "Launch").plain(platform.clone());
            f.choices = spec.platforms.iter().map(|p| Choice::same(p)).collect();
            out.push(f);
        }
        let runner_own = config.runners.get(spec.id);
        if emulator {
            let (program, source) = self.found(spec);
            let fallback = if source == "config" { Origin::Runner } else { Origin::Found };
            let f = Field::new("launch.runner_exe", format!("{} program", spec.name), "path", "Launch");
            let own = render(dig(game, "launch.runner_exe"));
            out.push(if own.is_empty() && program.is_empty() { f } else { f.inherits(own, Origin::Game, program, fallback) });
        }
        for o in spec.options {
            let key = format!("launch.options.{}", o.key);
            let from_runner = table_value(runner_own, o.key);
            let (inherited, fallback) = if from_runner.is_empty() { (o.default.to_string(), Origin::Default) } else { (from_runner, Origin::Runner) };
            out.push(Field::new(&key, o.label, o.kind, "Launch").inherits(render(dig(game, &key)), Origin::Game, inherited, fallback));
        }

        let kind = spec.kind.as_str();
        for row in self.launch_rows(Scope::Game).iter().filter(|r| r.runners.is_empty() || r.runners.contains(&kind)) {
            let key = format!("launch.{}", row.key);
            let own_value = dig(game, &key);
            let mut f = self.launch_field(row);
            if row.kind == "map" {
                let own = own_value.as_object().cloned().unwrap_or_default();
                let mut merged = if row.scope == "both" { dig(set, &key).as_object().cloned().unwrap_or_default() } else { Default::default() };
                merged.extend(own.clone());
                f.entries = entries(&Value::Object(merged), |name| if own.contains_key(name) { Origin::Game } else { Origin::Global });
                out.push(f);
                continue;
            }
            let own = as_kind(row.kind, render(own_value));
            if row.scope == "both" {
                let fallback = if render(dig(set, &key)).is_empty() { Origin::Default } else { Origin::Global };
                f = f.inherits(own, Origin::Game, as_kind(row.kind, render(&global[row.key])), fallback);
                let gamescope = effective["gamescope"].as_bool().unwrap_or_default();
                let refresh = render(&effective["gamescope_refresh"]);
                f.resolved = machine.resolve(row.key, row.kind, &f.value, gamescope, &refresh);
                f.inherited_resolved = machine.resolve(row.key, row.kind, &f.inherited, gamescope, &refresh);
                f.fits = machine.gpu_says("fits", row.key);
            } else if COMPUTED.contains(&row.key) {
                f = f.inherits(own, Origin::Game, render(&effective[row.key]), Origin::Default);
            } else {
                f = f.plain(own);
            }
            out.push(f);
        }

        let section = "Desktop and library";
        let fallback = if dig(set, "desktop.hide_cursor").is_null() { Origin::Default } else { Origin::Global };
        out.push(
            Field::new("desktop.hide_cursor", "Hide the cursor while playing", "bool", section)
                .about("Hide the desktop cursor while the game runs.", false)
                .inherits(render(dig(game, "desktop.hide_cursor")), Origin::Game, config.desktop.hide_cursor.to_string(), fallback),
        );
        let flag = |key: &str| game[key].as_bool().unwrap_or_default().to_string();
        out.push(Field::new("favorite", "Favourite", "bool", section).plain(flag("favorite")));
        out.push(Field::new("hidden", "Hidden", "bool", section).plain(flag("hidden")));
        out.push(Field::new("tags", "Tags", "list", section).plain(render(&game["tags"])));
        for (key, label) in [("metadata.steam_appid", "Steam app id"), ("metadata.gamesdb_id", "GOG GamesDB id"), ("metadata.sgdb_id", "SteamGridDB id")] {
            let n = dig(game, key).as_u64().unwrap_or_default();
            out.push(Field::new(key, label, "int", "Artwork").about("", true).plain(if n == 0 { String::new() } else { n.to_string() }));
        }

        for m in owners(&self.inputs.modules)?.iter().filter(|m| m.enabled && m.applies.takes(kind)) {
            let global = config.modules.settings.get(&m.id);
            for s in m.settings.iter().filter(|s| s.scope == "game" && s.applies_to(spec.id, &platform)) {
                let key = format!("modules.{}.{}", m.id, s.key);
                let from_config = table_value(global, &s.key);
                let (inherited, fallback) = if from_config.is_empty() { (render(&s.default), Origin::Default) } else { (from_config, Origin::Global) };
                out.push(setting_field(s, key.clone(), m.name()).inherits(render(dig(game, &key)), Origin::Game, inherited, fallback));
            }
        }
        let source_kind = render(dig(game, "source.kind"));
        if let Some(source) = owners(&self.inputs.sources)?.into_iter().find(|s| s.id == source_kind && s.enabled) {
            let global = config.sources.settings.get(&source.id);
            for s in source.settings.iter().filter(|s| s.scope == "game" && s.applies_to(spec.id, &platform)) {
                let key = format!("sources.{}.{}", source.id, s.key);
                let from_config = table_value(global, &s.key);
                let (inherited, fallback) =
                    if from_config.is_empty() { (render(&self.source_default(s)), Origin::Default) } else { (from_config, Origin::Global) };
                out.push(setting_field(s, key.clone(), source.name()).inherits(render(dig(game, &key)), Origin::Game, inherited, fallback));
            }
        }
        for f in &mut out {
            f.promotable = f.origin == Some(Origin::Game) && !f.own.is_empty() && twin(&f.key).is_some();
            for e in &mut f.entries {
                e.promotable = e.origin == Origin::Game && twin(&format!("{}.{}", f.key, e.name)).is_some();
            }
        }
        Ok(out)
    }

    fn module_form(&self, id: &str) -> Result<Vec<Field>> {
        let m = owners(&self.inputs.modules)?.into_iter().find(|m| m.id == id).ok_or_else(|| Error::NotFound(format!("module {id}")))?;
        let mut out = vec![Field::new("enabled", "Enabled", "bool", "").plain(m.enabled.to_string())];
        let own = self.config.modules.settings.get(id);
        for s in m.settings.iter().filter(|s| matches!(s.scope.as_str(), "global" | "config")) {
            let f = setting_field(s, s.key.clone(), "Settings");
            out.push(f.inherits(table_value(own, &s.key), Origin::Global, render(&s.default), Origin::Default));
        }
        Ok(out)
    }

    fn source_form(&self, id: &str) -> Result<Vec<Field>> {
        let source = owners(&self.inputs.sources)?.into_iter().find(|s| s.id == id).ok_or_else(|| Error::NotFound(format!("source {id}")))?;
        let mut out = vec![Field::new("enabled", "Enabled", "bool", "").plain(source.enabled.to_string())];
        let own = self.config.sources.settings.get(id);
        for s in source.settings.iter().filter(|s| matches!(s.scope.as_str(), "global" | "config")) {
            let f = setting_field(s, s.key.clone(), "Settings");
            out.push(f.inherits(table_value(own, &s.key), Origin::Global, render(&self.source_default(s)), Origin::Default));
        }
        Ok(out)
    }
}

impl Core {
    /// What `fields` builds `form` from, gathered as a frontend reads it.
    pub async fn form_inputs(&self, form: &Form) -> Result<Inputs> {
        let config = self.config.read().await.clone();
        let mut inputs = Inputs { set: Config::file_set(), under_steam: self.under_steam(), ..Inputs::default() };
        let spec = match form {
            Form::Game(id) => {
                let r = self.get(id).await?;
                inputs.game = r.to_json();
                runners::spec(&r.effective.runner).or_else(|| runners::spec("proton"))
            }
            Form::Runner(id) => runners::spec(id),
            _ => None,
        };
        if let Some(spec) = spec {
            let found = runners::locate(spec, &config);
            inputs.runners = vec![serde_json::json!({ "id": spec.id, "path": found.program, "source": found.source })];
        }
        if !matches!(form, Form::Module(_) | Form::Source(_)) {
            inputs.protons = config.proton_names();
        }
        inputs.modules = self.modules().await;
        inputs.sources = self.sources.read().await.iter().map(|s| s.to_json()).collect();
        inputs.gpu = self.gpu().await;
        Ok(inputs)
    }

    pub async fn form(&self, form: &Form, screen: Option<Mode>) -> Result<Vec<Field>> {
        fields(form, &self.form_inputs(form).await?, screen)
    }

    /// Writes one field of `form`; an empty value clears the form's own, so the field falls back to what it inherits.
    pub async fn set_field(&self, form: &Form, key: &str, value: &str) -> Result<()> {
        self.apply_writes(set_writes(form, key, value)).await
    }

    /// A game's own value of `key` becomes the global one and leaves the game; other games keep theirs.
    pub async fn promote_field(&self, form: &Form, key: &str) -> Result<()> {
        let writes = promote_writes(form, key, &self.form_inputs(form).await?)?;
        self.apply_writes(writes).await
    }

    async fn apply_writes(&self, writes: Vec<Write>) -> Result<()> {
        for w in writes {
            match w {
                Write::Config { key, value } => self.set_setting(&key, &value).await?,
                Write::Runner { runner, key, value } => self.set_runner_setting(&runner, &key, &value).await?,
                Write::Game { game, key, value } => self.set(&game, &key, &value).await?,
                Write::Module { module, key, value } => self.set_module_setting(&module, "", &key, &value).await?,
                Write::Source { source, key, value } => self.set_source_setting(&source, "", &key, &value).await?,
                Write::EnableModule { module, on } => self.enable_module(&module, on).await?,
                Write::EnableSource { source, on } => self.enable_source(&source, on).await?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::paths;
    use crate::session::tests::{open, sandbox};

    fn field<'a>(fields: &'a [Field], key: &str) -> &'a Field {
        fields.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("no field {key}"))
    }

    /// The sandbox's `[launch]` gets `launch` at its head, and `tables` go at the end.
    fn edit_config(launch: &str, tables: &str) {
        let file = paths::config_file();
        let was = std::fs::read_to_string(&file).unwrap().replace("[launch]\n", &format!("[launch]\n{launch}"));
        std::fs::write(&file, format!("{was}{tables}")).unwrap();
    }

    #[tokio::test]
    async fn a_game_field_says_where_its_value_comes_from_and_a_reset_hands_it_back() {
        let _sb = sandbox();
        edit_config("", "[launch.env]\nFROM_GLOBAL = \"1\"\n");
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.pause_on_home = Some(false);
        g.launch.env.insert("FROM_GAME".into(), "2".into());
        g.save().unwrap();
        let (core, _) = open().await;
        let form = Form::Game("sample".into());
        let fields = core.form(&form, None).await.unwrap();

        let pause = field(&fields, "launch.pause_on_home");
        assert_eq!((pause.own.as_str(), pause.value.as_str(), pause.inherited.as_str(), pause.origin), ("false", "false", "true", Some(Origin::Game)));
        assert_eq!(pause.fallback, Some(Origin::Default), "what a reset would show");
        assert!(pause.resettable);
        let hud = field(&fields, "launch.mangohud");
        assert_eq!((hud.own.as_str(), hud.value.as_str(), hud.origin, hud.resettable), ("", "false", Some(Origin::Global), false), "config.toml sets it");
        let gpu = field(&fields, "launch.discrete_gpu");
        assert_eq!((gpu.value.as_str(), gpu.origin), ("true", Some(Origin::Default)));
        let env = field(&fields, "launch.env");
        let names: Vec<(&str, Origin, bool)> = env.entries.iter().map(|e| (e.name.as_str(), e.origin, e.resettable)).collect();
        assert_eq!(names, [("FROM_GAME", Origin::Game, true), ("FROM_GLOBAL", Origin::Global, false)]);
        assert_eq!(field(&fields, "launch.gamescope_filter").resolved, "linear", "an unset scaling key is gamescope's own");
        assert!(fields.iter().all(|f| f.key != "launch.gamescope_bin"), "a global-only key is not a game's");
        assert!(fields.iter().all(|f| f.key != "launch.esync"), "a Wine key is not an emulator's");
        assert_eq!(field(&fields, "launch.runner").value, "dolphin");
        assert_eq!(field(&fields, "launch.runner_exe").origin, Some(Origin::Game), "the sandbox names its emulator");

        core.set_field(&form, "launch.pause_on_home", "").await.unwrap();
        let fields = core.form(&form, None).await.unwrap();
        let pause = field(&fields, "launch.pause_on_home");
        assert_eq!((pause.own.as_str(), pause.value.as_str(), pause.origin), ("", "true", Some(Origin::Default)));
        core.set_field(&form, &entry_key("launch.env", "NEW VAR!").unwrap(), "x").await.unwrap();
        assert_eq!(core.get("sample").await.unwrap().game.launch.env.get("NEWVAR").map(String::as_str), Some("x"), "the name is cleaned to one key");
    }

    #[tokio::test]
    async fn a_game_value_promoted_becomes_the_global_one_and_leaves_the_game() {
        let _sb = sandbox();
        let mut g = Game::load(&Game::new("Sample").toml_path()).unwrap();
        g.launch.pause_on_home = Some(false);
        g.launch.env.insert("FROM_GAME".into(), "2".into());
        g.launch.options.insert("batch".into(), toml::Value::Boolean(false));
        g.save().unwrap();
        let (core, _) = open().await;
        let form = Form::Game("sample".into());
        let fields = core.form(&form, None).await.unwrap();
        assert!(field(&fields, "launch.pause_on_home").promotable && field(&fields, "launch.options.batch").promotable);
        assert!(!field(&fields, "launch.mangohud").promotable, "nothing of the game's own");
        assert!(!field(&fields, "launch.runner_exe").promotable && !field(&fields, "launch.exe").promotable, "a game's alone");
        assert!(field(&fields, "launch.env").entries.iter().all(|e| e.promotable == (e.name == "FROM_GAME")));

        core.promote_field(&form, "launch.pause_on_home").await.unwrap();
        core.promote_field(&form, "launch.env.FROM_GAME").await.unwrap();
        core.promote_field(&form, "launch.options.batch").await.unwrap();
        let fields = core.form(&form, None).await.unwrap();
        let pause = field(&fields, "launch.pause_on_home");
        assert_eq!((pause.own.as_str(), pause.value.as_str(), pause.origin), ("", "false", Some(Origin::Global)));
        let env = field(&fields, "launch.env");
        assert_eq!(env.entries.iter().map(|e| (e.name.as_str(), e.origin)).collect::<Vec<_>>(), [("FROM_GAME", Origin::Global)]);
        let batch = field(&fields, "launch.options.batch");
        assert_eq!((batch.own.as_str(), batch.value.as_str(), batch.origin), ("", "false", Some(Origin::Runner)));
        assert!(core.promote_field(&form, "launch.pause_on_home").await.is_err(), "the game no longer sets it");
        assert!(core.promote_field(&form, "launch.exe").await.is_err(), "no global program");
    }

    #[tokio::test]
    async fn the_launch_form_holds_the_global_keys_and_the_config_ones() {
        let _sb = sandbox();
        edit_config("", "[proton]\nproton-em = \"~/proton-em\"\n");
        let (core, _) = open().await;
        let fields = core.form(&Form::Launch, Some(Mode { width: 2560, height: 1440, refresh: 144, vrr: true })).await.unwrap();
        let gamescope = field(&fields, "launch.gamescope");
        assert_eq!((gamescope.own.as_str(), gamescope.value.as_str(), gamescope.origin), ("false", "false", Some(Origin::Global)), "the sandbox turns it off");
        assert_eq!(field(&fields, "launch.gamescope_resolution").resolved, "2560x1440");
        assert_eq!(field(&fields, "launch.gamescope_refresh").resolved, "144");
        assert_eq!(field(&fields, "launch.fps_limit").resolved, "", "the sandbox sets none");
        assert_eq!(field(&fields, "launch.gamescope_adaptive_sync").resolved, "on");
        assert!(fields.iter().all(|f| f.key != "launch.proton"), "a Proton key is the Proton runner's");
        assert_eq!(field(&fields, "paths.games_root").origin, Some(Origin::Default));
        assert_eq!(field(&fields, "keys.sgdb").kind, "secret");
        assert_eq!(field(&fields, "proton").entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["proton-em"]);
    }

    #[tokio::test]
    async fn a_runner_program_set_on_its_form_is_the_runners_and_its_reset_finds_it_again() {
        let _sb = sandbox();
        edit_config("", "[proton]\nproton-em = \"~/proton-em\"\n");
        let (core, _) = open().await;
        let form = Form::Runner("dolphin".into());
        core.set_field(&form, "exe", "/opt/dolphin/dolphin-emu").await.unwrap();
        let fields = core.form(&form, None).await.unwrap();
        let exe = field(&fields, "exe");
        assert_eq!((exe.own.as_str(), exe.origin, exe.resettable), ("/opt/dolphin/dolphin-emu", Some(Origin::Runner), true));
        assert!(fields.iter().any(|f| f.section == "Options"), "dolphin's own options");
        assert!(fields.iter().all(|f| f.key != "launch.esync"), "a Wine key is not an emulator's");
        core.set_field(&form, "exe", "").await.unwrap();
        assert_eq!(field(&core.form(&form, None).await.unwrap(), "exe").own, "");
        let proton = core.form(&Form::Runner("proton".into()), None).await.unwrap();
        let default = field(&proton, "launch.proton");
        assert_eq!(default.section, "Builds", "the default build sits with the builds");
        assert!(default.choices.iter().any(|c| c.value == "proton-em"), "every Proton found, config.toml's included");
        assert!(proton.iter().any(|f| f.key == "launch.wayland") && proton.iter().any(|f| f.key == "launch.esync"));
    }

    /// A `controls` module under the sandbox's modules path, enabled in its config.toml.
    fn controls_module(manifest: &str) {
        let dir = paths::system_module_dirs()[0].join("controls");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("module.toml"), format!("api = 2\nid = \"controls\"\nname = \"Emulator controls\"\n{manifest}")).unwrap();
        let file = paths::config_file();
        let was = std::fs::read_to_string(&file).unwrap().replace("enabled = []", "enabled = [\"controls\"]");
        std::fs::write(&file, was).unwrap();
    }

    #[tokio::test]
    async fn a_module_setting_carries_its_description_and_choice_labels() {
        let _sb = sandbox();
        controls_module(
            r#"
[[settings]]
key = "layout"
type = "enum"
default = "positional"
label = "Nintendo button layout"
description = "Where A sits."
scope = "game"
choices = ["positional", "xbox"]
choice_labels = { positional = "Switch (A on the right)" }

[[settings]]
key = "guide"
type = "bool"
default = false
label = "Share HOME with the emulator"
description = "The emulator also gets HOME."
"#,
        );
        let (core, _) = open().await;
        let fields = core.form(&Form::Game("sample".into()), None).await.unwrap();
        let layout = field(&fields, "modules.controls.layout");
        assert_eq!(layout.description, "Where A sits.");
        assert_eq!(
            layout.choices,
            [Choice { value: "positional".into(), label: "Switch (A on the right)".into() }, Choice::same("xbox")],
            "a stored value without a label reads as itself"
        );
        let guide = field(&core.form(&Form::Module("controls".into()), None).await.unwrap(), "guide").clone();
        assert_eq!(guide.description, "The emulator also gets HOME.");
    }

    #[tokio::test]
    async fn a_module_setting_shows_on_the_games_it_applies_to() {
        let _sb = sandbox();
        controls_module(
            r#"
[applies]
runner_kinds = ["emulator"]

[[settings]]
key = "layout"
type = "enum"
default = "positional"
scope = "game"
choices = ["positional", "xbox"]

[[settings]]
key = "wiimote"
type = "enum"
default = "nunchuk"
scope = "game"
choices = ["nunchuk", "sideways"]
platforms = ["Nintendo Wii"]

[[settings]]
key = "dolphin_only"
type = "bool"
default = false
scope = "game"
runners = ["dolphin"]
"#,
        );
        let mut proton = Game::new("Hades");
        proton.launch.runner = "proton".into();
        proton.save().unwrap();
        let (core, _) = open().await;
        let keys = |fields: Vec<Field>| fields.into_iter().filter(|f| f.key.starts_with("modules.controls.")).map(|f| f.key).collect::<Vec<_>>();

        let gamecube = keys(core.form(&Form::Game("sample".into()), None).await.unwrap());
        assert_eq!(gamecube, ["modules.controls.enabled", "modules.controls.layout", "modules.controls.dolphin_only"], "no Wii row on a GameCube game");
        core.set("sample", "platform", "Nintendo Wii").await.unwrap();
        let wii = keys(core.form(&Form::Game("sample".into()), None).await.unwrap());
        assert!(wii.contains(&"modules.controls.wiimote".to_string()), "{wii:?}");
        assert_eq!(keys(core.form(&Form::Game("hades".into()), None).await.unwrap()), Vec::<String>::new(), "a Proton game shows no emulator module");
    }

    #[tokio::test]
    async fn a_stand_in_handing_over_the_same_views_as_json_gets_the_same_forms_and_writes() {
        let _sb = sandbox();
        controls_module("[[settings]]\nkey = \"layout\"\ntype = \"enum\"\ndefault = \"positional\"\nscope = \"game\"\nchoices = [\"positional\", \"xbox\"]\n");
        let (core, _) = open().await;
        core.set("sample", "modules.controls.layout", "xbox").await.unwrap();
        let screen = Some(Mode { width: 2560, height: 1440, refresh: 144, vrr: true });
        for form in [Form::Launch, Form::Runner("dolphin".into()), Form::Game("sample".into()), Form::Module("controls".into())] {
            let views: Inputs = serde_json::from_str(&serde_json::to_string(&core.form_inputs(&form).await.unwrap()).unwrap()).unwrap();
            assert_eq!(fields(&form, &views, screen).unwrap(), core.form(&form, screen).await.unwrap(), "{form:?}");
        }
        let game = Form::Game("sample".into());
        let views = core.form_inputs(&game).await.unwrap();
        assert_eq!(
            promote_writes(&game, "modules.controls.layout", &views).unwrap(),
            [
                Write::Module { module: "controls".into(), key: "layout".into(), value: "xbox".into() },
                Write::Game { game: "sample".into(), key: "modules.controls.layout".into(), value: String::new() }
            ]
        );
        assert!(promote_writes(&Form::Game("other".into()), "modules.controls.layout", &views).is_err(), "views of another game");
    }

    #[test]
    fn an_entry_name_is_one_key() {
        assert_eq!(entry_key("launch.env", "DXVK_HUD").as_deref(), Some("launch.env.DXVK_HUD"));
        assert_eq!(entry_key("launch.dll_overrides", "d3d11.dll").as_deref(), Some("launch.dll_overrides.d3d11dll"));
        assert_eq!(entry_key("launch.env", "…"), None);
    }
}
