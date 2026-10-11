//! Pads are read from evdev without a grab so the game keeps the pad whole.

pub mod engine;
pub mod keys;
pub mod volume;
pub mod watch;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::paths;

pub const TRIGGERS: [&str; 2] = ["press", "hold"];
/// The launcher's own button: `api.home` reads its press and hold, so no macro fires on it.
pub const HOME_SLOT: &str = "guide";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Macro {
    pub family: String,
    pub button: String,
    pub trigger: String,
    pub action: String,
    pub keys: String,
    pub command: String,
}

impl Default for Macro {
    fn default() -> Self {
        Macro { family: "*".into(), button: String::new(), trigger: "press".into(), action: String::new(), keys: String::new(), command: String::new() }
    }
}

impl Macro {
    fn seeded(family: &str, button: &str, action: &str) -> Macro {
        Macro { family: family.into(), button: button.into(), trigger: "press".into(), action: action.into(), ..Macro::default() }
    }

    pub fn validate(&self) -> crate::Result<()> {
        let bad = crate::Error::Invalid;
        if !TRIGGERS.contains(&self.trigger.as_str()) {
            return Err(bad(format!("trigger must be press or hold, not '{}'", self.trigger)));
        }
        if self.button == HOME_SLOT {
            return Err(bad("Guide is HOME: a press opens the dock, a hold goes home".into()));
        }
        let preset = PRESETS.iter().find(|p| p.id == self.action).ok_or_else(|| bad(format!("unknown action '{}'", self.action)))?;
        if preset.hold_only && self.trigger != "hold" {
            return Err(bad(format!("{} only fires on a hold", preset.label)));
        }
        if self.action == "keys" {
            keys::parse_combo(&self.keys).map_err(bad)?;
        }
        if self.action == "command" && self.command.trim().is_empty() {
            return Err(bad("command is empty".into()));
        }
        match (self.family.as_str(), family_by_id(&self.family)) {
            ("*", _) if !STANDARD.iter().any(|s| s.id == self.button) => Err(bad(format!("'{}' is not a standard button; bind it on its family", self.button))),
            ("*", _) => Ok(()),
            (_, Some(f)) if !f.slots().any(|s| s.id == self.button) => Err(bad(format!("{} has no button '{}'", f.name, self.button))),
            (_, Some(_)) => Ok(()),
            _ => Err(bad(format!("unknown family '{}'", self.family))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ControllerConfig {
    pub enabled: bool,
    pub hold_ms: u64,
    pub volume_step: u8,
    /// HOME pressed while another app has the focus raises the launcher; off, the press is dropped.
    pub home_summons: bool,
    /// family → slot → learned codes, first present on the pad wins
    pub buttons: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// family → stick or trigger (`lx ly rx ry lt rt`) → the axis learned for it, `ABS_Z-` when it runs backwards
    pub axes: BTreeMap<String, BTreeMap<String, String>>,
    /// family → where A sits in Nintendo games on that model (`LAYOUTS`); absent, where its print puts it
    pub layout: BTreeMap<String, String>,
    /// absent: the seeded workflow; `macros = []` is none at all
    pub macros: Option<Vec<Macro>>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        ControllerConfig {
            enabled: true,
            hold_ms: 600,
            volume_step: 2,
            home_summons: true,
            buttons: BTreeMap::new(),
            axes: BTreeMap::new(),
            layout: BTreeMap::new(),
            macros: None,
        }
    }
}

/// A model's Nintendo layout: A on the right (`positional`) or at the bottom (`xbox`).
pub const LAYOUTS: [&str; 2] = ["positional", "xbox"];

/// `controller.layout.<family>`: a known family, and one of `LAYOUTS` or empty to go back to the print.
pub fn layout_value(key: &str, value: &str) -> crate::Result<()> {
    let family = key.strip_prefix("controller.layout.").unwrap_or("");
    if family_by_id(family).is_none() {
        return Err(crate::Error::Invalid(format!("{key}: unknown family '{family}'")));
    }
    if !value.is_empty() && !LAYOUTS.contains(&value) {
        return Err(crate::Error::Invalid(format!("{key}: positional or xbox, not '{value}'")));
    }
    Ok(())
}

pub fn volume_step_value(value: &str) -> crate::Result<u8> {
    match value.parse::<u8>() {
        Ok(p) if (1..=100).contains(&p) => Ok(p),
        _ => Err(crate::Error::Invalid(format!("controller.volume_step: a percent from 1 to 100, not '{value}'"))),
    }
}

impl ControllerConfig {
    pub fn macros(&self) -> Vec<Macro> {
        match &self.macros {
            Some(m) => m.clone(),
            None => seeded_macros(),
        }
    }

    /// The macros bound to one slot of a family: the family's own, then the `*` ones it does not shadow.
    pub fn macros_for(&self, family: &str, slot: &str) -> Vec<Macro> {
        if slot == HOME_SLOT {
            return vec![];
        }
        let all = self.macros();
        let mut out: Vec<Macro> = all.iter().filter(|m| m.family == family && m.button == slot).cloned().collect();
        for m in all.iter().filter(|m| m.family == "*" && m.button == slot) {
            if !out.iter().any(|o| o.trigger == m.trigger) {
                out.push(m.clone());
            }
        }
        out
    }

    /// A learned list replaces the seeds, so a seed taken away stays away.
    pub fn codes_for(&self, family: &Family, slot: &str) -> Vec<String> {
        if let Some(learned) = self.buttons.get(family.id).and_then(|b| b.get(slot)) {
            return learned.clone();
        }
        family.slots().find(|s| s.id == slot).map(|s| s.codes.iter().map(|c| (*c).to_string()).collect()).unwrap_or_default()
    }
}

pub fn seeded_macros() -> Vec<Macro> {
    vec![
        Macro::seeded("dualsense-edge", "fn_left", "screenshot"),
        Macro::seeded("dualsense-edge", "fn_right", "mangohud"),
        Macro::seeded("dualsense-edge", "paddle_right", "volume_up"),
        Macro::seeded("dualsense-edge", "paddle_left", "volume_down"),
        Macro::seeded("xbox-elite", "paddle_p1", "mangohud"),
        Macro::seeded("xbox-elite", "paddle_p2", "volume_up"),
        Macro::seeded("xbox-elite", "paddle_p3", "screenshot"),
        Macro::seeded("xbox-elite", "paddle_p4", "volume_down"),
        Macro::seeded("8bitdo-pro-3", "paddle_r4", "mangohud"),
        Macro::seeded("8bitdo-pro-3", "paddle_pr", "volume_up"),
        Macro::seeded("8bitdo-pro-3", "paddle_pl", "volume_down"),
        Macro::seeded("xbox", "share", "screenshot"),
        Macro::seeded("switch-pro", "capture", "screenshot"),
        Macro::seeded("switch2-pro", "capture", "screenshot"),
    ]
}

pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub hold_only: bool,
    pub repeats: bool,
    /// Still fires while the launcher's dock has the pad: nothing that reads the screen, types, or ends the game.
    pub docked: bool,
}

pub const PRESETS: [Preset; 8] = [
    Preset { id: "volume_up", label: "Volume up", hold_only: false, repeats: true, docked: true },
    Preset { id: "volume_down", label: "Volume down", hold_only: false, repeats: true, docked: true },
    Preset { id: "mute", label: "Mute", hold_only: false, repeats: false, docked: true },
    Preset { id: "screenshot", label: "Screenshot", hold_only: false, repeats: false, docked: false },
    Preset { id: "mangohud", label: "Toggle MangoHud", hold_only: false, repeats: false, docked: true },
    Preset { id: "stop", label: "Stop the game", hold_only: true, repeats: false, docked: false },
    Preset { id: "keys", label: "Key combo…", hold_only: false, repeats: false, docked: false },
    Preset { id: "command", label: "Command…", hold_only: false, repeats: false, docked: false },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub id: &'static str,
    pub label: &'static str,
    pub codes: &'static [&'static str],
    pub extra: bool,
}

const fn slot(id: &'static str, label: &'static str, codes: &'static [&'static str]) -> Slot {
    Slot { id, label, codes, extra: false }
}
const fn extra(id: &'static str, label: &'static str, codes: &'static [&'static str]) -> Slot {
    Slot { id, label, codes, extra: true }
}

