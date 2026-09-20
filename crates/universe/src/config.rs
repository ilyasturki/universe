use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;

/// A switch left to detection unless set: `auto`, `on` or `off`; a bool reads as on/off, and it is written as the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Toggle {
    #[default]
    Auto,
    On,
    Off,
}

impl Toggle {
    pub const CHOICES: [&'static str; 3] = ["auto", "on", "off"];

    pub fn parse(s: &str) -> Option<Toggle> {
        match s.trim() {
            "auto" => Some(Toggle::Auto),
            "on" | "true" => Some(Toggle::On),
            "off" | "false" => Some(Toggle::Off),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Toggle::Auto => "auto",
            Toggle::On => "on",
            Toggle::Off => "off",
        }
    }

    pub fn or(self, auto: impl FnOnce() -> bool) -> bool {
        match self {
            Toggle::Auto => auto(),
            Toggle::On => true,
            Toggle::Off => false,
        }
    }
}

impl From<bool> for Toggle {
    fn from(on: bool) -> Toggle {
        if on { Toggle::On } else { Toggle::Off }
    }
}

impl Serialize for Toggle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Toggle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Toggle, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Bool(b) => Ok(b.into()),
            Raw::Text(t) => Toggle::parse(&t).ok_or_else(|| serde::de::Error::custom(format!("expected auto, on or off, not '{t}'"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema: u32,
    pub paths: Paths,
    pub launch: LaunchDefaults,
    pub desktop: DesktopConfig,
    pub proton: BTreeMap<String, String>,
    /// [runners.<id>]: `exe`, `args`, and the runner's options.
    pub runners: BTreeMap<String, toml::Table>,
    pub modules: ModulesConfig,
    pub sources: SourcesConfig,
    pub keys: Keys,
    pub lutris: LutrisConfig,
    pub controller: crate::controller::ControllerConfig,
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
    pub ntsync: bool,
    pub wayland: bool,
    pub hdr: bool,
    pub dlss_upgrade: Toggle,
    pub fsr4_upgrade: Toggle,
    pub xess_upgrade: Toggle,
    pub optiscaler: bool,
    pub mangohud: bool,
    pub pause_on_home: bool,
    pub gamescope: bool,
    pub gamescope_args: String,
    pub gamescope_bin: String,
    pub gamescope_resolution: String,
    pub gamescope_refresh: String,
    pub gamescope_scaler: String,
    pub gamescope_filter: String,
    pub gamescope_sharpness: Option<u32>,
    pub gamescope_adaptive_sync: Toggle,
    pub fps_limit: String,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModulesConfig {
    pub enabled: Vec<String>,
    #[serde(flatten)]
    pub settings: BTreeMap<String, toml::Table>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesConfig {
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
            runners: BTreeMap::new(),
            modules: ModulesConfig::default(),
            sources: SourcesConfig::default(),
            keys: Keys::default(),
            lutris: LutrisConfig::default(),
            controller: crate::controller::ControllerConfig::default(),
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
            ntsync: true,
            wayland: true,
            hdr: false,
            dlss_upgrade: Toggle::Off,
            fsr4_upgrade: Toggle::Off,
            xess_upgrade: Toggle::Off,
            optiscaler: false,
            mangohud: false,
            pause_on_home: true,
            gamescope: true,
            gamescope_args: String::new(),
            gamescope_bin: "gamescope".into(),
            gamescope_resolution: "auto".into(),
            gamescope_refresh: "auto".into(),
            gamescope_scaler: String::new(),
            gamescope_filter: String::new(),
            gamescope_sharpness: None,
            gamescope_adaptive_sync: Toggle::Auto,
            fps_limit: "auto".into(),
            env: BTreeMap::new(),
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

impl Default for SourcesConfig {
    fn default() -> Self {
        SourcesConfig {
            enabled: vec!["gog".into()],
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

/// The newest directory across `dirs` whose name is a build of `name`'s family: the name's words in either
/// order, letters only, followed by a version (`proton-ge` ↔ `GE-Proton10-4`, `proton-cachyos-10.0-20250101`).
fn newest_of_family(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    let words: Vec<String> = name.split(['-', '_']).map(|w| w.to_ascii_lowercase()).filter(|w| !w.is_empty()).collect();
    if words.is_empty() || words.iter().any(|w| w.chars().any(|c| !c.is_ascii_alphabetic())) {
        return None;
    }
    let mut prefixes = vec![words.concat()];
    prefixes.push(words.iter().rev().map(String::as_str).collect());
    let is_build = |dir: &str| {
        let flat: String = dir.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        prefixes.iter().any(|p| flat.strip_prefix(p.as_str()).is_some_and(|rest| rest.chars().next().is_none_or(|c| c.is_ascii_digit())))
    };
    dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| is_build(&n.to_string_lossy())))
        .max_by(|a, b| version_key(a).cmp(&version_key(b)))
}

/// A name split into text and number runs, so `GE-Proton10-4` sorts above `GE-Proton9-27`.
fn version_key(p: &Path) -> Vec<(u64, String)> {
    let name = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let mut out = Vec::new();
    let mut chars = name.chars().peekable();
    while let Some(&c) = chars.peek() {
        let mut run = String::new();
        let digit = c.is_ascii_digit();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() != digit {
                break;
            }
            run.push(c);
            chars.next();
        }
        out.push(if digit { (run.parse().unwrap_or(u64::MAX), String::new()) } else { (0, run) });
    }
    out
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

    /// Resolves a Proton name (or path) to a directory; proton/<name> under data_home wins, then [proton], then a path,
    /// then the name under each tool directory, then the newest build of the name's family in any of them
    /// (`proton-ge` is Steam's and Heroic's `GE-Proton10-4`).
    pub fn proton_path(&self, name: &str) -> Option<PathBuf> {
        let own = paths::expand(name);
        let dirs = self.proton_dirs();
        let candidates = [
            Some(paths::data_home().join("proton").join(name)),
            self.proton.get(name).map(|p| paths::expand(p)),
            own.is_absolute().then_some(own),
        ];
        let p = candidates
            .into_iter()
            .flatten()
            .chain(dirs.iter().map(|d| d.join(name)))
            .find(|p| p.exists())
            .or_else(|| newest_of_family(&dirs, name))?;
        Some(std::fs::canonicalize(&p).unwrap_or(p))
    }

    /// Where Lutris, Steam and Heroic keep their Proton builds, plus Universe's own.
    fn proton_dirs(&self) -> Vec<PathBuf> {
        let home = paths::home();
        vec![
            paths::data_home().join("proton"),
            paths::expand(&self.lutris.runners_dir),
            home.join(".local/share/Steam/compatibilitytools.d"),
            home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
            home.join(".config/heroic/tools/proton"),
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton"),
        ]
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
        v["config_writable"] = serde_json::Value::Bool(Self::writable(&paths::config_file()));
        v["data_home"] = serde_json::Value::String(paths::data_home().to_string_lossy().into());
        v
    }

    pub fn set_key(path: &Path, key: &str, value: &str) -> crate::Result<()> {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| crate::Error::Invalid(e.to_string()))?;
        crate::game::set_dotted(&mut doc, key, value)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, doc.to_string()).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => crate::Error::Invalid(format!(
                "{} is read-only: home-manager's programs.universe.settings owns it; set it to null to change settings here",
                path.display()
            )),
            _ => e.into(),
        })?;
        Ok(())
    }

    pub fn writable(path: &Path) -> bool {
        std::fs::metadata(path).map(|m| !m.permissions().readonly()).unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_parse_and_override() {
        let c: Config = toml::from_str("[launch]\nproton = \"proton-em\"\n[modules]\nenabled = [\"capture\"]\n[modules.capture]\ncodec = \"hevc\"\n[sources]\nenabled = []\n[sources.gog]\nplatform = \"linux\"\n").unwrap();
        assert_eq!(c.launch.proton, "proton-em");
        assert!(c.launch.esync);
        assert_eq!(c.modules.enabled, vec!["capture"]);
        assert_eq!(c.modules.settings["capture"]["codec"].as_str(), Some("hevc"));
        assert!(c.sources.enabled.is_empty());
        assert_eq!(c.sources.settings["gog"]["platform"].as_str(), Some("linux"));
        assert_eq!(Config::default().sources.enabled, vec!["gog"]);
        assert!(c.paths.games_root.ends_with("/Games"), "{}", c.paths.games_root);
        assert!(c.paths.prefixes_root.ends_with("/prefixes"));
    }

    #[test]
    fn nothing_runs_unasked_out_of_the_box() {
        let c = Config::default();
        assert!(c.modules.enabled.is_empty(), "a module is opt-in");
        assert!(!c.launch.mangohud, "the HUD stays hidden until shown");
        assert_eq!((c.launch.dlss_upgrade, c.launch.fsr4_upgrade, c.launch.xess_upgrade), (Toggle::Off, Toggle::Off, Toggle::Off), "a DLL swap is opt-in, as the vendors ship it");
        assert_eq!(c.launch.gamescope_adaptive_sync, Toggle::Auto);
    }

    #[test]
    fn a_toggle_reads_a_bool_or_a_word_and_writes_the_word() {
        let c: Config = toml::from_str("[launch]\ndlss_upgrade = true\nfsr4_upgrade = \"off\"\nxess_upgrade = \"auto\"\n").unwrap();
        assert_eq!((c.launch.dlss_upgrade, c.launch.fsr4_upgrade, c.launch.xess_upgrade), (Toggle::On, Toggle::Off, Toggle::Auto));
        let text = toml::to_string(&c.launch).unwrap();
        assert!(text.contains("dlss_upgrade = \"on\"\n") && text.contains("fsr4_upgrade = \"off\"\n") && text.contains("xess_upgrade = \"auto\"\n"), "{text}");
        assert!(toml::from_str::<Config>("[launch]\ndlss_upgrade = \"maybe\"\n").is_err());
        assert!(Toggle::On.or(|| false) && !Toggle::Off.or(|| true) && Toggle::Auto.or(|| true));
    }

    #[test]
    fn a_proton_name_finds_the_newest_build_of_its_family() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["GE-Proton9-27", "GE-Proton10-4", "GE-Proton10-12", "proton-cachyos-10.0-20250901", "Proton 10.0", "gecko"] {
            std::fs::create_dir(dir.path().join(d)).unwrap();
        }
        std::fs::write(dir.path().join("GE-Proton11-1"), b"a file, not a build").unwrap();
        let dirs = vec![dir.path().to_path_buf()];
        assert_eq!(newest_of_family(&dirs, "proton-ge").unwrap(), dir.path().join("GE-Proton10-12"), "numbers sort as numbers");
        assert_eq!(newest_of_family(&dirs, "proton-cachyos").unwrap(), dir.path().join("proton-cachyos-10.0-20250901"));
        assert!(newest_of_family(&dirs, "proton-em").is_none());
        assert!(newest_of_family(&dirs, "proton-gecko").is_none(), "a family is followed by its version");
        assert!(newest_of_family(&dirs, "/opt/proton").is_none(), "a path is no family");
    }
}
