//! The version a game's files say is installed: a Windows program's version resource, a Unity game's player settings, a
//! PS3 game's `PARAM.SFO`, the update a Switch emulator applies, a ROM's name, a GOG install's build.

use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::game::Game;
use crate::{paths, Error, Result};

/// What a build leaves when nobody set a version: it looks authoritative and never changes.
const PLACEHOLDERS: [&str; 6] = ["", "1.0.0.0", "0.0.0.0", "1.0", "0.1", "Dev-CL-0"];
/// Read by their name, like a file without an extension.
const ROM_SUFFIXES: [&str; 8] = ["xci", "nsp", "iso", "wbfs", "rvz", "nds", "gba", "bin"];
const RT_VERSION: u32 = 16;
const SUBDIR: u32 = 0x8000_0000;
/// A version resource is a few hundred bytes; past this a data entry is not one.
const MAX_BLOB: u32 = 64 * 1024;
const RULE_BYTES: u64 = 2_000_000;
const UNITY_BYTES: u64 = 200_000;
/// `applicationCategoryType`'s value, which Unity writes just before `bundleVersion`.
const UNITY_ANCHOR: &[u8] = b"public.app-category.games";
/// How far up from the program a probe's file is looked for: a launcher stub sits below the install's root.
const RULE_LEVELS: usize = 5;
/// How far up from the program an Unreal `Engine/Binaries` tree is looked for.
const UNREAL_LEVELS: usize = 4;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Installed {
    /// The pin, else what the files say; empty when they say nothing.
    pub installed: String,
    /// Where `installed` comes from: `pin`, `exe`, `unity`, `file`, `name`, `eden`, `ryujinx`, `param.sfo`, `rpcs3`.
    pub from: String,
    /// What kind of updates the game takes, by its runner and its files: `pc`, `gog` (a GOG install), `switch`, `ps3`;
    /// `final` for the consoles no update ever reached; empty for the rest.
    pub channel: &'static str,
    /// The Switch title id, the PS3 `TITLE_ID`, the GOG game id.
    pub title_id: String,
    /// What an upstream list numbers the installed build by: the GOG build id, the Switch title version.
    pub build: String,
    /// The day the file the version is read from last changed (the program, its probe's file, a Unity game's settings), `YYYY-MM-DD`.
    pub modified: String,
}

pub fn channel_of(runner: &str) -> &'static str {
    match runner {
        "proton" | "wine" | "linux" => "pc",
        "eden" | "ryujinx" => "switch",
        "rpcs3" => "ps3",
        "dolphin" | "melonds" | "mgba" | "pcsx2" | "duckstation" | "ppsspp" | "xemu" | "mupen64plus" | "snes9x" | "flycast" | "scummvm" | "dosbox" | "mame" => {
            "final"
        }
        _ => "",
    }
}

/// `value` as `set` writes `version.<field>`.
pub fn checked(field: &str, value: &str) -> Result<String> {
    let value = value.trim();
    match field {
        "pin" | "seen" => Ok(value.into()),
        "final" if matches!(value, "" | "true" | "false") => Ok(value.into()),
        "final" => Err(Error::Invalid("version.final takes true or false".into())),
        "switch_title_id" if value.is_empty() || is_title_id(value) => Ok(value.to_ascii_lowercase()),
        "switch_title_id" => Err(Error::Invalid(format!("{value}: a Switch title id is 16 hex digits"))),
        "probe" if value.is_empty() => Ok(String::new()),
        "probe" => {
            let (file, pattern) = rule(value).ok_or_else(|| Error::Invalid(format!("{value}: a probe is @<file> or @<file>:<regex>")))?;
            if file.is_empty() {
                return Err(Error::Invalid(format!("{value}: no file")));
            }
            if let Some(p) = pattern {
                Regex::new(p).map_err(|e| Error::Invalid(format!("{value}: {e}")))?;
            }
            Ok(value.into())
        }
        _ => Err(Error::Invalid(format!("unknown key version.{field}"))),
    }
}

