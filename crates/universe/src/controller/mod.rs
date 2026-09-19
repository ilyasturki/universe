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
    /// family → slot → learned codes, first present on the pad wins
    pub buttons: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// absent: the seeded workflow; `macros = []` is none at all
    pub macros: Option<Vec<Macro>>,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        ControllerConfig { enabled: true, hold_ms: 600, volume_step: 2, buttons: BTreeMap::new(), macros: None }
    }
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

macro_rules! standard {
    ($south:literal, $east:literal, $north:literal, $west:literal, $lb:literal, $rb:literal, $lt:literal, $rt:literal, $select:literal, $start:literal, $guide:literal, $ls:literal, $rs:literal) => {
        [
            slot("south", $south, &["BTN_SOUTH"]),
            slot("east", $east, &["BTN_EAST"]),
            slot("north", $north, &["BTN_NORTH"]),
            slot("west", $west, &["BTN_WEST"]),
            slot("lb", $lb, &["BTN_TL"]),
            slot("rb", $rb, &["BTN_TR"]),
            slot("lt", $lt, &["BTN_TL2", "ABS_Z+", "ABS_BRAKE+"]),
            slot("rt", $rt, &["BTN_TR2", "ABS_RZ+", "ABS_GAS+"]),
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
const XBOX: [Slot; 17] = standard!("A", "B", "Y", "X", "LB", "RB", "LT", "RT", "View", "Menu", "Xbox", "LS", "RS");
const SWITCH: [Slot; 17] = standard!("B", "A", "X", "Y", "L", "R", "ZL", "ZR", "Minus", "Plus", "Home", "LS", "RS");
const EIGHTBITDO: [Slot; 17] = standard!("B", "A", "X", "Y", "L1", "R1", "L2", "R2", "Select", "Start", "Home", "L3", "R3");

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
}

const FAMILIES: [Family; 8] = [
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
    Family { id: "dualshock4", name: "DualShock 4", standard: &SONY_DS4, extras: &[], ids: &[(0x054c, 0x05c4), (0x054c, 0x09cc), (0x054c, 0x0ba0)], name_hints: &["dualshock", "wireless controller"] },
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
    Family { id: "xbox", name: "Xbox", standard: &XBOX, extras: &[extra("share", "Share", &["BTN_TRIGGER_HAPPY1", "KEY_RECORD"])], ids: &[(0x045e, 0)], name_hints: &["xbox", "microsoft", "x-box"] },
    Family { id: "switch-pro", name: "Switch Pro", standard: &SWITCH, extras: &[extra("capture", "Capture", &["BTN_Z"])], ids: &[(0x057e, 0x2009), (0x057e, 0x2017)], name_hints: &["pro controller", "nintendo"] },
    Family {
        id: "8bitdo-pro-3",
        name: "8BitDo Pro 3",
        standard: &EIGHTBITDO,
        extras: &[
            extra("paddle_l4", "L4", &[]),
            extra("paddle_r4", "R4", &[]),
            extra("paddle_pl", "PL", &[]),
            extra("paddle_pr", "PR", &[]),
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
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| crate::Error::Invalid(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_by_id_name_and_capabilities() {
        assert_eq!(detect_family(0x054c, 0x0df2, "DualSense Edge Wireless Controller", &[]).id, "dualsense-edge");
        assert_eq!(detect_family(0x054c, 0x0ce6, "Sony Interactive Entertainment DualSense Wireless Controller", &[]).id, "dualsense");
        assert_eq!(detect_family(0x045e, 0x0b22, "Xbox Wireless Controller", &[]).id, "xbox-elite");
        assert_eq!(detect_family(0x045e, 0x0b12, "Xbox Wireless Controller", &[0x130]).id, "xbox");
        assert_eq!(detect_family(0x045e, 0x028e, "Xbox Wireless Controller", &[0x130, 548, 549, 550, 551]).id, "xbox-elite", "xpadneo's identity for an Elite: the grips give it away");
        assert_eq!(detect_family(0x045e, 0x028e, "Xbox Wireless Controller", &[0x130, 708]).id, "xbox-elite", "xpad over USB");
        assert_eq!(detect_family(0x2dc8, 0x6009, "8BitDo Pro 3", &[]).id, "8bitdo-pro-3");
        assert_eq!(detect_family(0x1234, 0x0001, "Some Pad", &[]).id, "generic");
        assert_eq!(detect_family(0x054c, 0x0df2, "DualSense Edge Wireless Controller", &[]).slots().filter(|s| s.extra).count(), 4);
    }

    #[test]
    fn resolve_edge_on_7_2_and_elite_on_both_drivers() {
        let cfg = ControllerConfig::default();
        let edge = family_by_id("dualsense-edge").unwrap();
        let keys: Vec<u16> = [0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x138, 0x139, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e, 704, 705, 706, 707].to_vec();
        let axes = vec![0, 1, 2, 3, 4, 5, 16, 17];
        let slots = resolve_slots(&cfg, edge, &keys, &axes);
        assert_eq!(slots["paddle_left"].unwrap().to_string(), "BTN_TRIGGER_HAPPY3");
        assert_eq!(slots["lt"].unwrap().to_string(), "BTN_TL2");
        assert_eq!(slots["dpad_left"].unwrap().to_string(), "ABS_HAT0X-");
        let older: Vec<u16> = keys.iter().copied().filter(|k| *k < 700).collect();
        assert!(resolve_slots(&cfg, edge, &older, &axes)["paddle_left"].is_none(), "no paddles before kernel 7.2");

        let elite = family_by_id("xbox-elite").unwrap();
        let bt: Vec<u16> = [0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e, 548, 549, 550, 551].to_vec();
        let s = resolve_slots(&cfg, elite, &bt, &[0, 1, 3, 4, 16, 17, 9, 10]);
        assert_eq!(s["paddle_p1"].unwrap().to_string(), "BTN_GRIPR");
        assert_eq!(s["paddle_p3"].unwrap().to_string(), "BTN_GRIPL");
        assert_eq!(s["lt"].unwrap().to_string(), "ABS_BRAKE+");
        let usb: Vec<u16> = [0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e, 708, 709, 710, 711].to_vec();
        let s = resolve_slots(&cfg, elite, &usb, &[0, 1, 2, 3, 4, 5, 16, 17]);
        assert_eq!(s["paddle_p1"].unwrap().to_string(), "BTN_TRIGGER_HAPPY5");
        assert_eq!(s["paddle_p4"].unwrap().to_string(), "BTN_TRIGGER_HAPPY8");
        assert_eq!(s["lt"].unwrap().to_string(), "ABS_Z+");
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
        assert_eq!(cfg.macros().len(), 11);
        let cfg: ControllerConfig = toml::from_str("macros = []").unwrap();
        assert!(cfg.macros().is_empty());
        let cfg: ControllerConfig = toml::from_str("[[macros]]\nfamily = \"*\"\nbutton = \"start\"\ntrigger = \"hold\"\naction = \"stop\"\n[[macros]]\nfamily = \"*\"\nbutton = \"guide\"\ntrigger = \"hold\"\naction = \"stop\"\n").unwrap();
        assert_eq!(cfg.macros_for("dualsense", "start")[0].action, "stop");
        assert!(cfg.macros_for("dualsense", "guide").is_empty(), "Guide is HOME, whatever an older config bound on it");
        assert!(cfg.macros_for("dualsense-edge", "fn_left").is_empty(), "written macros replace the seeds");
    }

    #[test]
    fn volume_step_is_a_percent() {
        let cfg: ControllerConfig = toml::from_str("").unwrap();
        assert_eq!(cfg.volume_step, 2);
        assert_eq!(toml::from_str::<ControllerConfig>("volume_step = 5").unwrap().volume_step, 5);
        assert!(toml::from_str::<ControllerConfig>("volume_step = \"loud\"").is_err());
        assert_eq!(volume_step_value("10").unwrap(), 10);
        assert!(volume_step_value("0").is_err() && volume_step_value("101").is_err() && volume_step_value("precise").is_err());
        assert_eq!(state_json(&cfg)["volume_step"], 2);
    }

    #[test]
    fn macro_validation() {
        assert!(Macro { family: "dualsense-edge".into(), button: "paddle_left".into(), trigger: "press".into(), action: "screenshot".into(), ..Macro::default() }.validate().is_ok());
        assert!(Macro { family: "*".into(), button: "start".into(), trigger: "press".into(), action: "stop".into(), ..Macro::default() }.validate().is_err(), "stop is hold only");
        assert!(Macro { family: "*".into(), button: "guide".into(), trigger: "hold".into(), action: "stop".into(), ..Macro::default() }.validate().is_err(), "Guide is HOME");
        assert!(Macro { family: "*".into(), button: "paddle_left".into(), trigger: "press".into(), action: "mute".into(), ..Macro::default() }.validate().is_err(), "paddles are not standard");
        assert!(Macro { family: "xbox".into(), button: "share".into(), trigger: "press".into(), action: "keys".into(), keys: "Ctrl+Shift+F12".into(), ..Macro::default() }.validate().is_ok());
        assert!(Macro { family: "xbox".into(), button: "share".into(), trigger: "press".into(), action: "keys".into(), keys: "Ctrl+Nope".into(), ..Macro::default() }.validate().is_err());
    }

    #[test]
    fn macro_toml_round_trip() {
        let list = vec![Macro { family: "xbox".into(), button: "share".into(), trigger: "hold".into(), action: "command".into(), command: "notify-send hi".into(), ..Macro::default() }];
        let mut doc = toml_edit::DocumentMut::new();
        controller_table(&mut doc)["macros"] = toml_edit::Item::ArrayOfTables(macros_to_toml(&list));
        let text = doc.to_string();
        assert!(text.contains("[[controller.macros]]"), "{text}");
        let cfg: crate::config::Config = toml::from_str(&text).unwrap();
        assert_eq!(cfg.controller.macros(), list);
    }

    #[test]
    fn state_lists_families_macros_and_presets() {
        let v = state_json(&ControllerConfig::default());
        assert_eq!(v["families"].as_array().unwrap().len(), 8);
        assert_eq!(v["presets"].as_array().unwrap().len(), 8);
        let edge = v["families"].as_array().unwrap().iter().find(|f| f["id"] == "dualsense-edge").unwrap();
        let slots = edge["slots"].as_array().unwrap();
        assert_eq!(slots[0]["label"], "Cross");
        assert_eq!(slots.last().unwrap()["id"], "paddle_right");
        assert_eq!(slots.last().unwrap()["codes"][0], "BTN_TRIGGER_HAPPY4");
    }
}