// An Android-style pad (a D-input 8BitDo) has BRAKE and GAS for triggers and Z/RZ for its right stick, so BRAKE/GAS come before Z/RZ.
macro_rules! standard {
    ($south:literal, $east:literal, $north:literal, $west:literal, $lb:literal, $rb:literal, $lt:literal, $rt:literal, $select:literal, $start:literal, $guide:literal, $ls:literal, $rs:literal) => {
        standard!($south, $east, $north, $west, $lb, $rb, $lt, $rt, $select, $start, $guide, $ls, $rs; &["BTN_SOUTH"], &["BTN_EAST"], &["BTN_NORTH"], &["BTN_WEST"])
    };
    ($south:literal, $east:literal, $north:literal, $west:literal, $lb:literal, $rb:literal, $lt:literal, $rt:literal, $select:literal, $start:literal, $guide:literal, $ls:literal, $rs:literal; $south_codes:expr, $east_codes:expr, $north_codes:expr, $west_codes:expr) => {
        standard!($south, $east, $north, $west, $lb, $rb, $lt, $rt, $select, $start, $guide, $ls, $rs; $south_codes, $east_codes, $north_codes, $west_codes; &["BTN_TL2", "ABS_BRAKE+", "ABS_Z+"], &["BTN_TR2", "ABS_GAS+", "ABS_RZ+"])
    };
    ($south:literal, $east:literal, $north:literal, $west:literal, $lb:literal, $rb:literal, $lt:literal, $rt:literal, $select:literal, $start:literal, $guide:literal, $ls:literal, $rs:literal; $south_codes:expr, $east_codes:expr, $north_codes:expr, $west_codes:expr; $lt_codes:expr, $rt_codes:expr) => {
        [
            slot("south", $south, $south_codes),
            slot("east", $east, $east_codes),
            slot("north", $north, $north_codes),
            slot("west", $west, $west_codes),
            slot("lb", $lb, &["BTN_TL"]),
            slot("rb", $rb, &["BTN_TR"]),
            slot("lt", $lt, $lt_codes),
            slot("rt", $rt, $rt_codes),
            slot("select", $select, &["BTN_SELECT"]),
            slot("start", $start, &["BTN_START"]),
            slot("guide", $guide, &["BTN_MODE"]),
            slot("ls", $ls, &["BTN_THUMBL"]),
            slot("rs", $rs, &["BTN_THUMBR"]),
            slot("dpad_up", "D-pad up", &["BTN_DPAD_UP", "ABS_HAT0Y-"]),
            slot("dpad_down", "D-pad down", &["BTN_DPAD_DOWN", "ABS_HAT0Y+"]),
            slot("dpad_left", "D-pad left", &["BTN_DPAD_LEFT", "ABS_HAT0X-"]),
            slot("dpad_right", "D-pad right", &["BTN_DPAD_RIGHT", "ABS_HAT0X+"]),
        ]
    };
}

