//! The built binary on an empty, isolated profile.
use std::process::Command;

fn universe(dir: &tempfile::TempDir, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    command(dir, args, env).output().unwrap()
}

fn command(dir: &tempfile::TempDir, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_universe"));
    for (var, sub) in [
        ("UNIVERSE_DATA_HOME", "data"),
        ("UNIVERSE_CONFIG_HOME", "config"),
        ("UNIVERSE_STATE_HOME", "state"),
        ("UNIVERSE_CACHE_HOME", "cache"),
        ("UNIVERSE_MODULES_PATH", "modules"),
        ("UNIVERSE_SOURCES_PATH", "sources"),
    ] {
        std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        cmd.env(var, dir.path().join(sub));
    }
    cmd.env_remove("NO_COLOR").env_remove("CLICOLOR_FORCE").env("HOME", dir.path());
    // The emulators' own configs (discover's folder scan) live under these, not the Universe homes.
    cmd.env("XDG_CONFIG_HOME", dir.path().join(".config")).env("XDG_DATA_HOME", dir.path().join(".local/share"));
    cmd.envs(env.iter().copied()).args(args);
    cmd
}

fn no_desktop() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    std::fs::write(dir.path().join("config/config.toml"), "schema = 1\n[desktop]\nprofile = \"none\"\n").unwrap();
    dir
}

const NO_BUS: (&str, &str) = ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/dev/null/bus");

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn colour_reaches_a_terminal_only() {
    let dir = tempfile::tempdir().unwrap();
    let piped = universe(&dir, &["doctor"], &[]);
    let text = String::from_utf8_lossy(&piped.stdout);
    assert!(!text.is_empty() && !text.contains('\x1b'), "escapes on a pipe: {text:?}");
    let forced = String::from_utf8_lossy(&universe(&dir, &["doctor"], &[("CLICOLOR_FORCE", "1")]).stdout).into_owned();
    assert!(forced.contains('\x1b'), "CLICOLOR_FORCE colours a pipe: {forced:?}");
    let off = String::from_utf8_lossy(&universe(&dir, &["doctor"], &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")]).stdout).into_owned();
    assert!(!off.contains('\x1b'), "NO_COLOR wins: {off:?}");
}

#[test]
fn version_shows_the_rev_baked_in_else_the_one_handed_over_at_run_time() {
    let dir = tempfile::tempdir().unwrap();
    let version = |rev: Option<&str>| {
        let mut cmd = command(&dir, &["--version"], &[]);
        match rev {
            Some(rev) => cmd.env("UNIVERSE_GIT_REV", rev),
            None => cmd.env_remove("UNIVERSE_GIT_REV"),
        };
        text(&cmd.output().unwrap().stdout).trim().to_string()
    };
    let bare = version(None);
    let handed = version(Some("abc1234"));
    if bare == format!("universe {}", universe::VERSION) {
        assert_eq!(handed, format!("universe {} (abc1234)", universe::VERSION));
    } else {
        assert!(bare.starts_with(&format!("universe {} (", universe::VERSION)), "{bare}");
        assert_eq!(handed, bare, "a rev baked in wins over the run time's");
    }
    assert_eq!(version(Some("")), bare);
}

#[test]
fn doctor_needs_journalctl_not_the_systemd_clients() {
    let dir = tempfile::tempdir().unwrap();
    let out = universe(&dir, &["doctor", "--json"], &[]);
    let checks: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    let names: Vec<&str> = checks.iter().filter_map(|c| c["check"].as_str()).collect();
    assert!(names.contains(&"journalctl") && names.contains(&"umu-run"), "{names:?}");
    assert!(!names.contains(&"systemd-run") && !names.contains(&"systemctl"), "{names:?}");
}

#[test]
fn discover_on_a_bare_home_finds_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = universe(&dir, &["discover", "--json"], &[]);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    let launchers = report["launchers"].as_array().unwrap();
    let ids: Vec<&str> = launchers.iter().filter_map(|l| l["id"].as_str()).collect();
    assert_eq!(ids, ["lutris", "steam", "heroic-gog", "heroic-epic", "heroic-amazon", "itch", "roms"]);
    assert!(launchers.iter().all(|l| l["found"] == false && l["games"] == 0), "{launchers:?}");
    assert_eq!((&report["gog_dirs"], &report["install_dirs"]), (&serde_json::json!([]), &serde_json::json!([])));
    let text = String::from_utf8_lossy(&universe(&dir, &["discover"], &[]).stdout).into_owned();
    assert!(text.contains("not installed"), "{text:?}");
}

#[test]
fn a_ps5_dump_in_each_ps5_emulators_game_folder_is_discovered_and_rescanned_and_one_elsewhere_added_by_hand() {
    let dir = tempfile::tempdir().unwrap();
    let dump = |at: &str, id: &str| {
        let d = dir.path().join(at);
        std::fs::create_dir_all(d.join("sce_sys")).unwrap();
        std::fs::write(d.join("eboot.bin"), b"").unwrap();
        std::fs::write(d.join("sce_sys/param.json"), format!(r#"{{"titleId": "{id}", "localizedParameters": {{"en-US": {{"titleName": "Title {id}"}}}}}}"#))
            .unwrap();
        d
    };
    dump("ps5/Kyty games/PPSA01234", "PPSA01234");
    dump("SharpEmu games/PPSA09999", "PPSA09999");
    let elsewhere = dump("elsewhere/PPSA05678", "PPSA05678");
    let kyty = dir.path().join("apps/Kyty");
    std::fs::create_dir_all(&kyty).unwrap();
    std::fs::write(kyty.join("kyty_emulator"), b"#!/bin/sh\n").unwrap();
    std::fs::create_dir_all(dir.path().join(".config/Kyty")).unwrap();
    std::fs::write(dir.path().join(".config/Kyty/Kyty.ini"), format!("[Launcher]\ngame_dirs={}\n", dir.path().join("ps5").display())).unwrap();
    let sharpemu = dir.path().join("apps/SharpEmu");
    std::fs::create_dir_all(&sharpemu).unwrap();
    std::fs::write(sharpemu.join("SharpEmu"), b"#!/bin/sh\n").unwrap();
    std::fs::write(sharpemu.join("gui-settings.json"), format!(r#"{{"GameFolders": ["{}"]}}"#, dir.path().join("SharpEmu games").display())).unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    std::fs::write(
        dir.path().join("config/config.toml"),
        format!(
            "schema = 1\n[desktop]\nprofile = \"none\"\n[runners.kytyps5]\nexe = \"{}\"\n[runners.sharpemu]\nexe = \"{}\"\n",
            kyty.join("kyty_emulator").display(),
            sharpemu.join("SharpEmu").display()
        ),
    )
    .unwrap();
    let json = |args: &[&str]| -> serde_json::Value {
        let offline = "http://127.0.0.1:9";
        let out = universe(&dir, args, &[("HTTP_PROXY", offline), ("HTTPS_PROXY", offline), ("ALL_PROXY", offline)]);
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{args:?}: {e}: {}", text(&out.stderr)))
    };
    let report = json(&["discover", "--json"]);
    let roms = report["launchers"].as_array().unwrap().iter().find(|l| l["id"] == "roms").unwrap().clone();
    assert_eq!((roms["found"].as_bool(), roms["games"].as_u64()), (Some(true), Some(2)), "{roms}");
    let rescan = json(&["rescan", "--json"]);
    let imported: Vec<(&str, &str)> =
        rescan["imported"].as_array().unwrap().iter().map(|f| (f["id"].as_str().unwrap(), f["runner"].as_str().unwrap())).collect();
    assert_eq!(imported, [("title-ppsa01234", "kytyps5"), ("title-ppsa09999", "sharpemu")]);
    let added = json(&["add", &elsewhere.join("eboot.bin").to_string_lossy(), "--runner", "kytyps5", "--json"]);
    let effective = &added["effective"];
    assert_eq!((effective["runner"].as_str(), effective["platform"].as_str()), (Some("kytyps5"), Some("Sony PlayStation 5")));
    assert_eq!(effective["working_dir"].as_str(), Some(kyty.canonicalize().unwrap().to_str().unwrap()), "it starts beside the player's own Kyty");
}

#[test]
fn an_older_config_gives_up_its_all_games_xbox_layout_and_a_game_keeps_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let controls = dir.path().join("modules/controls");
    std::fs::create_dir_all(&controls).unwrap();
    std::fs::copy(concat!(env!("CARGO_MANIFEST_DIR"), "/../../modules/controls/module.toml"), controls.join("module.toml")).unwrap();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::write(&config, "schema = 1\n[modules]\nenabled = [\"controls\"]\n[modules.controls]\nlayout = \"xbox\"\n").unwrap();
    let game = dir.path().join("data/games/strikers");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(
        game.join("game.toml"),
        "schema = 1\nid = \"strikers\"\ntitle = \"Super Mario Strikers\"\nplatform = \"Nintendo GameCube\"\n[launch]\nrunner = \"dolphin\"\nexe = \"/roms/strikers.rvz\"\n[modules.controls]\nlayout = \"xbox\"\n",
    )
    .unwrap();
    let layout = |args: &[&str]| -> serde_json::Value {
        let out = universe(&dir, &[&["--json", "module", "settings", "controls"], args].concat(), &[]);
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stderr)))["layout"].clone()
    };
    assert_eq!(layout(&[]), "pad", "each controller's own for every game");
    assert_eq!(layout(&["strikers"]), "xbox", "the game's own choice stays");
    assert!(std::fs::read_to_string(&config).unwrap().starts_with("schema = 2\n"));
}

