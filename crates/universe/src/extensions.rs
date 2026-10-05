use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::components::{self, Format};
use crate::config::Config;
use crate::core::{Core, Progress};
use crate::paths;
use crate::{Error, Result};

pub const API: u32 = 2;
pub const MIN_API: u32 = 2;
pub const SCHEMA: u32 = 1;
pub const DEFAULT_INDEX: &str = "https://raw.githubusercontent.com/ilyasturki/universe-extensions/index/index.json";
pub const KINDS: [&str; 2] = ["module", "source"];

/// Why a manifest's `api` rules it out; `None` when this Universe runs it.
pub fn unsupported(api: u32) -> Option<String> {
    let reads = if MIN_API == API { API.to_string() } else { format!("{MIN_API} to {API}") };
    match api {
        0 => Some(format!("declares no extension api: this Universe reads api {reads}")),
        a if (MIN_API..=API).contains(&a) => None,
        a => Some(format!("written for extension api {a}: this Universe reads api {reads}")),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    pub schema: u32,
    pub extensions: Vec<Listed>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Listed {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub api: u32,
    /// A tar (plain, gzip or xz) or a zip holding the extension's folder, at its root or in its one folder.
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub homepage: String,
}

/// `<kind>/<id>.json` beside the extension's folder: where `update` fetches it from again.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Installed {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub version: String,
    /// `registry` (from the index) or `unlisted` (a URL, an archive or a folder).
    pub origin: String,
    /// The index's URL for the registry's, else the URL, archive or folder.
    pub from: String,
    pub sha256: String,
    pub installed_at: String,
    #[serde(skip)]
    pub api: u32,
    #[serde(skip)]
    pub description: String,
}

/// The fields every `module.toml` and `source.toml` shares.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Header {
    api: u32,
    id: String,
    name: String,
    version: String,
    description: String,
}

pub fn index_url(config: &Config) -> String {
    std::env::var("UNIVERSE_EXTENSIONS_INDEX")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| Some(config.extensions.index.clone()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| DEFAULT_INDEX.into())
}

/// The path a `file://` URL or a path names, `None` for an https:// URL.
fn local(from: &str) -> Result<Option<PathBuf>> {
    if from.starts_with("https://") {
        return Ok(None);
    }
    if from.starts_with("http://") {
        return Err(Error::Invalid(format!("{from}: extensions come over https:// only")));
    }
    Ok(Some(paths::expand(from.strip_prefix("file://").unwrap_or(from))))
}

pub async fn fetch_index(url: &str) -> Result<Index> {
    let text = match local(url)? {
        Some(path) => std::fs::read_to_string(&path).map_err(|e| Error::Unavailable(format!("{url}: {e}")))?,
        None => {
            let failed = |e: reqwest::Error| Error::Unavailable(format!("the extension index could not be fetched: {e}"));
            components::client()?.get(url).send().await.and_then(reqwest::Response::error_for_status).map_err(failed)?.text().await.map_err(failed)?
        }
    };
    let index: Index = serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("the extension index: {e}")))?;
    if index.schema > SCHEMA {
        return Err(Error::Unavailable(format!("the extension index is schema {}, this Universe reads {SCHEMA}: update Universe", index.schema)));
    }
    Ok(index)
}

/// What an install names: an entry of the index, an https:// URL, or an archive or a folder on disk.
pub enum Origin {
    Listed(Box<Listed>, String),
    Url(String),
    Path(PathBuf),
}

/// `what`: an https:// URL, a `file://` URL or a path holding a `/` (`./now-playing`), else an id of the index.
pub async fn resolve(config: &Config, what: &str) -> Result<Origin> {
    let Some(path) = local(what)? else { return Ok(Origin::Url(what.into())) };
    if what.starts_with("file://") || what.contains('/') {
        if !path.exists() {
            return Err(Error::NotFound(format!("{}: no such archive or folder", path.display())));
        }
        return Ok(Origin::Path(std::fs::canonicalize(&path)?));
    }
    let url = index_url(config);
    let mut listed = fetch_index(&url).await?.extensions.into_iter().filter(|e| e.id == what);
    match (listed.next(), listed.next()) {
        (Some(_), Some(_)) => Err(Error::Invalid(format!("the extension index ({url}) lists {what} more than once"))),
        (Some(one), None) => Ok(Origin::Listed(Box::new(one), url)),
        (None, _) => {
            let here = if path.exists() { format!("; ./{what} names the one in this folder") } else { String::new() };
            Err(Error::NotFound(format!("{what} is not in the extension index ({url}){here}")))
        }
    }
}

fn work_dir() -> PathBuf {
    paths::data_home().join("extensions").join(".tmp")
}

