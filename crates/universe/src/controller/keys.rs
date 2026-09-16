use std::str::FromStr;

use evdev::KeyCode;

use super::ControllerConfig;

/// Codes the evdev crate has no name for yet.
const EXTRA_NAMES: [(&str, u16); 4] = [("BTN_GRIPL", 0x224), ("BTN_GRIPR", 0x225), ("BTN_GRIPL2", 0x226), ("BTN_GRIPR2", 0x227)];

pub fn key_name(code: u16) -> String {
    if let Some((n, _)) = EXTRA_NAMES.iter().find(|(_, c)| *c == code) {
        return (*n).to_string();
    }
    let dbg = format!("{:?}", KeyCode::new(code));
    if dbg.starts_with("KEY_") || dbg.starts_with("BTN_") {
        dbg
    } else {
        format!("{code}")
    }
}

pub fn parse_key(name: &str) -> Option<u16> {
    let name = name.trim();
    if let Some((_, c)) = EXTRA_NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
        return Some(*c);
    }
    if let Ok(k) = KeyCode::from_str(&name.to_uppercase()) {
        return Some(k.code());
    }
    name.parse::<u16>().ok()
}

/// What a slot listens to: a key, or an axis crossing half its range in one direction (hats, triggers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Key(u16),
    Axis { code: u16, positive: bool },
}

impl Source {
    pub fn present(&self, keys: &[u16], axes: &[u16]) -> bool {
        match self {
            Source::Key(c) => keys.contains(c),
            Source::Axis { code, .. } => axes.contains(code),
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Key(c) => f.write_str(&key_name(*c)),
            Source::Axis { code, positive } => write!(f, "{:?}{}", evdev::AbsoluteAxisCode(*code), if *positive { "+" } else { "-" }),
        }
    }
}

/// `BTN_TRIGGER_HAPPY3`, `548`, `ABS_HAT0X-`, `ABS_Z+` (an axis without a sign is positive).
pub fn parse_source(text: &str) -> Option<Source> {
    let t = text.trim();
    if let Some(body) = t.strip_prefix("ABS_") {
        let (name, positive) = match body.chars().last() {
            Some('-') => (&body[..body.len() - 1], false),
            Some('+') => (&body[..body.len() - 1], true),
            _ => (body, true),
        };
        let code = evdev::AbsoluteAxisCode::from_str(&format!("ABS_{name}")).ok()?.0;
        return Some(Source::Axis { code, positive });
    }
    parse_key(t).map(Source::Key)
}

/// X keysym names as MangoHud.conf spells them, plus the plain names people type.
const ALIASES: [(&str, &str); 26] = [
    ("shift", "KEY_LEFTSHIFT"), ("shift_l", "KEY_LEFTSHIFT"), ("shift_r", "KEY_RIGHTSHIFT"),
    ("ctrl", "KEY_LEFTCTRL"), ("control", "KEY_LEFTCTRL"), ("control_l", "KEY_LEFTCTRL"), ("control_r", "KEY_RIGHTCTRL"),
    ("alt", "KEY_LEFTALT"), ("alt_l", "KEY_LEFTALT"), ("alt_r", "KEY_RIGHTALT"),
    ("super", "KEY_LEFTMETA"), ("super_l", "KEY_LEFTMETA"), ("super_r", "KEY_RIGHTMETA"), ("meta", "KEY_LEFTMETA"), ("win", "KEY_LEFTMETA"),
    ("return", "KEY_ENTER"), ("enter", "KEY_ENTER"), ("escape", "KEY_ESC"), ("esc", "KEY_ESC"), ("space", "KEY_SPACE"), ("tab", "KEY_TAB"),
    ("print", "KEY_SYSRQ"), ("printscreen", "KEY_SYSRQ"), ("delete", "KEY_DELETE"), ("backspace", "KEY_BACKSPACE"), ("pause", "KEY_PAUSE"),
];

