use std::cell::{Cell, RefCell};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib::subclass::Signal;
use gtk::{gio, glib};
use universe::changes::{self, Ended, Event};
use universe::frames::Frames;
use universe::session::Current;

use crate::backend;
use crate::config;
use crate::dialogs::{system_check, whats_new};
use crate::jobs::{self, Job, Kind, Outcome};
use crate::library::Library;
use crate::script::{self, Step};
use crate::window::Window;

/// The running game's unlocks as its source files them: each announced once, none from before the session.
#[derive(Debug, Default)]
pub struct Unlocks {
    game: String,
    title: String,
    since: i64,
    known: std::collections::HashSet<String>,
}

const BIG_SCREEN: &str = "universe-ui";

// Never opened: `activate-link` intercepts it.
const SYSTEM_CHECK_URI: &str = "universe-desktop:system-check";

// Started by the shell for a search, the app stays up between keystrokes rather than open the core for each.
const SEARCH_LINGER_MS: u32 = 60_000;

/// A delete held back while its toast offers Undo: what runs once the toast goes.
pub type Commit = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>>>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Application {
        pub ready: Cell<bool>,
        pub failure: RefCell<Option<String>>,
        pub watch: RefCell<Option<changes::Watch>>,
        pub script: RefCell<Option<Vec<Step>>>,
        pub library: Library,
        pub current: RefCell<Option<Current>>,
        /// A game asked for while another ran: it starts once that one has ended.
        pub pending: RefCell<Option<String>>,
        /// One install, update, scan or art refresh at a time, as a source runs one downloader.
        pub job: RefCell<Option<Job>>,
        /// What the signed-in stores have newer builds of, each with its `source`.
        pub updates: RefCell<Vec<serde_json::Value>>,
        pub updates_at: Cell<Option<Instant>>,
        pub checking: Cell<bool>,
        /// Samples the recordings' frames for the pictures that stand for them.
        pub frames: RefCell<Option<Frames>>,
        /// Deletes waiting on their Undo, by what they delete.
        pub deferred: RefCell<Vec<(String, Commit)>>,
        pub unlocks: RefCell<Unlocks>,
        /// Universe Big Screen, while the one started from here runs.
        pub big_screen: RefCell<Option<gio::Subprocess>>,
        pub search: RefCell<Option<gio::RegistrationId>>,
    }

    impl std::fmt::Debug for Application {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Application").field("ready", &self.ready).field("current", &self.current).finish_non_exhaustive()
        }
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
                    Signal::builder("job-changed").build(),
                    Signal::builder("updates-changed").build(),
                    Signal::builder("frame-landed").param_types([String::static_type(), u32::static_type()]).build(),
                    Signal::builder("deferred-changed").build(),
                ]
            })
        }
    }

    impl ApplicationImpl for Application {
        fn startup(&self) {
            self.parent_startup();
            let app = self.obj();
            if app.flags().contains(gio::ApplicationFlags::IS_SERVICE) {
                app.set_inactivity_timeout(SEARCH_LINGER_MS);
            }
            app.setup_actions();
            app.setup_accels();
            app.open_core();
            app.quit_on_signals();
        }

        fn activate(&self) {
            self.parent_activate();
            let app = self.obj();
            app.window().present();
            if app.is_ready() && self.updates_at.get().is_none() {
                app.check_updates(None);
            }
        }

        fn dbus_register(&self, connection: &gio::DBusConnection, object_path: &str) -> Result<(), glib::Error> {
            self.parent_dbus_register(connection, object_path)?;
            match crate::search::register(&self.obj(), connection) {
                Ok(id) => drop(self.search.replace(Some(id))),
                Err(e) => tracing::warn!("search provider: {e}"),
            }
            Ok(())
        }

        fn dbus_unregister(&self, connection: &gio::DBusConnection, object_path: &str) {
            if let Some(id) = self.search.take() {
                let _ = connection.unregister_object(id);
            }
            self.parent_dbus_unregister(connection, object_path);
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
        self.sync_big_screen();
        let since = chrono::DateTime::parse_from_rfc3339(&current.started_at).map(|t| t.timestamp()).unwrap_or(0);
        self.imp().unlocks.replace(Unlocks { game: current.id.clone(), title: current.title.clone(), since, known: Default::default() });
        self.look_for_unlocks();
    }

    /// A game with no list yet is not asked: the ask would reach its store on every library write.
    fn look_for_unlocks(&self) {
        let id = self.imp().unlocks.borrow().game.clone();
        if id.is_empty() {
            return;
        }
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            let asked = id.clone();
            let listing = backend::pinned(move |core| async move {
                let listed = core.get(&asked).await.is_ok_and(|r| r.achievements.total > 0);
                if listed {
                    core.achievements(&asked, false).await.ok()
                } else {
                    None
                }
            })
            .await;
            let (Some(app), Some(listing)) = (app.upgrade(), listing) else { return };
            let fresh: Vec<serde_json::Value> = {
                let mut unlocks = app.imp().unlocks.borrow_mut();
                if unlocks.game != id {
                    return;
                }
                let items: Vec<serde_json::Value> = listing["items"].as_array().cloned().unwrap_or_default();
                let mut fresh = Vec::new();
                for item in items {
                    let Some(at) = item["unlocked_at"].as_str().filter(|at| !at.is_empty()) else { continue };
                    let key = item["key"].as_str().unwrap_or_default().to_string();
                    let when = chrono::DateTime::parse_from_rfc3339(at).map(|t| t.timestamp()).unwrap_or(0);
                    if unlocks.known.insert(key) && when >= unlocks.since {
                        fresh.push(item);
                    }
                }
                fresh
            };
            let title = app.imp().unlocks.borrow().title.clone();
            for item in fresh {
                app.unlocked(&id, &title, &item);
            }
        });
    }

    fn unlocked(&self, id: &str, game: &str, item: &serde_json::Value) {
        let text = |key: &str| item[key].as_str().unwrap_or_default().to_string();
        let name = if text("name").is_empty() { text("key") } else { text("name") };
        let notification = gio::Notification::new(&gettext("Achievement Unlocked: {}").replace("{}", &name));
        let description = text("description");
        notification.set_body(Some(&if description.is_empty() { game.to_string() } else { format!("{game} · {description}") }));
        notification.set_icon(&gio::ThemedIcon::new(config::APP_ID));
        self.send_notification(Some(&format!("unlock-{id}-{}", text("key"))), &notification);
        if let Some(win) = self.active_window().and_downcast::<Window>().filter(|w| w.is_active()) {
            win.toast(adw::Toast::new(&gettext("Achievement unlocked: {}").replace("{}", &name)));
        }
    }

    fn session_ended(&self, ended: &Ended) {
        if let Some(game) = self.library().get(&ended.id) {
            game.set_playing(false);
            game.set_launching(false);
        }
        self.imp().current.replace(None);
        self.imp().unlocks.replace(Unlocks::default());
        self.emit_by_name::<()>("session-changed", &[]);
        self.sync_big_screen();
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
            app.flush_deferred().await;
            app.end_session().await;
            app.end_job().await;
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

    pub fn job(&self) -> Option<Job> {
        self.imp().job.borrow().clone()
    }

    /// Starts a job over `targets` (see `jobs::start`) unless one runs; false, and a word to the player, when it does.
    pub fn start_job(&self, kind: Kind, source: &str, targets: Vec<(String, String)>, force: bool) -> bool {
        self.begin(|| jobs::start(kind, source, targets, force))
    }

    pub fn start_component(&self, component: jobs::ComponentJob) -> bool {
        self.begin(|| jobs::start_component(component))
    }

    fn begin<F: std::future::Future<Output = Outcome> + 'static>(&self, start: impl FnOnce() -> (Job, F)) -> bool {
        let window = self.active_window().and_downcast::<Window>();
        if let Some(job) = self.job() {
            if let Some(win) = &window {
                win.toast(adw::Toast::new(&gettext("{} first: stop it or let it end").replace("{}", &job.label())));
            }
            return false;
        }
        let (job, outcome) = start();
        self.imp().job.replace(Some(job.clone()));
        self.emit_by_name::<()>("job-changed", &[]);
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            let outcome = outcome.await;
            let Some(app) = app.upgrade() else { return };
            app.library().refresh(&[]).await;
            app.imp().job.replace(None);
            app.emit_by_name::<()>("job-changed", &[]);
            app.job_ended(&job, &outcome);
            if job.pauses() && outcome.ok {
                app.check_updates(None);
            }
            app.fetch_art(outcome.arrived);
        });
        true
    }

    /// The art of games just imported, `(id, title)`: an art job when none runs, else fetched quietly beside it.
    pub fn fetch_art(&self, games: Vec<(String, String)>) {
        if games.is_empty() {
            return;
        }
        if self.job().is_none() {
            self.start_job(Kind::Artwork, "", games, false);
            return;
        }
        let (app, ids): (_, Vec<String>) = (self.downgrade(), games.into_iter().map(|(id, _)| id).collect());
        glib::spawn_future_local(async move {
            let wanted = ids.clone();
            let _ = backend::pinned(move |core| async move { core.media_refresh_many(&ids, false, None).await }).await;
            if let Some(app) = app.upgrade() {
                app.library().refresh(&wanted).await;
            }
        });
    }

    fn job_ended(&self, job: &Job, outcome: &Outcome) {
        let window = self.active_window().and_downcast::<Window>();
        let toast = adw::Toast::builder()
            .title(&outcome.text)
            .priority(if outcome.ok || job.cancelled() { adw::ToastPriority::Normal } else { adw::ToastPriority::High })
            .build();
        if !outcome.game.is_empty() {
            toast.set_button_label(Some(&gettext("_Play")));
            toast.set_action_name(Some("win.play-game"));
            toast.set_action_target_value(Some(&outcome.game.to_variant()));
        }
        let unseen = window.as_ref().is_none_or(|win| !win.is_active());
        if let Some(win) = &window {
            win.toast(toast);
        }
        if unseen && job.pauses() && !job.cancelled() {
            let notification = gio::Notification::new(&outcome.text);
            notification.set_icon(&gio::ThemedIcon::new(config::APP_ID));
            if !outcome.ok {
                notification.set_priority(gio::NotificationPriority::High);
            }
            self.send_notification(Some("job"), &notification);
        }
    }

    /// Stops the running job and waits for its end: an install or an update keeps its download for next time.
    pub async fn end_job(&self) {
        let Some(job) = self.job() else { return };
        job.cancel();
        for _ in 0..300 {
            if !job.running() {
                return;
            }
            glib::timeout_future(Duration::from_millis(100)).await;
        }
        tracing::warn!("{} did not end within 30 s", job.label());
    }

    pub fn updates(&self) -> Vec<serde_json::Value> {
        self.imp().updates.borrow().clone()
    }

    /// Asks the signed-in stores what they have newer builds of, unless they were asked within `max_age`; never in a
    /// scripted run, which stays off the network.
    pub fn check_updates(&self, max_age: Option<Duration>) {
        let imp = self.imp();
        if self.scripted() || imp.checking.get() || imp.updates_at.get().zip(max_age).is_some_and(|(at, age)| at.elapsed() < age) {
            return;
        }
        imp.checking.set(true);
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::pinned(|core| async move {
                let signed_in = core.sources().await.iter().any(|s| s["enabled"].as_bool() == Some(true) && s["logged_in"].as_bool() == Some(true));
                if signed_in {
                    core.source_updates().await
                } else {
                    Ok(Vec::new())
                }
            })
            .await;
            let Some(app) = app.upgrade() else { return };
            let imp = app.imp();
            imp.checking.set(false);
            match result {
                Ok(updates) => {
                    imp.updates_at.set(Some(Instant::now()));
                    imp.updates.replace(updates);
                    app.mark_updatable();
                    app.emit_by_name::<()>("updates-changed", &[]);
                }
                Err(e) => tracing::warn!("updates: {e}"),
            }
        });
    }

    fn mark_updatable(&self) {
        let updates = self.imp().updates.borrow();
        for game in self.library().games() {
            let wanted = {
                let row = game.row();
                !row.store_id.is_empty() && updates.iter().any(|u| u["source"] == row.source.as_str() && u["id"] == row.store_id.as_str())
            };
            if game.updatable() != wanted {
                game.set_updatable(wanted);
            }
        }
    }

    /// The recording's frame `index` when it is cached; asks for the thumbnail frame otherwise, which `frame-landed` announces.
    pub fn frame(&self, recording: &str, index: usize, duration_s: u64) -> Option<String> {
        let file = universe::frames::file(Path::new(recording), index);
        if file.is_file() {
            return Some(file.to_string_lossy().into_owned());
        }
        if let Some(frames) = self.imp().frames.borrow().as_ref() {
            if index == universe::frames::THUMB {
                frames.thumbnail(Path::new(recording), duration_s as f64);
            } else {
                frames.select(Path::new(recording), duration_s as f64);
            }
        }
        None
    }

    /// Every frame of the recording ahead of the rest.
    pub fn select_frames(&self, recording: &str, duration_s: u64) {
        if let Some(frames) = self.imp().frames.borrow().as_ref() {
            frames.select(Path::new(recording), duration_s as f64);
        }
    }

    pub fn forget_frames(&self, recording: &str) {
        if let Some(frames) = self.imp().frames.borrow().as_ref() {
            frames.forget(Path::new(recording));
        }
    }

    pub fn connect_frame_landed<F: Fn(&str, usize) + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("frame-landed", false, move |args| {
            let recording: String = args[1].get().unwrap_or_default();
            let index: u32 = args[2].get().unwrap_or_default();
            f(&recording, index as usize);
            None
        })
    }

    /// Takes `key` out of what the pages show and commits the delete once the toast goes, unless Undo brings it back.
    pub fn defer<F: Future<Output = ()> + 'static>(&self, key: &str, text: &str, commit: impl FnOnce() -> F + 'static) {
        self.imp().deferred.borrow_mut().push((key.to_string(), Box::new(move || Box::pin(commit()))));
        self.emit_by_name::<()>("deferred-changed", &[]);
        let toast = adw::Toast::builder().title(text).button_label(gettext("_Undo")).priority(adw::ToastPriority::High).build();
        let (app, undone) = (self.downgrade(), key.to_string());
        toast.connect_button_clicked(move |_| {
            let Some(app) = app.upgrade() else { return };
            app.imp().deferred.borrow_mut().retain(|(k, _)| *k != undone);
            app.emit_by_name::<()>("deferred-changed", &[]);
        });
        let (app, due) = (self.downgrade(), key.to_string());
        toast.connect_dismissed(move |_| {
            let Some(app) = app.upgrade() else { return };
            let commit = app.take_deferred(&due);
            if let Some(commit) = commit {
                let app = app.clone();
                glib::spawn_future_local(async move {
                    commit().await;
                    app.emit_by_name::<()>("deferred-changed", &[]);
                });
            }
        });
        match self.active_window().and_downcast::<Window>() {
            Some(win) => win.toast_undoable(toast),
            None => drop(glib::spawn_future_local(self.clone().flush_deferred_owned())),
        }
    }

    fn take_deferred(&self, key: &str) -> Option<Commit> {
        let mut deferred = self.imp().deferred.borrow_mut();
        let at = deferred.iter().position(|(k, _)| k == key)?;
        Some(deferred.remove(at).1)
    }

    pub fn has_deferred(&self) -> bool {
        !self.imp().deferred.borrow().is_empty()
    }

    pub fn is_deferred(&self, key: &str) -> bool {
        self.imp().deferred.borrow().iter().any(|(k, _)| k == key)
    }

    pub fn connect_deferred_changed<F: Fn() + 'static>(&self, f: F) -> glib::SignalHandlerId {
        self.connect_local("deferred-changed", false, move |_| {
            f();
            None
        })
    }

    /// Commits every delete still offering Undo: the app is going.
    pub async fn flush_deferred(&self) {
        let deferred = self.imp().deferred.take();
        for (_, commit) in deferred {
            commit().await;
        }
    }

    async fn flush_deferred_owned(self) {
        self.flush_deferred().await;
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
                glib::spawn_future_local(backend::pinned(|core| async move { core.apply_system().await }));
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
            let weak = app.downgrade();
            app.library().connect_updated(move || {
                weak.upgrade().inspect(|app| app.mark_updatable());
            });
            app.library().refresh(&[]).await;
            app.start_frames().await;
            app.imp().ready.set(true);
            app.emit_by_name::<()>("core-ready", &[]);
            if let Some(events) = events {
                app.follow(events);
            }
            glib::timeout_future_seconds(5).await;
            if !app.windows().is_empty() {
                app.check_updates(None);
            }
        });
    }

    /// Once the core has opened, or failed to.
    pub async fn opened(&self) {
        if self.is_ready() || self.failure().is_some() {
            return;
        }
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = std::rc::Rc::new(RefCell::new(Some(tx)));
        let handlers: Vec<glib::SignalHandlerId> = ["core-ready", "core-failed"]
            .into_iter()
            .map(|signal| {
                let tx = tx.clone();
                self.connect_local(signal, false, move |_| {
                    if let Some(tx) = tx.borrow_mut().take() {
                        let _ = tx.send(());
                    }
                    None
                })
            })
            .collect();
        let _ = rx.await;
        for handler in handlers {
            self.disconnect(handler);
        }
    }

    /// A game picked in the shell's search: its page in the window, and the game started when it can be.
    pub fn play_from_search(&self, id: &str) {
        let win = self.window();
        win.present();
        if let Some(game) = self.library().get(id) {
            win.open_game(&game);
            if game.installed() {
                win.play(&game);
            }
        }
    }

    /// The shell's search carried over to the library's.
    pub fn search_from_shell(&self, text: &str) {
        let win = self.window();
        win.present();
        win.search_for(text);
    }

    /// The frame sampler runs on the core's runtime; what it lands is announced on the main loop.
    async fn start_frames(&self) {
        let (frames, mut landed) = backend::run(async { Frames::start() }).await;
        self.imp().frames.replace(Some(frames));
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            while let Some(frame) = landed.recv().await {
                let Some(app) = app.upgrade() else { return };
                let recording = frame.recording.to_string_lossy().into_owned();
                app.emit_by_name::<()>("frame-landed", &[&recording, &(frame.index as u32)]);
            }
        });
    }

    fn follow(&self, mut events: tokio::sync::mpsc::UnboundedReceiver<Event>) {
        let app = self.downgrade();
        glib::spawn_future_local(async move {
            while let Some(event) = events.recv().await {
                let Some(app) = app.upgrade() else { return };
                match &event {
                    Event::Library(ids) => {
                        app.library().refresh(ids).await;
                        let watched = app.imp().unlocks.borrow().game.clone();
                        if !watched.is_empty() && (ids.is_empty() || ids.contains(&watched)) {
                            app.look_for_unlocks();
                        }
                    }
                    Event::SessionStarted(current) => app.session_started(current),
                    Event::SessionEnded(ended) => app.session_ended(ended),
                    Event::Media(id) => {
                        if let Some(game) = app.library().get(id) {
                            game.art_changed();
                        }
                    }
                    Event::JournalWriting { title, .. } => app.say(&gettext("Writing the journal entry for {}…").replace("{}", title)),
                    Event::JournalDone { id, session, state, text } => app.journal_done(id, session, state, text),
                    _ => {}
                }
                app.emit_by_name::<()>("changed", &[&glib::BoxedAnyObject::new(event)]);
            }
        });
    }

    pub fn say(&self, text: &str) {
        if let Some(win) = self.active_window().and_downcast::<Window>() {
            win.toast(adw::Toast::new(text));
        }
    }

    /// An entry left `pending`: read it, or why it was put off or failed.
    fn journal_done(&self, id: &str, session: &str, state: &str, text: &str) {
        let title = self.library().get(id).map(|g| g.title()).unwrap_or_else(|| id.to_string());
        let (line, read) = match state {
            "written" => (gettext("Journal entry written: “{}”").replace("{}", text), true),
            "deferred" => (gettext("The journal entry for {} is put off: {}").replacen("{}", &title, 1).replacen("{}", text, 1), false),
            _ => (gettext("The journal entry for {} could not be written: {}").replacen("{}", &title, 1).replacen("{}", text, 1), false),
        };
        let window = self.active_window().and_downcast::<Window>();
        let toast = adw::Toast::new(&line);
        if read {
            toast.set_button_label(Some(&gettext("_Read")));
            toast.set_action_name(Some("win.open-journal-entry"));
            toast.set_action_target_value(Some(&format!("{id}/{session}").to_variant()));
        }
        if read && window.as_ref().is_none_or(|win| !win.is_active()) {
            let notification = gio::Notification::new(&gettext("Journal entry written"));
            notification.set_body(Some(&format!("{title}: {text}")));
            notification.set_icon(&gio::ThemedIcon::new(config::APP_ID));
            notification.set_default_action_and_target_value("app.open-journal-entry", Some(&format!("{id}/{session}").to_variant()));
            self.send_notification(Some("journal"), &notification);
        }
        if let Some(win) = window {
            win.toast(toast);
        }
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
        let entry = gio::ActionEntry::builder("open-journal-entry")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|app: &Self, _, param| {
                let win = app.window();
                win.present();
                let _ = WidgetExt::activate_action(&win, "win.open-journal-entry", param);
            })
            .build();
        let dialogs = [("system-check", "doctor"), ("storage", "storage"), ("artwork", "artwork")].map(|(name, page)| {
            gio::ActionEntry::builder(name)
                .activate(move |app: &Self, _, _| {
                    if let Some(win) = app.active_window().and_downcast::<Window>() {
                        crate::dialogs::preferences::present(&win, page);
                    }
                })
                .build()
        });
        self.add_action_entries([quit, about, preferences, preferences_page, entry]);
        self.add_action_entries(dialogs);
        if glib::find_program_in_path(BIG_SCREEN).is_some() {
            self.add_action_entries([gio::ActionEntry::builder("big-screen").activate(|app: &Self, _, _| app.big_screen()).build()]);
        }
    }

    /// Universe Big Screen, the couch launcher, over the desktop; the window stays open under it.
    fn big_screen(&self) {
        let Some(program) = glib::find_program_in_path(BIG_SCREEN) else { return };
        match gio::Subprocess::newv(&[program.as_os_str()], gio::SubprocessFlags::NONE) {
            Ok(child) => {
                self.imp().big_screen.replace(Some(child.clone()));
                self.sync_big_screen();
                let app = self.downgrade();
                child.wait_async(gio::Cancellable::NONE, move |_| {
                    let Some(app) = app.upgrade() else { return };
                    app.imp().big_screen.replace(None);
                    app.sync_big_screen();
                });
            }
            Err(e) => self.say(&e.to_string()),
        }
    }

    /// One Big Screen at a time, and none over a game.
    fn sync_big_screen(&self) {
        if let Some(action) = self.lookup_action("big-screen").and_downcast::<gio::SimpleAction>() {
            action.set_enabled(self.current().is_none() && self.imp().big_screen.borrow().is_none());
        }
    }

    fn setup_accels(&self) {
        for (action, accels) in [
            ("app.quit", &["<Control>q"][..]),
            ("app.preferences", &["<Control>comma"]),
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
        let notes = whats_new::about_notes(&universe::changelog::releases(), universe::VERSION);
        if !notes.is_empty() {
            about.set_release_notes_version(universe::VERSION);
            about.set_release_notes(&notes);
        }
        about.add_link(&gettext("_System Check"), SYSTEM_CHECK_URI);
        about.connect_activate_link(|about, uri| {
            if uri != SYSTEM_CHECK_URI {
                return false;
            }
            about.close();
            gio::Application::default().inspect(|app| app.activate_action("system-check", None));
            true
        });
        let facts = debug_facts();
        about.set_debug_info(&system_check::debug_info(&facts, None));
        about.set_debug_info_filename("universe-desktop-debug.txt");
        let weak = about.downgrade();
        glib::spawn_future_local(async move {
            let checks = backend::pinned(|core| async move { core.doctor().await }).await;
            if let Some(about) = weak.upgrade() {
                about.set_debug_info(&system_check::debug_info(&facts, Some(&checks)));
            }
        });
        about.present(self.active_window().as_ref());
    }
}

fn debug_facts() -> Vec<(&'static str, String)> {
    let env = |var: &str| std::env::var(var).unwrap_or_default();
    let distro = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
        .unwrap_or_default();
    vec![
        ("Universe Desktop", universe::BUILD.to_string()),
        ("GTK", format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version())),
        ("libadwaita", format!("{}.{}.{}", adw::major_version(), adw::minor_version(), adw::micro_version())),
        ("Distribution", distro),
        ("Desktop", [env("XDG_CURRENT_DESKTOP"), env("XDG_SESSION_TYPE")].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" · ")),
        ("Steam Deck", universe::deck::model().map(|m| format!("{m:?}")).unwrap_or_default()),
        ("Config", universe::paths::config_file().display().to_string()),
        ("Data", universe::paths::data_home().display().to_string()),
    ]
}