#[test]
fn the_overlay_unit_runs_universe_ui_over_the_display_splash_records_and_ends_with_no_game() {
    let dir = no_desktop();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let ui = bin.join("universe-ui");
    std::fs::write(&ui, format!("#!/bin/sh\necho \"$@\" > {}\n", dir.path().join("ui-args").display())).unwrap();
    std::fs::set_permissions(&ui, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let out = universe(&dir, &["overlay", "/run/user/1000/universe/display-x"], &[("PATH", &path), NO_BUS]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_to_string(dir.path().join("ui-args")).unwrap(), "--overlay /run/user/1000/universe/display-x\n");
}

// A node that would not open is passed over, then left alone until something asks: Find controllers is that ask.
#[test]
fn a_rescan_tries_again_the_nodes_the_watcher_passed_over() {
    use std::io::{BufRead, Write};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input");
    std::fs::create_dir_all(&input).unwrap();
    std::fs::write(input.join("event5"), "not a device").unwrap();
    std::fs::write(input.join("mouse0"), "").unwrap();
    std::fs::create_dir_all(dir.path().join("run")).unwrap();
    let run = dir.path().join("run");
    let input_dir = input.to_string_lossy().into_owned();
    let mut child =
        command(&dir, &["controller", "watch", "--json"], &[("UNIVERSE_INPUT_DIR", &input_dir), ("XDG_RUNTIME_DIR", &run.to_string_lossy()), NO_BUS])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let (tx, lines) = std::sync::mpsc::channel::<serde_json::Value>();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(serde_json::from_str(&line).unwrap_or_default());
        }
    });
    let until = |event: &str| loop {
        let line = lines.recv_timeout(std::time::Duration::from_secs(5)).unwrap_or_else(|_| panic!("no {event}"));
        if line["event"] == event {
            return line;
        }
    };
    until("ready");
    for (command, skipped) in [(serde_json::json!({"cmd": "rescan"}), ["event5"]), (serde_json::json!({"cmd": "rescan", "id": "event5"}), ["event5"])] {
        writeln!(stdin, "{command}").unwrap();
        let rescanned = until("rescanned");
        assert_eq!((rescanned["devices"].as_u64(), rescanned["skipped"].clone()), (Some(0), serde_json::json!(skipped)), "{command}");
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn a_write_the_config_cannot_hold_or_to_a_read_only_config_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "schema = 1\n").unwrap();
    for (step, taken) in [("0", false), ("101", false), ("precise", false), ("10", true)] {
        let out = universe(&dir, &["config", "set", "controller.volume_step", step], &[]);
        assert_eq!(out.status.success(), taken, "volume step {step}: {}", text(&out.stderr));
    }
    let settings: serde_json::Value = serde_json::from_slice(&universe(&dir, &["config", "get", "--json"], &[]).stdout).unwrap();
    assert_eq!(settings["controller"]["volume_step"], 10, "a percent from 1 to 100");
    let held = std::fs::read(&config).unwrap();
    for (key, value) in [("desktop.keep_awake", "maybe"), ("controller.hold_ms", "fast")] {
        let out = universe(&dir, &["config", "set", key, value], &[]);
        assert_eq!(out.status.code(), Some(1), "{key}: {}", text(&out.stderr));
        assert!(text(&out.stderr).contains(key), "{}", text(&out.stderr));
        assert_eq!(std::fs::read(&config).unwrap(), held, "{key} left the file byte for byte");
    }
    assert!(universe(&dir, &["ls"], &[]).status.success(), "the next command still opens");
    std::fs::set_permissions(&config, std::os::unix::fs::PermissionsExt::from_mode(0o444)).unwrap();
    let out = universe(&dir, &["config", "set", "launch.hdr", "true"], &[]);
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(!out.status.success() && err.contains("read-only") && err.contains("make it writable"), "a file of the user's own: {err:?}");
    let settings: serde_json::Value = serde_json::from_slice(&universe(&dir, &["config", "get", "--json"], &[]).stdout).unwrap();
    assert_eq!((&settings["config_writable"], &settings["config_owner"]), (&serde_json::json!(false), &serde_json::json!("")));
    let text = String::from_utf8_lossy(&universe(&dir, &["doctor"], &[]).stdout).into_owned();
    assert!(text.contains("read-only"), "{text:?}");
}

#[test]
fn a_write_to_a_config_it_cannot_read_fails_and_leaves_it_in_place() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::create_dir_all(config.join("kept")).unwrap();
    assert_eq!(universe(&dir, &["config", "set", "launch.hdr", "true"], &[]).status.code(), Some(1));
    assert!(config.join("kept").is_dir(), "a directory stays what it was");
    std::fs::remove_dir_all(&config).unwrap();

    std::fs::write(&config, "schema = 2\n# mine\n").unwrap();
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000)).unwrap();
    assert_eq!(universe(&dir, &["config", "set", "launch.hdr", "true"], &[]).status.code(), Some(1));
    std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(std::fs::read_to_string(&config).unwrap(), "schema = 2\n# mine\n");
}