fn key_of_name(name: &str) -> Option<u16> {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return None;
    }
    if let Some((_, k)) = ALIASES.iter().find(|(a, _)| *a == lower) {
        return parse_key(k);
    }
    parse_key(name).or_else(|| parse_key(&format!("KEY_{}", lower.to_uppercase()))).or_else(|| lower.strip_prefix("xf86audio").and_then(|rest| parse_key(&format!("KEY_{}", audio_key(rest)?))))
}

fn audio_key(rest: &str) -> Option<&'static str> {
    Some(match rest {
        "raisevolume" => "VOLUMEUP",
        "lowervolume" => "VOLUMEDOWN",
        "mute" => "MUTE",
        "play" => "PLAYPAUSE",
        "next" => "NEXTSONG",
        "prev" => "PREVIOUSSONG",
        _ => return None,
    })
}

pub fn parse_combo(text: &str) -> Result<Vec<u16>, String> {
    let mut codes = Vec::new();
    for part in text.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        codes.push(key_of_name(part).ok_or_else(|| format!("unknown key '{part}' in '{text}'"))?);
    }
    if codes.is_empty() {
        return Err("empty key combo".into());
    }
    Ok(codes)
}

pub fn mangohud_toggle(config: &ControllerConfig) -> String {
    if !config.mangohud_toggle.trim().is_empty() {
        return config.mangohud_toggle.trim().to_string();
    }
    mangohud_combo("toggle_hud", "Shift_R+F12")
}

/// `default` is MangoHud's own binding for `key`.
pub fn mangohud_combo(key: &str, default: &str) -> String {
    let conf = crate::paths::xdg("XDG_CONFIG_HOME", ".config").join("MangoHud/MangoHud.conf");
    std::fs::read_to_string(conf).ok().and_then(|s| combo_of(&s, key)).unwrap_or_else(|| default.into())
}

pub fn combo_of(conf: &str, key: &str) -> Option<String> {
    conf.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.split('#').next().unwrap_or("").trim().to_string()))
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_including_the_grips() {
        assert_eq!(key_name(0x130), "BTN_SOUTH");
        assert_eq!(key_name(548), "BTN_GRIPL");
        assert_eq!(parse_key("BTN_GRIPR2"), Some(551));
        assert_eq!(parse_key("btn_trigger_happy3"), Some(706));
        assert_eq!(parse_key("706"), Some(706));
        assert_eq!(parse_key("BTN_NOPE"), None);
        assert_eq!(parse_source("ABS_HAT0X-"), Some(Source::Axis { code: 16, positive: false }));
        assert_eq!(parse_source("ABS_Z").unwrap().to_string(), "ABS_Z+");
        assert_eq!(parse_source("BTN_GRIPL").unwrap().to_string(), "BTN_GRIPL");
    }

    #[test]
    fn combos_in_mangohud_and_plain_spellings() {
        assert_eq!(parse_combo("Super_L+F12").unwrap(), vec![125, 88]);
        assert_eq!(parse_combo("Shift_R+F12").unwrap(), vec![54, 88]);
        assert_eq!(parse_combo("ctrl+shift+f2").unwrap(), vec![29, 42, 60]);
        assert_eq!(parse_combo("KEY_LEFTSHIFT+KEY_VOLUMEUP").unwrap(), vec![42, 115]);
        assert_eq!(parse_combo("XF86AudioRaiseVolume").unwrap(), vec![115]);
        assert_eq!(parse_combo("a").unwrap(), vec![30]);
        assert!(parse_combo("Ctrl+Nope").is_err());
        assert!(parse_combo("").is_err());
    }

    #[test]
    fn mangohud_conf_toggle() {
        assert_eq!(combo_of("legacy_layout=false\n# toggle_hud=F1\ntoggle_hud=Super_L+F12 # the one\n", "toggle_hud"), Some("Super_L+F12".into()));
        assert_eq!(combo_of("toggle_hud_position=Super_L+F11\n", "toggle_hud"), None);
        assert_eq!(combo_of("reload_cfg=Shift_L+F4\n", "reload_cfg"), Some("Shift_L+F4".into()));
    }
}
