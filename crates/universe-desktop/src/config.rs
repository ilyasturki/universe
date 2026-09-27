pub const APP_ID: &str = "io.github.ilyasturki.UniverseDesktop";
pub const GETTEXT_PACKAGE: &str = "universe-desktop";
pub const LOCALE_DIR: &str = match option_env!("UNIVERSE_DESKTOP_LOCALEDIR") {
    Some(dir) => dir,
    None => "/usr/share/locale",
};