#[test]
fn a_write_keeps_a_linked_config_linked_and_a_store_link_says_who_owns_it() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    let dotfiles = dir.path().join("dotfiles");
    std::fs::create_dir_all(&dotfiles).unwrap();
    std::fs::write(dotfiles.join("universe.toml"), "schema = 2\n# mine\n").unwrap();
    std::os::unix::fs::symlink(dotfiles.join("universe.toml"), &config).unwrap();
    let out = universe(&dir, &["config", "set", "launch.hdr", "true"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_link(&config).unwrap(), dotfiles.join("universe.toml"));
    let written = std::fs::read_to_string(dotfiles.join("universe.toml")).unwrap();
    assert!(written.contains("# mine") && written.contains("hdr = true"), "{written:?}");
    assert_eq!(std::fs::read_dir(&dotfiles).unwrap().count(), 1, "no temporary file left");

    std::fs::remove_file(dotfiles.join("universe.toml")).unwrap();
    let out = universe(&dir, &["config", "set", "launch.hdr", "true"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_link(&config).unwrap(), dotfiles.join("universe.toml"), "a dangling link stays one");
    assert!(std::fs::read_to_string(dotfiles.join("universe.toml")).unwrap().contains("hdr = true"), "its target is made");

    let store = dir.path().join("store");
    std::fs::create_dir_all(&store).unwrap();
    let generated = store.join("universe-config.toml");
    std::fs::write(&generated, "schema = 2\n").unwrap();
    std::fs::set_permissions(&generated, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::fs::remove_file(&config).unwrap();
    std::os::unix::fs::symlink(&generated, &config).unwrap();
    let store_dir = store.to_str().unwrap();
    let out = universe(&dir, &["config", "set", "launch.hdr", "true"], &[("NIX_STORE_DIR", store_dir)]);
    let err = text(&out.stderr);
    assert!(!out.status.success() && err.contains("read-only") && err.contains("home-manager"), "{err:?}");
    assert_eq!(std::fs::read_to_string(&generated).unwrap(), "schema = 2\n");
    assert_eq!(std::fs::read_link(&config).unwrap(), generated);
}

fn left_open(dir: &tempfile::TempDir, game: &str, session: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    left_open_after(dir, game, session, "")
}

/// `first` runs in the undo before it notes itself in the returned `undone`.
fn left_open_after(dir: &tempfile::TempDir, game: &str, session: &str, first: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let marker = dir.path().join("state/current-session.json");
    let undone = dir.path().join("undone");
    std::fs::create_dir_all(marker.parent().unwrap()).unwrap();
    let current = serde_json::json!({
        "session_id": session,
        "id": game,
        "title": game,
        "unit": format!("universe-game-{game}-{session}.service"),
        "screen": "",
        "started_at": "2026-11-06T00:00:00+00:00",
        "undo": [{"step": "post_command", "command": format!("{first}echo undone >> {}", undone.display()), "cwd": "/", "env": {}}],
    });
    std::fs::write(&marker, current.to_string()).unwrap();
    (marker, undone)
}

#[test]
fn a_session_that_cannot_be_filed_waits_aside_and_one_that_cannot_wait_says_what_to_free() {
    let dir = no_desktop();
    let exe = dir.path().join("quest.sh");
    std::fs::write(&exe, "").unwrap();
    let added = universe(&dir, &["add", exe.to_str().unwrap(), "--runner", "linux", "--title", "Quest", "--json"], &[]);
    assert!(added.status.success(), "{}", text(&added.stderr));
    let rows = dir.path().join("data/games/quest/sessions.jsonl");
    std::fs::create_dir_all(&rows).unwrap();
    let (marker, undone) = left_open(&dir, "quest", "20261106-000000");

    let aside = dir.path().join("state/closing-sessions");
    std::fs::write(&aside, "").unwrap();
    let out = universe(&dir, &["storage"], &[NO_BUS]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(marker.exists() && !undone.exists(), "kept nowhere else, the row and its undo stay on the marker");
    let out = universe(&dir, &["play", "quest", "--no-wait"], &[NO_BUS]);
    let err = text(&out.stderr);
    assert!(!out.status.success() && err.contains("closing-sessions"), "{err}");
    assert!(marker.exists(), "a launch does not write over it");

    std::fs::remove_file(&aside).unwrap();
    for _ in 0..2 {
        let out = universe(&dir, &["storage"], &[NO_BUS]);
        assert!(out.status.success(), "{}", text(&out.stderr));
    }
    assert!(!marker.exists(), "the row kept aside, the marker is free");
    assert_eq!(std::fs::read_to_string(&undone).unwrap().lines().count(), 1, "undone once");
    let out = universe(&dir, &["play", "quest", "--no-wait"], &[NO_BUS]);
    assert!(!text(&out.stderr).contains("sessions.jsonl"), "{}", text(&out.stderr));

    std::fs::remove_dir(&rows).unwrap();
    let opens: Vec<_> = (0..8).map(|_| command(&dir, &["ls"], &[NO_BUS]).stdout(std::process::Stdio::null()).spawn().unwrap()).collect();
    for mut open in opens {
        assert!(open.wait().unwrap().success());
    }
    let filed = std::fs::read_to_string(&rows).unwrap();
    assert_eq!(filed.lines().count(), 1, "{filed}");
    let row: serde_json::Value = serde_json::from_str(filed.trim()).unwrap();
    assert_eq!(row["session"], "20261106-000000");
    assert_eq!(std::fs::read_to_string(&undone).unwrap().lines().count(), 1);
}

fn add_game(dir: &tempfile::TempDir, title: &str) {
    let exe = dir.path().join(format!("{title}.sh"));
    std::fs::write(&exe, "").unwrap();
    let added = universe(dir, &["add", exe.to_str().unwrap(), "--runner", "linux", "--title", title, "--json"], &[]);
    assert!(added.status.success(), "{}", text(&added.stderr));
}

fn lines(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path).map_or(0, |t| t.lines().count())
}

fn shim(dir: &std::path::Path, name: &str, body: &str) -> String {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), format!("#!/bin/sh\n{body}")).unwrap();
    std::fs::set_permissions(dir.join(name), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    format!("{}:{}", dir.display(), std::env::var("PATH").unwrap_or_default())
}

#[test]
fn a_close_held_in_the_journal_while_another_files_the_session_files_it_once() {
    let dir = no_desktop();
    add_game(&dir, "Quest");
    add_game(&dir, "Other");
    let rows = dir.path().join("data/games/quest/sessions.jsonl");
    std::fs::create_dir_all(&rows).unwrap();
    let other = dir.path().join("data/games/other/sessions.jsonl");
    std::fs::write(&other, "{\"session\":\"20261101-000000\",\"game\":\"other\"}\n").unwrap();
    let (marker, undone) = left_open(&dir, "quest", "20261106-000000");
    let quick = shim(&dir.path().join("quick"), "journalctl", "");
    let (entered, release) = (dir.path().join("entered"), dir.path().join("release"));
    let path = shim(&dir.path().join("held"), "journalctl", &format!("touch {}\nwhile [ ! -e {} ]; do sleep 0.02; done\n", entered.display(), release.display()));
    let mut held = command(&dir, &["storage"], &[NO_BUS, ("PATH", &path)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let started = std::time::Instant::now();
    while !entered.exists() {
        if started.elapsed().as_secs() >= 10 || held.try_wait().unwrap().is_some() {
            let _ = held.kill();
            panic!("the first open never read the journal");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let out = universe(&dir, &["storage"], &[NO_BUS, ("PATH", &quick)]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(!marker.exists() && lines(&undone) == 1, "the second open closed it, its history still broken");
    std::fs::remove_dir(&rows).unwrap();
    let out = universe(&dir, &["storage"], &[NO_BUS, ("PATH", &quick)]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(lines(&rows), 1, "the third open filed it");
    std::fs::write(&release, "").unwrap();
    assert!(held.wait().unwrap().success());
    let out = universe(&dir, &["storage"], &[NO_BUS, ("PATH", &quick)]);
    assert!(out.status.success(), "{}", text(&out.stderr));

    let filed = std::fs::read_to_string(&rows).unwrap();
    assert_eq!(filed.lines().count(), 1, "{filed}");
    let row: serde_json::Value = serde_json::from_str(filed.trim()).unwrap();
    assert_eq!((row["session"].as_str(), row["unit"].as_str()), (Some("20261106-000000"), Some("universe-game-quest-20261106-000000.service")));
    assert_eq!(lines(&undone), 1);
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "{\"session\":\"20261101-000000\",\"game\":\"other\"}\n");
}

#[test]
fn a_close_killed_mid_way_finishes_on_the_next_open_and_repeats_nothing() {
    use std::os::unix::process::ExitStatusExt;
    let dir = no_desktop();
    add_game(&dir, "Quest");
    let rows = dir.path().join("data/games/quest/sessions.jsonl");
    std::fs::create_dir_all(&rows).unwrap();
    let ended = dir.path().join("ended");
    let crash_once = |name: &str| format!("[ -e {0} ] || {{ touch {0}; kill -9 $PPID; exit 1; }}\n", dir.path().join(name).display());
    shim(&dir.path().join("modules/end/bin"), "end", &format!("{}echo \"$GAME_ID\" >> {}\n", crash_once("hook-crashed"), ended.display()));
    std::fs::write(dir.path().join("modules/end/module.toml"), "api = 2\nid = \"end\"\n[hooks]\nsession-end = \"bin/end\"\n").unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::write(&config, std::fs::read_to_string(&config).unwrap() + "[modules]\nenabled = [\"end\"]\n").unwrap();
    let (marker, undone) = left_open_after(&dir, "quest", "20261106-000000", &crash_once("undo-crashed"));

    let storage = || universe(&dir, &["storage"], &[NO_BUS]);
    assert_eq!(storage().status.signal(), Some(9), "killed in the undo");
    assert!(!marker.exists() && lines(&undone) == 0);
    assert_eq!(storage().status.signal(), Some(9), "the next open undid it, then was killed in the session-end hook");
    assert_eq!((lines(&undone), lines(&ended)), (1, 0));
    for _ in 0..2 {
        let out = storage();
        assert!(out.status.success(), "{}", text(&out.stderr));
        assert_eq!((lines(&undone), lines(&ended)), (1, 1), "each ran once");
    }
    std::fs::remove_dir(&rows).unwrap();
    let out = storage();
    assert!(out.status.success(), "{}", text(&out.stderr));
    let filed = std::fs::read_to_string(&rows).unwrap();
    assert_eq!(filed.lines().count(), 1, "{filed}");
    assert_eq!(serde_json::from_str::<serde_json::Value>(filed.trim()).unwrap()["session"], "20261106-000000");
    assert_eq!((lines(&undone), lines(&ended)), (1, 1));
}

#[test]
fn a_session_of_a_game_with_no_folder_is_dropped_without_failing_the_open() {
    let dir = no_desktop();
    let (marker, undone) = left_open(&dir, "gone", "20261106-000000");
    let aside = dir.path().join("state/closing-sessions");
    std::fs::create_dir_all(&aside).unwrap();
    let waiting = aside.join("20261105-000000.gone.json");
    std::fs::write(&waiting, serde_json::json!({"row": {"session": "20261105-000000", "game": "gone"}}).to_string()).unwrap();
    let out = universe(&dir, &["storage"], &[NO_BUS]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(!marker.exists() && !waiting.exists(), "no folder to file a row in: nothing is owed");
    assert!(!dir.path().join("data/games/gone").exists(), "and none is made for it");
    assert!(undone.exists(), "what the launch began is undone all the same");
}

#[test]
fn journal_add_files_an_entry_the_journal_lists_and_calls_only_a_refusal_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("quest.sh");
    std::fs::write(&exe, "").unwrap();
    let added = universe(&dir, &["add", exe.to_str().unwrap(), "--runner", "linux", "--title", "Quest", "--json"], &[]);
    let game: serde_json::Value = serde_json::from_slice(&added.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&added.stderr)));
    let id = game["id"].as_str().unwrap();
    let sid = "20261004-120000";
    let game_dir = dir.path().join("data/games").join(id);
    let (shot, frame) = (game_dir.join("screenshots/20261004-121500.png"), game_dir.join("journal/attachments/20261004-120000-1.png"));
    let copied = game_dir.join("journal/attachments/20261004-123000.png");
    for file in [&shot, &frame, &copied] {
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, "png").unwrap();
    }
    let entry = serde_json::json!({
        "session": sid, "game": id, "written_at": "2026-10-04T13:00:00+02:00", "lang": "en", "title": "First steps",
        "provider": "test", "paragraphs": ["Left the cave."], "next_up": "",
        "images": ["attachments/20261004-121500.png", "attachments/20261004-120000-1.png", "attachments/20261004-123000.png"],
    });
    let out = universe(&dir, &["journal-add", sid, &entry.to_string()], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let listed: serde_json::Value = serde_json::from_slice(&universe(&dir, &["journal", id, "--json"], &[]).stdout).unwrap();
    let listed = listed.as_array().unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(
        (&listed[0]["session"], &listed[0]["game"], &listed[0]["state"], &listed[0]["paragraphs"]),
        (&serde_json::json!(sid), &serde_json::json!(id), &serde_json::json!("written"), &entry["paragraphs"])
    );
    assert_eq!(listed[0]["images"], serde_json::json!([shot, frame, copied]), "an import's attachments/<shot> is the shot screenshots/ holds");

    // modules/journal gives up on "invalid:"; on "not found:" it writes the file itself at once, on anything else after retries.
    let escape = "../x";
    for (case, session, entry, refused) in [
        ("not JSON", sid, "{".to_string(), true),
        ("another session's", sid, serde_json::json!({"session": "20261004-130000", "game": id, "title": "t"}).to_string(), true),
        ("no title, no paragraphs", sid, serde_json::json!({"session": sid, "game": id}).to_string(), true),
        ("an image out of the journal", sid, serde_json::json!({"session": sid, "game": id, "title": "t", "images": ["../x.png"]}).to_string(), true),
        ("a session that is a path", escape, serde_json::json!({"session": escape, "game": id, "title": "t"}).to_string(), true),
        ("a path given only as the argument", escape, serde_json::json!({"game": id, "title": "t"}).to_string(), true),
        ("a game the library lost", sid, serde_json::json!({"session": sid, "game": "gone", "title": "t"}).to_string(), false),
    ] {
        let out = universe(&dir, &["journal-add", session, &entry], &[]);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{case}: {err}");
        assert_eq!(err.contains("invalid:"), refused, "{case}: {err}");
    }
    let escaped: Vec<_> = files_under(dir.path()).into_iter().filter(|p| p.file_name().is_some_and(|n| n == "x.json")).collect();
    assert!(escaped.is_empty(), "{escaped:?}");

    let out = universe(&dir, &["journal", id, "--remove", sid, "--yes"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let listed: serde_json::Value = serde_json::from_slice(&universe(&dir, &["journal", id, "--json"], &[]).stdout).unwrap();
    assert_eq!(listed, serde_json::json!([]));
    assert!(shot.is_file() && !frame.exists() && !copied.exists(), "the entry's frames and copies go with it, the player's shot stays");
}

#[test]
fn a_ludusavi_lookup_dies_with_the_cli_killed_mid_way() {
    let dir = no_desktop();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let pid_file = dir.path().join("ludusavi.pid");
    let stub = bin.join("ludusavi");
    std::fs::write(&stub, format!("#!/bin/sh\necho $$ > {0}.part && mv {0}.part {0}\nexec sleep 60\n", pid_file.display())).unwrap();
    std::fs::set_permissions(&stub, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let exe = dir.path().join("quest.sh");
    std::fs::write(&exe, "").unwrap();
    let added = universe(&dir, &["add", exe.to_str().unwrap(), "--runner", "linux", "--title", "Quest", "--json"], &[]);
    let game: serde_json::Value = serde_json::from_slice(&added.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&added.stderr)));
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    let mut cli = command(&dir, &["saves", "backup", game["id"].as_str().unwrap()], &[("PATH", &path)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    let pid = loop {
        if let Ok(pid) = std::fs::read_to_string(&pid_file) {
            break pid.trim().parse::<i32>().unwrap();
        }
        if started.elapsed().as_secs() >= 10 || cli.try_wait().unwrap().is_some() {
            let _ = cli.kill();
            let _ = cli.wait();
            panic!("the CLI never ran ludusavi");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    cli.kill().unwrap();
    cli.wait().unwrap();
    // Reaped, or a zombie its new parent has yet to reap: either way it runs no more.
    let alive = || std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| s.rsplit_once(") ").is_some_and(|(_, rest)| !rest.starts_with('Z')));
    let killed = std::time::Instant::now();
    while alive() && killed.elapsed().as_secs() < 2 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let outlived = alive();
    if outlived {
        // SAFETY: a plain signal to a pid this test started.
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(!outlived, "ludusavi {pid} outlived the CLI");
}

const FIXTURE_SAVES: &str = "drive_c/users/steamuser/AppData/Roaming/Fixture";

/// Fixture Game in a Proton prefix, keeping `keep` backups, with the real ludusavi behind a script on PATH.
struct SavedGame {
    dir: tempfile::TempDir,
    path: String,
    live: std::path::PathBuf,
}

impl SavedGame {
    /// `before` is shell, run ahead of every backup that writes; `$path` is where ludusavi will write it. A `crash` file kills the CLI once such a backup is written.
    fn new(keep: u32, before: impl Fn(&std::path::Path) -> String) -> SavedGame {
        let real = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|d| d.join("ludusavi"))
            .find(|p| p.is_file())
            .expect("ludusavi is not on PATH: the dev shell and the Nix check have it");
        let dir = no_desktop();
        std::fs::write(dir.path().join("config/config.toml"), format!("schema = 1\n[desktop]\nprofile = \"none\"\n[saves]\nkeep = {keep}\n")).unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            "#!/bin/sh\npath=; prev=; writes=\nfor a in \"$@\"; do [ \"$prev\" = --path ] && path=$a; prev=$a; done\ncase \" $* \" in *\" --preview \"*) ;; *\" backup \"*)\nwrites=1\n{}\n;; esac\n{} \"$@\"\nstatus=$?\n[ -n \"$writes\" ] && [ -e {} ] && kill -9 $PPID\nexit $status\n",
            before(dir.path()),
            real.display(),
            dir.path().join("crash").display()
        );
        std::fs::write(bin.join("ludusavi"), script).unwrap();
        std::fs::set_permissions(bin.join("ludusavi"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let tools = dir.path().join("data/ludusavi");
        std::fs::create_dir_all(&tools).unwrap();
        let entry = |title: &str, folder: &str| format!("{title}:\n  files:\n    \"<winAppData>/{folder}\":\n      when:\n        - os: windows\n");
        std::fs::write(tools.join("manifest.yaml"), entry("Fixture Game", "Fixture") + &entry("Other Game", "Other")).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        std::fs::write(tools.join("cache.yaml"), format!("manifests:\n  \"https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml\":\n    checked: \"{now}\"\n    updated: \"{now}\"\n")).unwrap();
        let exe = dir.path().join("Game.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let added = universe(&dir, &["add", exe.to_str().unwrap(), "--runner", "proton", "--title", "Fixture Game", "--json"], &[]);
        assert!(added.status.success(), "{}", text(&added.stderr));
        let live = dir.path().join("data/prefixes/fixture-game").join(FIXTURE_SAVES);
        std::fs::create_dir_all(&live).unwrap();
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
        SavedGame { dir, path, live }
    }

    fn saves(&self, args: &[&str]) -> std::process::Output {
        universe(&self.dir, &[&["saves"], args].concat(), &[("PATH", &self.path)])
    }

    fn spawn_backup(&self) -> std::process::Child {
        self.spawn(&["backup", "fixture-game"])
    }

    fn spawn(&self, args: &[&str]) -> std::process::Child {
        command(&self.dir, &[&["saves"], args].concat(), &[("PATH", &self.path)])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn put(&self, slot: &str, bytes: &str) {
        std::fs::write(self.live.join(slot), bytes).unwrap();
    }

    fn read(&self, slot: &str) -> String {
        std::fs::read_to_string(self.live.join(slot)).unwrap()
    }

    /// What each backup holds of `slot`, read from the backup itself.
    fn held(&self, slot: &str) -> Vec<String> {
        let list = self.saves(&["list", "fixture-game", "--json"]);
        let data: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&list.stderr)));
        let file = |b: &serde_json::Value| std::path::Path::new(b["path"].as_str().unwrap()).join("drive-0/universe/prefix").join(FIXTURE_SAVES).join(slot);
        data["backups"].as_array().unwrap().iter().map(|b| std::fs::read_to_string(file(b)).unwrap_or_default()).collect()
    }

    fn mapping(&self) -> std::path::PathBuf {
        self.dir.path().join("data/saves/fixture-game/Fixture Game/mapping.yaml")
    }

    /// Other Game beside the fixture, with a save.
    fn add_other_game(&self) {
        let exe = self.dir.path().join("Other.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let added = universe(&self.dir, &["add", exe.to_str().unwrap(), "--runner", "proton", "--title", "Other Game", "--json"], &[]);
        assert!(added.status.success(), "{}", text(&added.stderr));
        let live = self.dir.path().join("data/prefixes/other-game/drive_c/users/steamuser/AppData/Roaming/Other");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::write(live.join("slot1.sav"), "other").unwrap();
    }

    /// The process `args` start, stopped by a fifo in the mapping's place where it first reads it. Writing `mapping` to the fifo lets it go; anyone else finds the file `mapping` back in its place.
    fn stopped_at_the_mapping(&self, args: &[&str], mapping: &str) -> (std::process::Child, std::fs::File) {
        use std::os::unix::fs::OpenOptionsExt;
        let file = self.mapping();
        let beside = file.with_file_name("beside.yaml");
        std::fs::write(&beside, mapping).unwrap();
        // A fifo in the mapping's place stops the first process between reading the mapping and acting on it.
        std::fs::remove_file(&file).unwrap();
        rustix::fs::mknodat(rustix::fs::CWD, &file, rustix::fs::FileType::Fifo, rustix::fs::Mode::from_raw_mode(0o600), 0).unwrap();
        let process = self.spawn(args);
        let started = std::time::Instant::now();
        let fifo = loop {
            match std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(&file) {
                Ok(fifo) => break fifo,
                Err(_) if started.elapsed().as_secs() < 10 => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(e) => panic!("{args:?} never read the mapping: {e}"),
            }
        };
        std::fs::rename(&beside, &file).unwrap();
        (process, fifo)
    }

    fn backup_folder(&self) -> std::path::PathBuf {
        let list = self.saves(&["list", "fixture-game", "--json"]);
        let data: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&list.stderr)));
        data["backups"][0]["path"].as_str().unwrap().into()
    }
}

/// ludusavi cannot write slot2.sav while `obstruct` exists, whichever folder of the second it picks.
fn slot2_obstructed(dir: &std::path::Path) -> String {
    format!(
        "if [ -e {flag} ]; then\nrel=drive-0/universe/prefix/{FIXTURE_SAVES}/slot2.sav\nrm -rf \"$path/Fixture Game/$rel\"\nmkdir -p \"$path/Fixture Game/$rel/taken\"\nnow=$(date +%s)\nfor s in 0 1 2 3 4 5; do mkdir -p \"$path/Fixture Game/backup-$(date -u -d @$((now + s)) +%Y%m%dT%H%M%SZ)/$rel/taken\"; done\nfi",
        flag = dir.join("obstruct").display()
    )
}

fn clear_obstruction(game: &SavedGame) {
    fn remove_slot2_folders(dir: &std::path::Path) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
            if entry.file_name() == "slot2.sav" {
                std::fs::remove_dir_all(entry.path()).unwrap();
            } else {
                remove_slot2_folders(&entry.path());
            }
        }
    }
    std::fs::remove_file(game.dir.path().join("obstruct")).unwrap();
    remove_slot2_folders(&game.dir.path().join("data/saves"));
}

#[test]
fn a_restore_whose_backup_failed_part_way_refuses_and_its_retry_keeps_every_save() {
    let game = SavedGame::new(2, slot2_obstructed);
    game.put("slot1.sav", "one");
    game.put("slot2.sav", "two");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    game.put("slot1.sav", "newer one");
    game.put("slot2.sav", "newer two");

    std::fs::write(game.dir.path().join("obstruct"), "").unwrap();
    let refused = game.saves(&["restore", "fixture-game", "--yes"]);
    assert!(!refused.status.success(), "{}", text(&refused.stdout));
    assert_eq!((game.read("slot1.sav").as_str(), game.read("slot2.sav").as_str()), ("newer one", "newer two"));

    clear_obstruction(&game);
    let retry = game.saves(&["restore", "fixture-game", "--yes"]);
    assert!(retry.status.success(), "{}", text(&retry.stderr));
    assert_eq!((game.read("slot1.sav").as_str(), game.read("slot2.sav").as_str()), ("one", "two"));
    assert!(
        game.held("slot1.sav").contains(&"newer one".to_string()) && game.held("slot2.sav").contains(&"newer two".to_string()),
        "{:?}",
        game.held("slot2.sav")
    );
}

#[test]
fn a_backup_that_failed_part_way_at_a_limit_of_one_loses_nothing_and_its_retry_is_whole() {
    let game = SavedGame::new(1, slot2_obstructed);
    game.put("slot1.sav", "one");
    game.put("slot2.sav", "two");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    game.put("slot1.sav", "newer one");
    game.put("slot2.sav", "newer two");

    std::fs::write(game.dir.path().join("obstruct"), "").unwrap();
    assert!(!game.saves(&["backup", "fixture-game"]).status.success());
    clear_obstruction(&game);
    assert!(game.held("slot2.sav").contains(&"two".to_string()) || game.held("slot2.sav").contains(&"newer two".to_string()), "{:?}", game.held("slot2.sav"));

    let retry = game.saves(&["backup", "fixture-game"]);
    assert!(retry.status.success(), "{}", text(&retry.stderr));
    assert_eq!((game.held("slot1.sav"), game.held("slot2.sav")), (vec!["newer one".to_string()], vec!["newer two".to_string()]));
}

#[test]
fn a_backup_in_another_process_survives_the_repair_of_a_stale_mapping() {
    use std::io::Write;
    // Each backup sees saves of its own: a game still writing them.
    let game = SavedGame::new(5, |dir| {
        let (count, save) = (dir.join("runs"), dir.join("data/prefixes/fixture-game").join(FIXTURE_SAVES).join("slot1.sav"));
        format!("n=$(($(cat {0} 2>/dev/null || echo 0) + 1))\necho $n > {0}\nprintf save-%s $n > {1}", count.display(), save.display())
    });
    game.put("slot1.sav", "");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());

    let mapping = game.mapping();
    let phantom =
        "  - name: backup-20000101T000000Z\n    when: \"2000-01-01T00:00:00Z\"\n    os: linux\n    files: {}\n    registry:\n      hash: ~\n    children: []\n";
    let stale = std::fs::read_to_string(&mapping).unwrap().trim_end().to_string() + "\n" + phantom;
    let (first, mut reading) = game.stopped_at_the_mapping(&["backup", "fixture-game"], &stale);

    let mut second = game.spawn_backup();
    let window = std::time::Instant::now();
    while second.try_wait().unwrap().is_none() && window.elapsed().as_millis() < 500 {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(second.try_wait().unwrap().is_none(), "the second backup ran beside the first's reading of the mapping");
    reading.write_all(stale.as_bytes()).unwrap();
    drop(reading);
    for process in [first.wait_with_output(), second.wait_with_output()] {
        let done = process.unwrap();
        assert!(done.status.success(), "{}", text(&done.stderr));
    }

    let runs: u32 = std::fs::read_to_string(game.dir.path().join("runs")).unwrap().trim().parse().unwrap();
    let held = game.held("slot1.sav");
    for run in 1..=runs {
        assert!(held.contains(&format!("save-{run}")), "backup {run} of {runs} is gone: {held:?}");
    }
    assert!(!std::fs::read_to_string(&mapping).unwrap().contains("backup-20000101T000000Z"), "the record with no folder is forgotten");
}

#[test]
fn a_restore_and_an_export_in_another_process_wait_out_a_running_backup() {
    // Only the first backup to get here stops: one that overtakes it runs through.
    let game = SavedGame::new(5, |dir| {
        format!("if mkdir {0} 2>/dev/null; then while [ -e {1} ]; do sleep 0.02; done; fi", dir.join("started").display(), dir.join("hold").display())
    });
    game.put("slot1.sav", "selected");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    let list: serde_json::Value = serde_json::from_slice(&game.saves(&["list", "fixture-game", "--json"]).stdout).unwrap();
    let selected = list["backups"][0]["id"].as_str().unwrap().to_string();
    game.put("slot1.sav", "progress");

    let (started, hold, zips) = (game.dir.path().join("started"), game.dir.path().join("hold"), game.dir.path().join("zips"));
    std::fs::remove_dir(&started).unwrap();
    std::fs::write(&hold, "").unwrap();
    let backup = game.spawn_backup();
    let waiting = std::time::Instant::now();
    while !started.exists() {
        assert!(waiting.elapsed().as_secs() < 10, "the backup never reached ludusavi");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut restore = game.spawn(&["restore", "fixture-game", &selected, "--yes"]);
    let mut export = game.spawn(&["export", "fixture-game", zips.to_str().unwrap()]);
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(restore.try_wait().unwrap().is_none(), "the restore ran beside a backup");
    assert!(export.try_wait().unwrap().is_none(), "the export ran beside a backup");
    assert_eq!(game.read("slot1.sav"), "progress");

    std::fs::remove_file(&hold).unwrap();
    for process in [backup.wait_with_output(), restore.wait_with_output(), export.wait_with_output()] {
        let done = process.unwrap();
        assert!(done.status.success(), "{}", text(&done.stderr));
    }
    assert_eq!(game.read("slot1.sav"), "selected");
    let held = game.held("slot1.sav");
    assert!(held.contains(&"selected".to_string()) && held.contains(&"progress".to_string()), "{held:?}");
    assert!(!game.dir.path().join("data/saves/.staging").exists());
}

#[test]
fn a_crash_between_staging_and_commit_costs_nothing_and_its_leftovers_go_with_the_retry() {
    let game = SavedGame::new(2, |_| String::new());
    let (crash, staging) = (game.dir.path().join("crash"), game.dir.path().join("data/saves/.staging"));

    game.put("slot1.sav", "first");
    std::fs::write(&crash, "").unwrap();
    assert!(!game.saves(&["backup", "fixture-game"]).status.success());
    assert!(files_under(&staging).iter().any(|f| f.ends_with("slot1.sav")), "the crash came before ludusavi staged");
    assert!(game.held("slot1.sav").is_empty());
    // Left by another game's backup, the folder is the only name it goes by.
    std::fs::rename(staging.join("Fixture Game"), staging.join("Other Game")).unwrap();
    std::fs::remove_file(&crash).unwrap();
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    assert_eq!(game.held("slot1.sav"), ["first"]);
    assert!(!game.dir.path().join("data/saves/fixture-game/Other Game").exists(), "a stale folder joined this game's backups");

    game.put("slot1.sav", "progress");
    std::fs::write(&crash, "").unwrap();
    assert!(!game.saves(&["restore", "fixture-game", "--yes"]).status.success());
    assert!(files_under(&staging).iter().any(|f| f.ends_with("slot1.sav")), "the crash came before ludusavi staged");
    assert_eq!((game.read("slot1.sav").as_str(), game.held("slot1.sav")), ("progress", vec!["first".to_string()]));
    std::fs::remove_file(&crash).unwrap();

    // What a crash leaves between the rename into the backups and the mapping's write.
    let orphan = game.mapping().with_file_name("backup-20000101T000000Z");
    std::fs::create_dir_all(orphan.join("drive-0")).unwrap();
    assert_eq!(game.held("slot1.sav"), ["first"]);

    let retry = game.saves(&["restore", "fixture-game", "--yes"]);
    assert!(retry.status.success(), "{}", text(&retry.stderr));
    assert_eq!(game.read("slot1.sav"), "first");
    let mut held = game.held("slot1.sav");
    held.sort();
    assert_eq!(held, ["first", "progress"]);
    assert!(!staging.exists() && !orphan.exists());
}

/// `process`'s output once it ends, `None` when it outlives `limit` and is killed.
fn output_within(mut process: std::process::Child, limit: std::time::Duration) -> Option<std::process::Output> {
    let started = std::time::Instant::now();
    while process.try_wait().unwrap().is_none() {
        if started.elapsed() > limit {
            process.kill().unwrap();
            process.wait().unwrap();
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    Some(process.wait_with_output().unwrap())
}

/// Long enough for `process` to have ended on a loaded machine, were nothing holding it.
fn waits(process: &mut std::process::Child) -> bool {
    let started = std::time::Instant::now();
    while started.elapsed() < speed_limit(500, 3000) {
        if process.try_wait().unwrap().is_some() {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    true
}

fn only_zip(folder: &std::path::Path) -> zip::ZipArchive<std::fs::File> {
    let found: Vec<_> = std::fs::read_dir(folder).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].file_name().unwrap().to_string_lossy().starts_with("fixture-game-saves-"), "{found:?}");
    zip::ZipArchive::new(std::fs::File::open(&found[0]).unwrap()).unwrap()
}

fn read_zipped(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> String {
    let mut bytes = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(name).unwrap_or_else(|e| panic!("{name}: {e}")), &mut bytes).unwrap();
    bytes
}

#[test]
fn another_games_saves_answer_beside_a_long_export_within_its_budget() {
    use std::io::Write;
    let game = SavedGame::new(5, |_| String::new());
    game.put("slot1.sav", "saved");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    game.add_other_game();
    let mapping = std::fs::read_to_string(game.mapping()).unwrap();
    let zips = game.dir.path().join("zips");
    let (export, mut fifo) = game.stopped_at_the_mapping(&["export", "fixture-game", zips.to_str().unwrap()], &mapping);

    // Its own game's backup and restore wait for it, and hold up no other game while they do.
    let waiting = [game.spawn_backup(), game.spawn(&["restore", "fixture-game", "--yes"])];
    std::thread::sleep(std::time::Duration::from_millis(100));

    let limit = speed_limit(1500, 20000);
    let list = output_within(game.spawn(&["list", "other-game"]), limit);
    let backup = output_within(game.spawn(&["backup", "other-game"]), limit);
    fifo.write_all(mapping.as_bytes()).unwrap();
    drop(fifo);
    let done = export.wait_with_output().unwrap();
    for process in waiting {
        let ended = process.wait_with_output().unwrap();
        assert!(ended.status.success(), "{}", text(&ended.stderr));
    }

    for (what, ended) in [("list", list), ("backup", backup)] {
        let ended = ended.unwrap_or_else(|| panic!("another game's {what} waited for the export"));
        assert!(ended.status.success(), "{what}: {}", text(&ended.stderr));
    }
    assert!(done.status.success(), "{}", text(&done.stderr));
    only_zip(&zips);
}

#[test]
fn an_export_beside_a_pruning_backup_zips_every_backup_whole() {
    use std::io::Write;
    let game = SavedGame::new(1, |_| String::new());
    game.put("slot1.sav", "kept");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    let exported = game.backup_folder();
    let id = exported.file_name().unwrap().to_string_lossy().into_owned();
    let mapping = std::fs::read_to_string(game.mapping()).unwrap();
    let zips = game.dir.path().join("zips");
    let (export, mut fifo) = game.stopped_at_the_mapping(&["export", "fixture-game", zips.to_str().unwrap()], &mapping);

    game.put("slot1.sav", "newer");
    let mut backup = game.spawn_backup();
    let waited = waits(&mut backup);
    let held = exported.exists();
    fifo.write_all(mapping.as_bytes()).unwrap();
    drop(fifo);
    for process in [export.wait_with_output(), backup.wait_with_output()] {
        let done = process.unwrap();
        assert!(done.status.success(), "{}", text(&done.stderr));
    }
    assert!(waited && held, "the backup pruned under the export");

    let mut zip = only_zip(&zips);
    assert!(read_zipped(&mut zip, "fixture-game/Fixture Game/mapping.yaml").contains(&id));
    assert_eq!(read_zipped(&mut zip, &format!("fixture-game/Fixture Game/{id}/drive-0/universe/prefix/{FIXTURE_SAVES}/slot1.sav")), "kept");
    assert_eq!(game.held("slot1.sav"), ["newer"]);

    let mapping = std::fs::read_to_string(game.mapping()).unwrap();
    let zips = game.dir.path().join("zips again");
    let (export, mut fifo) = game.stopped_at_the_mapping(&["export", "fixture-game", zips.to_str().unwrap()], &mapping);
    let mut restore = game.spawn(&["restore", "fixture-game", "--yes"]);
    let waited = waits(&mut restore);
    fifo.write_all(mapping.as_bytes()).unwrap();
    drop(fifo);
    for process in [export.wait_with_output(), restore.wait_with_output()] {
        let done = process.unwrap();
        assert!(done.status.success(), "{}", text(&done.stderr));
    }
    assert!(waited, "the restore ran beside the export");
}

#[test]
fn a_failed_export_leaves_no_zip_and_names_the_file_it_could_not_read() {
    use std::os::unix::fs::PermissionsExt;
    let game = SavedGame::new(5, |_| String::new());
    game.put("slot1.sav", "saved");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    let stored = game.backup_folder().join("drive-0/universe/prefix").join(FIXTURE_SAVES).join("slot1.sav");
    let zips = game.dir.path().join("zips");

    std::fs::set_permissions(&stored, std::fs::Permissions::from_mode(0o000)).unwrap();
    let failed = game.saves(&["export", "fixture-game", zips.to_str().unwrap()]);
    std::fs::set_permissions(&stored, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!failed.status.success(), "{}", text(&failed.stdout));
    assert!(text(&failed.stdout).contains(stored.to_str().unwrap()), "{}", text(&failed.stdout));
    assert_eq!(files_under(&zips), Vec::<std::path::PathBuf>::new());

    let done = game.saves(&["export", "fixture-game", zips.to_str().unwrap()]);
    assert!(done.status.success(), "{}", text(&done.stderr));
    let id = game.backup_folder().file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(read_zipped(&mut only_zip(&zips), &format!("fixture-game/Fixture Game/{id}/drive-0/universe/prefix/{FIXTURE_SAVES}/slot1.sav")), "saved");
}

#[test]
fn a_mapping_that_cant_be_read_is_reported_and_costs_no_backup() {
    let game = SavedGame::new(1, |_| String::new());
    game.put("slot1.sav", "kept");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    let mapping = game.mapping();
    let (good, folder) = (std::fs::read_to_string(&mapping).unwrap(), game.backup_folder());
    let stored = folder.join("drive-0/universe/prefix").join(FIXTURE_SAVES).join("slot1.sav");
    let named = mapping.to_str().unwrap();
    game.put("slot1.sav", "progress");

    for (broken, damage) in [("cut short", Some("name: Fixture Game\nbackups: [")), ("empty", Some("")), ("not a mapping", Some("words")), ("gone", None)] {
        match damage {
            Some(bytes) => std::fs::write(&mapping, bytes).unwrap(),
            None => std::fs::remove_file(&mapping).unwrap(),
        }
        let list = game.saves(&["list", "fixture-game", "--json"]);
        let data: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap_or_else(|e| panic!("{broken}: {e}: {}", text(&list.stderr)));
        assert!(data["error"].as_str().unwrap().contains(named), "{broken}: {data}");
        assert_eq!(data["unreadable"], named, "{broken}: {data}");
        assert_eq!(data["files"].as_array().unwrap().len(), 1, "{broken}: the live saves are still found: {data}");

        for action in [
            &["saves", "backup", "fixture-game"][..],
            &["saves", "restore", "fixture-game", "--yes"][..],
            &["data", "fixture-game", "reset-prefix", "--yes"][..],
        ] {
            let refused = universe(&game.dir, action, &[("PATH", &game.path)]);
            assert!(!refused.status.success(), "{broken}: {action:?}: {}", text(&refused.stdout));
            assert!(text(&refused.stderr).contains(named), "{broken}: {action:?}: {}", text(&refused.stderr));
        }
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "kept", "{broken}");
        assert_eq!(game.read("slot1.sav"), "progress", "{broken}");

        let zips = game.dir.path().join("zips").join(broken);
        let exported = game.saves(&["export", "fixture-game", zips.to_str().unwrap()]);
        assert!(exported.status.success(), "{broken}: {}", text(&exported.stderr));
        let id = folder.file_name().unwrap().to_string_lossy();
        assert_eq!(
            read_zipped(&mut only_zip(&zips), &format!("fixture-game/Fixture Game/{id}/drive-0/universe/prefix/{FIXTURE_SAVES}/slot1.sav")),
            "kept",
            "{broken}"
        );
        std::fs::write(&mapping, &good).unwrap();
    }

    assert_eq!(game.saves(&["list", "fixture-game", "--json"]).status.code(), Some(0));
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    assert_eq!(game.held("slot1.sav"), ["progress"], "the file put back, backups go on");

    let title = mapping.parent().unwrap();
    std::fs::write(&mapping, "words").unwrap();
    std::fs::rename(title, game.dir.path().join("moved away")).unwrap();
    std::fs::create_dir_all(title).unwrap();
    let list = game.saves(&["list", "fixture-game", "--json"]);
    let data: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!((&data["error"], &data["unreadable"]), (&serde_json::json!(""), &serde_json::Value::Null), "an empty folder lost nothing");
    game.put("slot1.sav", "fresh start");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    assert_eq!(game.held("slot1.sav"), ["fresh start"], "the guide's way forward: the folder moved, the next backup starts a new list");
}

#[test]
fn a_backup_reaches_the_disk_before_the_one_it_prunes_goes() {
    let game = SavedGame::new(1, |_| String::new());
    game.put("slot1.sav", "kept");
    assert!(game.saves(&["backup", "fixture-game"]).status.success());
    let old = game.backup_folder().file_name().unwrap().to_string_lossy().into_owned();
    game.put("slot1.sav", "newer");

    let log = game.dir.path().join("strace.log");
    let backup = command(&game.dir, &["saves", "backup", "fixture-game"], &[("PATH", &game.path)]);
    let mut traced = Command::new("strace");
    traced
        .args(["-f", "-y", "-qq", "-s", "512", "-e", "trace=fsync,fdatasync,rename,renameat,renameat2,unlinkat", "-o"])
        .arg(&log)
        .arg(backup.get_program())
        .args(backup.get_args());
    traced.envs(backup.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))));
    let done = traced.output().expect("strace is not on PATH: the dev shell and the Nix check have it");
    assert!(done.status.success(), "{}", text(&done.stderr));
    assert_eq!(game.held("slot1.sav"), ["newer"]);

    let trace = std::fs::read_to_string(&log).unwrap();
    let calls: Vec<&str> = trace.lines().collect();
    let first = |what: &str, seen: &dyn Fn(&str) -> bool| calls.iter().position(|l| seen(l)).unwrap_or_else(|| panic!("no {what} in:\n{trace}"));
    let new = game.backup_folder().file_name().unwrap().to_string_lossy().into_owned();
    let unlink = first("unlink of the old backup", &|l| l.contains("unlinkat(") && l.contains(&format!("/{old}/")) && !l.contains(".staging"));
    let moved = first("rename into the store", &|l| l.contains("rename") && l.contains(".staging") && l.contains(&format!("/{new}\"")));
    // Its name while staged: within the second of the old backup, the store gives it another.
    let staged = calls[moved].split(".staging/Fixture Game/").nth(1).and_then(|l| l.split('"').next()).unwrap();
    let file = first("fsync of the new file", &|l| l.contains("fsync(") && l.contains(&format!("/{staged}/")) && l.contains("slot1.sav>"));
    let folder = first("fsync of the new folder", &|l| l.contains("fsync(") && l.contains(&format!("/{staged}>")));
    let written = first("fsync of the mapping", &|l| l.contains("fsync(") && l.contains("/mapping.tmp>"));
    let committed = first("rename of the mapping", &|l| l.contains("rename") && l.contains("mapping.tmp"));
    let store = |after: usize| calls[after..].iter().position(|l| l.contains("fsync(") && l.contains("/Fixture Game>")).map(|at| at + after);
    let (entry, record) = (store(moved).expect("no fsync of the store after the rename"), store(committed).expect("no fsync of the store after the mapping"));
    assert!(file < moved && folder < moved, "{trace}");
    assert!(moved < entry && entry < committed, "{trace}");
    assert!(written < committed && committed < record && record < unlink, "{trace}");
}

fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().flat_map(|e| if e.path().is_dir() { files_under(&e.path()) } else { vec![e.path()] }).collect()
}

/// The strict limit at conftest.py's default 2 s test budget, the loose one where it was raised or turned off (a loaded run, the flake).
fn speed_limit(strict_ms: u64, loose_ms: u64) -> std::time::Duration {
    let strict = std::env::var("UNIVERSE_TEST_BUDGET_S").map_or(true, |b| b.parse::<f64>() == Ok(2.0));
    std::time::Duration::from_millis(if strict { strict_ms } else { loose_ms })
}

#[test]
fn ls_lists_a_thousand_games_within_its_budget() {
    let dir = no_desktop();
    for i in 0..1000 {
        let game = dir.path().join(format!("data/games/game-{i:04}"));
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("game.toml"), format!("schema = 1\nid = \"game-{i:04}\"\ntitle = \"Game {i}\"\n")).unwrap();
    }
    let started = std::time::Instant::now();
    let out = universe(&dir, &["ls", "--json"], &[NO_BUS]);
    let took = started.elapsed();
    let games: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stderr)));
    assert_eq!(games.len(), 1000);
    assert!(took < speed_limit(1500, 6000), "listed in {took:?}");
}