fn is_title_id(s: &str) -> bool {
    s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn rule(probe: &str) -> Option<(&str, Option<&str>)> {
    let rest = probe.strip_prefix('@')?;
    Some(match rest.split_once(':') {
        Some((file, pattern)) => (file, Some(pattern)),
        None => (rest, None),
    })
}

pub fn read(game: &Game, probes: &mut Probes) -> Installed {
    let runner = game.runner_id();
    let mut out = Installed { channel: channel_of(&runner), ..Default::default() };
    let exe = game.exe_path();
    if !game.launch.exe.is_empty() && exe.exists() {
        let probe = &game.version.probe;
        let mut file = exe.clone();
        let own = match out.channel {
            "switch" => switch_version(&exe, &runner, &game.version.switch_title_id, &mut out),
            "ps3" => ps3_version(&exe, &mut out),
            "pc" if probe.is_empty() => file_version(&exe, probes, &mut file),
            "pc" => None,
            _ => name_version(&exe).map(|v| (v, "name")),
        };
        if out.channel == "pc" {
            if let Some((build, id)) = gog_build(game) {
                (out.channel, out.build, out.title_id) = ("gog", build, id);
            }
        }
        let found = if probe.is_empty() { own } else { probe_version(&exe, probe, probes, &mut file) };
        if let Some((v, from)) = found {
            (out.installed, out.from) = (v, from.into());
        }
        out.modified = modified(&file);
    }
    if !game.version.pin.is_empty() {
        (out.installed, out.from) = (game.version.pin.clone(), "pin".into());
    }
    out
}

/// The file a probe names, looked for from the program's folder up; `file` becomes the one read.
fn probe_version(exe: &Path, probe: &str, probes: &mut Probes, file: &mut PathBuf) -> Option<(String, &'static str)> {
    let (rel, pattern) = rule(probe)?;
    for root in exe.parent()?.ancestors().take(RULE_LEVELS) {
        let target = root.join(rel);
        match pattern {
            Some(p) if target.is_file() => {
                if let Some(v) = probes.cached(&format!("file\t{p}"), &target, |t| file_match(t, p)) {
                    *file = target;
                    return Some((v, "file"));
                }
            }
            None if target.exists() => {
                *file = target.clone();
                return file_version(&target, probes, file);
            }
            _ => {}
        }
    }
    None
}

/// A ROM's or a Linux binary's name, a Unity game's settings, else the program's version resource.
fn file_version(path: &Path, probes: &mut Probes, file: &mut PathBuf) -> Option<(String, &'static str)> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ext.is_empty() || ROM_SUFFIXES.contains(&ext.as_str()) {
        return name_version(path).map(|v| (v, "name"));
    }
    let ggm = unity_settings(path);
    if ggm.is_file() {
        *file = ggm.clone();
        return probes.cached("unity", &ggm, unity_version).map(|v| (v, "unity"));
    }
    // Unreal's linker stamps the engine's version into the fixed block: 4.27.2.0 for Lies of P.
    let (how, fixed) = if under_unreal(path) { ("exe-strings", false) } else { ("exe", true) };
    probes.cached(how, path, |p| exe_version(p, fixed)).map(|v| (v, "exe"))
}

fn unity_settings(exe: &Path) -> PathBuf {
    let stem = exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    exe.with_file_name(format!("{stem}_Data")).join("globalgamemanagers")
}

fn under_unreal(exe: &Path) -> bool {
    exe.parent().is_some_and(|d| d.ancestors().take(UNREAL_LEVELS).any(|d| d.join("Engine/Binaries").is_dir()))
}

/// The manifest beside the install, else the GOG source's own record. An older Lutris import's `source.dir` is the exe's
/// folder, below the manifest: no stop.
fn gog_build(game: &Game) -> Option<(String, String)> {
    let stops: Vec<PathBuf> = [&game.launch.prefix].into_iter().filter(|p| !p.is_empty()).map(|p| paths::expand(p)).collect();
    if let Some((_, m)) = game.exe_path().parent().and_then(|d| crate::lutris::find_gog_manifest(d, &stops)) {
        return Some((m.build_id, m.game_id));
    }
    (game.source.kind == "gog" && !game.source.build_id.is_empty()).then(|| (game.source.build_id.clone(), game.source.id.clone()))
}

