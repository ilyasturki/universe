use std::cell::{Cell, OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::app::Application;
use crate::game::{removal_key, GameObject};
use crate::library::{self, Sort, View};
use crate::widgets::GameCard;

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/library-page.ui")]
    pub struct LibraryPage {
        #[template_child]
        pub sidebar_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub search: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub menu_button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub stack: TemplateChild<adw::ViewStack>,
        #[template_child]
        pub grid: TemplateChild<gtk::GridView>,
        #[template_child]
        pub compact: TemplateChild<adw::Breakpoint>,
        pub is_compact: Cell<bool>,
        pub cards: RefCell<Vec<glib::WeakRef<GameCard>>>,
        pub view: RefCell<View>,
        pub sort: Cell<Sort>,
        pub show_hidden: Cell<bool>,
        pub words: RefCell<Vec<String>>,
        pub filter: OnceCell<gtk::CustomFilter>,
        pub sorter: OnceCell<gtk::CustomSorter>,
        pub shown: OnceCell<gtk::SortListModel>,
        pub store: OnceCell<gtk::gio::ListStore>,
        pub loaded: Cell<bool>,
    }

    impl Default for LibraryPage {
        fn default() -> Self {
            LibraryPage {
                sidebar_button: TemplateChild::default(),
                search: TemplateChild::default(),
                menu_button: TemplateChild::default(),
                stack: TemplateChild::default(),
                grid: TemplateChild::default(),
                compact: TemplateChild::default(),
                is_compact: Cell::new(false),
                cards: RefCell::default(),
                view: RefCell::new(View::All),
                sort: Cell::new(Sort::LastPlayed),
                show_hidden: Cell::new(false),
                words: RefCell::default(),
                filter: OnceCell::new(),
                sorter: OnceCell::new(),
                shown: OnceCell::new(),
                store: OnceCell::new(),
                loaded: Cell::new(false),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LibraryPage {
        const NAME: &'static str = "UniverseLibraryPage";
        type Type = super::LibraryPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for LibraryPage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            self.menu_button.set_menu_model(Some(&crate::menus::main()));
            let page = obj.downgrade();
            self.search.connect_search_changed(move |entry| {
                let Some(page) = page.upgrade() else { return };
                page.imp().words.replace(library::search_words(&entry.text()));
                page.refilter();
            });
            let page = obj.downgrade();
            self.search.connect_stop_search(move |entry| {
                entry.set_text("");
                page.upgrade().inspect(|page| {
                    page.imp().grid.grab_focus();
                });
            });
            let page = obj.downgrade();
            self.compact.connect_apply(move |_| {
                page.upgrade().inspect(|page| page.set_compact(true));
            });
            let page = obj.downgrade();
            self.compact.connect_unapply(move |_| {
                page.upgrade().inspect(|page| page.set_compact(false));
            });
            let page = obj.downgrade();
            self.grid.connect_activate(move |grid, position| {
                let Some(game) = grid.model().and_then(|m| m.item(position)).and_downcast::<GameObject>() else { return };
                if let Some(win) = page.upgrade().and_then(|p| p.root()).and_downcast::<crate::window::Window>() {
                    win.game_action("details", &game);
                }
            });
        }
    }

    impl WidgetImpl for LibraryPage {}
    impl BinImpl for LibraryPage {}
}

glib::wrapper! {
    /// The games of the sidebar's pick as a grid of covers, searched, sorted, the hidden ones apart.
    pub struct LibraryPage(ObjectSubclass<imp::LibraryPage>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl LibraryPage {
    pub fn search_entry(&self) -> gtk::SearchEntry {
        self.imp().search.get()
    }

    pub fn sidebar_button(&self) -> gtk::Button {
        self.imp().sidebar_button.get()
    }

    /// Shows `app`'s library, less the games whose removal waits on its Undo.
    pub fn set_library(&self, app: &Application) {
        let imp = self.imp();
        let library = app.library();
        let (page, weak_app) = (self.downgrade(), app.downgrade());
        let filter = gtk::CustomFilter::new(move |item| {
            let (Some(page), Some(app), Some(game)) = (page.upgrade(), weak_app.upgrade(), item.downcast_ref::<GameObject>()) else { return false };
            let imp = page.imp();
            let row = game.row();
            row.hidden == imp.show_hidden.get()
                && imp.view.borrow().holds(&row)
                && library::matches(&row, &imp.words.borrow())
                && !app.is_deferred(&removal_key(&row.id))
        });
        let page = self.downgrade();
        app.connect_deferred_changed(move || {
            page.upgrade().inspect(|page| page.refilter());
        });
        let page = self.downgrade();
        let sorter = gtk::CustomSorter::new(move |a, b| {
            let (Some(page), Some(a), Some(b)) = (page.upgrade(), a.downcast_ref::<GameObject>(), b.downcast_ref::<GameObject>()) else {
                return gtk::Ordering::Equal;
            };
            page.imp().sort.get().order(&a.row(), &b.row()).into()
        });
        let filtered = gtk::FilterListModel::new(Some(library.store.clone()), Some(filter.clone()));
        let shown = gtk::SortListModel::new(Some(filtered), Some(sorter.clone()));
        let factory = gtk::SignalListItemFactory::new();
        let page = self.downgrade();
        factory.connect_setup(move |_, item| {
            let card = GameCard::default();
            if let Some(page) = page.upgrade() {
                card.set_compact(page.imp().is_compact.get());
                page.imp().cards.borrow_mut().push(card.downgrade());
            }
            item.downcast_ref::<gtk::ListItem>().expect("a list item").set_child(Some(&card));
        });
        factory.connect_bind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
            if let (Some(card), Some(game)) = (item.child().and_downcast::<GameCard>(), item.item().and_downcast::<GameObject>()) {
                card.bind(&game);
            }
        });
        factory.connect_unbind(|_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
            item.child().and_downcast::<GameCard>().inspect(|card| card.unbind());
        });
        imp.grid.set_factory(Some(&factory));
        imp.grid.set_model(Some(&gtk::NoSelection::new(Some(shown.clone()))));
        let page = self.downgrade();
        shown.connect_items_changed(move |_, _, _, _| {
            page.upgrade().inspect(|page| page.update_state());
        });
        let _ = imp.filter.set(filter);
        let _ = imp.sorter.set(sorter);
        let _ = imp.shown.set(shown);
        let _ = imp.store.set(library.store.clone());
        let page = self.downgrade();
        library.connect_updated(move || {
            let Some(page) = page.upgrade() else { return };
            page.imp().loaded.set(true);
            page.refilter();
            page.resort();
        });
    }

    fn set_compact(&self, compact: bool) {
        let imp = self.imp();
        imp.is_compact.set(compact);
        imp.cards.borrow_mut().retain(|card| card.upgrade().inspect(|card| card.set_compact(compact)).is_some());
    }

    pub fn set_view(&self, view: View) {
        if *self.imp().view.borrow() == view {
            return;
        }
        self.imp().view.replace(view);
        self.refilter();
    }

    pub fn set_sort(&self, sort: Sort) {
        self.imp().sort.set(sort);
        self.resort();
    }

    pub fn set_show_hidden(&self, shown: bool) {
        self.imp().show_hidden.set(shown);
        self.refilter();
    }

    fn refilter(&self) {
        if let Some(filter) = self.imp().filter.get() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.update_state();
    }

    fn resort(&self) {
        if let Some(sorter) = self.imp().sorter.get() {
            sorter.changed(gtk::SorterChange::Different);
        }
    }

    fn update_state(&self) {
        let imp = self.imp();
        let name = if !imp.loaded.get() {
            "loading"
        } else if imp.store.get().is_none_or(|s| s.n_items() == 0) {
            "empty"
        } else if imp.shown.get().is_some_and(|m| m.n_items() > 0) {
            "grid"
        } else if !imp.words.borrow().is_empty() {
            "no-results"
        } else if imp.show_hidden.get() {
            "no-hidden"
        } else if *imp.view.borrow() == View::Favorites {
            "no-favorites"
        } else {
            "no-results"
        };
        imp.stack.set_visible_child_name(name);
    }
}
