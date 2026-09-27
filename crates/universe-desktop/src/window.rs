use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::app::Application;
use crate::backend;
use crate::game::GameObject;
use crate::jobs::Kind;
use crate::library::{self, Sort, View};
use crate::pages::{GamePage, LibraryPage, MediaPage, StorePage};
use crate::script;
use crate::state::State;
use crate::widgets::NowPlaying;

/// What the sidebar lists besides the fixed items; rebuilt only when it changes, so the selection stays put.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Shape {
    favorites: bool,
    sources: BTreeSet<String>,
    platforms: BTreeSet<String>,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub toasts: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub navigation: TemplateChild<adw::NavigationView>,
        #[template_child]
        pub split_view: TemplateChild<adw::OverlaySplitView>,
        #[template_child]
        pub sidebar: TemplateChild<adw::Sidebar>,
        #[template_child]
        pub content_page: TemplateChild<adw::NavigationPage>,
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub failed_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub library_page: TemplateChild<LibraryPage>,
        #[template_child]
        pub media_page: TemplateChild<MediaPage>,
        #[template_child]
        pub store_page: TemplateChild<StorePage>,
        #[template_child]
        pub now_playing: TemplateChild<NowPlaying>,
        pub state: RefCell<State>,
        /// The player said to quit the running game: the next close goes through.
        pub closing: Cell<bool>,
        pub shape: RefCell<Option<Shape>>,
        /// The sidebar's items in index order: view key and title.
        pub keys: RefCell<Vec<(String, String)>>,
        pub rebuilding: Cell<bool>,
        /// The Store item's count of updates, in the sidebar built last.
        pub store_badge: RefCell<Option<gtk::Label>>,
        /// The toasts on screen whose Undo Ctrl+Z runs, the newest last.
        pub undoable: RefCell<Vec<glib::WeakRef<adw::Toast>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "UniverseWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            LibraryPage::ensure_type();
            MediaPage::ensure_type();
            StorePage::ensure_type();
            NowPlaying::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let state = State::load();
            obj.set_default_size(state.width, state.height);
            obj.set_maximized(state.maximized);
            self.library_page.set_sort(Sort::parse(&state.sort));
            self.state.replace(state);
            self.library_page.search_entry().set_key_capture_widget(Some(&*obj));
            for button in [self.library_page.sidebar_button(), self.media_page.sidebar_button(), self.store_page.sidebar_button()] {
                self.split_view.bind_property("show-sidebar", &button, "visible").invert_boolean().sync_create().build();
            }
            obj.setup_actions();
            obj.setup_sidebar();
        }
    }

    impl WidgetImpl for Window {}

    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();
            if !self.closing.get() {
                let app = obj.app();
                let job = app.job();
                if app.current().is_some() || job.as_ref().is_some_and(|j| j.cancellable()) {
                    obj.confirm_close();
                    return glib::Propagation::Stop;
                }
                if job.is_some() || app.has_deferred() {
                    obj.leave();
                    return glib::Propagation::Stop;
                }
                if !app.scripted() {
                    obj.save_state();
                }
            }
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends adw::ApplicationWindow, gtk::ApplicationWindow, gtk::Window, gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        let win: Self = glib::Object::builder().property("application", app).build();
        win.follow_core();
        win
    }

    pub fn app(&self) -> Application {
        self.application().and_downcast::<Application>().expect("a Universe window belongs to the Universe application")
    }

    fn setup_actions(&self) {
        let state = self.imp().state.borrow().clone();
        let show_sidebar = gio::ActionEntry::builder("show-sidebar")
            .activate(|win: &Self, _, _| {
                let split = &win.imp().split_view;
                split.set_show_sidebar(!split.shows_sidebar());
            })
            .build();
        let search = gio::ActionEntry::builder("search")
            .activate(|win: &Self, _, _| {
                if let Some(entry) = win.search_entry() {
                    entry.grab_focus();
                }
            })
            .build();
        let sort = gio::ActionEntry::builder("sort")
            .parameter_type(Some(glib::VariantTy::STRING))
            .state(state.sort.to_variant())
            .activate(|win: &Self, action, param| {
                let Some(key) = param.and_then(|p| p.get::<String>()) else { return };
                action.set_state(&key.to_variant());
                win.imp().library_page.set_sort(Sort::parse(&key));
                win.imp().state.borrow_mut().sort = key;
            })
            .build();
        let show_hidden = gio::ActionEntry::builder("show-hidden")
            .state(state.show_hidden.to_variant())
            .activate(|win: &Self, action, _| {
                let shown = !action.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
                action.set_state(&shown.to_variant());
                win.imp().library_page.set_show_hidden(shown);
                win.imp().state.borrow_mut().show_hidden = shown;
            })
            .build();
        let view = gio::ActionEntry::builder("view")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|win: &Self, _, param| {
                let Some(key) = param.and_then(|p| p.get::<String>()) else { return };
                let index = win.imp().keys.borrow().iter().position(|(k, _)| *k == key);
                if let Some(index) = index {
                    win.imp().sidebar.set_selected(index as u32);
                }
            })
            .build();
        let add_game = gio::ActionEntry::builder("add-game").activate(|win: &Self, _, _| crate::dialogs::add_game::present(win)).build();
        let onboarding = gio::ActionEntry::builder("onboarding").activate(|win: &Self, _, _| crate::dialogs::onboarding::present(win)).build();
        let rescan = gio::ActionEntry::builder("rescan").activate(|win: &Self, _, _| win.rescan()).build();
        let entry = gio::ActionEntry::builder("open-journal-entry")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|win: &Self, _, param| {
                let target = param.and_then(|p| p.get::<String>()).unwrap_or_default();
                if let Some((game, session)) = target.split_once('/') {
                    crate::pages::journal::open_entry(win, game, session);
                }
            })
            .build();
        let undo = gio::ActionEntry::builder("undo").activate(|win: &Self, _, _| win.undo()).build();
        self.add_action_entries([show_sidebar, search, sort, show_hidden, view, add_game, onboarding, rescan, entry, undo]);
        self.imp().library_page.set_show_hidden(state.show_hidden);
    }

    /// The first-run flow is behind the player: it opens again only from the empty library's button.
    pub fn set_onboarded(&self) {
        let mut state = self.imp().state.borrow_mut();
        if !state.onboarded {
            state.onboarded = true;
            if !self.app().scripted() {
                state.save();
            }
        }
    }

    /// The library read again and the emulators' folders searched for games new to it.
    fn rescan(&self) {
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::pinned(|core| async move { core.rescan().await }).await;
            let Some(win) = win.upgrade() else { return };
            win.app().library().refresh(&[]).await;
            let text = match result {
                Ok(report) if report.imported.is_empty() => gettext("The library is up to date"),
                Ok(report) => {
                    gettextrs::ngettext("{} game added from the emulator folders", "{} games added from the emulator folders", report.imported.len() as u32)
                        .replace("{}", &report.imported.len().to_string())
                }
                Err(e) => e.to_string(),
            };
            win.toast(adw::Toast::new(&text));
        });
    }

    fn save_state(&self) {
        let imp = self.imp();
        let mut state = imp.state.borrow_mut();
        state.maximized = self.is_maximized();
        if !state.maximized {
            let (width, height) = self.default_size();
            state.width = width;
            state.height = height;
        }
        state.save();
    }

    /// The search of the page in view; the media page has none.
    fn search_entry(&self) -> Option<gtk::SearchEntry> {
        let imp = self.imp();
        match imp.stack.visible_child_name().as_deref() {
            Some("store") => Some(imp.store_page.search_entry()),
            Some("library") => Some(imp.library_page.search_entry()),
            _ => None,
        }
    }

    /// Pushes a page over the rest; one already open under the same tag closes first, with what was over it.
    pub fn push_page(&self, page: &adw::NavigationPage) {
        let navigation = &self.imp().navigation;
        if let Some(open) = page.tag().and_then(|tag| navigation.find_page(&tag)) {
            if let Some(below) = navigation.previous_page(&open) {
                navigation.pop_to_page(&below);
            }
        }
        navigation.push(page);
    }

    pub fn toast(&self, toast: adw::Toast) {
        toast.set_use_markup(false);
        self.imp().toasts.add_toast(toast);
    }

    /// A toast whose button undoes what it tells, which Ctrl+Z presses too while it shows.
    pub fn toast_undoable(&self, toast: adw::Toast) {
        let win = self.downgrade();
        toast.connect_dismissed(move |toast| {
            if let Some(win) = win.upgrade() {
                win.imp().undoable.borrow_mut().retain(|t| t.upgrade().is_some_and(|t| t != *toast));
            }
        });
        self.imp().undoable.borrow_mut().push(toast.downgrade());
        self.toast(toast);
    }

    /// Presses the newest Undo on screen, as its button does: the undo runs before the toast goes.
    fn undo(&self) {
        let toast = std::iter::from_fn(|| self.imp().undoable.borrow_mut().pop()).find_map(|t| t.upgrade());
        if let Some(toast) = toast {
            toast.emit_by_name::<()>("button-clicked", &[]);
            toast.dismiss();
        }
    }

    fn follow_core(&self) {
        let app = self.app();
        if app.scripted() {
            self.set_maximized(false);
            self.set_default_size(1280, 800);
        }
        self.imp().library_page.set_library(&app);
        self.imp().store_page.follow(&app);
        self.imp().media_page.follow(&app);
        self.setup_play_actions();
        let win = self.downgrade();
        app.library().connect_updated(move || {
            win.upgrade().inspect(|win| win.rebuild_sidebar());
        });
        let win = self.downgrade();
        app.connect_local("updates-changed", false, move |_| {
            win.upgrade().inspect(|win| win.sync_badge());
            None
        });
        let win = self.downgrade();
        app.connect_local("session-changed", false, move |_| {
            win.upgrade().inspect(|win| win.session_changed());
            None
        });
        let win = self.downgrade();
        app.connect_changed(move |event| {
            if let (Some(win), universe::changes::Event::Library(ids)) = (win.upgrade(), event) {
                win.games_changed(ids);
            }
        });
        if app.is_ready() {
            self.core_ready();
        } else if let Some(failure) = app.failure() {
            self.core_failed(&failure);
        } else {
            let win = self.downgrade();
            app.connect_local("core-ready", false, move |_| {
                win.upgrade().inspect(|win| win.core_ready());
                None
            });
            let win = self.downgrade();
            app.connect_local("core-failed", false, move |args| {
                let failure: String = args[1].get().unwrap_or_default();
                win.upgrade().inspect(|win| win.core_failed(&failure));
                None
            });
        }
    }

    fn core_ready(&self) {
        self.rebuild_sidebar();
        let onboarded = self.imp().state.borrow().onboarded;
        if !onboarded && !self.app().scripted() {
            if self.app().library().is_empty() {
                crate::dialogs::onboarding::present(self);
            } else {
                self.set_onboarded();
            }
        }
        if let Some(steps) = self.app().take_script() {
            glib::spawn_future_local(script::run(self.clone().upcast(), steps));
        }
    }

    fn core_failed(&self, failure: &str) {
        self.imp().failed_page.set_description(Some(&glib::markup_escape_text(failure)));
        self.imp().stack.set_visible_child_name("failed");
        if let Some(steps) = self.app().take_script() {
            glib::spawn_future_local(script::run(self.clone().upcast(), steps));
        }
    }

    fn setup_sidebar(&self) {
        let imp = self.imp();
        let win = self.downgrade();
        imp.sidebar.connect_selected_notify(move |sidebar| {
            let Some(win) = win.upgrade() else { return };
            if !win.imp().rebuilding.get() {
                win.select(sidebar.selected());
            }
        });
        let win = self.downgrade();
        imp.sidebar.connect_activated(move |_, _| {
            let Some(win) = win.upgrade() else { return };
            let split = &win.imp().split_view;
            if split.is_collapsed() {
                split.set_show_sidebar(false);
            }
        });
    }

    fn rebuild_sidebar(&self) {
        let imp = self.imp();
        let games = self.app().library().games();
        let shape = Shape {
            favorites: games.iter().any(|g| g.favorite()),
            sources: games.iter().map(|g| g.source()).filter(|s| !s.is_empty()).collect(),
            platforms: games.iter().map(|g| g.platform()).filter(|p| !p.is_empty()).collect(),
        };
        if imp.shape.borrow().as_ref() == Some(&shape) {
            return;
        }
        let sidebar = &*imp.sidebar;
        let mut keys: Vec<(String, String)> = Vec::new();
        let item = |icon: &str, title: &str| adw::SidebarItem::builder().icon_name(icon).title(title).build();

        imp.rebuilding.set(true);
        sidebar.remove_all();
        let main = adw::SidebarSection::new();
        main.append(item("view-grid-symbolic", &gettext("All Games")));
        keys.push(("all".into(), gettext("All Games")));
        if shape.favorites {
            main.append(item("starred-symbolic", &gettext("Favourites")));
            keys.push(("favorites".into(), gettext("Favourites")));
        }
        main.append(item("camera-photo-symbolic", &gettext("Media")));
        keys.push(("media".into(), gettext("Media")));
        let store = item("system-software-install-symbolic", &gettext("Store"));
        let badge = gtk::Label::builder().valign(gtk::Align::Center).css_classes(["count-badge", "numeric"]).build();
        store.set_suffix(Some(&badge));
        imp.store_badge.replace(Some(badge));
        main.append(store);
        keys.push(("store".into(), gettext("Store")));
        sidebar.append(main);
        if !shape.sources.is_empty() {
            let section = adw::SidebarSection::new();
            section.set_title(Some(&gettext("Sources")));
            for kind in &shape.sources {
                let name = library::source_name(kind);
                section.append(item(library::source_icon(kind), &name));
                keys.push((View::Source(kind.clone()).key(), name));
            }
            sidebar.append(section);
        }
        if !shape.platforms.is_empty() {
            let section = adw::SidebarSection::new();
            section.set_title(Some(&gettext("Platforms")));
            let mut platforms: Vec<&String> = shape.platforms.iter().collect();
            platforms.sort_by_key(|p| library::fold(&library::platform_name(p)));
            for platform in platforms {
                let name = library::platform_name(platform);
                section.append(item(library::platform_icon(platform), &name));
                keys.push((View::Platform(platform.clone()).key(), name));
            }
            sidebar.append(section);
        }
        let wanted = imp.state.borrow().view.clone();
        let index = keys.iter().position(|(k, _)| *k == wanted).unwrap_or(0);
        imp.keys.replace(keys);
        imp.shape.replace(Some(shape));
        sidebar.set_selected(index as u32);
        imp.rebuilding.set(false);
        self.sync_badge();
        self.select(index as u32);
    }

    fn sync_badge(&self) {
        let count = self.app().updates().len();
        if let Some(badge) = self.imp().store_badge.borrow().as_ref() {
            badge.set_label(&count.to_string());
            badge.set_visible(count > 0);
            badge.set_tooltip_text(Some(&gettextrs::ngettext("{} update", "{} updates", count as u32).replace("{}", &count.to_string())));
        }
    }

    fn select(&self, index: u32) {
        let imp = self.imp();
        let Some((key, title)) = imp.keys.borrow().get(index as usize).cloned() else { return };
        imp.content_page.set_title(&title);
        let page = match key.as_str() {
            "store" | "media" => key.as_str(),
            _ => {
                imp.library_page.set_view(View::parse(&key));
                "library"
            }
        };
        for (name, entry) in [("library", imp.library_page.search_entry()), ("store", imp.store_page.search_entry())] {
            entry.set_key_capture_widget(if name == page { Some(self.upcast_ref::<gtk::Widget>()) } else { None });
        }
        if imp.stack.visible_child_name().as_deref() != Some(page) {
            imp.stack.set_visible_child_name(page);
        }
        if page == "store" {
            imp.store_page.load(false);
        }
        imp.state.borrow_mut().view = key;
    }

    fn session_changed(&self) {
        let current = self.app().current();
        let cover = current.as_ref().and_then(|c| self.app().library().get(&c.id)).map(|g| g.cover()).unwrap_or_default();
        self.imp().now_playing.set_session(current.as_ref(), &cover);
        self.sync_play_actions();
    }

    /// A page showing one of these games reads it again.
    fn games_changed(&self, ids: &[String]) {
        let navigation = &self.imp().navigation;
        for page in navigation.navigation_stack().iter::<glib::Object>().flatten() {
            if let Some(page) = page.downcast_ref::<GamePage>() {
                if ids.is_empty() || page.game().is_some_and(|g| ids.contains(&g.id())) {
                    page.reload();
                }
            }
        }
    }

    /// Asks before a close ends the game started here or stops the running job.
    fn confirm_close(&self) {
        let (current, job) = (self.app().current(), self.app().job().filter(|j| j.cancellable()));
        let mut body = Vec::new();
        if current.is_some() {
            body.push(gettext("Games started here close with Universe Desktop."));
        }
        if let Some(job) = &job {
            body.push(match job.kind() {
                Kind::Install | Kind::Update => gettext("{} pauses, and resumes from the Store.").replace("{}", &job.title()),
                _ => gettext("Fetching artwork stops; the art fetched so far is kept."),
            });
        }
        let dialog = match &current {
            Some(current) => {
                let dialog = adw::AlertDialog::new(Some(&gettext("Quit {}?").replace("{}", &current.title)), Some(&body.join(" ")));
                dialog.add_responses(&[("cancel", &gettext("_Keep Playing")), ("quit", &gettext("_Quit Game"))]);
                dialog
            }
            None => {
                let dialog = adw::AlertDialog::new(Some(&gettext("Quit Universe Desktop?")), Some(&body.join(" ")));
                dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("quit", &gettext("_Quit"))]);
                dialog
            }
        };
        dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let win = self.downgrade();
        dialog.connect_response(Some("quit"), move |_, _| {
            win.upgrade().inspect(|win| win.leave());
        });
        dialog.present(Some(self));
    }

    /// Out of sight at once; the game ends and the job stops before the window goes.
    fn leave(&self) {
        self.imp().closing.set(true);
        if !self.app().scripted() {
            self.save_state();
        }
        self.set_visible(false);
        let win = self.clone();
        glib::spawn_future_local(async move {
            win.app().flush_deferred().await;
            win.app().end_session().await;
            win.app().end_job().await;
            win.close();
        });
    }

    /// A `game.*` action from a card, a page or a menu.
    pub fn game_action(&self, name: &str, game: &GameObject) {
        match name {
            "play" => self.play(game),
            "details" => self.open_game(game),
            "open-folder" => self.open_folder(game),
            "settings" => crate::dialogs::game_settings::present(self, game),
            "artwork" => crate::dialogs::artwork::present(self, game),
            "hide" => self.set_flag(game, "hidden", true),
            "unhide" => self.set_flag(game, "hidden", false),
            "favorite" => self.set_flag(game, "favorite", true),
            "unfavorite" => self.set_flag(game, "favorite", false),
            "update" => self.update(game),
            "uninstall" => self.confirm_uninstall(game),
            "remove" => self.remove(game),
            "remove-purge" => self.confirm_purge(game),
            _ => tracing::warn!("game.{name}: no such action"),
        }
    }

    fn update(&self, game: &GameObject) {
        let row = game.row().clone();
        if self.app().start_job(Kind::Update, &row.source, vec![(row.store_id, row.title.clone())], false) {
            let toast = adw::Toast::builder().title(gettext("Updating {}").replace("{}", &row.title)).button_label(gettext("_Show")).build();
            toast.set_action_name(Some("win.view"));
            toast.set_action_target_value(Some(&"store".to_variant()));
            self.toast(toast);
        }
    }

    /// Out of sight at once; the core removes it once the toast goes, unless Undo brings it back.
    fn remove(&self, game: &GameObject) {
        let (id, title) = (game.id(), game.title());
        self.close_game_pages(&id);
        let app = self.app();
        let weak = app.downgrade();
        app.defer(&crate::game::removal_key(&id), &gettext("“{}” removed from the library").replace("{}", &title), move || async move {
            let target = id.clone();
            let result = backend::call(move |core| async move { core.remove(&target, false).await }).await;
            let Some(app) = weak.upgrade() else { return };
            if let Err(e) = result {
                app.say(&e.to_string());
            }
            app.library().refresh(&[id]).await;
        });
    }

    fn confirm_purge(&self, game: &GameObject) {
        let (id, title) = (game.id(), game.title());
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Remove {} and Its Wine Prefix?").replace("{}", &title)),
            Some(&gettext("The prefix goes to the trash with the saves and settings the game keeps there. Its hours, journal and recordings are kept.")),
        );
        dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("remove", &gettext("_Remove"))]);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let win = self.downgrade();
        dialog.connect_response(Some("remove"), move |_, _| {
            let Some(win) = win.upgrade() else { return };
            win.close_game_pages(&id);
            let (id, title, win) = (id.clone(), title.clone(), win.downgrade());
            glib::spawn_future_local(async move {
                let target = id.clone();
                let result = backend::call(move |core| async move { core.remove(&target, true).await }).await;
                let Some(win) = win.upgrade() else { return };
                win.app().library().refresh(std::slice::from_ref(&id)).await;
                win.toast(adw::Toast::new(&match result {
                    Ok(()) => gettext("“{}” removed with its Wine prefix").replace("{}", &title),
                    Err(e) => e.to_string(),
                }));
            });
        });
        dialog.present(Some(self));
    }

    fn confirm_uninstall(&self, game: &GameObject) {
        let (id, title) = (game.id(), game.title());
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Uninstall {}?").replace("{}", &title)),
            Some(&gettext("Its install folder goes to the trash. The game stays in the library with its hours, journal and recordings.")),
        );
        dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("uninstall", &gettext("_Uninstall"))]);
        dialog.set_response_appearance("uninstall", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let win = self.downgrade();
        dialog.connect_response(Some("uninstall"), move |_, _| {
            let (id, title, win) = (id.clone(), title.clone(), win.clone());
            glib::spawn_future_local(async move {
                let target = id.clone();
                let result = backend::call(move |core| async move { core.uninstall(&target).await }).await;
                let Some(win) = win.upgrade() else { return };
                win.app().library().refresh(std::slice::from_ref(&id)).await;
                win.toast(adw::Toast::new(&match result {
                    Ok(()) => gettext("{} is uninstalled").replace("{}", &title),
                    Err(e) => e.to_string(),
                }));
            });
        });
        dialog.present(Some(self));
    }

    /// The first page over the game closes, with every page above it.
    fn close_game_pages(&self, id: &str) {
        let navigation = &self.imp().navigation;
        let stack = navigation.navigation_stack();
        let first =
            stack.iter::<glib::Object>().flatten().position(|page| page.downcast_ref::<GamePage>().and_then(|p| p.game()).is_some_and(|g| g.id() == id));
        if let Some(below) = first.and_then(|at| at.checked_sub(1)).and_then(|at| stack.item(at as u32)).and_downcast::<adw::NavigationPage>() {
            navigation.pop_to_page(&below);
        }
    }

    fn set_flag(&self, game: &GameObject, key: &'static str, on: bool) {
        let (id, title) = (game.id(), game.title());
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let Some(result) = win.upgrade().map(|w| w.write_flag(&id, key, on)) else { return };
            let ok = result.await;
            let Some(win) = win.upgrade() else { return };
            if !ok {
                return;
            }
            let text = match (key, on) {
                ("hidden", true) => gettext("“{}” hidden"),
                ("hidden", false) => gettext("“{}” unhidden"),
                ("favorite", true) => gettext("“{}” added to your favourites"),
                _ => gettext("“{}” removed from your favourites"),
            };
            let toast = adw::Toast::builder().title(text.replace("{}", &title)).button_label(gettext("_Undo")).priority(adw::ToastPriority::High).build();
            let back = win.downgrade();
            toast.connect_button_clicked(move |_| {
                if let Some(win) = back.upgrade() {
                    glib::spawn_future_local(win.write_flag(&id, key, !on));
                }
            });
            win.toast_undoable(toast);
        });
    }

    /// Writes a `game.toml` flag and reads the game back at once, rather than on the watch's next pass.
    fn write_flag(&self, id: &str, key: &'static str, on: bool) -> impl std::future::Future<Output = bool> + 'static {
        let (id, win) = (id.to_string(), self.downgrade());
        async move {
            let value = if on { "true" } else { "false" };
            let target = id.clone();
            let result = backend::call(move |core| async move { core.set(&target, key, value).await }).await;
            let Some(win) = win.upgrade() else { return false };
            match result {
                Ok(()) => {
                    win.app().library().refresh(std::slice::from_ref(&id)).await;
                    true
                }
                Err(e) => {
                    win.toast(adw::Toast::new(&e.to_string()));
                    false
                }
            }
        }
    }
}