/// A folder of the work dir, gone with it.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Scratch> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = work_dir().join(format!("{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Scratch(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An extension fetched, unpacked and checked, waiting to be put in place.
pub struct Staged {
    pub id: String,
    pub kind: &'static str,
    pub name: String,
    pub version: String,
    pub description: String,
    pub origin: &'static str,
    pub from: String,
    sha256: String,
    dir: PathBuf,
    _scratch: Scratch,
}

/// Where `url` comes from, as a confirmation names it: its host, or the file.
pub fn host(url: &str) -> String {
    match url.split_once("://") {
        Some(("https", rest)) => rest.split('/').next().unwrap_or(rest).to_string(),
        _ => url.strip_prefix("file://").unwrap_or(url).to_string(),
    }
}

impl Staged {
    /// What a confirmation shows before the install: the kind, that it runs programs as you, where it comes from.
    pub fn warning(&self) -> String {
        let origin = if self.origin == "registry" { format!("Listed in the index at {}", host(&self.from)) } else { "Unlisted".into() };
        format!("{} {}, a {}\n{origin} · runs programs as you\nfrom {}", self.name, self.version, self.kind, self.from)
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id, "kind": self.kind, "name": self.name, "version": self.version, "description": self.description,
            "origin": self.origin, "from": self.from,
        })
    }
}

fn sniff(file: &Path) -> Result<Format> {
    use std::io::Read;
    let mut head = Vec::with_capacity(262);
    std::fs::File::open(file)?.take(262).read_to_end(&mut head)?;
    match head.as_slice() {
        [0x1f, 0x8b, ..] => Ok(Format::TarGz),
        [0xfd, b'7', b'z', b'X', b'Z', 0, ..] => Ok(Format::TarXz),
        [b'P', b'K', 3, 4, ..] => Ok(Format::Zip),
        h if h.get(257..262) == Some(b"ustar") => Ok(Format::Tar),
        _ => Err(Error::Invalid(format!("{}: not a tar or a zip", file.display()))),
    }
}

fn digest(file: &Path) -> Result<String> {
    use sha2::Digest;
    use std::io::Read;
    let (mut f, mut hasher, mut buf) = (std::fs::File::open(file)?, sha2::Sha256::new(), vec![0u8; 1 << 16]);
    loop {
        match f.read(&mut buf)? {
            0 => break,
            n => hasher.update(&buf[..n]),
        }
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten().filter(|e| e.file_name() != ".git") {
        let (src, dest) = (e.path(), to.join(e.file_name()));
        let kind = e.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&src)?, &dest)?;
        } else if kind.is_dir() {
            copy_tree(&src, &dest)?;
        } else {
            std::fs::copy(&src, &dest)?;
        }
    }
    Ok(())
}

/// The folder holding the manifest, `dir` or its one folder, and its kind.
fn manifest_root(dir: &Path) -> Result<(PathBuf, &'static str)> {
    let kinds = |d: &Path| KINDS.into_iter().filter(|k| d.join(format!("{k}.toml")).is_file()).collect::<Vec<_>>();
    let root = if kinds(dir).is_empty() { components::single_child(dir) } else { dir.to_path_buf() };
    match kinds(&root).as_slice() {
        [kind] => Ok((root, kind)),
        [] => Err(Error::Invalid("no module.toml or source.toml at its root: it is no extension".into())),
        _ => Err(Error::Invalid("both a module.toml and a source.toml: an extension is one or the other".into())),
    }
}

fn shipped_and_user(kind: &str) -> (Vec<PathBuf>, PathBuf) {
    match kind {
        "module" => (paths::system_module_dirs(), paths::user_modules_dir()),
        _ => (paths::system_source_dirs(), paths::user_sources_dir()),
    }
}

/// Why `id` cannot be installed as a `kind`: one ships with Universe, or the user's own folder holds one.
fn held(kind: &str, id: &str) -> Option<String> {
    let (shipped, user) = shipped_and_user(kind);
    let file = format!("{kind}.toml");
    if crate::modules::read_manifests::<Header>(shipped.into_iter(), &file, |m| &m.id).contains_key(id) {
        return Some(format!("{id} ships with Universe: a {kind} of that id cannot be installed"));
    }
    let own = crate::modules::read_manifests::<Header>(std::iter::once(user.clone()), &file, |m| &m.id);
    own.get(id).map(|(dir, _)| format!("{id} is your own {kind} in {}, which would hide the installed one", dir.display()))
}

fn held_ids(kind: &str) -> std::collections::BTreeSet<String> {
    let (shipped, user) = shipped_and_user(kind);
    crate::modules::read_manifests::<Header>(shipped.into_iter().chain([user]), &format!("{kind}.toml"), |m| &m.id).into_keys().collect()
}

fn other(kind: &str) -> &'static str {
    if kind == "module" {
        "source"
    } else {
        "module"
    }
}

