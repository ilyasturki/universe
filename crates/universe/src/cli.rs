// Colour reaches a terminal only; NO_COLOR and CLICOLOR_FORCE are honoured.
use anstream::{eprintln, print, println};
use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Attribute, Cell, ColumnConstraint, ContentArrangement, Table};
use owo_colors::OwoColorize;
use serde_json::Value;

use crate::core::Core;
use crate::journal::Locale;
use crate::launcher;
use crate::sessions;

const AFTER_HELP: &str = "\
Games are named by id, whole word, substring or path; an ambiguous name asks on a terminal and fails elsewhere.
A game starts through its runner: proton (umu-run), wine, linux, or an emulator (`universe runner ls`); `universe add <file> --runner <id>` adds one.
A game runs as the transient unit universe-game-<id>-<session>.service; `universe session-end` closes it when its cgroup empties.

Files:
  ~/.config/universe/config.toml — paths, launch defaults, enabled modules and sources, [modules.<id>] and [sources.<id>] settings, keys
  ~/.local/share/universe/games/<id>/ — game.toml, sessions.jsonl, journal/, media/
  ~/.config/universe/modules/<id>/, ~/.config/universe/sources/<id>/ — user modules and sources, overriding the shipped ones
  ~/.local/share/universe/modules/<id>/, ~/.local/share/universe/sources/<id>/ — their data: caches, logins
  ~/.local/share/universe/saves/<id>/ — a game's save backups, as ludusavi lays them out (paths.saves_root)
  ~/.local/share/universe/ludusavi/ — the config and manifest of Universe's ludusavi, apart from your own
  ~/.local/state/universe/current-session.json — the running session
  ~/.local/state/universe/logs/<id>/<session>/ — Proton's and DXVK's logs of a launch with debug_log on; the game's own output is the journal's (`universe logs`)
  $XDG_RUNTIME_DIR/universe/controller.lock — held by the one controller watcher (the launcher's, or a session's)
  $XDG_RUNTIME_DIR/universe/active-pad — the sysfs device of the pad a button was last pressed on: player 1 in the emulators

Environment:
  UNIVERSE_CONFIG_HOME, UNIVERSE_DATA_HOME, UNIVERSE_STATE_HOME — replace the XDG directories
  UNIVERSE_MODULES_PATH, UNIVERSE_SOURCES_PATH — extra module and source roots, colon-separated
  RUST_LOG — tracing filter, warn by default

Errors are printed as `universe: <kind>: <message>` on stderr with exit status 1; --json works on every command.";

