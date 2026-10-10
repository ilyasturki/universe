pub mod achievements;
pub mod bluetooth;
pub mod changelog;
pub mod changes;
pub mod cli;
pub mod components;
pub mod config;
pub mod controller;
pub mod core;
pub mod data;
pub mod deck;
pub mod desktop;
pub mod discover;
pub mod distro;
pub mod doctor;
pub mod extensions;
pub mod forms;
pub mod frames;
pub mod game;
pub mod gamescope;
pub mod gpu;
pub mod hardware;
pub mod host;
pub mod journal;
pub mod keyboard;
pub mod launch_keys;
pub mod launcher;
pub mod library;
pub mod logind;
pub mod lutris;
pub mod mangoapp;
pub mod media;
pub mod modules;
pub mod nest;
pub mod network;
pub mod packagekit;
pub mod paths;
pub mod recording;
pub mod roms;
pub mod runners;
pub mod saves;
pub mod screenshots;
pub mod session;
pub mod sessions;
pub mod slug;
pub mod sound;
pub mod sources;
pub mod splash;
pub mod system_install;
pub mod thumbs;
pub mod timeline;
pub mod tools;

#[cfg(test)]
mod testbus;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
// The version with the short git rev behind it (build.rs), `-dirty` when the tree was.
pub const BUILD: &str = env!("UNIVERSE_BUILD");

/// Warnings and each screenshot's timings to stderr (`RUST_LOG` overrides); the CLI and the Python binding both call it, a second call is a no-op.
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,screenshot=info".into()))
        .with_target(false)
        .try_init();
}

/// SIGKILL to the child when the thread that spawned it ends, the process's exit included, which `kill_on_drop` never sees.
pub fn dies_with_parent(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: the closure runs between fork and exec, and calls only async-signal-safe functions and allocates nothing.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL as libc::c_ulong) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // The parent may have died before prctl armed the signal.
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("ambiguous: {0}")]
    Ambiguous(String),
    #[error("busy: {0}")]
    Busy(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("unavailable: {0}")]
    Unavailable(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}
impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Invalid(e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Invalid(e.to_string())
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Io(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod version_tests {
    // The checked-in copies `just bump` rewrites; absent under nix, which builds crates/ alone.
    #[test]
    fn copies_match_cargo_version() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = vec![root.join("ui/pyproject.toml"), root.join("docs/api.md")];
        // The flake's core build carries only part of the tree.
        let (Ok(modules), Ok(sources), true) =
            (std::fs::read_dir(root.join("modules")), std::fs::read_dir(root.join("sources")), files.iter().all(|f| f.exists()))
        else {
            return;
        };
        files.extend(modules.flatten().map(|d| d.path().join("module.toml")).filter(|p| p.exists()));
        files.extend(sources.flatten().map(|d| d.path().join("source.toml")).filter(|p| p.exists()));
        let version = format!("version = \"{}\"", super::VERSION);
        for f in files {
            let s = std::fs::read_to_string(&f).unwrap();
            assert!(s.lines().any(|l| l == version), "{}: no `{version}`", f.display());
        }
    }
}