/// Every id the other kind holds, shipped, the user's own or installed: a module and a source never share one.
fn other_ids(kind: &str, installed: &[Installed]) -> std::collections::BTreeSet<String> {
    let other = other(kind);
    let mut ids = held_ids(other);
    ids.extend(installed.iter().filter(|i| i.kind == other).map(|i| i.id.clone()));
    ids
}

fn other_kind_holds(kind: &str, id: &str) -> Option<String> {
    other_ids(kind, &installed()).contains(id).then(|| format!("{id} is already a {}: a {kind} of that id cannot be installed", other(kind)))
}

fn valid_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

async fn unpack_into(file: PathBuf, dir: PathBuf, label: String) -> Result<()> {
    crate::core::blocking(move || {
        std::fs::create_dir_all(&dir)?;
        components::unpack_archive(sniff(&file)?, &file, &dir, &label)
    })
    .await
}

/// `origin` fetched and unpacked into the work dir, then checked: one manifest that parses, an id that is free, an api this Universe reads.
pub async fn prepare(origin: Origin, mut progress: Option<Progress<'_, '_>>, cancel: &AtomicBool) -> Result<Staged> {
    let scratch = Scratch::new()?;
    let unpacked = scratch.0.join("unpacked");
    let (from, sha256, listed) = match origin {
        Origin::Path(path) if path.is_dir() => {
            let (src, to) = (path.clone(), unpacked.clone());
            crate::core::blocking(move || copy_tree(&src, &to)).await?;
            (path.to_string_lossy().into_owned(), String::new(), None)
        }
        Origin::Path(path) => {
            let sha = digest(&path)?;
            unpack_into(path.clone(), unpacked.clone(), path.display().to_string()).await?;
            (path.to_string_lossy().into_owned(), sha, None)
        }
        Origin::Url(url) => {
            let file = scratch.0.join("download");
            let sha = components::fetch(&url, &file, 0, &host(&url), &mut progress, cancel).await?;
            unpack_into(file, unpacked.clone(), url.clone()).await?;
            (url, sha, None)
        }
        Origin::Listed(listed, index) => {
            let (file, sha) = match local(&listed.url)? {
                Some(_) if local(&index)?.is_none() => {
                    return Err(Error::Invalid(format!("{}: the index at {index} names a file on this computer", listed.url)));
                }
                Some(path) => {
                    let sha = digest(&path)?;
                    (path, sha)
                }
                None => {
                    let file = scratch.0.join("download");
                    let sha = components::fetch(&listed.url, &file, listed.size, &listed.name, &mut progress, cancel).await?;
                    (file, sha)
                }
            };
            if !sha.eq_ignore_ascii_case(listed.sha256.trim()) {
                return Err(Error::Io(format!("{}: sha256 {sha}, not the index's {}", listed.url, listed.sha256)));
            }
            unpack_into(file, unpacked.clone(), listed.url.clone()).await?;
            (index, sha, Some(*listed))
        }
    };
    let (dir, kind) = manifest_root(&unpacked)?;
    let text = std::fs::read_to_string(dir.join(format!("{kind}.toml")))?;
    let bad = |e: toml::de::Error| Error::Invalid(format!("{kind}.toml: {e}"));
    let header: Header = toml::from_str(&text).map_err(bad)?;
    if kind == "module" {
        toml::from_str::<crate::modules::Manifest>(&text).map_err(bad)?;
    } else if toml::from_str::<crate::sources::Manifest>(&text).map_err(bad)?.exe.is_empty() {
        return Err(Error::Invalid("source.toml names no exe".into()));
    }
    let id = header.id;
    if !valid_id(&id) {
        return Err(Error::Invalid(format!("{kind}.toml: id {id:?} is not lowercase letters, digits, - and _")));
    }
    if let Some(l) = listed.as_ref().filter(|l| l.id != id || l.kind != kind) {
        return Err(Error::Invalid(format!("the index lists {} as a {}, its archive holds the {kind} {id}", l.id, l.kind)));
    }
    if let Some(l) = listed.as_ref().filter(|l| l.version != header.version) {
        return Err(Error::Invalid(format!("the index lists {id} {}, its archive holds {id} {}", l.version, header.version)));
    }
    if let Some(why) = unsupported(header.api) {
        return Err(Error::Invalid(format!("{id}: {why}")));
    }
    if let Some(why) = held(kind, &id).or_else(|| other_kind_holds(kind, &id)) {
        return Err(Error::Invalid(why));
    }
    Ok(Staged {
        name: if header.name.is_empty() { id.clone() } else { header.name },
        id,
        kind,
        version: header.version,
        description: header.description,
        origin: if listed.is_some() { "registry" } else { "unlisted" },
        from,
        sha256,
        dir,
        _scratch: scratch,
    })
}