pub const STANDARD: [Slot; 17] = standard!("A", "B", "Y", "X", "LB", "RB", "LT", "RT", "Select", "Start", "Guide", "LS", "RS");
const SONY: [Slot; 17] = standard!("Cross", "Circle", "Triangle", "Square", "L1", "R1", "L2", "R2", "Create", "Options", "PS", "L3", "R3");
const SONY_DS4: [Slot; 17] = standard!("Cross", "Circle", "Triangle", "Square", "L1", "R1", "L2", "R2", "Share", "Options", "PS", "L3", "R3");
// xpad, hid-microsoft, hid-steam and Steam Input's pad report X and Y by letter: X on the left is BTN_X (BTN_NORTH), Y on top BTN_Y (BTN_WEST).
// hid-microsoft lists BTN_TL2/BTN_TR2 from the Bluetooth descriptor's unused buttons 9 and 10: the pull is the trigger.
const XBOX: [Slot; 17] = standard!("A", "B", "Y", "X", "LB", "RB", "LT", "RT", "View", "Menu", "Xbox", "LS", "RS";
    &["BTN_SOUTH"], &["BTN_EAST"], &["BTN_WEST"], &["BTN_NORTH"]; &["ABS_BRAKE+", "ABS_Z+", "BTN_TL2"], &["ABS_GAS+", "ABS_RZ+", "BTN_TR2"]);
const DECK: [Slot; 17] =
    standard!("A", "B", "Y", "X", "L1", "R1", "L2", "R2", "View", "Menu", "Steam", "L3", "R3"; &["BTN_SOUTH"], &["BTN_EAST"], &["BTN_WEST"], &["BTN_NORTH"]);
const SWITCH: [Slot; 17] = standard!("B", "A", "X", "Y", "L", "R", "ZL", "ZR", "Minus", "Plus", "Home", "LS", "RS");
// In D-input mode the Pro 3 reports its lettered buttons by their letters, Xbox-fashion: the A on the right is BTN_SOUTH, the B at the bottom BTN_EAST.
const EIGHTBITDO: [Slot; 17] =
    standard!("B", "A", "X", "Y", "L1", "R1", "L2", "R2", "Select", "Start", "Home", "L3", "R3"; &["BTN_EAST"], &["BTN_SOUTH"], &["BTN_NORTH"], &["BTN_WEST"]);

// The hid-nintendo Switch 2 driver (v13, not yet merged) reports A, B, X and Y by letter: the A on the right is BTN_SOUTH.
const SWITCH2: [Slot; 17] =
    standard!("B", "A", "X", "Y", "L", "R", "ZL", "ZR", "Minus", "Plus", "Home", "LS", "RS"; &["BTN_EAST"], &["BTN_SOUTH"], &["BTN_NORTH"], &["BTN_WEST"]);

pub struct Family {
    pub id: &'static str,
    pub name: &'static str,
    standard: &'static [Slot; 17],
    extras: &'static [Slot],
    /// (vendor, product); product 0 matches the vendor alone
    ids: &'static [(u16, u16)],
    name_hints: &'static [&'static str],
}

impl Family {
    pub fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.standard.iter().chain(self.extras)
    }

    /// Where its print puts A: on the right on a Nintendo-style pad, at the bottom otherwise (a PlayStation pad's Cross).
    pub fn printed_layout(&self) -> &'static str {
        if self.standard[1].label == "A" {
            "positional"
        } else {
            "xbox"
        }
    }
}

