//! The session's keyboard layout: gamescope builds a US keymap of its own, reading only `XKB_DEFAULT_LAYOUT` / `XKB_DEFAULT_VARIANT`.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Layout {
    pub layout: String,
    pub variant: String,
}

impl Layout {
    pub fn new(layout: &str, variant: &str) -> Layout {
        Layout { layout: layout.trim().to_string(), variant: variant.trim().to_string() }
    }

    /// One entry as the tools write it: `fr`, `fr+bepo` (GNOME), `fr:bepo` (gamescope), `fr(bepo)` (xkb). Empty is none.
    pub fn parse(spec: &str) -> Option<Layout> {
        let spec = spec.trim();
        if spec.is_empty() {
            return None;
        }
        let (layout, variant) = match spec.find(['+', ':', '(']) {
            Some(at) => (&spec[..at], spec[at + 1..].trim_end_matches(')')),
            None => (spec, ""),
        };
        let layout = layout.trim();
        (!layout.is_empty()).then(|| Layout::new(layout, variant))
    }

    /// `fr,us` and `bepo,`: the first of each list, the variant list running alongside the layouts'.
    fn from_lists(layouts: &str, variants: &str) -> Option<Layout> {
        let layout = layouts.split(',').next().unwrap_or("").trim();
        let variant = variants.split(',').next().unwrap_or("").trim();
        (!layout.is_empty()).then(|| Layout::new(layout, variant))
    }

    pub fn env(&self) -> [(String, String); 2] {
        [("XKB_DEFAULT_LAYOUT".into(), self.layout.clone()), ("XKB_DEFAULT_VARIANT".into(), self.variant.clone())]
    }
}

pub fn probe() -> Layout {
    from_env().or_else(from_desktop).or_else(from_localectl).unwrap_or_else(|| Layout::new("us", ""))
}

fn from_env() -> Option<Layout> {
    let layouts = std::env::var("XKB_DEFAULT_LAYOUT").unwrap_or_default();
    let variants = std::env::var("XKB_DEFAULT_VARIANT").unwrap_or_default();
    Layout::from_lists(&layouts, &variants)
}

fn from_desktop() -> Option<Layout> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_ascii_lowercase();
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return from_hyprland();
    }
    if desktop.contains("gnome") {
        return from_gnome();
    }
    if desktop.contains("kde") {
        return from_kde();
    }
    None
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn from_gnome() -> Option<Layout> {
    // The switched-to source leads `mru-sources`; `sources` is the list as configured.
    ["mru-sources", "sources"].iter().find_map(|key| gnome_sources(&output("gsettings", &["get", "org.gnome.desktop.input-sources", key])?))
}

/// `[('xkb', 'fr'), ('ibus', 'anthy')]`, or `@a(ss) []` when unset: the first `xkb` source.
fn gnome_sources(text: &str) -> Option<Layout> {
    text.split('(').skip(1).find_map(|entry| {
        let mut parts = entry.split('\'').skip(1).step_by(2);
        let kind = parts.next()?;
        let source = parts.next()?;
        (kind == "xkb").then(|| Layout::parse(source)).flatten()
    })
}

fn from_hyprland() -> Option<Layout> {
    let layouts = hyprland_option(&output("hyprctl", &["-j", "getoption", "input:kb_layout"])?)?;
    let variants = output("hyprctl", &["-j", "getoption", "input:kb_variant"]).and_then(|t| hyprland_option(&t)).unwrap_or_default();
    Layout::from_lists(&layouts, &variants)
}

fn hyprland_option(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("str").and_then(|s| s.as_str()).map(|s| s.to_string())
}

fn from_kde() -> Option<Layout> {
    let config =
        std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))?;
    kxkbrc(&std::fs::read_to_string(config.join("kxkbrc")).ok()?)
}

