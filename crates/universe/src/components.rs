use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::{version_key, Config};
use crate::core::{Core, Progress};
use crate::library::Resolved;
use crate::paths;
use crate::runners;
use crate::{Error, Result};

pub const DEFAULT_CATALOGUE: &str = "https://raw.githubusercontent.com/ilyasturki/universe/catalogue/catalogue.json";
pub const SCHEMA: u32 = 1;
const FRESH: Duration = Duration::from_secs(24 * 3600);
const RECENT_DAYS: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Proton,
    Wine,
    Emulator,
    Tool,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Rolling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    #[serde(rename = "appimage")]
    AppImage,
    #[serde(rename = "tar")]
    Tar,
    #[serde(rename = "tar.gz")]
    TarGz,
    #[serde(rename = "tar.xz")]
    TarXz,
    #[serde(rename = "zip")]
    Zip,
    #[serde(rename = "binary")]
    Binary,
}

fn default_arch() -> String {
    "x86_64".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    /// `any` for an architecture-independent download (a Python zipapp).
    #[serde(default = "default_arch")]
    pub arch: String,
    /// `x86_64_v3` is taken over the plain build where the CPU has that level.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub variant: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    pub format: Format,
    /// The one file of a tar that is the program.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub member: String,
    /// An AppImage inside the archive, unpacked in turn.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub appimage: String,
    /// Under the build's folder; empty takes the kind's default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub program: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    pub version: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub channel: Channel,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    /// A Proton build's family: the name `launch.proton` follows (`proton-ge`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bin: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub homepage: String,
    /// Shown before an install, which waits for it to be accepted.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notice: String,
    #[serde(default)]
    pub builds: Vec<Build>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Catalogue {
    pub schema: u32,
    pub generated_at: String,
    pub components: BTreeMap<String, Entry>,
}

fn builtin() -> Catalogue {
    let proton = |name: &str, family: &str, homepage: &str| serde_json::json!({"name": name, "kind": "proton", "family": family, "homepage": homepage});
    let tool = |name: &str, bin: &str, version: &str, asset: serde_json::Value| serde_json::json!({"name": name, "kind": "tool", "bin": bin, "builds": [{"version": version, "assets": [asset]}]});
    serde_json::from_value(serde_json::json!({"schema": SCHEMA, "components": {
        "ge-proton": proton("GE-Proton", "proton-ge", "https://github.com/GloriousEggroll/proton-ge-custom"),
        "proton-cachyos": proton("Proton-CachyOS", "proton-cachyos", "https://github.com/CachyOS/proton-cachyos"),
        "proton-em": proton("Proton-EM", "proton-em", "https://github.com/Etaash-mathamsetty/Proton"),
        "umu-proton": proton("UMU-Proton", "umu-proton", "https://github.com/Open-Wine-Components/umu-proton"),
        "umu-run": tool("umu-launcher", "umu-run", "1.4.4", serde_json::json!({"arch": "any", "format": "tar", "member": "umu/umu-run",
            "url": "https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.4/umu-launcher-1.4.4-zipapp.tar",
            "sha256": "eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851"})),
        "ludusavi": tool("ludusavi", "ludusavi", "0.31.0", serde_json::json!({"format": "tar.gz", "member": "ludusavi",
            "url": "https://github.com/mtkennerly/ludusavi/releases/download/v0.31.0/ludusavi-v0.31.0-linux.tar.gz",
            "sha256": "7322ff45d41eae7ae064a80d8c9ecccc5b8fb6fc090a603a66369cd4b054068d"})),
    }}))
    .expect("the built-in catalogue")
}

/// What stands in for the catalogue where it lacks an entry: the core's own, then each source's `[[tools]]`.
pub fn pinned() -> Catalogue {
    let mut out = builtin();
    let roots = paths::system_source_dirs().into_iter().rev().chain([paths::user_sources_dir()]);
    for (_, manifest) in crate::modules::read_manifests::<crate::sources::Manifest>(roots, "source.toml", |m| &m.id).into_values() {
        for tool in manifest.tools {
            out.components.entry(tool.id).or_insert(tool.entry);
        }
    }
    out
}

impl Catalogue {
    fn over_pinned(mut self) -> Catalogue {
        for (id, entry) in pinned().components {
            self.components.entry(id).or_insert(entry);
        }
        self
    }

    pub(crate) fn tool(&self, bin: &str) -> Option<(&String, &Entry)> {
        self.components.iter().find(|(_, e)| e.kind == Kind::Tool && e.bin == bin)
    }
}

#[cfg(target_arch = "x86_64")]
fn x86_64_v3() -> bool {
    is_x86_feature_detected!("avx2")
        && is_x86_feature_detected!("bmi1")
        && is_x86_feature_detected!("bmi2")
        && is_x86_feature_detected!("fma")
        && is_x86_feature_detected!("f16c")
        && is_x86_feature_detected!("lzcnt")
        && is_x86_feature_detected!("movbe")
}

#[cfg(not(target_arch = "x86_64"))]
fn x86_64_v3() -> bool {
    false
}

impl Build {
    pub fn asset(&self) -> Option<&Asset> {
        let fits = |a: &&Asset| a.arch == std::env::consts::ARCH || a.arch == "any";
        let v3 = x86_64_v3();
        self.assets.iter().filter(fits).find(|a| v3 && a.variant == "x86_64_v3").or_else(|| self.assets.iter().filter(fits).find(|a| a.variant.is_empty()))
    }
}

fn order(date: &str, version: &str) -> (String, Vec<(u64, String)>) {
    (date.to_string(), version_key(version))
}

impl Entry {
    pub fn latest(&self) -> Option<&Build> {
        let newest = |channel: Option<Channel>| {
            self.builds.iter().filter(|b| b.asset().is_some() && channel.is_none_or(|c| b.channel == c)).max_by_key(|b| order(&b.date, &b.version))
        };
        newest(Some(Channel::Stable)).or_else(|| newest(None))
    }

    pub fn build(&self, version: &str) -> Option<&Build> {
        self.builds.iter().find(|b| b.version == version)
    }
}

pub fn catalogue_url(config: &Config) -> String {
    std::env::var("UNIVERSE_CATALOGUE")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| Some(config.components.catalogue.clone()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| DEFAULT_CATALOGUE.into())
}

fn cache_file() -> PathBuf {
    paths::cache_home().join("catalogue.json")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cached {
    url: String,
    fetched_at: String,
    catalogue: Catalogue,
}

fn read_cache(url: &str) -> Option<(Cached, bool)> {
    let file = cache_file();
    let cached: Cached = serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()?;
    if cached.url != url {
        return None;
    }
    let fresh = std::fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < FRESH);
    Some((cached, fresh))
}

pub struct Loaded {
    pub catalogue: Catalogue,
    pub url: String,
    /// When the catalogue in hand was fetched; empty when there is none but the built-in one.
    pub fetched_at: String,
    pub error: String,
}

pub fn cached(config: &Config) -> Catalogue {
    read_cache(&catalogue_url(config)).map(|(c, _)| c.catalogue).unwrap_or_default().over_pinned()
}

pub async fn load(config: &Config, refresh: bool) -> Loaded {
    let url = catalogue_url(config);
    let cache = match read_cache(&url) {
        Some((c, true)) if !refresh => return Loaded { catalogue: c.catalogue.over_pinned(), url, fetched_at: c.fetched_at, error: String::new() },
        other => other,
    };
    match fetch_catalogue(&url).await {
        Ok(catalogue) => {
            let cached = Cached { url: url.clone(), fetched_at: now(), catalogue };
            if let Err(e) = serde_json::to_vec(&cached).map_err(Error::from).and_then(|b| write_atomic(&cache_file(), &b)) {
                tracing::warn!("catalogue cache: {e}");
            }
            Loaded { catalogue: cached.catalogue.over_pinned(), url, fetched_at: cached.fetched_at, error: String::new() }
        }
        Err(e) => {
            let (catalogue, fetched_at) = cache.map(|(c, _)| (c.catalogue, c.fetched_at)).unwrap_or_default();
            Loaded { catalogue: catalogue.over_pinned(), url, fetched_at, error: e.to_string() }
        }
    }
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .user_agent(concat!("universe/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| Error::Io(e.to_string()))
}

fn is_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

async fn fetch_catalogue(url: &str) -> Result<Catalogue> {
    let text = if is_url(url) {
        let failed = |e: reqwest::Error| Error::Unavailable(format!("the catalogue could not be fetched: {e}"));
        client()?.get(url).send().await.and_then(reqwest::Response::error_for_status).map_err(failed)?.text().await.map_err(failed)?
    } else {
        std::fs::read_to_string(paths::expand(url.strip_prefix("file://").unwrap_or(url))).map_err(|e| Error::Unavailable(format!("{url}: {e}")))?
    };
    let catalogue: Catalogue = serde_json::from_str(&text).map_err(|e| Error::Invalid(format!("the catalogue: {e}")))?;
    if catalogue.schema > SCHEMA {
        return Err(Error::Unavailable(format!("the catalogue is schema {}, this Universe reads {SCHEMA}: update Universe", catalogue.schema)));
    }
    Ok(catalogue)
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = path.with_extension("part");
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path)?;
    Ok(())
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub version: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub channel: Channel,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub bin: String,
    /// Under `dir`; empty for a Proton build, which is the folder itself.
    #[serde(default)]
    pub program: String,
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub disk: u64,
    pub installed_at: String,
    /// Installed by an update or on first use rather than asked for.
    #[serde(default)]
    pub auto: bool,
    #[serde(skip)]
    pub dir: PathBuf,
}

impl Installed {
    pub fn program_path(&self) -> PathBuf {
        if self.program.is_empty() {
            self.dir.clone()
        } else {
            self.dir.join(&self.program)
        }
    }
}

pub fn root() -> PathBuf {
    paths::data_home().join("components")
}

fn slot(version: &str) -> Result<String> {
    let s = version.trim().replace(['/', '\\'], "_");
    if s.is_empty() || s.starts_with('.') {
        return Err(Error::Invalid(format!("bad version '{version}'")));
    }
    Ok(s)
}

/// Oldest first. A folder without its sidecar is an install that did not finish.
pub fn installed(id: &str) -> Vec<Installed> {
    let dir = root().join(id);
    let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
    let mut out: Vec<Installed> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let mut i: Installed = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
            i.dir = dir.join(slot(&i.version).ok()?);
            i.dir.is_dir().then_some(i)
        })
        .collect();
    out.sort_by_key(|i| order(&i.date, &i.version));
    out
}

