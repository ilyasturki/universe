use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::game::Game;
use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Proton,
    Wine,
    Linux,
    Emulator,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Proton => "proton",
            Kind::Wine => "wine",
            Kind::Linux => "linux",
            Kind::Emulator => "emulator",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionSpec {
    pub key: &'static str,
    pub kind: &'static str,
    pub default: &'static str,
    pub label: &'static str,
    pub argument: &'static str,
    pub off_argument: &'static str,
}

const fn bool_opt(key: &'static str, default: bool, label: &'static str, argument: &'static str, off: &'static str) -> OptionSpec {
    OptionSpec { key, kind: "bool", default: if default { "true" } else { "false" }, label, argument, off_argument: off }
}

const fn path_opt(key: &'static str, label: &'static str, argument: &'static str) -> OptionSpec {
    OptionSpec { key, kind: "path", default: "", label, argument, off_argument: "" }
}

pub const INPUTPLUMBER: OptionSpec = bool_opt("inputplumber", true, "Manage pads with InputPlumber", "", "");

#[derive(Debug, Clone)]
pub struct RunnerSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    /// Lutris ids and forks sharing the CLI.
    pub aliases: &'static [&'static str],
    pub lutris: &'static str,
    pub binaries: &'static [&'static str],
    pub platforms: &'static [&'static str],
    pub extensions: &'static [&'static str],
    pub file_flag: &'static [&'static str],
    pub options: &'static [OptionSpec],
    pub via_proton: bool,
    pub file_required: bool,
}

