use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib::subclass::Signal;
use gtk::{gio, glib};
use universe::changes::{self, Event};

use crate::backend;
use crate::config;
use crate::script::{self, Step};
use crate::window::Window;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct Application {
        pub ready: Cell<bool>,
        pub failure: RefCell<Option<String>>,
        pub watch: RefCell<Option<changes::Watch>>,
        pub script: RefCell<Option<Vec<Step>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "UniverseApplication";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![
                    Signal::builder("core-ready").build(),
                    Signal::builder("core-failed").param_types([String::static_type()]).build(),
                    Signal::builder("changed").param_types([glib::BoxedAnyObject::static_type()]).build(),
                ]
            })
        }
    }

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();
            app.setup_actions();
            app.setup_accels();
            app.open_core();
        }

        fn activate(&self) {
            self.parent_activate();
            self.obj().window().present();
        }
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends adw::Application, gtk::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        let script = script::from_env();
        let flags = if script.is_some() { gio::ApplicationFlags::NON_UNIQUE } else { gio::ApplicationFlags::empty() };
        let app: Self = glib::Object::builder()
            .property("application-id", config::APP_ID)
            .property("resource-base-path", "/io/github/ilyasturki/UniverseDesktop")
            .property("flags", flags)
            .build();
        app.imp().script.replace(script);
        app
    }

    pub fn scripted(&self) -> bool {
        self.imp().script.borrow().is_some()
    }

    pub fn take_script(&self) -> Option<Vec<Step>> {
        self.imp().script.take()
    }

    pub fn is_ready(&self) -> bool {
        self.imp().ready.get()
    }

    pub fn failure(&self) -> Option<String> {
        self.imp().failure.borrow().clone()
    }

    fn window(&self) -> Window {
        self.active_window().and_downcast::<Window>().unwrap_or_else(|| Window::new(self))
    }

    /// The core opens off the main loop; the games it launches from here on are bound to this process, which owns them.
    fn open_core(&self) {
        let app = self.clone();
        glib::spawn_future_local(async move {
            if let Err(e) = backend::open().await {
                app.imp().failure.replace(Some(e.to_string()));
                app.emit_by_name::<()>("core-failed", &[&e.to_string()]);
                return;
            }
            let scripted = app.scripted();
            if !scripted {
                if let Err(e) = backend::call(|core| async move { core.adopt_scope().await }).await {
                    tracing::warn!("adopt_scope: {e}");
                }
            }
            let options = changes::Options { journal_sweep: !scripted, ..changes::Options::default() };
            let core = backend::core();
            match backend::run(changes::watch(core, options)).await {
                Ok((watch, events)) => {
                    app.imp().watch.replace(Some(watch));
                    app.follow(events);
                }
                Err(e) => tracing::warn!("change watch: {e}"),
            }
            app.imp().ready.set(true);
            app.emit_by_name::<()>("core-ready", &[]);
        });
    }

    fn follow(&self, mut events: tokio::sync::mpsc::UnboundedReceiver<Event>) {
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            while let Some(event) = events.recv().await {
                let Some(app) = app.upgrade() else { return };
                app.emit_by_name::<()>("changed", &[&glib::BoxedAnyObject::new(event)]);
            }
        });
    }

    pub fn connect_changed<F: Fn(&Event) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("changed", false, move |args| {
            let boxed = args[1].get::<glib::BoxedAnyObject>().expect("an event");
            f(&boxed.borrow::<Event>());
            None
        })
    }

    fn setup_actions(&self) {
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &Self, _, _| {
                for window in app.windows() {
                    window.close();
                }
            })
            .build();
        let about = gio::ActionEntry::builder("about").activate(|app: &Self, _, _| app.show_about()).build();
        self.add_action_entries([quit, about]);
    }

    fn setup_accels(&self) {
        for (action, accels) in [
            ("app.quit", &["<Control>q"][..]),
            ("app.preferences", &["<Control>comma"]),
            ("app.shortcuts", &["<Control>question"]),
            ("win.search", &["<Control>f"]),
            ("win.add-game", &["<Control>n"]),
            ("win.show-hidden", &["<Control>h"]),
            ("win.undo", &["<Control>z"]),
            ("win.show-sidebar", &["F9"]),
            ("window.close", &["<Control>w"]),
        ] {
            self.set_accels_for_action(action, accels);
        }
    }

    fn show_about(&self) {
        let about = adw::AboutDialog::builder()
            .application_name(gettext("Universe Desktop"))
            .application_icon(config::APP_ID)
            .developer_name("Ilyas Turki")
            .version(universe::BUILD)
            .license_type(gtk::License::MitX11)
            .website("https://github.com/ilyasturki/universe")
            .issue_url("https://github.com/ilyasturki/universe/issues")
            .comments(gettext("Your games, from every store and emulator, in one library"))
            .build();
        about.present(self.active_window().as_ref());
    }
}
