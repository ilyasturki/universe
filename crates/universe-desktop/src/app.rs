use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib::subclass::Signal;
use gtk::{gio, glib};
use universe::changes::{self, Ended, Event};
use universe::session::Current;

use crate::backend;
use crate::config;
use crate::library::Library;
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
        pub library: Library,
        pub current: RefCell<Option<Current>>,
        /// A game asked for while another ran: it starts once that one has ended.
        pub pending: RefCell<Option<String>>,
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
                    Signal::builder("session-changed").build(),
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
            app.quit_on_signals();
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

    pub fn library(&self) -> &Library {
        &self.imp().library
    }

    pub fn current(&self) -> Option<Current> {
        self.imp().current.borrow().clone()
    }

    pub fn set_pending(&self, id: Option<String>) {
        self.imp().pending.replace(id);
    }

    /// Reads the session marker now: a launch or a stop from here just returned.
    pub fn check_session(&self) {
        if let Some(watch) = self.imp().watch.borrow().as_ref() {
            watch.check();
        }
    }

    fn session_started(&self, current: &Current) {
        if let Some(game) = self.library().get(&current.id) {
            game.set_launching(false);
            game.set_playing(true);
        }
        self.imp().current.replace(Some(current.clone()));
        self.emit_by_name::<()>("session-changed", &[]);
    }

    fn session_ended(&self, ended: &Ended) {
        if let Some(game) = self.library().get(&ended.id) {
            game.set_playing(false);
            game.set_launching(false);
        }
        self.imp().current.replace(None);
        self.emit_by_name::<()>("session-changed", &[]);
        let length = crate::format::duration(ended.duration_s);
        let failure = match ended.end.as_str() {
            "crashed" => Some(gettext("{} crashed after {}")),
            "killed" => Some(gettext("{} was killed after {}")),
            _ => None,
        };
        let window = self.active_window().and_downcast::<Window>();
        match failure {
            Some(text) => {
                let title = text.replacen("{}", &ended.title, 1).replacen("{}", &length, 1);
                let notification = gio::Notification::new(&title);
                notification.set_body(Some(&gettext("Its log is under Sessions and Logs, on the game's page.")));
                notification.set_priority(gio::NotificationPriority::High);
                notification.set_icon(&gio::ThemedIcon::new(config::APP_ID));
                self.send_notification(Some("session-end"), &notification);
                if let Some(win) = &window {
                    win.toast(adw::Toast::builder().title(title).priority(adw::ToastPriority::High).build());
                }
            }
            None if ended.duration_s > 0 => {
                if let Some(win) = &window {
                    win.toast(adw::Toast::new(&format!("{} · {}", ended.title, length)));
                }
            }
            None => {}
        }
        let pending = self.imp().pending.take().and_then(|id| self.library().get(&id));
        if let (Some(game), Some(win)) = (pending, window) {
            win.play(&game);
        }
    }

    /// SIGINT and SIGTERM quit as the window's close does, the game stopped and its end run first.
    fn quit_on_signals(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        backend::runtime().spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else { return };
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
            let _ = tx.send(());
        });
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            if rx.await.is_err() {
                return;
            }
            let Some(app) = app.upgrade() else { return };
            app.end_session().await;
            app.quit();
        });
    }

    /// Stops the running game and waits for its end, `session-end` included, before the window goes.
    pub async fn end_session(&self) {
        if self.current().is_none() {
            return;
        }
        if let Err(e) = backend::call(|core| async move { core.stop("").await }).await {
            tracing::warn!("stop: {e}");
        }
        for tick in 0..300 {
            if self.current().is_none() {
                return;
            }
            if tick % 10 == 0 {
                self.check_session();
            }
            glib::timeout_future(std::time::Duration::from_millis(100)).await;
        }
        tracing::warn!("the session did not end within 30 s");
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
            let events = match backend::run(changes::watch(core, options)).await {
                Ok((watch, events)) => {
                    app.imp().watch.replace(Some(watch));
                    Some(events)
                }
                Err(e) => {
                    tracing::warn!("change watch: {e}");
                    None
                }
            };
            app.library().refresh(&[]).await;
            app.imp().ready.set(true);
            app.emit_by_name::<()>("core-ready", &[]);
            if let Some(events) = events {
                app.follow(events);
            }
        });
    }

    fn follow(&self, mut events: tokio::sync::mpsc::UnboundedReceiver<Event>) {
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            while let Some(event) = events.recv().await {
                let Some(app) = app.upgrade() else { return };
                match &event {
                    Event::Library(ids) => app.library().refresh(ids).await,
                    Event::SessionStarted(current) => app.session_started(current),
                    Event::SessionEnded(ended) => app.session_ended(ended),
                    _ => {}
                }
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
        let preferences = gio::ActionEntry::builder("preferences")
            .activate(|app: &Self, _, _| {
                if let Some(win) = app.active_window().and_downcast::<Window>() {
                    crate::dialogs::preferences::present(&win, "");
                }
            })
            .build();
        let preferences_page = gio::ActionEntry::builder("preferences-page")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|app: &Self, _, param| {
                let page = param.and_then(|p| p.get::<String>()).unwrap_or_default();
                if let Some(win) = app.active_window().and_downcast::<Window>() {
                    crate::dialogs::preferences::present(&win, &page);
                }
            })
            .build();
        self.add_action_entries([quit, about, preferences, preferences_page]);
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