/// Over what was installed under the same id, the sidecar written last.
fn place(staged: Staged) -> Result<Installed> {
    let home = paths::extensions_dir(staged.kind);
    std::fs::create_dir_all(&home)?;
    let dest = home.join(&staged.id);
    let old = staged._scratch.0.join("old");
    if dest.exists() {
        std::fs::rename(&dest, &old)?;
    }
    if let Err(e) = std::fs::rename(&staged.dir, &dest) {
        if old.exists() {
            let _ = std::fs::rename(&old, &dest);
        }
        return Err(e.into());
    }
    let installed = Installed {
        id: staged.id.clone(),
        kind: staged.kind.into(),
        name: staged.name.clone(),
        version: staged.version.clone(),
        origin: staged.origin.into(),
        from: staged.from.clone(),
        sha256: staged.sha256.clone(),
        installed_at: components::now(),
        ..Installed::default()
    };
    components::write_atomic(&home.join(format!("{}.json", staged.id)), &serde_json::to_vec_pretty(&installed)?)?;
    tracing::info!("{} {} {} installed in {}", staged.kind, staged.id, staged.version, dest.display());
    Ok(installed)
}

/// `registry` or `unlisted` for an installed extension's folder, empty for a shipped one or the user's own.
pub fn origin_of(kind: &str, dir: &Path) -> String {
    if dir.parent() != Some(paths::extensions_dir(kind).as_path()) {
        return String::new();
    }
    let sidecar = std::fs::read_to_string(dir.with_extension("json")).ok().and_then(|s| serde_json::from_str::<Installed>(&s).ok());
    sidecar.map(|i| i.origin).filter(|o| !o.is_empty()).unwrap_or_else(|| "unlisted".into())
}

/// By kind, then id; a folder without its sidecar reads as unlisted, from nowhere.
pub fn installed() -> Vec<Installed> {
    let mut out = Vec::new();
    for kind in KINDS {
        let home = paths::extensions_dir(kind);
        for (id, (_, header)) in crate::modules::read_manifests::<Header>(std::iter::once(home.clone()), &format!("{kind}.toml"), |m| &m.id) {
            let sidecar = std::fs::read_to_string(home.join(format!("{id}.json"))).ok().and_then(|s| serde_json::from_str::<Installed>(&s).ok());
            let mut i = sidecar.unwrap_or_else(|| Installed { origin: "unlisted".into(), ..Installed::default() });
            i.name = if header.name.is_empty() { id.clone() } else { header.name };
            (i.id, i.kind, i.version, i.api, i.description) = (id, kind.into(), header.version, header.api, header.description);
            out.push(i);
        }
    }
    out
}

