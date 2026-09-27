use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use serde_json::Value;

use crate::app::Application;
use crate::backend;
use crate::format;
use crate::jobs::{Job, Kind};
use crate::library;
use crate::widgets::Cover;
use crate::window::Window;

// The cached listing and the disk are read again once this old; the store itself only on Refresh.
const STALE: Duration = Duration::from_secs(15 * 60);

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn bytes(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or(0)
}

fn size(n: u64) -> String {
    glib::format_size(n).to_string()
}

/// A folder with the home as `~`.
fn tilde(path: &str) -> String {
    match std::path::Path::new(path).strip_prefix(universe::paths::home()) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.into(),
    }
}

fn signed_in(source: &Value) -> bool {
    source["enabled"].as_bool() == Some(true) && source["available"].as_bool() != Some(false) && source["logged_in"].as_bool() == Some(true)
}

/// The space left where a store installs, through the folder's nearest existing parent.
async fn free_space(dir: &str) -> u64 {
    if dir.is_empty() {
        return 0;
    }
    let path = universe::paths::expand(dir);
    let Some(existing) = path.ancestors().find(|p| p.exists()) else { return 0 };
    gio::File::for_path(existing)
        .query_filesystem_info_future("filesystem::free", glib::Priority::DEFAULT)
        .await
        .map(|info| info.attribute_uint64("filesystem::free"))
        .unwrap_or(0)
}

fn button(label: &str, action: &str, id: &str) -> gtk::Button {
    gtk::Button::builder().label(label).use_underline(true).valign(gtk::Align::Center).action_name(action).action_target(&id.to_variant()).build()
}

/// What a store lists of one game, as the page shows it.
#[derive(Debug, Clone, Default)]
struct Entry {
    id: String,
    title: String,
    /// The library's id for it, once it is there.
    game_id: String,
    image: String,
    owned: Option<bool>,
    installed: bool,
    /// A stopped download waits on the disk.
    partial: bool,
    download: u64,
    disk: u64,
    kept: u64,
}

impl Entry {
    fn of(v: &Value) -> Entry {
        let installed = v["installed"].as_bool() == Some(true);
        Entry {
            id: text(v, "id"),
            title: text(v, "title"),
            game_id: text(v, "game_id"),
            image: text(v, "image"),
            owned: v["owned"].as_bool(),
            installed,
            partial: !installed && !text(v, "partial_dir").is_empty(),
            download: bytes(v, "download_size"),
            disk: bytes(v, "disk_size"),
            kept: bytes(v, "partial_bytes"),
        }
    }

    fn matches(&self, words: &[String]) -> bool {
        let hay = library::fold(&self.title);
        words.iter().all(|w| hay.contains(w.as_str()))
    }

    /// What installing it takes, as far as the store said.
    fn cost(&self) -> String {
        match (self.download, self.disk) {
            (0, 0) => String::new(),
            (0, disk) => gettext("{} on disk").replace("{}", &size(disk)),
            (download, _) => gettext("{} to download").replace("{}", &size(download)),
        }
    }

