//! The built binary on an empty, isolated profile.
use std::process::Command;

fn universe(dir: &tempfile::TempDir, args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
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
    cmd.envs(env.iter().copied()).args(args).output().unwrap()
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
fn a_read_only_config_is_named_when_a_write_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "schema = 1\n").unwrap();
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
fn journal_add_files_an_entry_the_journal_lists_and_calls_only_a_refusal_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("quest.sh");
    std::fs::write(&exe, "").unwrap();
    let added = universe(&dir, &["add", exe.to_str().unwrap(), "--runner", "linux", "--title", "Quest", "--json"], &[]);
    let game: serde_json::Value = serde_json::from_slice(&added.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&added.stderr)));
    let id = game["id"].as_str().unwrap();
    let sid = "20261004-120000";
    let entry = serde_json::json!({
        "session": sid, "game": id, "written_at": "2026-10-04T13:00:00+02:00", "lang": "en", "title": "First steps",
        "provider": "test", "paragraphs": ["Left the cave."], "next_up": "", "images": [],
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

    // modules/journal gives up on "invalid:"; on anything else it retries, then writes the file itself.
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
}

fn files_under(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir).into_iter().flatten().flatten().flat_map(|e| if e.path().is_dir() { files_under(&e.path()) } else { vec![e.path()] }).collect()
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
