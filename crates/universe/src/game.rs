use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Game {
    pub schema: u32,
    pub id: String,
    pub title: String,
    pub sort_title: String,
    pub platform: String,
    pub release_year: u32,
    pub hidden: bool,
    pub favorite: bool,
    pub tags: Vec<String>,
    pub removed_at: String,
    pub source: Source,
    pub launch: Launch,
    pub desktop: Desktop,
    pub metadata: Metadata,
    pub modules: BTreeMap<String, toml::Table>,
    /// Parked data (lutris, repack pins…): kept verbatim so nothing is lost.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub kind: String,
    pub gog_id: String,
    pub dir: String,
    pub build_id: String,
    pub dlcs: Vec<String>,
    pub lutris_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Launch {
    pub backend: String,
    pub exe: String,
    pub args: Vec<String>,
    pub working_dir: String,
    pub prefix: String,
    pub proton: String,
    pub arch: String,
    pub esync: Option<bool>,
    pub fsync: Option<bool>,
    pub dll_overrides: BTreeMap<String, String>,
    pub env: BTreeMap<String, String>,
    pub pre_command: String,
    pub post_command: String,
    pub umu_id: String,
    pub store: String,
    pub mangohud: Option<bool>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Desktop {
    pub hide_cursor: Option<bool>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Metadata {
    pub rawg_id: u64,
    pub sgdb_id: u64,
    pub steam_appid: u64,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub genres: Vec<String>,
    pub summary: String,
    pub description: String,
    pub metacritic: u32,
    pub players: u32,
}

impl Default for Game {
    fn default() -> Self {
        Game {
            schema: 1,
            id: String::new(),
            title: String::new(),
            sort_title: String::new(),
            platform: "windows".into(),
            release_year: 0,
            hidden: false,
            favorite: false,
            tags: vec![],
            removed_at: String::new(),
            source: Source::default(),
            launch: Launch::default(),
            desktop: Desktop::default(),
            metadata: Metadata::default(),
            modules: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl Default for Source {
    fn default() -> Self {
        Source { kind: "manual".into(), gog_id: String::new(), dir: String::new(), build_id: String::new(), dlcs: vec![], lutris_slug: String::new() }
    }
}

impl Default for Launch {
    fn default() -> Self {
        Launch {
            backend: "proton".into(),
            exe: String::new(),
            args: vec![],
            working_dir: String::new(),
            prefix: String::new(),
            proton: String::new(),
            arch: "win64".into(),
            esync: None,
            fsync: None,
            dll_overrides: BTreeMap::new(),
            env: BTreeMap::new(),
            pre_command: String::new(),
            post_command: String::new(),
            umu_id: String::new(),
            store: String::new(),
            mangohud: None,
        }
    }
}

impl Game {
    pub fn new(title: &str) -> Game {
        Game { id: crate::slug::slug(title), title: title.to_string(), ..Default::default() }
    }

    pub fn dir(&self) -> PathBuf {
        paths::game_dir(&self.id)
    }
    pub fn toml_path(&self) -> PathBuf {
        self.dir().join("game.toml")
    }
    pub fn sessions_path(&self) -> PathBuf {
        self.dir().join("sessions.jsonl")
    }
    pub fn journal_dir(&self) -> PathBuf {
        self.dir().join("journal")
    }
    pub fn media_dir(&self) -> PathBuf {
        self.dir().join("media")
    }

    pub fn exe_path(&self) -> PathBuf {
        paths::expand(&self.launch.exe)
    }
    pub fn game_root(&self) -> PathBuf {
        if !self.source.dir.is_empty() {
            return paths::expand(&self.source.dir);
        }
        self.exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }
    pub fn working_dir(&self) -> PathBuf {
        if !self.launch.working_dir.is_empty() {
            return paths::expand(&self.launch.working_dir);
        }
        // Some titles exit cleanly in 1-2 s when launched from anywhere but the exe's own directory.
        self.exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_default()
    }

    pub fn load(path: &Path) -> crate::Result<Game> {
        let s = std::fs::read_to_string(path)?;
        let mut g: Game = toml::from_str(&s)?;
        if g.id.is_empty() {
            g.id = path.parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into()).unwrap_or_default();
        }
        Ok(g)
    }

    pub fn load_id(id: &str) -> crate::Result<Game> {
        let p = paths::game_dir(id).join("game.toml");
        if !p.exists() {
            return Err(crate::Error::NotFound(id.into()));
        }
        Self::load(&p)
    }

    pub fn save(&self) -> crate::Result<()> {
        paths::ensure_dir(&self.dir())?;
        let text = toml::to_string_pretty(self).map_err(|e| crate::Error::Invalid(e.to_string()))?;
        atomic_write(&self.toml_path(), text.as_bytes())
    }

    pub fn is_installed(&self) -> bool {
        !self.launch.exe.is_empty() && self.exe_path().exists()
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn parse_value(value: &str) -> toml_edit::Value {
    if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        let mut arr = toml_edit::Array::new();
        for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            arr.push(item.trim_matches('"'));
        }
        return toml_edit::Value::Array(arr);
    }
    match value {
        "true" => return true.into(),
        "false" => return false.into(),
        _ => {}
    }
    if let Ok(i) = value.parse::<i64>() {
        return i.into();
    }
    if let Ok(f) = value.parse::<f64>() {
        if value.contains('.') {
            return f.into();
        }
    }
    value.into()
}

/// Writes `a.b.c = value` into a TOML document; "" removes the key; lists as "a,b" for known list keys or "[a,b]".
pub fn set_dotted(doc: &mut toml_edit::DocumentMut, key: &str, value: &str) -> crate::Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err(crate::Error::Invalid(format!("bad key {key}")));
    }
    let list_keys = ["tags", "args", "dlcs", "enabled"];
    let mut table: &mut toml_edit::Table = doc.as_table_mut();
    for p in &parts[..parts.len() - 1] {
        if !table.contains_key(p) || !table[p].is_table() {
            let mut t = toml_edit::Table::new();
            t.set_implicit(true);
            table[p] = toml_edit::Item::Table(t);
        }
        table = table[p].as_table_mut().unwrap();
    }
    let last = parts[parts.len() - 1];
    if value.is_empty() {
        table.remove(last);
        return Ok(());
    }
    let v = if list_keys.contains(&last) && !value.starts_with('[') {
        parse_value(&format!("[{value}]"))
    } else {
        parse_value(value)
    };
    table[last] = toml_edit::value(v);
    Ok(())
}

/// Validates a Set key: core keys are typed against `Game`; unknown top-level keys are refused.
pub fn set_key(game_toml: &Path, key: &str, value: &str) -> crate::Result<Game> {
    let text = std::fs::read_to_string(game_toml)?;
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| crate::Error::Invalid(e.to_string()))?;
    let top = key.split('.').next().unwrap_or("");
    let allowed = ["title", "sort_title", "platform", "release_year", "hidden", "favorite", "tags", "source", "launch", "desktop", "metadata", "modules"];
    if !allowed.contains(&top) {
        return Err(crate::Error::Invalid(format!("unknown key {key}")));
    }
    if top == "id" || top == "schema" {
        return Err(crate::Error::Invalid("immutable key".into()));
    }
    set_dotted(&mut doc, key, value)?;
    let g: Game = toml::from_str(&doc.to_string())?;
    atomic_write(game_toml, doc.to_string().as_bytes())?;
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema = 1
id = "the-technomancer"
title = "The Technomancer"
platform = "windows"
release_year = 2016

[source]
kind = "gog"
gog_id = "1972906591"
dir = "/mnt/games/PC/The Technomancer"
build_id = "52654527801265271"

[launch]
backend = "proton"
exe = "/mnt/games/PC/The Technomancer/TheTechnomancer.exe"
prefix = "/mnt/games/gog/the-technomancer"
proton = "proton-ge"
env = { WINE_CPU_TOPOLOGY = "4:0,1,2,3" }
mangohud = true

[desktop]
hide_cursor = true

[modules.capture]
enabled = true
cursor = false

[lutris]
configpath = "the-technomancer-1780794348"
"#;

    #[test]
    fn parses_plan_sample() {
        let g: Game = toml::from_str(SAMPLE).unwrap();
        assert_eq!(g.id, "the-technomancer");
        assert_eq!(g.source.kind, "gog");
        assert_eq!(g.launch.env["WINE_CPU_TOPOLOGY"], "4:0,1,2,3");
        assert_eq!(g.launch.mangohud, Some(true));
        assert_eq!(g.launch.esync, None);
        assert_eq!(g.modules["capture"]["cursor"].as_bool(), Some(false));
        assert!(g.extra.contains_key("lutris"));
        let back = toml::to_string_pretty(&g).unwrap();
        let g2: Game = toml::from_str(&back).unwrap();
        assert_eq!(g, g2);
    }

    #[test]
    fn set_dotted_edits() {
        let mut doc: toml_edit::DocumentMut = SAMPLE.parse().unwrap();
        set_dotted(&mut doc, "launch.proton", "proton-em").unwrap();
        set_dotted(&mut doc, "modules.capture.cursor", "true").unwrap();
        set_dotted(&mut doc, "tags", "rpg,indie").unwrap();
        set_dotted(&mut doc, "launch.env.FOO", "bar").unwrap();
        set_dotted(&mut doc, "launch.mangohud", "").unwrap();
        let g: Game = toml::from_str(&doc.to_string()).unwrap();
        assert_eq!(g.launch.proton, "proton-em");
        assert_eq!(g.modules["capture"]["cursor"].as_bool(), Some(true));
        assert_eq!(g.tags, vec!["rpg", "indie"]);
        assert_eq!(g.launch.env["FOO"], "bar");
        assert_eq!(g.launch.mangohud, None);
        assert!(doc.to_string().contains("gog_id = \"1972906591\""));
    }
}
