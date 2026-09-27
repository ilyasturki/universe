use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::app::Application;
use crate::backend;
use crate::game::GameObject;
use crate::library::{self, Sort, View};
use crate::pages::{GamePage, LibraryPage};
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
        pub now_playing: TemplateChild<NowPlaying>,
        pub state: RefCell<State>,
        /// The player said to quit the running game: the next close goes through.
        pub closing: Cell<bool>,
        pub shape: RefCell<Option<Shape>>,
        /// The sidebar's items in index order: view key and title.
        pub keys: RefCell<Vec<(String, String)>>,
        pub rebuilding: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "UniverseWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            LibraryPage::ensure_type();
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
            self.split_view.bind_property("show-sidebar", &self.library_page.sidebar_button(), "visible").invert_boolean().sync_create().build();
            obj.setup_actions();
            obj.setup_sidebar();
        }
    }

    impl WidgetImpl for Window {}

    impl WindowImpl for Window {
        fn close_request(&self) -> glib::Propagation {
            let obj = self.obj();
            if obj.app().current().is_some() && !self.closing.get() {
                obj.confirm_close();
                return glib::Propagation::Stop;
            }
            if !obj.app().scripted() {
                obj.save_state();
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
                win.imp().library_page.search_entry().grab_focus();
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
        self.add_action_entries([show_sidebar, search, sort, show_hidden, view, add_game, onboarding, rescan]);
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

    pub fn toast(&self, toast: adw::Toast) {
        self.imp().toasts.add_toast(toast);
    }

    fn follow_core(&self) {
        let app = self.app();
        if app.scripted() {
            self.set_maximized(false);
            self.set_default_size(1280, 800);
        }
        self.imp().library_page.set_library(app.library());
        self.setup_play_actions();
        let win = self.downgrade();
        app.library().connect_updated(move || {
            win.upgrade().inspect(|win| win.rebuild_sidebar());
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
        self.select(index as u32);
    }

    fn select(&self, index: u32) {
        let imp = self.imp();
        let Some((key, title)) = imp.keys.borrow().get(index as usize).cloned() else { return };
        imp.content_page.set_title(&title);
        imp.library_page.set_view(View::parse(&key));
        if imp.stack.visible_child_name().as_deref() != Some("library") {
            imp.stack.set_visible_child_name("library");
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

    fn confirm_close(&self) {
        let Some(current) = self.app().current() else { return };
        let dialog =
            adw::AlertDialog::new(Some(&gettext("Quit {}?").replace("{}", &current.title)), Some(&gettext("Games started here close with Universe Desktop.")));
        dialog.add_responses(&[("cancel", &gettext("_Keep Playing")), ("quit", &gettext("_Quit Game"))]);
        dialog.set_response_appearance("quit", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        let win = self.downgrade();
        dialog.connect_response(None, move |_, response| {
            let Some(win) = win.upgrade().filter(|_| response == "quit") else { return };
            win.imp().closing.set(true);
            glib::spawn_future_local(async move {
                win.app().end_session().await;
                win.close();
            });
        });
        dialog.present(Some(self));
    }

    /// A `game.*` action from a card, a page or a menu.
    pub fn game_action(&self, name: &str, game: &GameObject) {
        match name {
            "play" => self.play(game),
            "details" => self.open_game(game),
            "open-folder" => self.open_folder(game),
            "settings" => crate::dialogs::game_settings::present(self, game),
            "hide" => self.set_flag(game, "hidden", true),
            "unhide" => self.set_flag(game, "hidden", false),
            "favorite" => self.set_flag(game, "favorite", true),
            "unfavorite" => self.set_flag(game, "favorite", false),
            _ => tracing::debug!("game.{name}: not wired yet"),
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
            let toast = adw::Toast::builder().title(text.replace("{}", &title)).button_label(gettext("Undo")).priority(adw::ToastPriority::High).build();
            let back = win.downgrade();
            toast.connect_button_clicked(move |_| {
                if let Some(win) = back.upgrade() {
                    glib::spawn_future_local(win.write_flag(&id, key, !on));
                }
            });
            win.toast(toast);
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
