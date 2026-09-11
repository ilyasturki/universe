pub mod config;
pub mod game;
pub mod index;
pub mod journal;
pub mod launcher;
pub mod library;
pub mod lutris;
pub mod media;
pub mod modules;
pub mod paths;
pub mod recording;
pub mod sessions;
pub mod slug;
pub mod desktop;
pub mod core;
pub mod dbus;
pub mod client;
pub mod cli;
pub mod doctor;

pub const BUS_NAME: &str = "io.github.ilyasturki.Universe";
pub const OBJECT_PATH: &str = "/io/github/ilyasturki/Universe";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::Io(format!("{e:#}"))
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

impl Error {
    pub fn dbus_name(&self) -> &'static str {
        match self {
            Error::NotFound(_) => "io.github.ilyasturki.Universe.Error.NotFound",
            Error::Ambiguous(_) => "io.github.ilyasturki.Universe.Error.Ambiguous",
            Error::Busy(_) => "io.github.ilyasturki.Universe.Error.Busy",
            Error::Invalid(_) => "io.github.ilyasturki.Universe.Error.Invalid",
            Error::Unavailable(_) => "io.github.ilyasturki.Universe.Error.Unavailable",
            Error::Io(_) => "io.github.ilyasturki.Universe.Error.Io",
        }
    }
}

impl From<Error> for zbus::fdo::Error {
    fn from(e: Error) -> Self {
        zbus::fdo::Error::Failed(format!("{}: {}", e.dbus_name(), e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;