pub fn ids() -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(root()) else { return vec![] };
    let mut out: Vec<String> =
        rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
    out.sort();
    out
}

pub fn find_build(id: &str, version: &str) -> Option<Installed> {
    let mut builds = installed(id);
    if version.is_empty() || version == "latest" {
        builds.pop()
    } else {
        builds.into_iter().find(|b| b.version == version)
    }
}

pub fn proton_dirs() -> Vec<PathBuf> {
    ids().into_iter().filter(|id| installed(id).iter().any(|b| b.kind == Kind::Proton)).map(|id| root().join(id)).collect()
}

fn skipped_file(id: &str) -> PathBuf {
    root().join(id).join(".skipped")
}

pub fn skipped(id: &str) -> BTreeSet<String> {
    std::fs::read_to_string(skipped_file(id)).unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

fn set_skipped(id: &str, version: &str, skip: bool) -> Result<()> {
    let mut list = skipped(id);
    let changed = if skip { list.insert(version.to_string()) } else { list.remove(version) };
    if !changed {
        return Ok(());
    }
    write_atomic(&skipped_file(id), list.into_iter().collect::<Vec<_>>().join("\n").as_bytes())
}

/// `[runners.<id>] build`: `""` (the system's program first), `latest` or a version.
pub fn build_setting(config: &Config, id: &str) -> String {
    match config.runners.get(id).and_then(|t| t.get("build")) {
        Some(toml::Value::String(s)) => s.trim().to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn default_program(kind: Kind, asset: &Asset, bin: &str) -> String {
    if !asset.program.is_empty() {
        return asset.program.clone();
    }
    if asset.format == Format::AppImage || !asset.appimage.is_empty() {
        return "AppRun".into();
    }
    match kind {
        Kind::Wine => "bin/wine".into(),
        Kind::Tool => bin.into(),
        Kind::Proton | Kind::Emulator | Kind::System => String::new(),
    }
}

pub fn fhs() -> Option<PathBuf> {
    (crate::distro::detect() == crate::distro::Family::NixOs).then(|| runners::on_path("universe-fhs")).flatten()
}

fn set_executable(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    Ok(std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755))?)
}

fn appimage_extract(image: &Path, work: &Path) -> Result<PathBuf> {
    set_executable(image)?;
    let wrapper = fhs();
    let run = || {
        std::process::Command::new(wrapper.as_deref().unwrap_or(image))
            .args(wrapper.is_some().then_some(image))
            .arg("--appimage-extract")
            .current_dir(work)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
    };
    // ETXTBSY: a fork elsewhere in the process can still hold the file's write descriptor for an instant.
    let mut attempt = run();
    for _ in 0..20 {
        match &attempt {
            Err(e) if e.raw_os_error() == Some(libc::ETXTBSY) => std::thread::sleep(Duration::from_millis(50)),
            _ => break,
        }
        attempt = run();
    }
    let out = attempt.map_err(|e| Error::Io(format!("{}: {e}", image.display())))?;
    let root = work.join("squashfs-root");
    if !out.status.success() || !root.is_dir() {
        let why = String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("").to_string();
        return Err(Error::Io(format!("{} did not unpack ({}): {why}", image.display(), out.status)));
    }
    // uruntime (Eden's, the anylinux builds) unpacks into AppDir: squashfs-root only links to it.
    Ok(root.canonicalize()?)
}

fn single_child(dir: &Path) -> PathBuf {
    let entries: Vec<PathBuf> = std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    match entries.as_slice() {
        [only] if only.symlink_metadata().is_ok_and(|m| m.is_dir()) => only.clone(),
        _ => dir.to_path_buf(),
    }
}

fn file_name(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).split('?').next().unwrap_or("").to_string()
}

fn unpack(asset: &Asset, kind: Kind, bin: &str, file: &Path, staging: &Path) -> Result<(PathBuf, String)> {
    use std::io::{BufReader, Read};
    std::fs::create_dir_all(staging)?;
    let program = default_program(kind, asset, bin);
    let single = || if program.is_empty() { file_name(&asset.url) } else { program.clone() };
    let mut root = match asset.format {
        Format::Binary => {
            let dest = staging.join(single());
            std::fs::copy(file, &dest)?;
            set_executable(&dest)?;
            staging.to_path_buf()
        }
        Format::AppImage => appimage_extract(file, staging)?,
        Format::Tar | Format::TarGz | Format::TarXz => {
            let f = std::fs::File::open(file)?;
            let reader: Box<dyn Read> = match asset.format {
                Format::TarGz => Box::new(flate2::read::GzDecoder::new(f)),
                Format::TarXz => Box::new(liblzma::read::XzDecoder::new(f)),
                _ => Box::new(f),
            };
            let mut archive = tar::Archive::new(BufReader::new(reader));
            archive.set_preserve_permissions(true);
            archive.set_overwrite(true);
            if asset.member.is_empty() {
                archive.unpack(staging)?;
                single_child(staging)
            } else {
                let mut entry = archive
                    .entries()?
                    .flatten()
                    .find(|e| e.path().is_ok_and(|p| p.as_os_str() == asset.member.as_str()))
                    .ok_or_else(|| Error::Io(format!("{}: no {} inside", asset.url, asset.member)))?;
                let dest = staging.join(single());
                entry.unpack(&dest)?;
                set_executable(&dest)?;
                staging.to_path_buf()
            }
        }
        Format::Zip => {
            let zipped = |e: zip::result::ZipError| Error::Io(format!("{}: {e}", asset.url));
            zip::ZipArchive::new(std::fs::File::open(file)?).map_err(zipped)?.extract(staging).map_err(zipped)?;
            single_child(staging)
        }
    };
    if !asset.appimage.is_empty() {
        let image = root.join(&asset.appimage);
        if !image.is_file() {
            return Err(Error::Io(format!("{}: no {} inside", asset.url, asset.appimage)));
        }
        root = appimage_extract(&image, staging)?;
    }
    let what = if kind == Kind::Proton { "proton" } else { program.as_str() };
    if what.is_empty() || !root.join(what).metadata().is_ok_and(|m| kind != Kind::Proton || m.is_file()) {
        return Err(Error::Io(format!("{}: unpacked, but no {what} in it", asset.url)));
    }
    Ok((root, program))
}

fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1_000_000.0)
}

fn check_space(dir: &Path, asset: &Asset) -> Result<()> {
    if asset.size == 0 {
        return Ok(());
    }
    let unpacked_per_download = match asset.format {
        Format::Binary => 2,
        Format::Tar if !asset.member.is_empty() => 2,
        Format::AppImage => 3,
        _ => 4,
    };
    let need = asset.size.saturating_mul(unpacked_per_download);
    let st = rustix::fs::statvfs(dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
    let free = st.f_bavail.saturating_mul(st.f_frsize);
    if free < need {
        return Err(Error::Unavailable(format!("{} free under {}, {} needed", megabytes(free), dir.display(), megabytes(need))));
    }
    Ok(())
}

async fn download(url: &str, dest: &Path, sha256: &str, size: u64, label: &str, progress: &mut Option<Progress<'_, '_>>, cancel: &AtomicBool) -> Result<()> {
    use sha2::Digest;
    use tokio::io::AsyncWriteExt;
    let failed = |e: reqwest::Error| Error::Unavailable(format!("{label} could not be downloaded: {e}"));
    let mut resp = client()?.get(url).send().await.and_then(reqwest::Response::error_for_status).map_err(failed)?;
    let total = resp.content_length().filter(|n| *n > 0).unwrap_or(size);
    let mut file = tokio::fs::File::create(dest).await?;
    let mut hasher = sha2::Sha256::new();
    let (mut done, mut shown) = (0u64, None);
    let result: Result<()> = async {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Busy(format!("{label}: cancelled")));
            }
            let Some(chunk) = resp.chunk().await.map_err(failed)? else { break };
            hasher.update(&chunk);
            file.write_all(&chunk).await?;
            done += chunk.len() as u64;
            let percent = (done * 100).checked_div(total).unwrap_or(0);
            if shown != Some(percent) {
                shown = Some(percent);
                if let Some(p) = progress.as_mut() {
                    let of = if total > 0 { format!(" of {}", megabytes(total)) } else { String::new() };
                    p(done, total, &format!("Downloading {label} · {}{of}", megabytes(done)));
                }
            }
        }
        file.flush().await?;
        file.sync_all().await?;
        Ok(())
    }
    .await;
    drop(file);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let result = match result {
        Ok(()) if !digest.eq_ignore_ascii_case(sha256.trim()) => Err(Error::Io(format!("{url}: sha256 {digest}, not the pinned {sha256}"))),
        r => r,
    };
    if result.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    result
}

fn relink(id: &str, kind: Kind, bin: &str) -> Result<()> {
    if kind != Kind::Tool || bin.is_empty() || bin.contains('/') {
        return Ok(());
    }
    let link = crate::tools::dir().join(bin);
    match find_build(id, "") {
        Some(b) => {
            std::fs::create_dir_all(crate::tools::dir())?;
            let part = crate::tools::dir().join(format!(".{bin}.link"));
            let _ = std::fs::remove_file(&part);
            std::os::unix::fs::symlink(b.program_path(), &part)?;
            std::fs::rename(&part, &link)?;
        }
        None => {
            if std::fs::read_link(&link).is_ok_and(|to| to.starts_with(root())) {
                std::fs::remove_file(&link)?;
            }
        }
    }
    Ok(())
}