    fn paused(&self) -> String {
        match (self.kept, self.disk) {
            (0, _) => gettext("Paused"),
            (kept, 0) => gettext("Paused · {} kept").replace("{}", &size(kept)),
            (kept, disk) => gettext("Paused · {} of {} kept").replacen("{}", &size(kept), 1).replacen("{}", &size(disk), 1),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/store-page.ui")]
    pub struct StorePage {
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub search: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub refresh_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub refresh: TemplateChild<gtk::Button>,
        #[template_child]
        pub refresh_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub sources: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub failed_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub job_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub job_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub job_art: TemplateChild<Cover>,
        #[template_child]
        pub job_progress: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub job_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub job_cancel: TemplateChild<gtk::Button>,
        #[template_child]
        pub updates_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub installed_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub owned_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub results_group: TemplateChild<adw::PreferencesGroup>,
        pub app: glib::WeakRef<Application>,
        pub actions: OnceCell<gio::SimpleActionGroup>,
        /// The signed-in stores, in the picker's order.
        pub stores: RefCell<Vec<Value>>,
        pub source: RefCell<String>,
        pub games: RefCell<Vec<Value>>,
        pub error: RefCell<String>,
        pub free: Cell<u64>,
        pub loaded: Cell<bool>,
        pub loaded_at: Cell<Option<Instant>>,
        pub busy: Cell<bool>,
        pub query: RefCell<String>,
        pub words: RefCell<Vec<String>>,
        /// The catalogue's answer to `query`, once it came.
        pub results: RefCell<Option<Vec<Value>>>,
        pub peeked: RefCell<HashSet<String>>,
        pub peeking: Cell<bool>,
        pub rows: RefCell<Vec<(adw::PreferencesGroup, adw::ActionRow)>>,
        /// The rows of games whose size is not known yet, by store id.
        pub sizeless: RefCell<HashMap<String, adw::ActionRow>>,
        pub job: RefCell<Option<(Job, Vec<glib::SignalHandlerId>)>>,
        /// The picker is being filled: its selection is not the player's.
        pub syncing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StorePage {
        const NAME: &'static str = "UniverseStorePage";
        type Type = super::StorePage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Cover::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for StorePage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_actions();
            let page = obj.downgrade();
            self.search.connect_search_changed(move |entry| {
                page.upgrade().inspect(|page| page.searched(&entry.text()));
            });
            self.search.connect_stop_search(|entry| entry.set_text(""));
            let page = obj.downgrade();
            self.sources.connect_selected_notify(move |picker| {
                let Some(page) = page.upgrade().filter(|p| !p.imp().syncing.get()) else { return };
                let id = page.imp().stores.borrow().get(picker.selected() as usize).map(|s| text(s, "id"));
                if let Some(id) = id {
                    page.set_source(&id);
                }
            });
        }
    }

    impl WidgetImpl for StorePage {}
    impl BinImpl for StorePage {}
}

glib::wrapper! {
    /// What the signed-in stores hold for the player: installs running and paused, updates, the games installed and not.
    pub struct StorePage(ObjectSubclass<imp::StorePage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl StorePage {
    pub fn search_entry(&self) -> gtk::SearchEntry {
        self.imp().search.get()
    }

    pub fn sidebar_button(&self) -> gtk::Button {
        self.imp().sidebar_button.get()
    }

    fn app(&self) -> Option<Application> {
        self.imp().app.upgrade()
    }

    fn say(&self, text: &str) {
        if let Some(win) = self.root().and_downcast::<Window>() {
            win.toast(adw::Toast::new(text));
        }
    }

    pub fn follow(&self, app: &Application) {
        self.imp().app.set(Some(app));
        let page = self.downgrade();
        app.connect_local("job-changed", false, move |_| {
            page.upgrade().inspect(|page| page.job_changed());
            None
        });
        let page = self.downgrade();
        app.connect_local("updates-changed", false, move |_| {
            page.upgrade().inspect(|page| page.rebuild());
            None
        });
        let page = self.downgrade();
        app.connect_changed(move |event| {
            if let (Some(page), universe::changes::Event::Library(_)) = (page.upgrade(), event) {
                page.imp().loaded_at.set(None);
                if page.is_mapped() {
                    page.load(false);
                }
            }
        });
        self.job_changed();
    }

    fn setup_actions(&self) {
        let group = gio::SimpleActionGroup::new();
        let add = |name: &str, targeted: bool, act: fn(&StorePage, &str)| {
            let action = gio::SimpleAction::new(name, targeted.then_some(glib::VariantTy::STRING));
            let page = self.downgrade();
            action.connect_activate(move |_, param| {
                let arg = param.and_then(|p| p.get::<String>()).unwrap_or_default();
                if let Some(page) = page.upgrade() {
                    act(&page, &arg);
                }
            });
            group.add_action(&action);
        };
        add("refresh", false, |page, _| page.load(true));
        add("scan", false, |page, _| page.start(Kind::Scan, Vec::new()));
        add("update-all", false, |page, _| page.update_all());
        add("install", true, |page, id| page.confirm_install(id));
        add("resume", true, |page, id| page.start_on(Kind::Install, id));
        add("update", true, |page, id| page.start_on(Kind::Update, id));
        add("uninstall", true, |page, id| page.confirm_uninstall(id));
        add("cancel", false, |page, _| {
            if let Some(job) = page.app().and_then(|app| app.job()) {
                job.cancel();
            }
        });
        self.insert_action_group("store", Some(&group));
        let _ = self.imp().actions.set(group);
    }

    fn sync_actions(&self) {
        let Some(group) = self.imp().actions.get() else { return };
        let job = self.app().and_then(|app| app.job());
        let (idle, stores) = (job.is_none(), !self.imp().stores.borrow().is_empty());
        for (name, on) in [
            ("refresh", !self.imp().busy.get()),
            ("scan", idle && stores),
            ("update-all", idle),
            ("install", idle),
            ("resume", idle),
            ("update", idle),
            ("cancel", job.is_some_and(|j| j.cancellable() && !j.cancelled())),
        ] {
            if let Some(action) = group.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                action.set_enabled(on);
            }
        }
    }

    /// The listing on showing the page: the cached one and the disk, read again once `STALE`; `refresh` asks the store.
    pub fn load(&self, refresh: bool) {
        let imp = self.imp();
        if imp.busy.get() || (!refresh && imp.loaded_at.get().is_some_and(|at| at.elapsed() < STALE)) {
            return;
        }
        imp.busy.set(true);
        self.sync_busy();
        let (page, wanted) = (self.downgrade(), imp.source.borrow().clone());
        glib::spawn_future_local(async move {
            let stores = || async { backend::pinned(|core| async move { core.sources().await }).await.into_iter().filter(signed_in).collect::<Vec<_>>() };
            let listed_stores = stores().await;
            let source =
                if listed_stores.iter().any(|s| text(s, "id") == wanted) { wanted } else { listed_stores.first().map(|s| text(s, "id")).unwrap_or_default() };
            let listed = if source.is_empty() {
                Ok(Vec::new())
            } else {
                let id = source.clone();
                backend::pinned(move |core| async move { core.source_library(&id, refresh).await }).await
            };
            let listed_stores = if refresh && !source.is_empty() { stores().await } else { listed_stores };
            let dir = listed_stores.iter().find(|s| text(s, "id") == source).map(|s| text(s, "games_dir")).unwrap_or_default();
            let free = free_space(&dir).await;
            if let Some(page) = page.upgrade() {
                page.listed(listed_stores, source, listed, free, refresh);
            }
        });
    }

    fn listed(&self, stores: Vec<Value>, source: String, listed: universe::Result<Vec<Value>>, free: u64, refresh: bool) {
        let imp = self.imp();
        imp.busy.set(false);
        imp.loaded.set(true);
        if *imp.source.borrow() != source {
            imp.games.replace(Vec::new());
        }
        self.set_stores(stores, &source);
        imp.free.set(free);
        match listed {
            Ok(games) => {
                imp.games.replace(games);
                imp.error.replace(String::new());
                if !source.is_empty() {
                    imp.loaded_at.set(Some(Instant::now()));
                }
            }
            Err(e) => {
                if !imp.games.borrow().is_empty() {
                    self.say(&e.to_string());
                }
                imp.error.replace(e.to_string());
            }
        }
        self.sync_busy();
        if let Some(app) = self.app() {
            app.check_updates(if refresh { None } else { Some(STALE) });
        }
        self.rebuild();
    }

    fn sync_busy(&self) {
        let imp = self.imp();
        let shown: &gtk::Widget = if imp.busy.get() { imp.refresh_spinner.upcast_ref() } else { imp.refresh.upcast_ref() };
        imp.refresh_stack.set_visible_child(shown);
        self.sync_actions();
    }

    fn set_stores(&self, stores: Vec<Value>, source: &str) {
        let imp = self.imp();
        let ids = |list: &[Value]| list.iter().map(|s| text(s, "id")).collect::<Vec<_>>();
        let same = ids(&imp.stores.borrow()) == ids(&stores);
        let index = stores.iter().position(|s| text(s, "id") == source);
        let names: Vec<String> = stores.iter().map(|s| text(s, "name")).collect();
        imp.sources.set_visible(stores.len() > 1);
        imp.stores.replace(stores);
        imp.source.replace(source.to_string());
        imp.syncing.set(true);
        if !same {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            imp.sources.set_model(Some(&gtk::StringList::new(&names)));
        }
        if let Some(index) = index {
            imp.sources.set_selected(index as u32);
        }
        imp.syncing.set(false);
    }

    fn set_source(&self, id: &str) {
        let imp = self.imp();
        if *imp.source.borrow() == id {
            return;
        }
        imp.source.replace(id.to_string());
        imp.games.replace(Vec::new());
        imp.error.replace(String::new());
        imp.loaded_at.set(None);
        let query = imp.query.take();
        self.searched(&query);
        self.load(false);
    }

    fn searched(&self, query: &str) {
        let imp = self.imp();
        let query = query.trim().to_string();
        if *imp.query.borrow() == query && imp.results.borrow().is_some() {
            return;
        }
        imp.words.replace(library::search_words(&query));
        imp.query.replace(query.clone());
        imp.results.replace(None);
        self.rebuild();
        let source = imp.source.borrow().clone();
        if query.is_empty() || source.is_empty() {
            return;
        }
        let page = self.downgrade();
        glib::spawn_future_local(async move {
            let asked = query.clone();
            let found = backend::pinned(move |core| async move { core.source_search(&source, &asked).await }).await;
            let Some(page) = page.upgrade().filter(|p| *p.imp().query.borrow() == query) else { return };
            let results = found.unwrap_or_else(|e| {
                page.say(&e.to_string());
                Vec::new()
            });
            page.imp().results.replace(Some(results));
            page.rebuild();
        });
    }

    fn games_dir(&self) -> String {
        let imp = self.imp();
        let source = imp.source.borrow();
        imp.stores.borrow().iter().find(|s| text(s, "id") == *source).map(|s| text(s, "games_dir")).unwrap_or_default()
    }

    /// A game of the listing or of the search, by store id.
    fn entry(&self, id: &str) -> Option<Entry> {
        let imp = self.imp();
        let games = imp.games.borrow();
        let results = imp.results.borrow();
        games.iter().chain(results.iter().flatten()).find(|g| text(g, "id") == id).map(Entry::of)
    }

    /// The art a row shows: the library's own for the game when it has some, else the store's.
    fn art(&self, entry: &Entry) -> String {
        let game = self.app().and_then(|app| app.library().get(&entry.game_id));
        let Some(game) = game else { return entry.image.clone() };
        let row = game.row().clone();
        [row.banner, entry.image.clone(), row.background, row.cover].into_iter().find(|p| !p.is_empty()).unwrap_or_default()
    }

    fn in_library(&self, entry: &Entry) -> bool {
        !entry.game_id.is_empty() && self.app().is_some_and(|app| app.library().get(&entry.game_id).is_some())
    }

    fn row(&self, entry: &Entry, line: &str) -> adw::ActionRow {
        let row = adw::ActionRow::builder().title(&entry.title).subtitle(line).use_markup(false).title_lines(1).subtitle_lines(1).build();
        let art = Cover::new(80, 45);
        art.add_css_class("thumb");
        art.set_valign(gtk::Align::Center);
        art.set_path(self.art(entry));
        row.add_prefix(&art);
        row
    }

    fn add(&self, group: &adw::PreferencesGroup, row: &adw::ActionRow) {
        group.add(row);
        self.imp().rows.borrow_mut().push((group.clone(), row.clone()));
    }

    fn installed_menu(&self, entry: &Entry) -> gio::Menu {
        let menu = gio::Menu::new();
        if self.in_library(entry) {
            let game = gio::Menu::new();
            for (label, action) in
                [(gettext("_Play"), "win.play-game"), (gettext("_Details"), "win.open-game"), (gettext("Game _Settings"), "win.game-settings")]
            {
                let item = gio::MenuItem::new(Some(&label), None);
                item.set_action_and_target_value(Some(action), Some(&entry.game_id.to_variant()));
                game.append_item(&item);
            }
            menu.append_section(None, &game);
        }
        let item = gio::MenuItem::new(Some(&gettext("_Uninstall…")), None);
        item.set_action_and_target_value(Some("store.uninstall"), Some(&entry.id.to_variant()));
        let end = gio::Menu::new();
        end.append_item(&item);
        menu.append_section(None, &end);
        menu
    }

    fn rebuild(&self) {
        let imp = self.imp();
        for (group, row) in imp.rows.take() {
            group.remove(&row);
        }
        imp.sizeless.borrow_mut().clear();
        let app = self.app();
        let source = imp.source.borrow().clone();
        let words = imp.words.borrow().clone();
        let games: Vec<Entry> = imp.games.borrow().iter().map(Entry::of).collect();
        let job = app.as_ref().and_then(|app| app.job());
        let busy = job.as_ref().filter(|j| j.pauses() && j.source() == source).map(|j| j.target()).unwrap_or_default();
        let shown = |e: &Entry| e.id != busy && e.matches(&words);
        let by_title = |a: &&Entry, b: &&Entry| library::fold(&a.title).cmp(&library::fold(&b.title));

        let mut paused: Vec<&Entry> = games.iter().filter(|e| e.partial && shown(e)).collect();
        paused.sort_by(by_title);
        for entry in &paused {
            let row = self.row(entry, &entry.paused());
            row.add_suffix(&button(&gettext("_Resume"), "store.resume", &entry.id));
            self.add(&imp.job_group, &row);
        }
        imp.job_row.set_visible(job.is_some());
        imp.job_group.set_visible(job.is_some() || !paused.is_empty());
        imp.job_group.set_title(&match job.as_ref().map(|j| j.kind()) {
            Some(Kind::Install) => gettext("Installing"),
            Some(Kind::Update) => gettext("Updating"),
            Some(Kind::Scan | Kind::Artwork) => gettext("In Progress"),
            None => gettext("Paused"),
        });

        let updates: Vec<Value> = app.as_ref().map(|app| app.updates()).unwrap_or_default().into_iter().filter(|u| text(u, "source") == source).collect();
        let pending: HashSet<String> = updates.iter().map(|u| text(u, "id")).collect();
        let mut updating = 0;
        for update in &updates {
            let found = games.iter().find(|e| e.id == text(update, "id")).cloned();
            let entry = found.unwrap_or_else(|| Entry { id: text(update, "id"), title: text(update, "title"), ..Entry::default() });
            if !shown(&entry) {
                continue;
            }
            let version = text(update, "version");
            let line = if version.is_empty() { gettext("A newer build") } else { gettext("Version {}").replace("{}", &version) };
            let row = self.row(&entry, &line);
            row.add_suffix(&button(&gettext("_Update"), "store.update", &entry.id));
            self.add(&imp.updates_group, &row);
            updating += 1;
        }
        imp.updates_group.set_visible(updating > 0);

        let mut installed: Vec<&Entry> = games.iter().filter(|e| e.installed && shown(e)).collect();
        installed.sort_by(by_title);
        for entry in &installed {
            let mut line = vec![];
            if pending.contains(&entry.id) {
                line.push(gettext("Update available"));
            }
            if entry.disk > 0 {
                line.push(size(entry.disk));
            }
            let row = self.row(entry, &line.join(" · "));
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text(gettext("More"))
                .valign(gtk::Align::Center)
                .menu_model(&self.installed_menu(entry))
                .css_classes(["flat"])
                .build();
            row.add_suffix(&more);
            if self.in_library(entry) {
                row.set_activatable(true);
                row.set_action_name(Some("win.open-game"));
                row.set_action_target_value(Some(&entry.game_id.to_variant()));
            }
            self.add(&imp.installed_group, &row);
        }
        let count = games.iter().filter(|e| e.installed).count();
        let on_disk: u64 = games.iter().filter(|e| e.installed).map(|e| e.disk).sum();
        let mut about = vec![ngettext("{} game", "{} games", count as u32).replace("{}", &count.to_string())];
        if on_disk > 0 {
            about.push(size(on_disk));
        }
        if !self.games_dir().is_empty() {
            about.push(tilde(&self.games_dir()));
        }
        imp.installed_group.set_description(Some(&about.join(" · ")));
        imp.installed_group.set_visible(!installed.is_empty());

        let mut owned: Vec<&Entry> = games.iter().filter(|e| !e.installed && !e.partial && e.owned != Some(false) && shown(e)).collect();
        owned.sort_by(by_title);
        for entry in &owned {
            let row = self.row(entry, &entry.cost());
            row.add_suffix(&button(&gettext("_Install"), "store.install", &entry.id));
            if entry.download == 0 && entry.disk == 0 {
                let motion = gtk::EventControllerMotion::new();
                let (page, id) = (self.downgrade(), entry.id.clone());
                motion.connect_enter(move |_, _, _| {
                    page.upgrade().inspect(|page| page.peek(&id));
                });
                row.add_controller(motion);
                imp.sizeless.borrow_mut().insert(entry.id.clone(), row.clone());
            }
            self.add(&imp.owned_group, &row);
        }
        imp.owned_group.set_description(Some(&self.listing_line(games.iter().filter(|e| !e.installed && !e.partial).count())));
        imp.owned_group.set_visible(!owned.is_empty());

        let results = imp.results.borrow().clone();
        let mut found = 0;
        for entry in results.iter().flatten().map(Entry::of) {
            if games.iter().any(|g| g.id == entry.id) {
                continue;
            }
            let owned = entry.owned == Some(true);
            let row = self.row(&entry, &if owned { gettext("Owned") } else { gettext("Not owned") });
            if owned && !entry.installed {
                row.add_suffix(&button(&gettext("_Install"), "store.install", &entry.id));
            }
            self.add(&imp.results_group, &row);
            found += 1;
        }
        imp.results_group.set_visible(found > 0);

        let any = job.is_some() || !paused.is_empty() || updating > 0 || !installed.is_empty() || !owned.is_empty() || found > 0;
        let error = imp.error.borrow().clone();
        let name = if !imp.loaded.get() {
            "loading"
        } else if imp.stores.borrow().is_empty() {
            "signin"
        } else if games.is_empty() && !error.is_empty() {
            imp.failed_page.set_description(Some(&glib::markup_escape_text(&error)));
            "failed"
        } else if any {
            "list"
        } else if words.is_empty() {
            "empty"
        } else if results.is_none() {
            "loading"
        } else {
            "no-results"
        };
        imp.stack.set_visible_child_name(name);
        imp.search.set_sensitive(!imp.stores.borrow().is_empty());
        self.sync_job_row();
    }

    /// `12 games · refreshed 3 h ago`, or why the store could not be reached.
    fn listing_line(&self, count: usize) -> String {
        let imp = self.imp();
        let games = ngettext("{} game", "{} games", count as u32).replace("{}", &count.to_string());
        let source = imp.source.borrow().clone();
        let store = imp.stores.borrow().iter().find(|s| text(s, "id") == source).cloned().unwrap_or_default();
        let at = chrono::DateTime::parse_from_rfc3339(&text(&store, "library_at")).map(|t| t.timestamp()).ok();
        let age = at.map(|at| format::ago(at, chrono::Local::now()));
        match (imp.error.borrow().is_empty(), age) {
            (true, Some(age)) => format!("{games} · {}", gettext("refreshed {}").replace("{}", &age)),
            (true, None) => games,
            (false, Some(age)) => gettext("{} could not be reached · listing from {}").replacen("{}", &text(&store, "name"), 1).replacen("{}", &age, 1),
            (false, None) => gettext("{} could not be reached").replace("{}", &text(&store, "name")),
        }
    }

    fn job_changed(&self) {
        let imp = self.imp();
        if let Some((job, handlers)) = imp.job.take() {
            for handler in handlers {
                job.disconnect(handler);
            }
        }
        match self.app().and_then(|app| app.job()) {
            Some(job) => {
                let mut handlers: Vec<glib::SignalHandlerId> = ["message", "done", "total", "cancelled"]
                    .into_iter()
                    .map(|name| {
                        let page = self.downgrade();
                        job.connect_notify_local(Some(name), move |_, _| {
                            page.upgrade().inspect(|page| page.sync_job_row());
                        })
                    })
                    .collect();
                let page = self.downgrade();
                handlers.push(job.connect_notify_local(Some("step"), move |_, _| {
                    page.upgrade().inspect(|page| page.rebuild());
                }));
                imp.job.replace(Some((job, handlers)));
            }
            None if imp.loaded.get() => {
                imp.loaded_at.set(None);
                if self.is_mapped() {
                    self.load(false);
                }
            }
            None => {}
        }
        self.sync_actions();
        self.rebuild();
    }

    fn sync_job_row(&self) {
        let imp = self.imp();
        self.sync_actions();
        let Some(job) = imp.job.borrow().as_ref().map(|(job, _)| job.clone()) else { return };
        imp.job_row.set_title(&if job.title().is_empty() { job.label() } else { job.title() });
        let mut line = vec![];
        if job.cancelled() {
            line.push(gettext("Stopping…"));
        } else {
            line.push(match job.kind() {
                Kind::Install => gettext("Installing"),
                Kind::Update if job.steps() > 1 => {
                    gettext("Updating, {} of {}").replacen("{}", &job.step().to_string(), 1).replacen("{}", &job.steps().to_string(), 1)
                }
                Kind::Update => gettext("Updating"),
                Kind::Artwork if !job.title().is_empty() => gettext("Fetching art"),
                Kind::Scan | Kind::Artwork => String::new(),
            });
            line.push(job.message());
            if job.total() > 0 {
                let (done, total) = if job.pauses() { (size(job.done()), size(job.total())) } else { (job.done().to_string(), job.total().to_string()) };
                line.push(gettext("{} of {}").replacen("{}", &done, 1).replacen("{}", &total, 1));
            }
        }
        line.retain(|part| !part.is_empty());
        imp.job_row.set_subtitle(&line.join(" · "));
        let measured = job.total() > 0;
        imp.job_progress.set_visible(measured);
        imp.job_progress.set_fraction(job.fraction());
        imp.job_spinner.set_visible(!measured);
        imp.job_cancel.set_visible(job.cancellable());
        let entry = self.entry(&job.target()).unwrap_or_default();
        imp.job_art.set_path(self.art(&entry));
    }

    /// The cursor on a game without a size: the store's `info` for it, one at a time; the core keeps what it learns.
    fn peek(&self, id: &str) {
        let imp = self.imp();
        if imp.peeking.get() || !imp.peeked.borrow_mut().insert(id.to_string()) {
            return;
        }
        imp.peeking.set(true);
        let (page, source, id) = (self.downgrade(), imp.source.borrow().clone(), id.to_string());
        glib::spawn_future_local(async move {
            let info = page.upgrade().map(|page| page.info(&source, &id));
            let Some(info) = info else { return };
            let learnt = info.await;
            let Some(page) = page.upgrade() else { return };
            page.imp().peeking.set(false);
            if let Some((download, disk)) = learnt {
                page.sized(&source, &id, download, disk);
            }
        });
    }

    /// The store's sizes for a game: download and disk.
    fn info(&self, source: &str, id: &str) -> impl std::future::Future<Output = Option<(u64, u64)>> + 'static {
        let (source, id) = (source.to_string(), id.to_string());
        async move {
            let (s, i) = (source.clone(), id.clone());
            match backend::pinned(move |core| async move { core.source_info(&s, &i).await }).await {
                Ok(info) => Some((bytes(&info, "download_size"), bytes(&info, "disk_size"))).filter(|&(d, k)| d > 0 || k > 0),
                Err(e) => {
                    tracing::debug!("{source} info {id}: {e}");
                    None
                }
            }
        }
    }

    /// A game's sizes came in: the listing keeps them and its row says them.
    fn sized(&self, source: &str, id: &str, download: u64, disk: u64) -> Option<Entry> {
        let imp = self.imp();
        if *imp.source.borrow() != source {
            return None;
        }
        let mut learnt = None;
        for game in imp.games.borrow_mut().iter_mut().filter(|g| text(g, "id") == id) {
            game["download_size"] = download.into();
            game["disk_size"] = disk.into();
            learnt = Some(Entry::of(game));
        }
        if let (Some(entry), Some(row)) = (&learnt, imp.sizeless.borrow().get(id)) {
            row.set_subtitle(&entry.cost());
        }
        learnt
    }

    fn install_body(&self, entry: &Entry, checking: bool) -> String {
        let sizes = match (entry.download, entry.disk) {
            (0, 0) if checking => gettext("Checking the size…"),
            (0, 0) => gettext("The size is not known"),
            (download, 0) => gettext("{} to download").replace("{}", &size(download)),
            (0, disk) => gettext("{} on disk").replace("{}", &size(disk)),
            (download, disk) => gettext("{} to download, {} on disk").replacen("{}", &size(download), 1).replacen("{}", &size(disk), 1),
        };
        let (dir, free) = (self.games_dir(), self.imp().free.get());
        let mut lines = vec![sizes];
        match (free, dir.is_empty()) {
            (0, false) => lines.push(gettext("Into {}").replace("{}", &tilde(&dir))),
            (free, false) => lines.push(gettext("{} free in {}").replacen("{}", &size(free), 1).replacen("{}", &tilde(&dir), 1)),
            _ => {}
        }
        if free > 0 && entry.disk > free {
            lines.push(gettext("There is not enough free space for it."));
        }
        lines.join("\n")
    }

    fn confirm_install(&self, id: &str) {
        let Some(entry) = self.entry(id) else { return };
        let unknown = entry.download == 0 && entry.disk == 0;
        let dialog = adw::AlertDialog::new(Some(&gettext("Install {}?").replace("{}", &entry.title)), Some(&self.install_body(&entry, unknown)));
        dialog.add_responses(&[("cancel", &gettext("_Not Now")), ("install", &gettext("_Install"))]);
        dialog.set_response_appearance("install", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("install"));
        dialog.set_close_response("cancel");
        let (page, asked, source, dir) = (self.downgrade(), dialog.downgrade(), self.imp().source.borrow().clone(), self.games_dir());
        let known = entry.clone();
        glib::spawn_future_local(async move {
            let free = free_space(&dir).await;
            let sizes = match page.upgrade() {
                Some(page) if unknown => page.info(&source, &known.id).await,
                _ => None,
            };
            let (Some(page), Some(dialog)) = (page.upgrade(), asked.upgrade()) else { return };
            page.imp().free.set(free);
            let entry = sizes.and_then(|(download, disk)| page.sized(&source, &known.id, download, disk)).unwrap_or(known);
            dialog.set_body(&page.install_body(&entry, false));
        });
        let page = self.downgrade();
        let target = (entry.id.clone(), entry.title.clone());
        dialog.connect_response(Some("install"), move |_, _| {
            page.upgrade().inspect(|page| page.start(Kind::Install, vec![target.clone()]));
        });
        dialog.present(Some(self));
    }

    fn confirm_uninstall(&self, id: &str) {
        let Some(entry) = self.entry(id).filter(|e| !e.game_id.is_empty()) else { return };
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Uninstall {}?").replace("{}", &entry.title)),
            Some(&gettext("Its install folder goes to the trash. The game stays in the library with its hours, journal and recordings.")),
        );
        dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("uninstall", &gettext("_Uninstall"))]);
        dialog.set_response_appearance("uninstall", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let page = self.downgrade();
        dialog.connect_response(Some("uninstall"), move |_, _| {
            page.upgrade().inspect(|page| page.uninstall(&entry));
        });
        dialog.present(Some(self));
    }

