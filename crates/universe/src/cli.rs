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
A game runs as the transient unit universe-game-<id>-<session>.service; `universe session-end` closes it when its cgroup empties.

Files:
  ~/.config/universe/config.toml — paths, launch defaults, enabled modules, [modules.<id>] settings, keys
  ~/.local/share/universe/games/<id>/ — game.toml, sessions.jsonl, journal/, media/
  ~/.config/universe/modules/<id>/ — user modules, overriding the shipped ones
  ~/.local/share/universe/modules/<id>/ — module data: caches, logins
  ~/.local/state/universe/current-session.json — the running session
  $XDG_RUNTIME_DIR/universe/controller.lock — held by the one controller watcher (the launcher's, or a session's)

Environment:
  UNIVERSE_CONFIG_HOME, UNIVERSE_DATA_HOME, UNIVERSE_STATE_HOME, UNIVERSE_CACHE_HOME — replace the XDG directories
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
        /// Source module
        #[arg(long, default_value = "gog")]
        source: String,
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
    /// Set game keys: proton=proton-em capture.cursor=true hidden=true
    Set {
        /// Game: exact id, then whole word, substring or path
        name: String,
        /// key=value; launch keys (proton, exe, prefix, args…) need no `launch.` prefix
        pairs: Vec<String>,
    },
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
    },
    /// Recordings of a game
    Recordings {
        /// Game: exact id, then whole word, substring or path
        name: String,
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
    #[command(alias = "gog-scan")]
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
    /// GOG shortcuts
    Gog {
        #[command(subcommand)]
        verb: GogCmd,
    },
    /// Completion candidates for the shell: games | sources | modules
    #[command(name = "__complete", hide = true)]
    Complete { what: String },
    /// Write the Fish completions and the man pages under <dir>
    #[command(name = "__generate", hide = true)]
    Generate { dir: std::path::PathBuf },
}

const MEDIA_SLOTS: [&str; 5] = ["box_front", "tile", "background", "logo", "screenshot"];

