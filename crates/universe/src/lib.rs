pub mod cli;
pub mod config;
pub mod controller;
pub mod core;
pub mod desktop;
pub mod doctor;
pub mod game;
pub mod gamescope;
pub mod gpu;
pub mod host;
pub mod inputplumber;
pub mod journal;
pub mod launch_keys;
pub mod launcher;
pub mod library;
pub mod lutris;
pub mod mangoapp;
pub mod media;
pub mod modules;
pub mod nest;
pub mod paths;
pub mod recording;
pub mod runners;
pub mod screenshots;
pub mod session;
pub mod sessions;
pub mod slug;
pub mod sources;
pub mod splash;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
// The version with the short git rev behind it (build.rs), `-dirty` when the tree was.
pub const BUILD: &str = env!("UNIVERSE_BUILD");

/// Warnings to stderr (`RUST_LOG` overrides); the CLI and the Python binding both call it, a second call is a no-op.
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into())).with_target(false).try_init();
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
        let (Ok(modules), Ok(sources)) = (std::fs::read_dir(root.join("modules")), std::fs::read_dir(root.join("sources"))) else { return };
        let mut files = vec![root.join("ui/pyproject.toml"), root.join("docs/api.md")];
        files.extend(modules.flatten().map(|d| d.path().join("module.toml")).filter(|p| p.exists()));
        files.extend(sources.flatten().map(|d| d.path().join("source.toml")).filter(|p| p.exists()));
        let version = format!("version = \"{}\"", super::VERSION);
        for f in files {
            let s = std::fs::read_to_string(&f).unwrap();
            assert!(s.lines().any(|l| l == version), "{}: no `{version}`", f.display());
        }
    }
}
