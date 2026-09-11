use clap::{Parser, Subcommand};
use comfy_table::{presets::UTF8_FULL_CONDENSED, Attribute, Cell, ColumnConstraint, ContentArrangement, Table};
use futures_util::StreamExt;
use owo_colors::OwoColorize;
use serde_json::Value;

use crate::client::Client;

#[derive(Parser, Debug)]
#[command(name = "universe", version, about = "Universe: launch, record and remember your games")]
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
        #[arg(long)]
        all: bool,
    },
    /// Launch a game (exact › word › substring › path) and wait for it to end
    #[command(alias = "launch")]
    Play {
        name: String,
        #[arg(long, default_value = "")]
        screen: String,
        #[arg(long)]
        no_wait: bool,
    },
    /// Stop the running session
    Stop,
    /// Current session and last sessions
    Status,
    /// Source, build, prefix, hours, modules
    Info { name: String },
    /// Search a source's catalogue
    Search {
        query: String,
        #[arg(long, default_value = "gog")]
        source: String,
    },
    /// Install a title from a source
    Install {
        id: String,
        #[arg(long, default_value = "gog")]
        source: String,
    },
    /// Pending updates, or update one game
    Update {
        name: Option<String>,
        #[arg(long, short)]
        yes: bool,
        #[arg(long, default_value = "gog")]
        source: String,
    },
    /// Remove a game: parks recordings and journal; --purge trashes the prefix
    Rm {
        name: String,
        #[arg(long, short)]
        yes: bool,
        #[arg(long)]
        purge: bool,
    },
    /// Set game keys: proton=proton-em capture.cursor=true hidden=true
    Set { name: String, pairs: Vec<String> },
    /// Sessions of a game
    Sessions { name: String },
    /// Journal entries of a game
    Journal {
        name: String,
        /// Render the Markdown note and print its path
        #[arg(long)]
        render: bool,
        #[arg(long)]
        open: bool,
    },
    /// Recordings of a game
    Recordings { name: String },
    /// Artwork: refresh, set <slot> <path>, unset <slot>, candidates <slot>, pin <sgdb|rawg|steam> <id>
    Media { name: String, action: String, args: Vec<String> },
    /// Modules: ls, enable <id>, disable <id>, settings <id> [game], set <id> key=value [--game g]
    Module {
        action: String,
        args: Vec<String>,
        #[arg(long, default_value = "")]
        game: String,
    },
    /// Sources and their state
    Sources,
    /// Log into a source (prints the URL, then takes the code)
    Login { source: String, code: Option<String> },
    /// Owned titles of a source
    Library {
        #[arg(default_value = "gog")]
        source: String,
        #[arg(long)]
        refresh: bool,
    },
    /// Scan installed games of the sources (all by default)
    #[command(alias = "gog-scan")]
    Scan { source: Option<String> },
    /// Import the Lutris library (report only without --apply)
    Migrate {
        #[arg(long)]
        apply: bool,
    },
    /// Check prerequisites of the core and enabled modules
    Doctor,
    /// Global config: get | set <key> <value>
    Config { action: String, args: Vec<String> },
    /// Take a screenshot through the module that provides one
    Screenshot,
    /// Reload config and rescan the library
    Rescan,
    /// Run the daemon in the foreground
    Daemon,
    /// GOG shortcuts: gog scan | gog library | gog search <q> | gog login [code]
    Gog { verb: String, args: Vec<String> },
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