pub async fn install(id: &str, entry: &Entry, build: &Build, auto: bool, mut progress: Option<Progress<'_, '_>>, cancel: &AtomicBool) -> Result<Installed> {
    if let Some(i) = installed(id).into_iter().find(|i| i.version == build.version) {
        relink(id, entry.kind, &entry.bin)?;
        return Ok(i);
    }
    let label = format!("{} {}", entry.name, build.version);
    let asset = build.asset().ok_or_else(|| Error::Unavailable(format!("{label}: no build for {}", std::env::consts::ARCH)))?.clone();
    let slot = slot(&build.version)?;
    let home = root().join(id);
    let tmp = root().join(".tmp");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&tmp)?;
    check_space(&tmp, &asset)?;
    let part = tmp.join(format!("{id}-{slot}.part"));
    download(&asset.url, &part, &asset.sha256, asset.size, &label, &mut progress, cancel).await?;
    if let Some(p) = progress.as_mut() {
        p(1, 1, &format!("Unpacking {label}"));
    }
    let staging = tmp.join(format!("{id}-{slot}"));
    let _ = std::fs::remove_dir_all(&staging);
    let unpacked = {
        let (asset, part, staging, kind, bin) = (asset.clone(), part.clone(), staging.clone(), entry.kind, entry.bin.clone());
        crate::core::blocking(move || unpack(&asset, kind, &bin, &part, &staging)).await
    };
    let _ = std::fs::remove_file(&part);
    let (unpacked_root, program) = unpacked.inspect_err(|_| {
        let _ = std::fs::remove_dir_all(&staging);
    })?;
    let dest = home.join(&slot);
    let _ = std::fs::remove_dir_all(&dest);
    let moved = std::fs::rename(&unpacked_root, &dest);
    let _ = std::fs::remove_dir_all(&staging);
    moved?;
    let installed = Installed {
        id: id.into(),
        name: entry.name.clone(),
        kind: entry.kind,
        version: build.version.clone(),
        date: build.date.clone(),
        channel: build.channel,
        family: entry.family.clone(),
        bin: entry.bin.clone(),
        program,
        url: asset.url.clone(),
        sha256: asset.sha256.clone(),
        size: asset.size,
        disk: crate::data::disk_usage(&dest),
        installed_at: now(),
        auto,
        dir: dest,
    };
    write_atomic(&home.join(format!("{slot}.json")), &serde_json::to_vec_pretty(&installed)?)?;
    relink(id, entry.kind, &entry.bin)?;
    tracing::info!("{label} installed in {}", installed.dir.display());
    Ok(installed)
}

/// The sidecar goes first: a folder left half removed is no build.
fn remove_build(b: &Installed) -> Result<()> {
    let sidecar = b.dir.with_file_name(format!("{}.json", slot(&b.version)?));
    std::fs::remove_file(&sidecar)?;
    Ok(std::fs::remove_dir_all(&b.dir)?)
}

pub fn update_of<'a>(builds: &[Installed], id: &str, entry: &'a Entry) -> Option<&'a Build> {
    let newest = builds.last()?;
    let latest = entry.latest()?;
    let fresh = !builds.iter().any(|b| b.version == latest.version) && !skipped(id).contains(&latest.version);
    (fresh && order(&latest.date, &latest.version) > order(&newest.date, &newest.version)).then_some(latest)
}

pub fn prune(id: &str, pinned: &BTreeSet<String>) -> Result<Vec<String>> {
    let builds = installed(id);
    let keep_from = builds.len().saturating_sub(2);
    let mut removed = Vec::new();
    for b in builds.iter().take(keep_from).filter(|b| !pinned.contains(&b.version)) {
        remove_build(b)?;
        removed.push(b.version.clone());
    }
    Ok(removed)
}

fn version_under(id: &str, path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let p = canonical(&paths::expand(path));
    let rel = p.strip_prefix(canonical(&root().join(id))).ok()?.components().next()?.as_os_str().to_string_lossy().into_owned();
    installed(id).into_iter().find(|b| slot(&b.version).is_ok_and(|s| s == rel)).map(|b| b.version)
}

pub fn pins(id: &str, kind: Kind, config: &Config, games: &[Resolved], running: Option<&str>) -> BTreeSet<String> {
    let versions: BTreeSet<String> = installed(id).into_iter().map(|b| b.version).collect();
    let mut out = BTreeSet::new();
    let mut named = |v: &str| {
        if versions.contains(v) {
            out.insert(v.to_string());
        }
    };
    match kind {
        Kind::Emulator | Kind::Wine => {
            named(&build_setting(config, id));
            let exe = config.runners.get(id).and_then(|t| t.get("exe")).and_then(|v| v.as_str()).unwrap_or("");
            let mut under: Vec<String> = version_under(id, exe).into_iter().collect();
            for g in games.iter().filter(|g| g.game.runner_id() == id) {
                named(&g.game.launch.runner_build);
                under.extend(version_under(id, &g.game.launch.runner_exe));
                if running == Some(g.game.id.as_str()) {
                    under.extend(version_under(id, &g.effective.runner_path));
                }
            }
            under.iter().for_each(|v| named(v));
        }
        Kind::Proton => {
            named(&config.launch.proton);
            let mut under: Vec<String> = config.proton.values().filter_map(|p| version_under(id, p)).collect();
            for g in games {
                named(&g.game.launch.proton);
                if running == Some(g.game.id.as_str()) {
                    under.extend(version_under(id, &g.effective.proton_path));
                }
            }
            under.iter().for_each(|v| named(v));
        }
        Kind::Tool | Kind::System => {}
    }
    out
}

/// `None` when unsure: a snapshot build, or two numbering schemes.
pub(crate) fn newer(candidate: &str, current: &str) -> Option<bool> {
    let norm = |s: &str| s.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
    let (a, b) = (norm(candidate), norm(current));
    if a.is_empty() || b.is_empty() || [&a, &b].iter().any(|s| s.contains("unstable") || s.contains("git")) {
        return None;
    }
    let (ka, kb) = (version_key(&a), version_key(&b));
    let lead = |k: &[(u64, String)]| k.first().map(|(_, t)| t.clone()).unwrap_or_default();
    (lead(&ka) == lead(&kb)).then(|| ka > kb)
}

fn name_version(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split('-').collect();
    let i = parts.iter().position(|p| p.starts_with(|c: char| c.is_ascii_digit()))?;
    Some(parts[i..].join("-"))
}

fn nix_version(real: &Path) -> Option<String> {
    let store = real.strip_prefix("/nix/store").ok()?.components().next()?.as_os_str().to_string_lossy().into_owned();
    name_version(store.split_once('-')?.1)
}

fn wrapper_copied_from(script: &str, name: &str) -> Option<PathBuf> {
    let target = format!("\"$wrapperDir/{name}\"");
    script.lines().find_map(|l| Some(PathBuf::from(l.strip_prefix("cp ")?.strip_suffix(target.as_str())?.trim())))
}

// /run/wrappers holds root-only copies of store binaries: the unit that makes them names each source.
fn wrapper_source(name: &str) -> Option<PathBuf> {
    let unit = std::fs::read_to_string("/etc/systemd/system/suid-sgid-wrappers.service").ok()?;
    let start = unit.lines().find_map(|l| l.strip_prefix("ExecStart="))?.trim_start_matches(['@', '-', '+', '!']).split_whitespace().next()?;
    wrapper_copied_from(&std::fs::read_to_string(start).ok()?, name)
}

fn embedded_program(file: &Path, bin: &str) -> Option<PathBuf> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(file).ok()?.take(4 << 20).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let (plain, wrapped) = (format!("/bin/{bin}"), format!("/bin/.{bin}-wrapped"));
    text.match_indices("/nix/store/").find_map(|(i, _)| {
        let path: String = text[i..].chars().take_while(|c| c.is_ascii_alphanumeric() || "/._+-".contains(*c)).collect();
        (path.ends_with(&plain) || path.ends_with(&wrapped)).then(|| PathBuf::from(path)).filter(|p| p != file)
    })
}

fn store_program(program: &Path) -> Option<PathBuf> {
    let bin = program.file_name()?.to_str()?;
    let mut path = if program.starts_with("/run/wrappers") { wrapper_source(bin)? } else { canonical(program) };
    for _ in 0..4 {
        if !path.starts_with("/nix/store") {
            return None;
        }
        if nix_version(&path).is_some() {
            break;
        }
        match embedded_program(&path, bin) {
            Some(next) => path = canonical(&next),
            None => break,
        }
    }
    path.starts_with("/nix/store").then_some(path)
}

fn package_version(real: &Path) -> Option<String> {
    use crate::distro::Family;
    let path = real.as_os_str();
    let strip = |v: &str| {
        let v = v.split_once(':').map(|(_, rest)| rest).unwrap_or(v);
        v.rsplit_once('-').map(|(ver, _)| ver).unwrap_or(v).to_string()
    };
    match crate::distro::detect() {
        Family::Arch => {
            let line = crate::keyboard::output("pacman", &["-Qo".as_ref(), path])?;
            line.trim().rsplit(' ').next().map(strip)
        }
        Family::Debian => {
            let owner = crate::keyboard::output("dpkg-query", &["-S".as_ref(), path])?;
            let pkg = owner.split(':').next()?.trim().to_string();
            crate::keyboard::output("dpkg-query", &["-W", "-f=${Version}", &pkg]).map(|v| strip(v.trim()))
        }
        Family::Fedora => crate::keyboard::output("rpm", &["-qf".as_ref(), "--qf".as_ref(), "%{VERSION}".as_ref(), path]).map(|v| v.trim().to_string()),
        Family::NixOs | Family::Other => None,
    }
}

