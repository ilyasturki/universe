//! Volume macros as `wpctl` calls on the default sink: nothing is typed, so no key reaches the game.

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up,
    Down,
    ToggleMute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub percent: u8,
    pub muted: bool,
    pub output: String,
}

fn wpctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("wpctl").args(args).output().map_err(|e| format!("wpctl: {e}"))?;
    if !out.status.success() {
        return Err(format!("wpctl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Blocking: applies the change to the default sink, capped at the normal volume on the way up.
pub fn apply(change: Change, percent: u8) -> Result<Level, String> {
    const SINK: &str = "@DEFAULT_AUDIO_SINK@";
    match change {
        Change::Up => wpctl(&["set-volume", "-l", "1.0", SINK, &format!("{percent}%+")])?,
        Change::Down => wpctl(&["set-volume", SINK, &format!("{percent}%-")])?,
        Change::ToggleMute => wpctl(&["set-mute", SINK, "toggle"])?,
    };
    let volume = wpctl(&["get-volume", SINK])?;
    let level: f64 = volume.split_whitespace().nth(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("wpctl get-volume: {volume:?}"))?;
    let output = wpctl(&["inspect", SINK])?.lines().find_map(|l| l.split_once("node.description = ")).map(|(_, v)| v.trim().trim_matches('"').to_string()).unwrap_or_default();
    Ok(Level { percent: (level * 100.0).round() as u8, muted: volume.contains("[MUTED]"), output })
}
