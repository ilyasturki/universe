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
    pub kind: Vec<String>,
    pub version: String,
    pub requires: Requires,
    pub hooks: BTreeMap<String, toml::Value>,
    pub limits: Limits,
    pub source: SourceSpec,
    pub frontend: Frontend,
    pub settings: Vec<Setting>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SourceSpec {
    pub exe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Frontend {
    pub qml: String,
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

pub const HOOKS: [&str; 5] = ["pre-launch", "post-launch", "session-end", "post-process", "screenshot"];

impl Module {
    pub fn id(&self) -> &str {
        &self.manifest.id
    }
    pub fn is_hooks(&self) -> bool {
        self.manifest.kind.iter().any(|k| k == "hooks")
    }
    pub fn is_source(&self) -> bool {
        self.manifest.kind.iter().any(|k| k == "source") && !self.manifest.source.exe.is_empty()
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
        let verbs: Vec<&str> = if self.is_source() { vec!["login", "library", "search", "info", "install", "update", "scan"] } else { vec![] };
        serde_json::json!({
            "id": m.id,
            "name": if m.name.is_empty() { m.id.clone() } else { m.name.clone() },
            "kind": m.kind,
            "version": m.version,
            "dir": self.dir,
            "enabled": self.enabled,
            "available": self.available,
            "missing": self.missing,
            "hooks": hooks,
            "verbs": verbs,
            "settings": self.settings_json(),
            "frontend_qml": if m.frontend.qml.is_empty() { serde_json::Value::Null } else { serde_json::json!(self.dir.join(&m.frontend.qml)) },
        })
    }

    pub fn settings_json(&self) -> serde_json::Value {
        let mut list: Vec<serde_json::Value> = Vec::new();
        let has_enabled = self.manifest.settings.iter().any(|s| s.key == "enabled");
        if !has_enabled && self.is_hooks() {
            list.push(serde_json::json!({"key": "enabled", "type": "bool", "default": true, "label": "Enable", "scope": "game", "choices": []}));
        }
        for s in &self.manifest.settings {
            list.push(serde_json::json!({
                "key": s.key,
                "type": if s.kind.is_empty() { "string" } else { s.kind.as_str() },
                "default": toml_to_json(&s.default),
                "label": s.label,
                "scope": if s.scope.is_empty() { "global" } else { s.scope.as_str() },
                "choices": s.choices,
                "dynamic": !s.choices_exec.is_empty(),
            }));
        }
        serde_json::Value::Array(list)
    }

    /// Global defaults ← config.toml [modules.<id>] ← game.toml [modules.<id>].
    pub fn merged_settings(&self, config: &Config, game: Option<&crate::game::Game>) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        if self.is_hooks() {
            out.insert("enabled".into(), serde_json::Value::Bool(true));
        }
        for s in &self.manifest.settings {
            out.insert(s.key.clone(), toml_to_json(&s.default));
        }
        if out.get("games_dir").and_then(|v| v.as_str()) == Some("") {
            out.insert("games_dir".into(), serde_json::Value::String(config.games_root().to_string_lossy().into()));
        }
        if let Some(t) = config.modules.settings.get(self.id()) {
            for (k, v) in t {
                out.insert(k.clone(), toml_to_json(v));
            }
        }
        if let Some(g) = game {
            if let Some(t) = g.modules.get(self.id()) {
                for (k, v) in t {
                    out.insert(k.clone(), toml_to_json(v));
                }
            }
        }
        out
    }

    pub fn validate_setting(&self, key: &str, value: &str, scope_game: bool) -> crate::Result<String> {
        if key == "enabled" && self.is_hooks() && !self.manifest.settings.iter().any(|s| s.key == "enabled") {
            return match value {
                "true" | "false" => Ok(value.into()),
                _ => Err(crate::Error::Invalid("enabled must be true or false".into())),
            };
        }
        let s = self.manifest.settings.iter().find(|s| s.key == key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown setting {key}", self.id())))?;
        let scope = if s.scope.is_empty() { "global" } else { s.scope.as_str() };
        if scope_game && scope != "game" {
            return Err(crate::Error::Invalid(format!("{}.{key} is a global setting", self.id())));
        }
        match s.kind.as_str() {
            "bool" => match value {
                "true" | "false" => Ok(value.into()),
                _ => Err(crate::Error::Invalid(format!("{key} must be true or false"))),
            },
            // A listed non-numeric choice is a named value ("auto") the module resolves itself.
            "int" if value.parse::<i64>().is_ok() || s.choices.iter().any(|c| c == value) => Ok(value.into()),
            "int" => Err(crate::Error::Invalid(format!("{key} must be an integer"))),
            "enum" => {
                if s.choices.iter().any(|c| c == value) {
                    Ok(value.into())
                } else {
                    Err(crate::Error::Invalid(format!("{key} must be one of {}", s.choices.join(", "))))
                }
            }
            _ => Ok(value.into()),
        }
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

fn which(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).exists();
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push(PathBuf::from("/run/wrappers/bin"));
    dirs.iter().any(|d| d.join(bin).is_file())
}

/// User modules override system modules on the same id.
pub fn discover(config: &Config) -> Vec<Module> {
    let mut found: BTreeMap<String, Module> = BTreeMap::new();
    let mut roots = paths::system_module_dirs();
    roots.insert(0, paths::user_modules_dir());
    for root in roots.iter().rev() {
        let Ok(rd) = std::fs::read_dir(root) else { continue };
        for e in rd.flatten() {
            let dir = e.path();
            let mp = dir.join("module.toml");
            if !mp.is_file() {
                continue;
            }
            match std::fs::read_to_string(&mp).map_err(crate::Error::from).and_then(|s| toml::from_str::<Manifest>(&s).map_err(Into::into)) {
                Ok(m) if !m.id.is_empty() => {
                    let missing: Vec<String> = m.requires.bins.iter().filter(|b| !which(b)).cloned().collect();
                    let enabled = config.modules.enabled.iter().any(|e| e == &m.id);
                    let module = Module { available: missing.is_empty(), missing, enabled, dir: dir.clone(), manifest: m };
                    found.insert(module.id().to_string(), module);
                }
                Ok(_) => tracing::warn!("{}: manifest without id", mp.display()),
                Err(err) => tracing::warn!("{}: {err}", mp.display()),
            }
        }
    }
    found.into_values().collect()
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

/// Blocking hook (pre-launch, session-end, screenshot): waits up to the manifest timeout, then kills.
pub async fn run_blocking(module: &Module, hook: &str, env: &HookEnv) -> crate::Result<HookOutcome> {
    let Some(exe) = module.hook(hook) else {
        return Ok(HookOutcome { status: 0, stdout: String::new(), stderr: String::new() });
    };
    std::fs::create_dir_all(module.data_dir())?;
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.envs(env.vars.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .env("MODULE_DIR", &module.dir)
        .env("MODULE_DATA_DIR", module.data_dir())
        .current_dir(&module.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let timeout = if hook == "screenshot" { Duration::from_secs(30) } else { module.timeout() };
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => {
            let o = HookOutcome { status: out.status.code().unwrap_or(-1), stdout: String::from_utf8_lossy(&out.stdout).into(), stderr: String::from_utf8_lossy(&out.stderr).into() };
            if o.status != 0 {
                tracing::warn!("hook {}:{hook} exited {}: {}", module.id(), o.status, o.stderr.trim());
            }
            Ok(o)
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => {
            tracing::warn!("hook {}:{hook} timed out after {timeout:?}", module.id());
            Err(crate::Error::Io(format!("{}:{hook} timed out", module.id())))
        }
    }
}

/// Async hook (post-launch, post-process): a transient unit with the manifest limits; returns the unit name.
/// `bind_to` (post-launch) ties the unit to the game's so it is stopped with it whatever happens to the caller.
pub fn run_async(module: &Module, hook: &str, env: &HookEnv, session_id: &str, bind_to: Option<&str>) -> crate::Result<Option<String>> {
    let Some(exe) = module.hook(hook) else { return Ok(None) };
    std::fs::create_dir_all(module.data_dir())?;
    let unit = format!("universe-{}-{}-{}", module.id(), hook, session_id);
    let mut cmd = std::process::Command::new("systemd-run");
    cmd.arg("--user").arg("--collect").arg("--quiet").arg(format!("--unit={unit}"))
        .arg(format!("--property=CPUWeight={}", module.manifest.limits.cpu_weight))
        .arg(format!("--property=MemoryHigh={}", module.manifest.limits.memory_high))
        .arg(format!("--working-directory={}", module.dir.display()));
    if let Some(game_unit) = bind_to {
        cmd.arg(format!("--property=BindsTo={game_unit}")).arg(format!("--property=After={game_unit}"));
    }
    for (k, v) in &env.vars {
        cmd.arg(format!("--setenv={k}={v}"));
    }
    cmd.arg(format!("--setenv=MODULE_DIR={}", module.dir.display()));
    cmd.arg(format!("--setenv=MODULE_DATA_DIR={}", module.data_dir().display()));
    cmd.arg(&exe);
    let status = cmd.stdin(Stdio::null()).status().map_err(|e| crate::Error::Io(format!("systemd-run: {e}")))?;
    if !status.success() {
        return Err(crate::Error::Io(format!("systemd-run failed for {unit}")));
    }
    Ok(Some(unit))
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

/// Runs `bin/source <verb> [args]`, streaming one JSON event per stdout line to `on_event`.
pub async fn run_source<F>(module: &Module, settings: &serde_json::Map<String, serde_json::Value>, verb: &str, args: &[String], mut on_event: F) -> crate::Result<()>
where
    F: FnMut(SourceEvent),
{
    use tokio::io::AsyncBufReadExt;
    let exe = module.dir.join(&module.manifest.source.exe);
    std::fs::create_dir_all(module.data_dir())?;
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg(verb).args(args)
        .env("MODULE_SETTINGS_JSON", serde_json::Value::Object(settings.clone()).to_string())
        .env("MODULE_DIR", &module.dir)
        .env("MODULE_DATA_DIR", module.data_dir())
        .env("UNIVERSE_BIN", paths::self_exe())
        .current_dir(&module.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let id = module.id().to_string();
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
            Err(e) => tracing::warn!("source {}: bad event {e}: {line}", module.id()),
        }
    }
    let status = child.wait().await?;
    let last_err = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(crate::Error::Io(format!("{} {verb} failed ({}): {last_err}", module.id(), status.code().unwrap_or(-1))));
    }
    Ok(())
}

/// The choices a setting's `choices_exec` prints (a JSON array of strings); the static list when
/// there is none. Run as `<exec> <key>` in the source's environment, bounded to 20 s.
pub async fn setting_choices(module: &Module, settings: &serde_json::Map<String, serde_json::Value>, key: &str) -> crate::Result<Vec<String>> {
    let s = module.manifest.settings.iter().find(|s| s.key == key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown setting {key}", module.id())))?;
    if s.choices_exec.is_empty() {
        return Ok(s.choices.clone());
    }
    let exe = module.dir.join(&s.choices_exec);
    std::fs::create_dir_all(module.data_dir())?;
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg(key)
        .env("MODULE_SETTINGS_JSON", serde_json::Value::Object(settings.clone()).to_string())
        .env("MODULE_DIR", &module.dir)
        .env("MODULE_DATA_DIR", module.data_dir())
        .current_dir(&module.dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| crate::Error::Io(format!("{}: {e}", exe.display())))?;
    let out = tokio::time::timeout(Duration::from_secs(20), child.wait_with_output())
        .await
        .map_err(|_| crate::Error::Io(format!("{} {key}: choices timed out", module.id())))??;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(crate::Error::Io(format!("{} {key}: choices failed ({}): {}", module.id(), out.status.code().unwrap_or(-1), err.trim())));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| crate::Error::Io(format!("{} {key}: bad choices: {e}", module.id())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_and_settings_merge() {
        let m: Manifest = toml::from_str(r#"
api = 1
id = "capture"
name = "Capture"
kind = ["hooks"]
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
"#).unwrap();
        let module = Module { available: false, missing: vec!["x".into()], enabled: true, dir: PathBuf::from("/m"), manifest: m };
        let cfg: Config = toml::from_str("[modules.capture]\ncodec = \"hevc\"").unwrap();
        let mut g = crate::game::Game::new("X");
        g.modules.insert("capture".into(), toml::from_str("cursor = true").unwrap());
        let merged = module.merged_settings(&cfg, Some(&g));
        assert_eq!(merged["enabled"], true);
        assert_eq!(merged["cursor"], true);
        assert_eq!(merged["codec"], "hevc");
        assert!(module.validate_setting("codec", "vp9", false).is_err());
        assert!(module.validate_setting("codec", "hevc", true).is_err());
        assert!(module.validate_setting("cursor", "true", true).is_ok());
        // An int takes a number or one of its listed names, nothing else.
        assert!(module.validate_setting("fps", "144", false).is_ok());
        assert!(module.validate_setting("fps", "auto", false).is_ok());
        assert!(module.validate_setting("fps", "fast", false).is_err());
        assert_eq!(module.timeout(), Duration::from_secs(5));
        assert_eq!(module.hook("post-launch"), Some(PathBuf::from("/m/bin/start")));
        let j = module.to_json();
        assert_eq!(j["settings"][0]["key"], "enabled");
        assert_eq!(j["settings"][3]["dynamic"], false);
        assert_eq!(j["settings"][4]["dynamic"], true);
    }

    #[tokio::test]
    async fn setting_choices_run_the_module_or_stay_static() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        let exe = dir.path().join("bin/choices");
        // Echoes the provider it was handed, as `["provider:codex"]`.
        std::fs::write(&exe, r##"#!/bin/sh
[ "$1" = model ] || exit 2
provider=$(printf '%s' "$MODULE_SETTINGS_JSON" | sed 's/.*"provider":"\([a-z]*\)".*/\1/')
printf '["provider:%s"]\n' "$provider"
"##).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let m: Manifest = toml::from_str(r#"
id = "journal"
kind = ["hooks"]
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

    #[test]
    fn source_events_parse() {
        let e: SourceEvent = serde_json::from_str(r#"{"event":"game","id":"1","title":"T","owned":true}"#).unwrap();
        assert!(matches!(e, SourceEvent::Game(_)));
        let e: SourceEvent = serde_json::from_str(r#"{"event":"progress","done":1,"total":2}"#).unwrap();
        assert!(matches!(e, SourceEvent::Progress { done: 1, total: 2, .. }));
        let e: SourceEvent = serde_json::from_str(r#"{"event":"done"}"#).unwrap();
        assert!(matches!(e, SourceEvent::Done));
    }
}
