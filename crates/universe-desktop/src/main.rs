mod actions;
mod app;
mod backend;
mod components;
mod config;
mod covers;
mod dialogs;
mod form_view;
mod format;
mod game;
mod jobs;
mod library;
mod media;
mod menus;
mod pages;
mod play;
mod qr;
mod rows;
mod script;
mod search;
mod state;
mod watcher;
mod widgets;
mod window;

use gettextrs::LocaleCategory;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    // The core runs `$UNIVERSE_BIN` for session hooks, falling back to this binary, which is not the CLI.
    if std::env::var_os("UNIVERSE_BIN").is_none_or(|bin| bin.is_empty()) {
        if let Some(cli) = glib::find_program_in_path("universe") {
            std::env::set_var("UNIVERSE_BIN", cli);
        }
    }
    universe::init_tracing();
    // SAFETY: no other thread exists yet to read the locale.
    unsafe { gettextrs::setlocale(LocaleCategory::LcAll, "") };
    let _ = gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALE_DIR);
    let _ = gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = gettextrs::textdomain(config::GETTEXT_PACKAGE);
    gio::resources_register_include!("universe-desktop.gresource").expect("the bundled resources");
    glib::set_application_name("Universe");
    app::Application::new().run()
}