const FAMILIES: [Family; 10] = [
    // hid-steam's own device. Steam Input's virtual pad counts only on Deck hardware: `detect_family_on`.
    Family {
        id: "steam-deck",
        name: "Steam Deck",
        standard: &DECK,
        extras: &[
            extra("grip_l4", "L4", &["BTN_GRIPL"]),
            extra("grip_l5", "L5", &["BTN_GRIPL2"]),
            extra("grip_r4", "R4", &["BTN_GRIPR"]),
            extra("grip_r5", "R5", &["BTN_GRIPR2"]),
            extra("quick", "Quick access", &["BTN_BASE"]),
            extra("pad_left", "Left trackpad", &["BTN_THUMB"]),
            extra("pad_right", "Right trackpad", &["BTN_THUMB2"]),
        ],
        ids: &[(0x28de, 0x1205)],
        name_hints: &["steam deck"],
    },
    Family {
        id: "dualsense-edge",
        name: "DualSense Edge",
        standard: &SONY,
        extras: &[
            extra("fn_left", "Left Fn", &["BTN_TRIGGER_HAPPY1"]),
            extra("fn_right", "Right Fn", &["BTN_TRIGGER_HAPPY2"]),
            extra("paddle_left", "Left back button (LB)", &["BTN_TRIGGER_HAPPY3"]),
            extra("paddle_right", "Right back button (RB)", &["BTN_TRIGGER_HAPPY4"]),
        ],
        ids: &[(0x054c, 0x0df2)],
        name_hints: &["dualsense edge"],
    },
    Family { id: "dualsense", name: "DualSense", standard: &SONY, extras: &[], ids: &[(0x054c, 0x0ce6)], name_hints: &["dualsense"] },
    Family {
        id: "dualshock4",
        name: "DualShock 4",
        standard: &SONY_DS4,
        extras: &[],
        ids: &[(0x054c, 0x05c4), (0x054c, 0x09cc), (0x054c, 0x0ba0)],
        name_hints: &["dualshock", "wireless controller"],
    },
    Family {
        id: "xbox-elite",
        name: "Xbox Elite Series 2",
        standard: &XBOX,
        extras: &[
            extra("paddle_p1", "P1 (right upper)", &["BTN_GRIPR", "BTN_TRIGGER_HAPPY5"]),
            extra("paddle_p2", "P2 (right lower)", &["BTN_GRIPR2", "BTN_TRIGGER_HAPPY6"]),
            extra("paddle_p3", "P3 (left upper)", &["BTN_GRIPL", "BTN_TRIGGER_HAPPY7"]),
            extra("paddle_p4", "P4 (left lower)", &["BTN_GRIPL2", "BTN_TRIGGER_HAPPY8"]),
        ],
        ids: &[(0x045e, 0x0b00), (0x045e, 0x0b05), (0x045e, 0x0b22)],
        name_hints: &["elite"],
    },
    Family {
        id: "xbox",
        name: "Xbox",
        standard: &XBOX,
        extras: &[extra("share", "Share", &["BTN_TRIGGER_HAPPY1", "KEY_RECORD"])],
        ids: &[(0x045e, 0)],
        name_hints: &["xbox", "microsoft", "x-box"],
    },
    Family {
        id: "switch2-pro",
        name: "Switch 2 Pro",
        standard: &SWITCH2,
        extras: &[
            extra("gl", "GL", &["BTN_GRIPL"]),
            extra("gr", "GR", &["BTN_GRIPR"]),
            extra("c", "C", &["BTN_C"]),
            extra("capture", "Capture", &["KEY_RECORD"]),
        ],
        ids: &[(0x057e, 0x2069)],
        name_hints: &["pro controller 2", "switch 2 pro"],
    },
    Family {
        id: "switch-pro",
        name: "Switch Pro",
        standard: &SWITCH,
        extras: &[extra("capture", "Capture", &["BTN_Z"])],
        ids: &[(0x057e, 0x2009), (0x057e, 0x2017)],
        name_hints: &["pro controller", "nintendo"],
    },
    Family {
        id: "8bitdo-pro-3",
        name: "8BitDo Pro 3",
        standard: &EIGHTBITDO,
        extras: &[
            extra("paddle_l4", "L4", &["BTN_TRIGGER_HAPPY1"]),
            extra("paddle_r4", "R4", &["BTN_TRIGGER_HAPPY2"]),
            extra("paddle_pl", "PL", &["BTN_Z"]),
            extra("paddle_pr", "PR", &["BTN_C"]),
            // Handled on the pad in D-input mode: it reaches evdev in no mode seen yet.
            extra("star", "Star", &[]),
        ],
        ids: &[(0x2dc8, 0x6009)],
        name_hints: &["8bitdo pro 3"],
    },
    Family { id: "generic", name: "Controller", standard: &STANDARD, extras: &[], ids: &[], name_hints: &[] },
];

pub fn families() -> &'static [Family] {
    &FAMILIES
}

pub fn family_by_id(id: &str) -> Option<&'static Family> {
    FAMILIES.iter().find(|s| s.id == id)
}

/// Steam Input's virtual pad: one per physical pad, whatever that pad is.
pub const STEAM_VIRTUAL: (u16, u16) = (0x28de, 0x11ff);

/// On a Deck, Steam's virtual pad is taken for the built-in controls; an external pad routed through Steam reads as one too.
pub fn detect_family_on(deck: bool, vendor: u16, product: u16, name: &str, keys: &[u16]) -> &'static Family {
    if deck && (vendor, product) == STEAM_VIRTUAL {
        return family_by_id("steam-deck").unwrap();
    }
    detect_family(vendor, product, name, keys)
}

/// Off a Deck, Steam's virtual pad is a copy of a real one.
pub fn steam_copy(deck: bool, id: (u16, u16)) -> bool {
    !deck && id == STEAM_VIRTUAL
}

/// Which pads, by (vendor, product), are shown: Steam's copies only while no real pad is here.
pub fn shown_pads(deck: bool, pads: &[(u16, u16)]) -> Vec<bool> {
    let real = pads.iter().any(|id| !steam_copy(deck, *id));
    pads.iter().map(|id| !real || !steam_copy(deck, *id)).collect()
}

