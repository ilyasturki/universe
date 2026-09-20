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
    cmd.envs(env.iter().copied()).args(args).output().unwrap()
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
    assert_eq!(ids, ["lutris", "steam", "heroic-gog", "heroic-epic", "heroic-amazon"]);
    assert!(launchers.iter().all(|l| l["found"] == false && l["games"] == 0), "{launchers:?}");
    assert_eq!(report["gog_dirs"], serde_json::json!([]));
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
    assert!(!out.status.success() && err.contains("read-only") && err.contains("programs.universe.settings"), "{err:?}");
    let settings: serde_json::Value = serde_json::from_slice(&universe(&dir, &["config", "get", "--json"], &[]).stdout).unwrap();
    assert_eq!(settings["config_writable"], false);
    let text = String::from_utf8_lossy(&universe(&dir, &["doctor"], &[]).stdout).into_owned();
    assert!(text.contains("read-only"), "{text:?}");
}
