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
  ~/.config/universe/config.toml — paths, launch defaults, enabled modules, [modules.<id>] settings, keys
  ~/.local/share/universe/games/<id>/ — game.toml, sessions.jsonl, journal/, media/
  ~/.config/universe/modules/<id>/ — user modules, overriding the shipped ones
  ~/.local/share/universe/modules/<id>/ — module data: caches, logins
  ~/.local/state/universe/current-session.json — the running session
  $XDG_RUNTIME_DIR/universe/controller.lock — held by the one controller watcher (the launcher's, or a session's)

Environment:
  UNIVERSE_CONFIG_HOME, UNIVERSE_DATA_HOME, UNIVERSE_STATE_HOME — replace the XDG directories
  UNIVERSE_MODULES_PATH — extra module roots, colon-separated
  RUST_LOG — tracing filter, warn by default

Errors are printed as `universe: <kind>: <message>` on stderr with exit status 1; --json works on every command.";

#[derive(Parser, Debug)]
#[command(name = "universe", version, about = "Universe: launch, record and remember your games", after_long_help = AFTER_HELP)]
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
        /// Source module
        #[arg(long, default_value = "gog")]
        source: String,
    },
    /// Install a title from a source
    Install {
        /// Id in the source's catalogue
        id: String,
        /// Source module
        #[arg(long, default_value = "gog")]
        source: String,
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
    /// Journal entries of a game
    Journal {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// Render the Markdown note and print its path
        #[arg(long)]
        render: bool,
        /// Render, then open the note with xdg-open
        #[arg(long)]
        open: bool,
        /// Trash the entry of this session (a pending one is cancelled)
        #[arg(long, value_name = "SESSION")]
        remove: Option<String>,
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
    /// Sources and their state
    Sources,
    /// Log into a source (prints the URL, then takes the code)
    Login {
        /// Source module
        source: String,
        /// The code= value from the address bar after logging in
        code: Option<String>,
    },
    /// Owned titles of a source
    Library {
        /// Source module
        #[arg(default_value = "gog")]
        source: String,
        /// Fetch again instead of reading the cache
        #[arg(long)]
        refresh: bool,
    },
    /// Scan installed games of the sources (all by default)
    Scan {
        /// Source module; every source when omitted
        source: Option<String>,
    },
    /// Import the Lutris library (report only without --apply)
    Migrate {
        /// Write the games and their hours instead of reporting
        #[arg(long)]
        apply: bool,
    },
    /// Check prerequisites of the core and enabled modules
    Doctor,
    /// Global config
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Take a screenshot through the module that provides one
    Screenshot,
    /// Controller macros: paddles and spare buttons bound to actions
    Controller {
        #[command(subcommand)]
        action: ControllerCmd,
    },
    /// Reload config and rescan the library
    Rescan,
    /// Close a session: run by systemd's ExecStopPost when the game's cgroup empties
    #[command(name = "session-end", hide = true)]
    SessionEnd { id: String, session: String },
    /// File a recording under the session (capture module's session-end hook)
    #[command(name = "recording-file", hide = true)]
    RecordingFile { session: String, path: String },
    /// Add a journal entry to the session (journal module's post-process hook)
    #[command(name = "journal-add", hide = true)]
    JournalAdd { session: String, entry: String },
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
    /// Completion candidates for the shell: games | sources | modules
    #[command(name = "__complete", hide = true)]
    Complete { what: String },
    /// Write the Fish completions and the man pages under <dir>
    #[command(name = "__generate", hide = true)]
    Generate { dir: std::path::PathBuf },
}

const MEDIA_SLOTS: [&str; 6] = ["box_front", "square", "banner", "background", "logo", "screenshot"];

#[derive(Subcommand, Debug)]
pub enum MediaCmd {
    /// Fetch artwork from SteamGridDB, RAWG and Steam
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
        /// Image file or http(s) URL, placed under paths.overrides/<id>/
        source: String,
    },
    /// Remove a slot's override, so it shows the fetched default again
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
    Search {
        query: Vec<String>,
    },
    /// Pin the game to a provider id
    Pin {
        #[arg(value_parser = ["sgdb", "rawg", "steam"])]
        provider: String,
        /// The game's id at the provider
        id: String,
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
    /// Set a runner's global keys: exe=/path args="--flag" fullscreen=false inputplumber=false (an empty value resets)
    Set {
        /// Runner id
        id: String,
        /// key=value, validated against the runner's options
        pairs: Vec<String>,
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
    t.load_preset(UTF8_FULL_CONDENSED);
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

fn hours(v: &Value) -> String {
    let h = v["stats"]["hours"].as_f64().unwrap_or(0.0);
    if h == 0.0 { String::new() } else { format!("{h:.1}") }
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

fn when(ts: &str, loc: &Locale) -> String {
    local(ts).map(|t| loc.datetime(&t)).unwrap_or_else(|| ts.to_string())
}

/// A list as the text arms read it: `Value` rows, whatever the core's type.
fn rows(v: &impl serde::Serialize) -> Vec<Value> {
    serde_json::to_value(v).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

fn print_json(v: &impl serde::Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn ask(prompt: &str) -> String {
    use std::io::Write;
    print!("{prompt} ");
    let _ = std::io::stdout().flush();
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

fn finish(json: bool, r: crate::Result<String>) {
    match r {
        Ok(m) => report(json, true, &m),
        Err(e) => {
            report(json, false, &e.to_string());
            std::process::exit(1);
        }
    }
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
        Cmd::RecordingFile { session, path } => println!("{}", core.file_recording(&session, &path).await?),
        Cmd::JournalAdd { session, entry } => core.add_entry(&session, serde_json::from_str(&entry).map_err(crate::Error::from)?).await?,
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
                t.add_row(vec![title, Cell::new(s(&g["effective"], "runner")), Cell::new(s(&g["source"], "kind")), Cell::new(hours(&g)), Cell::new(day(&s(&g["stats"], "last_played"), &loc))]);
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
                        println!("{} after {}", "ended".yellow(), fmt_duration(s.duration_s));
                    }
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            anyhow::bail!("session {sid} ended but was not closed; check `journalctl --user -u {unit}`");
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
            let mut t = table(&["Session", "Game", "Duration", "Source", "Recording"]);
            for r in recent {
                t.add_row(vec![s(&r, "session"), s(&r, "title"), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), s(&r, "source"), if r["recording"].is_null() { String::new() } else { "✓".into() }]);
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
            println!("  source     {} {}  build {}", s(&g["source"], "kind"), s(&g["source"], "gog_id"), s(&g["source"], "build_id"));
            println!("  runner     {} ({}) · {}", s(&g["effective"], "runner"), s(&g["effective"], "runner_name"), if s(&g["effective"], "runner_path").is_empty() { "not found".to_string() } else { s(&g["effective"], "runner_path") });
            println!("  platform   {}", s(&g, "platform"));
            println!("  exe        {}", s(&g["launch"], "exe"));
            if s(&g["effective"], "runner_kind") == "emulator" {
                println!("  options    {}", g["effective"]["options"]);
            } else {
                println!("  prefix     {}", s(&g["launch"], "prefix"));
                println!("  proton     {} ({})", s(&g["effective"], "proton"), s(&g["effective"], "proton_path"));
            }
            println!("  esync/fsync/ntsync/mangohud  {}/{}/{}/{}", g["effective"]["esync"], g["effective"]["fsync"], g["effective"]["ntsync"], g["effective"]["mangohud"]);
            let on: Vec<&str> = ["wayland", "hdr", "dlss_upgrade", "fsr4_upgrade", "xess_upgrade", "optiscaler"].into_iter().filter(|k| g["effective"][k].as_bool() == Some(true)).collect();
            println!("  switches   {}{}", if on.is_empty() { "no switches on".to_string() } else { on.join(" ") }, if s(&g["launch"], "wrapper").is_empty() { String::new() } else { format!(" · wrapper {}", s(&g["launch"], "wrapper")) });
            println!("  gamescope  {} {}", g["effective"]["gamescope"], s(&g["effective"], "gamescope_args"));
            println!("  fps limit  {}", s(&g["effective"], "fps_limit"));
            println!("  env        {}", g["effective"]["env"]);
            println!("  hours      {}  plays {}  last {}", hours(&g), g["stats"]["play_count"], when(&s(&g["stats"], "last_played"), &loc));
            println!("  media      {}", g["media"]);
            println!("  modules    {}", g["modules"]);
            println!("  journal    {} entries · recordings {}", g["journal_count"], g["recording_count"]);
        }
        Cmd::Search { query, source } => {
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
            if !json {
                println!("installing {id} from {source}");
            }
            let mut p = progress_printer(json);
            finish(json, core.source_install(&source, &id, Some(&mut p)).await);
        }
        Cmd::Update { name, yes } => {
            match name {
                Some(n) => {
                    let id = pick(&core, &n).await?;
                    let g = core.get(&id).await?.to_json();
                    let gid = s(&g["source"], "gog_id");
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
            }
        }
        Cmd::Rm { name, yes, purge } => {
            let id = pick(&core, &name).await?;
            if yes || confirm(&format!("remove {id}{}?", if purge { " and trash its prefix" } else { "" })) {
                core.remove(&id, purge).await?;
                println!("removed {id}");
            }
        }
        Cmd::Uninstall { name, yes } => {
            let id = pick(&core, &name).await?;
            if yes || confirm(&format!("trash the install folder of {id}?")) {
                core.uninstall(&id).await?;
                println!("uninstalled {id}");
            }
        }
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
                    println!("{}", format!("{} was not found: install it or `universe runner set {} exe=…`", g.effective.runner_name, g.effective.runner).yellow());
                }
            }
        }
        Cmd::Runner { action } => return runner(core, action, json).await,
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
            let mut t = table(&["Session", "Started", "Duration", "Source", "Journal", "Recording"]);
            for r in rows(&list) {
                t.add_row(vec![s(&r, "session"), when(&s(&r, "started_at"), &loc), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), s(&r, "source"), s(&r["journal"], "state"), s(&r["recording"], "path")]);
            }
            println!("{t}");
        }
        Cmd::Journal { name, render, open, remove, yes } => {
            let id = pick(&core, &name).await?;
            if let Some(session) = remove {
                if yes || confirm(&format!("trash the journal entry {session} of {id}?")) {
                    core.remove_journal_entry(&id, &session).await?;
                    println!("removed {session}");
                }
                return Ok(());
            }
            if render || open {
                let path = core.render_journal(&id).await?;
                println!("{path}");
                if open {
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
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
                        let entry = match (s(&g, "sgdb_name"), g["sgdb_id"].as_u64().unwrap_or(0)) {
                            (name, id) if !name.is_empty() => format!("{name} ({id})"),
                            (_, 0) => String::new(),
                            (_, id) => id.to_string(),
                        };
                        for slot in g["slots"].as_array().cloned().unwrap_or_default() {
                            t.add_row(vec![s(&g, "id"), entry.clone(), s(&slot, "slot"), s(&slot, "kind"), s(&slot, "origin"), s(&slot, "path"), if s(&slot, "override").is_empty() { String::new() } else { s(&slot, "default") }]);
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
                        t.add_row(vec![h["id"].to_string(), s(&h, "name"), year, if h["verified"].as_bool().unwrap_or(false) { "verified".into() } else { String::new() }, if h["current"].as_bool().unwrap_or(false) { "current".into() } else { String::new() }]);
                    }
                    println!("{t}");
                }
                MediaCmd::Pin { provider, id: pid } => {
                    core.media_pin(&id, &provider, &pid).await?;
                    println!("pinned");
                }
            }
        }
        Cmd::Module { action } => {
            match action {
                ModuleCmd::Ls => {
                    let list = core.modules().await;
                    if json {
                        return print_json(&list);
                    }
                    let mut t = table(&["Id", "Name", "Kind", "Enabled", "Available", "Missing", "Hooks"]);
                    for m in list {
                        let hooks: Vec<String> = m["hooks"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                        t.add_row(vec![s(&m, "id"), s(&m, "name"), joined(&m["kind"], ","), flag(&m["enabled"]), flag(&m["available"]), joined(&m["missing"], ","), hooks.join(",")]);
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
                    let gid = match game { Some(g) => pick(&core, &g).await?, None => String::new() };
                    print_json(&core.module_settings(&id, &gid).await?)?;
                }
                ModuleCmd::Set { id, pairs, game } => {
                    let gid = match game { Some(g) => pick(&core, &g).await?, None => String::new() };
                    for p in &pairs {
                        let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value"))?;
                        core.set_module_setting(&id, &gid, k, v).await?;
                        println!("{id}.{k} = {v}{}", if gid.is_empty() { String::new() } else { format!(" ({gid})") });
                    }
                }
            }
        }
        Cmd::Sources => {
            let list = core.sources().await;
            if json {
                print_json(&list)?;
            } else {
                let mut t = table(&["Id", "Name", "Enabled", "Available", "Library (cached)", "Games dir"]);
                for m in list {
                    t.add_row(vec![s(&m, "id"), s(&m, "name"), flag(&m["enabled"]), flag(&m["available"]), m["library_cached"].to_string(), s(&m, "games_dir")]);
                }
                println!("{t}");
            }
        }
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
            let list = core.source_library(&source, refresh).await?;
            if json {
                return print_json(&list);
            }
            let mut t = table(&["Id", "Title", "Installed", "Dir"]);
            for g in list {
                t.add_row(vec![s(&g, "id"), s(&g, "title"), flag(&g["installed"]), s(&g, "dir")]);
            }
            println!("{t}");
        }
        Cmd::Scan { source } => {
            let mut p = progress_printer(json);
            let n = core.source_scan(source.as_deref().unwrap_or(""), Some(&mut p)).await?;
            report(json, true, &format!("{n} game(s)"));
            if !json {
                let mut t = table(&["Title", "Source", "Id", "Build"]);
                for g in core.list().await {
                    if s(&g["source"], "kind") == source.clone().unwrap_or_else(|| "gog".into()) {
                        t.add_row(vec![s(&g, "title"), s(&g["source"], "gog_id"), s(&g, "id"), s(&g["source"], "build_id")]);
                    }
                }
                println!("{t}");
            }
        }
        Cmd::Migrate { apply } => {
            let report = serde_json::to_value(core.import_lutris(apply).await?)?;
            if json {
                return print_json(&report);
            }
            let n = |k: &str| report[k].as_array().map(|a| a.len()).unwrap_or(0);
            println!("{} imported {}, skipped (already present) {}, updated {}, media from pegasus-library {}", if apply { "applied:" } else { "dry run:" }, n("imported"), n("skipped"), n("updated"), n("media_imported"));
            if let Some(h) = report["hours_imported"].as_object() {
                for (k, v) in h {
                    println!("  hours {k}: {v}");
                }
            }
            if n("backend_promoted") > 0 {
                println!("  backend: {} pre-runner game(s) {} launch.runner instead of backend: {}", n("backend_promoted"), if apply { "now have" } else { "would have" }, joined(&report["backend_promoted"], ", "));
            }
            if n("options_promoted") > 0 {
                println!("  options: {} {} what Lutris's prefix command and PROTON_* env now have fields for: {}", n("options_promoted"), if apply { "game(s) took" } else { "game(s) would take" }, joined(&report["options_promoted"], ", "));
            }
            for h in report["runners"].as_array().cloned().unwrap_or_default() {
                println!("  [runners.{}] {} {}{}", s(&h, "runner"), s(&h, "program"), joined(&h["args"], " "), if s(&h, "wrapped") == "true" { " (Lutris wrapper dropped)".dimmed().to_string() } else { String::new() });
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
        Cmd::Doctor => {
            let list = core.doctor().await;
            if json {
                return print_json(&list);
            }
            let mut bad = 0;
            for c in rows(&list) {
                let ok = c["ok"].as_bool().unwrap_or(false);
                if !ok {
                    bad += 1;
                }
                println!("{} {:<18} {:<10} {}", if ok { "✓".green().to_string() } else { "✗".red().to_string() }, s(&c, "check"), s(&c, "module").dimmed(), s(&c, "detail"));
            }
            if bad > 0 {
                println!("{bad} problem(s)");
                std::process::exit(1);
            }
            println!("{}", "all good".green());
        }
        Cmd::Config { action } => {
            match action {
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
            }
        }
        Cmd::Screenshot => println!("{}", core.screenshot().await?),
        Cmd::Controller { action } => return controller(core, action, json).await,
        Cmd::Rescan => {
            core.reload_config().await?;
            println!("rescanned");
        }
        Cmd::Complete { .. } | Cmd::Generate { .. } | Cmd::Splash { .. } | Cmd::LaunchKeys => unreachable!(),
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
                    let of = |t: &str| macros.iter().find(|m| m.trigger == t).map(|m| if m.keys.is_empty() && m.command.is_empty() { m.action.clone() } else { format!("{} {}{}", m.action, m.keys, m.command) }).unwrap_or_default();
                    let code = if entry["bound"] == true { s(entry, "code") } else { "unbound".to_string() };
                    t.add_row(vec![Cell::new(label), if entry["bound"] == true { Cell::new(code) } else { Cell::new(code).add_attribute(Attribute::Dim) }, Cell::new(of("press")), Cell::new(of("hold"))]);
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
                let program = if s(&r, "path").is_empty() { if s(&r, "kind") == "linux" { "the game itself".to_string() } else { "not found".to_string() } } else { s(&r, "path") };
                let program = if r["available"].as_bool() == Some(true) { Cell::new(program) } else { Cell::new(program).add_attribute(Attribute::Dim) };
                t.add_row(vec![Cell::new(s(&r, "id")), Cell::new(s(&r, "name")), Cell::new(joined(&r["platforms"], ", ")), program, Cell::new(s(&r, "source"))]);
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
        "sources" | "modules" => {
            let config = crate::config::Config::load()?;
            for m in crate::modules::discover(&config) {
                if what == "sources" && !m.is_source() {
                    continue;
                }
                println!("{}\t{}", m.id(), m.manifest.name);
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
    let mut keys: Vec<String> = LAUNCH_KEYS.iter().filter(|k| k.scope != Scope::Global).map(|k| if k.kind == Kind::Map { format!("{}.", k.key) } else { format!("{}=", k.key) }).collect();
    keys.extend(["hide_cursor=", "hidden=", "favorite=", "tags=", "sort_title=", "platform=", "metadata.sgdb_id=", "capture.cursor="].map(String::from));
    keys
}

fn config_keys() -> Vec<String> {
    use crate::launch_keys::{Kind, Scope, LAUNCH_KEYS};
    let mut keys: Vec<String> = ["paths.games_root", "paths.prefixes_root", "paths.recordings_root", "paths.journal_root", "paths.overrides"].map(String::from).to_vec();
    keys.extend(LAUNCH_KEYS.iter().filter(|k| k.scope != Scope::Game).map(|k| if k.kind == Kind::Map { format!("launch.{}.", k.key) } else { format!("launch.{}", k.key) }));
    keys.extend(
        [
            "runners.", "desktop.profile", "desktop.hide_cursor", "desktop.cursor_extension", "keys.sgdb", "keys.sgdb_file", "keys.rawg", "keys.rawg_file",
            "controller.enabled", "controller.hold_ms", "controller.volume_step",
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
    ("sessions", 1, "games"),
    ("journal", 1, "games"),
    ("recordings", 1, "games"),
    ("update", 1, "games"),
    ("media", 1, "games all"),
    ("media set", 1, "SLOTS"),
    ("media set", 2, "FILES"),
    ("media unset", 1, "SLOTS"),
    ("media candidates", 1, "SLOTS"),
    ("media pin", 1, "sgdb rawg steam"),
    ("module enable", 1, "modules"),
    ("module disable", 1, "modules"),
    ("module settings", 1, "modules"),
    ("module settings", 2, "games"),
    ("module set", 1, "modules"),
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
            "games" | "sources" | "modules" | "families" | "buttons" | "runners" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"(universe __complete {what})\"\n")),
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

    let man = dir.join("man");
    std::fs::create_dir_all(&man)?;
    clap_mangen::generate_to(cmd, &man)?;
    Ok(())
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
