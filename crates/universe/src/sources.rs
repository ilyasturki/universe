use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::config::Config;
use crate::game::Game;
use crate::modules::{self, Limits, Requires, Setting};
use crate::paths;

/// The hooks a source may declare: the session ones run for the games it installed or found, `check` from doctor.
pub const HOOKS: [&str; 7] = ["pre-launch", "post-launch", "freeze", "thaw", "session-end", "post-process", "check"];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Manifest {
    pub api: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub exe: String,
    pub requires: Requires,
    /// Optional verbs past the required ones: `achievements`.
    pub capabilities: Vec<String>,
    pub hooks: BTreeMap<String, toml::Value>,
    pub limits: Limits,
    pub settings: Vec<Setting>,
    pub login: Login,
    /// Programs Universe fetches for the source when they are not on PATH, as catalogue entries; the catalogue's own win.
    pub tools: Vec<Tool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    #[serde(flatten)]
    pub entry: crate::components::Entry,
}

/// How a frontend words the store's sign-in: `kind` is `code` (the page shows a code once signed in) or `key` (the
/// page makes an API key), `hint` how to get it, `purpose` what signing in is for.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Login {
    pub kind: String,
    pub hint: String,
    pub purpose: String,
}

impl Login {
    fn to_json(&self) -> serde_json::Value {
        let key = self.kind == "key";
        let or = |own: &str, default: &str| if own.is_empty() { default.to_string() } else { own.to_string() };
        serde_json::json!({
            "kind": if key { "key" } else { "code" },
            "hint": or(&self.hint, if key { "Open the link, make a key, then enter it." } else { "Open the link, sign in, then enter the code it shows." }),
            "purpose": or(&self.purpose, "install games"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Source {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub enabled: bool,
    pub available: bool,
    pub missing: Vec<String>,
    /// Why `[requires] core` rules out this Universe; empty when it fits.
    pub incompatible: String,
}

impl Source {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn name(&self) -> &str {
        if self.manifest.name.is_empty() {
            &self.manifest.id
        } else {
            &self.manifest.name
        }
    }
    pub fn data_dir(&self) -> PathBuf {
        paths::sources_data_dir(self.id())
    }
    pub fn active(&self) -> bool {
        self.enabled && self.available
    }
    /// Why the source is unavailable: the Universe its manifest asks for, else the binaries it misses.
    pub fn unavailable(&self) -> String {
        modules::unavailable(&self.incompatible, &self.missing)
    }
    pub fn can(&self, capability: &str) -> bool {
        self.manifest.capabilities.iter().any(|c| c == capability)
    }
    pub fn hook(&self, name: &str) -> Option<PathBuf> {
        HOOKS.contains(&name).then(|| self.manifest.hooks.get(name).and_then(|v| v.as_str()).map(|p| self.dir.join(p))).flatten()
    }
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.manifest.hooks.get("timeout_s").and_then(|v| v.as_integer()).unwrap_or(20).max(1) as u64)
    }

    pub fn to_json(&self) -> serde_json::Value {
        let hooks: BTreeMap<&str, &str> =
            self.manifest.hooks.iter().filter(|(k, _)| HOOKS.contains(&k.as_str())).map(|(k, v)| (k.as_str(), v.as_str().unwrap_or(""))).collect();
        serde_json::json!({
            "id": self.id(),
            "name": self.name(),
            "version": self.manifest.version,
            "description": self.manifest.description,
            "dir": self.dir,
            "enabled": self.enabled,
            "available": self.available,
            "missing": self.missing,
            "incompatible": self.incompatible,
            "capabilities": self.manifest.capabilities,
            "hooks": hooks,
            "settings": self.manifest.settings.iter().map(modules::setting_json).collect::<Vec<_>>(),
            "login": self.manifest.login.to_json(),
        })
    }

    /// Defaults ← config.toml [sources.<id>] ← game.toml [sources.<id>] (game-scope keys only).
    pub fn merged_settings(&self, config: &Config, game: Option<&Game>) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        for s in &self.manifest.settings {
            out.insert(s.key.clone(), default_value(&s.key, &modules::toml_to_json(&s.default), config));
        }
        for (k, v) in config.sources.settings.get(self.id()).into_iter().flatten() {
            out.insert(k.clone(), modules::toml_to_json(v));
        }
        let game_keys = self.manifest.settings.iter().filter(|s| s.scope == "game").map(|s| s.key.as_str()).collect::<Vec<_>>();
        for (k, v) in game.and_then(|g| g.sources.get(self.id())).into_iter().flatten().filter(|(k, _)| game_keys.contains(&k.as_str())) {
            out.insert(k.clone(), modules::toml_to_json(v));
        }
        out
    }

    pub fn validate_setting(&self, key: &str, value: &str, scope_game: bool) -> crate::Result<()> {
        let s = self.manifest.settings.iter().find(|s| s.key == key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown setting {key}", self.id())))?;
        if scope_game && s.scope != "game" {
            return Err(crate::Error::Invalid(format!("{}.{key} is a global setting", self.id())));
        }
        modules::validate_value(key, &s.kind, &s.choices, value)
    }
}

/// A setting's value with nothing set: the manifest's default, an empty `games_dir` the config's games folder.
pub fn default_value(key: &str, default: &serde_json::Value, config: &Config) -> serde_json::Value {
    if key == "games_dir" && default.as_str() == Some("") {
        return serde_json::Value::String(config.games_root().to_string_lossy().into());
    }
    default.clone()
}

pub fn discover(config: &Config) -> Vec<Source> {
    modules::read_manifests::<Manifest>(modules::roots("source", paths::system_source_dirs(), paths::user_sources_dir()), "source.toml", |m| &m.id)
        .into_values()
        .filter(|(dir, m)| {
            if m.exe.is_empty() {
                tracing::warn!("{}: source.toml without exe", dir.display());
            }
            !m.exe.is_empty()
        })
        .map(|(dir, m)| {
            // A disabled source's pins are left out of the catalogue: its own still keep it available.
            let pins = |bin: &String| m.tools.iter().any(|t| t.entry.bin == *bin && t.entry.latest().is_some());
            let missing: Vec<String> = modules::missing_bins(&m.requires, &m.id).into_iter().filter(|b| !pins(b)).collect();
            let incompatible = modules::incompatible(m.api, &m.requires);
            let enabled = config.sources.enabled.iter().any(|e| e == &m.id);
            Source { available: missing.is_empty() && incompatible.is_empty(), missing, incompatible, enabled, dir, manifest: m }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SourceEvent {
    LoginUrl {
        url: String,
    },
    LoggedIn {
        #[serde(default)]
        user: String,
    },
    Game(serde_json::Map<String, serde_json::Value>),
    Progress {
        #[serde(default)]
        done: u64,
        #[serde(default)]
        total: u64,
        #[serde(default)]
        message: String,
    },
    Info {
        data: serde_json::Value,
        #[serde(default)]
        download_size: Option<u64>,
        #[serde(default)]
        disk_size: Option<u64>,
    },
    Update(serde_json::Map<String, serde_json::Value>),
    Achievement(serde_json::Map<String, serde_json::Value>),
    Cloud(serde_json::Map<String, serde_json::Value>),
    Window {
        class: String,
        #[serde(default)]
        title: String,
    },
    Done,
    #[serde(other)]
    Unknown,
}

/// `spawned` gets the process id as soon as there is one, so a `cancel` can SIGTERM it; `env` is a game's hook environment, for a verb about one game.
pub async fn run<F>(
    source: &Source,
    settings: &serde_json::Map<String, serde_json::Value>,
    verb: &str,
    args: &[String],
    env: &[(String, String)],
    spawned: impl FnOnce(u32),
    mut on_event: F,
) -> crate::Result<()>
where
    F: FnMut(SourceEvent),
{
    use tokio::io::AsyncBufReadExt;
    let exe = source.dir.join(&source.manifest.exe);
    let mut child = modules::command(&exe, &source.dir, &source.data_dir(), "SOURCE")?
        .arg(verb)
        .args(args)
        .envs(env.iter().map(|(k, v)| (k, v)))
        .env("SOURCE_SETTINGS_JSON", serde_json::Value::Object(settings.clone()).to_string())
        .env("UNIVERSE_BIN", paths::self_exe())
        .spawn()
        .map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    if let Some(pid) = child.id() {
        spawned(pid);
    }
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
        let m: Manifest = toml::from_str(
            r#"
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
"#,
        )
        .unwrap();
        let source = Source { available: false, missing: vec!["x".into()], incompatible: String::new(), enabled: true, dir: PathBuf::from("/s"), manifest: m };
        let cfg: Config = toml::from_str("[paths]\ngames_root = \"/mnt/games\"\n[sources.gog]\nplatform = \"linux\"").unwrap();
        let merged = source.merged_settings(&cfg, None);
        assert_eq!(merged["games_dir"], "/mnt/games", "an empty games_dir is paths.games_root");
        assert_eq!(merged["platform"], "linux");
        assert!(source.validate_setting("platform", "mac", false).is_err());
        assert!(source.validate_setting("platform", "linux", false).is_ok());
        assert!(source.validate_setting("nope", "1", false).is_err());
        let j = source.to_json();
        assert_eq!(j["name"], "GOG");
        assert_eq!(j["settings"][1]["choices"][1], "linux");
        assert_eq!(j["settings"][0]["scope"], "global");
        assert_eq!((&j["login"]["kind"], &j["login"]["purpose"]), (&serde_json::json!("code"), &serde_json::json!("install games")), "no [login]: a code");
        assert!(j["login"]["hint"].as_str().unwrap().contains("code"));
    }

    #[test]
    fn a_login_table_words_an_api_key() {
        let m: Manifest = toml::from_str("api = 2\nid = \"steam\"\n[login]\nkind = \"key\"\npurpose = \"list the games you own\"\n").unwrap();
        let login = m.login.to_json();
        assert_eq!((&login["kind"], &login["purpose"]), (&serde_json::json!("key"), &serde_json::json!("list the games you own")));
        assert!(login["hint"].as_str().unwrap().contains("key"), "the key's own default hint");
    }

    #[test]
    fn discover_reads_source_toml_and_needs_an_exe() {
        let env = crate::paths::test_env();
        for (id, body) in [("gog", "api = 2\nid = \"gog\"\nexe = \"bin/source\"\n"), ("noexe", "api = 2\nid = \"noexe\"\n")] {
            std::fs::create_dir_all(env.path().join("sources").join(id)).unwrap();
            std::fs::write(env.path().join("sources").join(id).join("source.toml"), body).unwrap();
        }
        let cfg: Config = toml::from_str("[sources]\nenabled = [\"gog\"]").unwrap();
        let found = discover(&cfg);
        assert_eq!(found.iter().map(|s| s.id()).collect::<Vec<_>>(), vec!["gog"]);
        assert!(found[0].enabled);
    }
}