/// xpadneo presents an Elite as a plain "Xbox Wireless Controller" 045e:028e: grip or paddle codes make it an Elite whatever it says.
pub fn detect_family(vendor: u16, product: u16, name: &str, keys: &[u16]) -> &'static Family {
    let lname = name.to_lowercase();
    let family = FAMILIES
        .iter()
        .find(|s| s.ids.iter().any(|(v, p)| *v == vendor && *p == product && *p != 0))
        .or_else(|| FAMILIES.iter().find(|s| s.name_hints.iter().any(|h| lname.contains(h)) && s.ids.iter().any(|(v, _)| *v == vendor)))
        .or_else(|| FAMILIES.iter().find(|s| s.ids.iter().any(|(v, p)| *v == vendor && *p == 0)))
        .or_else(|| FAMILIES.iter().find(|s| s.name_hints.iter().any(|h| lname.contains(h))))
        .unwrap_or_else(|| family_by_id("generic").unwrap());
    if family.id == "xbox" && keys.iter().any(|k| (0x224..=0x227).contains(k) || (708..=711).contains(k)) {
        return family_by_id("xbox-elite").unwrap();
    }
    family
}

/// The first code of a slot the pad advertises wins; a slot with none present stays unbound.
pub fn resolve_slots(config: &ControllerConfig, family: &Family, keys: &[u16], axes: &[u16]) -> BTreeMap<String, Option<keys::Source>> {
    let mut out = BTreeMap::new();
    let mut taken: Vec<keys::Source> = Vec::new();
    for s in family.slots() {
        let found = config.codes_for(family, s.id).iter().filter_map(|c| keys::parse_source(c)).find(|src| src.present(keys, axes) && !taken.contains(src));
        if let Some(src) = found {
            taken.push(src);
        }
        out.insert(s.id.to_string(), found);
    }
    out
}

pub const AXIS_ROLES: [&str; 6] = ["lx", "ly", "rx", "ry", "lt", "rt"];

