//! Volume macros as `wpctl` calls on the default sink: nothing is typed, so no key reaches the game.

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::sound::{wpctl, Graph};

const SINK: &str = "@DEFAULT_AUDIO_SINK@";
// Above the engine's 400 ms repeat delay: a held key's repeats reuse its first press's label.
const BURST: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up,
    Down,
    ToggleMute,
    Set(u8),
    Get,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Batch {
    pub change: Change,
    pub percent: u8,
    pub times: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub percent: u8,
    pub muted: bool,
    pub output: String,
}

pub async fn change(change: Change, percent: u8, desktop: crate::desktop::Profile) -> Result<Level, String> {
    let batch = Batch { change, percent, times: 1 };
    let level = tokio::task::spawn_blocking(move || apply(batch)).await.map_err(|e| e.to_string())??;
    if shows_osd(change) {
        osd(&level, desktop).await;
    }
    Ok(level)
}

/// Inside the launcher's gamescope its overlay draws the level whatever the desktop; the shell's would be a second one.
pub fn shows_osd(change: Change) -> bool {
    change != Change::Get && !crate::nest::inside()
}

pub async fn osd(level: &Level, desktop: crate::desktop::Profile) {
    static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let icon = match level.percent {
        p if level.muted || p == 0 => "audio-volume-muted-symbolic",
        1..=33 => "audio-volume-low-symbolic",
        34..=66 => "audio-volume-medium-symbolic",
        _ => "audio-volume-high-symbolic",
    };
    if let Err(e) = crate::desktop::show_osd(desktop, icon, &level.output, Some(f64::from(level.percent) / 100.0)).await {
        if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("no OSD: {e}");
        }
    }
}

pub fn coalesce(changes: impl IntoIterator<Item = (Change, u8)>) -> Vec<Batch> {
    let mut batches: Vec<Batch> = Vec::new();
    for (change, percent) in changes {
        match batches.last_mut() {
            Some(b) if b.change == change && b.percent == percent => b.times += 1,
            _ => batches.push(Batch { change, percent, times: 1 }),
        }
    }
    batches
}

fn commands(b: Batch) -> Vec<Vec<String>> {
    let step = u32::from(b.percent) * b.times;
    let call = |args: &[&str]| args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    match b.change {
        // wpctl's set-volume leaves the mute alone: up unmutes, as GNOME's and KDE's keys do.
        Change::Up => vec![call(&["set-volume", "-l", "1.0", SINK, &format!("{step}%+")]), call(&["set-mute", SINK, "0"])],
        Change::Down => vec![call(&["set-volume", SINK, &format!("{step}%-")])],
        Change::ToggleMute if b.times % 2 == 1 => vec![call(&["set-mute", SINK, "toggle"])],
        Change::Set(p) => vec![call(&["set-volume", "-l", "1.0", SINK, &format!("{}%", p.min(100))])],
        Change::ToggleMute | Change::Get => vec![],
    }
}

pub fn apply(b: Batch) -> Result<Level, String> {
    for args in commands(b) {
        wpctl(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    }
    let volume = wpctl(&["get-volume", SINK])?;
    let level: f64 = volume.split_whitespace().nth(1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("wpctl get-volume: {volume:?}"))?;
    Ok(Level { percent: (level * 100.0).round() as u8, muted: volume.contains("[MUTED]"), output: output_label(b.change == Change::Get) })
}

fn output_label(fresh: bool) -> String {
    static LAST: Mutex<Option<(String, Instant)>> = Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(PoisonError::into_inner);
    let now = Instant::now();
    match last.as_mut() {
        Some((label, used)) if !fresh && now.duration_since(*used) < BURST => {
            *used = now;
            label.clone()
        }
        _ => {
            let label = Graph::read().map(|g| g.current_label()).unwrap_or_default();
            *last = Some((label.clone(), now));
            label
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_fold_into_one_step_and_the_order_between_changes_stands() {
        let pressed = [(Change::Up, 2), (Change::Up, 2), (Change::Up, 2), (Change::ToggleMute, 2), (Change::Down, 2), (Change::Up, 2), (Change::Up, 5)];
        let batch = |change, percent, times| Batch { change, percent, times };
        assert_eq!(
            coalesce(pressed),
            [batch(Change::Up, 2, 3), batch(Change::ToggleMute, 2, 1), batch(Change::Down, 2, 1), batch(Change::Up, 2, 1), batch(Change::Up, 5, 1)],
            "a step changed between presses is a batch of its own"
        );
    }

    #[test]
    fn up_unmutes_and_down_leaves_the_mute_alone() {
        let run = |change, times| commands(Batch { change, percent: 2, times });
        assert_eq!(
            run(Change::Up, 3),
            [vec!["set-volume", "-l", "1.0", SINK, "6%+"], vec!["set-mute", SINK, "0"]],
            "the level moves, then the sound comes back"
        );
        assert_eq!(run(Change::Down, 1), [vec!["set-volume", SINK, "2%-"]]);
        assert_eq!(run(Change::ToggleMute, 1), [vec!["set-mute", SINK, "toggle"]]);
        assert!(run(Change::ToggleMute, 2).is_empty(), "two toggles in a row are none");
        assert!(run(Change::Get, 1).is_empty());
    }
}