pub fn system_version(program: &Path) -> String {
    if let Some(v) = store_program(program).and_then(|p| nix_version(&p)) {
        return v;
    }
    let real = canonical(program);
    let name = real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if name.to_ascii_lowercase().ends_with(".appimage") {
        return name_version(&name)
            .map(|v| v.split(['.', '-']).take_while(|p| p.starts_with(|c: char| c.is_ascii_digit())).collect::<Vec<_>>().join("."))
            .unwrap_or_default();
    }
    package_version(&real).unwrap_or_default()
}

fn origin(program: &Path, fallback: &str) -> String {
    if store_program(program).is_some() { "nix" } else { fallback }.into()
}

fn proton_version(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join("version")).ok().and_then(|s| s.split_whitespace().last().map(String::from)).unwrap_or_else(|| name.to_string())
}

#[derive(Debug, Clone)]
pub struct Found {
    pub name: String,
    pub version: String,
    pub origin: String,
    pub program: String,
}

fn found_at(p: &Path, fallback: &str) -> Found {
    Found { name: String::new(), version: system_version(p), origin: origin(p, fallback), program: p.to_string_lossy().into() }
}

fn found_json(f: &Found, in_use: bool) -> serde_json::Value {
    serde_json::json!({
        "version": f.version, "name": f.name, "origin": f.origin, "program": f.program, "managed": false,
        "in_use": in_use, "pinned": false, "disk": 0, "size": 0, "date": "", "installed_at": "", "auto": false,
    })
}

fn proton_found(config: &Config) -> Vec<Found> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut add = |dir: PathBuf, name: String, owner: &str| {
        if dir.join("proton").is_file() && seen.insert(canonical(&dir)) {
            out.push(Found { version: proton_version(&dir, &name), origin: origin(&dir, owner), program: dir.to_string_lossy().into_owned(), name });
        }
    };
    for (home, owner) in config.proton_homes() {
        for e in std::fs::read_dir(&home).into_iter().flatten().flatten() {
            add(e.path(), e.file_name().to_string_lossy().into_owned(), owner);
        }
    }
    for (name, path) in &config.proton {
        add(paths::expand(path), name.clone(), "config");
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct Usage {
    pub runners: BTreeMap<String, usize>,
    pub protons: Vec<PathBuf>,
}

impl Usage {
    pub fn of(games: &[Resolved]) -> Usage {
        let mut usage = Usage::default();
        for g in games.iter().filter(|g| g.game.removed_at.is_empty()) {
            *usage.runners.entry(g.effective.runner.clone()).or_default() += 1;
            let proton = runners::spec(&g.effective.runner).is_some_and(|s| s.kind == runners::Kind::Proton || s.via_proton);
            if proton && !g.effective.proton_path.is_empty() {
                usage.protons.push(canonical(Path::new(&g.effective.proton_path)));
            }
        }
        usage
    }

    fn proton_games(&self) -> usize {
        runners::RUNNERS.iter().filter(|s| s.kind == runners::Kind::Proton || s.via_proton).map(|s| self.runners.get(s.id).copied().unwrap_or(0)).sum()
    }
}

fn rows(catalogue: &Catalogue) -> Vec<(String, Entry)> {
    let mut all = catalogue.components.clone();
    for spec in runners::RUNNERS.iter().filter(|s| matches!(s.kind, runners::Kind::Emulator | runners::Kind::Wine)) {
        let kind = if spec.kind == runners::Kind::Wine { Kind::Wine } else { Kind::Emulator };
        all.entry(spec.id.into()).or_insert(Entry {
            name: spec.name.into(),
            kind,
            family: String::new(),
            bin: String::new(),
            homepage: String::new(),
            notice: String::new(),
            builds: vec![],
        });
    }
    let held: Vec<String> = ids().into_iter().filter(|id| !all.contains_key(id)).collect();
    for id in held {
        if let Some(b) = installed(&id).pop() {
            all.insert(id, Entry { name: b.name, kind: b.kind, family: b.family, bin: b.bin, homepage: String::new(), notice: String::new(), builds: vec![] });
        }
    }
    let mut rows: Vec<(String, Entry)> = all.into_iter().collect();
    rows.sort_by_key(|(_, e)| (e.kind, e.name.to_lowercase()));
    rows
}

fn build_json(b: &Build) -> serde_json::Value {
    let size = b.asset().map(|a| a.size).unwrap_or(0);
    serde_json::json!({"version": b.version, "date": b.date, "channel": b.channel, "size": size})
}

fn row_json(id: &str, entry: &Entry, config: &Config, usage: &Usage, protons: &[Found], pinned: &BTreeSet<String>) -> serde_json::Value {
    let spec = runners::spec(id).filter(|s| s.id == id);
    let ours = installed(id);
    let skipped = skipped(id);
    let found: Vec<Found> = match entry.kind {
        Kind::Emulator | Kind::Wine => spec.and_then(runners::system_program).map(|p| found_at(&p, "system")).into_iter().collect(),
        Kind::Proton => protons.iter().filter(|f| !entry.family.is_empty() && crate::config::of_family(&f.name, &entry.family)).cloned().collect(),
        Kind::Tool | Kind::System => runners::on_system_path(&entry.bin).map(|p| found_at(&p, "system")).into_iter().collect(),
    };
    let in_use_path: Option<PathBuf> = match entry.kind {
        Kind::Emulator | Kind::Wine => spec.map(|s| runners::locate(s, config).program).filter(|p| !p.is_empty()).map(PathBuf::from),
        Kind::Proton => config.proton_path(&config.launch.proton),
        Kind::Tool | Kind::System => runners::on_path(&entry.bin),
    }
    .map(|p| canonical(&p));
    let is_used = |program: &Path| in_use_path.as_ref().is_some_and(|u| *u == canonical(program));
    let recent_since = chrono::Local::now() - chrono::Duration::days(RECENT_DAYS);
    let mut builds: Vec<serde_json::Value> = ours
        .iter()
        .rev()
        .map(|b| {
            serde_json::json!({
                "version": b.version, "origin": "universe", "program": b.program_path(), "managed": true,
                "in_use": is_used(&b.program_path()), "pinned": pinned.contains(&b.version), "disk": b.disk, "size": b.size,
                "date": b.date, "installed_at": b.installed_at, "auto": b.auto,
            })
        })
        .collect();
    builds.extend(found.iter().map(|f| found_json(f, is_used(Path::new(&f.program)))));
    let in_use = builds.iter().find(|b| b["in_use"] == true).cloned();
    let latest = entry.latest();
    let managed_latest = latest.is_some_and(|l| ours.iter().any(|b| b.version == l.version));
    let used_by = match entry.kind {
        Kind::Emulator | Kind::Wine => usage.runners.get(id).copied().unwrap_or(0),
        Kind::Proton => {
            let dirs: Vec<PathBuf> = ours.iter().map(|b| canonical(&b.dir)).chain(found.iter().map(|f| canonical(Path::new(&f.program)))).collect();
            usage.protons.iter().filter(|p| dirs.contains(p)).count()
        }
        Kind::Tool | Kind::System => 0,
    };
    let has_any = !ours.is_empty() || !found.is_empty();
    let wanted = match entry.kind {
        Kind::Emulator | Kind::Wine => used_by > 0,
        Kind::Proton => usage.proton_games() > 0 && crate::config::of_family(&config.launch.proton, &entry.family),
        Kind::Tool => entry.bin == "umu-run" && usage.proton_games() > 0,
        Kind::System => false,
    };
    let newer_than_used = |l: &Build, u: &serde_json::Value| newer(&l.version, u["version"].as_str().unwrap_or("")) == Some(true);
    let proposal = match (latest, &in_use) {
        (Some(_), _) if !has_any && wanted => "install",
        (Some(l), Some(u))
            if entry.kind != Kind::Tool && u["managed"] == false && !managed_latest && !skipped.contains(&l.version) && newer_than_used(l, u) =>
        {
            "newer"
        }
        _ => "",
    };
    let recent = ours
        .iter()
        .rev()
        .find(|b| b.auto && chrono::DateTime::parse_from_rfc3339(&b.installed_at).is_ok_and(|t| t > recent_since))
        .map(|b| serde_json::json!({"version": b.version, "at": b.installed_at}));
    let mut available: Vec<&Build> = entry.builds.iter().filter(|b| b.asset().is_some()).collect();
    available.sort_by_key(|b| std::cmp::Reverse(order(&b.date, &b.version)));
    let available: Vec<serde_json::Value> = available
        .into_iter()
        .map(|b| {
            let mut j = build_json(b);
            j["installed"] = ours.iter().any(|i| i.version == b.version).into();
            j["skipped"] = skipped.contains(&b.version).into();
            j
        })
        .collect();
    let setting = match entry.kind {
        Kind::Emulator | Kind::Wine => build_setting(config, id),
        Kind::Proton if config.launch.proton == entry.family || crate::config::of_family(&config.launch.proton, &entry.family) => config.launch.proton.clone(),
        _ => String::new(),
    };
    serde_json::json!({
        "id": id, "name": entry.name, "kind": entry.kind, "family": entry.family, "bin": entry.bin, "homepage": entry.homepage, "notice": entry.notice,
        "runner": if spec.is_some() { id } else { "" },
        "builds": builds, "in_use": in_use, "latest": latest.map(build_json), "available": available,
        "update": update_of(&ours, id, entry).map(|b| b.version.clone()).unwrap_or_default(),
        "proposal": proposal, "used_by": used_by, "setting": setting, "recent": recent,
        "skipped": skipped.into_iter().collect::<Vec<_>>(),
    })
}

pub struct SystemTool {
    pub id: &'static str,
    pub name: &'static str,
    pub bin: &'static str,
    arch: &'static [&'static str],
    fedora: &'static [&'static str],
    debian: &'static [&'static str],
    nixos: &'static str,
}