fn modified(path: &Path) -> String {
    std::fs::metadata(path).and_then(|m| m.modified()).map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d").to_string()).unwrap_or_default()
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Positioned reads of a file, each checked against its length: a 100 GB program is read a few bytes at a time.
struct Reader {
    file: std::fs::File,
    len: u64,
}

impl Reader {
    fn open(path: &Path) -> Option<Reader> {
        let file = std::fs::File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        Some(Reader { file, len })
    }

    fn at(&self, offset: u64, n: u64) -> Option<Vec<u8>> {
        if offset.checked_add(n)? > self.len {
            return None;
        }
        let mut b = vec![0; n as usize];
        self.file.read_exact_at(&mut b, offset).ok()?;
        Some(b)
    }
}

/// `ProductVersion`, then `FileVersion`, then the fixed block, of a PE's version resource: the headers, the section
/// table, the resource directories down to the first `RT_VERSION` entry, then that entry's blob, and nothing else.
fn exe_version(path: &Path, fixed: bool) -> Option<String> {
    let r = Reader::open(path)?;
    let dos = r.at(0, 64)?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let pe = u32_at(&dos, 0x3c)? as u64;
    let coff = r.at(pe, 24)?;
    if &coff[..4] != b"PE\0\0" {
        return None;
    }
    let (sections, opt_size) = (u16_at(&coff, 6)? as u64, u16_at(&coff, 20)? as u64);
    let opt = r.at(pe + 24, opt_size)?;
    // The data directories follow 96 bytes of optional header in a PE32, 112 in a PE32+; the resources' is the third.
    let dirs = if u16_at(&opt, 0)? == 0x20b { 112 } else { 96 };
    let rsrc = u32_at(&opt, dirs + 16).filter(|&rva| rva != 0)?;
    let table = r.at(pe + 24 + opt_size, sections.min(96) * 40)?;
    let offset_of = |rva: u32| {
        table.as_chunks::<40>().0.iter().find_map(|s| {
            let (vaddr, size, raw) = (u32_at(s, 12)?, u32_at(s, 16)?, u32_at(s, 20)?);
            (vaddr <= rva && rva - vaddr < size).then(|| raw as u64 + (rva - vaddr) as u64)
        })
    };
    let base = offset_of(rsrc)?;
    // A directory: a 16-byte header ending in its named and id entry counts, then 8-byte entries of an id and an offset
    // whose top bit marks a subdirectory.
    let entry = |dir: u32, id: Option<u32>| -> Option<u32> {
        let head = r.at(base + dir as u64, 16)?;
        let n = u16_at(&head, 12)? as u64 + u16_at(&head, 14)? as u64;
        let list = r.at(base + dir as u64 + 16, n * 8)?;
        list.as_chunks::<8>().0.iter().find(|e| id.is_none_or(|id| u32_at(*e, 0) == Some(id))).and_then(|e| u32_at(e, 4))
    };
    let names = entry(0, Some(RT_VERSION)).filter(|o| o & SUBDIR != 0)? & !SUBDIR;
    let langs = entry(names, None).filter(|o| o & SUBDIR != 0)? & !SUBDIR;
    let leaf = entry(langs, None).filter(|o| o & SUBDIR == 0)?;
    let data = r.at(base + leaf as u64, 8)?;
    let (rva, size) = (u32_at(&data, 0)?, u32_at(&data, 4)?);
    let blob = r.at(offset_of(rva)?, size.min(MAX_BLOB) as u64)?;
    version_info(&blob, fixed)
}

/// A VS_VERSIONINFO node: `wLength`, `wValueLength` (in words when `wType` is 1, text), `wType`, a UTF-16 key, then the
/// value and the children, each 4-aligned.
struct Node {
    key: String,
    value_at: usize,
    children_at: usize,
    end: usize,
}

impl Node {
    fn at(b: &[u8], at: usize) -> Option<Node> {
        let (len, value_len, text) = (u16_at(b, at)? as usize, u16_at(b, at + 2)? as usize, u16_at(b, at + 4)? == 1);
        let end = (at + len).min(b.len());
        if len < 6 {
            return None;
        }
        let (key, after) = utf16z(b, at + 6, end);
        let value_at = after.next_multiple_of(4);
        let children_at = (value_at + if text { value_len * 2 } else { value_len }).next_multiple_of(4);
        Some(Node { key, value_at, children_at, end })
    }

    fn children(&self, b: &[u8]) -> Vec<Node> {
        let mut out = Vec::new();
        let mut at = self.children_at;
        while at + 6 <= self.end {
            let Some(n) = Node::at(b, at) else { break };
            at = n.end.next_multiple_of(4);
            out.push(n);
        }
        out
    }
}

/// The UTF-16 text from `at` to its NUL or `end`, and the offset past the NUL.
fn utf16z(b: &[u8], at: usize, end: usize) -> (String, usize) {
    let units: Vec<u16> = b.get(at..end).unwrap_or_default().as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
    (String::from_utf16_lossy(&units), at + units.len() * 2 + 2)
}

/// The publisher sets `ProductVersion` by hand; `FileVersion` is often a build counter; the linker fills the fixed block in
/// even when nobody set it, but it is at times the only one kept up.
fn version_info(b: &[u8], fixed: bool) -> Option<String> {
    let root = Node::at(b, 0)?;
    let strings: Vec<(String, String)> = root
        .children(b)
        .into_iter()
        .filter(|n| n.key == "StringFileInfo")
        .flat_map(|info| info.children(b))
        .flat_map(|table| table.children(b))
        .map(|s| (s.key.clone(), utf16z(b, s.value_at, s.end).0))
        .collect();
    let string = |key: &str| strings.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let fixed_block = || {
        let at = b.windows(4).position(|w| w == 0xFEEF_04BDu32.to_le_bytes())?;
        let (ms, ls) = (u32_at(b, at + 8)?, u32_at(b, at + 12)?);
        Some(format!("{}.{}.{}.{}", ms >> 16, ms & 0xffff, ls >> 16, ls & 0xffff))
    };
    [string("ProductVersion"), string("FileVersion"), if fixed { fixed_block() } else { None }]
        .into_iter()
        .flatten()
        .map(|v| v.replace(", ", ".").trim().to_string())
        .find(|v| !PLACEHOLDERS.contains(&v.as_str()))
}

/// PlayerSettings' `bundleVersion` in `globalgamemanagers`: a per-platform map of no fixed size lies between it and
/// `UNITY_ANCHOR`, so the first length-prefixed string after the anchor that reads as a version.
fn unity_version(ggm: &Path) -> Option<String> {
    static VERSIONISH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(\.\d+)+[0-9a-zA-Z.\-+_ ()]*$").unwrap());
    let mut b = Vec::new();
    std::fs::File::open(ggm).ok()?.take(UNITY_BYTES).read_to_end(&mut b).ok()?;
    let anchor = b.windows(UNITY_ANCHOR.len()).position(|w| w == UNITY_ANCHOR)?;
    let mut at = (anchor + 3) & !3;
    for _ in 0..200 {
        at += 4;
        let Some(len) = u32_at(&b, at).map(|l| l as usize) else { break };
        if !(1..=64).contains(&len) || at + 4 + len > b.len() {
            continue;
        }
        let Ok(v) = std::str::from_utf8(&b[at + 4..at + 4 + len]) else { continue };
        if VERSIONISH.is_match(v) {
            return (!PLACEHOLDERS.contains(&v)).then(|| v.to_string());
        }
    }
    None
}

/// A probe's regex on the start of its file: its first group, else the whole match.
fn file_match(path: &Path, pattern: &str) -> Option<String> {
    let re = Regex::new(pattern).ok()?;
    let mut b = Vec::new();
    std::fs::File::open(path).ok()?.take(RULE_BYTES).read_to_end(&mut b).ok()?;
    let text = String::from_utf8_lossy(&b);
    let c = re.captures(&text)?;
    c.get(1).or_else(|| c.get(0)).map(|m| m.as_str().to_string())
}

/// What a scene release's name says, where it names its update and nowhere else: `v1.2.0`, `[v131072]`, `_102_`, `(Rev 2)`.
struct Named {
    dotted: Option<String>,
    number: Option<String>,
    digits: Option<String>,
    rev: Option<String>,
}

fn named(path: &Path) -> Named {
    static PATTERNS: LazyLock<[Regex; 4]> =
        LazyLock::new(|| [r"[vV](\d+(?:\.\d+)+)", r"\[v(\d+)\]", r"[_ ](\d)(\d)(\d)[_ ]", r"\(Rev (\w+)\)"].map(|p| Regex::new(p).unwrap()));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let [dotted, number, digits, rev] = &*PATTERNS;
    Named {
        dotted: dotted.captures(&name).map(|c| c[1].to_string()),
        number: number.captures(&name).map(|c| c[1].to_string()),
        digits: digits.captures(&name).map(|c| format!("{}.{}.{}", &c[1], &c[2], &c[3])),
        rev: rev.captures(&name).map(|c| format!("Rev {}", &c[1])),
    }
}

fn name_version(path: &Path) -> Option<String> {
    let n = named(path);
    n.dotted.or(n.number).or(n.digits).or(n.rev)
}

/// The update the emulator that runs the game applies (the other's when it knows none), else the ROM's name: the
/// version as its release names it, and the title version (`update << 16`) when one is known.
fn switch_version(rom: &Path, runner: &str, title_id: &str, out: &mut Installed) -> Option<(String, &'static str)> {
    static TITLE_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([0-9A-Fa-f]{16})\]").unwrap());
    let name = rom.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    out.title_id = TITLE_ID.captures(&name).map(|c| c[1].to_ascii_lowercase()).unwrap_or_else(|| title_id.to_ascii_lowercase());
    let id = out.title_id.as_str();
    let eden = || eden_update(id).map(|u| (u, "eden"));
    let ryujinx = || ryujinx_update(id).map(|u| (u, "ryujinx"));
    let applied = match id {
        "" => None,
        _ if runner == "ryujinx" => ryujinx().or_else(eden),
        _ => eden().or_else(ryujinx),
    };
    let ((version, number), from) = applied.unwrap_or_else(|| {
        let n = named(rom);
        ((n.dotted.or(n.digits).unwrap_or_default(), n.number.unwrap_or_default()), "name")
    });
    out.build = number;
    (!version.is_empty()).then_some((version, from))
}