#[path = "../src/testpe.rs"]
mod testpe;

#[test]
fn info_reads_the_installed_version_from_the_program_until_set_pins_one() {
    let dir = no_desktop();
    let exe = dir.path().join("games/Sekiro/sekiro.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    testpe::Exe { strings: &[("ProductVersion", "1.6.0.0")], ..Default::default() }.write(&exe);
    let game = dir.path().join("data/games/sekiro");
    std::fs::create_dir_all(&game).unwrap();
    let toml = format!("schema = 1\nid = \"sekiro\"\ntitle = \"Sekiro\"\n[launch]\nrunner = \"proton\"\nexe = \"{}\"\n", exe.display());
    std::fs::write(game.join("game.toml"), toml).unwrap();
    let version = || {
        let out = universe(&dir, &["--json", "info", "sekiro"], &[NO_BUS]);
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stderr)))["version"].clone()
    };
    let v = version();
    assert_eq!(
        (v["installed"].as_str(), v["from"].as_str(), v["channel"].as_str(), v["final"].as_bool()),
        (Some("1.6.0.0"), Some("exe"), Some("pc"), Some(false))
    );
    let set = universe(&dir, &["set", "sekiro", "version.pin=1.06", "version.seen=5794815", "version.final=true"], &[NO_BUS]);
    assert!(set.status.success(), "{}", text(&set.stderr));
    let v = version();
    assert_eq!(
        (v["installed"].as_str(), v["from"].as_str(), v["seen"].as_str(), v["final"].as_bool()),
        (Some("1.06"), Some("pin"), Some("5794815"), Some(true)),
        "a pin wins, a build stays text"
    );
    assert!(!universe(&dir, &["set", "sekiro", "version.switch_title_id=0100"], &[NO_BUS]).status.success());
}