/// What each absolute axis of a pad stands for (role, runs backwards): what was learned for the family, else read off the
/// pad's shape — a right stick on RX/RY leaves Z/RZ to the triggers (xpad, hid-playstation); without one, Z/RZ are the
/// right stick and BRAKE/GAS the triggers (Android-style HID, a D-input 8BitDo); HAT2Y/HAT2X are the Deck's (hid-steam).
pub fn axis_roles(config: &ControllerConfig, family: &Family, axes: &[u16]) -> BTreeMap<u16, (&'static str, bool)> {
    let mut out = BTreeMap::new();
    let has = |c: u16| axes.contains(&c);
    let stick_right = has(3) || has(4);
    let guess: [(u16, &'static str); 10] = [
        (0, "lx"),
        (1, "ly"),
        (3, "rx"),
        (4, "ry"),
        (2, if stick_right { "lt" } else { "rx" }),
        (5, if stick_right { "rt" } else { "ry" }),
        (10, "lt"),
        (9, "rt"),
        (21, "lt"),
        (20, "rt"),
    ];
    for (code, role) in guess {
        if has(code) && !out.values().any(|(r, _)| *r == role) {
            out.insert(code, (role, false));
        }
    }
    if let Some(learned) = config.axes.get(family.id) {
        for role in AXIS_ROLES {
            let Some(text) = learned.get(role) else { continue };
            let Some(keys::Source::Axis { code, positive }) = keys::parse_source(text) else { continue };
            if !has(code) {
                continue;
            }
            out.retain(|_, (r, _)| *r != role);
            out.insert(code, (role, !positive));
        }
    }
    out
}

/// A slot's source as SDL numbers a Linux joystick: buttons from BTN_JOYSTICK up then the rest, axes in code order without the hats.
pub fn sdl_element(source: &keys::Source, keys: &[u16], axes: &[u16]) -> String {
    match *source {
        keys::Source::Key(code) => {
            let mut order: Vec<u16> = keys.to_vec();
            order.sort_by_key(|k| if *k >= 0x120 { (0, *k) } else { (1, *k) });
            order.iter().position(|k| *k == code).map(|i| format!("b{i}")).unwrap_or_default()
        }
        keys::Source::Axis { code: code @ 16..=23, positive } => {
            let hat = (code - 16) / 2;
            let bit = match (code % 2, positive) {
                (0, false) => 8,
                (0, true) => 2,
                (_, false) => 1,
                _ => 4,
            };
            format!("h{hat}.{bit}")
        }
        keys::Source::Axis { code, positive } => sdl_axis(code, axes).map(|a| format!("{}{a}", if positive { "+" } else { "-" })).unwrap_or_default(),
    }
}

pub fn sdl_axis(code: u16, axes: &[u16]) -> Option<String> {
    let mut order: Vec<u16> = axes.iter().copied().filter(|a| !(16..=23).contains(a)).collect();
    order.sort_unstable();
    order.iter().position(|a| *a == code).map(|i| format!("a{i}"))
}

pub fn state_json(config: &ControllerConfig) -> serde_json::Value {
    let families: Vec<serde_json::Value> = families()
        .iter()
        .map(|f| {
            serde_json::json!({
                "id": f.id, "name": f.name,
                "slots": f.slots().map(|s| serde_json::json!({"id": s.id, "label": s.label, "codes": config.codes_for(f, s.id), "extra": s.extra})).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::json!({
        "enabled": config.enabled,
        "hold_ms": config.hold_ms,
        "volume_step": config.volume_step,
        "home_summons": config.home_summons,
        "layout": config.layout,
        "families": families,
        "macros": config.macros(),
        "presets": PRESETS.iter().map(|p| serde_json::json!({"id": p.id, "label": p.label, "hold_only": p.hold_only})).collect::<Vec<_>>(),
    })
}

fn macros_to_toml(list: &[Macro]) -> toml_edit::ArrayOfTables {
    let mut arr = toml_edit::ArrayOfTables::new();
    for m in list {
        let mut t = toml_edit::Table::new();
        t["family"] = toml_edit::value(&m.family);
        t["button"] = toml_edit::value(&m.button);
        t["trigger"] = toml_edit::value(&m.trigger);
        t["action"] = toml_edit::value(&m.action);
        if !m.keys.is_empty() {
            t["keys"] = toml_edit::value(&m.keys);
        }
        if !m.command.is_empty() {
            t["command"] = toml_edit::value(&m.command);
        }
        arr.push(t);
    }
    arr
}

fn controller_table(doc: &mut toml_edit::DocumentMut) -> &mut toml_edit::Table {
    let item = doc.entry("controller").or_insert(toml_edit::table());
    if !item.is_table() {
        *item = toml_edit::table();
    }
    let t = item.as_table_mut().unwrap();
    t.set_implicit(false);
    t
}

/// Writes the whole macro list: the seeds materialize the first time anything changes, so removing one sticks.
pub fn write_macros(list: &[Macro]) -> crate::Result<()> {
    let path = paths::config_file();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc = crate::config::document(&text)?;
    controller_table(&mut doc)["macros"] = toml_edit::Item::ArrayOfTables(macros_to_toml(list));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

/// `None` forgets what was learned (the seeds apply again); an empty list leaves the slot unbound.
pub fn write_button(family: &str, slot: &str, codes: Option<&[String]>) -> crate::Result<()> {
    let value = codes.map(|c| format!("[{}]", c.join(","))).unwrap_or_default();
    crate::config::Config::set_key(&paths::config_file(), &format!("controller.buttons.{family}.{slot}"), &value)
}

/// `None` forgets the learned axis; the pad's shape decides again.
pub fn write_axis(family: &str, role: &str, code: Option<&str>) -> crate::Result<()> {
    crate::config::Config::set_key(&paths::config_file(), &format!("controller.axes.{family}.{role}"), code.unwrap_or_default())
}

pub fn learn_axis(family: &Family, role: &str, code: &str) -> crate::Result<()> {
    if !AXIS_ROLES.contains(&role) {
        return Err(crate::Error::Invalid(format!("'{role}' is not a stick or trigger")));
    }
    if !matches!(keys::parse_source(code), Some(keys::Source::Axis { code: 0..=15, .. })) {
        return Err(crate::Error::Invalid(format!("'{code}' is not an axis")));
    }
    write_axis(family.id, role, Some(code))
}

pub fn upsert_macro(list: &mut Vec<Macro>, m: Macro) {
    list.retain(|x| (&x.family, &x.button, &x.trigger) != (&m.family, &m.button, &m.trigger));
    list.push(m);
}

pub fn learn_code(config: &ControllerConfig, family: &Family, slot: &str, code: &str) -> crate::Result<Option<String>> {
    if !family.slots().any(|s| s.id == slot) {
        return Err(crate::Error::Invalid(format!("{} has no button '{slot}'", family.name)));
    }
    let mut previous = None;
    for s in family.slots() {
        if s.id == slot {
            continue;
        }
        let codes = config.codes_for(family, s.id);
        if codes.iter().any(|c| c == code) {
            previous = Some(s.id.to_string());
            let rest: Vec<String> = codes.into_iter().filter(|c| c != code).collect();
            write_button(family.id, s.id, Some(&rest))?;
        }
    }
    let mut codes = vec![code.to_string()];
    codes.extend(config.codes_for(family, slot).into_iter().filter(|c| c != code));
    write_button(family.id, slot, Some(&codes))?;
    Ok(previous)
}

/// The pads recorded under tests/pads, one file per pad, bus and driver, as the tests read their evdev side.
#[cfg(test)]
pub(crate) mod corpus {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::str::FromStr;

    pub struct Pad {
        pub id: String,
        pub entry: serde_json::Value,
    }

    pub fn pads() -> Vec<Pad> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/pads");
        let mut out: Vec<Pad> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .flatten()
            .map(|f| f.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .map(|p| Pad {
                id: p.file_stem().unwrap().to_string_lossy().into_owned(),
                entry: serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn pad(id: &str) -> Pad {
        pads().into_iter().find(|p| p.id == id).unwrap_or_else(|| panic!("no tests/pads/{id}.json"))
    }

    impl Pad {
        pub fn id_field(&self, name: &str) -> u16 {
            u16::from_str_radix(self.entry["evdev"][name].as_str().unwrap(), 16).unwrap()
        }

        pub fn name(&self) -> &str {
            self.entry["evdev"]["name"].as_str().unwrap()
        }

        pub fn keys(&self) -> Vec<u16> {
            let names = self.entry["evdev"]["keys"].as_array().unwrap();
            names.iter().map(|k| super::keys::parse_key(k.as_str().unwrap()).unwrap_or_else(|| panic!("{}: key {k}", self.id))).collect()
        }

        pub fn ranges(&self) -> BTreeMap<u16, (i32, i32)> {
            let abs = self.entry["evdev"]["abs"].as_object().unwrap();
            abs.iter()
                .map(|(name, r)| {
                    let code = evdev::AbsoluteAxisCode::from_str(name).unwrap_or_else(|_| panic!("{}: axis {name}", self.id)).0;
                    (code, (r[0].as_i64().unwrap() as i32, r[1].as_i64().unwrap() as i32))
                })
                .collect()
        }

        pub fn axes(&self) -> Vec<u16> {
            self.ranges().into_keys().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The recorded pads are found by their ids (watch.rs); left over: names alone, an older xpadneo's paddle codes, a pad nobody knows.
    #[test]
    fn families_by_id_name_and_capabilities() {
        assert_eq!(detect_family(0x045e, 0x0b22, "Xbox Wireless Controller", &[]).id, "xbox-elite");
        assert_eq!(detect_family(0x045e, 0x028e, "Xbox Wireless Controller", &[0x130, 708]).id, "xbox-elite", "xpadneo before 0.10: TRIGGER_HAPPY5-8");
        assert_eq!(detect_family(0x054c, 0x0e5f, "DualSense Edge Wireless Controller", &[]).id, "dualsense-edge", "an unknown product, by its name");
        assert_eq!(detect_family(0x1234, 0x0001, "Some Pad", &[]).id, "generic");
    }

    #[test]
    fn steams_copy_of_a_pad_stays_hidden_while_a_real_pad_is_here() {
        let (copy, elite) = (STEAM_VIRTUAL, (0x045e, 0x028e));
        assert_eq!(shown_pads(false, &[copy, elite]), [false, true]);
        assert_eq!(shown_pads(false, &[elite, copy]), [true, false]);
        assert_eq!(shown_pads(false, &[copy]), [true], "a pad only Steam passes on");
        assert_eq!(shown_pads(true, &[copy, elite]), [true, true], "a Deck's own controls");
    }

    #[test]
    fn a_deck_is_known_by_hid_steam_and_by_steam_inputs_pad_on_valve_hardware() {
        let deck = detect_family(0x28de, 0x1205, "Steam Deck", &[]);
        assert_eq!(deck.id, "steam-deck");
        let codes: Vec<&str> = deck.slots().filter(|s| s.extra).map(|s| s.codes[0]).collect();
        assert_eq!(codes, ["BTN_GRIPL", "BTN_GRIPL2", "BTN_GRIPR", "BTN_GRIPR2", "BTN_BASE", "BTN_THUMB", "BTN_THUMB2"]);
        assert_eq!(detect_family_on(true, 0x28de, 0x11ff, "Microsoft X-Box 360 pad 0", &[]).id, "steam-deck");
        assert_eq!(detect_family_on(false, 0x28de, 0x11ff, "Microsoft X-Box 360 pad 0", &[]).id, "xbox", "a desktop's Steam Input pad");
        assert_eq!(detect_family_on(true, 0x054c, 0x0ce6, "DualSense Wireless Controller", &[]).id, "dualsense", "a pad Steam leaves alone");
        // ABS_X ABS_Y ABS_RX ABS_RY, the pads on HAT0/HAT1, the triggers on HAT2Y (left) and HAT2X (right)
        let roles = axis_roles(&ControllerConfig::default(), deck, &[0, 1, 3, 4, 16, 17, 18, 19, 20, 21]);
        assert_eq!((roles[&21].0, roles[&20].0), ("lt", "rt"));
        assert!(!roles.contains_key(&16), "a trackpad is no stick");
    }

    fn pro3() -> (Vec<u16>, Vec<u16>) {
        let pad = corpus::pad("8bitdo-pro-3-dinput-bt");
        (pad.keys(), pad.axes())
    }

    #[test]
    fn a_learned_axis_replaces_the_guess() {
        let (_, axes) = pro3();
        let pro3 = family_by_id("8bitdo-pro-3").unwrap();
        let mut cfg = ControllerConfig::default();
        cfg.axes.insert(
            "8bitdo-pro-3".into(),
            [("rx".to_string(), "ABS_RZ-".to_string()), ("ry".to_string(), "ABS_Z".to_string()), ("lt".to_string(), "ABS_RX".to_string())].into(),
        );
        let learned = axis_roles(&cfg, pro3, &axes);
        assert_eq!((learned[&5], learned[&2]), (("rx", true), ("ry", false)), "backwards when thrown the other way");
        assert_eq!(learned[&10], ("lt", false), "an axis the pad lacks leaves the guess");
    }

    #[test]
    fn sdl_numbers_the_pad_as_its_linux_joystick_does() {
        let (keys, axes) = pro3();
        let el = |src: &str| sdl_element(&keys::parse_source(src).unwrap(), &keys, &axes);
        assert_eq!(el("BTN_SOUTH"), "b0");
        assert_eq!(el("BTN_THUMBR"), "b14");
        assert_eq!(el("BTN_TRIGGER_HAPPY1"), "b16");
        assert_eq!(el("ABS_HAT0Y-"), "h0.1");
        assert_eq!(el("ABS_HAT0X+"), "h0.2");
        assert_eq!(el("ABS_HAT0Y+"), "h0.4");
        assert_eq!(el("ABS_HAT0X-"), "h0.8");
        assert_eq!(el("ABS_GAS+"), "+a4");
        assert_eq!(el("ABS_BRAKE+"), "+a5");
        assert_eq!(sdl_axis(5, &axes).as_deref(), Some("a3"));
        assert_eq!(el("BTN_GRIPL"), "", "a code the pad lacks has no number");
        let misc: Vec<u16> = [0x100, 0x130, 0x131].to_vec();
        assert_eq!(sdl_element(&keys::Source::Key(0x100), &misc, &[]), "b2", "BTN_MISC comes after the joystick range");
    }

    // hid-steam reports a Deck's X and Y by letter, as xpad does an Xbox pad's (watch.rs reads the recorded ones).
    #[test]
    fn a_decks_x_and_y_land_where_they_sit_and_a_walk_stays() {
        let pad = corpus::pad("xbox-usb");
        let (keys, axes) = (pad.keys(), pad.axes());
        let s = resolve_slots(&ControllerConfig::default(), family_by_id("steam-deck").unwrap(), &keys, &axes);
        assert_eq!((s["west"].unwrap().to_string(), s["north"].unwrap().to_string()), ("BTN_NORTH".into(), "BTN_WEST".into()));
        let mut walked = ControllerConfig::default();
        walked.buttons.entry("xbox".into()).or_default().insert("north".into(), vec!["BTN_NORTH".into()]);
        let north = resolve_slots(&walked, family_by_id("xbox").unwrap(), &keys, &axes)["north"].unwrap().to_string();
        assert_eq!(north, "BTN_NORTH", "a walk's learned codes stay");
    }

    #[test]
    fn learned_codes_come_first_and_one_source_serves_one_slot() {
        let mut cfg = ControllerConfig::default();
        cfg.buttons.entry("xbox-elite".into()).or_default().insert("paddle_p1".into(), vec!["BTN_TRIGGER_HAPPY8".into()]);
        let elite = family_by_id("xbox-elite").unwrap();
        let usb: Vec<u16> = [0x130, 708, 709, 710, 711].to_vec();
        let s = resolve_slots(&cfg, elite, &usb, &[]);
        assert_eq!(s["paddle_p1"].unwrap().to_string(), "BTN_TRIGGER_HAPPY8");
        assert!(s["paddle_p4"].is_none(), "its seed now drives P1");
        assert_eq!(cfg.codes_for(elite, "paddle_p1"), vec!["BTN_TRIGGER_HAPPY8"], "a learned list replaces the seeds");
        cfg.buttons.get_mut("xbox-elite").unwrap().insert("paddle_p2".into(), vec![]);
        assert!(resolve_slots(&cfg, elite, &usb, &[])["paddle_p2"].is_none(), "an empty learned list keeps the slot unbound");
    }

    #[test]
    fn macros_default_to_the_seeds_until_written() {
        let cfg: ControllerConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.macros().len(), 14);
        let cfg: ControllerConfig = toml::from_str("macros = []").unwrap();
        assert!(cfg.macros().is_empty());
        let cfg: ControllerConfig = toml::from_str("[[macros]]\nfamily = \"*\"\nbutton = \"start\"\ntrigger = \"hold\"\naction = \"stop\"\n[[macros]]\nfamily = \"*\"\nbutton = \"guide\"\ntrigger = \"hold\"\naction = \"stop\"\n").unwrap();
        assert_eq!(cfg.macros_for("dualsense", "start")[0].action, "stop");
        assert!(cfg.macros_for("dualsense", "guide").is_empty(), "Guide is HOME, whatever the file binds on it");
        assert!(cfg.macros_for("dualsense-edge", "fn_left").is_empty(), "written macros replace the seeds");
    }

    #[test]
    fn macro_validation() {
        assert!(Macro {
            family: "dualsense-edge".into(),
            button: "paddle_left".into(),
            trigger: "press".into(),
            action: "screenshot".into(),
            ..Macro::default()
        }
        .validate()
        .is_ok());
        assert!(
            Macro { family: "*".into(), button: "start".into(), trigger: "press".into(), action: "stop".into(), ..Macro::default() }.validate().is_err(),
            "stop is hold only"
        );
        assert!(
            Macro { family: "*".into(), button: "guide".into(), trigger: "hold".into(), action: "stop".into(), ..Macro::default() }.validate().is_err(),
            "Guide is HOME"
        );
        assert!(
            Macro { family: "*".into(), button: "paddle_left".into(), trigger: "press".into(), action: "mute".into(), ..Macro::default() }.validate().is_err(),
            "paddles are not standard"
        );
        assert!(Macro {
            family: "xbox".into(),
            button: "share".into(),
            trigger: "press".into(),
            action: "keys".into(),
            keys: "Ctrl+Shift+F12".into(),
            ..Macro::default()
        }
        .validate()
        .is_ok());
        assert!(Macro {
            family: "xbox".into(),
            button: "share".into(),
            trigger: "press".into(),
            action: "keys".into(),
            keys: "Ctrl+Nope".into(),
            ..Macro::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn macro_toml_round_trip() {
        let list = vec![Macro {
            family: "xbox".into(),
            button: "share".into(),
            trigger: "hold".into(),
            action: "command".into(),
            command: "notify-send hi".into(),
            ..Macro::default()
        }];
        let mut doc = toml_edit::DocumentMut::new();
        controller_table(&mut doc)["macros"] = toml_edit::Item::ArrayOfTables(macros_to_toml(&list));
        let text = doc.to_string();
        assert!(text.contains("[[controller.macros]]"), "{text}");
        let cfg: crate::config::Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg.controller.macros(), list);
    }
}
