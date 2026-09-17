use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::config::Config;
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Manifest {
    pub api: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub requires: Requires,
    pub hooks: BTreeMap<String, toml::Value>,
    pub limits: Limits,
    pub settings: Vec<Setting>,
}

/// The manifest api both module.toml and source.toml speak; an older one is left out with a warning.
pub const API: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Requires {
    pub core: String,
    pub bins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Limits {
    pub cpu_weight: u32,
    pub memory_high: String,
}
impl Default for Limits {
    fn default() -> Self {
        Limits { cpu_weight: 20, memory_high: "2G".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Setting {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: toml::Value,
    pub label: String,
    pub scope: String,
    pub choices: Vec<String>,
    pub choices_exec: String,
}
impl Default for Setting {
    fn default() -> Self {
        Setting { key: String::new(), kind: "string".into(), default: toml::Value::String(String::new()), label: String::new(), scope: "global".into(), choices: vec![], choices_exec: String::new() }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Module {
    pub manifest: Manifest,
    pub dir: PathBuf,
    pub enabled: bool,
    pub available: bool,
    pub missing: Vec<String>,
}

pub const HOOKS: [&str; 7] = ["pre-launch", "post-launch", "freeze", "thaw", "session-end", "post-process", "screenshot"];

impl Module {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn hook(&self, name: &str) -> Option<PathBuf> {
        self.manifest.hooks.get(name).and_then(|v| v.as_str()).map(|p| self.dir.join(p))
    }
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.manifest.hooks.get("timeout_s").and_then(|v| v.as_integer()).unwrap_or(20).max(1) as u64)
    }
    pub fn data_dir(&self) -> PathBuf {
        paths::modules_data_dir(self.id())
    }
    pub fn active(&self) -> bool {
        self.enabled && self.available
    }

    pub fn to_json(&self) -> serde_json::Value {
        let m = &self.manifest;
        let hooks: BTreeMap<String, String> = m
            .hooks
            .iter()
            .filter(|(k, _)| HOOKS.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
            .collect();
        serde_json::json!({
            "id": m.id,
            "name": if m.name.is_empty() { m.id.clone() } else { m.name.clone() },
            "version": m.version,
            "dir": self.dir,
            "enabled": self.enabled,
            "available": self.available,
            "missing": self.missing,
            "hooks": hooks,
            "settings": self.settings_json(),
        })
    }

    pub fn settings_json(&self) -> serde_json::Value {
        let mut list: Vec<serde_json::Value> = Vec::new();
        let has_enabled = self.manifest.settings.iter().any(|s| s.key == "enabled");
        if !has_enabled {
            list.push(serde_json::json!({"key": "enabled", "type": "bool", "default": true, "label": "Enable", "scope": "game", "choices": []}));
        }
        for s in &self.manifest.settings {
            let mut j = setting_json(s);
            j["scope"] = serde_json::Value::String(s.scope.clone());
            list.push(j);
        }
        serde_json::Value::Array(list)
    }

    /// Global defaults ← config.toml [modules.<id>] ← game.toml [modules.<id>].
    pub fn merged_settings(&self, config: &Config, game: Option<&crate::game::Game>) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        out.insert("enabled".into(), serde_json::Value::Bool(true));
        for s in &self.manifest.settings {
            out.insert(s.key.clone(), toml_to_json(&s.default));
        }
        for t in config.modules.settings.get(self.id()).into_iter().chain(game.and_then(|g| g.modules.get(self.id()))) {
            for (k, v) in t {
                out.insert(k.clone(), toml_to_json(v));
            }
        }
        // An earlier set wrote enabled as ["false"]; a hook would read that list as true.
        if let Some(b) = out.get("enabled").and_then(|v| v.as_array()).filter(|a| a.len() == 1).and_then(|a| a[0].as_str()).and_then(|s| s.parse::<bool>().ok()) {
            out.insert("enabled".into(), serde_json::Value::Bool(b));
        }
        out
    }

    pub fn validate_setting(&self, key: &str, value: &str, scope_game: bool) -> crate::Result<()> {
        let (kind, choices): (&str, &[String]) = match self.manifest.settings.iter().find(|s| s.key == key) {
            Some(s) if scope_game && s.scope != "game" => return Err(crate::Error::Invalid(format!("{}.{key} is a global setting", self.id()))),
            Some(s) => (&s.kind, &s.choices),
            None if key == "enabled" => ("bool", &[]),
            None => return Err(crate::Error::Invalid(format!("{}: unknown setting {key}", self.id()))),
        };
        validate_value(key, kind, choices, value)
    }
}

pub fn setting_json(s: &Setting) -> serde_json::Value {
    serde_json::json!({
        "key": s.key,
        "type": s.kind,
        "default": toml_to_json(&s.default),
        "label": s.label,
        "choices": s.choices,
        "dynamic": !s.choices_exec.is_empty(),
    })
}

pub fn validate_value(key: &str, kind: &str, choices: &[String], value: &str) -> crate::Result<()> {
    match kind {
        "bool" if !matches!(value, "true" | "false") => Err(crate::Error::Invalid(format!("{key} must be true or false"))),
        // A listed non-numeric choice is a named value ("auto") the module resolves itself.
        "int" if value.parse::<i64>().is_err() && !choices.iter().any(|c| c == value) => Err(crate::Error::Invalid(format!("{key} must be an integer"))),
        "enum" if !choices.iter().any(|c| c == value) => Err(crate::Error::Invalid(format!("{key} must be one of {}", choices.join(", ")))),
        _ => Ok(()),
    }
}

pub fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(i),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => serde_json::Value::Object(t.iter().map(|(k, v)| (k.clone(), toml_to_json(v))).collect()),
    }
}

/// `<root>/*/<file>` over the roots in order, a later root overriding an earlier one on the same id; a manifest of an older api is left out.
pub fn read_manifests<M: serde::de::DeserializeOwned>(roots: impl Iterator<Item = PathBuf>, file: &str, id_api: impl Fn(&M) -> (&str, u32)) -> BTreeMap<String, (PathBuf, M)> {
    let mut found = BTreeMap::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else { continue };
        for e in rd.flatten() {
            let dir = e.path();
            let mp = dir.join(file);
            if !mp.is_file() {
                continue;
            }
            match std::fs::read_to_string(&mp).map_err(crate::Error::from).and_then(|s| toml::from_str::<M>(&s).map_err(Into::into)) {
                Ok(m) => match id_api(&m) {
                    ("", _) => tracing::warn!("{}: manifest without id", mp.display()),
                    (_, api) if api != 0 && api < API => tracing::warn!("{}: api {api} manifest ignored; api {API} keeps modules and sources apart (sources/<id>/source.toml)", mp.display()),
                    (id, _) => {
                        found.insert(id.to_string(), (dir, m));
                    }
                },
                Err(err) => tracing::warn!("{}: {err}", mp.display()),
            }
        }
    }
    found
}

pub fn missing_bins(requires: &Requires) -> Vec<String> {
    requires.bins.iter().filter(|b| crate::runners::on_path(b).is_none()).cloned().collect()
}

/// User modules override system modules on the same id.
pub fn discover(config: &Config) -> Vec<Module> {
    let roots = paths::system_module_dirs().into_iter().rev().chain([paths::user_modules_dir()]);
    read_manifests::<Manifest>(roots, "module.toml", |m| (&m.id, m.api))
        .into_values()
        .map(|(dir, m)| {
            let missing = missing_bins(&m.requires);
            let enabled = config.modules.enabled.iter().any(|e| e == &m.id);
            Module { available: missing.is_empty(), missing, enabled, dir, manifest: m }
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
pub struct HookEnv {
    pub vars: Vec<(String, String)>,
}

impl HookEnv {
    pub fn set(&mut self, k: &str, v: impl Into<String>) {
        self.vars.retain(|(kk, _)| kk != k);
        self.vars.push((k.into(), v.into()));
    }
}

pub struct HookOutcome {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

/// An executable of a module or a source: run in its directory with `<PREFIX>_DIR` and `<PREFIX>_DATA_DIR`, stdout and stderr piped.
pub(crate) fn command(exe: &Path, dir: &Path, data_dir: &Path, prefix: &str) -> crate::Result<tokio::process::Command> {
    std::fs::create_dir_all(data_dir)?;
    let mut cmd = tokio::process::Command::new(exe);
    cmd.env(format!("{prefix}_DIR"), dir).env(format!("{prefix}_DATA_DIR"), data_dir).current_dir(dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    Ok(cmd)
}

fn module_cmd(module: &Module, exe: &Path) -> crate::Result<tokio::process::Command> {
    command(exe, &module.dir, &module.data_dir(), "MODULE")
}

pub async fn run_blocking(module: &Module, hook: &str, env: &HookEnv) -> crate::Result<HookOutcome> {
    let Some(exe) = module.hook(hook) else {
        return Ok(HookOutcome { status: 0, stdout: String::new(), stderr: String::new() });
    };
    let child = module_cmd(module, &exe)?.envs(env.vars.iter().map(|(k, v)| (k.as_str(), v.as_str()))).spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let timeout = module.timeout();
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => {
            let o = HookOutcome { status: out.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&out.stdout).into(), stderr: String::from_utf8_lossy(&out.stderr).into() };
            if o.status != 0 {
                tracing::warn!("hook {}:{hook} exited {}: {}", module.id(), o.status, o.stderr.trim());
            }
            Ok(o)
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(crate::Error::Io(format!("{}:{hook} timed out after {timeout:?}", module.id()))),
    }
}

/// A transient unit under the manifest limits, bound to the game's unit when `bind_to` names it; returns its name.
pub async fn run_async(units: &crate::host::Units, module: &Module, hook: &str, env: &HookEnv, session_id: &str, bind_to: Option<&str>) -> crate::Result<Option<String>> {
    let Some(exe) = module.hook(hook) else { return Ok(None) };
    std::fs::create_dir_all(module.data_dir())?;
    let mut unit_env: BTreeMap<String, String> = env.vars.iter().cloned().collect();
    unit_env.insert("MODULE_DIR".into(), module.dir.to_string_lossy().into());
    unit_env.insert("MODULE_DATA_DIR".into(), module.data_dir().to_string_lossy().into());
    let spec = crate::host::UnitSpec {
        name: format!("universe-{}-{}-{}", module.id(), hook, session_id),
        program: exe.to_string_lossy().into(),
        args: vec![],
        env: unit_env,
        unset_env: vec![],
        cwd: Some(module.dir.clone()),
        properties: vec![("CPUWeight".into(), module.manifest.limits.cpu_weight.to_string()), ("MemoryHigh".into(), module.manifest.limits.memory_high.clone())],
        bind_to: bind_to.map(String::from),
        stop_post: vec![],
    };
    units.start(&spec).await?;
    Ok(Some(spec.name))
}

/// `<exec> <key>` prints a JSON array of strings, bounded to 20 s; the static list when there is no exec.
pub async fn setting_choices(module: &Module, settings: &serde_json::Map<String, serde_json::Value>, key: &str) -> crate::Result<Vec<String>> {
    let s = module.manifest.settings.iter().find(|s| s.key == key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown setting {key}", module.id())))?;
    if s.choices_exec.is_empty() {
        return Ok(s.choices.clone());
    }
    let exe = module.dir.join(&s.choices_exec);
    run_choices(module_cmd(module, &exe)?, &exe, "MODULE", module.id(), settings, key).await
}

pub(crate) async fn run_choices(mut cmd: tokio::process::Command, exe: &Path, prefix: &str, id: &str, settings: &serde_json::Map<String, serde_json::Value>, key: &str) -> crate::Result<Vec<String>> {
    let child = cmd.arg(key).env(format!("{prefix}_SETTINGS_JSON"), serde_json::Value::Object(settings.clone()).to_string()).env("UNIVERSE_BIN", paths::self_exe()).spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let out = tokio::time::timeout(Duration::from_secs(20), child.wait_with_output())
        .await
        .map_err(|_| crate::Error::Io(format!("{id} {key}: choices timed out")))??;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(crate::Error::Io(format!("{id} {key}: choices failed ({}): {}", out.status.code().unwrap_or(-1), err.trim())));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| crate::Error::Io(format!("{id} {key}: bad choices: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_settings_merge() {
        let m: Manifest = toml::from_str(r#"
api = 2
id = "capture"
name = "Capture"
[requires]
bins = ["definitely-missing-binary-xyz"]
[hooks]
post-launch = "bin/start"
timeout_s = 5
[[settings]]
key = "cursor"
type = "bool"
default = false
label = "Curseur"
scope = "game"
[[settings]]
key = "codec"
type = "enum"
default = "av1_10bit"
choices = ["av1_10bit", "hevc"]
[[settings]]
key = "fps"
type = "int"
default = 60
choices = ["auto", "60"]
[[settings]]
key = "model"
type = "string"
default = "m"
choices_exec = "bin/choices"
[[settings]]
key = "gsr_extra_args"
type = "string"
default = ""
scope = "config"
"#).unwrap();
        let module = Module { available: false, missing: vec!["x".into()], enabled: true, dir: PathBuf::from("/m"), manifest: m };
        let cfg: Config = toml::from_str("[modules.capture]\ncodec = \"hevc\"\ngsr_extra_args = \"-cr full\"").unwrap();
        let mut g = crate::game::Game::new("X");
        g.modules.insert("capture".into(), toml::from_str("cursor = true").unwrap());
        let merged = module.merged_settings(&cfg, Some(&g));
        assert_eq!(merged["enabled"], true);
        assert_eq!(merged["cursor"], true);
        assert_eq!(merged["codec"], "hevc");
        g.modules.insert("capture".into(), toml::from_str("enabled = [\"false\"]").unwrap());
        assert_eq!(module.merged_settings(&cfg, Some(&g))["enabled"], false);
        assert!(module.validate_setting("codec", "vp9", false).is_err());
        assert!(module.validate_setting("codec", "hevc", true).is_err());
        assert!(module.validate_setting("cursor", "true", true).is_ok());
        assert!(module.validate_setting("fps", "144", false).is_ok());
        assert!(module.validate_setting("fps", "auto", false).is_ok());
        assert!(module.validate_setting("fps", "fast", false).is_err());
        assert_eq!(merged["gsr_extra_args"], "-cr full");
        assert!(module.validate_setting("gsr_extra_args", "-keyint 2", false).is_ok());
        assert!(module.validate_setting("gsr_extra_args", "-keyint 2", true).is_err());
        assert_eq!(module.timeout(), Duration::from_secs(5));
        assert_eq!(module.hook("post-launch"), Some(PathBuf::from("/m/bin/start")));
        let j = module.to_json();
        assert_eq!(j["settings"][0]["key"], "enabled");
        assert_eq!(j["settings"][3]["dynamic"], false);
        assert_eq!(j["settings"][4]["dynamic"], true);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn setting_choices_run_the_module_or_stay_static() {
        let _env = crate::paths::ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("UNIVERSE_DATA_HOME", dir.path().join("data"));
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        let exe = dir.path().join("bin/choices");
        std::fs::write(&exe, r##"#!/bin/sh
[ "$1" = model ] || exit 2
provider=$(printf '%s' "$MODULE_SETTINGS_JSON" | sed 's/.*"provider":"\([a-z]*\)".*/\1/')
printf '["provider:%s"]\n' "$provider"
"##).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let m: Manifest = toml::from_str(r#"
id = "journal"
[[settings]]
key = "provider"
type = "enum"
default = "codex"
choices = ["codex", "claude"]
[[settings]]
key = "model"
type = "string"
default = "m"
choices_exec = "bin/choices"
"#).unwrap();
        let module = Module { available: true, missing: vec![], enabled: true, dir: dir.path().to_path_buf(), manifest: m };
        let cfg: Config = toml::from_str("").unwrap();
        let settings = module.merged_settings(&cfg, None);
        assert_eq!(setting_choices(&module, &settings, "provider").await.unwrap(), vec!["codex", "claude"]);
        let listed = setting_choices(&module, &settings, "model").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].contains("provider:codex"), "{listed:?}");
        assert!(setting_choices(&module, &settings, "nope").await.is_err());
    }
}