pub const SYSTEM: &[SystemTool] = &[
    SystemTool {
        id: "gamescope",
        name: "gamescope",
        bin: "gamescope",
        arch: &["gamescope"],
        fedora: &["gamescope"],
        debian: &["gamescope"],
        nixos: "set programs.universe.gamescope.enable in the NixOS module",
    },
    SystemTool {
        id: "mangohud",
        name: "MangoHud",
        bin: "mangohud",
        arch: &["mangohud", "lib32-mangohud"],
        fedora: &["mangohud", "mangohud.i686"],
        debian: &["mangohud", "mangohud:i386"],
        nixos: "add pkgs.mangohud to your packages",
    },
    SystemTool {
        id: "gpu-screen-recorder",
        name: "gpu-screen-recorder",
        bin: "gpu-screen-recorder",
        arch: &["gpu-screen-recorder"],
        fedora: &[],
        debian: &[],
        nixos: "set programs.universe.capture.enable in the NixOS module",
    },
];

impl SystemTool {
    pub fn packages(&self, family: crate::distro::Family) -> &'static [&'static str] {
        use crate::distro::Family;
        match family {
            Family::Arch => self.arch,
            Family::Fedora => self.fedora,
            Family::Debian => self.debian,
            Family::NixOs | Family::Other => &[],
        }
    }

    /// `asked`: the tools the enabled modules' `[requires] system` names.
    fn wanted(&self, config: &Config, asked: &[String]) -> bool {
        match self.id {
            "gamescope" => config.launch.gamescope,
            "mangohud" => config.launch.mangohud || config.launch.fps_limit != "none",
            id => asked.iter().any(|a| a == id),
        }
    }
}

pub fn system_tool(id: &str) -> Option<&'static SystemTool> {
    SYSTEM.iter().find(|t| t.id == id)
}

fn system_json(tool: &SystemTool, config: &Config, asked: &[String], family: crate::distro::Family, packagekit: bool) -> serde_json::Value {
    let packages = tool.packages(family);
    let installable = packagekit && !packages.is_empty();
    let build = runners::on_system_path(tool.bin).map(|p| found_json(&found_at(&p, "system"), true));
    let fix = match (family, installable) {
        (crate::distro::Family::NixOs, _) => tool.nixos.to_string(),
        (_, true) => String::new(),
        (_, false) if packages.is_empty() => format!("install {} from its project: your distribution does not package it", tool.name),
        (_, false) => format!("install {} with your package manager", packages.join(" and ")),
    };
    serde_json::json!({
        "id": tool.id, "name": tool.name, "kind": Kind::System, "family": "", "bin": tool.bin, "homepage": "", "notice": "", "runner": "",
        "builds": build.iter().collect::<Vec<_>>(), "in_use": build, "latest": null, "available": [], "update": "",
        "proposal": if build.is_none() && installable && tool.wanted(config, asked) { "install" } else { "" },
        "used_by": 0, "setting": "", "recent": null, "skipped": [],
        "packages": packages, "installable": installable, "fix": fix,
    })
}

/// Runs the package manager to read a system program's version: off the runtime. `asked`: the system tools the enabled modules require.
pub fn list(config: &Config, catalogue: &Catalogue, games: &[Resolved], running: Option<&str>, asked: &[String], packagekit: bool) -> Vec<serde_json::Value> {
    let usage = Usage::of(games);
    let protons = proton_found(config);
    let family = crate::distro::detect();
    let mut out: Vec<serde_json::Value> =
        rows(catalogue).iter().map(|(id, entry)| row_json(id, entry, config, &usage, &protons, &pins(id, entry.kind, config, games, running))).collect();
    out.extend(SYSTEM.iter().map(|tool| system_json(tool, config, asked, family, packagekit)));
    out
}

pub(crate) struct JobGuard<'a> {
    core: &'a Core,
    id: String,
    pub cancel: Arc<AtomicBool>,
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        self.core.component_jobs.lock().unwrap().remove(&self.id);
    }
}

impl Core {
    fn component_job(&self, id: &str) -> Result<JobGuard<'_>> {
        let mut jobs = self.component_jobs.lock().unwrap();
        if jobs.contains_key(id) {
            return Err(Error::Busy(format!("{id} is being installed or updated")));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.insert(id.to_string(), cancel.clone());
        Ok(JobGuard { core: self, id: id.to_string(), cancel })
    }

    pub async fn components(&self, refresh: bool) -> Result<serde_json::Value> {
        let config = self.config.read().await.clone();
        let auto_update = config.components.auto_update;
        let Loaded { catalogue, url, fetched_at, error } = load(&config, refresh).await;
        let generated_at = catalogue.generated_at.clone();
        let games = self.games.read().await.clone();
        let running = self.current().await.map(|c| c.id);
        let packagekit = crate::system_install::available().await;
        let asked: Vec<String> = self.modules.read().await.iter().filter(|m| m.enabled).flat_map(|m| m.manifest.requires.system.clone()).collect();
        let listed = crate::core::blocking(move || Ok(list(&config, &catalogue, &games, running.as_deref(), &asked, packagekit))).await?;
        Ok(serde_json::json!({
            "catalogue": {"url": url, "fetched_at": fetched_at, "generated_at": generated_at, "error": error},
            "auto_update": auto_update,
            "components": listed,
        }))
    }