pub const RUNNERS: &[RunnerSpec] = &[
    RunnerSpec { id: "proton", name: "Proton", kind: Kind::Proton, aliases: &["umu"], lutris: "wine", binaries: &["umu-run"], platforms: &["windows"], extensions: &["exe", "bat", "msi"], file_flag: &[], options: &[], via_proton: false, file_required: true },
    RunnerSpec { id: "wine", name: "Wine", kind: Kind::Wine, aliases: &[], lutris: "wine", binaries: &["wine"], platforms: &["windows"], extensions: &["exe", "bat", "msi"], file_flag: &[], options: &[], via_proton: false, file_required: true },
    RunnerSpec { id: "linux", name: "Linux", kind: Kind::Linux, aliases: &["native"], lutris: "linux", binaries: &[], platforms: &["linux"], extensions: &[], file_flag: &[], options: &[], via_proton: false, file_required: true },
    RunnerSpec {
        id: "dolphin", name: "Dolphin", kind: Kind::Emulator, aliases: &[], lutris: "dolphin", binaries: &["dolphin-emu"],
        platforms: &["Nintendo GameCube", "Nintendo Wii"], extensions: &["iso", "gcm", "gcz", "ciso", "wbfs", "rvz", "wia", "dol", "elf", "m3u", "wad"],
        file_flag: &["-e"],
        options: &[bool_opt("batch", true, "Batch mode (quit with the game)", "--batch", ""), path_opt("user_directory", "User directory", "-u"), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "eden", name: "Eden", kind: Kind::Emulator, aliases: &["yuzu", "citron", "sudachi", "suyu"], lutris: "yuzu", binaries: &["eden", "citron", "sudachi", "suyu", "yuzu"],
        platforms: &["Nintendo Switch"], extensions: &["nsp", "xci", "nca", "nro", "nso", "nsz", "xcz"],
        file_flag: &["-g"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "ryujinx", name: "Ryujinx", kind: Kind::Emulator, aliases: &["ryubing"], lutris: "ryujinx", binaries: &["Ryujinx", "ryujinx", "Ryujinx.Headless.SDL2"],
        platforms: &["Nintendo Switch"], extensions: &["nsp", "xci", "nca", "nro", "nso"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "--fullscreen", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "rpcs3", name: "RPCS3", kind: Kind::Emulator, aliases: &[], lutris: "rpcs3", binaries: &["rpcs3"],
        platforms: &["Sony PlayStation 3"], extensions: &["bin", "self", "elf", "pkg"],
        file_flag: &[],
        options: &[bool_opt("nogui", true, "No GUI (quit with the game)", "--no-gui", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "pcsx2", name: "PCSX2", kind: Kind::Emulator, aliases: &[], lutris: "pcsx2", binaries: &["pcsx2-qt", "pcsx2", "PCSX2"],
        platforms: &["Sony PlayStation 2"], extensions: &["iso", "chd", "cso", "zso", "gz", "bin", "elf", "irx"],
        file_flag: &["--"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-fullscreen", ""), bool_opt("nogui", true, "No GUI (quit with the game)", "-nogui", ""), bool_opt("full_boot", false, "Full boot (BIOS screen)", "-slowboot", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "duckstation", name: "DuckStation", kind: Kind::Emulator, aliases: &[], lutris: "duckstation", binaries: &["duckstation-qt", "duckstation-nogui", "DuckStation", "duckstation"],
        platforms: &["Sony PlayStation"], extensions: &["cue", "bin", "chd", "iso", "img", "pbp", "ecm", "mds", "m3u", "psexe", "exe"],
        file_flag: &["--"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-fullscreen", ""), bool_opt("nogui", true, "No GUI (quit with the game)", "-nogui", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "cemu", name: "Cemu", kind: Kind::Emulator, aliases: &[], lutris: "cemu", binaries: &["Cemu", "cemu"],
        platforms: &["Nintendo Wii U"], extensions: &["wud", "wux", "wua", "rpx", "iso", "elf"],
        file_flag: &["-g"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f", ""), path_opt("mlc", "MLC folder", "-m"), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "azahar", name: "Azahar", kind: Kind::Emulator, aliases: &["citra", "lime3ds"], lutris: "azahar", binaries: &["azahar", "azahar-qt", "lime3ds", "citra-qt", "citra"],
        platforms: &["Nintendo 3DS"], extensions: &["3ds", "3dsx", "cci", "cxi", "cia", "app", "elf", "axf"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "melonds", name: "melonDS", kind: Kind::Emulator, aliases: &[], lutris: "melonds", binaries: &["melonDS", "melonds"],
        platforms: &["Nintendo DS"], extensions: &["nds", "dsi", "ids", "srl"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "--fullscreen", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "mgba", name: "mGBA", kind: Kind::Emulator, aliases: &[], lutris: "mgba", binaries: &["mgba-qt", "mgba"],
        platforms: &["Nintendo Game Boy Advance", "Nintendo Game Boy"], extensions: &["gba", "gb", "gbc", "agb", "mb"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "ppsspp", name: "PPSSPP", kind: Kind::Emulator, aliases: &[], lutris: "ppsspp", binaries: &["PPSSPPSDL", "PPSSPPQt", "ppsspp", "ppsspp-sdl", "ppsspp-qt"],
        platforms: &["Sony PlayStation Portable"], extensions: &["iso", "cso", "chd", "pbp", "elf", "prx"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "--fullscreen", ""), bool_opt("pause_exit", true, "Quit from the pause menu", "--pause-menu-exit", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "xemu", name: "xemu", kind: Kind::Emulator, aliases: &[], lutris: "xemu", binaries: &["xemu"],
        platforms: &["Microsoft Xbox"], extensions: &["iso", "xiso"],
        file_flag: &["-dvd_path"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-full-screen", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "xenia", name: "Xenia", kind: Kind::Emulator, aliases: &["xenia-canary"], lutris: "xenia", binaries: &["xenia_canary.exe", "xenia.exe"],
        platforms: &["Microsoft Xbox 360"], extensions: &["iso", "xex", "zar", "xcp"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "--fullscreen", ""), INPUTPLUMBER],
        via_proton: true, file_required: true,
    },
    RunnerSpec {
        id: "shadps4", name: "shadPS4", kind: Kind::Emulator, aliases: &[], lutris: "shadps4", binaries: &["shadps4", "shadPS4", "shadps4-qt"],
        platforms: &["Sony PlayStation 4"], extensions: &["bin", "elf", "self", "pkg"],
        file_flag: &["-g"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f true", "-f false"), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "vita3k", name: "Vita3K", kind: Kind::Emulator, aliases: &[], lutris: "vita3k", binaries: &["Vita3K", "vita3k"],
        platforms: &["Sony PlayStation Vita"], extensions: &["vpk"],
        file_flag: &["-r"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-F", ""), INPUTPLUMBER],
        via_proton: false, file_required: false, // -r takes a title id, not a path
    },
    RunnerSpec {
        id: "mupen64plus", name: "Mupen64Plus", kind: Kind::Emulator, aliases: &["m64p"], lutris: "mupen64plus", binaries: &["mupen64plus", "m64p"],
        platforms: &["Nintendo 64"], extensions: &["n64", "z64", "v64", "rom"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "--fullscreen", "--windowed"), bool_opt("hide_osd", true, "Hide the on-screen display", "--noosd", "--osd"), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "snes9x", name: "Snes9x", kind: Kind::Emulator, aliases: &[], lutris: "snes9x", binaries: &["snes9x-gtk", "snes9x"],
        platforms: &["Nintendo SNES"], extensions: &["sfc", "smc", "fig", "swc", "bs"],
        file_flag: &[],
        options: &[INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "flycast", name: "Flycast", kind: Kind::Emulator, aliases: &["reicast"], lutris: "reicast", binaries: &["flycast", "reicast"],
        platforms: &["Sega Dreamcast"], extensions: &["gdi", "cdi", "chd", "cue", "elf", "bin"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-config window:fullscreen=yes", "-config window:fullscreen=no"), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "scummvm", name: "ScummVM", kind: Kind::Emulator, aliases: &[], lutris: "scummvm", binaries: &["scummvm"],
        platforms: &["ScummVM"], extensions: &[],
        file_flag: &["--auto-detect", "-p"],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-f", ""), bool_opt("subtitles", false, "Subtitles", "-n", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "dosbox", name: "DOSBox", kind: Kind::Emulator, aliases: &["dosbox-staging", "dosbox-x"], lutris: "dosbox", binaries: &["dosbox-staging", "dosbox-x", "dosbox"],
        platforms: &["MS-DOS"], extensions: &["exe", "com", "bat", "conf"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "-fullscreen", ""), bool_opt("exit", true, "Quit with the program", "-exit", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
    RunnerSpec {
        id: "mame", name: "MAME", kind: Kind::Emulator, aliases: &[], lutris: "mame", binaries: &["mame"],
        platforms: &["Arcade"], extensions: &["zip", "7z", "chd"],
        file_flag: &[],
        options: &[bool_opt("fullscreen", true, "Fullscreen", "", "-window"), bool_opt("skip_gameinfo", true, "Skip the game info screen", "-skip_gameinfo", ""), INPUTPLUMBER],
        via_proton: false, file_required: true,
    },
];

pub fn spec(id: &str) -> Option<&'static RunnerSpec> {
    let id = canonical(id);
    RUNNERS.iter().find(|r| r.id == id)
}

pub fn canonical(name: &str) -> String {
    let n = name.trim().to_lowercase();
    RUNNERS.iter().find(|r| r.id == n || r.aliases.contains(&n.as_str())).map(|r| r.id.to_string()).unwrap_or(n)
}

impl RunnerSpec {
    pub fn option(&self, key: &str) -> Option<&OptionSpec> {
        self.options.iter().find(|o| o.key == key)
    }

    pub fn default_platform(&self) -> &'static str {
        self.platforms.first().copied().unwrap_or("")
    }

    pub fn merged_options(&self, config: &Config, game: Option<&Game>) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        for o in self.options {
            out.insert(o.key.into(), typed_value(o, o.default));
        }
        let global = config.runners.get(self.id).into_iter().flatten();
        for (k, v) in global.chain(game.into_iter().flat_map(|g| &g.launch.options)) {
            if let Some(o) = self.option(k) {
                out.insert(k.clone(), coerce(o, v));
            }
        }
        out
    }

    pub fn option_args(&self, options: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
        let mut args = Vec::new();
        for o in self.options {
            let v = options.get(o.key);
            match o.kind {
                "bool" => {
                    let on = v.and_then(|v| v.as_bool()).unwrap_or(o.default == "true");
                    let flag = if on { o.argument } else { o.off_argument };
                    args.extend(flag.split_whitespace().map(String::from));
                }
                _ => {
                    let s = v.and_then(|v| v.as_str()).unwrap_or("").to_string();
                    if !s.is_empty() && !o.argument.is_empty() {
                        args.extend(o.argument.split_whitespace().map(String::from));
                        args.push(if o.kind == "path" { paths::expand(&s).to_string_lossy().into() } else { s });
                    }
                }
            }
        }
        args
    }

    pub fn validate_option(&self, key: &str, value: &str) -> crate::Result<()> {
        let o = self.option(key).ok_or_else(|| crate::Error::Invalid(format!("{}: unknown option {key}", self.id)))?;
        match o.kind {
            "bool" => match value {
                "true" | "false" => Ok(()),
                _ => Err(crate::Error::Invalid(format!("{key} must be true or false"))),
            },
            _ => Ok(()),
        }
    }

    pub fn options_json(&self, config: &Config) -> serde_json::Value {
        let merged = self.merged_options(config, None);
        serde_json::Value::Array(
            self.options
                .iter()
                .map(|o| {
                    serde_json::json!({
                        "key": o.key, "type": o.kind, "default": typed_value(o, o.default), "label": o.label,
                        "choices": [], "value": merged.get(o.key).cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect(),
        )
    }
}

fn typed_value(o: &OptionSpec, s: &str) -> serde_json::Value {
    match o.kind {
        "bool" => serde_json::Value::Bool(s == "true"),
        _ => serde_json::Value::String(s.to_string()),
    }
}

fn coerce(o: &OptionSpec, v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => typed_value(o, s),
        other => crate::modules::toml_to_json(other),
    }
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Located {
    pub program: String,
    /// `config`, `path`, `lutris`, or empty when nothing was found
    pub source: String,
}

pub(crate) fn on_path(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        let p = paths::expand(bin);
        return p.is_file().then_some(p);
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push("/run/wrappers/bin".into());
    dirs.iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

fn executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file() && std::fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

fn in_lutris_dir(config: &Config, spec: &RunnerSpec) -> Option<PathBuf> {
    let root = paths::expand(&config.lutris.runners_dir).parent()?.join(spec.lutris);
    let mut stack = vec![(root, 0u8)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                if depth < 2 {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let appimage = name.to_lowercase().ends_with(".appimage");
            if (spec.binaries.contains(&name) || appimage) && executable(&p) {
                return Some(p);
            }
        }
    }
    None
}

pub fn locate(spec: &RunnerSpec, config: &Config) -> Located {
    if spec.kind == Kind::Linux {
        return Located { program: String::new(), source: "path".into() };
    }
    if let Some(exe) = config.runners.get(spec.id).and_then(|t| t.get("exe")).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
        let program = on_path(exe).unwrap_or_else(|| paths::expand(exe));
        return Located { program: program.to_string_lossy().into(), source: "config".into() };
    }
    if spec.kind == Kind::Proton {
        return Located { program: on_path(&config.launch.umu_run).map(|p| p.to_string_lossy().into()).unwrap_or_default(), source: "path".into() };
    }
    for b in spec.binaries {
        if let Some(p) = on_path(b) {
            return Located { program: p.to_string_lossy().into(), source: "path".into() };
        }
    }
    if let Some(p) = in_lutris_dir(config, spec) {
        return Located { program: p.to_string_lossy().into(), source: "lutris".into() };
    }
    Located::default()
}

pub fn global_args(spec: &RunnerSpec, config: &Config) -> Vec<String> {
    match config.runners.get(spec.id).and_then(|t| t.get("args")) {
        Some(toml::Value::String(s)) => shell_words::split(s).unwrap_or_else(|_| vec![s.clone()]),
        Some(toml::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

pub fn file_args(spec: &RunnerSpec, file: &Path) -> Vec<String> {
    let f = file.to_string_lossy().to_string();
    let dir = file.parent().map(|p| p.to_string_lossy().to_string());
    let target = match spec.id {
        "mame" => return vec!["-rompath".into(), dir.unwrap_or_default(), file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(f)],
        "dosbox" if f.to_lowercase().ends_with(".conf") => return vec!["-conf".into(), f],
        "scummvm" if !file.is_dir() => dir.unwrap_or(f),
        _ => f,
    };
    spec.file_flag.iter().map(|s| s.to_string()).chain([target]).collect()
}

pub fn to_json(spec: &RunnerSpec, config: &Config) -> serde_json::Value {
    let located = locate(spec, config);
    let available = match spec.kind {
        Kind::Linux => true,
        Kind::Proton => !located.program.is_empty() && config.proton_path(&config.launch.proton).is_some(),
        _ => !located.program.is_empty(),
    };
    let configured = config.runners.get(spec.id);
    serde_json::json!({
        "id": spec.id, "name": spec.name, "kind": spec.kind.as_str(), "aliases": spec.aliases, "lutris": spec.lutris,
        "binaries": spec.binaries, "platforms": spec.platforms, "extensions": spec.extensions,
        "exe": configured.and_then(|t| t.get("exe")).and_then(|v| v.as_str()).unwrap_or(""),
        "args": shell_words::join(global_args(spec, config)),
        "gamescope": configured.and_then(|t| t.get("gamescope")).and_then(|v| v.as_bool()),
        "path": located.program, "source": located.source, "available": available,
        "options": spec.options_json(config),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_resolve() {
        assert_eq!(canonical("yuzu"), "eden");
        assert_eq!(canonical("Citra"), "azahar");
        assert_eq!(canonical("native"), "linux");
        assert_eq!(canonical("reicast"), "flycast");
        assert_eq!(canonical("dolphin"), "dolphin");
        assert_eq!(canonical("unknown-thing"), "unknown-thing");
        assert!(spec("wine").is_some() && spec("umu").is_some());
    }

    #[test]
    fn options_merge_and_flag() {
        let s = spec("dolphin").unwrap();
        let mut cfg = Config::default();
        let mut t = toml::Table::new();
        t.insert("batch".into(), toml::Value::Boolean(false));
        t.insert("user_directory".into(), toml::Value::String("~/dolphin".into()));
        cfg.runners.insert("dolphin".into(), t);
        let merged = s.merged_options(&cfg, None);
        assert_eq!(merged["batch"], false);
        assert_eq!(merged["inputplumber"], true);
        let args = s.option_args(&merged);
        assert_eq!(args[0], "-u");
        assert!(args[1].ends_with("/dolphin"));
        let mut g = Game::new("x");
        g.launch.options.insert("batch".into(), toml::Value::Boolean(true));
        let merged = s.merged_options(&cfg, Some(&g));
        assert_eq!(s.option_args(&merged)[0], "--batch");
        assert!(s.validate_option("batch", "maybe").is_err());
        assert!(s.validate_option("nope", "1").is_err());
    }

    #[test]
    fn file_args_shapes() {
        assert_eq!(file_args(spec("dolphin").unwrap(), Path::new("/g/F-Zero GX.iso")), vec!["-e", "/g/F-Zero GX.iso"]);
        assert_eq!(file_args(spec("mame").unwrap(), Path::new("/roms/sf2.zip")), vec!["-rompath", "/roms", "sf2"]);
        assert_eq!(file_args(spec("dosbox").unwrap(), Path::new("/d/game.conf")), vec!["-conf", "/d/game.conf"]);
        assert_eq!(file_args(spec("ryujinx").unwrap(), Path::new("/s/a.nsp")), vec!["/s/a.nsp"]);
        let sh = spec("shadps4").unwrap();
        assert_eq!(sh.option_args(&sh.merged_options(&Config::default(), None))[..2], ["-f", "true"]);
    }
}
