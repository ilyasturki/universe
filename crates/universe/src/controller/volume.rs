//! Volume macros as `wpctl` calls on the default sink: nothing is typed, so no key reaches the game.

use crate::sound::{wpctl, Graph};

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
    let output = Graph::read().map(|g| g.current_label()).unwrap_or_default();
    Ok(Level { percent: (level * 100.0).round() as u8, muted: volume.contains("[MUTED]"), output })
}