/// Eden and its forks list a title's add-ons per line; `Update (1.4.0)` or `Update (NAND) (1.4.0)`, `[D]` before one it has turned off.
fn eden_update(title_id: &str) -> Option<(String, String)> {
    static UPDATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Update(?: \(NAND\))? \((\d+(?:\.\d+)+)\)").unwrap());
    let cache = paths::xdg("XDG_CACHE_HOME", ".cache");
    crate::roms::EDEN_CONFIGS.iter().find_map(|fork| {
        let text = std::fs::read_to_string(cache.join(fork).join("game_list").join(format!("{}.pv.txt", title_id.to_ascii_uppercase()))).ok()?;
        let version =
            text.lines().filter(|l| !l.starts_with("[D]")).filter_map(|l| UPDATE.captures(l).map(|c| c[1].to_string())).max_by(|a, b| compare(a, b))?;
        Some((version, String::new()))
    })
}

/// Ryujinx keeps the update file it applies, whose name carries the version and at times the title version.
fn ryujinx_update(title_id: &str) -> Option<(String, String)> {
    static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"v(\d{4,})").unwrap());
    static DOTTED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[vV](\d+(?:\.\d+)+)").unwrap());
    let games = paths::xdg("XDG_CONFIG_HOME", ".config").join("Ryujinx/games");
    let text =
        [title_id.to_ascii_lowercase(), title_id.to_ascii_uppercase()].iter().find_map(|d| std::fs::read_to_string(games.join(d).join("updates.json")).ok())?;
    let updates: serde_json::Value = serde_json::from_str(&text).ok()?;
    let selected = Path::new(updates["selected"].as_str()?).file_name()?.to_string_lossy().into_owned();
    let version = DOTTED.captures(&selected).map(|c| c[1].to_string()).unwrap_or_default();
    let number = NUMBER.captures(&selected).map(|c| c[1].to_string()).unwrap_or_default();
    (!version.is_empty() || !number.is_empty()).then_some((version, number))
}

