use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::Config;
use crate::modules::{self, Requires, Setting};
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Manifest {
    pub api: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub exe: String,
    pub requires: Requires,
    pub settings: Vec<Setting>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub enabled: bool,
    pub available: bool,
    pub missing: Vec<String>,
}

impl Source {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn name(&self) -> &str {
        if self.manifest.name.is_empty() { &self.manifest.id } else { &self.manifest.name }
    }
    pub fn data_dir(&self) -> PathBuf {
        paths::sources_data_dir(self.id())
    }
    pub fn active(&self) -> bool {
        self.enabled && self.available
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id(),
            "name": self.name(),
            "version": self.manifest.version,
            "dir": self.dir,
            "enabled": self.enabled,
            "available": self.available,
            "missing": self.missing,
            "settings": self.manifest.settings.iter().map(modules::setting_json).collect::<Vec<_>>(),
        })
    }

    pub fn merged_settings(&self, config: &Config) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        for s in &self.manifest.settings {
            out.insert(s.key.clone(), modules::toml_to_json(&s.default));
        }
        if out.get("games_dir").and_then(|v| v.as_str()) == Some("") {
            out.insert("games_dir".into(), serde_json::Value::String(config.games_root().to_string_lossy().into()));
        }
        for (k, v) in config.sources.settings.get(self.id()).into_iter().flatten() {
            out.insert(k.clone(), modules::toml_to_json(v));
        }
        out
    }

    pub fn validate_setting(&self, key: &str, value: &str) -> crate::Result<()> {
        let s = self.manifest.settings.iter().find(|s| s.key == key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown setting {key}", self.id())))?;
        modules::validate_value(key, &s.kind, &s.choices, value)
    }
}

/// User sources override system sources on the same id.
pub fn discover(config: &Config) -> Vec<Source> {
    let roots = paths::system_source_dirs().into_iter().rev().chain([paths::user_sources_dir()]);
    modules::read_manifests::<Manifest>(roots, "source.toml", |m| (&m.id, m.api))
        .into_values()
        .filter(|(dir, m)| {
            if m.exe.is_empty() {
                tracing::warn!("{}: source.toml without exe", dir.display());
            }
            !m.exe.is_empty()
        })
        .map(|(dir, m)| {
            let missing = modules::missing_bins(&m.requires);
            let enabled = config.sources.enabled.iter().any(|e| e == &m.id);
            Source { available: missing.is_empty(), missing, enabled, dir, manifest: m }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SourceEvent {
    LoginUrl { url: String },
    LoggedIn { #[serde(default)] user: String },
    Game(serde_json::Map<String, serde_json::Value>),
    Progress { #[serde(default)] done: u64, #[serde(default)] total: u64, #[serde(default)] message: String },
    Info { data: serde_json::Value },
    Update(serde_json::Map<String, serde_json::Value>),
    Done,
    #[serde(other)]
    Unknown,
}

pub async fn run<F>(source: &Source, settings: &serde_json::Map<String, serde_json::Value>, verb: &str, args: &[String], mut on_event: F) -> crate::Result<()>
where
    F: FnMut(SourceEvent),
{
    use tokio::io::AsyncBufReadExt;
    let exe = source.dir.join(&source.manifest.exe);
    let mut child = modules::command(&exe, &source.dir, &source.data_dir(), "SOURCE")?.arg(verb).args(args).env("SOURCE_SETTINGS_JSON", serde_json::Value::Object(settings.clone()).to_string()).env("UNIVERSE_BIN", paths::self_exe()).spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let id = source.id().to_string();
    let err_task = tokio::spawn(async move {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        let mut last = String::new();
        while let Ok(Some(l)) = lines.next_line().await {
            tracing::info!("source {id}: {l}");
            last = l;
        }
        last
    });
    let mut lines = tokio::io::BufReader::new(stdout).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<SourceEvent>(&line) {
            Ok(ev) => on_event(ev),
            Err(e) => tracing::warn!("source {}: bad event {e}: {line}", source.id()),
        }
    }
    let status = child.wait().await?;
    let last_err = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(crate::Error::Io(format!("{} {verb} failed ({}): {last_err}", source.id(), status.code().unwrap_or(-1))));
    }
    Ok(())
}

pub async fn setting_choices(source: &Source, settings: &serde_json::Map<String, serde_json::Value>, key: &str) -> crate::Result<Vec<String>> {
    modules::run_choices(&source.manifest.settings, &source.dir, &source.data_dir(), "SOURCE", source.id(), settings, key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_settings_and_validation() {
        let m: Manifest = toml::from_str(r#"
api = 2
id = "gog"
name = "GOG"
exe = "bin/source"
[requires]
bins = ["definitely-missing-binary-xyz"]
[[settings]]
key = "games_dir"
type = "path"
default = ""
[[settings]]
key = "platform"
type = "enum"
default = "windows"
choices = ["windows", "linux"]
"#).unwrap();
        let source = Source { available: false, missing: vec!["x".into()], enabled: true, dir: PathBuf::from("/s"), manifest: m };
        let cfg: Config = toml::from_str("[paths]\ngames_root = \"/mnt/games\"\n[sources.gog]\nplatform = \"linux\"").unwrap();
        let merged = source.merged_settings(&cfg);
        assert_eq!(merged["games_dir"], "/mnt/games", "an empty games_dir is paths.games_root");
        assert_eq!(merged["platform"], "linux");
        assert!(source.validate_setting("platform", "mac").is_err());
        assert!(source.validate_setting("platform", "linux").is_ok());
        assert!(source.validate_setting("nope", "1").is_err());
        let j = source.to_json();
        assert_eq!(j["name"], "GOG");
        assert_eq!(j["settings"][1]["choices"][1], "linux");
        assert_eq!(j["settings"][0]["scope"], "global");
    }

    #[test]
    fn discover_reads_source_toml_and_skips_old_manifests() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        for (id, body) in [("gog", "api = 2\nid = \"gog\"\nexe = \"bin/source\"\n"), ("old", "api = 1\nid = \"old\"\nexe = \"bin/source\"\n"), ("noexe", "api = 2\nid = \"noexe\"\n")] {
            std::fs::create_dir_all(dir.path().join(id)).unwrap();
            std::fs::write(dir.path().join(id).join("source.toml"), body).unwrap();
        }
        std::env::set_var("UNIVERSE_SOURCES_PATH", dir.path());
        std::env::set_var("UNIVERSE_CONFIG_HOME", dir.path().join("config"));
        let cfg: Config = toml::from_str("[sources]\nenabled = [\"gog\"]").unwrap();
        let found = discover(&cfg);
        std::env::remove_var("UNIVERSE_SOURCES_PATH");
        std::env::remove_var("UNIVERSE_CONFIG_HOME");
        assert_eq!(found.iter().map(|s| s.id()).collect::<Vec<_>>(), vec!["gog"]);
        assert!(found[0].enabled);
    }
}