fn day(rfc: &str) -> String {
    rfc.get(0..10).unwrap_or("").to_string()
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

async fn wait_job(client: &Client, job: &str, json: bool) -> anyhow::Result<bool> {
    let sources = client.sources().await?;
    let mut progress = sources.receive_progress().await?;
    let mut finished = sources.receive_job_finished().await?;
    let finished_in_table = || async {
        let jobs: Value = parse_json(&sources.jobs().await?);
        let j = jobs.as_array().and_then(|a| a.iter().find(|j| j["id"] == job)).cloned();
        anyhow::Ok(j.filter(|j| j["finished"].as_bool() == Some(true)))
    };
    if let Some(j) = finished_in_table().await? {
        return Ok(j["ok"].as_bool().unwrap_or(false));
    }
    let mut poll = tokio::time::interval(std::time::Duration::from_secs(2));
    poll.tick().await;
    loop {
        tokio::select! {
            _ = poll.tick() => {
                if let Some(j) = finished_in_table().await? {
                    let ok = j["ok"].as_bool().unwrap_or(false);
                    let message = j["message"].as_str().unwrap_or_default();
                    if json {
                        print_json(&serde_json::json!({"job": job, "ok": ok, "message": message}));
                    } else if ok {
                        println!("{} {}", "done".green(), message);
                    } else {
                        println!("{} {}", "failed".red(), message);
                    }
                    return Ok(ok);
                }
            }
            Some(p) = progress.next() => {
                let a = p.args()?;
                if a.job_id == job && !json {
                    let pct = if a.total > 0 { format!("{:3.0}%", a.done as f64 * 100.0 / a.total as f64) } else { "    ".into() };
                    eprintln!("  {pct} {}", a.message);
                }
            }
            Some(f) = finished.next() => {
                let a = f.args()?;
                if a.job_id == job {
                    if json {
                        print_json(&serde_json::json!({"job": job, "ok": a.ok, "message": a.message}));
                    } else if a.ok {
                        println!("{} {}", "done".green(), a.message);
                    } else {
                        println!("{} {}", "failed".red(), a.message);
                    }
                    return Ok(a.ok);
                }
            }
        }
    }
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let json = cli.json;
    let cmd = cli.cmd.unwrap_or(Cmd::Ls { all: false });
    if let Cmd::Daemon = cmd {
        return run_daemon().await;
    }
    let client = Client::connect().await?;
    match cmd {
        Cmd::Daemon => unreachable!(),
        Cmd::Ls { all } => {
            let list = parse_json(&client.library().await?.list().await?);
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
                t.add_row(vec![title, Cell::new(s(&g["source"], "kind")), Cell::new(hours(&g)), Cell::new(day(&s(&g["stats"], "last_played")))]);
            }
            println!("{t}");
        }
        Cmd::Play { name, screen, no_wait } => {
            let session = client.session().await?;
            let mut ended = session.receive_session_ended().await?;
            let id = pick(&client, &name).await?;
            let sid = session.launch(&id, &screen).await?;
            if json {
                print_json(&serde_json::json!({"session": sid, "id": id}));
            } else {
                println!("{} {id} · session {sid}", "launched".green());
            }
            if no_wait {
                return Ok(());
            }
            while let Some(e) = ended.next().await {
                let a = e.args()?;
                if a.session_id == sid {
                    if !json {
                        println!("{} after {}", "ended".yellow(), fmt_duration(a.duration_s as u64));
                    }
                    break;
                }
            }
        }
        Cmd::Stop => {
            client.session().await?.stop("").await?;
            println!("stopped");
        }
        Cmd::Status => {
            let session = client.session().await?;
            let cur = session.current().await?;
            let list = parse_json(&client.library().await?.list().await?);
            let mut recent: Vec<Value> = Vec::new();
            for g in list.as_array().cloned().unwrap_or_default() {
                for sess in parse_json(&session.sessions(&s(&g, "id")).await?).as_array().cloned().unwrap_or_default() {
                    let mut sess = sess;
                    sess["title"] = g["title"].clone();
                    recent.push(sess);
                }
            }
            recent.sort_by(|a, b| s(b, "ended_at").cmp(&s(a, "ended_at")));
            recent.truncate(10);
            if json {
                print_json(&serde_json::json!({"current": if cur.is_empty() { Value::Null } else { parse_json(&cur) }, "recent": recent}));
                return Ok(());
            }
            if cur.is_empty() {
                println!("{}", "no session running".dimmed());
            } else {
                let c = parse_json(&cur);
                println!("{} {} · session {} · {} · since {}", "running".green(), s(&c, "title"), s(&c, "session_id"), s(&c, "unit"), s(&c, "started_at"));
            }
            let mut t = table(&["Session", "Game", "Duration", "Source", "Recording"]);
            for r in recent {
                t.add_row(vec![s(&r, "session"), s(&r, "title"), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), s(&r, "source"), if r["recording"].is_null() { String::new() } else { "✓".into() }]);
            }
            println!("{t}");
        }
        Cmd::Info { name } => {
            let id = pick(&client, &name).await?;
            let g = parse_json(&client.library().await?.get(&id).await?);
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
            println!("  hours      {}  plays {}  last {}", hours(&g), g["stats"]["play_count"], s(&g["stats"], "last_played"));
            println!("  media      {}", g["media"]);
            println!("  modules    {}", g["modules"]);
            println!("  journal    {} entries · recordings {}", g["journal_count"], g["recording_count"]);
        }
        Cmd::Search { query, source } => {
            let list = parse_json(&client.sources().await?.search(&source, &query).await?);
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
            let job = client.sources().await?.install(&source, &id).await?;
            if !json {
                println!("installing {id} from {source} ({job})");
            }
            let ok = wait_job(&client, &job, json).await?;
            if !ok {
                std::process::exit(1);
            }
        }
        Cmd::Update { name, yes, source } => {
            let sources = client.sources().await?;
            match name {
                Some(n) => {
                    let id = pick(&client, &n).await?;
                    let g = parse_json(&client.library().await?.get(&id).await?);
                    let gid = s(&g["source"], "gog_id");
                    if gid.is_empty() {
                        anyhow::bail!("{id} has no source id");
                    }
                    let job = sources.update(&s(&g["source"], "kind"), &gid).await?;
                    let ok = wait_job(&client, &job, json).await?;
                    if !ok {
                        std::process::exit(1);
                    }
                }
                None => {
                    let pending = parse_json(&sources.updates().await?);
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
                        t.add_row(vec![s(u, "id"), s(u, "title"), s(u, "local_build"), s(u, "remote_build"), s(u, "version"), s(u, "date")]);
                    }
                    println!("{t}");
                    if yes || confirm("download?") {
                        for u in &list {
                            let src = if s(u, "source").is_empty() { source.clone() } else { s(u, "source") };
                            let job = sources.update(&src, &s(u, "id")).await?;
                            wait_job(&client, &job, json).await?;
                        }
                    }
                }
            }
        }
        Cmd::Rm { name, yes, purge } => {
            let id = pick(&client, &name).await?;
            if yes || confirm(&format!("remove {id}{}?", if purge { " and trash its prefix" } else { "" })) {
                client.library().await?.remove(&id, purge).await?;
                println!("removed {id}");
            }
        }
        Cmd::Set { name, pairs } => {
            let id = pick(&client, &name).await?;
            let lib = client.library().await?;
            for p in pairs {
                let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value, got {p}"))?;
                let k = match k {
                    "proton" | "exe" | "prefix" | "args" | "working_dir" | "esync" | "fsync" | "mangohud" | "umu_id" | "store" | "pre_command" | "post_command" | "arch" | "backend" => format!("launch.{k}"),
                    "hide_cursor" => "desktop.hide_cursor".into(),
                    _ => k.to_string(),
                };
                lib.set(&id, &k, v).await?;
                println!("{id}: {k} = {v}");
            }
        }
        Cmd::Sessions { name } => {
            let id = pick(&client, &name).await?;
            let list = parse_json(&client.session().await?.sessions(&id).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            let mut t = table(&["Session", "Started", "Duration", "Source", "Recording"]);
            for r in list.as_array().cloned().unwrap_or_default() {
                t.add_row(vec![s(&r, "session"), s(&r, "started_at"), fmt_duration(r["duration_s"].as_u64().unwrap_or(0)), s(&r, "source"), s(&r, "recording")]);
            }
            println!("{t}");
        }
        Cmd::Journal { name, render, open } => {
            let id = pick(&client, &name).await?;
            let j = client.journal().await?;
            if render || open {
                let path = j.render(&id).await?;
                println!("{path}");
                if open {
                    let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
                }
                return Ok(());
            }
            let list = parse_json(&j.list(&id).await?);
            if json {
                print_json(&list);
                return Ok(());
            }
            for e in list.as_array().cloned().unwrap_or_default() {
                println!("{} {} {}", s(&e, "session").dimmed(), s(&e, "title").bold(), format!("[{}]", s(&e, "lang")).dimmed());
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
            let id = pick(&client, &name).await?;
            let list = parse_json(&client.recording().await?.list(&id).await?);
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
        Cmd::Media { name, action, args } => {
            let media = client.media().await?;
            match action.as_str() {
                "refresh" => {
                    let id = if name == "all" { String::new() } else { pick(&client, &name).await? };
                    let job = media.refresh(&id, args.iter().any(|a| a == "--force")).await?;
                    wait_job(&client, &job, json).await?;
                }
                "set" => {
                    let id = pick(&client, &name).await?;
                    let path = std::fs::canonicalize(args.get(1).ok_or_else(|| anyhow::anyhow!("media set <slot> <path>"))?)?;
                    media.set_slot(&id, &args[0], &path.to_string_lossy()).await?;
                    println!("{id}: {} set", args[0]);
                }
                "unset" => {
                    let id = pick(&client, &name).await?;
                    media.unset(&id, args.first().ok_or_else(|| anyhow::anyhow!("media unset <slot>"))?).await?;
                }
                "candidates" => {
                    let id = pick(&client, &name).await?;
                    let list = parse_json(&media.candidates(&id, args.first().map(|s| s.as_str()).unwrap_or("box_front")).await?);
                    print_json(&list);
                }
                "pin" => {
                    let id = pick(&client, &name).await?;
                    media.pin(&id, args.first().ok_or_else(|| anyhow::anyhow!("media pin <provider> <id>"))?, args.get(1).ok_or_else(|| anyhow::anyhow!("media pin <provider> <id>"))?).await?;
                    println!("pinned");
                }
                other => anyhow::bail!("unknown media action {other}"),
            }
        }
        Cmd::Module { action, args, game } => {
            let modules = client.modules().await?;
            match action.as_str() {
                "ls" | "list" => {
                    let list = parse_json(&modules.list().await?);
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
                "enable" | "disable" => {
                    let id = args.first().ok_or_else(|| anyhow::anyhow!("module {action} <id>"))?;
                    modules.enable(id, action == "enable").await?;
                    println!("{id} {action}d");
                }
                "settings" => {
                    let id = args.first().ok_or_else(|| anyhow::anyhow!("module settings <id> [game]"))?;
                    let gid = match args.get(1) { Some(g) => pick(&client, g).await?, None => game.clone() };
                    print_json(&parse_json(&modules.get_settings(id, &gid).await?));
                }
                "set" => {
                    let id = args.first().ok_or_else(|| anyhow::anyhow!("module set <id> key=value"))?;
                    let gid = if game.is_empty() { String::new() } else { pick(&client, &game).await? };
                    for p in &args[1..] {
                        let (k, v) = p.split_once('=').ok_or_else(|| anyhow::anyhow!("expected key=value"))?;
                        modules.set_setting(id, &gid, k, v).await?;
                        println!("{id}.{k} = {v}{}", if gid.is_empty() { String::new() } else { format!(" ({gid})") });
                    }
                }
                other => anyhow::bail!("unknown module action {other}"),
            }
        }
        Cmd::Sources => {
            let list = parse_json(&client.sources().await?.list().await?);
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
            let sources = client.sources().await?;
            let code = match code {
                Some(c) => c,
                None => {
                    let url = sources.login_url(&source).await?;
                    println!("Open this URL, log in, then paste the code= value from the address bar:\n\n  {url}\n");
                    use std::io::Write;
                    print!("code: ");
                    let _ = std::io::stdout().flush();
                    let mut s = String::new();
                    std::io::stdin().read_line(&mut s)?;
                    s.trim().to_string()
                }
            };
            let job = sources.login(&source, &code).await?;
            wait_job(&client, &job, json).await?;
        }
        Cmd::Library { source, refresh } => {
            let sources = client.sources().await?;
            let raw = if refresh { sources.refresh_library(&source).await? } else { sources.library(&source).await? };
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
            let job = client.sources().await?.scan(source.as_deref().unwrap_or("")).await?;
            wait_job(&client, &job, json).await?;
            if !json {
                let list = parse_json(&client.library().await?.list().await?);
                let mut t = table(&["Title", "Source", "Id", "Build"]);
                for g in list.as_array().cloned().unwrap_or_default() {
                    if s(&g["source"], "kind") == source.clone().unwrap_or_else(|| "gog".into()) {
                        t.add_row(vec![s(&g, "title"), s(&g["source"], "gog_id"), s(&g, "id"), s(&g["source"], "build_id")]);
                    }
                }
                println!("{t}");
            }
        }
        Cmd::Gog { verb, args } => {
            let sources = client.sources().await?;
            match verb.as_str() {
                "scan" => {
                    let job = sources.scan("gog").await?;
                    wait_job(&client, &job, json).await?;
                    let list = parse_json(&client.library().await?.list().await?);
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
                "library" => return Box::pin(run(Cli { json, cmd: Some(Cmd::Library { source: "gog".into(), refresh: args.iter().any(|a| a == "--refresh") }) })).await,
                "search" => return Box::pin(run(Cli { json, cmd: Some(Cmd::Search { query: args.join(" "), source: "gog".into() }) })).await,
                "login" => return Box::pin(run(Cli { json, cmd: Some(Cmd::Login { source: "gog".into(), code: args.first().cloned() }) })).await,
                other => anyhow::bail!("unknown gog verb {other}"),
            }
        }
        Cmd::Migrate { apply } => {
            let report = parse_json(&client.library().await?.import_lutris(apply).await?);
            if json {
                print_json(&report);
                return Ok(());
            }
            let n = |k: &str| report[k].as_array().map(|a| a.len()).unwrap_or(0);
            println!("{} imported {}, skipped (already present) {}, updated {}", if apply { "applied:" } else { "dry run:" }, n("imported"), n("skipped"), n("updated"));
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
            let list = parse_json(&client.modules().await?.doctor().await?);
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
        Cmd::Config { action, args } => {
            let settings = client.settings().await?;
            match action.as_str() {
                "get" => {
                    let mut v = parse_json(&settings.get().await?);
                    if let Some(key) = args.first() {
                        for part in key.split('.') {
                            v = v.get(part).cloned().unwrap_or(Value::Null);
                        }
                    }
                    match v {
                        Value::String(s) => println!("{s}"),
                        other => print_json(&other),
                    }
                }
                "set" => {
                    let (k, v) = (args.first().ok_or_else(|| anyhow::anyhow!("config set <key> <value>"))?, args.get(1).map(|s| s.as_str()).unwrap_or(""));
                    settings.set(k, v).await?;
                    println!("{k} = {v}");
                }
                other => anyhow::bail!("unknown config action {other}"),
            }
        }
        Cmd::Screenshot => println!("{}", client.session().await?.screenshot().await?),
        Cmd::Rescan => {
            client.library().await?.rescan().await?;
            println!("rescanned");
        }
    }
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
async fn pick(client: &Client, name: &str) -> anyhow::Result<String> {
    let ids = client.library().await?.resolve(name).await?;
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

async fn run_daemon() -> anyhow::Result<()> {
    let config = crate::config::Config::load()?;
    std::fs::create_dir_all(crate::paths::games_dir())?;
    let core = crate::core::Core::new(config)?;
    let _conn = crate::dbus::serve(core.clone()).await?;
    eprintln!("universed {} on {}", crate::VERSION, crate::BUS_NAME);
    tokio::signal::ctrl_c().await?;
    Ok(())
}
