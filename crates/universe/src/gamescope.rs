//! Left alone, gamescope's nested screen is 1280×720 whatever the window covers, so the screen's mode is passed explicitly.

use serde::{Deserialize, Serialize};

use crate::Error;

pub const SCALERS: [&str; 5] = ["auto", "integer", "fit", "fill", "stretch"];
pub const FILTERS: [&str; 5] = ["linear", "nearest", "fsr", "nis", "pixel"];
pub const SHARPNESS_MAX: u32 = 20;
const REFRESH_RATES: [u32; 12] = [240, 165, 144, 120, 100, 90, 75, 60, 50, 48, 40, 30];
const RESOLUTION_HEIGHTS: [u32; 5] = [2160, 1800, 1440, 1080, 720];

/// A screen's current mode; refresh in Hz, rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
}

/// Serialized under the `launch` key names so a frontend reads them as it reads the other effective switches.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Fields {
    /// `auto` or `WxH`.
    #[serde(rename = "gamescope_resolution")]
    pub resolution: String,
    /// `auto` or Hz.
    #[serde(rename = "gamescope_refresh")]
    pub refresh: String,
    #[serde(rename = "gamescope_scaler")]
    pub scaler: String,
    #[serde(rename = "gamescope_filter")]
    pub filter: String,
    #[serde(rename = "gamescope_sharpness")]
    pub sharpness: Option<u32>,
    #[serde(rename = "gamescope_adaptive_sync")]
    pub adaptive_sync: bool,
}

pub fn parse_resolution(s: &str) -> crate::Result<Option<(u32, u32)>> {
    let s = s.trim();
    if s.is_empty() || s == "auto" {
        return Ok(None);
    }
    let bad = || Error::Invalid(format!("gamescope_resolution must be auto or WIDTHxHEIGHT, not '{s}'"));
    let (w, h) = s.split_once(['x', 'X', '×']).ok_or_else(bad)?;
    match (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
        (Ok(w), Ok(h)) if w > 0 && h > 0 => Ok(Some((w, h))),
        _ => Err(bad()),
    }
}

pub fn parse_refresh(s: &str) -> crate::Result<Option<u32>> {
    let s = s.trim();
    if s.is_empty() || s == "auto" {
        return Ok(None);
    }
    match s.parse::<u32>() {
        Ok(hz) if hz > 0 => Ok(Some(hz)),
        _ => Err(Error::Invalid(format!("gamescope_refresh must be auto or a rate in Hz, not '{s}'"))),
    }
}

/// `auto`, the screen's `WxH`, then the standard heights below it at its aspect ratio; two common sizes when no screen is known.
pub fn resolution_choices(screen: Option<Mode>) -> Vec<String> {
    let Some(s) = screen.filter(|s| s.width > 0 && s.height > 0) else { return ["auto", "1920x1080", "1280x720"].map(String::from).to_vec() };
    let mut out = vec!["auto".to_string()];
    for h in std::iter::once(s.height).chain(RESOLUTION_HEIGHTS).filter(|h| *h <= s.height) {
        let w = (s.width as f64 * h as f64 / s.height as f64 / 2.0).round() as u32 * 2;
        let wh = format!("{w}x{h}");
        if !out.contains(&wh) {
            out.push(wh);
        }
    }
    out
}

/// `auto`, the screen's rate, then the common rates below it: a game sees no more than the screen shows.
pub fn refresh_choices(screen: Option<Mode>) -> Vec<String> {
    let mut rates: Vec<u32> = match screen.map(|s| s.refresh).filter(|hz| *hz > 0) {
        Some(hz) => std::iter::once(hz).chain(REFRESH_RATES.into_iter().filter(|r| *r < hz)).collect(),
        None => REFRESH_RATES.to_vec(),
    };
    rates.sort_unstable_by(|a, b| b.cmp(a));
    std::iter::once("auto".to_string()).chain(rates.iter().map(u32::to_string)).collect()
}

/// `auto` (the refresh the game sees), `none`, then the rates the screen can show.
pub fn fps_limit_choices(screen: Option<Mode>) -> Vec<String> {
    ["auto".to_string(), "none".to_string()].into_iter().chain(refresh_choices(screen).into_iter().skip(1)).collect()
}