/// The disc's `PARAM.SFO` above its `EBOOT.BIN`, or the update RPCS3 installed for the same title on its HDD, the higher.
fn ps3_version(eboot: &Path, out: &mut Installed) -> Option<(String, &'static str)> {
    let sfo = eboot.parent()?.ancestors().take(3).map(|d| d.join("PARAM.SFO")).find(|p| p.is_file())?;
    out.title_id = crate::roms::sfo_value(&sfo, "TITLE_ID").unwrap_or_default();
    let disc = crate::roms::sfo_value(&sfo, "APP_VER").map(|v| (v, "param.sfo"));
    let hdd = Some(&out.title_id).filter(|id| !id.is_empty()).and_then(|id| {
        let sfo = paths::xdg("XDG_CONFIG_HOME", ".config").join("rpcs3/dev_hdd0/game").join(id).join("PARAM.SFO");
        crate::roms::sfo_value(&sfo, "APP_VER").map(|v| (v, "rpcs3"))
    });
    match (disc, hdd) {
        (Some(d), Some(h)) => Some(if compare(&h.0, &d.0).is_gt() { h } else { d }),
        (d, h) => d.or(h),
    }
}

/// Dotted versions by their numbers: `01.02` after `01.00`, `1.10` after `1.9`.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |s: &str| s.split('.').map(|p| p.parse::<u64>().map_err(|_| p.to_string())).collect::<Vec<_>>();
    parts(a).cmp(&parts(b))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Probe {
    modified_ns: i64,
    size: u64,
    version: String,
}

/// What reading a program, a Unity game's settings or a probe's file came to, kept under the cache by the file's size and
/// time: the file is read again only once it changed.
#[derive(Debug, Default)]
pub struct Probes {
    entries: Option<BTreeMap<String, Probe>>,
    changed: bool,
}

impl Probes {
    fn file() -> PathBuf {
        paths::cache_home().join("versions.json")
    }

