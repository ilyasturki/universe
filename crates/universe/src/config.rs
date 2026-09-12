use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema: u32,
    pub paths: Paths,
    pub launch: LaunchDefaults,
    pub desktop: DesktopConfig,
    pub proton: BTreeMap<String, String>,
    pub modules: ModulesConfig,
    pub keys: Keys,
    pub lutris: LutrisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Paths {
    pub games_root: String,
    pub prefixes_root: String,
    pub recordings_root: String,
    pub journal_root: String,
    pub overrides: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchDefaults {
    pub proton: String,
    pub esync: bool,
    pub fsync: bool,
    pub mangohud: bool,
    pub env: BTreeMap<String, String>,
    pub umu_run: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopConfig {
    pub profile: String,
    pub hide_cursor: bool,
    pub cursor_extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModulesConfig {
    pub enabled: Vec<String>,
    #[serde(flatten)]
    pub settings: BTreeMap<String, toml::Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Keys {
    pub sgdb: String,
    pub rawg: String,
    pub sgdb_file: String,
    pub rawg_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LutrisConfig {
    pub config_dir: String,
    pub pga_db: String,
    pub runners_dir: String,
    pub pegasus_library: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            schema: 1,
            paths: Paths::default(),
            launch: LaunchDefaults::default(),
            desktop: DesktopConfig::default(),
            proton: BTreeMap::from([
                ("proton-ge".into(), "~/.local/share/lutris/runners/wine/proton-ge".into()),
                ("proton-em".into(), "~/.local/share/lutris/runners/wine/proton-em".into()),
                ("proton-cachyos".into(), "~/.local/share/lutris/runners/wine/proton-cachyos".into()),
            ]),
            modules: ModulesConfig::default(),
            keys: Keys::default(),
            lutris: LutrisConfig::default(),
        }
    }
}

impl Default for Paths {
    fn default() -> Self {
        Paths {
            games_root: paths::user_dir("GAMES", "Games").to_string_lossy().into(),
            prefixes_root: paths::data_home().join("prefixes").to_string_lossy().into(),
            recordings_root: paths::user_dir("VIDEOS", "Videos").join("universe").to_string_lossy().into(),
            journal_root: paths::user_dir("DOCUMENTS", "Documents").join("universe/journal").to_string_lossy().into(),
            overrides: paths::config_home().join("overrides").to_string_lossy().into(),
        }
    }
}

impl Default for LaunchDefaults {
    fn default() -> Self {
        LaunchDefaults {
            proton: "proton-ge".into(),
            esync: true,
            fsync: true,
            mangohud: true,
            env: BTreeMap::from([("PROTON_ENABLE_WAYLAND".into(), "1".into())]),
            umu_run: "umu-run".into(),
        }
    }
}

impl Default for DesktopConfig {
    fn default() -> Self {
        DesktopConfig {
            profile: "auto".into(),
            hide_cursor: true,
            cursor_extension: "hide-cursor@elcste.com".into(),
        }
    }
}

impl Default for ModulesConfig {
    fn default() -> Self {
        ModulesConfig {
            enabled: vec!["gog".into(), "capture".into(), "journal".into(), "tracker-md".into()],
            settings: BTreeMap::new(),
        }
    }
}

impl Default for Keys {
    fn default() -> Self {
        Keys {
            sgdb: String::new(),
            rawg: String::new(),
            sgdb_file: "~/.config/steamgriddb/api_key".into(),
            rawg_file: "~/.config/rawg/api_key".into(),
        }
    }
}

impl Default for LutrisConfig {
    fn default() -> Self {
        LutrisConfig {
            config_dir: "~/.config/lutris".into(),
            pga_db: "~/.local/share/lutris/pga.db".into(),
            runners_dir: "~/.local/share/lutris/runners/wine".into(),
            pegasus_library: "~/.local/share/pegasus-library".into(),
        }
    }
}

impl Config {
    pub fn load() -> crate::Result<Config> {
        Self::load_from(&paths::config_file())
    }

    pub fn load_from(path: &Path) -> crate::Result<Config> {
        match std::fs::read_to_string(path) {
            Ok(s) => Ok(toml::from_str(&s)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn recordings_root(&self) -> PathBuf {
        paths::expand(&self.paths.recordings_root)
    }
    pub fn journal_root(&self) -> PathBuf {
        paths::expand(&self.paths.journal_root)
    }
    pub fn overrides_dir(&self) -> PathBuf {
        paths::expand(&self.paths.overrides)
    }
    pub fn games_root(&self) -> PathBuf {
        paths::expand(&self.paths.games_root)
    }
    pub fn prefixes_root(&self) -> PathBuf {
        paths::expand(&self.paths.prefixes_root)
    }

    /// Resolves a Proton name (or path) to a directory; proton/<name> under data_home wins, then [proton], then a path.
    pub fn proton_path(&self, name: &str) -> Option<PathBuf> {
        let linked = paths::data_home().join("proton").join(name);
        if linked.exists() {
            return Some(std::fs::canonicalize(&linked).unwrap_or(linked));
        }
        if let Some(p) = self.proton.get(name) {
            let p = paths::expand(p);
            if p.exists() {
                return Some(std::fs::canonicalize(&p).unwrap_or(p));
            }
        }
        let p = paths::expand(name);
        if p.is_absolute() && p.exists() {
            return Some(p);
        }
        let lutris = paths::expand(&self.lutris.runners_dir).join(name);
        if lutris.exists() {
            return Some(std::fs::canonicalize(&lutris).unwrap_or(lutris));
        }
        let steam = paths::home().join(".local/share/Steam/compatibilitytools.d").join(name);
        if steam.exists() {
            return Some(steam);
        }
        None
    }

    pub fn api_key(&self, which: &str) -> Option<String> {
        let (inline, file) = match which {
            "sgdb" => (&self.keys.sgdb, &self.keys.sgdb_file),
            "rawg" => (&self.keys.rawg, &self.keys.rawg_file),
            _ => return None,
        };
        if !inline.is_empty() {
            return Some(inline.clone());
        }
        std::fs::read_to_string(paths::expand(file))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        if let Some(p) = v.get_mut("paths").and_then(|p| p.as_object_mut()) {
            for (_, val) in p.iter_mut() {
                if let Some(s) = val.as_str() {
                    *val = serde_json::Value::String(paths::expand(s).to_string_lossy().into());
                }
            }
        }
        v["config_file"] = serde_json::Value::String(paths::config_file().to_string_lossy().into());
        v["data_home"] = serde_json::Value::String(paths::data_home().to_string_lossy().into());
        v
    }

    /// Dotted-key write into config.toml, preserving the rest of the file.
    pub fn set_key(path: &Path, key: &str, value: &str) -> crate::Result<()> {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| crate::Error::Invalid(e.to_string()))?;
        crate::game::set_dotted(&mut doc, key, value)?;
        if let Some(parent) = path.parent() {
            paths::ensure_dir(parent)?;
        }
        std::fs::write(path, doc.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_and_override() {
        let c: Config = toml::from_str("[launch]\nproton = \"proton-em\"\n[modules]\nenabled = [\"gog\"]\n[modules.capture]\ncodec = \"hevc\"\n").unwrap();
        assert_eq!(c.launch.proton, "proton-em");
        assert!(c.launch.esync);
        assert_eq!(c.modules.enabled, vec!["gog"]);
        assert_eq!(c.modules.settings["capture"]["codec"].as_str(), Some("hevc"));
        assert!(c.paths.games_root.ends_with("/Games"), "{}", c.paths.games_root);
        assert!(c.paths.prefixes_root.ends_with("/prefixes"));
    }
}
