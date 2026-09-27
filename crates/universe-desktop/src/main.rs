mod actions;
mod app;
mod backend;
mod config;
mod covers;
mod dialogs;
mod form_view;
mod format;
mod game;
mod jobs;
mod library;
mod pages;
mod play;
mod qr;
mod script;
mod state;
mod widgets;
mod window;

use gettextrs::LocaleCategory;
use gtk::prelude::*;
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    universe::init_tracing();
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    let _ = gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALE_DIR);
    let _ = gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = gettextrs::textdomain(config::GETTEXT_PACKAGE);
    gio::resources_register_include!("universe-desktop.gresource").expect("the bundled resources");
    glib::set_application_name("Universe");
    app::Application::new().run()
}
