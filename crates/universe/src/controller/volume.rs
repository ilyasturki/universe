//! Volume macros as `wpctl` calls on the default sink: nothing is typed, so no key reaches the game.

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up,
    Down,
    ToggleMute,
    Set(u8),
    Get,
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

pub async fn change(change: Change, percent: u8) -> Result<Level, String> {
    let level = tokio::task::spawn_blocking(move || apply(change, percent)).await.map_err(|e| e.to_string())??;
    if change != Change::Get {
        osd(&level).await;
    }
    Ok(level)
}

async fn osd(level: &Level) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let icon = match level.percent {
        p if level.muted || p == 0 => "audio-volume-muted-symbolic",
        1..=33 => "audio-volume-low-symbolic",
        34..=66 => "audio-volume-medium-symbolic",
        _ => "audio-volume-high-symbolic",
    };
    if let Err(e) = crate::desktop::show_osd(icon, Some(&level.output), Some(f64::from(level.percent) / 100.0)).await {
        if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("no OSD: {e} (installed extensions load after a logout)");
        }
    }
}

pub fn apply(change: Change, percent: u8) -> Result<Level, String> {
    const SINK: &str = "@DEFAULT_AUDIO_SINK@";
    match change {
        Change::Up => wpctl(&["set-volume", "-l", "1.0", SINK, &format!("{percent}%+")])?,
        Change::Down => wpctl(&["set-volume", SINK, &format!("{percent}%-")])?,
        Change::ToggleMute => wpctl(&["set-mute", SINK, "toggle"])?,
        Change::Set(p) => wpctl(&["set-volume", "-l", "1.0", SINK, &format!("{}%", p.min(100))])?,
        Change::Get => String::new(),
    };
    let volume = wpctl(&["get-volume", SINK])?;
    let level: f64 = volume.split_whitespace().nth(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("wpctl get-volume: {volume:?}"))?;
    let inspect = wpctl(&["inspect", SINK])?;
    let prop = |key: &str| inspect.lines().find_map(|l| l.split_once(&format!("{key} = "))).map(|(_, v)| v.trim().trim_matches('"').to_string());
    let output = prop("device.id")
        .zip(prop("card.profile.device").and_then(|d| d.parse().ok()))
        .and_then(|(device, profile_device)| {
            Command::new("pw-dump")
                .arg(device)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| route_description(&String::from_utf8_lossy(&o.stdout), profile_device))
        })
        .or_else(|| prop("node.description"))
        .unwrap_or_default();
    Ok(Level { percent: (level * 100.0).round() as u8, muted: volume.contains("[MUTED]"), output })
}

/// The active output route of the sink's device — what GNOME's own OSD labels the volume with; `device` in a Route is the sink's `card.profile.device`.
fn route_description(pw_dump: &str, profile_device: u64) -> Option<String> {
    let dump: serde_json::Value = serde_json::from_str(pw_dump).ok()?;
    dump.as_array()?
        .iter()
        .filter_map(|obj| obj.pointer("/info/params/Route")?.as_array())
        .flatten()
        .find(|r| r["direction"] == "Output" && r["device"] == profile_device)
        .and_then(|r| r["description"].as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::route_description;

    const DUMP: &str = r#"[{"id":173,"type":"PipeWire:Interface:Device","info":{"params":{"Route":[
        {"index":0,"direction":"Output","name":"analog-output","description":"Speakers","available":"yes","device":0},
        {"index":1,"direction":"Input","name":"analog-input","description":"Microphone","available":"yes","device":4},
        {"index":2,"direction":"Output","name":"hdmi-output-0","description":"HDMI / DisplayPort","available":"yes","device":4}
    ]}}}]"#;

    #[test]
    fn picks_the_output_route_of_the_profile_device() {
        assert_eq!(route_description(DUMP, 4).as_deref(), Some("HDMI / DisplayPort"));
        assert_eq!(route_description(DUMP, 0).as_deref(), Some("Speakers"));
        assert_eq!(route_description(DUMP, 9), None);
        assert_eq!(route_description("[]", 4), None);
        assert_eq!(route_description("not json", 4), None);
    }
}