#[derive(Subcommand, Debug)]
pub enum MediaCmd {
    /// Fetch artwork from SteamGridDB, RAWG and Steam
    Refresh {
        /// Fetch again even when every slot is filled
        #[arg(long)]
        force: bool,
    },
    /// Copy a file into a slot
    Set {
        #[arg(value_parser = MEDIA_SLOTS)]
        slot: String,
        /// Image file, copied into the game's media directory
        path: std::path::PathBuf,
    },
    /// Clear a slot
    Unset {
        #[arg(value_parser = MEDIA_SLOTS)]
        slot: String,
    },
    /// List the candidates of a slot
    Candidates {
        #[arg(default_value = "box_front", value_parser = MEDIA_SLOTS)]
        slot: String,
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

#[derive(Subcommand, Debug)]
pub enum GogCmd {
    /// Cross installed folders with the owned library
    Scan,
    /// Owned titles
    Library {
        /// Fetch again instead of reading the cache
        #[arg(long)]
        refresh: bool,
    },
    /// Search the catalogue
    Search {
        /// Search terms
        query: Vec<String>,
    },
    /// Log in (prints the URL, then takes the code)
    Login {
        /// The code= value from the address bar after logging in
        code: Option<String>,
    },
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

fn when(ts: &str, loc: &Locale) -> String {
    local(ts).map(|t| loc.datetime(&t)).unwrap_or_else(|| ts.to_string())
}

fn parse_json(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn confirm(prompt: &str) -> bool {
    use std::io::Write;
    print!("{prompt} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    s.trim().to_lowercase().starts_with('y')
}

fn progress_printer(json: bool) -> impl FnMut(u64, u64, &str) {
    move |done, total, message| {
        if !json {
            let pct = if total > 0 { format!("{:3.0}%", done as f64 * 100.0 / total as f64) } else { "    ".into() };
            eprintln!("  {pct} {message}");
        }
    }
}

fn report(json: bool, ok: bool, message: &str) {
    if json {
        print_json(&serde_json::json!({"ok": ok, "message": message}));
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
        Cmd::JournalAdd { session, entry } => core.add_entry(&session, &entry).await?,
        Cmd::Ls { all } => {
            let list = parse_json(&core.list_json().await);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Title", "Source", "Hours", "Last played"]);
            for i in 1..=3 {
                t.column_mut(i).unwrap().set_constraint(ColumnConstraint::ContentWidth);
            }
            for g in list.as_array().cloned().unwrap_or_default() {
                if !all && g["hidden"].as_bool() == Some(true) {
                    continue;
                }
                let title = if g["favorite"].as_bool() == Some(true) { format!("★ {}", s(&g, "title")) } else { s(&g, "title") };
                let title = if g["hidden"].as_bool() == Some(true) { Cell::new(title).add_attribute(Attribute::Dim) } else { Cell::new(title) };
                t.add_row(vec![title, Cell::new(s(&g["source"], "kind")), Cell::new(hours(&g)), Cell::new(day(&s(&g["stats"], "last_played"), &loc))]);
            }
            println!("{t}");
        }
        Cmd::Play { name, screen, no_wait } => {
            let id = pick(&core, &name).await?;
            // The game is bound to this process's scope: it goes down with us, cleanly on Ctrl-C.
            if !no_wait {
                core.adopt_scope().await?;
            }
            let sid = core.launch(&id, &screen).await?;
            if json {
                print_json(&serde_json::json!({"session": sid, "id": id}));
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
            while launcher::is_active(&unit).await {
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
            let cur = core.current_json().await;
            let pending = parse_json(&core.pending_journals_json().await);
            let list = parse_json(&core.list_json().await);
            let mut recent: Vec<Value> = Vec::new();
            for g in list.as_array().cloned().unwrap_or_default() {
                for sess in parse_json(&core.sessions_json(&s(&g, "id")).await?).as_array().cloned().unwrap_or_default() {
                    let mut sess = sess;
                    sess["title"] = g["title"].clone();
                    recent.push(sess);
                }
            }
            recent.sort_by_key(|r| std::cmp::Reverse(s(r, "ended_at")));
            recent.truncate(10);
            if json {
                print_json(&serde_json::json!({"current": if cur.is_empty() { Value::Null } else { parse_json(&cur) }, "recent": recent, "pending_journals": pending}));
                return Ok(());
            }
            if cur.is_empty() {
                println!("{}", "no session running".dimmed());
            } else {
                let c = parse_json(&cur);
                println!("{} {} · session {} · {} · since {}", "running".green(), s(&c, "title"), s(&c, "session_id"), s(&c, "unit"), when(&s(&c, "started_at"), &loc));
            }
            for p in pending.as_array().cloned().unwrap_or_default() {
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
                print_json(&g);
                return Ok(());
            }
            println!("{}  {}", s(&g, "title").bold(), s(&g, "id").dimmed());
            println!("  source     {} {}  build {}", s(&g["source"], "kind"), s(&g["source"], "gog_id"), s(&g["source"], "build_id"));
            println!("  exe        {}", s(&g["launch"], "exe"));
            println!("  prefix     {}", s(&g["launch"], "prefix"));
            println!("  proton     {} ({})", s(&g["effective"], "proton"), s(&g["effective"], "proton_path"));
            println!("  esync/fsync/mangohud  {}/{}/{}", g["effective"]["esync"], g["effective"]["fsync"], g["effective"]["mangohud"]);
            println!("  env        {}", g["effective"]["env"]);
            println!("  hours      {}  plays {}  last {}", hours(&g), g["stats"]["play_count"], when(&s(&g["stats"], "last_played"), &loc));
            println!("  media      {}", g["media"]);
            println!("  modules    {}", g["modules"]);
            println!("  journal    {} entries · recordings {}", g["journal_count"], g["recording_count"]);
        }
        Cmd::Search { query, source } => {
            let list = parse_json(&core.source_search(&source, &query).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Id", "Title", "Owned", "Installed"]);
            for g in list.as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&g, "id"), s(&g, "title"), flag(&g["owned"]), flag(&g["installed"])]);
            }
            println!("{t}");
        }
        Cmd::Install { id, source } => {
            if !json {
                println!("installing {id} from {source}");
            }
            let mut p = progress_printer(json);
            match core.source_install(&source, &id, Some(&mut p)).await {
                Ok(gid) => report(json, true, &gid),
                Err(e) => {
                    report(json, false, &e.to_string());
                    std::process::exit(1);
                }
            }
        }
        Cmd::Update { name, yes, source } => {
            match name {
                Some(n) => {
                    let id = pick(&core, &n).await?;
                    let g = core.get(&id).await?.to_json();
                    let gid = s(&g["source"], "gog_id");
                    if gid.is_empty() {
                        anyhow::bail!("{id} has no source id");
                    }
                    let mut p = progress_printer(json);
                    match core.source_update(&s(&g["source"], "kind"), &gid, Some(&mut p)).await {
                        Ok(n) => report(json, true, &format!("{n} updated")),
                        Err(e) => {
                            report(json, false, &e.to_string());
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    let pending = parse_json(&core.source_updates().await?);
                    if json {
                        print_json(&pending);
                        return Ok(());
                    }
                    let list = pending.as_array().cloned().unwrap_or_default();
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
                            let src = if s(u, "source").is_empty() { source.clone() } else { s(u, "source") };
                            match core.source_update(&src, &s(u, "id"), Some(&mut p)).await {
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
        Cmd::Set { name, pairs } => {
            let id = pick(&core, &name).await?;
            for p in pairs {
                let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value, got {p}"))?;
                let k = match k {
                    "proton" | "exe" | "prefix" | "args" | "working_dir" | "esync" | "fsync" | "mangohud" | "umu_id" | "store" | "pre_command" | "post_command" | "arch" | "backend" => format!("launch.{k}"),
                    "hide_cursor" => "desktop.hide_cursor".into(),
                    _ => k.to_string(),
                };
                core.set(&id, &k, v).await?;
                println!("{id}: {k} = {v}");
            }
        }
        Cmd::Sessions { name } => {
            let id = pick(&core, &name).await?;
            let list = parse_json(&core.sessions_json(&id).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Session", "Started", "Duration", "Source", "Recording"]);
            for r in list.as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&r, "session"), when(&s(&r, "started_at"), &loc), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), s(&r, "source"), s(&r, "recording")]);
            }
            println!("{t}");
        }
        Cmd::Journal { name, render, open } => {
            let id = pick(&core, &name).await?;
            if render || open {
                let path = core.render_journal(&id).await?;
                println!("{path}");
                if open {
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                }
                return Ok(());
            }
            let list = parse_json(&core.journal_json(&id).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            for e in list.as_array().cloned().unwrap_or_default() {
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
        Cmd::Recordings { name } => {
            let id = pick(&core, &name).await?;
            let list = parse_json(&core.recordings_json(&id).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Session", "Duration", "Size", "Path"]);
            for r in list.as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&r, "session"), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), format!("{:.1} G", r["size"].as_u64().unwrap_or(0) as f64 / 1e9), s(&r, "path")]);
            }
            println!("{t}");
        }
        Cmd::Media { name, action } => {
            match action {
                MediaCmd::Refresh { force } => {
                    let id = if name == "all" { String::new() } else { pick(&core, &name).await? };
                    let mut p = progress_printer(json);
                    let (changed, total) = core.media_refresh(&id, force, Some(&mut p)).await?;
                    report(json, true, &format!("{changed}/{total} updated"));
                }
                MediaCmd::Set { slot, path } => {
                    let id = pick(&core, &name).await?;
                    let path = std::fs::canonicalize(path)?;
                    core.media_set_slot(&id, &slot, &path.to_string_lossy()).await?;
                    println!("{id}: {slot} set");
                }
                MediaCmd::Unset { slot } => {
                    let id = pick(&core, &name).await?;
                    core.media_unset(&id, &slot).await?;
                }
                MediaCmd::Candidates { slot } => {
                    let id = pick(&core, &name).await?;
                    let list = parse_json(&core.media_candidates(&id, &slot).await?);
                    print_json(&list);
                }
                MediaCmd::Pin { provider, id: pid } => {
                    let id = pick(&core, &name).await?;
                    core.media_pin(&id, &provider, &pid).await?;
                    println!("pinned");
                }
            }
        }
        Cmd::Module { action } => {
            match action {
                ModuleCmd::Ls => {
                    let list = parse_json(&core.modules_json().await);
                    if json {
                        print_json(&list);
                        return Ok(());
                    }
                    let mut t = table(&["Id", "Name", "Kind", "Enabled", "Available", "Missing", "Hooks"]);
                    for m in list.as_array().cloned().unwrap_or_default() {
                        let hooks: Vec<String> = m["hooks"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                        t.add_row(vec![s(&m, "id"), s(&m, "name"), m["kind"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_default(), flag(&m["enabled"]), flag(&m["available"]), m["missing"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_default(), hooks.join(",")]);
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
                    print_json(&parse_json(&core.module_settings_json(&id, &gid).await?));
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
            let list = parse_json(&core.sources_json().await);
            if json {
                print_json(&list);
            } else {
                let mut t = table(&["Id", "Name", "Enabled", "Available", "Library (cached)", "Games dir"]);
                for m in list.as_array().cloned().unwrap_or_default() {
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
                    use std::io::Write;
                    print!("code: ");
                    let _ = std::io::stdout().flush();
                    let mut s = String::new();
                    std::io::stdin().read_line(&mut s)?;
                    s.trim().to_string()
                }
            };
            let user = core.source_login(&source, &code).await?;
            report(json, true, &user);
        }
        Cmd::Library { source, refresh } => {
            let raw = core.source_library(&source, refresh).await?;
            let list = parse_json(&raw);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Id", "Title", "Installed", "Dir"]);
            for g in list.as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&g, "id"), s(&g, "title"), flag(&g["installed"]), s(&g, "dir")]);
            }
            println!("{t}");
        }
        Cmd::Scan { source } => {
            let mut p = progress_printer(json);
            let n = core.source_scan(source.as_deref().unwrap_or(""), Some(&mut p)).await?;
            report(json, true, &format!("{n} game(s)"));
            if !json {
                let list = parse_json(&core.list_json().await);
                let mut t = table(&["Title", "Source", "Id", "Build"]);
                for g in list.as_array().cloned().unwrap_or_default() {
                    if s(&g["source"], "kind") == source.clone().unwrap_or_else(|| "gog".into()) {
                        t.add_row(vec![s(&g, "title"), s(&g["source"], "gog_id"), s(&g, "id"), s(&g["source"], "build_id")]);
                    }
                }
                println!("{t}");
            }
        }
        Cmd::Gog { verb } => {
            match verb {
                GogCmd::Scan => {
                    let mut p = progress_printer(json);
                    let n = core.source_scan("gog", Some(&mut p)).await?;
                    report(json, true, &format!("{n} game(s)"));
                    let list = parse_json(&core.list_json().await);
                    if json {
                        print_json(&list);
                        return Ok(());
                    }
                    let mut t = table(&["Title", "GOG id", "Id", "Build", "Hours"]);
                    for g in list.as_array().cloned().unwrap_or_default() {
                        if s(&g["source"], "kind") == "gog" {
                            t.add_row(vec![s(&g, "title"), s(&g["source"], "gog_id"), s(&g, "id"), s(&g["source"], "build_id"), hours(&g)]);
                        }
                    }
                    println!("{t}");
                }
                GogCmd::Library { refresh } => return Box::pin(run(Cli { json, cmd: Some(Cmd::Library { source: "gog".into(), refresh }) })).await,
                GogCmd::Search { query } => return Box::pin(run(Cli { json, cmd: Some(Cmd::Search { query: query.join(" "), source: "gog".into() }) })).await,
                GogCmd::Login { code } => return Box::pin(run(Cli { json, cmd: Some(Cmd::Login { source: "gog".into(), code }) })).await,
            }
        }
        Cmd::Migrate { apply } => {
            let report = parse_json(&core.import_lutris(apply).await?);
            if json {
                print_json(&report);
                return Ok(());
            }
            let n = |k: &str| report[k].as_array().map(|a| a.len()).unwrap_or(0);
            println!("{} imported {}, skipped (already present) {}, updated {}, media from pegasus-library {}", if apply { "applied:" } else { "dry run:" }, n("imported"), n("skipped"), n("updated"), n("media_imported"));
            if let Some(h) = report["hours_imported"].as_object() {
                for (k, v) in h {
                    println!("  hours {k}: {v}");
                }
            }
            let mut t = table(&["Game", "Added", "Removed", "Changed"]);
            for d in report["env_diffs"].as_array().cloned().unwrap_or_default() {
                let j = |k: &str| d[k].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
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
            let list = parse_json(&core.doctor_json().await);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut bad = 0;
            for c in list.as_array().cloned().unwrap_or_default() {
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
                    let mut v = parse_json(&core.settings_json().await);
                    if let Some(key) = key {
                        for part in key.split('.') {
                            v = v.get(part).cloned().unwrap_or(Value::Null);
                        }
                    }
                    match v {
                        Value::String(s) => println!("{s}"),
                        other => print_json(&other),
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
        Cmd::Complete { .. } | Cmd::Generate { .. } => unreachable!(),
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
                print_json(&state);
                return Ok(());
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
                let order: Vec<String> = fam.as_ref().map(|f| f.slots.iter().map(|s| s.id.to_string()).collect()).unwrap_or_default();
                for slot in order {
                    let entry = &d["slots"][&slot];
                    let label = fam.as_ref().and_then(|f| f.slots.iter().find(|s| s.id == slot)).map(|s| s.label).unwrap_or(&slot);
                    let macros = cfg.macros_for(&family, &slot);
                    let of = |t: &str| macros.iter().find(|m| m.trigger == t).map(|m| if m.keys.is_empty() && m.command.is_empty() { m.action.clone() } else { format!("{} {}{}", m.action, m.keys, m.command) }).unwrap_or_default();
                    let code = if entry["bound"] == true { s(entry, "code") } else { "unbound".to_string() };
                    t.add_row(vec![Cell::new(label), if entry["bound"] == true { Cell::new(code) } else { Cell::new(code).add_attribute(Attribute::Dim) }, Cell::new(of("press")), Cell::new(of("hold"))]);
                }
                println!("{t}");
            }
            println!("{} enabled {} · hold {} ms · volume step {}% · mangohud {}", "engine".bold(), cfg.enabled, cfg.hold_ms, cfg.volume_step, s(&state, "mangohud_toggle"));
            let mut t = table(&["Family", "Button", "Trigger", "Action", "Keys / command"]);
            for m in cfg.macros() {
                t.add_row(vec![m.family, m.button, m.trigger, m.action, format!("{}{}", m.keys, m.command)]);
            }
            println!("{t}");
        }
        ControllerCmd::Bind { family, button, trigger, action, keys, command } => {
            let m = controller::Macro { family, button, trigger, action, keys, command };
            core.set_controller_macro(&serde_json::to_string(&m)?).await?;
            report(json, true, "bound");
        }
        ControllerCmd::Unbind { family, button, trigger } => {
            core.remove_controller_macro(&family, &button, trigger.as_deref().unwrap_or("")).await?;
            report(json, true, "unbound");
        }
        ControllerCmd::Learn { family, slot } => {
            let fam = controller::family_by_id(&family).ok_or_else(|| anyhow::anyhow!("unknown family {family}"))?;
            if !fam.slots.iter().any(|s| s.id == slot) {
                anyhow::bail!("{} has no button {slot}", fam.name);
            }
            if !json {
                eprintln!("press the button for {} on the {}…", slot, fam.name);
            }
            let cfg = core.config.read().await.controller.clone();
            let (code, from) = controller::watch::learn_once(&cfg, &fam, &slot).await?;
            core.reload_config().await?;
            if json {
                print_json(&serde_json::json!({"family": family, "slot": slot, "code": code, "from": from}));
            } else {
                println!("{slot} = {code}{}", from.map(|f| format!(" (taken from {f})")).unwrap_or_default());
            }
        }
        ControllerCmd::Forget { family, slot } => {
            core.set_controller_button(&family, &slot, "null").await?;
            report(json, true, "forgotten");
        }
    }
    Ok(())
}

/// One candidate per line, `value<TAB>description`: Fish shows the description.
fn complete(what: &str) -> anyhow::Result<()> {
    match what {
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
                for s in f.slots {
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

const GAME_KEYS: [&str; 22] = [
    "proton=", "exe=", "prefix=", "args=", "working_dir=", "esync=", "fsync=", "mangohud=", "umu_id=", "store=", "pre_command=", "post_command=", "arch=", "backend=", "hide_cursor=",
    "hidden=", "favorite=", "tags=", "sort_title=", "metadata.sgdb_id=", "capture.cursor=", "launch.env.",
];

/// Positional completions clap's static Fish output cannot express: `(subcommand path, position of the
/// positional counted from that subcommand, candidates)`. `games`/`sources`/`modules` call the binary.
const POSITIONALS: &[(&str, usize, &str)] = &[
    ("play", 1, "games"),
    ("info", 1, "games"),
    ("set", 1, "games"),
    ("rm", 1, "games"),
    ("uninstall", 1, "games"),
    ("sessions", 1, "games"),
    ("journal", 1, "games"),
    ("recordings", 1, "games"),
    ("update", 1, "games"),
    ("media", 1, "games all"),
    ("media set", 1, "box_front tile background logo screenshot"),
    ("media set", 2, "FILES"),
    ("media unset", 1, "box_front tile background logo screenshot"),
    ("media candidates", 1, "box_front tile background logo screenshot"),
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

const CONFIG_KEYS: [&str; 20] = [
    "paths.games_root", "paths.prefixes_root", "paths.recordings_root", "paths.journal_root", "paths.overrides", "launch.proton", "launch.esync", "launch.fsync", "launch.mangohud",
    "desktop.profile", "desktop.hide_cursor", "desktop.cursor_extension", "keys.sgdb", "keys.sgdb_file", "keys.rawg", "keys.rawg_file",
    "controller.enabled", "controller.hold_ms", "controller.volume_step", "controller.mangohud_toggle",
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
            "CONFIG_KEYS" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"{}\"\n", CONFIG_KEYS.join(" "))),
            "games" | "sources" | "modules" | "families" | "buttons" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"(universe __complete {what})\"\n")),
            "games all" => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"(universe __complete games) all\"\n")),
            literal => fish.push_str(&format!("complete -c universe -n \"{cond}\" -f -a \"{literal}\"\n")),
        }
    }
    fish.push_str(&format!("complete -c universe -n \"__universe_at set 2+\" -f -a \"{}\"\n", GAME_KEYS.join(" ")));
    fish.push_str("complete -c universe -n \"__fish_seen_subcommand_from search install update\" -l source -x -a \"(universe __complete sources)\"\n");
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

pub fn fmt_duration(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{h}h{m:02}")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

/// Resolves a name to one id, listing the candidates when ambiguous.
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
            use std::io::Write;
            print!("which one? [1-{}] ", ids.len());
            let _ = std::io::stdout().flush();
            let mut s = String::new();
            std::io::stdin().read_line(&mut s)?;
            let n: usize = s.trim().parse().unwrap_or(0);
            ids.get(n.wrapping_sub(1)).cloned().ok_or_else(|| anyhow::anyhow!("no choice"))
        }
    }
}