/// No screen known: no size flags, so gamescope keeps its own default rather than a wrong one.
pub fn args(f: &Fields, screen: Option<Mode>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(s) = screen.filter(|s| s.width > 0 && s.height > 0) {
        out.extend(["-W".into(), s.width.to_string(), "-H".into(), s.height.to_string()]);
    }
    let game = parse_resolution(&f.resolution).unwrap_or(None).or_else(|| screen.filter(|s| s.width > 0 && s.height > 0).map(|s| (s.width, s.height)));
    if let Some((w, h)) = game {
        out.extend(["-w".into(), w.to_string(), "-h".into(), h.to_string()]);
    }
    let hz = parse_refresh(&f.refresh).unwrap_or(None).or_else(|| screen.map(|s| s.refresh).filter(|hz| *hz > 0));
    if let Some(hz) = hz {
        out.extend(["-r".into(), hz.to_string()]);
    }
    if !f.scaler.is_empty() {
        out.extend(["-S".into(), f.scaler.clone()]);
    }
    if !f.filter.is_empty() {
        out.extend(["-F".into(), f.filter.clone()]);
    }
    if let Some(n) = f.sharpness {
        out.extend(["--sharpness".into(), n.to_string()]);
    }
    if f.adaptive_sync {
        out.push("--adaptive-sync".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_and_refresh_parse() {
        assert_eq!(parse_resolution("auto").unwrap(), None);
        assert_eq!(parse_resolution("").unwrap(), None);
        assert_eq!(parse_resolution("2560x1440").unwrap(), Some((2560, 1440)));
        assert_eq!(parse_resolution("1920×1080").unwrap(), Some((1920, 1080)));
        assert!(parse_resolution("1080p").is_err());
        assert!(parse_resolution("0x1080").is_err());
        assert_eq!(parse_refresh("auto").unwrap(), None);
        assert_eq!(parse_refresh("120").unwrap(), Some(120));
        assert!(parse_refresh("fast").is_err());
        assert!(parse_refresh("0").is_err());
    }

    #[test]
    fn choices_follow_the_screen() {
        let screen = Some(Mode { width: 3840, height: 2160, refresh: 60 });
        assert_eq!(resolution_choices(screen), ["auto", "3840x2160", "3200x1800", "2560x1440", "1920x1080", "1280x720"]);
        assert_eq!(resolution_choices(Some(Mode { width: 3440, height: 1440, refresh: 100 })), ["auto", "3440x1440", "2580x1080", "1720x720"]);
        assert_eq!(resolution_choices(None), ["auto", "1920x1080", "1280x720"]);
        assert_eq!(refresh_choices(screen), ["auto", "60", "50", "48", "40", "30"]);
        assert_eq!(refresh_choices(Some(Mode { width: 1, height: 1, refresh: 72 })), ["auto", "72", "60", "50", "48", "40", "30"]);
        assert_eq!(refresh_choices(None).len(), 13);
        assert_eq!(fps_limit_choices(screen), ["auto", "none", "60", "50", "48", "40", "30"]);
    }

    #[test]
    fn auto_takes_the_screen_and_a_field_wins_over_it() {
        let screen = Some(Mode { width: 3840, height: 2160, refresh: 60 });
        let auto = Fields { resolution: "auto".into(), refresh: "auto".into(), ..Default::default() };
        assert_eq!(args(&auto, screen), ["-W", "3840", "-H", "2160", "-w", "3840", "-h", "2160", "-r", "60"]);
        let set = Fields { resolution: "1920x1080".into(), refresh: "120".into(), scaler: "fit".into(), filter: "fsr".into(), sharpness: Some(0), adaptive_sync: true };
        assert_eq!(args(&set, screen), ["-W", "3840", "-H", "2160", "-w", "1920", "-h", "1080", "-r", "120", "-S", "fit", "-F", "fsr", "--sharpness", "0", "--adaptive-sync"]);
    }

    #[test]
    fn no_screen_means_no_size_flags() {
        let auto = Fields::default();
        assert!(args(&auto, None).is_empty());
        let set = Fields { resolution: "1280x720".into(), ..Default::default() };
        assert_eq!(args(&set, None), ["-w", "1280", "-h", "720"]);
    }
}