    fn cached(&mut self, how: &str, path: &Path, read: impl FnOnce(&Path) -> Option<String>) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let modified_ns = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos() as i64);
        let key = format!("{how}\t{}", path.display());
        let entries = self.entries.get_or_insert_with(|| std::fs::read(Self::file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default());
        if let Some(p) = entries.get(&key).filter(|p| p.modified_ns == modified_ns && p.size == meta.len()) {
            return Some(p.version.clone()).filter(|v| !v.is_empty());
        }
        let version = read(path);
        entries.insert(key, Probe { modified_ns, size: meta.len(), version: version.clone().unwrap_or_default() });
        self.changed = true;
        version
    }

    /// Written only when a file was read; entries for files gone are dropped then.
    pub fn save(self) {
        let (true, Some(mut entries)) = (self.changed, self.entries) else { return };
        entries.retain(|key, _| key.rsplit_once('\t').is_some_and(|(_, path)| Path::new(path).exists()));
        let path = Self::file();
        // Two threads of one process may save at once.
        static SAVES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), SAVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let written = std::fs::create_dir_all(paths::cache_home()).and_then(|_| std::fs::write(&tmp, serde_json::to_vec(&entries).unwrap_or_default()));
        if written.and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

pub fn read_one(game: &Game) -> Installed {
    let mut probes = Probes::default();
    let out = read(game, &mut probes);
    probes.save();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testpe::Exe;

    fn write(path: &Path, bytes: &[u8]) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
        path.to_path_buf()
    }

    fn exe(path: &Path, e: Exe) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        e.write(path);
        path.to_path_buf()
    }

    fn game(runner: &str, exe: &Path) -> Game {
        let mut g = Game::new("Game");
        g.launch.runner = runner.into();
        g.launch.exe = exe.to_string_lossy().into();
        g
    }

    #[test]
    fn a_programs_version_resource_reads_its_product_then_file_version_then_its_fixed_block() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("game.exe");
        let read = |e: Exe, fixed: bool| {
            e.write(&p);
            exe_version(&p, fixed)
        };
        let both = [("FileVersion", "9.9.9.9"), ("ProductVersion", "1.0.42.49788435")];
        assert_eq!(read(Exe { strings: &both, fixed: Some([1, 2, 3, 4]), ..Default::default() }, true).as_deref(), Some("1.0.42.49788435"));
        let placeholder = [("ProductVersion", "1.0.0.0"), ("FileVersion", "2, 1, 0, 7")];
        assert_eq!(
            read(Exe { strings: &placeholder, ..Default::default() }, true).as_deref(),
            Some("2.1.0.7"),
            "a placeholder is passed over, commas read as dots"
        );
        let empty = [("ProductVersion", ""), ("FileVersion", "3.0618.0.0")];
        assert_eq!(read(Exe { strings: &empty, ..Default::default() }, true).as_deref(), Some("3.0618.0.0"), "an empty value is none");
        assert_eq!(read(Exe { fixed: Some([1, 6, 0, 0]), pe32: true, ..Default::default() }, true).as_deref(), Some("1.6.0.0"), "a PE32's fixed block");
        assert_eq!(read(Exe { fixed: Some([4, 27, 2, 0]), ..Default::default() }, false), None, "the fixed block left out");
        assert_eq!(read(Exe { strings: &[("ProductVersion", "Dev-CL-0")], fixed: Some([0, 0, 0, 0]), ..Default::default() }, true), None);

        Exe { strings: &both, ..Default::default() }.write(&p);
        let whole = std::fs::read(&p).unwrap();
        std::fs::write(&p, &whole[..whole.len() - 40]).unwrap();
        assert_eq!(exe_version(&p, true), None, "a cut program reads as none");
        std::fs::write(&p, b"MZ not a program").unwrap();
        assert_eq!(exe_version(&p, true), None);
    }

    #[test]
    fn a_three_gigabyte_program_is_read_by_its_headers_and_version_resource_alone() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("GTA5_Enhanced.exe");
        Exe { strings: &[("ProductVersion", "1.0.1158.13")], gap: 3 << 30, ..Default::default() }.write(&p);
        assert!(std::fs::metadata(&p).unwrap().len() > 3 << 30);
        assert_eq!(exe_version(&p, true).as_deref(), Some("1.0.1158.13"));
    }

    #[test]
    fn a_pc_game_reads_its_probes_file_its_unity_settings_or_its_program() {
        let env = paths::test_env();
        let gta = env.path().join("games/Grand Theft Auto V Enhanced");
        let stub = exe(&gta.join("PlayGTAV.exe"), Exe { strings: &[("ProductVersion", "2.0.1.5")], ..Default::default() });
        exe(&gta.join("GTA5_Enhanced.exe"), Exe { strings: &[("ProductVersion", "1.0.1158.13")], ..Default::default() });
        let mut g = game("proton", &stub);
        assert_eq!((read_one(&g).installed.as_str(), read_one(&g).from.as_str()), ("2.0.1.5", "exe"));
        g.version.probe = "@GTA5_Enhanced.exe".into();
        assert_eq!(read_one(&g).installed, "1.0.1158.13", "the probe's program instead");

        let nested = exe(&gta.join("bin/x64/launcher.exe"), Exe::default());
        write(&gta.join("version.txt"), b"Build: 1.0.1158.13-final\nVersion=1.0.3\n");
        let mut g = game("proton", &nested);
        g.version.probe = r"@version.txt:Version=(\S+)".into();
        assert_eq!((read_one(&g).installed.as_str(), read_one(&g).from.as_str()), ("1.0.3", "file"), "found two folders up");
        g.version.probe = r"@version.txt:Build: \S+".into();
        assert_eq!(read_one(&g).installed, "Build: 1.0.1158.13-final", "no group: the whole match");
        g.version.probe = "@missing.txt:(.*)".into();
        assert_eq!(read_one(&g).installed, "", "a probe that finds nothing says nothing");

        let motorways = env.path().join("games/Mini Motorways");
        let unity = exe(&motorways.join("Mini Motorways.exe"), Exe { strings: &[("ProductVersion", "2020.3.14f1")], ..Default::default() });
        let mut ggm = vec![0u8; 40];
        for (i, s) in ["public.app-category.games", "Standalone", "com.DinosaurPolo.MiniMotorways", "1.8.3"].iter().enumerate() {
            if i == 1 {
                ggm.extend(1u32.to_le_bytes());
            }
            ggm.extend((s.len() as u32).to_le_bytes());
            ggm.extend(s.as_bytes());
            ggm.resize(ggm.len().next_multiple_of(4), 0);
        }
        write(&motorways.join("Mini Motorways_Data/globalgamemanagers"), &ggm);
        let read = read_one(&game("proton", &unity));
        assert_eq!((read.installed.as_str(), read.from.as_str()), ("1.8.3", "unity"), "Unity's player settings, not the engine's version");

        let lies = env.path().join("games/Lies of P");
        std::fs::create_dir_all(lies.join("Engine/Binaries/ThirdParty")).unwrap();
        let unreal = exe(&lies.join("LiesofP/Binaries/Win64/LOP-Win64-Shipping.exe"), Exe { fixed: Some([4, 27, 2, 0]), ..Default::default() });
        assert_eq!(read_one(&game("proton", &unreal)).installed, "", "Unreal's fixed block is the engine's");

        let edge = env.path().join("games/Mirror's Edge");
        let gog = exe(&edge.join("Binaries/MirrorsEdge.exe"), Exe::default());
        write(&edge.join("goggame-1893001152.info"), br#"{"gameId":"1893001152","rootGameId":"1893001152","buildId":"52130633394402702"}"#);
        let mut g = game("proton", &gog);
        g.source.dir = edge.join("Binaries").to_string_lossy().into();
        let read = read_one(&g);
        assert_eq!((read.channel, read.build.as_str(), read.title_id.as_str()), ("gog", "52130633394402702", "1893001152"), "above an older import's folder");

        let mut pinned = game("wine", &unity);
        pinned.version.pin = "1.8.4".into();
        let read = read_one(&pinned);
        assert_eq!((read.installed.as_str(), read.from.as_str(), read.channel), ("1.8.4", "pin", "pc"));
        assert_eq!(read.modified.len(), "2026-10-18".len());
    }

    #[test]
    fn a_program_is_read_again_only_once_its_size_or_time_changed() {
        let env = paths::test_env();
        let p = env.path().join("games/game.exe");
        exe(&p, Exe { strings: &[("ProductVersion", "1.0.1")], ..Default::default() });
        let was = std::fs::metadata(&p).unwrap().modified().unwrap();
        let g = game("proton", &p);
        assert_eq!(read_one(&g).installed, "1.0.1");
        exe(&p, Exe { strings: &[("ProductVersion", "1.0.2")], ..Default::default() });
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(was).unwrap();
        assert_eq!(read_one(&g).installed, "1.0.1", "same size, same time: the cache's");
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(was + std::time::Duration::from_secs(60)).unwrap();
        assert_eq!(read_one(&g).installed, "1.0.2");
        std::fs::remove_file(&p).unwrap();
        exe(&env.path().join("games/other.exe"), Exe { strings: &[("ProductVersion", "3.0")], ..Default::default() });
        read_one(&game("proton", &env.path().join("games/other.exe")));
        let cached = std::fs::read_to_string(paths::cache_home().join("versions.json")).unwrap();
        assert!(cached.contains("other.exe") && !cached.contains("game.exe"), "a file gone leaves the cache: {cached}");
    }

    #[test]
    fn a_switch_game_reads_the_update_its_emulator_applies_else_its_name() {
        let env = paths::test_env();
        let roms = env.path().join("roms");
        write(
            &env.path().join("home/.cache/eden/game_list/0100000000010000.pv.txt"),
            b"[D] Update (1.3.0)\nUpdate (NAND) (1.2.0)\n[1920x1080 v1.4.1] (IPSwitch)\n",
        );
        let odyssey = write(&roms.join("Super Mario Odyssey [0100000000010000][v0].nsp"), b"");
        let read = read_one(&game("eden", &odyssey));
        assert_eq!(
            (read.installed.as_str(), read.from.as_str(), read.title_id.as_str(), read.channel),
            ("1.2.0", "eden", "0100000000010000", "switch"),
            "an update turned off is not applied"
        );

        let xeno = write(&roms.join("Xenoblade-Chronicles-Definitive-Edition-V1.1.2-Eur.xci"), b"");
        let mut g = game("ryujinx", &xeno);
        g.version.switch_title_id = "0100FF500E34A000".into();
        let updates = r#"{"selected": "/roms/updates/Xenoblade [0100FF500E34A800][v196608].nsp", "paths": []}"#;
        write(&env.path().join("home/.config/Ryujinx/games/0100ff500e34a000/updates.json"), updates.as_bytes());
        write(&env.path().join("home/.cache/eden/game_list/0100FF500E34A000.pv.txt"), b"Update (1.1.0)\n");
        let read = read_one(&g);
        assert_eq!((read.title_id.as_str(), read.build.as_str(), read.from.as_str()), ("0100ff500e34a000", "196608", ""), "the emulator that runs it first");
        g.launch.runner = "eden".into();
        assert_eq!(read_one(&g).installed, "1.1.0");

        let galaxy = write(&roms.join("Super Mario Galaxy 2 v1.2.0 [v131072].xci"), b"");
        let read = read_one(&game("eden", &galaxy));
        assert_eq!((read.installed.as_str(), read.build.as_str(), read.from.as_str(), read.title_id.as_str()), ("1.2.0", "131072", "name", ""));
    }

    #[test]
    fn a_ps3_game_reads_the_higher_of_its_discs_param_sfo_and_rpcs3s_update() {
        let env = paths::test_env();
        let disc = env.path().join("ps3/Disney Bolt (Europe)/PS3_GAME");
        write(&disc.join("PARAM.SFO"), &crate::roms::tests::sfo_of(&[("TITLE", "Bolt"), ("TITLE_ID", "BLES00142"), ("APP_VER", "01.00")]));
        let eboot = write(&disc.join("USRDIR/EBOOT.BIN"), b"");
        let read = read_one(&game("rpcs3", &eboot));
        assert_eq!((read.installed.as_str(), read.from.as_str(), read.title_id.as_str(), read.channel), ("01.00", "param.sfo", "BLES00142", "ps3"));
        let hdd = env.path().join("home/.config/rpcs3/dev_hdd0/game/BLES00142/PARAM.SFO");
        write(&hdd, &crate::roms::tests::sfo_of(&[("TITLE_ID", "BLES00142"), ("APP_VER", "01.02")]));
        assert_eq!((read_one(&game("rpcs3", &eboot)).installed.as_str(), read_one(&game("rpcs3", &eboot)).from.as_str()), ("01.02", "rpcs3"));
        write(&hdd, &crate::roms::tests::sfo_of(&[("TITLE_ID", "BLES00142"), ("APP_VER", "00.90")]));
        assert_eq!(read_one(&game("rpcs3", &eboot)).installed, "01.00");
    }

    #[test]
    fn an_older_consoles_game_is_final_and_names_its_revision() {
        let env = paths::test_env();
        let rom = write(&env.path().join("roms/New Super Mario Bros. Wii (Europe) (Rev 2).rvz"), b"");
        let read = read_one(&game("dolphin", &rom));
        assert_eq!((read.installed.as_str(), read.from.as_str(), read.channel), ("Rev 2", "name", "final"));
        assert_eq!(name_version(Path::new("Mario_Kart_102_EU.nds")).as_deref(), Some("1.0.2"));
        assert_eq!(read_one(&game("azahar", &rom)).channel, "", "a console with updates no list tracks");
    }

    #[test]
    fn set_takes_a_version_key_as_it_reads_it() {
        assert_eq!(checked("switch_title_id", "0100FF500E34A000").unwrap(), "0100ff500e34a000");
        assert!(checked("switch_title_id", "0100").is_err());
        assert!(checked("probe", "GTA5.exe").is_err() && checked("probe", "@v.txt:(").is_err() && checked("probe", "@:x").is_err());
        assert_eq!(checked("probe", "@GTA5_Enhanced.exe").unwrap(), "@GTA5_Enhanced.exe");
        assert!(checked("final", "yes").is_err() && checked("installed", "1").is_err());
    }
}