    fn uninstall(&self, entry: &Entry) {
        let (page, game_id, title) = (self.downgrade(), entry.game_id.clone(), entry.title.clone());
        glib::spawn_future_local(async move {
            let id = game_id.clone();
            let result = backend::call(move |core| async move { core.uninstall(&id).await }).await;
            let Some(page) = page.upgrade() else { return };
            match result {
                Ok(()) => {
                    page.say(&gettext("{} is uninstalled").replace("{}", &title));
                    if let Some(app) = page.app() {
                        app.library().refresh(&[game_id]).await;
                    }
                }
                Err(e) => page.say(&e.to_string()),
            }
            page.imp().loaded_at.set(None);
            page.load(false);
        });
    }

    fn start(&self, kind: Kind, targets: Vec<(String, String)>) {
        let source = self.imp().source.borrow().clone();
        if let (Some(app), false) = (self.app(), source.is_empty()) {
            app.start_job(kind, &source, targets, false);
        }
    }

    fn start_on(&self, kind: Kind, id: &str) {
        let title = self.entry(id).map(|e| e.title).unwrap_or_else(|| id.to_string());
        self.start(kind, vec![(id.to_string(), title)]);
    }

    fn update_all(&self) {
        let source = self.imp().source.borrow().clone();
        let updates = self.app().map(|app| app.updates()).unwrap_or_default();
        let targets: Vec<(String, String)> = updates.iter().filter(|u| text(u, "source") == source).map(|u| (text(u, "id"), text(u, "title"))).collect();
        if !targets.is_empty() {
            self.start(Kind::Update, targets);
        }
    }
}