fn take_out(kind: &str, id: &str) -> Result<()> {
    let home = paths::extensions_dir(kind);
    let scratch = Scratch::new()?;
    std::fs::rename(home.join(id), scratch.0.join(id))?;
    match std::fs::remove_file(home.join(format!("{id}.json"))) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

fn row(listed: Option<&Listed>, mine: Option<&Installed>, enabled: bool) -> serde_json::Value {
    let pick = |l: fn(&Listed) -> &String, i: fn(&Installed) -> &String| mine.map(i).or(listed.map(l)).cloned().unwrap_or_default();
    let api = mine.map(|i| i.api).or(listed.map(|l| l.api)).unwrap_or_default();
    let update = match (listed, mine) {
        (Some(l), Some(i)) if i.origin == "registry" && l.version != i.version && unsupported(l.api).is_none() => l.version.clone(),
        _ => String::new(),
    };
    serde_json::json!({
        "id": pick(|l| &l.id, |i| &i.id),
        "kind": pick(|l| &l.kind, |i| &i.kind),
        "name": pick(|l| &l.name, |i| &i.name),
        "description": pick(|l| &l.description, |i| &i.description),
        "version": listed.map(|l| l.version.clone()).unwrap_or_else(|| pick(|l| &l.version, |i| &i.version)),
        "homepage": listed.map(|l| l.homepage.clone()).unwrap_or_default(),
        "size": listed.map(|l| l.size).unwrap_or(0),
        "listed": listed.is_some(),
        "installed": mine.is_some(),
        "installed_version": mine.map(|i| i.version.clone()).unwrap_or_default(),
        "origin": mine.map(|i| i.origin.clone()).unwrap_or_default(),
        "from": mine.map(|i| i.from.clone()).unwrap_or_default(),
        "update": update,
        "enabled": enabled,
        "incompatible": unsupported(api).unwrap_or_default(),
    })
}

impl Core {
    /// The index's modules and sources, then the installed ones it does not list, by kind then name.
    pub async fn extensions(&self) -> Result<serde_json::Value> {
        let config = self.config.read().await.clone();
        let url = index_url(&config);
        let (index, error) = match fetch_index(&url).await {
            Ok(index) => (index, String::new()),
            Err(e) => (Index::default(), e.to_string()),
        };
        let enabled = |kind: &str, id: &str| match kind {
            "module" => config.modules.enabled.iter().any(|e| e == id),
            _ => config.sources.enabled.iter().any(|e| e == id),
        };
        let mine = installed();
        let taken: std::collections::BTreeMap<&str, std::collections::BTreeSet<String>> =
            KINDS.into_iter().map(|k| (k, held_ids(k).into_iter().chain(other_ids(k, &mine)).collect())).collect();
        let shown: Vec<&Listed> =
            index.extensions.iter().filter(|l| valid_id(&l.id) && taken.get(l.kind.as_str()).is_some_and(|ids| !ids.contains(&l.id))).collect();
        let mut rows: Vec<serde_json::Value> =
            shown.iter().map(|l| row(Some(l), mine.iter().find(|i| i.kind == l.kind && i.id == l.id), enabled(&l.kind, &l.id))).collect();
        for i in mine.iter().filter(|i| !shown.iter().any(|l| l.kind == i.kind && l.id == i.id)) {
            rows.push(row(None, Some(i), enabled(&i.kind, &i.id)));
        }
        rows.sort_by_key(|r| (r["kind"].as_str().unwrap_or_default().to_string(), r["name"].as_str().unwrap_or_default().to_lowercase()));
        Ok(serde_json::json!({"index": {"url": url, "error": error}, "extensions": rows}))
    }

    /// `accepted`: what the extension is, that it runs programs as you and where it comes from was shown and accepted.
    pub async fn extension_install(&self, what: &str, accepted: bool, progress: Option<Progress<'_, '_>>) -> Result<serde_json::Value> {
        if !accepted {
            return Err(Error::Invalid(format!("{what} runs programs as you: its install waits to be accepted")));
        }
        let config = self.config.read().await.clone();
        let origin = resolve(&config, what).await?;
        let staged = prepare(origin, progress, &AtomicBool::new(false)).await?;
        self.extension_place(staged).await
    }

    pub async fn extension_place(&self, staged: Staged) -> Result<serde_json::Value> {
        if let Some(c) = self.current().await.filter(|_| paths::extensions_dir(staged.kind).join(&staged.id).exists()) {
            return Err(Error::Busy(format!("{} is running: replacing {} waits until it ends", c.title, staged.id)));
        }
        let i = place(staged)?;
        self.reload_modules().await;
        self.reload_all().await;
        Ok(serde_json::json!({"id": i.id, "kind": i.kind, "name": i.name, "version": i.version, "origin": i.origin}))
    }

    /// Fetched again from where each came from: the index's when it lists a newer version, else the URL, archive or folder;
    /// every installed one when `id` is empty. Returns what was replaced.
    pub async fn extension_update(&self, id: &str, mut progress: Option<Progress<'_, '_>>) -> Result<Vec<serde_json::Value>> {
        if let Some(c) = self.current().await {
            return Err(Error::Busy(format!("{} is running: updates wait until it ends", c.title)));
        }
        let config = self.config.read().await.clone();
        let targets: Vec<Installed> = installed().into_iter().filter(|i| id.is_empty() || i.id == id).collect();
        if !id.is_empty() && targets.is_empty() {
            return Err(Error::NotFound(format!("{id} is not an installed extension")));
        }
        let url = index_url(&config);
        let index = if targets.iter().any(|t| t.origin == "registry") { Some(fetch_index(&url).await) } else { None };
        let mut out = Vec::new();
        for t in targets {
            let origin = match t.origin.as_str() {
                "registry" => match index.as_ref().expect("fetched for a registry extension") {
                    Ok(index) => match index.extensions.iter().find(|l| l.kind == t.kind && l.id == t.id) {
                        Some(l) if l.version == t.version => continue,
                        Some(l) => match unsupported(l.api) {
                            Some(why) => Err(Error::Invalid(format!("{} {}: {why}", t.id, l.version))),
                            None => Ok(Origin::Listed(Box::new(l.clone()), url.clone())),
                        },
                        None => Err(Error::NotFound(format!("{} is no longer in the extension index", t.id))),
                    },
                    Err(e) => Err(Error::Unavailable(e.to_string())),
                },
                _ if t.from.is_empty() => continue,
                _ => resolve(&config, &t.from).await,
            };
            let staged = match origin {
                Ok(origin) => prepare(origin, progress.as_deref_mut(), &AtomicBool::new(false)).await,
                Err(e) => Err(e),
            };
            let placed = staged.and_then(|s| {
                if s.id != t.id || s.kind != t.kind {
                    return Err(Error::Invalid(format!("{} now holds the {} {}, not the {} {}", t.from, s.kind, s.id, t.kind, t.id)));
                }
                place(s)
            });
            match placed {
                Ok(i) => out.push(serde_json::json!({"id": i.id, "kind": i.kind, "name": i.name, "version": i.version})),
                Err(e) if !id.is_empty() => return Err(e),
                Err(e) => {
                    tracing::warn!("{} {}: {e}", t.kind, t.id);
                    out.push(serde_json::json!({"id": t.id, "kind": t.kind, "name": t.name, "version": t.version, "error": e.to_string()}));
                }
            }
        }
        self.reload_modules().await;
        self.reload_all().await;
        Ok(out)
    }

    /// Switched off first, so that another extension of the same id installed later is not on from the start.
    pub async fn extension_remove(&self, id: &str) -> Result<()> {
        let Some(found) = installed().into_iter().find(|i| i.id == id) else {
            return Err(match KINDS.iter().find_map(|k| held(k, id)) {
                Some(why) => Error::Invalid(format!("{why}; turn it off instead")),
                None => Error::NotFound(format!("{id} is not an installed extension")),
            });
        };
        if let Some(c) = self.current().await {
            return Err(Error::Busy(format!("{} is running: removing {id} waits until it ends", c.title)));
        }
        let config = self.config.read().await.clone();
        let home = paths::extensions_dir(&found.kind).join(id);
        let runs = match found.kind.as_str() {
            "module" => self.modules.read().await.iter().any(|m| m.id() == id && m.dir == home),
            _ => self.sources.read().await.iter().any(|s| s.id() == id && s.dir == home),
        };
        let off = match found.kind.as_str() {
            "module" if runs && config.modules.enabled.iter().any(|e| e == id) => Some(self.enable_module(id, false).await),
            "source" if runs && config.sources.enabled.iter().any(|e| e == id) => Some(self.enable_source(id, false).await),
            _ => None,
        };
        if let Some(Err(e)) = off {
            tracing::warn!("{id} stays in config.toml's enabled list: {e}");
        }
        take_out(&found.kind, id)?;
        self.reload_modules().await;
        self.reload_all().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn an_api_out_of_range_or_absent_is_unsupported() {
        assert_eq!(unsupported(API), None);
        assert!(unsupported(0).is_some_and(|w| w.contains("no extension api")));
        assert!(unsupported(API + 1).is_some_and(|w| w.contains(&format!("api {}", API + 1))));
        assert!(unsupported(MIN_API - 1).is_some());
    }

    /// A module whose pre-launch hook, or a source whose exe, prints `<id> <version>`.
    fn tree(root: &Path, kind: &str, id: &str, version: &str, api: u32) -> PathBuf {
        let dir = root.join(format!("{id}-{version}"));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        let wiring = if kind == "module" { "[hooks]\npre-launch = \"bin/run\"\n" } else { "exe = \"bin/run\"\n" };
        std::fs::write(dir.join(format!("{kind}.toml")), format!("api = {api}\nid = \"{id}\"\nname = \"{id} name\"\nversion = \"{version}\"\n{wiring}"))
            .unwrap();
        let script = if kind == "module" {
            format!("#!/bin/sh\necho {id} {version}\n")
        } else {
            format!("#!/bin/sh\necho '{{\"event\":\"logged_in\",\"user\":\"{id} {version}\"}}'\necho '{{\"event\":\"done\"}}'\n")
        };
        std::fs::write(dir.join("bin/run"), script).unwrap();
        std::fs::set_permissions(dir.join("bin/run"), std::fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }

    /// `dir` in a gzipped tar, under one folder as a release's archive has it.
    fn tgz(dir: &Path, to: &Path) -> PathBuf {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast()));
        b.append_dir_all("release", dir).unwrap();
        std::fs::write(to, b.into_inner().unwrap().finish().unwrap()).unwrap();
        to.to_path_buf()
    }

    fn sha(file: &Path) -> String {
        digest(file).unwrap()
    }

    async fn open() -> Core {
        Core::open_with(Config::load().unwrap(), Host::memory().0).await.unwrap()
    }

    async fn ran(core: &Core, kind: &str, id: &str) -> String {
        if kind == "module" {
            let m = core.modules.read().await.iter().find(|m| m.id() == id).cloned().unwrap_or_else(|| panic!("no module {id}"));
            assert!(m.available, "{id}: {}", m.unavailable());
            let out = crate::modules::run_blocking(&crate::modules::Hooker::Module(m), "pre-launch", &Default::default()).await.unwrap();
            return out.stdout.trim().to_string();
        }
        let s = core.sources.read().await.iter().find(|s| s.id() == id).cloned().unwrap_or_else(|| panic!("no source {id}"));
        assert!(s.available, "{id}: {}", s.unavailable());
        let mut user = String::new();
        crate::sources::run(
            &s,
            &Default::default(),
            "status",
            &[],
            |_| {},
            |ev| {
                if let crate::sources::SourceEvent::LoggedIn { user: u } = ev {
                    user = u;
                }
            },
        )
        .await
        .unwrap();
        user
    }

    #[tokio::test]
    async fn a_module_and_a_source_install_from_a_folder_and_an_archive_run_update_and_go() {
        let env = paths::test_env();
        let work = env.path().join("work");
        let core = open().await;
        for (kind, id, how) in [("module", "hello", "folder"), ("source", "shop", "folder"), ("module", "greet", "archive"), ("source", "mart", "archive")] {
            let what = |version: &str| {
                let dir = tree(&work, kind, id, version, API);
                if how == "folder" {
                    dir.to_string_lossy().into_owned()
                } else {
                    format!("file://{}", tgz(&dir, &work.join(format!("{id}.tar.gz"))).display())
                }
            };
            let first = what("1.0.0");
            assert!(core.extension_install(&first, false, None).await.is_err(), "nothing is installed unaccepted");
            let placed = core.extension_install(&first, true, None).await.unwrap();
            assert_eq!((placed["kind"].as_str(), placed["origin"].as_str()), (Some(kind), Some("unlisted")));
            assert!(paths::extensions_dir(kind).join(id).join(format!("{kind}.toml")).is_file());
            let listed = if kind == "module" { core.modules().await } else { core.sources().await };
            assert_eq!(listed.iter().find(|e| e["id"] == id).map(|e| e["origin"].clone()), Some("unlisted".into()), "its entry says where it came from");
            assert_eq!(ran(&core, kind, id).await, format!("{id} 1.0.0"), "{how}: the installed {kind} runs");

            if how == "folder" {
                std::fs::remove_dir_all(&first).unwrap();
                std::fs::rename(tree(&work, kind, id, "2.0.0", API), &first).unwrap();
            } else {
                what("2.0.0");
            }
            let updated = core.extension_update(id, None).await.unwrap();
            assert_eq!(updated[0]["version"], "2.0.0");
            assert_eq!(ran(&core, kind, id).await, format!("{id} 2.0.0"), "{how}: an update replaces it in place");

            core.extension_remove(id).await.unwrap();
            assert!(!paths::extensions_dir(kind).join(id).exists() && !paths::extensions_dir(kind).join(format!("{id}.json")).exists());
            assert!(!core.modules.read().await.iter().any(|m| m.id() == id) && !core.sources.read().await.iter().any(|s| s.id() == id));
        }
        assert!(core.extension_remove("hello").await.is_err_and(|e| matches!(e, Error::NotFound(_))));
    }

    #[tokio::test]
    async fn removing_a_copy_that_another_hides_leaves_that_one_on() {
        let env = paths::test_env();
        let work = env.path().join("work");
        let core = open().await;
        core.extension_install(&tree(&work, "module", "hello", "1.0.0", API).to_string_lossy(), true, None).await.unwrap();
        core.enable_module("hello", true).await.unwrap();
        std::fs::create_dir_all(paths::user_modules_dir()).unwrap();
        std::fs::rename(tree(&work, "module", "hello", "2.0.0", API), paths::user_modules_dir().join("hello")).unwrap();
        core.reload_modules().await;
        core.extension_remove("hello").await.unwrap();
        assert!(core.config.read().await.modules.enabled.iter().any(|e| e == "hello"), "the user's own hello stays on");
    }

    #[tokio::test]
    async fn a_shipped_id_an_unsupported_api_and_plain_http_are_refused() {
        let env = paths::test_env();
        let work = env.path().join("work");
        let shipped = tree(&env.path().join("modules"), "module", "capture", "1.0.0", API);
        std::fs::rename(&shipped, env.path().join("modules/capture")).unwrap();
        let core = open().await;
        fn refused<T>(r: Result<T>) -> String {
            r.err().map(|e| e.to_string()).unwrap_or_default()
        }
        let capture = tree(&work, "module", "capture", "9.0.0", API);
        assert!(refused(core.extension_install(&capture.to_string_lossy(), true, None).await).contains("ships with Universe"));
        let future = tree(&work, "source", "future", "1.0.0", API + 1);
        assert!(refused(core.extension_install(&future.to_string_lossy(), true, None).await).contains("extension api"));
        let bare = tree(&work, "module", "bare", "1.0.0", 0);
        assert!(refused(core.extension_install(&bare.to_string_lossy(), true, None).await).contains("no extension api"));
        assert!(refused(core.extension_install("http://example.org/x.tar.gz", true, None).await).contains("https://"));
        assert!(refused(core.extension_remove("capture").await).contains("ships with Universe"));
        let as_source = tree(&work, "source", "capture", "1.0.0", API);
        assert!(
            refused(core.extension_install(&as_source.to_string_lossy(), true, None).await).contains("already a module"),
            "a source never takes a module's id"
        );
        std::fs::write(env.path().join("index.json"), r#"{"schema": 1, "extensions": []}"#).unwrap();
        std::env::set_var("UNIVERSE_EXTENSIONS_INDEX", env.path().join("index.json"));
        assert!(
            refused(core.extension_install("capture", true, None).await).contains("not in the extension index"),
            "a bare word is an id of the index, never a path"
        );
        assert!(installed().is_empty(), "nothing refused is left behind");
        assert_eq!(std::fs::read_dir(work_dir()).map(|d| d.count()).unwrap_or(0), 0, "nor in the work dir");
    }

    #[tokio::test]
    async fn the_index_installs_by_id_checks_the_pin_and_offers_its_newer_version() {
        let env = paths::test_env();
        let work = env.path().join("work");
        let archive = |version: &str| tgz(&tree(&work, "source", "shop", version, API), &work.join(format!("shop-{version}.tar.gz")));
        let (one, two) = (archive("1.0.0"), archive("2.0.0"));
        let index = env.path().join("index.json");
        let write = |version: &str, file: &Path, pin: &str, api: u32| {
            let entry = serde_json::json!({"id": "shop", "kind": "source", "name": "Shop", "version": version, "api": api,
                "url": format!("file://{}", file.display()), "sha256": pin});
            let other = serde_json::json!({"id": "later", "kind": "module", "name": "Later", "version": "1.0.0", "api": API + 1, "url": "https://example.org/l.tar.gz", "sha256": ""});
            std::fs::write(&index, serde_json::json!({"schema": SCHEMA, "extensions": [entry, other]}).to_string()).unwrap();
        };
        std::env::set_var("UNIVERSE_EXTENSIONS_INDEX", format!("file://{}", index.display()));
        let core = open().await;

        write("1.0.0", &one, &"0".repeat(64), API);
        assert!(core.extension_install("shop", true, None).await.is_err_and(|e| e.to_string().contains("sha256")), "a pin that does not match");
        write("1.0.0", &one, &sha(&one), API);
        assert_eq!(core.extension_install("shop", true, None).await.unwrap()["origin"], "registry");
        assert_eq!(ran(&core, "source", "shop").await, "shop 1.0.0");

        write("2.0.0", &two, &sha(&two), API);
        let listing = core.extensions().await.unwrap();
        let rows = listing["extensions"].as_array().unwrap();
        let shop = rows.iter().find(|r| r["id"] == "shop").unwrap();
        assert_eq!((&shop["installed"], &shop["installed_version"], &shop["update"]), (&true.into(), &"1.0.0".into(), &"2.0.0".into()));
        let later = rows.iter().find(|r| r["id"] == "later").unwrap();
        assert!(later["incompatible"].as_str().unwrap().contains("extension api"), "the index's entry for another Universe says so");
        assert_eq!(core.extension_update("", None).await.unwrap().len(), 1);
        assert_eq!(ran(&core, "source", "shop").await, "shop 2.0.0");
        assert!(core.extension_update("", None).await.unwrap().is_empty(), "current: nothing to fetch");

        write("2.5.0", &two, &sha(&two), API);
        assert!(
            core.extension_update("shop", None).await.is_err_and(|e| e.to_string().contains("its archive holds shop 2.0.0")),
            "an entry naming another version"
        );
        write("3.0.0", &two, &sha(&two), API + 1);
        let listing = core.extensions().await.unwrap();
        let shop = listing["extensions"].as_array().unwrap().iter().find(|r| r["id"] == "shop").cloned().unwrap();
        assert_eq!(shop["update"], "", "no update to a version for another Universe");
        let tried = core.extension_update("", None).await.unwrap();
        assert!(tried[0]["error"].as_str().is_some_and(|e| e.contains("extension api")), "{tried:?}");
        assert_eq!(ran(&core, "source", "shop").await, "shop 2.0.0");
    }

    #[tokio::test]
    async fn an_installed_extension_for_another_api_stays_listed_unavailable_and_doctor_says_so() {
        let env = paths::test_env();
        let installed = tree(&env.path().join("work"), "module", "old", "1.0.0", API + 1);
        std::fs::create_dir_all(paths::extensions_dir("module")).unwrap();
        std::fs::rename(installed, paths::extensions_dir("module").join("old")).unwrap();
        let core = open().await;
        let old = core.modules.read().await.iter().find(|m| m.id() == "old").cloned().unwrap();
        assert!(!old.available && old.incompatible.contains("extension api"), "{}", old.incompatible);
        let checks = core.doctor().await;
        assert!(checks.iter().any(|c| c.check == "extension-api" && c.module == "old" && !c.ok), "reported though it is off");
    }
}