#[test]
fn restore_brings_a_removed_game_back_with_its_recordings_and_overwrites_none() {
    let dir = no_desktop();
    let rec = dir.path().join("recordings");
    std::fs::write(
        dir.path().join("config/config.toml"),
        format!("schema = 1\n[desktop]\nprofile = \"none\"\n[paths]\nrecordings_root = \"{}\"\n", rec.display()),
    )
    .unwrap();
    let game = dir.path().join("data/games/dishonored");
    std::fs::create_dir_all(&game).unwrap();
    std::fs::write(
        game.join("game.toml"),
        "schema = 1\nid = \"dishonored\"\ntitle = \"Dishonored\"\n[source]\nkind = \"lutris\"\n[lutris]\nslug = \"dishonored\"\n",
    )
    .unwrap();
    let (live, archived) = (rec.join("dishonored"), rec.join(".archive/dishonored"));
    std::fs::create_dir_all(live.join("clips")).unwrap();
    for (name, text) in [("a.mkv", "archived"), ("b.mkv", "b"), ("clips/c.mkv", "c")] {
        std::fs::write(live.join(name), text).unwrap();
    }
    let listed = |dir: &tempfile::TempDir| -> Vec<serde_json::Value> { serde_json::from_slice(&universe(dir, &["ls", "--json"], &[NO_BUS]).stdout).unwrap() };
    assert_eq!(listed(&dir)[0]["source"]["kind"], "manual", "a Lutris import from before reads as added by hand");

    assert!(universe(&dir, &["rm", "-y", "dishonored"], &[NO_BUS]).status.success());
    assert!(archived.join("b.mkv").is_file() && !live.exists(), "rm archives the recordings");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::write(live.join("a.mkv"), "recorded since").unwrap();
    let out = universe(&dir, &["restore", "dishonored"], &[NO_BUS]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(std::fs::read_to_string(live.join("a.mkv")).unwrap(), "recorded since", "never over a file already there");
    assert_eq!(std::fs::read_to_string(archived.join("a.mkv")).unwrap(), "archived", "which stays archived");
    assert!(text(&out.stderr).contains(&*archived.join("a.mkv").to_string_lossy()), "and is named");
    assert!(live.join("b.mkv").is_file() && live.join("clips/c.mkv").is_file(), "the rest back in place");
    assert!(!archived.join("b.mkv").exists() && !archived.join("clips").exists());
    let games = listed(&dir);
    assert_eq!((games.len(), games[0]["hidden"].as_bool(), games[0]["removed"].as_bool()), (1, Some(false), Some(false)), "in the library again");
    assert!(!universe(&dir, &["restore", "dishonored"], &[NO_BUS]).status.success(), "a game in the library has nothing to restore");

    assert!(universe(&dir, &["rm", "-y", "dishonored"], &[NO_BUS]).status.success());
    assert!(archived.join("b.mkv").is_file() && archived.join("clips/c.mkv").is_file(), "a second rm archives next to what stayed archived");
    assert_eq!(std::fs::read_to_string(live.join("a.mkv")).unwrap(), "recorded since", "and never over it");
}

#[test]
fn update_lists_a_game_updated_by_hand_until_it_is_marked_current() {
    let dir = no_desktop();
    let exe = dir.path().join("games/GTA V/GTA5_Enhanced.exe");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    testpe::Exe { strings: &[("ProductVersion", "1.0.1158.13")], ..Default::default() }.write(&exe);
    let game = dir.path().join("data/games/gta");
    std::fs::create_dir_all(&game).unwrap();
    let toml = format!(
        "schema = 1\nid = \"gta\"\ntitle = \"GTA V\"\n[launch]\nrunner = \"proton\"\nexe = \"{}\"\n[metadata]\nsteam_appid = 3240220\n[version]\nseen = \"24129078\"\n",
        exe.display()
    );
    std::fs::write(game.join("game.toml"), &toml).unwrap();
    let hidden = dir.path().join("data/games/hidden");
    std::fs::create_dir_all(&hidden).unwrap();
    std::fs::write(hidden.join("game.toml"), toml.replace("\"gta\"", "\"hidden\"").replace("\"GTA V\"", "\"Put Away\"\nhidden = true")).unwrap();
    // Steam's answer kept an hour ago: the check asks nothing.
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - 3600;
    let kept = format!(r#"{{"steam": {{"3240220": {{"build": "24129523", "date": "2026-09-11", "at": {at}}}}}}}"#);
    std::fs::create_dir_all(dir.path().join("cache")).unwrap();
    std::fs::write(dir.path().join("cache/upstream.json"), kept).unwrap();
    let rows = |args: &[&str]| {
        let out = universe(&dir, &[&["--json", "update"], args].concat(), &[NO_BUS]);
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stderr)));
        rows.iter()
            .map(|r| [&r["id"], &r["status"], &r["how"], &r["latest"], &r["seen"]].map(|v| v.as_str().unwrap_or_default().to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&[]), [["gta", "update", "manual", "24129523", "24129078"]], "a hidden game's update is not pending");
    let one = universe(&dir, &["update", "gta"], &[NO_BUS]);
    assert!(!one.status.success() && text(&one.stderr).contains("universe update gta --mark-current"), "{}", text(&one.stderr));
    assert!(!universe(&dir, &["update", "gta", "--mark-current", "v2"], &[NO_BUS]).status.success(), "a build is a number");
    let marked = universe(&dir, &["update", "gta", "--mark-current", "24129523"], &[NO_BUS]);
    assert!(marked.status.success(), "{}", text(&marked.stderr));
    assert!(rows(&[]).is_empty());
    assert_eq!(rows(&["--all"]), [["gta", "current", "manual", "24129523", "24129523"], ["hidden", "update", "manual", "24129523", "24129078"]]);
}