    fn entry_of<'a>(loaded: &'a Loaded, id: &str) -> Result<&'a Entry> {
        loaded.catalogue.components.get(id).ok_or_else(|| {
            let why = if loaded.error.is_empty() { String::new() } else { format!(" ({})", loaded.error) };
            Error::NotFound(format!("{id} is not in the catalogue{why}"))
        })
    }

    /// `accepted`: the entry's notice was shown and accepted; an entry with one is refused without it.
    pub async fn component_install(&self, id: &str, version: &str, accepted: bool, progress: Option<Progress<'_, '_>>) -> Result<String> {
        if let Some(tool) = system_tool(id) {
            let _job = self.component_job(id)?;
            match crate::system_install::helper() {
                Some(helper) => crate::system_install::install(&helper, tool, progress).await?,
                None => crate::packagekit::install(tool.packages(crate::distro::detect()), tool.name, progress).await?,
            }
            return Ok(runners::on_system_path(tool.bin).map(|p| system_version(&p)).unwrap_or_default());
        }
        let config = self.config.read().await.clone();
        let loaded = load(&config, false).await;
        let entry = Self::entry_of(&loaded, id)?;
        if !entry.notice.is_empty() && !accepted {
            return Err(Error::Invalid(format!("{} waits for its notice to be accepted: {}", entry.name, entry.notice)));
        }
        let build = if version.is_empty() { entry.latest() } else { entry.build(version) }
            .ok_or_else(|| Error::NotFound(format!("{} {version}: no such build in the catalogue", entry.name)))?;
        let job = self.component_job(id)?;
        let installed = install(id, entry, build, false, progress, &job.cancel).await?;
        set_skipped(id, &installed.version, false)?;
        drop(job);
        self.reload_all().await;
        Ok(installed.version)
    }

    async fn pinned(&self, id: &str, kind: Kind) -> BTreeSet<String> {
        let config = self.config.read().await.clone();
        let running = self.current().await.map(|c| c.id);
        pins(id, kind, &config, &self.games.read().await, running.as_deref())
    }

    pub async fn component_remove(&self, id: &str, version: &str) -> Result<()> {
        let b = find_build(id, version).filter(|_| !version.is_empty()).ok_or_else(|| Error::NotFound(format!("{id} {version} is not installed")))?;
        if self.pinned(id, b.kind).await.contains(version) {
            return Err(Error::Busy(format!("{id} {version} is in use: pick another build for what names it first")));
        }
        let _job = self.component_job(id)?;
        remove_build(&b)?;
        relink(id, b.kind, &b.bin)?;
        self.reload_all().await;
        Ok(())
    }

    /// Returns the versions removed, oldest first.
    pub async fn component_uninstall(&self, id: &str) -> Result<Vec<String>> {
        let builds = installed(id);
        let newest = builds.last().cloned().ok_or_else(|| Error::NotFound(format!("Universe holds no build of {id}")))?;
        let mut config = self.config.read().await.clone();
        let chosen = match newest.kind {
            Kind::Emulator | Kind::Wine => config.runners.get_mut(id).and_then(|t| t.remove("build")).is_some(),
            Kind::Proton if builds.iter().any(|b| b.version == config.launch.proton) => {
                config.launch.proton = newest.family.clone();
                true
            }
            _ => false,
        };
        let running = self.current().await;
        if let Some(c) = running.as_ref().filter(|_| newest.kind == Kind::Tool) {
            return Err(Error::Busy(format!("{} is running: uninstalling {} waits until it ends", c.title, newest.name)));
        }
        let pinned = pins(id, newest.kind, &config, &self.games.read().await, running.map(|c| c.id).as_deref());
        if let Some(v) = pinned.first() {
            return Err(Error::Busy(format!("{id} {v} is in use: pick another build for what names it first")));
        }
        let _job = self.component_job(id)?;
        if chosen {
            self.component_use(id, "").await?;
        }
        for b in &builds {
            remove_build(b)?;
        }
        std::fs::remove_dir_all(root().join(id))?;
        relink(id, newest.kind, &newest.bin)?;
        self.reload_all().await;
        Ok(builds.into_iter().map(|b| b.version).collect())
    }

    pub async fn component_update(&self, id: &str, mut progress: Option<Progress<'_, '_>>) -> Result<Vec<serde_json::Value>> {
        if let Some(c) = self.current().await {
            return Err(Error::Busy(format!("{} is running: updates wait until it ends", c.title)));
        }
        let config = self.config.read().await.clone();
        let loaded = load(&config, false).await;
        // Compat: a tool fetched before components is a plain file in <data>/bin.
        let fetched_before = |e: &Entry| e.kind == Kind::Tool && crate::tools::dir().join(&e.bin).symlink_metadata().is_ok_and(|m| m.is_file());
        let mut targets: Vec<String> = if id.is_empty() { ids() } else { vec![id.to_string()] };
        if id.is_empty() {
            let legacy =
                loaded.catalogue.components.iter().filter(|(i, e)| fetched_before(e) && !targets.contains(i)).map(|(i, _)| i.clone()).collect::<Vec<_>>();
            targets.extend(legacy);
        }
        let mut out = Vec::new();
        for target in targets {
            let Some(entry) = loaded.catalogue.components.get(&target) else { continue };
            let legacy = installed(&target).is_empty() && fetched_before(entry);
            let build = update_of(&installed(&target), &target, entry).or_else(|| entry.latest().filter(|_| legacy)).cloned();
            if let Some(build) = build {
                let job = self.component_job(&target)?;
                match install(&target, entry, &build, true, progress.as_deref_mut(), &job.cancel).await {
                    Ok(_) => out.push(serde_json::json!({"id": target, "name": entry.name, "version": build.version})),
                    Err(e) if !id.is_empty() => return Err(e),
                    Err(e) => {
                        tracing::warn!("{} {}: {e}", entry.name, build.version);
                        out.push(serde_json::json!({"id": target, "name": entry.name, "version": build.version, "error": e.to_string()}));
                    }
                }
            }
            let pinned = self.pinned(&target, entry.kind).await;
            for v in prune(&target, &pinned)? {
                tracing::info!("{} {v} removed: two newer builds are kept", entry.name);
            }
            relink(&target, entry.kind, &entry.bin)?;
        }
        self.reload_all().await;
        Ok(out)
    }

    /// Returns the version now newest, `""` when the system's program takes over.
    pub async fn component_rollback(&self, id: &str) -> Result<String> {
        let builds = installed(id);
        let newest = builds.last().cloned().ok_or_else(|| Error::NotFound(format!("Universe holds no build of {id}")))?;
        let config = self.config.read().await.clone();
        let system = match newest.kind {
            Kind::Emulator | Kind::Wine => runners::spec(id).and_then(runners::system_program).is_some(),
            Kind::Proton => proton_found(&config).iter().any(|f| crate::config::of_family(&f.name, &newest.family)),
            Kind::Tool | Kind::System => runners::on_system_path(&newest.bin).is_some(),
        };
        if builds.len() < 2 && !system {
            return Err(Error::Invalid(format!("{} {} is the only build of {id}: nothing to roll back to", newest.name, newest.version)));
        }
        if self.pinned(id, newest.kind).await.contains(&newest.version) {
            return Err(Error::Busy(format!("{id} {} is in use: pick another build for what names it first", newest.version)));
        }
        let _job = self.component_job(id)?;
        remove_build(&newest)?;
        set_skipped(id, &newest.version, true)?;
        relink(id, newest.kind, &newest.bin)?;
        self.reload_all().await;
        Ok(installed(id).pop().map(|b| b.version).unwrap_or_default())
    }

    pub async fn component_use(&self, id: &str, build: &str) -> Result<()> {
        let config = self.config.read().await.clone();
        let (kind, family) = cached(&config)
            .components
            .get(id)
            .map(|e| (e.kind, e.family.clone()))
            .or_else(|| installed(id).pop().map(|b| (b.kind, b.family)))
            .or_else(|| {
                runners::spec(id).filter(|s| s.id == id).map(|s| (if s.kind == runners::Kind::Wine { Kind::Wine } else { Kind::Emulator }, String::new()))
            })
            .ok_or_else(|| Error::NotFound(format!("component {id}")))?;
        let own = |v: &str| installed(id).iter().any(|b| b.version == v);
        match kind {
            Kind::Emulator | Kind::Wine => {
                let value = match build {
                    "system" | "" => "",
                    "latest" => "latest",
                    v if own(v) => v,
                    v => return Err(Error::NotFound(format!("{id} {v} is not installed"))),
                };
                self.set_runner_setting(id, "build", value).await
            }
            Kind::Proton => {
                let value = match build {
                    "latest" | "" if !family.is_empty() => family,
                    v if own(v) || proton_found(&config).iter().any(|f| f.name == v) => v.to_string(),
                    v => return Err(Error::NotFound(format!("{v} is not a Proton build here"))),
                };
                self.set_setting("launch.proton", &value).await
            }
            Kind::Tool | Kind::System => Err(Error::Invalid(format!("{id} has no choice of build: the one on PATH runs, else Universe's"))),
        }
    }

    pub fn component_cancel(&self, id: &str) -> bool {
        self.component_jobs.lock().unwrap().get(id).inspect(|flag| flag.store(true, Ordering::Relaxed)).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    fn sha(bytes: &[u8]) -> String {
        sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn tar_of(files: &[(&str, &[u8])], gz: bool) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (name, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        let raw = b.into_inner().unwrap();
        if !gz {
            return raw;
        }
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut enc, &raw).unwrap();
        enc.finish().unwrap()
    }

    fn xz(raw: &[u8]) -> Vec<u8> {
        let mut enc = liblzma::write::XzEncoder::new(Vec::new(), 1);
        std::io::Write::write_all(&mut enc, raw).unwrap();
        enc.finish().unwrap()
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).unix_permissions(0o755);
        for (name, data) in files {
            w.start_file(*name, opts).unwrap();
            std::io::Write::write_all(&mut w, data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    const FAKE_APPIMAGE: &[u8] = b"#!/bin/sh\n[ \"$1\" = --appimage-extract ] || exit 2\nmkdir -p squashfs-root\nprintf '#!/bin/sh\\n' > squashfs-root/AppRun\nchmod +x squashfs-root/AppRun\n";
    const URUNTIME_APPIMAGE: &[u8] = b"#!/bin/sh\n[ \"$1\" = --appimage-extract ] || exit 2\nmkdir -p AppDir\nprintf '#!/bin/sh\\n' > AppDir/AppRun\nchmod +x AppDir/AppRun\nln -s ./AppDir squashfs-root\n";

    fn asset(format: Format, url: &str, bytes: &[u8]) -> Asset {
        Asset {
            arch: "any".into(),
            variant: String::new(),
            url: url.into(),
            sha256: sha(bytes),
            size: bytes.len() as u64,
            format,
            member: String::new(),
            appimage: String::new(),
            program: String::new(),
        }
    }

    fn entry(name: &str, kind: Kind, builds: Vec<Build>) -> Entry {
        Entry { name: name.into(), kind, family: String::new(), bin: String::new(), homepage: String::new(), notice: String::new(), builds }
    }

    fn build(version: &str, date: &str, assets: Vec<Asset>) -> Build {
        Build { version: version.into(), date: date.into(), channel: Channel::Stable, assets }
    }

    fn serve(bodies: Vec<(&'static str, Vec<u8>)>) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                }
                let mut out = stream;
                match bodies.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let _ = write!(out, "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
                        let _ = out.write_all(body);
                    }
                    None => {
                        let _ = write!(out, "HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
                    }
                }
            }
        });
        base
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn each_format_unpacks_to_a_build_with_its_program() {
        let _env = paths::test_env();
        let emu_tar = tar_of(&[("emu-1.0/emu", b"#!/bin/sh\n"), ("emu-1.0/lib/x.so", b"so")], true);
        let wine_tar = xz(&tar_of(&[("wine-11/bin/wine", b"#!/bin/sh\n")], false));
        let proton_tar = tar_of(&[("GE-Proton11-7-x86_64/proton", b"#!/usr/bin/env python3\n"), ("GE-Proton11-7-x86_64/version", b"1 GE-Proton11-7\n")], true);
        let umu_tar = tar_of(&[("umu/umu-run", b"#!/usr/bin/env python3\n"), ("umu/README", b"")], false);
        let zipped = zip_of(&[("melonDS-x86_64.AppImage", FAKE_APPIMAGE)]);
        let base = serve(vec![
            ("/emu.tar.gz", emu_tar.clone()),
            ("/wine.tar.xz", wine_tar.clone()),
            ("/proton.tar.gz", proton_tar.clone()),
            ("/umu.tar", umu_tar.clone()),
            ("/app.AppImage", FAKE_APPIMAGE.to_vec()),
            ("/eden.AppImage", URUNTIME_APPIMAGE.to_vec()),
            ("/melon.zip", zipped.clone()),
            ("/gogdl", b"#!/bin/sh\n".to_vec()),
        ]);
        let url = |p: &str| format!("{base}{p}");
        let never = AtomicBool::new(false);
        let mut emu_asset = asset(Format::TarGz, &url("/emu.tar.gz"), &emu_tar);
        emu_asset.program = "emu".into();
        let mut umu_asset = asset(Format::Tar, &url("/umu.tar"), &umu_tar);
        umu_asset.member = "umu/umu-run".into();
        let mut melon = asset(Format::Zip, &url("/melon.zip"), &zipped);
        melon.appimage = "melonDS-x86_64.AppImage".into();
        let mut umu = entry("umu-launcher", Kind::Tool, vec![]);
        umu.bin = "umu-run".into();
        let mut gogdl = entry("heroic-gogdl", Kind::Tool, vec![]);
        gogdl.bin = "gogdl".into();
        let cases: Vec<(&str, Entry, Build, &str)> = vec![
            ("dosbox", entry("DOSBox", Kind::Emulator, vec![]), build("1.0", "", vec![emu_asset]), "emu"),
            ("wine", entry("Wine", Kind::Wine, vec![]), build("11.17", "", vec![asset(Format::TarXz, &url("/wine.tar.xz"), &wine_tar)]), "bin/wine"),
            (
                "ge-proton",
                entry("GE-Proton", Kind::Proton, vec![]),
                build("GE-Proton11-7", "", vec![asset(Format::TarGz, &url("/proton.tar.gz"), &proton_tar)]),
                "",
            ),
            ("umu-run", umu, build("1.4.4", "", vec![umu_asset]), "umu-run"),
            (
                "xemu",
                entry("xemu", Kind::Emulator, vec![]),
                build("0.8.136", "", vec![asset(Format::AppImage, &url("/app.AppImage"), FAKE_APPIMAGE)]),
                "AppRun",
            ),
            (
                "eden",
                entry("Eden", Kind::Emulator, vec![]),
                build("0.2.1", "", vec![asset(Format::AppImage, &url("/eden.AppImage"), URUNTIME_APPIMAGE)]),
                "AppRun",
            ),
            ("melonds", entry("melonDS", Kind::Emulator, vec![]), build("1.1", "", vec![melon]), "AppRun"),
            ("gogdl", gogdl, build("1.3.0", "", vec![asset(Format::Binary, &url("/gogdl"), b"#!/bin/sh\n")]), "gogdl"),
        ];
        let rt = rt();
        for (id, e, b, program) in cases {
            let mut seen = 0;
            let mut p = |_: u64, _: u64, _: &str| seen += 1;
            let i = rt.block_on(install(id, &e, &b, false, Some(&mut p), &never)).unwrap_or_else(|err| panic!("{id}: {err}"));
            assert!(seen > 0, "{id}: progress reached the caller");
            assert_eq!(i.program, program, "{id}");
            assert!(i.program_path().exists(), "{id}: {}", i.program_path().display());
            assert!(i.dir.symlink_metadata().is_ok_and(|m| m.is_dir()), "{id}: the build is a folder, not a link into the staging area");
            assert_eq!(i.dir, root().join(id).join(slot(&b.version).unwrap()), "{id}: the top folder is stripped into the version's");
            assert_eq!(installed(id).len(), 1, "{id}: the sidecar lists it");
        }
        assert!(root().join("ge-proton/GE-Proton11-7/proton").is_file());
        assert!(root().join("dosbox/1.0/lib/x.so").is_file(), "the rest of the tarball comes along");
        let link = crate::tools::dir().join("umu-run");
        assert_eq!(std::fs::read_link(&link).unwrap(), root().join("umu-run/1.4.4/umu-run"), "a tool is linked into <data>/bin");
        assert!(!root().join(".tmp").read_dir().unwrap().any(|e| e.unwrap().path().extension().is_some_and(|x| x == "part")), "no download left behind");
    }

    #[test]
    fn a_download_that_is_not_the_pinned_one_writes_nothing() {
        let _env = paths::test_env();
        let base = serve(vec![("/emu.AppImage", b"tampered".to_vec())]);
        let a = asset(Format::AppImage, &format!("{base}/emu.AppImage"), FAKE_APPIMAGE);
        let err = rt().block_on(install("xemu", &entry("xemu", Kind::Emulator, vec![]), &build("1", "", vec![a]), false, None, &AtomicBool::new(false)));
        assert!(err.unwrap_err().to_string().contains("sha256"));
        assert!(installed("xemu").is_empty() && !root().join("xemu/1").exists());
    }

    #[test]
    fn a_cancelled_download_stops_and_leaves_nothing() {
        let _env = paths::test_env();
        let base = serve(vec![("/big", vec![0u8; 1 << 20])]);
        let a = asset(Format::Binary, &format!("{base}/big"), &[0u8; 1 << 20]);
        let err = rt().block_on(install("gogdl", &entry("gogdl", Kind::Tool, vec![]), &build("1", "", vec![a]), false, None, &AtomicBool::new(true)));
        assert!(matches!(err, Err(Error::Busy(_))));
        assert!(installed("gogdl").is_empty());
    }

    fn fake_build(id: &str, kind: Kind, version: &str, date: &str) {
        let dir = root().join(id).join(slot(version).unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AppRun"), b"#!/bin/sh\n").unwrap();
        let i = Installed {
            id: id.into(),
            name: id.into(),
            kind,
            version: version.into(),
            date: date.into(),
            channel: Channel::Stable,
            family: String::new(),
            bin: String::new(),
            program: "AppRun".into(),
            url: String::new(),
            sha256: String::new(),
            size: 0,
            disk: 0,
            installed_at: now(),
            auto: true,
            dir: dir.clone(),
        };
        write_atomic(&root().join(id).join(format!("{}.json", slot(version).unwrap())), &serde_json::to_vec(&i).unwrap()).unwrap();
    }

    #[test]
    fn an_update_takes_the_newest_stable_build_and_a_prune_keeps_two_and_the_pinned() {
        let _env = paths::test_env();
        let any = |v: &str| asset(Format::AppImage, &format!("https://x/{v}"), v.as_bytes());
        let mut rolling = build("0.3.1-rc1", "2026-09-20", vec![any("rc")]);
        rolling.channel = Channel::Rolling;
        let e = entry("Eden", Kind::Emulator, vec![build("0.2.1", "2026-06-01", vec![any("a")]), build("0.3.0", "2026-09-01", vec![any("b")]), rolling]);
        assert_eq!(e.latest().unwrap().version, "0.3.0", "a stable build over a newer rolling one");
        assert!(update_of(&installed("eden"), "eden", &e).is_none(), "nothing Universe holds: nothing to update");
        fake_build("eden", Kind::Emulator, "0.1.0", "2026-01-01");
        fake_build("eden", Kind::Emulator, "0.2.0", "2026-03-01");
        fake_build("eden", Kind::Emulator, "0.2.1", "2026-06-01");
        assert_eq!(update_of(&installed("eden"), "eden", &e).map(|b| b.version.as_str()), Some("0.3.0"));
        set_skipped("eden", "0.3.0", true).unwrap();
        assert!(update_of(&installed("eden"), "eden", &e).is_none(), "a rolled back version is never taken again");
        let pinned = BTreeSet::from(["0.1.0".to_string()]);
        assert_eq!(prune("eden", &pinned).unwrap(), Vec::<String>::new(), "0.1.0 pinned, 0.2.0 and 0.2.1 the newest two");
        fake_build("eden", Kind::Emulator, "0.3.0", "2026-09-01");
        assert_eq!(prune("eden", &BTreeSet::new()).unwrap(), ["0.1.0", "0.2.0"]);
        assert_eq!(installed("eden").iter().map(|b| b.version.as_str()).collect::<Vec<_>>(), ["0.2.1", "0.3.0"]);
        let rolling_only = entry("RPCS3", Kind::Emulator, vec![Build { channel: Channel::Rolling, ..build("0.0.38-1", "2026-09-26", vec![any("r")]) }]);
        assert_eq!(rolling_only.latest().unwrap().version, "0.0.38-1", "rolling where upstream ships nothing else");
    }

    #[tokio::test]
    async fn an_uninstall_takes_every_build_and_the_folder_unless_a_game_names_one() {
        let env = paths::test_env();
        std::fs::write(env.path().join("config/config.toml"), "[runners.xemu]\nbuild = \"0.8.1\"\n[modules]\nenabled = []\n").unwrap();
        fake_build("xemu", Kind::Emulator, "0.8.1", "2026-05-01");
        fake_build("xemu", Kind::Emulator, "0.8.136", "2026-06-08");
        set_skipped("xemu", "0.9.0", true).unwrap();
        let mut g = crate::game::Game::new("Halo");
        g.launch.runner = "xemu".into();
        g.launch.runner_build = "0.8.136".into();
        g.save().unwrap();
        let core = crate::core::Core::open_with(Config::load().unwrap(), crate::host::Host::memory().0).await.unwrap();
        assert!(matches!(core.component_uninstall("xemu").await, Err(Error::Busy(_))), "a game runs on 0.8.136");
        assert_eq!(installed("xemu").len(), 2, "refused before anything goes");
        assert_eq!(build_setting(&Config::load().unwrap(), "xemu"), "0.8.1");

        g.launch.runner_build = String::new();
        g.save().unwrap();
        core.reload_all().await;
        assert_eq!(core.component_uninstall("xemu").await.unwrap(), ["0.8.1", "0.8.136"], "the global setting's 0.8.1 goes too");
        assert!(!root().join("xemu").exists(), "the folder goes, its skipped versions with it");
        assert_eq!(build_setting(&Config::load().unwrap(), "xemu"), "", "back on the system's program");
        assert!(matches!(core.component_uninstall("xemu").await, Err(Error::NotFound(_))));

        fake_build("gogdl", Kind::Tool, "1.3.0", "");
        let sidecar = root().join("gogdl/1.3.0.json");
        let mut tool: serde_json::Value = serde_json::from_slice(&std::fs::read(&sidecar).unwrap()).unwrap();
        tool["bin"] = "gogdl".into();
        std::fs::write(&sidecar, serde_json::to_vec(&tool).unwrap()).unwrap();
        relink("gogdl", Kind::Tool, "gogdl").unwrap();
        let link = crate::tools::dir().join("gogdl");
        assert!(link.symlink_metadata().is_ok());
        core.component_uninstall("gogdl").await.unwrap();
        assert!(link.symlink_metadata().is_err(), "a tool's link in <data>/bin goes with it");
    }

    #[tokio::test]
    async fn an_entry_with_a_notice_installs_only_once_it_is_accepted() {
        let env = paths::test_env();
        std::fs::write(env.path().join("config/config.toml"), "[modules]\nenabled = []\n").unwrap();
        let base = serve(vec![("/emu.AppImage", FAKE_APPIMAGE.to_vec())]);
        let image = |name: &str| {
            let mut e =
                entry(name, Kind::Emulator, vec![build("1.0", "2026-06-01", vec![asset(Format::AppImage, &format!("{base}/emu.AppImage"), FAKE_APPIMAGE)])]);
            e.notice = if name == "Eden" { "Bring your own keys.".into() } else { String::new() };
            e
        };
        let catalogue = Catalogue {
            schema: SCHEMA,
            generated_at: String::new(),
            components: BTreeMap::from([("eden".into(), image("Eden")), ("cemu".into(), image("Cemu"))]),
        };
        let file = env.path().join("catalogue.json");
        std::fs::write(&file, serde_json::to_vec(&catalogue).unwrap()).unwrap();
        std::env::set_var("UNIVERSE_CATALOGUE", &file);
        let core = crate::core::Core::open_with(Config::load().unwrap(), crate::host::Host::memory().0).await.unwrap();
        let listed = core.components(true).await.unwrap();
        let eden = listed["components"].as_array().unwrap().iter().find(|c| c["id"] == "eden").unwrap();
        assert_eq!(eden["notice"], "Bring your own keys.");

        assert!(matches!(core.component_install("eden", "", false, None).await, Err(Error::Invalid(_))));
        assert!(installed("eden").is_empty(), "nothing is downloaded before the notice is accepted");
        assert_eq!(core.component_install("eden", "", true, None).await.unwrap(), "1.0");
        assert_eq!(core.component_install("cemu", "", false, None).await.unwrap(), "1.0", "an entry without a notice asks nothing");
    }

    #[test]
    fn a_runner_runs_the_system_program_first_then_follows_its_build_setting() {
        let _env = paths::test_env();
        let spec = runners::spec("xemu").unwrap();
        let mut config = Config::default();
        fake_build("xemu", Kind::Emulator, "0.8.1", "2026-05-01");
        fake_build("xemu", Kind::Emulator, "0.8.136", "2026-06-08");
        let located = runners::locate(spec, &config);
        if runners::system_program(spec).is_none() {
            assert_eq!((located.source.as_str(), located.build.as_str()), ("universe", "0.8.136"), "no system xemu: Universe's newest");
        }
        let mut t = toml::Table::new();
        t.insert("build".into(), toml::Value::String("0.8.1".into()));
        config.runners.insert("xemu".into(), t.clone());
        assert_eq!(runners::locate(spec, &config).build, "0.8.1");
        t.insert("build".into(), toml::Value::String("latest".into()));
        config.runners.insert("xemu".into(), t);
        let latest = runners::locate(spec, &config);
        assert_eq!((latest.source.as_str(), latest.build.as_str()), ("universe", "0.8.136"));
        assert!(latest.program.ends_with("xemu/0.8.136/AppRun"));
    }

    #[test]
    fn a_proton_family_follows_universes_newest_build_but_an_exact_name_wins() {
        let _env = paths::test_env();
        let config = Config::default();
        for v in ["GE-Proton11-6", "GE-Proton11-7"] {
            fake_build("ge-proton", Kind::Proton, v, "");
            std::fs::write(root().join("ge-proton").join(v).join("proton"), b"").unwrap();
        }
        let found = config.proton_path("proton-ge").unwrap();
        if found.starts_with(canonical(&root())) {
            assert!(found.ends_with("GE-Proton11-7"), "{}", found.display());
        }
        assert!(config.proton_path("GE-Proton11-6").unwrap().ends_with("ge-proton/GE-Proton11-6"), "a build by its name");
        let own = paths::data_home().join("proton/proton-ge");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::write(own.join("proton"), b"").unwrap();
        assert_eq!(config.proton_path("proton-ge").unwrap(), canonical(&own), "<data>/proton/<name> is the user's own: it wins");
        assert!(config.proton_names().contains(&"GE-Proton11-7".to_string()));
    }

    #[test]
    fn the_shipped_sources_pin_their_tools_and_the_core_umu_run_and_ludusavi() {
        let _env = crate::paths::test_env();
        assert_eq!(pinned().components.values().filter(|e| e.kind == Kind::Tool).map(|e| e.bin.as_str()).collect::<Vec<_>>(), ["ludusavi", "umu-run"]);
        std::env::set_var("UNIVERSE_SOURCES_PATH", concat!(env!("CARGO_MANIFEST_DIR"), "/../../sources"));
        let catalogue = pinned();
        for (bin, source) in [("gogdl", "gog"), ("legendary", "epic"), ("butler", "itch")] {
            let (_, entry) = catalogue.tool(bin).unwrap_or_else(|| panic!("sources/{source} pins no {bin}"));
            assert!(entry.latest().is_some_and(|b| b.asset().is_some_and(|a| a.sha256.len() == 64)), "{bin}: a build with a checked asset");
        }
    }

    #[test]
    fn a_system_tool_is_proposed_where_packagekit_installs_it_and_the_config_wants_it() {
        use crate::distro::Family;
        let config = Config::default();
        let gamescope = system_tool("gamescope").unwrap();
        let arch = system_json(gamescope, &config, &[], Family::Arch, true);
        assert_eq!(arch["packages"], serde_json::json!(["gamescope"]));
        assert_eq!(arch["installable"], true);
        if runners::on_system_path("gamescope").is_none() {
            assert_eq!(arch["proposal"], "install", "launch.gamescope is on by default");
        }
        assert_eq!(system_json(gamescope, &config, &[], Family::Arch, false)["proposal"], "", "no PackageKit, no proposal");
        let nixos = system_json(gamescope, &config, &[], Family::NixOs, true);
        assert_eq!(nixos["installable"], false);
        assert!(nixos["fix"].as_str().unwrap().contains("programs.universe.gamescope.enable"));
        let gsr = system_tool("gpu-screen-recorder").unwrap();
        assert_eq!(system_json(gsr, &config, &[], Family::Fedora, true)["installable"], false, "not in Fedora's repositories");
        if runners::on_system_path("gpu-screen-recorder").is_none() {
            assert_eq!(system_json(gsr, &config, &[], Family::Arch, true)["proposal"], "", "no enabled module asks for it");
            assert_eq!(
                system_json(gsr, &config, &["gpu-screen-recorder".into()], Family::Arch, true)["proposal"],
                "install",
                "the capture module's [requires] system"
            );
        }
        let mangohud = system_tool("mangohud").unwrap();
        assert_eq!(mangohud.packages(Family::Debian), ["mangohud", "mangohud:i386"], "the 32-bit layer that 32-bit games load");
    }

    #[test]
    fn a_version_is_read_from_a_store_path_or_a_file_name_and_compared_only_when_sure() {
        assert_eq!(nix_version(Path::new("/nix/store/pqdymjsb2g5lvammrj2bixbwlb2z6cpk-dolphin-emu-2606a/bin/dolphin-emu")).as_deref(), Some("2606a"));
        assert_eq!(nix_version(Path::new("/nix/store/ljfg6iwy7lcvnjj68lqb6g9ms5fwpwqr-ryujinx-canary-1.3.351/bin/Ryujinx")).as_deref(), Some("1.3.351"));
        assert_eq!(nix_version(Path::new("/usr/bin/eden")), None);
        assert_eq!(name_version("mGBA-0.10.5-appimage-x64.appimage").as_deref(), Some("0.10.5-appimage-x64.appimage"));
        assert_eq!(newer("0.3.0", "0.2.1"), Some(true));
        assert_eq!(newer("v2.8.2", "2.8.2"), Some(false));
        assert_eq!(newer("2609", "2606a"), Some(true));
        assert_eq!(newer("GE-Proton11-7", "GE-Proton10-15"), Some(true));
        assert_eq!(newer("0.0.38-18123", "0.0.42-unstable-2026-08-15"), None, "a snapshot proposes nothing");
        assert_eq!(newer("GE-Proton11-7", "0.2.1"), None, "two schemes");
    }

    #[test]
    fn a_nixos_wrapper_leads_to_the_versioned_program_it_runs() {
        let setcap = "/nix/store/z0rh-security-wrapper-gamescope-x86_64-unknown-linux-musl/bin/security-wrapper";
        let script = format!("cp {setcap} \"$wrapperDir/gamescope\"\nchmod 0000 \"$wrapperDir/gamescope\"\n");
        assert_eq!(wrapper_copied_from(&script, "gamescope"), Some(PathBuf::from(setcap)));
        assert_eq!(wrapper_copied_from(&script, "gamescopectl"), None);
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("gamescope");
        std::fs::write(&binary, b"\x7fELF\0/nix/store/6j9n-gamescope/lib/x.so\0/nix/store/sw8a-gamescope-3.16.23/bin/gamescope\0").unwrap();
        let found = embedded_program(&binary, "gamescope").unwrap();
        assert_eq!((found.to_str(), nix_version(&found).as_deref()), (Some("/nix/store/sw8a-gamescope-3.16.23/bin/gamescope"), Some("3.16.23")));
        let script = dir.path().join("dolphin-emu");
        std::fs::write(&script, "#!/bin/sh\nexec \"/nix/store/j01n-dolphin-emu/bin/.dolphin-emu-wrapped\" \"$@\"\n").unwrap();
        assert_eq!(embedded_program(&script, "dolphin-emu"), Some(PathBuf::from("/nix/store/j01n-dolphin-emu/bin/.dolphin-emu-wrapped")));
        assert_eq!(store_program(Path::new("/usr/bin/gamescope")), None);
    }

    #[test]
    fn the_catalogue_falls_back_to_the_cache_then_the_pinned_tools() {
        let _env = paths::test_env();
        let file = paths::data_home().join("catalogue.json");
        std::fs::create_dir_all(paths::data_home()).unwrap();
        let config = Config::default();
        std::env::set_var("UNIVERSE_CATALOGUE", &file);
        let rt = rt();
        let missing = rt.block_on(load(&config, true));
        assert!(!missing.error.is_empty());
        assert!(
            missing.catalogue.components.contains_key("umu-run") && missing.catalogue.components.contains_key("ge-proton"),
            "the built-in catalogue stands in"
        );
        let catalogue = serde_json::json!({"schema": 1, "generated_at": "2026-09-27T06:00:00Z", "components": {
            "eden": {"name": "Eden", "kind": "emulator", "builds": [{"version": "0.2.1", "date": "2026-06-01", "assets": [
                {"url": "https://x/eden.AppImage", "sha256": "00", "size": 60, "format": "appimage"}]}]}}});
        std::fs::write(&file, catalogue.to_string()).unwrap();
        let fetched = rt.block_on(load(&config, true));
        assert!(fetched.error.is_empty() && fetched.catalogue.components.contains_key("eden"));
        std::fs::remove_file(&file).unwrap();
        let offline = rt.block_on(load(&config, true));
        assert!(!offline.error.is_empty() && offline.catalogue.components.contains_key("eden"), "a failed fetch keeps the cache");
        assert!(cached(&config).components.contains_key("eden"));
    }
}