#[derive(Parser, Debug)]
#[command(name = "universe", version = crate::BUILD, about = "Universe: launch, record and remember your games", after_long_help = AFTER_HELP)]
pub struct Cli {
    /// Print raw JSON instead of tables
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Library, last played first (default command)
    Ls {
        /// Show hidden games too
        #[arg(long)]
        all: bool,
    },
    /// Launch a game (exact › word › substring or path) and wait for it to end; the game ends with this command (Ctrl-C stops it)
    #[command(alias = "launch")]
    Play {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// DRM connector the game runs on, e.g. DP-1 (default: the desktop profile decides)
        #[arg(long, default_value = "")]
        screen: String,
        /// Return once launched and leave the game to systemd instead of waiting for the session to end
        #[arg(long)]
        no_wait: bool,
    },
    /// Stop the running session
    Stop,
    /// Current session and last sessions
    Status,
    /// Source, build, prefix, hours, modules
    Info {
        /// Game: exact id, then whole word, substring or path
        name: String,
    },
    /// Search a source's catalogue
    Search {
        query: String,
        /// Source id; the one enabled when omitted
        #[arg(long)]
        source: Option<String>,
    },
    /// Install a title from a source
    Install {
        /// Id in the source's catalogue
        id: String,
        /// Source id; the one enabled when omitted
        #[arg(long)]
        source: Option<String>,
    },
    /// Pending updates, or update one game
    Update {
        /// Game: exact id, then whole word, substring or path; lists pending updates when omitted
        name: Option<String>,
        /// Do not ask before updating
        #[arg(long, short)]
        yes: bool,
    },
    /// Remove a game: parks recordings and journal; --purge trashes the prefix
    Rm {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
        /// Also trash the Wine prefix
        #[arg(long)]
        purge: bool,
    },
    /// Uninstall a game: trashes its install folder, keeps it in the library
    Uninstall {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Where a game's data lives, each part's size: install, prefix, saves, Universe's files, recordings, logs; or act on its prefix
    Data {
        /// Game: exact id, then whole word, substring or path
        name: String,
        #[command(subcommand)]
        action: Option<DataCmd>,
    },
    /// Universe's folders with their size and free space, the games by size, and what no game holds any more
    Storage {
        #[command(subcommand)]
        action: Option<StorageCmd>,
    },
    /// A game's saves through ludusavi: where they are, the backups kept, backing up, restoring, exporting
    Saves {
        #[command(subcommand)]
        action: SavesCmd,
    },
    /// Add a game by its file: a program, or a ROM, image or folder for an emulator
    Add {
        /// The game file (a .exe for proton/wine, a native program for linux, a ROM or image for an emulator)
        file: std::path::PathBuf,
        /// Runner id: proton, wine, linux, dolphin, eden, ryujinx, rpcs3, melonds, mgba, pcsx2, duckstation, cemu, azahar, ppsspp, xemu, xenia, shadps4, vita3k, mupen64plus, snes9x, flycast, scummvm, dosbox, mame (an alias such as yuzu or citra works)
        #[arg(long, short)]
        runner: String,
        /// Title; the file's name, cleaned of release tags, when omitted
        #[arg(long, short, default_value = "")]
        title: String,
        /// Platform; the runner's first when omitted (Dolphin: "Nintendo GameCube" or "Nintendo Wii")
        #[arg(long, short, default_value = "")]
        platform: String,
        /// Fetch artwork right away
        #[arg(long)]
        media: bool,
    },
    /// Runners: what starts a game (proton, wine, linux and the emulators), where each was found, their options
    Runner {
        #[command(subcommand)]
        action: RunnerCmd,
    },
    /// Components: the Proton and Wine builds, emulators and tools, those on the system and those Universe installs and updates
    #[command(alias = "components")]
    Component {
        #[command(subcommand)]
        action: ComponentCmd,
    },
    /// Set game keys: runner=dolphin proton=proton-em options.fullscreen=false capture.cursor=true hidden=true
    Set {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// key=value; launch keys (`universe launch-keys`) need no `launch.` prefix, a runner option is options.<key>
        pairs: Vec<String>,
    },
    /// The launch keys: what `set` and `config set launch.*` take, with each one's type, default and scope
    #[command(name = "launch-keys")]
    LaunchKeys,
    /// Sessions of a game
    Sessions {
        /// Game: exact id, then whole word, substring or path
        name: String,
    },
    /// What a session's processes wrote: the unit's journal, the launched command line first
    Logs {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// A session id (`universe sessions`); default the running one, else the last played
        session: Option<String>,
        /// The last N lines; 0 prints them all
        #[arg(short = 'n', long, default_value_t = 200)]
        lines: usize,
        /// Keep printing as the game writes (the running session)
        #[arg(short, long)]
        follow: bool,
    },
    /// Journal entries of a game
    Journal {
        /// Game: exact id, then whole word, substring or path
        name: Option<String>,
        /// Trash the entry of this session (a pending one is cancelled)
        #[arg(long, value_name = "SESSION")]
        remove: Option<String>,
        /// Write the entry of this session now: a retry, or a first entry for a session that never had one
        #[arg(long, value_name = "SESSION")]
        write: Option<String>,
        /// With --write: replace the entry the session already has
        #[arg(long)]
        force: bool,
        /// Start the entry owed to the oldest session waiting for another try; without a game, across the library
        #[arg(long)]
        retry: bool,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Recordings of a game
    Recordings {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Trash the recording of this session; the hours stay
        #[arg(long, value_name = "SESSION")]
        remove: Option<String>,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Artwork of a game (`all` refreshes every game)
    Media {
        /// Game: exact id, then whole word, substring or path, or `all`
        name: String,
        #[command(subcommand)]
        action: MediaCmd,
    },
    /// Modules and their settings
    Module {
        #[command(subcommand)]
        action: ModuleCmd,
    },
    /// Sources and their settings
    Source {
        #[command(subcommand)]
        action: SourceCmd,
    },
    /// Sources and their state: login, cached library, install folder
    Sources,
    /// Log into a source (prints the URL, then takes the code)
    Login {
        /// Source id
        source: String,
        /// The code= value from the address bar after logging in
        code: Option<String>,
    },
    /// Owned titles of a source
    Library {
        /// Source id; the one enabled when omitted
        source: Option<String>,
        /// Fetch again instead of reading the cache
        #[arg(long)]
        refresh: bool,
    },
    /// Scan installed games of the sources (all by default)
    Scan {
        /// Source id; every source when omitted
        source: Option<String>,
    },
    /// Import the Lutris library (report only without --apply)
    Migrate {
        /// Write the games and their hours instead of reporting
        #[arg(long)]
        apply: bool,
    },
    /// What other launchers (Lutris, Steam, Heroic) and the emulators' own folders hold on this machine, and what can be imported
    Discover,
    /// Check prerequisites of the core, the enabled modules and sources
    Doctor,
    /// After an install: put the desktop's part in place (GNOME's shell extension), then run doctor
    Setup,
    /// Global config
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Playback outputs; with an id, play through that one: the system default, as the desktop's own picker sets it
    Output {
        /// The output's id, as the list prints it
        id: Option<String>,
    },
    /// Take a screenshot through the module that provides one
    Screenshot,
    /// The player's own screenshots, newest first: of one game, or of every game
    Screenshots {
        /// Game: exact id, then whole word, substring or path
        name: Option<String>,
        /// Trash this shot (its file name) and drop it from the journal entry naming it
        #[arg(long, value_name = "NAME")]
        remove: Option<String>,
        /// Skip the confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// A game's achievements, as its store last listed them
    Achievements {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Ask the store again instead of reading the cache
        #[arg(long)]
        refresh: bool,
        /// Show the latest unlocks again over the running game, as banners; nothing reaches the store
        #[arg(long, conflicts_with = "refresh")]
        replay: bool,
        /// With --replay: only the N latest
        #[arg(long, value_name = "N", requires = "replay")]
        count: Option<usize>,
    },
    /// Controller macros: paddles and spare buttons bound to actions
    Controller {
        #[command(subcommand)]
        action: ControllerCmd,
    },
    /// Reload config, reread the library, add the games under the emulators' own folders (universe discover lists them)
    Rescan,
    /// Close a session: run by systemd's ExecStopPost when the game's cgroup empties
    #[command(name = "session-end", hide = true)]
    SessionEnd { id: String, session: String },
    /// File a recording under the session (capture module's session-end hook)
    #[command(name = "recording-file", hide = true)]
    RecordingFile {
        session: String,
        path: String,
        /// JSON `{"started_at": RFC3339, "pauses": [[from, to]]}`: the recorder's clock against the wall's
        #[arg(long)]
        timeline: Option<String>,
    },
    /// Add a journal entry to the session (journal module's post-process hook)
    #[command(name = "journal-add", hide = true)]
    JournalAdd { session: String, entry: String },
    /// Mark an achievement unlocked in the game's cache (a source's post-launch hook); prints whether it is new
    #[command(name = "achievement-unlocked", hide = true)]
    AchievementUnlocked {
        id: String,
        /// JSON `{"key", "name", "description", "unlocked_at", "icon", …}`
        achievement: String,
    },
    /// The running game's window as the shell extension lists it (capture module's post-launch hook)
    #[command(name = "session-window", hide = true)]
    SessionWindow {
        /// Block up to this many seconds for the window to map, then focus it
        #[arg(long)]
        wait: Option<u64>,
    },
    /// The connector's current mode: width, height, refresh (capture module's fps choices)
    #[command(name = "screen-mode", hide = true)]
    ScreenMode {
        /// DRM connector, e.g. DP-1 (default: the desktop profile decides)
        #[arg(default_value = "")]
        screen: String,
    },
    /// gamescope's primary child: the keep-alive window, then the game (universe splash [--image P] -- <cmd…>)
    #[command(hide = true)]
    Splash {
        #[arg(long)]
        image: Option<std::path::PathBuf>,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        cmd: Vec<String>,
    },
    /// Hold the desktop's idle inhibitor until stopped: the unit a launch binds to the game (desktop.keep_awake)
    #[command(name = "keep-awake", hide = true)]
    KeepAwake {
        /// What the desktop shows as the reason
        #[arg(long, default_value = "a game is running")]
        reason: String,
    },
    /// Save what the gamescope around this process shows to <path>, through its own screenshot: the screenshot module inside the launcher's gamescope
    #[command(name = "nest-shot", hide = true)]
    NestShot {
        path: std::path::PathBuf,
        /// The overlay layers too (mangoapp's HUD), not the game alone
        #[arg(long)]
        overlays: bool,
    },
    /// Save a screenshot to <path> through the desktop (desktop.profile): the screenshot module off gamescope
    #[command(name = "desktop-shot", hide = true)]
    DesktopShot {
        path: std::path::PathBuf,
        /// DRM connector of a whole-screen shot, e.g. DP-1 (default: the desktop's choice)
        #[arg(long, default_value = "")]
        screen: String,
        /// The focused window alone
        #[arg(long)]
        window: bool,
        /// Draw the pointer
        #[arg(long)]
        cursor: bool,
    },
    /// Show the desktop's on-screen display: <icon> is a themed icon name, --level in [0, 1] draws a bar
    #[command(name = "osd", hide = true)]
    Osd {
        icon: String,
        label: String,
        #[arg(long)]
        level: Option<f64>,
    },
    /// Completion candidates for the shell: games | sources | modules | …
    #[command(name = "__complete", hide = true)]
    Complete { what: String },
    /// Write the Fish completions and the man pages under <dir>
    #[command(name = "__generate", hide = true)]
    Generate { dir: std::path::PathBuf },
}

const MEDIA_SLOTS: [&str; 6] = ["box_front", "square", "banner", "background", "logo", "screenshot"];

#[derive(Subcommand, Debug)]
pub enum MediaCmd {
    /// Fetch art and details from the game's store, Steam, GOG GamesDB, libretro (and SteamGridDB with a key)
    Refresh {
        /// Fetch again even when every slot is filled
        #[arg(long)]
        force: bool,
    },
    /// Every slot: what shows, the fetched default, the override, where each came from
    Status,
    /// Pick a file or a URL for a slot: its override, kept over what refresh fetches
    Set {
        #[arg(value_parser = MEDIA_SLOTS)]
        slot: String,
        /// Image file or http(s) URL, placed under the game's media/picked/
        source: String,
    },
    /// Remove a slot's pick, so it shows the fetched default again
    Unset {
        #[arg(value_parser = MEDIA_SLOTS)]
        slot: String,
    },
    /// List the candidates of a slot
    Candidates {
        #[arg(default_value = "box_front", value_parser = MEDIA_SLOTS)]
        slot: String,
        /// The provider's page (50 per page)
        #[arg(long, default_value_t = 0)]
        page: u32,
    },
    /// Search the provider's games by name (the title when empty), to find the id to pin
    Search { query: Vec<String> },
    /// Pin the game to a provider id
    Pin {
        #[arg(value_parser = ["steam", "gamesdb", "sgdb"])]
        provider: String,
        /// The game's id at the provider
        id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum DataCmd {
    /// Move the prefix into prefixes_root (a rename, or a copy across filesystems); every game on it follows. Steam's compatdata stays
    MovePrefix {
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Trash Universe's prefix once its saves are backed up: the next launch makes a fresh one
    ResetPrefix {
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// winecfg in the game's prefix
    Winecfg,
    /// winetricks in the game's prefix, with its verbs; none opens its window
    Winetricks { verbs: Vec<String> },
    /// Run a Windows program in the game's prefix
    Run {
        exe: std::path::PathBuf,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Stop the prefix's wineserver, and every program in the prefix with it
    Kill,
}

#[derive(Subcommand, Debug)]
pub enum StorageCmd {
    /// Trash a leftover the storage view lists, named by its path
    Trash {
        path: String,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SavesCmd {
    /// Where ludusavi finds the saves, and the backups kept
    #[command(alias = "ls")]
    List {
        /// Game: exact id, then whole word, substring or path
        name: String,
    },
    /// Back the saves up now; unchanged saves make no new backup
    Backup {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// The unit a session's end starts: a game with nothing to back up is passed over without an error
        #[arg(long, hide = true)]
        auto: bool,
    },
    /// Put a backup's saves back where the game reads them
    Restore {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// The backup's id (`universe saves list`); the latest when omitted
        backup: Option<String>,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
    /// Every backup of the game in one zip, which any ludusavi restores from once unpacked
    Export {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// The folder the zip lands in
        #[arg(default_value = ".")]
        to: String,
    },
    /// The game's cloud saves through its store: their state, a sync now, or the side a conflict keeps
    Cloud {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Bring the cloud's saves down, as before a session
        #[arg(long, group = "cloud_action")]
        download: bool,
        /// Send the saves up, as after a session
        #[arg(long, group = "cloud_action")]
        upload: bool,
        /// Settle a conflict with the saves on this device: the cloud's are replaced
        #[arg(long, group = "cloud_action")]
        keep_local: bool,
        /// Settle a conflict with the cloud's saves: the ones here are backed up, then replaced
        #[arg(long, group = "cloud_action")]
        keep_cloud: bool,
        /// Do not ask for confirmation
        #[arg(long, short)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum RunnerCmd {
    /// Every runner: its platforms, where its program was found, whether it is usable
    #[command(alias = "list")]
    Ls,
    /// One runner's options and their current global values
    Options {
        /// Runner id
        id: String,
    },
    /// Set a runner's global keys: exe=/path args="--flag" fullscreen=false gamescope=false (an empty value resets)
    Set {
        /// Runner id
        id: String,
        /// key=value, validated against the runner's options
        pairs: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ComponentCmd {
    /// Every component: what runs, what Universe installed, what the catalogue offers
    #[command(alias = "list")]
    Ls {
        /// Fetch the catalogue even when the cached one is less than a day old
        #[arg(long)]
        refresh: bool,
    },
    /// Install a build: the newest this machine runs when no version is given
    Install {
        /// Component id (universe component ls)
        id: String,
        version: Option<String>,
        /// Accept the component's notice without asking
        #[arg(long, short)]
        yes: bool,
    },
    /// Remove a build Universe installed
    Remove { id: String, version: String },
    /// Remove every build Universe installed of a component
    Uninstall { id: String },
    /// Install what is newer than Universe's builds, keeping the previous one; every component when no id is given
    Update { id: Option<String> },
    /// Remove Universe's newest build and never take its version again
    Rollback { id: String },
    /// What runs: latest (Universe's newest build), system (a runner's own program) or a version
    Use { id: String, build: String },
}

#[derive(Subcommand, Debug)]
pub enum SourceCmd {
    /// Installed sources
    #[command(alias = "list")]
    Ls,
    /// Enable a source
    Enable {
        /// Source id
        id: String,
    },
    /// Disable a source
    Disable {
        /// Source id
        id: String,
    },
    /// Settings of a source, global or merged with a game's
    Settings {
        /// Source id
        id: String,
        /// Game: exact id, then whole word, substring or path: merge its overrides
        game: Option<String>,
    },
    /// Set source settings: key=value…, globally or for --game
    Set {
        /// Source id
        id: String,
        /// key=value, validated against the source's settings
        pairs: Vec<String>,
        /// Write a game-scope key into this game's game.toml instead of config.toml
        #[arg(long)]
        game: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModuleCmd {
    /// Installed modules
    #[command(alias = "list")]
    Ls,
    /// Enable a module
    Enable {
        /// Module id
        id: String,
    },
    /// Disable a module
    Disable {
        /// Module id
        id: String,
    },
    /// Settings of a module, global or merged with a game's
    Settings {
        /// Module id
        id: String,
        /// Game: exact id, then whole word, substring or path: merge its overrides
        game: Option<String>,
    },
    /// Set module settings: key=value…, globally or for --game
    Set {
        /// Module id
        id: String,
        /// key=value, validated against the module's settings
        pairs: Vec<String>,
        /// Write into this game's game.toml instead of config.toml
        #[arg(long)]
        game: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Resolved config, or one dotted key
    Get {
        /// Dotted key, e.g. launch.proton
        key: Option<String>,
    },
    /// Write a dotted key (an empty value deletes it)
    Set {
        /// Dotted key, e.g. launch.proton
        key: String,
        /// Empty deletes the key
        #[arg(default_value = "")]
        value: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ControllerCmd {
    /// Connected pads with their buttons, then the macros and presets
    #[command(alias = "list")]
    Ls,
    /// Run the macro engine (the launcher and a game's unit start it themselves)
    Watch {
        /// One JSON object per line on stdout, commands on stdin
        #[arg(long)]
        json: bool,
        /// Wait for the running watcher to end instead of exiting at once
        #[arg(long)]
        wait: bool,
    },
    /// Bind an action to a button
    Bind {
        /// Family: dualsense-edge, xbox-elite, … or `*` for every pad
        family: String,
        /// Button: paddle_left, fn_right, guide, …
        button: String,
        /// press or hold
        #[arg(value_parser = crate::controller::TRIGGERS)]
        trigger: String,
        /// volume_up, volume_down, mute, screenshot, mangohud, stop, keys, command
        action: String,
        /// For `keys`: the combo, e.g. Super_L+F12
        #[arg(long, default_value = "")]
        keys: String,
        /// For `command`: run with sh -c
        #[arg(long, default_value = "")]
        command: String,
    },
    /// Remove a button's macro (both triggers when none is given)
    Unbind {
        family: String,
        button: String,
        #[arg(value_parser = crate::controller::TRIGGERS)]
        trigger: Option<String>,
    },
    /// Press a button on the pad: its code becomes the slot's
    Learn {
        family: String,
        /// The slot the pressed button will drive
        slot: String,
    },
    /// Forget a learned button (the shipped codes apply again)
    Forget { family: String, slot: String },
}

fn table(headers: &[&str]) -> Table {
    let mut t = Table::new();
    t.load_style(UTF8_FULL_CONDENSED);
    t.set_content_arrangement(ContentArrangement::Dynamic);
    t.set_header(headers.iter().map(|h| Cell::new(h).add_attribute(Attribute::Bold)));
    t
}

fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A JSON list of strings, joined.
fn joined(v: &Value, sep: &str) -> String {
    v.as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(sep)).unwrap_or_default()
}

/// A module's or a source's Missing column: the Universe it needs, else its missing binaries.
fn missing(m: &Value) -> String {
    m["incompatible"].as_str().filter(|i| !i.is_empty()).map(String::from).unwrap_or_else(|| joined(&m["missing"], ","))
}

fn hours(v: &Value) -> String {
    let h = v["stats"]["hours"].as_f64().unwrap_or(0.0);
    if h == 0.0 {
        String::new()
    } else {
        format!("{h:.1}")
    }
}

fn local(ts: &str) -> Option<chrono::DateTime<chrono::Local>> {
    let ts = ts.trim();
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(ts) {
        return Some(t.with_timezone(&chrono::Local));
    }
    let d = chrono::NaiveDate::parse_from_str(ts, "%Y-%m-%d").ok()?;
    d.and_hms_opt(0, 0, 0)?.and_local_timezone(chrono::Local).single()
}

fn day(ts: &str, loc: &Locale) -> String {
    local(ts).map(|t| loc.date(&t)).unwrap_or_else(|| ts.get(0..10).unwrap_or("").to_string())
}

/// The row's `end`, the exit code behind a crash.
fn fmt_end(r: &Value) -> String {
    match s(r, "end").as_str() {
        "crashed" => format!("crashed ({})", r["exit"].as_i64().unwrap_or(-1)),
        other => other.to_string(),
    }
}

fn when(ts: &str, loc: &Locale) -> String {
    local(ts).map(|t| loc.datetime(&t)).unwrap_or_else(|| ts.to_string())
}

/// A list as the text arms read it: `Value` rows, whatever the core's type.
fn rows(v: &impl serde::Serialize) -> Vec<Value> {
    serde_json::to_value(v).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

fn print_sources(list: &[Value], json: bool) -> anyhow::Result<()> {
    if json {
        return print_json(&list);
    }
    let mut t = table(&["Id", "Name", "Version", "Enabled", "Available", "Missing", "Library (cached)", "Games dir"]);
    for m in list {
        t.add_row(vec![
            s(m, "id"),
            s(m, "name"),
            s(m, "version"),
            flag(&m["enabled"]),
            flag(&m["available"]),
            missing(m),
            m["library_cached"].to_string(),
            s(m, "games_dir"),
        ]);
    }
    println!("{t}");
    Ok(())
}

fn print_json(v: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn ask(prompt: &str) -> String {
    use std::io::Write;
    print!("{prompt} ");
    let _ = anstream::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    s.trim().to_string()
}

fn confirm(prompt: &str) -> bool {
    ask(&format!("{prompt} [y/N]")).to_lowercase().starts_with('y')
}

fn progress_printer(json: bool) -> impl FnMut(u64, u64, &str) {
    move |done, total, message| {
        if !json {
            let pct = if total > 0 { format!("{:3.0}%", done as f64 * 100.0 / total as f64) } else { "    ".into() };
            eprintln!("  {pct} {message}");
        }
    }
}

/// The art of games just brought in, the network asked once they are all there.
async fn fetch_art(core: &Core, json: bool, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let mut p = progress_printer(json);
    if let Err(e) = core.media_refresh_many(ids, false, Some(&mut p)).await {
        eprintln!("media: {e}");
    }
}

fn finish(json: bool, r: crate::Result<String>) {
    match r {
        Ok(m) => report(json, true, &m),
        Err(e) => {
            report(json, false, &e.to_string());
            std::process::exit(1);
        }
    }
}

fn print_doctor(list: &[crate::doctor::Check], json: bool) -> anyhow::Result<()> {
    if json {
        return print_json(&list);
    }
    let width = list.iter().map(|c| c.label.chars().count()).max().unwrap_or(0);
    for c in list {
        if c.ok {
            println!("{} {:<width$}  {}  {}", "✓".green(), c.label, format!("{:<10}", c.module).dimmed(), c.detail);
        } else {
            println!("{} {}  {}", "✗".red(), format!("{:<width$}", c.label).bold(), format!("{:<10}  {}", c.module, c.check).dimmed());
            println!("    {}", c.detail);
            if !c.fix.is_empty() {
                println!("    {} {}", "fix:".yellow(), c.fix);
            }
        }
    }
    let bad = list.iter().filter(|c| !c.ok).count();
    if bad > 0 {
        println!("{} checks pass, {}", list.len() - bad, format!("{bad} need{} attention", if bad == 1 { "s" } else { "" }).red());
        std::process::exit(1);
    }
    println!("{}", format!("all {} checks pass", list.len()).green());
    Ok(())
}

fn report(json: bool, ok: bool, message: &str) {
    if json {
        let _ = print_json(&serde_json::json!({"ok": ok, "message": message}));
    } else if ok {
        println!("{} {message}", "done".green());
    } else {
        println!("{} {message}", "failed".red());
    }
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let json = cli.json;
    let cmd = cli.cmd.unwrap_or(Cmd::Ls { all: false });
    let loc = Locale::from_env();
    // Shell helpers stay off Core::open: a Tab must not reconcile a session or probe logins.
    match &cmd {
        Cmd::Complete { what } => return complete(what),
        Cmd::Generate { dir } => return generate(dir),
        Cmd::LaunchKeys => return launch_keys(json),
        Cmd::Splash { image, cmd } => std::process::exit(crate::splash::run(image.as_deref(), cmd)),
        Cmd::KeepAwake { reason } => return keep_awake(reason).await,
        Cmd::NestShot { path, overlays } => return nest_shot(path, *overlays),
        Cmd::DesktopShot { path, screen, window, cursor } => {
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir)?;
            }
            crate::desktop::screenshot(desktop_profile(), path, screen, *window, *cursor).await.map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("{}", path.display());
            return Ok(());
        }
        Cmd::Osd { icon, label, level } => return crate::desktop::show_osd(desktop_profile(), icon, label, *level).await.map_err(|e| anyhow::anyhow!("{e}")),
        _ => {}
    }
    let core = Core::open().await?;
    match cmd {
        Cmd::SessionEnd { id, session } => {
            let exit = match std::env::var("EXIT_CODE").as_deref() {
                Ok("exited") => std::env::var("EXIT_STATUS").ok().and_then(|s| s.parse().ok()),
                _ => None,
            };
            core.session_end(&id, &session, exit, None).await?;
        }
        Cmd::RecordingFile { session, path, timeline } => {
            let timeline = timeline
                .map(|p| std::fs::read_to_string(&p).map_err(crate::Error::from).and_then(|s| serde_json::from_str(&s).map_err(Into::into)))
                .transpose()?;
            println!("{}", core.file_recording(&session, &path, timeline.as_ref()).await?)
        }
        Cmd::JournalAdd { session, entry } => core.add_entry(&session, serde_json::from_str(&entry).map_err(crate::Error::from)?).await?,
        Cmd::AchievementUnlocked { id, achievement } => {
            println!("{}", core.achievement_unlocked(&id, serde_json::from_str(&achievement).map_err(crate::Error::from)?).await?)
        }
        Cmd::SessionWindow { wait } => {
            let window = match wait {
                Some(secs) => {
                    let c = core.current().await.ok_or_else(|| crate::Error::NotFound("no session running".into()))?;
                    core.wait_session_window(&c.session_id, std::time::Duration::from_secs(secs)).await?
                }
                None => core.session_window().await?,
            };
            if json {
                return print_json(&window);
            }
            match window {
                Some(w) => println!("{}", w.title.unwrap_or_default()),
                None => println!("{}", "no window".dimmed()),
            }
        }
        Cmd::ScreenMode { screen } => {
            let mode = core.screen_mode(&screen).await;
            if json {
                return print_json(&mode);
            }
            println!("{} {}×{} @ {} Hz", s(&mode, "screen"), mode["width"], mode["height"], mode["refresh"]);
        }
        Cmd::Ls { all } => {
            let list = core.list().await;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Title", "Runner", "Source", "Hours", "Last played"]);
            for i in 1..=4 {
                t.column_mut(i).unwrap().set_constraint(ColumnConstraint::ContentWidth);
            }
            for g in list {
                if !all && g["hidden"].as_bool() == Some(true) {
                    continue;
                }
                let title = if g["favorite"].as_bool() == Some(true) { format!("★ {}", s(&g, "title")) } else { s(&g, "title") };
                let title = if g["hidden"].as_bool() == Some(true) { Cell::new(title).add_attribute(Attribute::Dim) } else { Cell::new(title) };
                t.add_row(vec![
                    title,
                    Cell::new(s(&g["effective"], "runner")),
                    Cell::new(s(&g["source"], "kind")),
                    Cell::new(hours(&g)),
                    Cell::new(day(&s(&g["stats"], "last_played"), &loc)),
                ]);
            }
            println!("{t}");
        }
        Cmd::Play { name, screen, no_wait } => {
            let id = pick(&core, &name).await?;
            // The game is bound to this process's scope: it goes down with us, cleanly on Ctrl-C.
            if !no_wait {
                core.adopt_scope().await?;
            }
            let sid = core.launch(&id, &screen, "").await?;
            if json {
                print_json(&serde_json::json!({"session": sid, "id": id}))?;
            } else {
                println!("{} {id} · session {sid}", "launched".green());
            }
            if no_wait {
                return Ok(());
            }
            let unit = format!("{}.service", launcher::unit_name(&id, &sid));
            use tokio::signal::unix::{signal, SignalKind};
            let (mut int, mut term) = (signal(SignalKind::interrupt())?, signal(SignalKind::terminate())?);
            let mut stopping = false;
            while core.host.units.is_active(&unit).await {
                let signalled = tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => false,
                    _ = int.recv() => true,
                    _ = term.recv() => true,
                };
                if signalled && stopping {
                    // A second signal: the scope takes the game down with us, ExecStopPost still closes the session.
                    std::process::exit(130);
                }
                if signalled {
                    stopping = true;
                    if !json {
                        eprintln!("stopping {id}…");
                    }
                    match core.stop("").await {
                        Ok(()) | Err(crate::Error::NotFound(_)) => {}
                        Err(e) => eprintln!("universe: {e}"),
                    }
                }
            }
            let path = core.get(&id).await?.game.sessions_path();
            for _ in 0..30 {
                if let Some(s) = sessions::read(&path).unwrap_or_default().into_iter().find(|s| s.session == sid) {
                    if !json {
                        let end = match sessions::end_of(&s) {
                            "crashed" => format!("crashed ({})", s.exit).red().to_string(),
                            other => other.yellow().to_string(),
                        };
                        println!("{end} after {}", fmt_duration(s.duration_s));
                    }
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            anyhow::bail!("session {sid} ended but was not closed; see `universe logs {id} {sid}` or `journalctl --user -u {unit}`");
        }
        Cmd::Stop => {
            core.stop("").await?;
            println!("stopped");
        }
        Cmd::Status => {
            let cur = core.current().await;
            let pending = core.pending_journals().await;
            let mut recent = rows(&core.sessions("").await?);
            recent.truncate(10);
            if json {
                return print_json(&serde_json::json!({"current": cur, "recent": recent, "pending_journals": pending}));
            }
            match cur {
                None => println!("{}", "no session running".dimmed()),
                Some(c) => println!("{} {} · session {} · {} · since {}", "running".green(), c.title, c.session_id, c.unit, when(&c.started_at, &loc)),
            }
            for p in pending {
                println!("{} writing {}… (session {})", "journal:".yellow(), s(&p, "title"), s(&p, "session"));
            }
            let mut t = table(&["Session", "Game", "Duration", "End", "Source", "Recording"]);
            for r in recent {
                t.add_row(vec![
                    s(&r, "session"),
                    s(&r, "title"),
                    fmt_duration(r["duration_s"].as_u64().unwrap_or(0)),
                    fmt_end(&r),
                    s(&r, "source"),
                    if r["recording"].is_null() { String::new() } else { "✓".into() },
                ]);
            }
            println!("{t}");
        }
        Cmd::Info { name } => {
            let id = pick(&core, &name).await?;
            let g = core.get(&id).await?.to_json();
            if json {
                return print_json(&g);
            }
            println!("{}  {}", s(&g, "title").bold(), s(&g, "id").dimmed());
            println!("  source     {} {}  build {}", s(&g["source"], "kind"), s(&g["source"], "id"), s(&g["source"], "build_id"));
            println!(
                "  runner     {} ({}) · {}",
                s(&g["effective"], "runner"),
                s(&g["effective"], "runner_name"),
                if s(&g["effective"], "runner_path").is_empty() { "not found".to_string() } else { s(&g["effective"], "runner_path") }
            );
            println!("  platform   {}", s(&g, "platform"));
            println!("  exe        {}", s(&g["launch"], "exe"));
            if s(&g["effective"], "runner_kind") == "emulator" {
                println!("  options    {}", g["effective"]["options"]);
            } else {
                println!("  prefix     {}", s(&g["launch"], "prefix"));
                println!("  proton     {} ({})", s(&g["effective"], "proton"), s(&g["effective"], "proton_path"));
            }
            println!(
                "  esync/fsync/ntsync/mangohud  {}/{}/{}/{}",
                g["effective"]["esync"], g["effective"]["fsync"], g["effective"]["ntsync"], g["effective"]["mangohud"]
            );
            let on: Vec<&str> = ["wayland", "hdr", "dlss_upgrade", "fsr4_upgrade", "xess_upgrade", "optiscaler"]
                .into_iter()
                .filter(|k| g["effective"][k].as_bool() == Some(true))
                .collect();
            println!(
                "  switches   {}{}",
                if on.is_empty() { "no switches on".to_string() } else { on.join(" ") },
                if s(&g["launch"], "wrapper").is_empty() { String::new() } else { format!(" · wrapper {}", s(&g["launch"], "wrapper")) }
            );
            println!("  gamescope  {} {}", g["effective"]["gamescope"], s(&g["effective"], "gamescope_args"));
            println!("  fps limit  {}", s(&g["effective"], "fps_limit"));
            println!("  env        {}", g["effective"]["env"]);
            println!("  hours      {}  plays {}  last {}", hours(&g), g["stats"]["play_count"], when(&s(&g["stats"], "last_played"), &loc));
            println!("  media      {}", g["media"]);
            println!("  modules    {}", g["effective"]["modules"]);
            println!("  journal    {} entries · recordings {}", g["journal_count"], g["recording_count"]);
        }
        Cmd::Search { query, source } => {
            let source = enabled_source(&core, source).await?;
            let list = core.source_search(&source, &query).await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Id", "Title", "Owned", "Installed"]);
            for g in list {
                t.add_row(vec![s(&g, "id"), s(&g, "title"), flag(&g["owned"]), flag(&g["installed"])]);
            }
            println!("{t}");
        }
        Cmd::Install { id, source } => {
            let source = enabled_source(&core, source).await?;
            if !json {
                println!("installing {id} from {source}");
            }
            let mut p = progress_printer(json);
            let installed = core.source_install(&source, &id, Some(&mut p)).await;
            if let Some(game) = installed.as_ref().ok().filter(|g| !g.is_empty()) {
                fetch_art(&core, json, std::slice::from_ref(game)).await;
            }
            finish(json, installed);
        }
        Cmd::Update { name, yes } => match name {
            Some(n) => {
                let id = pick(&core, &n).await?;
                let g = core.get(&id).await?.to_json();
                let gid = s(&g["source"], "id");
                if gid.is_empty() {
                    anyhow::bail!("{id} has no source id");
                }
                let mut p = progress_printer(json);
                finish(json, core.source_update(&s(&g["source"], "kind"), &gid, Some(&mut p)).await.map(|n| format!("{n} updated")));
            }
            None => {
                let list = core.source_updates().await?;
                if json {
                    return print_json(&list);
                }
                if list.is_empty() {
                    println!("everything is current");
                    return Ok(());
                }
                let mut t = table(&["Id", "Title", "Local", "Remote", "Version", "Date"]);
                for u in &list {
                    t.add_row(vec![s(u, "id"), s(u, "title"), s(u, "local_build"), s(u, "remote_build"), s(u, "version"), day(&s(u, "date"), &loc)]);
                }
                println!("{t}");
                if yes || confirm("download?") {
                    let mut p = progress_printer(json);
                    for u in &list {
                        match core.source_update(&s(u, "source"), &s(u, "id"), Some(&mut p)).await {
                            Ok(_) => report(json, true, &s(u, "title")),
                            Err(e) => report(json, false, &format!("{}: {e}", s(u, "title"))),
                        }
                    }
                }
            }
        },
        Cmd::Rm { name, yes, purge } => {
            let id = pick(&core, &name).await?;
            if yes || confirm(&format!("remove {id}{}?", if purge { " and trash its prefix" } else { "" })) {
                core.remove(&id, purge).await?;
                println!("removed {id}");
            }
        }
        Cmd::Uninstall { name, yes } => {
            let id = pick(&core, &name).await?;
            if yes || confirm(&format!("uninstall {id} and remove its files?")) {
                core.uninstall(&id).await?;
                println!("uninstalled {id}");
            }
        }
        Cmd::Data { name, action } => {
            let id = pick(&core, &name).await?;
            match action {
                None => {
                    let data = core.game_data(&id).await?;
                    if json {
                        return print_json(&data);
                    }
                    print_data(&data);
                }
                Some(DataCmd::MovePrefix { yes }) => {
                    let (game, config) = (core.get(&id).await?.game, core.config.read().await.clone());
                    let from = launcher::prefix_of(&game, &config);
                    let keeps = keeps_old_path(crate::data::prefix_owner(&from, &game, &config), &from.to_string_lossy());
                    let asked = format!("move {id}'s prefix into prefixes_root?{}", keeps.as_ref().map(|k| format!(" {k}")).unwrap_or_default());
                    if yes || confirm(&asked) {
                        let moved = core.move_prefix(&id).await?;
                        if json {
                            return print_json(&moved);
                        }
                        println!("{} {} → {}", "moved".green(), s(&moved, "from"), s(&moved, "to"));
                        if let Some(keeps) = keeps {
                            println!("{}", keeps.yellow());
                        }
                        if !moved["left"].is_null() {
                            println!("{}", format!("{} could not go to the trash: it is still there", s(&moved, "left")).yellow());
                        }
                    }
                }
                Some(DataCmd::ResetPrefix { yes }) => {
                    if yes || confirm(&format!("back {id}'s saves up and trash its prefix?")) {
                        let reset = core.reset_prefix(&id).await?;
                        if json {
                            return print_json(&reset);
                        }
                        println!("{} {}", "trashed".green(), s(&reset, "trashed"));
                    }
                }
                Some(tool) => {
                    let (tool, args) = match tool {
                        DataCmd::Winecfg => ("winecfg", vec![]),
                        DataCmd::Winetricks { verbs } => ("winetricks", verbs),
                        DataCmd::Run { exe, args } => ("run", std::iter::once(exe.to_string_lossy().into_owned()).chain(args).collect()),
                        _ => ("kill", vec![]),
                    };
                    let said = |done: Value| match done["stopped"].as_u64() {
                        Some(0) => format!("nothing was running in {id}'s prefix"),
                        Some(_) => format!("{id}'s prefix stopped"),
                        None => format!("{tool} started as {}", s(&done, "unit")),
                    };
                    finish(json, core.prefix_tool(&id, tool, &args).await.map(said));
                }
            }
        }
        Cmd::Storage { action } => match action {
            None => {
                let storage = core.storage().await?;
                if json {
                    return print_json(&storage);
                }
                print_storage(&storage);
            }
            Some(StorageCmd::Trash { path, yes }) => {
                if yes || confirm(&format!("trash {path}?")) {
                    core.trash_leftover(&path).await?;
                    println!("trashed {path}");
                }
            }
        },
        Cmd::Saves { action } => match action {
            SavesCmd::List { name } => {
                let id = pick(&core, &name).await?;
                let data = core.game_data(&id).await?;
                if json {
                    return print_json(&data["saves"]);
                }
                print_saves(&data["saves"], &loc);
            }
            SavesCmd::Backup { name, auto } => {
                let id = pick(&core, &name).await?;
                match core.saves_backup(&id).await {
                    Ok(done) if json => print_json(&done)?,
                    Ok(done) => println!("{id}: {}", backup_text(&done)),
                    Err(e @ (crate::Error::NotFound(_) | crate::Error::Unavailable(_))) if auto => tracing::info!("{id}: no backup: {e}"),
                    Err(e) => return Err(e.into()),
                }
            }
            SavesCmd::Restore { name, backup, yes } => {
                let id = pick(&core, &name).await?;
                let which = backup.clone().unwrap_or_default();
                let what = if which.is_empty() { "the latest backup".to_string() } else { which.clone() };
                if yes || confirm(&format!("put {id}'s saves back from {what}? the saves there now are replaced")) {
                    let done = core.saves_restore(&id, &which).await?;
                    if json {
                        return print_json(&done);
                    }
                    println!("{id}: {} restored ({})", files_text(done.files.len()), human_size(done.bytes));
                }
            }
            SavesCmd::Export { name, to } => {
                let id = pick(&core, &name).await?;
                finish(json, core.saves_export(&id, &to).await.map(|zip| zip.to_string_lossy().into_owned()));
            }
            SavesCmd::Cloud { name, download, upload, keep_local, keep_cloud, yes } => {
                let id = pick(&core, &name).await?;
                let action = [(download, "download"), (upload, "upload"), (keep_local, "keep-local"), (keep_cloud, "keep-cloud")]
                    .into_iter()
                    .find_map(|(on, action)| on.then_some(action))
                    .unwrap_or("status");
                let replaced = match action {
                    "keep-local" => Some("the cloud's saves are replaced by the ones on this device"),
                    "keep-cloud" => Some("the saves on this device are backed up, then replaced by the cloud's"),
                    _ => None,
                };
                if replaced.is_some_and(|what| !yes && !confirm(&format!("{id}: {what}?"))) {
                    return Ok(());
                }
                let cloud = core.saves_cloud(&id, action).await?;
                if json {
                    return print_json(&cloud);
                }
                println!("{id}: {}", cloud_text(&cloud));
            }
        },
        Cmd::Add { file, runner, title, platform, media } => {
            let payload = serde_json::json!({"title": title, "runner": runner, "exe": file.to_string_lossy(), "platform": platform});
            let id = core.add_game(&payload).await?;
            let g = core.get(&id).await?;
            if media {
                let mut p = progress_printer(json);
                if let Err(e) = core.media_refresh(&id, false, Some(&mut p)).await {
                    eprintln!("media: {e}");
                }
            }
            if json {
                print_json(&core.get(&id).await?.to_json())?;
            } else {
                println!("{} {} · {} · {}", "added".green(), id, g.effective.runner_name, g.effective.platform);
                if g.effective.runner_path.is_empty() && g.effective.runner_kind != "linux" {
                    let offer = crate::components::cached(&crate::config::Config::load().unwrap_or_default())
                        .components
                        .get(&g.effective.runner)
                        .and_then(|e| e.latest())
                        .map(|b| format!("`universe component install {}` ({}), or ", g.effective.runner, b.version))
                        .unwrap_or_default();
                    println!(
                        "{}",
                        format!("{} was not found: {offer}install it, or `universe runner set {} exe=…`", g.effective.runner_name, g.effective.runner).yellow()
                    );
                }
            }
        }
        Cmd::Runner { action } => return runner(core, action, json).await,
        Cmd::Component { action } => return component(core, action, json).await,
        Cmd::Set { name, pairs } => {
            let id = pick(&core, &name).await?;
            for p in pairs {
                let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value, got {p}"))?;
                let k = match k {
                    "hide_cursor" => "desktop.hide_cursor".into(),
                    _ if crate::launch_keys::find(k.split('.').next().unwrap_or(k)).is_some() => format!("launch.{k}"),
                    _ => k.to_string(),
                };
                core.set(&id, &k, v).await?;
                println!("{id}: {k} = {v}");
            }
        }
        Cmd::Sessions { name } => {
            let id = pick(&core, &name).await?;
            let list = core.sessions(&id).await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Session", "Started", "Duration", "End", "Source", "Journal", "Recording"]);
            for r in rows(&list) {
                t.add_row(vec![
                    s(&r, "session"),
                    when(&s(&r, "started_at"), &loc),
                    fmt_duration(r["duration_s"].as_u64().unwrap_or(0)),
                    fmt_end(&r),
                    s(&r, "source"),
                    s(&r["journal"], "state"),
                    s(&r["recording"], "path"),
                ]);
            }
            println!("{t}");
        }
        Cmd::Logs { name, session, lines, follow } => {
            let id = pick(&core, &name).await?;
            let session = session.unwrap_or_default();
            let log = core.session_log(&id, &session, lines).await?;
            if json {
                return print_json(&log);
            }
            for l in &log {
                let clock = local(&l.time).map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_default();
                let source = format!("{}:", l.source);
                let source = if l.priority <= 3 { source.red().to_string() } else { source.cyan().to_string() };
                println!("{} {source} {}", clock.dimmed(), l.message);
            }
            if follow {
                let Some(c) = core.current().await.filter(|c| c.id == id && (session.is_empty() || c.session_id == session)) else {
                    anyhow::bail!("{id} is not running; --follow needs the running session");
                };
                let mut child = tokio::process::Command::new("journalctl").args(["--user", "-u", &c.unit, "-f", "-n", "0", "-o", "short", "-q"]).spawn()?;
                let unit = c.unit.clone();
                loop {
                    tokio::select! {
                        _ = child.wait() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                            if !core.host.units.is_active(&unit).await {
                                child.start_kill()?;
                                break;
                            }
                        }
                    }
                }
            }
        }
        Cmd::Journal { name, remove, write, force, retry, yes } => {
            if retry {
                let report = match &name {
                    Some(n) => core.retry_journals(&pick(&core, n).await?).await,
                    None => core.sweep_journals().await,
                };
                if json {
                    return print_json(&report);
                }
                match report["started"].as_object() {
                    Some(s) => println!("{} {} ({})", "writing".yellow(), s["game"].as_str().unwrap_or(""), s["session"].as_str().unwrap_or("")),
                    None => println!("{}", report["held"].as_str().or(report["error"].as_str()).unwrap_or("nothing waiting").dimmed()),
                }
                let due = report["due"].as_u64().unwrap_or(0);
                if due > 0 {
                    println!("{due} more waiting; they go one at a time");
                }
                if let Some(next) = report["next"].as_str().filter(|s| !s.is_empty()) {
                    println!("next try {}", when(next, &loc).dimmed());
                }
                return Ok(());
            }
            let name = name.ok_or_else(|| anyhow::anyhow!("which game? (universe journal <name>)"))?;
            let id = pick(&core, &name).await?;
            if let Some(session) = write {
                let unit = core.journal_write(&id, &session, force).await?;
                println!("{} {session} ({unit})", "writing".yellow());
                return Ok(());
            }
            if let Some(session) = remove {
                if yes || confirm(&format!("trash the journal entry {session} of {id}?")) {
                    core.remove_journal_entry(&id, &session).await?;
                    println!("removed {session}");
                }
                return Ok(());
            }
            let list = core.journal(&id).await?;
            if json {
                return print_json(&list);
            }
            for e in rows(&list) {
                let state = s(&e, "state");
                let tag = match state.as_str() {
                    "pending" => "writing…".yellow().to_string(),
                    "deferred" => format!("waiting · another try {}", when(&s(&e, "retry_at"), &loc)).yellow().to_string(),
                    "failed" => "failed".red().to_string(),
                    _ => format!("[{}]", s(&e, "lang")).dimmed().to_string(),
                };
                let duration = e["duration_s"].as_u64().filter(|d| *d > 0).map(|d| fmt_duration(d).dimmed().to_string()).unwrap_or_default();
                println!("{} {} {} {}", s(&e, "session").dimmed(), s(&e, "title").bold(), tag, duration);
                for p in e["paragraphs"].as_array().cloned().unwrap_or_default() {
                    println!("  {}", p.as_str().unwrap_or(""));
                }
                if !s(&e, "next_up").is_empty() {
                    println!("  {} {}", "next:".yellow(), s(&e, "next_up"));
                }
                println!();
            }
        }
        Cmd::Recordings { name, remove, yes } => {
            let id = pick(&core, &name).await?;
            if let Some(session) = remove {
                if yes || confirm(&format!("trash the recording {session} of {id}?")) {
                    core.remove_recording(&id, &session).await?;
                    println!("removed {session}");
                }
                return Ok(());
            }
            let list: Vec<_> = core.sessions(&id).await?.into_iter().filter(|r| r.recording.is_some()).collect();
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Session", "Duration", "Size", "Path"]);
            for r in rows(&list) {
                let rec = &r["recording"];
                let duration = rec["duration_s"].as_u64().filter(|d| *d > 0).or(r["duration_s"].as_u64()).unwrap_or(0);
                t.add_row(vec![s(&r, "session"), fmt_duration(duration), format!("{:.1} G", rec["size"].as_u64().unwrap_or(0) as f64 / 1e9), s(rec, "path")]);
            }
            println!("{t}");
        }
        Cmd::Media { name, action } => {
            let id = if name == "all" && matches!(action, MediaCmd::Refresh { .. } | MediaCmd::Status) { String::new() } else { pick(&core, &name).await? };
            match action {
                MediaCmd::Refresh { force } => {
                    let mut p = progress_printer(json);
                    let (changed, total) = core.media_refresh(&id, force, Some(&mut p)).await?;
                    report(json, true, &format!("{changed}/{total} updated"));
                }
                MediaCmd::Status => {
                    let list = core.media_status(&id).await?;
                    if json {
                        return print_json(&list);
                    }
                    let mut t = table(&["Game", "Entry", "Slot", "Kind", "Origin", "Shows", "Default"]);
                    for g in rows(&list) {
                        let e = &g["entry"];
                        let entry = match (s(e, "name"), e["id"].as_u64().unwrap_or(0)) {
                            (_, 0) => String::new(),
                            (name, id) if !name.is_empty() => format!("{name} ({} {id})", s(e, "provider")),
                            (_, id) => format!("{} {id}", s(e, "provider")),
                        };
                        for slot in g["slots"].as_array().cloned().unwrap_or_default() {
                            t.add_row(vec![
                                s(&g, "id"),
                                entry.clone(),
                                s(&slot, "slot"),
                                s(&slot, "kind"),
                                s(&slot, "origin"),
                                s(&slot, "path"),
                                if s(&slot, "override").is_empty() { String::new() } else { s(&slot, "default") },
                            ]);
                        }
                    }
                    println!("{t}");
                }
                MediaCmd::Set { slot, source } => {
                    let placed = if source.starts_with("http://") || source.starts_with("https://") {
                        core.media_set_url(&id, &slot, &source).await?
                    } else {
                        let path = std::fs::canonicalize(&source)?;
                        core.media_set_slot(&id, &slot, &path.to_string_lossy()).await?
                    };
                    report(json, true, &format!("{id}: {slot} → {placed}"));
                }
                MediaCmd::Unset { slot } => {
                    let gone = core.media_unset(&id, &slot).await?;
                    report(json, true, &if gone { format!("{id}: {slot} override removed") } else { format!("{id}: {slot} had no override") });
                }
                MediaCmd::Candidates { slot, page } => {
                    print_json(&core.media_candidates(&id, &slot, page).await?)?;
                }
                MediaCmd::Search { query } => {
                    let hits = core.media_search(&id, &query.join(" ")).await?;
                    if json {
                        return print_json(&hits);
                    }
                    let mut t = table(&["Id", "Name", "Year", "", ""]);
                    for h in rows(&hits) {
                        let year = h["year"].as_u64().filter(|y| *y > 0).map(|y| y.to_string()).unwrap_or_default();
                        t.add_row(vec![
                            h["id"].to_string(),
                            s(&h, "name"),
                            year,
                            if h["verified"].as_bool().unwrap_or(false) { "verified".into() } else { String::new() },
                            if h["current"].as_bool().unwrap_or(false) { "current".into() } else { String::new() },
                        ]);
                    }
                    println!("{t}");
                }
                MediaCmd::Pin { provider, id: pid } => {
                    core.media_pin(&id, &provider, &pid).await?;
                    println!("pinned");
                }
            }
        }
        Cmd::Module { action } => match action {
            ModuleCmd::Ls => {
                let list = core.modules().await;
                if json {
                    return print_json(&list);
                }
                let mut t = table(&["Id", "Name", "Version", "Enabled", "Available", "Missing", "Hooks"]);
                for m in list {
                    let hooks: Vec<String> = m["hooks"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                    t.add_row(vec![s(&m, "id"), s(&m, "name"), s(&m, "version"), flag(&m["enabled"]), flag(&m["available"]), missing(&m), hooks.join(",")]);
                }
                println!("{t}");
            }
            ModuleCmd::Enable { id } => {
                core.enable_module(&id, true).await?;
                println!("{id} enabled");
            }
            ModuleCmd::Disable { id } => {
                core.enable_module(&id, false).await?;
                println!("{id} disabled");
            }
            ModuleCmd::Settings { id, game } => {
                let gid = match game {
                    Some(g) => pick(&core, &g).await?,
                    None => String::new(),
                };
                print_json(&core.module_settings(&id, &gid).await?)?;
            }
            ModuleCmd::Set { id, pairs, game } => {
                let gid = match game {
                    Some(g) => pick(&core, &g).await?,
                    None => String::new(),
                };
                for p in &pairs {
                    let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value"))?;
                    core.set_module_setting(&id, &gid, k, v).await?;
                    println!("{id}.{k} = {v}{}", if gid.is_empty() { String::new() } else { format!(" ({gid})") });
                }
            }
        },
        Cmd::Sources => print_sources(&core.sources().await, json)?,
        Cmd::Source { action } => match action {
            SourceCmd::Ls => print_sources(&core.sources().await, json)?,
            SourceCmd::Enable { id } => {
                core.enable_source(&id, true).await?;
                println!("{id} enabled");
            }
            SourceCmd::Disable { id } => {
                core.enable_source(&id, false).await?;
                println!("{id} disabled");
            }
            SourceCmd::Settings { id, game } => {
                let gid = match game {
                    Some(g) => pick(&core, &g).await?,
                    None => String::new(),
                };
                print_json(&core.source_settings(&id, &gid).await?)?;
            }
            SourceCmd::Set { id, pairs, game } => {
                let gid = match game {
                    Some(g) => pick(&core, &g).await?,
                    None => String::new(),
                };
                for p in &pairs {
                    let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value"))?;
                    core.set_source_setting(&id, &gid, k, v).await?;
                    println!("{id}.{k} = {v}{}", if gid.is_empty() { String::new() } else { format!(" ({gid})") });
                }
            }
        },
        Cmd::Login { source, code } => {
            let code = match code {
                Some(c) => c,
                None => {
                    let url = core.source_login_url(&source).await?;
                    println!("Open this URL, log in, then paste the code= value from the address bar:\n\n  {url}\n");
                    ask("code:")
                }
            };
            let user = core.source_login(&source, &code).await?;
            report(json, true, &user);
        }
        Cmd::Library { source, refresh } => {
            let source = enabled_source(&core, source).await?;
            let list = core.source_library(&source, refresh).await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Id", "Title", "Installed", "Size", "Dir"]);
            for g in list {
                let installed = g["installed"].as_bool().unwrap_or(false);
                let partial = g["partial_bytes"].as_u64();
                let size = match (installed, partial, g["disk_size"].as_u64(), g["download_size"].as_u64()) {
                    (true, _, Some(d), _) => human_size(d),
                    (false, Some(p), Some(d), _) => format!("{} of {} kept", human_size(p), human_size(d)),
                    (false, Some(p), None, _) => format!("{} kept", human_size(p)),
                    (false, None, _, Some(d)) => format!("{} download", human_size(d)),
                    _ => String::new(),
                };
                t.add_row(vec![
                    s(&g, "id"),
                    s(&g, "title"),
                    if partial.is_some() && !installed { "paused".into() } else { flag(&g["installed"]) },
                    size,
                    if installed { s(&g, "dir") } else { s(&g, "partial_dir") },
                ]);
            }
            println!("{t}");
        }
        Cmd::Scan { source } => {
            let mut p = progress_printer(json);
            let found = core.source_scan(source.as_deref().unwrap_or(""), Some(&mut p)).await?;
            fetch_art(&core, json, &found).await;
            report(json, true, &format!("{} game(s)", found.len()));
            if !json {
                let active = core.active_source_ids().await;
                let mut t = table(&["Title", "Source", "Store id", "Id", "Build"]);
                for g in core.list().await {
                    let kind = s(&g["source"], "kind");
                    if source.as_ref().map_or(active.contains(&kind), |src| *src == kind) {
                        t.add_row(vec![s(&g, "title"), kind, s(&g["source"], "id"), s(&g, "id"), s(&g["source"], "build_id")]);
                    }
                }
                println!("{t}");
            }
        }
        Cmd::Migrate { apply } => {
            let imported = core.import_lutris(apply).await?;
            if apply {
                fetch_art(&core, json, &imported.imported).await;
            }
            let report = serde_json::to_value(imported)?;
            if json {
                return print_json(&report);
            }
            let n = |k: &str| report[k].as_array().map(|a| a.len()).unwrap_or(0);
            println!(
                "{} imported {}, skipped (already present) {}, updated {}",
                if apply { "applied:" } else { "dry run:" },
                n("imported"),
                n("skipped"),
                n("updated")
            );
            if let Some(h) = report["hours_imported"].as_object() {
                for (k, v) in h {
                    println!("  hours {k}: {v}");
                }
            }
            for h in report["runners"].as_array().cloned().unwrap_or_default() {
                println!(
                    "  [runners.{}] {} {}{}",
                    s(&h, "runner"),
                    s(&h, "program"),
                    joined(&h["args"], " "),
                    if s(&h, "wrapped") == "true" { " (Lutris wrapper dropped)".dimmed().to_string() } else { String::new() }
                );
            }
            let mut t = table(&["Game", "Added", "Removed", "Changed"]);
            for d in report["env_diffs"].as_array().cloned().unwrap_or_default() {
                let j = |k: &str| joined(&d[k], ", ");
                if !(j("added").is_empty() && j("removed").is_empty() && j("changed").is_empty()) {
                    t.add_row(vec![s(&d, "id"), j("added"), j("removed"), j("changed")]);
                }
            }
            println!("{t}");
            if !apply {
                println!("{}", "run with --apply to write games/*/game.toml".dimmed());
            }
        }
        Cmd::Discover => {
            let report = core.discover().await;
            if json {
                return print_json(&report);
            }
            for l in rows(&report.launchers) {
                let found = l["found"].as_bool().unwrap_or(false);
                let games = l["games"].as_u64().unwrap_or(0);
                let titles = l["titles"].as_array().map(|t| t.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                let state = if !found {
                    "not installed".dimmed().to_string()
                } else if games == 0 {
                    "no games".dimmed().to_string()
                } else if l["importable"].as_bool().unwrap_or(false) {
                    format!("{games} to import").green().to_string()
                } else {
                    format!("{games} found").yellow().to_string()
                };
                println!("{:<22} {:<14} {}", s(&l, "name"), state, if games > 0 { titles } else { s(&l, "detail") });
            }
            if !report.gog_dirs.is_empty() {
                println!("GOG installs under {} — universe source set gog scan_dirs=… then universe scan gog", report.gog_dirs.join(", "));
            }
            if report.launchers.iter().any(|l| l.id == "roms" && l.games > 0) {
                println!("Emulator folders: universe rescan adds them");
            }
        }
        Cmd::Doctor => return print_doctor(&core.doctor().await, json),
        Cmd::Setup => {
            if core.desktop().await == crate::desktop::Profile::Gnome {
                use crate::desktop::gnome::{self, ExtensionCopy};
                let uuid = crate::desktop::UNIVERSE_EXTENSION;
                match gnome::install_extension() {
                    Ok(ExtensionCopy::Written(dir)) => println!("{} {uuid} into {}", "installed".green(), dir.display()),
                    Ok(ExtensionCopy::Current) => println!("{uuid} is up to date"),
                    Ok(ExtensionCopy::System) => println!("{uuid} comes with the system's package"),
                    Err(e) => println!("{} {uuid}: {e}", "failed".red()),
                }
                match gnome::enable_extension().await {
                    Ok(true) => println!("{} {uuid}: it loads at the next login", "enabled".green()),
                    Ok(false) => {}
                    Err(e) => println!("{} to enable {uuid}: {e}", "failed".red()),
                }
                println!();
            }
            return print_doctor(&core.doctor().await, json);
        }
        Cmd::Config { action } => match action {
            ConfigCmd::Get { key } => {
                let mut v = core.settings().await;
                if let Some(key) = key {
                    for part in key.split('.') {
                        v = v.get(part).cloned().unwrap_or(Value::Null);
                    }
                }
                match v {
                    Value::String(s) => println!("{s}"),
                    other => print_json(&other)?,
                }
            }
            ConfigCmd::Set { key, value } => {
                core.set_setting(&key, &value).await?;
                println!("{key} = {value}");
            }
        },
        Cmd::Output { id: Some(id) } => {
            let level = core.set_output(&id).await?;
            if json {
                return print_json(&level);
            }
            println!("playing through {}", s(&level, "output"));
        }
        Cmd::Output { id: None } => {
            let list = core.outputs().await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Output", "Device", "Id", ""]);
            for o in list {
                t.add_row(vec![o.label, o.device, o.id, if o.current { "current".into() } else { String::new() }]);
            }
            println!("{t}");
        }
        Cmd::Screenshot => println!("{}", core.screenshot().await?),
        Cmd::Screenshots { name, remove, yes } => {
            let id = match &name {
                Some(n) => pick(&core, n).await?,
                None => String::new(),
            };
            if let Some(shot) = remove {
                if id.is_empty() {
                    anyhow::bail!("--remove takes a game");
                }
                if yes || confirm(&format!("trash the screenshot {shot} of {id}?")) {
                    core.remove_screenshot(&id, &shot).await?;
                    println!("removed {shot}");
                }
                return Ok(());
            }
            let list = core.screenshots(&id).await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Taken", "Game", "Session", "Path"]);
            for r in rows(&list) {
                t.add_row(vec![when(&s(&r, "taken_at"), &loc), s(&r, "title"), s(&r, "session"), s(&r, "path")]);
            }
            println!("{t}");
        }
        Cmd::Achievements { name, replay: true, count, .. } => {
            let id = pick(&core, &name).await?;
            let keys = core.achievements_replay(&id, count).await?;
            if json {
                return print_json(&keys);
            }
            println!("Replaying {}: {}", keys.len(), keys.join(", "));
        }
        Cmd::Achievements { name, refresh, .. } => {
            let id = pick(&core, &name).await?;
            let list = core.achievements(&id, refresh).await?;
            if json {
                return print_json(&list);
            }
            println!("{} of {} unlocked", list["unlocked"], list["total"]);
            let mut t = table(&["Unlocked", "Achievement", "Description"]);
            for r in list["items"].as_array().into_iter().flatten() {
                let at = s(r, "unlocked_at");
                let (name, what) = if at.is_empty() && r["hidden"].as_bool().unwrap_or(false) {
                    ("Hidden".to_string(), String::new())
                } else {
                    (s(r, "name"), s(r, "description"))
                };
                t.add_row(vec![if at.is_empty() { "—".into() } else { when(&at, &loc) }, name, what]);
            }
            println!("{t}");
        }
        Cmd::Controller { action } => return controller(core, action, json).await,
        Cmd::Rescan => {
            let report = core.rescan().await?;
            let added: Vec<String> = report.imported.iter().map(|f| f.id.clone()).collect();
            fetch_art(&core, json, &added).await;
            if json {
                return print_json(&serde_json::to_value(&report)?);
            }
            let n = report.imported.len();
            println!("rescanned, {n} game{} from the emulators' folders", if n == 1 { "" } else { "s" });
            for f in &report.imported {
                println!("  {} {} {}", f.id, f.runner.dimmed(), f.path.dimmed());
            }
            for k in &report.skipped {
                println!("  {} {}", k.path.dimmed(), k.reason);
            }
        }
        Cmd::Complete { .. }
        | Cmd::Generate { .. }
        | Cmd::Splash { .. }
        | Cmd::LaunchKeys
        | Cmd::KeepAwake { .. }
        | Cmd::NestShot { .. }
        | Cmd::DesktopShot { .. }
        | Cmd::Osd { .. } => unreachable!(),
    }
    Ok(())
}

async fn controller(core: Core, action: ControllerCmd, json: bool) -> anyhow::Result<()> {
    use crate::controller;
    match action {
        ControllerCmd::Watch { json: as_json, wait } => {
            controller::watch::watch(std::sync::Arc::new(core), controller::watch::WatchOptions { json: as_json, wait }).await?;
        }
        ControllerCmd::Ls => {
            let cfg = core.config.read().await.controller.clone();
            let mut state = controller::state_json(&cfg);
            state["devices"] = Value::Array(controller::watch::enumerate_json(&cfg));
            if json {
                return print_json(&state);
            }
            let devices = state["devices"].as_array().cloned().unwrap_or_default();
            if devices.is_empty() {
                println!("{}", "no pad connected".dimmed());
            }
            for d in &devices {
                println!("{} {} · {} · {}", s(d, "id").dimmed(), s(d, "name").bold(), s(d, "family_name"), s(d, "bus"));
                let family = s(d, "family");
                let mut t = table(&["Button", "Code", "Press", "Hold"]);
                let fam = controller::family_by_id(&family);
                let order: Vec<String> = fam.map(|f| f.slots().map(|s| s.id.to_string()).collect()).unwrap_or_default();
                for slot in order {
                    let entry = &d["slots"][&slot];
                    let label = fam.and_then(|f| f.slots().find(|s| s.id == slot)).map(|s| s.label).unwrap_or(&slot);
                    let macros = cfg.macros_for(&family, &slot);
                    let of = |t: &str| {
                        macros
                            .iter()
                            .find(|m| m.trigger == t)
                            .map(|m| if m.keys.is_empty() && m.command.is_empty() { m.action.clone() } else { format!("{} {}{}", m.action, m.keys, m.command) })
                            .unwrap_or_default()
                    };
                    let code = if entry["bound"] == true { s(entry, "code") } else { "unbound".to_string() };
                    t.add_row(vec![
                        Cell::new(label),
                        if entry["bound"] == true { Cell::new(code) } else { Cell::new(code).add_attribute(Attribute::Dim) },
                        Cell::new(of("press")),
                        Cell::new(of("hold")),
                    ]);
                }
                println!("{t}");
            }
            println!("{} enabled {} · hold {} ms · volume step {}%", "engine".bold(), cfg.enabled, cfg.hold_ms, cfg.volume_step);
            let mut t = table(&["Family", "Button", "Trigger", "Action", "Keys / command"]);
            for m in cfg.macros() {
                t.add_row(vec![m.family, m.button, m.trigger, m.action, format!("{}{}", m.keys, m.command)]);
            }
            println!("{t}");
        }
        ControllerCmd::Bind { family, button, trigger, action, keys, command } => {
            let m = controller::Macro { family, button, trigger, action, keys, command };
            core.set_controller_macro(m).await?;
            report(json, true, "bound");
        }
        ControllerCmd::Unbind { family, button, trigger } => {
            core.remove_controller_macro(&family, &button, trigger.as_deref().unwrap_or("")).await?;
            report(json, true, "unbound");
        }
        ControllerCmd::Learn { family, slot } => {
            let fam = controller::family_by_id(&family).ok_or_else(|| anyhow::anyhow!("unknown family {family}"))?;
            if !fam.slots().any(|s| s.id == slot) {
                anyhow::bail!("{} has no button {slot}", fam.name);
            }
            if !json {
                eprintln!("press the button for {} on the {}…", slot, fam.name);
            }
            let cfg = core.config.read().await.controller.clone();
            let (code, from) = controller::watch::learn_once(&cfg, fam, &slot).await?;
            core.reload_settings().await?;
            if json {
                print_json(&serde_json::json!({"family": family, "slot": slot, "code": code, "from": from}))?;
            } else {
                println!("{slot} = {code}{}", from.map(|f| format!(" (taken from {f})")).unwrap_or_default());
            }
        }
        ControllerCmd::Forget { family, slot } => {
            core.set_controller_button(&family, &slot, None).await?;
            report(json, true, "forgotten");
        }
    }
    Ok(())
}

async fn runner(core: Core, action: RunnerCmd, json: bool) -> anyhow::Result<()> {
    match action {
        RunnerCmd::Ls => {
            let list = core.runners().await;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Id", "Name", "Platforms", "Program", "Found"]);
            for r in list {
                let program = if s(&r, "path").is_empty() {
                    if s(&r, "kind") == "linux" {
                        "the game itself".to_string()
                    } else {
                        "not found".to_string()
                    }
                } else {
                    s(&r, "path")
                };
                let program = if r["available"].as_bool() == Some(true) { Cell::new(program) } else { Cell::new(program).add_attribute(Attribute::Dim) };
                t.add_row(vec![
                    Cell::new(s(&r, "id")),
                    Cell::new(s(&r, "name")),
                    Cell::new(joined(&r["platforms"], ", ")),
                    program,
                    Cell::new(s(&r, "source")),
                ]);
            }
            println!("{t}");
        }
        RunnerCmd::Options { id } => {
            let Some(r) = core.runners().await.into_iter().find(|r| s(r, "id") == crate::runners::canonical(&id)) else { anyhow::bail!("unknown runner {id}") };
            if json {
                return print_json(&r);
            }
            println!("{}  {}", s(&r, "name").bold(), s(&r, "id").dimmed());
            println!("  exe   {}", if s(&r, "exe").is_empty() { format!("{} (detected)", s(&r, "path")).dimmed().to_string() } else { s(&r, "exe") });
            println!("  args  {}", s(&r, "args"));
            let mut t = table(&["Option", "Type", "Value", "Label"]);
            for o in r["options"].as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&o, "key"), s(&o, "type"), o["value"].to_string().trim_matches('"').to_string(), s(&o, "label")]);
            }
            println!("{t}");
        }
        RunnerCmd::Set { id, pairs } => {
            for p in &pairs {
                let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value, got {p}"))?;
                core.set_runner_setting(&id, k, v).await?;
                println!("runners.{}.{k} = {v}", crate::runners::canonical(&id));
            }
            if pairs.is_empty() {
                anyhow::bail!("nothing to set: key=value…");
            }
        }
    }
    Ok(())
}

async fn component(core: Core, action: ComponentCmd, json: bool) -> anyhow::Result<()> {
    match action {
        ComponentCmd::Ls { refresh } => {
            let v = core.components(refresh).await?;
            if json {
                return print_json(&v);
            }
            let error = s(&v["catalogue"], "error");
            if !error.is_empty() {
                eprintln!("{}", format!("catalogue: {error}").yellow());
            }
            let mut t = table(&["Id", "Name", "Kind", "In use", "Latest", ""]);
            for c in v["components"].as_array().cloned().unwrap_or_default() {
                let used = &c["in_use"];
                let in_use = if used.is_null() {
                    "—".to_string()
                } else {
                    let version = s(used, "version");
                    format!("{} · {}", if version.is_empty() { "?".into() } else { version }, s(used, "origin"))
                };
                let note = match (s(&c, "update"), s(&c, "proposal").as_str()) {
                    (update, _) if !update.is_empty() => format!("update {update}"),
                    (_, "install") => "missing: install it".into(),
                    (_, "newer") => "a newer build".into(),
                    _ => String::new(),
                };
                t.add_row(vec![s(&c, "id"), s(&c, "name"), s(&c, "kind"), in_use, s(&c["latest"], "version"), note]);
            }
            println!("{t}");
        }
        ComponentCmd::Install { id, version, yes } => {
            let config = core.config.read().await.clone();
            let loaded = crate::components::load(&config, false).await;
            let accepted = yes
                || match loaded.catalogue.components.get(&id).filter(|e| !e.notice.is_empty()) {
                    None => false,
                    Some(e) if json || !std::io::IsTerminal::is_terminal(&std::io::stdin()) => {
                        anyhow::bail!("{}: {}\npass --yes to accept it and install", e.name, e.notice)
                    }
                    Some(e) => {
                        eprintln!("{}", e.notice.yellow());
                        if !confirm(&format!("install {}?", e.name)) {
                            return Ok(());
                        }
                        true
                    }
                };
            let mut p = progress_printer(json);
            finish(json, core.component_install(&id, version.as_deref().unwrap_or(""), accepted, Some(&mut p)).await.map(|v| format!("{id} {v} installed")));
        }
        ComponentCmd::Remove { id, version } => {
            core.component_remove(&id, &version).await?;
            report(json, true, &format!("{id} {version} removed"));
        }
        ComponentCmd::Uninstall { id } => {
            let gone = core.component_uninstall(&id).await?;
            report(json, true, &format!("{id} uninstalled: {} removed", gone.join(", ")));
        }
        ComponentCmd::Update { id } => {
            let mut p = progress_printer(json);
            let list = core.component_update(id.as_deref().unwrap_or(""), Some(&mut p)).await?;
            if json {
                return print_json(&list);
            }
            if list.is_empty() {
                println!("everything is current");
            }
            for u in &list {
                let error = s(u, "error");
                let tail = if error.is_empty() { String::new() } else { format!(": {error}") };
                report(false, error.is_empty(), &format!("{} {}{tail}", s(u, "name"), s(u, "version")));
            }
        }
        ComponentCmd::Rollback { id } => {
            let now = core.component_rollback(&id).await?;
            report(json, true, &if now.is_empty() { format!("{id}: back on the system's build") } else { format!("{id}: back on {now}") });
        }
        ComponentCmd::Use { id, build } => {
            core.component_use(&id, &build).await?;
            report(json, true, &format!("{id}: {build}"));
        }
    }
    Ok(())
}

async fn keep_awake(reason: &str) -> anyhow::Result<()> {
    let inhibitor = crate::desktop::inhibit_idle(reason).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    tracing::info!("holding {}", inhibitor.held().join(", "));
    use tokio::signal::unix::{signal, SignalKind};
    let (mut int, mut term) = (signal(SignalKind::interrupt())?, signal(SignalKind::terminate())?);
    tokio::select! {
        _ = int.recv() => {}
        _ = term.recv() => {}
    }
    inhibitor.release().await;
    Ok(())
}

fn desktop_profile() -> crate::desktop::Profile {
    crate::desktop::detect(&crate::config::Config::load().unwrap_or_default())
}

fn nest_shot(path: &std::path::Path, overlays: bool) -> anyhow::Result<()> {
    const WAIT: std::time::Duration = std::time::Duration::from_secs(10);
    let nest = crate::nest::Nest::open()?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let shot = if overlays { crate::nest::Shot::AllRealLayers } else { crate::nest::Shot::BasePlane };
    let Some(saved) = nest.frame(path, shot, WAIT)? else { anyhow::bail!("gamescope wrote no screenshot within {} s", WAIT.as_secs()) };
    println!("{}", saved.display());
    Ok(())
}

/// One candidate per line, `value<TAB>description`: Fish shows the description.
fn complete(what: &str) -> anyhow::Result<()> {
    match what {
        "runners" => {
            for r in crate::runners::RUNNERS {
                println!("{}\t{}", r.id, r.name);
            }
        }
        "games" => {
            let Ok(rd) = std::fs::read_dir(crate::paths::games_dir()) else { return Ok(()) };
            let mut games: Vec<(String, String)> = rd
                .flatten()
                .filter_map(|e| crate::game::Game::load(&e.path().join("game.toml")).ok())
                .filter(|g| g.removed_at.is_empty())
                .map(|g| (g.id, g.title))
                .collect();
            games.sort();
            for (id, title) in games {
                println!("{id}\t{title}");
            }
        }
        "families" => {
            for f in crate::controller::families() {
                println!("{}\t{}", f.id, f.name);
            }
            println!("*\tevery pad");
        }
        "buttons" => {
            let mut seen = std::collections::BTreeSet::new();
            for f in crate::controller::families() {
                for s in f.slots() {
                    if seen.insert(s.id) {
                        println!("{}\t{}", s.id, s.label);
                    }
                }
            }
        }
        "modules" | "sources" => {
            let config = crate::config::Config::load()?;
            let rows: Vec<(String, String)> = if what == "modules" {
                crate::modules::discover(&config).into_iter().map(|m| (m.id().into(), m.manifest.name)).collect()
            } else {
                crate::sources::discover(&config).into_iter().map(|m| (m.id().into(), m.name().into())).collect()
            };
            for (id, name) in rows {
                println!("{id}\t{name}");
            }
        }
        "outputs" => {
            for o in crate::sound::outputs().unwrap_or_default() {
                println!("{}\t{} · {}", o.id, o.label, o.device);
            }
        }
        other => anyhow::bail!("unknown completion set {other}"),
    }
    Ok(())
}

/// `--json` is `rows(Both, None)`, what `ui/universe_ui/fixtures/launch_keys.json` copies; the table lists the maps too.
fn launch_keys(json: bool) -> anyhow::Result<()> {
    use crate::launch_keys::{Kind, Scope, LAUNCH_KEYS};
    if json {
        return print_json(&crate::launch_keys::rows(Scope::Both, None));
    }
    let mut t = table(&["Key", "Type", "Default", "Scope", "Label"]);
    for k in LAUNCH_KEYS {
        let kind = match k.kind {
            Kind::Bool => "bool".to_string(),
            Kind::Toggle => crate::config::Toggle::CHOICES.join(" | "),
            Kind::Int { max: Some(m) } => format!("0..{m}"),
            Kind::Int { max: None } => "int".into(),
            Kind::Str => "string".into(),
            Kind::Path => "path".into(),
            Kind::List => "list".into(),
            Kind::Enum(choices) => choices.join(" | "),
            Kind::Resolution => "auto | WxH".into(),
            Kind::Refresh => "auto | Hz".into(),
            Kind::Fps => "auto | none | fps".into(),
            Kind::Proton => "[proton] name".into(),
            Kind::Map => format!("{}.<name>", k.key),
        };
        let scope = match k.scope {
            Scope::Game => "game",
            Scope::Global => "global",
            Scope::Both => "game, global",
        };
        t.add_row(vec![Cell::new(format!("launch.{}", k.key)), Cell::new(kind), Cell::new(k.default), Cell::new(scope), Cell::new(k.label)]);
    }
    println!("{t}");
    Ok(())
}

/// `set`'s candidates: the launch keys unprefixed (a map as `env.`), then the other game keys.
fn game_keys() -> Vec<String> {
    use crate::launch_keys::{Kind, Scope, LAUNCH_KEYS};
    let mut keys: Vec<String> = LAUNCH_KEYS
        .iter()
        .filter(|k| k.scope != Scope::Global)
        .map(|k| if k.kind == Kind::Map { format!("{}.", k.key) } else { format!("{}=", k.key) })
        .collect();
    keys.extend(
        [
            "hide_cursor=",
            "hidden=",
            "favorite=",
            "tags=",
            "sort_title=",
            "platform=",
            "metadata.sgdb_id=",
            "metadata.gamesdb_id=",
            "metadata.steam_appid=",
            "capture.cursor=",
        ]
        .map(String::from),
    );
    keys
}

fn config_keys() -> Vec<String> {
    use crate::launch_keys::{Kind, Scope, LAUNCH_KEYS};
    let mut keys: Vec<String> = ["paths.games_root", "paths.prefixes_root", "paths.recordings_root"].map(String::from).to_vec();
    keys.extend(LAUNCH_KEYS.iter().filter(|k| k.scope != Scope::Game).map(|k| {
        if k.kind == Kind::Map {
            format!("launch.{}.", k.key)
        } else {
            format!("launch.{}", k.key)
        }
    }));
    keys.extend(
        [
            "runners.",
            "desktop.profile",
            "desktop.hide_cursor",
            "desktop.cursor_extension",
            "desktop.keep_awake",
            "desktop.whats_new",
            "keys.sgdb",
            "keys.sgdb_file",
            "keys.prefer_sgdb",
            "controller.enabled",
            "controller.hold_ms",
            "controller.volume_step",
            "controller.home_summons",
        ]
        .map(String::from),
    );
    keys
}

/// `(subcommand path, position of the positional counted from that subcommand, candidates)`.
const POSITIONALS: &[(&str, usize, &str)] = &[
    ("play", 1, "games"),
    ("add", 1, "FILES"),
    ("runner options", 1, "runners"),
    ("runner set", 1, "runners"),
    ("info", 1, "games"),
    ("set", 1, "games"),
    ("rm", 1, "games"),
    ("uninstall", 1, "games"),
    ("data", 1, "games"),
    ("data run", 1, "FILES"),
    ("storage trash", 1, "FILES"),
    ("saves list", 1, "games"),
    ("saves backup", 1, "games"),
    ("saves restore", 1, "games"),
    ("saves export", 1, "games"),
    ("saves export", 2, "FILES"),
    ("saves cloud", 1, "games"),
    ("sessions", 1, "games"),
    ("screenshots", 1, "games"),
    ("achievements", 1, "games"),
    ("output", 1, "outputs"),
    ("journal", 1, "games"),
    ("recordings", 1, "games"),
    ("update", 1, "games"),
    ("media", 1, "games all"),
    ("media set", 1, "SLOTS"),
    ("media set", 2, "FILES"),
    ("media unset", 1, "SLOTS"),
    ("media candidates", 1, "SLOTS"),
    ("media pin", 1, "steam gamesdb sgdb"),
    ("module enable", 1, "modules"),
    ("module disable", 1, "modules"),
    ("module settings", 1, "modules"),
    ("module settings", 2, "games"),
    ("module set", 1, "modules"),
    ("source enable", 1, "sources"),
    ("source disable", 1, "sources"),
    ("source settings", 1, "sources"),
    ("source settings", 2, "games"),
    ("source set", 1, "sources"),
    ("login", 1, "sources"),
    ("library", 1, "sources"),
    ("scan", 1, "sources"),
    ("config get", 1, "CONFIG_KEYS"),
    ("config set", 1, "CONFIG_KEYS"),
    ("controller bind", 1, "families"),
    ("controller bind", 2, "buttons"),
    ("controller bind", 3, "press hold"),
    ("controller bind", 4, "volume_up volume_down mute screenshot mangohud stop keys command"),
    ("controller unbind", 1, "families"),
    ("controller unbind", 2, "buttons"),
    ("controller unbind", 3, "press hold"),
    ("controller learn", 1, "families"),
    ("controller learn", 2, "buttons"),
    ("controller forget", 1, "families"),
    ("controller forget", 2, "buttons"),
];

fn generate(dir: &std::path::Path) -> anyhow::Result<()> {
    use clap::CommandFactory;
    let mut cmd = Cli::command().disable_help_subcommand(true);
    cmd.build();
    let hidden: Vec<String> = cmd.get_subcommands().filter(|c| c.is_hide_set()).map(|c| format!("{}\"", c.get_name())).collect();
    fn valued_options(cmd: &clap::Command, out: &mut Vec<String>) {
        for a in cmd.get_arguments() {
            if let (Some(l), true) = (a.get_long(), a.get_action().takes_values()) {
                out.push(format!("--{l}"));
            }
        }
        cmd.get_subcommands().for_each(|c| valued_options(c, out));
    }
    let mut valued = Vec::new();
    valued_options(&cmd, &mut valued);
    valued.sort();
    valued.dedup();

    let mut buf = Vec::new();
    clap_complete::generate(clap_complete::shells::Fish, &mut cmd, "universe", &mut buf);
    let generated = String::from_utf8(buf)?;
    // clap_complete lists hidden subcommands for Fish; it also knows nothing of positionals' values.
    let mut fish: String = generated.lines().filter(|l| !hidden.iter().any(|h| l.contains(h.as_str()))).map(|l| format!("{l}\n")).collect();
    fish.push_str("\n# positionals\n");
    fish.push_str("complete -c universe -f\n");
    fish.push_str("# `__universe_at 'media set' 2`: the cursor is on the 2nd positional after `media <name> set` (`2+`: the 2nd or later); options and their values are skipped\n");
    fish.push_str(&format!(
        concat!(
            "function __universe_at\n",
            "    set -l want (string split ' ' -- $argv[1])\n",
            "    set -l n (string trim -r -c + -- $argv[2])\n",
            "    set -l seen 0\n",
            "    set -l pos 0\n",
            "    set -l skip 0\n",
            "    for t in (commandline -opc)[2..]\n",
            "        if test $skip = 1; set skip 0; continue; end\n",
            "        if string match -q -- '-*' $t\n",
            "            contains -- $t {valued}; and set skip 1\n",
            "            continue\n",
            "        end\n",
            "        set -l next (math $seen + 1)\n",
            "        if test $seen -lt (count $want); and test \"$t\" = \"$want[$next]\"\n",
            "            set seen $next\n",
            "            set pos 0\n",
            "        else if test $seen = 0\n",
            "            return 1\n",
            "        else\n",
            "            set pos (math $pos + 1)\n",
            "        end\n",
            "    end\n",
            "    test $seen = (count $want); or return 1\n",
            "    if string match -q '*+' -- $argv[2]; test $pos -ge (math $n - 1); else; test $pos = (math $n - 1); end\n",
            "end\n",
        ),
        valued = valued.join(" ")
    ));
    for (path, n, what) in POSITIONALS {
        let cond = format!("__universe_at '{path}' {n}");
        match *what {
            "FILES" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -F\n")),
            "CONFIG_KEYS" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"{}\"\n", config_keys().join(" "))),
            "SLOTS" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"{}\"\n", MEDIA_SLOTS.join(" "))),
            "games" | "sources" | "modules" | "families" | "buttons" | "runners" => {
                fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"(universe __complete {what})\"\n"))
            }
            "games all" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"(universe __complete games) all\"\n")),
            literal => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"{literal}\"\n")),
        }
    }
    fish.push_str(&format!("complete -c universe -n \"__universe_at set 2+\" -f -a \"{}\"\n", game_keys().join(" ")));
    fish.push_str("complete -c universe -n \"__fish_seen_subcommand_from search install\" -l source -x -a \"(universe __complete sources)\"\n");
    fish.push_str("complete -c universe -n \"__fish_seen_subcommand_from add\" -s r -l runner -x -a \"(universe __complete runners)\"\n");
    fish.push_str("complete -c universe -n \"__fish_seen_subcommand_from module\" -l game -x -a \"(universe __complete games)\"\n");
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("universe.fish"), fish)?;
    for (shell, file) in [(clap_complete::Shell::Bash, "universe.bash"), (clap_complete::Shell::Zsh, "_universe")] {
        let mut buf = Vec::new();
        clap_complete::generate(shell, &mut cmd, "universe", &mut buf);
        std::fs::write(dir.join(file), buf)?;
    }

    let man = dir.join("man");
    std::fs::create_dir_all(&man)?;
    clap_mangen::generate_to(cmd, &man)?;
    Ok(())
}

fn keeps_old_path(owner: &str, from: &str) -> Option<String> {
    let who = match owner {
        "lutris" => "Lutris",
        "elsewhere" => "Any other launcher using it",
        _ => return None,
    };
    Some(format!("{who} keeps pointing at the old path, {from}."))
}

fn files_text(n: usize) -> String {
    if n == 1 {
        "1 file".into()
    } else {
        format!("{n} files")
    }
}

fn cloud_text(cloud: &Value) -> String {
    let s = |k: &str| cloud[k].as_str().unwrap_or_default().to_string();
    let state = match s("state").as_str() {
        "" => "never synced".to_string(),
        "synced" => format!("synced {}", s("at")),
        other => other.to_string(),
    };
    let off = if cloud["enabled"].as_bool() == Some(true) { "" } else { " (off: sessions do not sync)" };
    let message = if s("message").is_empty() { String::new() } else { format!(": {}", s("message")) };
    format!("{state}{off}{message}")
}

fn backup_text(done: &crate::saves::Outcome) -> String {
    match done.change.as_str() {
        "same" => "unchanged since the last backup".into(),
        "none" => "no saves found".into(),
        _ => format!("backed up, {} ({})", files_text(done.files.len()), human_size(done.bytes)),
    }
}

fn size_of(v: &Value) -> String {
    human_size(v["bytes"].as_u64().unwrap_or(0))
}

fn print_data(d: &Value) {
    println!("{}  {}  {}", s(d, "title").bold(), s(d, "id").dimmed(), format!("total {}", human_size(d["total"].as_u64().unwrap_or(0))).dimmed());
    let line = |label: &str, v: &Value, extra: String| {
        if v.is_null() {
            return;
        }
        let missing = if v["exists"] == false { " (missing)".dimmed().to_string() } else { String::new() };
        println!("  {label:<11} {:>8}  {}{missing}{extra}", size_of(v), s(v, "path"));
    };
    line("install", &d["install"], format!("  {}", s(&d["install"], "owner").dimmed()));
    let p = &d["prefix"];
    let shared = joined(&p["shared_with"], ", ");
    let shared = if shared.is_empty() { String::new() } else { format!(" · shared with {shared}") };
    line("prefix", p, format!("  {}", format!("{}{shared}", s(p, "owner")).dimmed()));
    let saves = &d["saves"];
    let backups = saves["backups"].as_array().map(Vec::len).unwrap_or(0);
    let engine = match s(saves, "engine").as_str() {
        "" => format!("every game's saves: {}", s(saves, "folder")),
        _ if !s(saves, "error").is_empty() => s(saves, "error"),
        _ => format!("{} · {}", files_text(saves["files"].as_array().map(Vec::len).unwrap_or(0)), size_of(saves)),
    };
    println!(
        "  {:<11} {:>8}  {}  {}",
        "saves",
        human_size(saves["backups_bytes"].as_u64().unwrap_or(0)),
        s(saves, "dir"),
        format!("{backups} backups · {engine}").dimmed()
    );
    let parts = &d["universe"]["parts"];
    let parts: Vec<String> =
        ["media", "screenshots", "journal", "sessions"].iter().map(|k| format!("{k} {}", human_size(parts[k].as_u64().unwrap_or(0)))).collect();
    line("universe", &d["universe"], format!("  {}", parts.join(" · ").dimmed()));
    line("recordings", &d["recordings"], if d["recordings"]["archived"] == true { format!("  {}", "archived".dimmed()) } else { String::new() });
    line("logs", &d["logs"], String::new());
}

fn print_saves(saves: &Value, loc: &Locale) {
    if !s(saves, "error").is_empty() {
        println!("{}", s(saves, "error").yellow());
    }
    if s(saves, "engine").is_empty() {
        println!("every game's saves are in {}: no backup of this one alone", s(saves, "folder"));
    }
    if !s(saves, "name").is_empty() {
        println!("ludusavi knows it as {}", s(saves, "name").bold());
    }
    let mut t = table(&["Save", "Size"]);
    for f in saves["files"].as_array().into_iter().flatten() {
        t.add_row(vec![s(f, "path"), size_of(f)]);
    }
    println!("{t}");
    let mut t = table(&["Backup", "When", "Size"]);
    for b in saves["backups"].as_array().into_iter().flatten() {
        t.add_row(vec![s(b, "id"), when(&s(b, "when"), loc), size_of(b)]);
    }
    println!("{t}");
    println!("{}", format!("kept in {} · the last {} · after each session: {}", s(saves, "dir"), saves["keep"], flag(&saves["auto"])).dimmed());
    if saves["cloud"].is_object() {
        println!("cloud saves: {}", cloud_text(&saves["cloud"]));
    }
}

fn print_storage(st: &Value) {
    let mut t = table(&["Folder", "Path", "Used", "Free"]);
    for r in st["roots"].as_array().into_iter().flatten() {
        let free = r["free"].as_u64().unwrap_or(0);
        t.add_row(vec![s(r, "id"), s(r, "path"), size_of(r), if free > 0 { human_size(free) } else { String::new() }]);
    }
    println!("{t}");
    let mut t = table(&["Game", "Total", "Install", "Prefix", "Saves", "Universe", "Recordings"]);
    for g in st["games"].as_array().into_iter().flatten() {
        let part = |k: &str| human_size(g[k].as_u64().unwrap_or(0));
        t.add_row(vec![s(g, "id"), size_of(g), part("install"), part("prefix"), part("saves"), part("universe"), part("recordings")]);
    }
    println!("{t}");
    let leftovers = st["leftovers"].as_array().cloned().unwrap_or_default();
    if leftovers.is_empty() {
        println!("no leftovers");
        return;
    }
    let mut t = table(&["Leftover", "Path", "Size", "Game"]);
    for l in &leftovers {
        t.add_row(vec![s(l, "kind"), s(l, "path"), size_of(l), s(l, "title")]);
    }
    println!("{t}");
    println!(
        "{}",
        format!("{} in leftovers: `universe storage trash <path>` puts one in the trash", human_size(st["leftover_bytes"].as_u64().unwrap_or(0))).dimmed()
    );
}

fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1_000_000_000 => format!("{:.1} GB", b as f64 / 1e9),
        b if b >= 1_000_000 => format!("{:.0} MB", b as f64 / 1e6),
        b => format!("{:.0} kB", b as f64 / 1e3),
    }
}

fn flag(v: &Value) -> String {
    match v.as_bool() {
        Some(true) => "✓".into(),
        Some(false) => "".into(),
        None => "?".into(),
    }
}

fn fmt_duration(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{h}h{m:02}")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

async fn enabled_source(core: &Core, source: Option<String>) -> anyhow::Result<String> {
    if let Some(s) = source {
        return Ok(s);
    }
    let active = core.active_source_ids().await;
    match active.as_slice() {
        [only] => Ok(only.clone()),
        [] => anyhow::bail!("no source is enabled (universe source enable <id>)"),
        _ => anyhow::bail!("{} are enabled: name the source", active.join(", ")),
    }
}

async fn pick(core: &Core, name: &str) -> anyhow::Result<String> {
    let ids = core.resolve(name).await;
    match ids.len() {
        0 => anyhow::bail!("no game matches '{name}'"),
        1 => Ok(ids[0].clone()),
        _ => {
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                anyhow::bail!("'{name}' is ambiguous: {}", ids.join(", "));
            }
            for (i, id) in ids.iter().enumerate() {
                println!("  {} {id}", i + 1);
            }
            let n: usize = ask(&format!("which one? [1-{}]", ids.len())).parse().unwrap_or(0);
            ids.get(n.wrapping_sub(1)).cloned().ok_or_else(|| anyhow::anyhow!("no choice"))
        }
    }
}