#[test]
fn session_window_without_a_session_fails_at_once_even_told_to_wait() {
    let dir = no_desktop();
    for args in [&["session-window", "--json"][..], &["session-window", "--wait", "60", "--json"]] {
        let started = std::time::Instant::now();
        let out = universe(&dir, args, &[NO_BUS]);
        assert!(started.elapsed() < std::time::Duration::from_secs(20), "{args:?} waited for a session that is not there");
        assert_eq!((out.status.code(), text(&out.stdout)), (Some(1), String::new()), "{args:?}");
        assert!(text(&out.stderr).contains("not found:"), "{args:?}: {}", text(&out.stderr));
    }
}

#[test]
fn screen_mode_answers_zeros_for_a_connector_that_is_not_there() {
    let dir = no_desktop();
    let mode = |args: &[&str]| -> serde_json::Value {
        let out = universe(&dir, args, &[NO_BUS]);
        assert!(out.status.success(), "{args:?}: {}", text(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    };
    assert_eq!(mode(&["screen-mode", "NOPE-9", "--json"]), serde_json::json!({"screen": "NOPE-9", "width": 0, "height": 0, "refresh": 0, "vrr": false}));
    let picked = mode(&["screen-mode", "--json"]);
    let mut keys: Vec<&str> = picked.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["height", "refresh", "screen", "vrr", "width"]);
    assert!(picked["refresh"].is_u64(), "{picked}");
}

#[test]
fn generate_markdown_writes_a_page_per_visible_command_and_the_global_options_once() {
    let dir = tempfile::tempdir().unwrap();
    let pages = dir.path().join("pages");
    let out = universe(&dir, &["__generate", "--markdown", pages.to_str().unwrap()], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let names: Vec<String> = std::fs::read_dir(&pages).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    for page in ["universe.md", "ls.md", "launch-keys.md", "media.md", "controller.md"] {
        assert!(names.iter().any(|n| n == page), "no {page}: {names:?}");
    }
    for hidden in ["__generate.md", "__complete.md", "osd.md", "session-end.md", "splash.md", "overlay.md"] {
        assert!(!names.iter().any(|n| n == hidden), "{hidden} is hidden: {names:?}");
    }
    let read = |name: &str| std::fs::read_to_string(pages.join(name)).unwrap();
    assert!(read("media.md").contains("\n## universe media refresh\n"), "{}", read("media.md"));
    assert!(read("universe.md").contains("`--json`") && !read("ls.md").contains("--json"), "{}", read("ls.md"));
}

#[test]
fn osd_takes_a_dash_led_label_past_the_dashes_and_refuses_a_bad_argument_as_usage() {
    let dir = no_desktop();
    // clap exits 2 on a usage error; a call it parsed and the desktop cannot show exits 1.
    for (args, code) in [
        (&["osd", "--", "video-display-symbolic", "-1 frame"][..], 1),
        (&["osd", "audio-volume-high-symbolic", "Volume", "--level", "0.5"], 1),
        (&["osd", "audio-volume-high-symbolic", "Volume", "--level", "loud"], 2),
        (&["osd", "video-display-symbolic"], 2),
    ] {
        let out = universe(&dir, args, &[NO_BUS]);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(code), "{args:?}: {err}");
        assert!(code == 2 || err.contains("desktop.profile"), "{args:?}: {err}");
    }
}

#[test]
fn an_extension_install_asks_first_then_lists_and_goes() {
    let dir = no_desktop();
    let ext = dir.path().join("work/hello");
    std::fs::create_dir_all(&ext).unwrap();
    std::fs::write(ext.join("module.toml"), "api = 2\nid = \"hello\"\nname = \"Hello\"\nversion = \"1.0.0\"\n").unwrap();
    let index = dir.path().join("index.json");
    std::fs::write(&index, r#"{"schema": 1, "extensions": []}"#).unwrap();
    let env = [("UNIVERSE_EXTENSIONS_INDEX", index.to_str().unwrap()), NO_BUS];
    let asked = universe(&dir, &["extension", "install", ext.to_str().unwrap()], &env);
    assert!(!asked.status.success() && text(&asked.stderr).contains("runs programs as you"), "no terminal, no --yes: {}", text(&asked.stderr));
    let out = universe(&dir, &["extension", "install", ext.to_str().unwrap(), "--yes", "--json"], &env);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let listed: serde_json::Value = serde_json::from_slice(&universe(&dir, &["extension", "ls", "--json"], &env).stdout).unwrap();
    let rows = listed["extensions"].as_array().unwrap();
    assert_eq!((rows.len(), &rows[0]["id"], &rows[0]["origin"]), (1, &serde_json::json!("hello"), &serde_json::json!("unlisted")));
    let modules = |dir| -> Vec<serde_json::Value> { serde_json::from_slice(&universe(dir, &["module", "ls", "--json"], &env).stdout).unwrap() };
    assert!(modules(&dir).iter().any(|m| m["id"] == "hello"), "an installed module is a module");
    assert!(universe(&dir, &["extension", "remove", "hello"], &env).status.success());
    assert!(!modules(&dir).iter().any(|m| m["id"] == "hello"));
}

#[path = "../src/testbus/mod.rs"]
#[allow(dead_code)]
mod testbus;

const NO_SYSTEM_BUS: (&str, &str) = ("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus");

#[test]
fn network_and_bluetooth_lists_read_as_unavailable_without_a_system_bus() {
    let dir = tempfile::tempdir().unwrap();
    for what in ["network", "bluetooth"] {
        let out = universe(&dir, &[what, "--json"], &[NO_SYSTEM_BUS]);
        assert!(out.status.success(), "{what}: {}", text(&out.stderr));
        let state: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(state["available"], false, "{what}: {state}");
        let out = universe(&dir, &[what, "ls"], &[NO_SYSTEM_BUS]);
        assert_eq!(out.status.code(), Some(1), "{what}");
        assert!(text(&out.stderr).contains("unavailable:"), "{what}: {}", text(&out.stderr));
    }
}

#[tokio::test]
async fn network_lists_what_networkmanager_on_the_system_bus_sees() {
    let bus = testbus::Bus::start();
    let nm = testbus::nm::Nm::serve(&bus, testbus::nm::Router::default()).await;
    nm.access_point("Home", 70, "psk").await;
    let dir = tempfile::tempdir().unwrap();
    let address = bus.address.clone();
    let out = tokio::task::spawn_blocking(move || universe(&dir, &["network", "--json"], &[("DBUS_SYSTEM_BUS_ADDRESS", &address)])).await.unwrap();
    let state: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&out.stderr)));
    let first = &state["networks"][0];
    assert_eq!((&state["device"], &first["ssid"], &first["security"]), (&serde_json::json!("wlan0"), &serde_json::json!("Home"), &serde_json::json!("psk")));
}