/// `[Layout]` with `LayoutList=fr,us` and `VariantList=bepo,`; a kxkbrc without them leaves the system's layout in place.
fn kxkbrc(text: &str) -> Option<Layout> {
    let mut in_layout = false;
    let (mut layouts, mut variants) = (String::new(), String::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_layout = line == "[Layout]";
        } else if in_layout {
            if let Some(v) = line.strip_prefix("LayoutList=") {
                layouts = v.to_string();
            } else if let Some(v) = line.strip_prefix("VariantList=") {
                variants = v.to_string();
            }
        }
    }
    Layout::from_lists(&layouts, &variants)
}

fn from_localectl() -> Option<Layout> {
    localectl(&output("localectl", &["status"])?)
}

/// `X11 Layout: fr` with its `X11 Variant`, else the `VC Keymap` up to its charset (`fr-latin9` is `fr`).
fn localectl(text: &str) -> Option<Layout> {
    let field = |name: &str| text.lines().find_map(|l| l.trim().strip_prefix(name).and_then(|rest| rest.strip_prefix(':')).map(|v| v.trim().to_string()));
    if let Some(layout) = Layout::from_lists(&field("X11 Layout").unwrap_or_default(), &field("X11 Variant").unwrap_or_default()) {
        return Some(layout);
    }
    let keymap = field("VC Keymap")?;
    let layout = keymap.split('-').next().unwrap_or("").trim();
    (!layout.is_empty() && layout != "(unset)").then(|| Layout::new(layout, ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_splits_into_layout_and_variant() {
        assert_eq!(Layout::parse("fr"), Some(Layout::new("fr", "")));
        assert_eq!(Layout::parse("fr+bepo"), Some(Layout::new("fr", "bepo")));
        assert_eq!(Layout::parse("fr:bepo"), Some(Layout::new("fr", "bepo")));
        assert_eq!(Layout::parse("us(intl)"), Some(Layout::new("us", "intl")));
        assert_eq!(Layout::parse(" "), None);
        assert_eq!(Layout::from_lists("fr,us", "bepo,"), Some(Layout::new("fr", "bepo")));
        assert_eq!(Layout::from_lists("", "bepo"), None);
    }

    #[test]
    fn gnome_takes_the_first_xkb_source() {
        assert_eq!(gnome_sources("[('xkb', 'fr')]\n"), Some(Layout::new("fr", "")));
        assert_eq!(gnome_sources("[('ibus', 'anthy'), ('xkb', 'de+nodeadkeys'), ('xkb', 'us')]"), Some(Layout::new("de", "nodeadkeys")));
        assert_eq!(gnome_sources("@a(ss) []"), None);
    }

    #[test]
    fn hyprland_kde_and_localectl_are_read() {
        assert_eq!(hyprland_option(r#"{"option": "input:kb_layout", "str": "fr,us", "set": true}"#).as_deref(), Some("fr,us"));
        assert_eq!(kxkbrc("[Layout]\nLayoutList=ch,us\nVariantList=fr,\nUse=true\n"), Some(Layout::new("ch", "fr")));
        assert_eq!(kxkbrc("[Layout]\nUse=false\n"), None);
        assert_eq!(
            localectl("   System Locale: LANG=en_US.UTF-8\n       VC Keymap: fr-latin9\n      X11 Layout: fr\n     X11 Variant: bepo\n"),
            Some(Layout::new("fr", "bepo"))
        );
        assert_eq!(localectl("   System Locale: LANG=C\n       VC Keymap: de-latin1-nodeadkeys\n"), Some(Layout::new("de", "")));
        assert_eq!(localectl("   System Locale: LANG=C\n       VC Keymap: (unset)\n"), None);
    }

    #[test]
    fn the_environment_is_exported_as_libxkbcommon_reads_it() {
        assert_eq!(Layout::new("fr", "").env(), [("XKB_DEFAULT_LAYOUT".to_string(), "fr".to_string()), ("XKB_DEFAULT_VARIANT".to_string(), String::new())]);
    }
}