fn echoes(fd: i32) -> bool {
    // SAFETY: an all-zero termios is a valid value for tcgetattr to fill.
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    // SAFETY: `fd` is an open terminal.
    assert_eq!(unsafe { libc::tcgetattr(fd, &mut t) }, 0);
    t.c_lflag & libc::ECHO != 0
}

// Ctrl-C at the password prompt ends the CLI with the terminal's echo back on.
#[tokio::test]
async fn a_password_prompt_cut_short_puts_the_terminals_echo_back() {
    use std::os::fd::FromRawFd;
    let bus = testbus::Bus::start();
    let nm = testbus::nm::Nm::serve(&bus, testbus::nm::Router::default()).await;
    nm.access_point("Home", 70, "psk").await;
    let (mut primary, mut terminal) = (0, 0);
    // SAFETY: openpty fills both descriptors; no name, settings or size is asked for.
    assert_eq!(unsafe { libc::openpty(&mut primary, &mut terminal, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) }, 0);
    // SAFETY: the duplicate is the child's stdin alone.
    let stdin = unsafe { std::fs::File::from_raw_fd(libc::dup(terminal)) };
    let dir = tempfile::tempdir().unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_universe"))
        .args(["network", "connect", "Home"])
        .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
        .env("HOME", dir.path())
        .stdin(stdin)
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let asking = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while echoes(terminal) {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    asking.await.expect("the prompt turns the echo off");
    // SAFETY: the child is ours and still running.
    unsafe { libc::kill(child.id().unwrap() as i32, libc::SIGINT) };
    let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await.unwrap().unwrap();
    assert!(!status.success());
    assert!(echoes(terminal), "the echo is back");
    // SAFETY: both descriptors are this test's.
    unsafe {
        libc::close(primary);
        libc::close(terminal);
    }
}

// The launcher's side of the stream: commands on stdin, a pairing's question out and its answer back in, on the watch's own agent.
#[tokio::test]
async fn bluetooth_watch_pairs_through_its_own_agent_over_stdin() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let bus = testbus::Bus::start();
    let bluez = testbus::bluez::Bluez::serve(&bus).await;
    bluez.add("AA:BB:CC:00:00:02", Some("K380"), "input-keyboard", false, testbus::bluez::Pairing::Confirm(4242)).await;
    let dir = tempfile::tempdir().unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_universe"))
        .args(["bluetooth", "watch", "--json"])
        .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
        .env("HOME", dir.path())
        .env_remove("UNIVERSE_SESSION")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = tokio::io::BufReader::new(child.stdout.take().unwrap()).lines();
    let mut next = async |event: &str| loop {
        let line =
            tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line()).await.expect("no line within 5 s").unwrap().expect("the watch ended");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["event"] == event {
            return v;
        }
    };
    assert_eq!(next("ready").await["default"], false, "outside the Universe session the desktop keeps its default agent");
    stdin.write_all(b"{\"cmd\": \"pair\", \"address\": \"AA:BB:CC:00:00:02\"}\n").await.unwrap();
    let ask = next("request").await;
    assert_eq!((&ask["kind"], &ask["code"]), (&serde_json::json!("confirm"), &serde_json::json!("004242")));
    stdin.write_all(format!("{{\"cmd\": \"answer\", \"id\": {}, \"yes\": true}}\n", ask["id"]).as_bytes()).await.unwrap();
    assert_eq!(next("done").await["action"], "pair");
    drop(stdin);
    let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await.expect("a closed stdin ends the watch").unwrap();
    assert!(status.success());
}

// A pad's screenshot macro: the grab reported as the hook prints it, the path once the file is written.
#[tokio::test]
async fn the_controller_watch_reports_a_shots_grab_before_its_path() {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    let dir = tempfile::tempdir().unwrap();
    let config = "schema = 1\n[desktop]\nprofile = \"none\"\n[controller]\nenabled = false\nmacros = []\n[modules]\nenabled = [\"screenshot\"]\n[sources]\nenabled = []\n";
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    std::fs::write(dir.path().join("config/config.toml"), config).unwrap();
    let module = dir.path().join("modules/screenshot");
    std::fs::create_dir_all(module.join("bin")).unwrap();
    std::fs::write(module.join("module.toml"), "api = 2\nid = \"screenshot\"\nname = \"Screenshots\"\n[hooks]\nscreenshot = \"bin/shot\"\n").unwrap();
    std::fs::write(module.join("bin/shot"), "#!/bin/sh\necho grabbed\nsleep 0.3\necho \"$SCREENSHOTS_DIR/x.png\"\n").unwrap();
    std::fs::set_permissions(module.join("bin/shot"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::create_dir_all(dir.path().join("run")).unwrap();
    let runtime = dir.path().join("run");
    let mut cmd = tokio::process::Command::from(command(
        &dir,
        &["controller", "watch", "--json"],
        &[NO_BUS, ("DBUS_SYSTEM_BUS_ADDRESS", "unix:path=/dev/null/bus"), ("XDG_RUNTIME_DIR", runtime.to_str().unwrap())],
    ));
    for var in ["UNIVERSE_SESSION", "SteamGameId", "GAMESCOPE_WAYLAND_DISPLAY", "RUST_LOG"] {
        cmd.env_remove(var);
    }
    let pipe = std::process::Stdio::piped;
    let mut child = cmd.stdin(pipe()).stdout(pipe()).stderr(pipe()).kill_on_drop(true).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = tokio::io::BufReader::new(child.stdout.take().unwrap()).lines();
    let mut log = tokio::io::BufReader::new(child.stderr.take().unwrap()).lines();
    let mut next = async |event: &str| loop {
        let line =
            tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line()).await.expect("no line within 5 s").unwrap().expect("the watch ended");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["event"] == event {
            return (v, std::time::Instant::now());
        }
    };
    next("ready").await;
    stdin.write_all(b"{\"cmd\": \"run\", \"action\": \"screenshot\"}\n").await.unwrap();
    let (grabbed, at) = next("screenshot").await;
    assert_eq!(grabbed, serde_json::json!({"event": "screenshot", "phase": "grabbed"}));
    let (saved, saved_at) = next("screenshot").await;
    assert!(saved["path"].as_str().is_some_and(|p| p.ends_with("/x.png")) && saved.get("phase").is_none(), "{saved}");
    assert!(saved_at - at >= std::time::Duration::from_millis(250), "the grab is told as the hook prints it");
    let timing = async {
        while let Some(line) = log.next_line().await.unwrap() {
            if line.contains("screenshot by screenshot: grabbed after") {
                return line;
            }
        }
        panic!("the watch ended");
    };
    let timing = tokio::time::timeout(std::time::Duration::from_secs(5), timing).await.expect("each shot's timings on stderr, whatever RUST_LOG");
    assert!(timing.contains("ms, saved after"), "{timing}");
}
