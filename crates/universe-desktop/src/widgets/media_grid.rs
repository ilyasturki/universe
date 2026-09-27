use std::cell::{Cell, OnceCell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use universe::changes::Event;

use crate::app::Application;
use crate::backend;
use crate::media::{Kind, MediaItem};
use crate::pages::{journal, player, viewer};
use crate::widgets::MediaCard;
use crate::window::Window;

// The core makes a listing's missing thumbnails in the background, three at a time, and says nothing when one lands.
const THUMB_POLL: Duration = Duration::from_secs(1);
const THUMB_POLLS: u32 = 120;

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct MediaGrid {
        pub stack: adw::ViewStack,
        pub grid: gtk::GridView,
        pub empty: adw::StatusPage,
        pub store: gio::ListStore,
        pub filter: OnceCell<gtk::CustomFilter>,
        pub shown: OnceCell<gtk::FilterListModel>,
        pub app: glib::WeakRef<Application>,
        /// The game whose media shows; empty for every game.
        pub game: RefCell<String>,
        pub kinds: RefCell<Vec<Kind>>,
        pub loaded: Cell<bool>,
        pub stale: Cell<bool>,
        pub generation: Cell<u64>,
        pub polls: Cell<u32>,
        pub reload: RefCell<Option<glib::SourceId>>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
    }

    impl Default for MediaGrid {
        fn default() -> Self {
            MediaGrid {
                stack: adw::ViewStack::new(),
                grid: gtk::GridView::builder().max_columns(8).min_columns(1).single_click_activate(true).name("media-grid").build(),
                empty: adw::StatusPage::new(),
                store: gio::ListStore::new::<MediaItem>(),
                filter: OnceCell::new(),
                shown: OnceCell::new(),
                app: glib::WeakRef::new(),
                game: RefCell::default(),
                kinds: RefCell::new(vec![Kind::Shot, Kind::Recording, Kind::Journal]),
                loaded: Cell::new(false),
                stale: Cell::new(true),
                generation: Cell::new(0),
                polls: Cell::new(0),
                reload: RefCell::default(),
                handlers: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaGrid {
        const NAME: &'static str = "UniverseMediaGrid";
        type Type = super::MediaGrid;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for MediaGrid {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let spinner = adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build();
            self.stack.add_named(&spinner, Some("loading"));
            self.stack.add_named(&self.empty, Some("empty"));
            let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&self.grid).build();
            self.stack.add_named(&scroll, Some("grid"));
            self.stack.set_visible_child_name("loading");
            obj.set_child(Some(&self.stack));

            let page = obj.downgrade();
            let filter = gtk::CustomFilter::new(move |item| {
                let (Some(page), Some(item)) = (page.upgrade(), item.downcast_ref::<MediaItem>()) else { return false };
                page.imp().kinds.borrow().contains(&item.kind()) && !page.app().is_some_and(|app| app.is_deferred(&item.key()))
            });
            let shown = gtk::FilterListModel::new(Some(self.store.clone()), Some(filter.clone()));
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, item| {
                item.downcast_ref::<gtk::ListItem>().expect("a list item").set_child(Some(&MediaCard::default()));
            });
            let page = obj.downgrade();
            factory.connect_bind(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
                let alone = page.upgrade().is_some_and(|page| !page.imp().game.borrow().is_empty());
                if let (Some(card), Some(media)) = (item.child().and_downcast::<MediaCard>(), item.item().and_downcast::<MediaItem>()) {
                    card.bind(&media, alone);
                }
            });
            factory.connect_unbind(|_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("a list item");
                item.child().and_downcast::<MediaCard>().inspect(|card| card.unbind());
            });
            self.grid.set_factory(Some(&factory));
            self.grid.set_model(Some(&gtk::NoSelection::new(Some(shown.clone()))));
            let page = obj.downgrade();
            shown.connect_items_changed(move |_, _, _, _| {
                page.upgrade().inspect(|page| page.update_state());
            });
            let page = obj.downgrade();
            self.grid.connect_activate(move |_, position| {
                page.upgrade().inspect(|page| page.open(position));
            });
            let _ = self.filter.set(filter);
            let _ = self.shown.set(shown);
            obj.connect_map(|grid| {
                if grid.imp().stale.get() {
                    grid.load();
                }
            });
        }

        fn dispose(&self) {
            self.release();
        }
    }

    impl MediaGrid {
        pub(super) fn release(&self) {
            if let Some(app) = self.app.upgrade() {
                for handler in self.handlers.take() {
                    app.disconnect(handler);
                }
            }
            if let Some(source) = self.reload.take() {
                source.remove();
            }
        }
    }

    impl WidgetImpl for MediaGrid {}
    impl BinImpl for MediaGrid {}
}

glib::wrapper! {
    /// The shots, recordings and journal entries of one game or all of them, newest first, as a grid of cards.
    pub struct MediaGrid(ObjectSubclass<imp::MediaGrid>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for MediaGrid {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl MediaGrid {
    fn app(&self) -> Option<Application> {
        self.imp().app.upgrade()
    }

    /// Lists `game`'s media (every game's when empty) and keeps up with what lands.
    pub fn follow(&self, app: &Application, game: &str) {
        let imp = self.imp();
        imp.app.set(Some(app));
        imp.game.replace(game.to_string());
        imp.release();
        let grid = self.downgrade();
        let changed = app.connect_changed(move |event| {
            let Some(grid) = grid.upgrade() else { return };
            let game = grid.imp().game.borrow().clone();
            let concerns = |id: &str| game.is_empty() || id == game;
            let moved = match event {
                Event::Screenshots(id) | Event::Journal(id) => concerns(id),
                Event::Library(ids) => ids.is_empty() || ids.iter().any(|id| concerns(id)),
                Event::SessionEnded(ended) => concerns(&ended.id),
                _ => false,
            };
            if moved {
                grid.reload_later();
            }
        });
        let grid = self.downgrade();
        let landed = app.connect_frame_landed(move |recording, index| {
            let Some(grid) = grid.upgrade() else { return };
            if index != universe::frames::THUMB {
                return;
            }
            for item in grid.imp().store.iter::<MediaItem>().flatten() {
                if item.kind() == Kind::Recording && item.row().path == recording {
                    item.set_picture(universe::frames::file(std::path::Path::new(recording), index).to_string_lossy().into_owned());
                }
            }
        });
        let grid = self.downgrade();
        let deferred = app.connect_deferred_changed(move || {
            if let Some(filter) = grid.upgrade().and_then(|g| g.imp().filter.get().cloned()) {
                filter.changed(gtk::FilterChange::Different);
            }
        });
        imp.handlers.replace(vec![changed, landed, deferred]);
    }

    pub fn set_kinds(&self, kinds: &[Kind]) {
        let imp = self.imp();
        if *imp.kinds.borrow() == kinds {
            return;
        }
        imp.kinds.replace(kinds.to_vec());
        if let Some(filter) = imp.filter.get() {
            filter.changed(gtk::FilterChange::Different);
        }
        self.update_state();
    }

    fn reload_later(&self) {
        let imp = self.imp();
        imp.stale.set(true);
        if !self.is_mapped() || imp.reload.borrow().is_some() {
            return;
        }
        let grid = self.downgrade();
        let source = glib::timeout_add_local_once(Duration::from_millis(400), move || {
            let Some(grid) = grid.upgrade() else { return };
            grid.imp().reload.take();
            grid.load();
        });
        imp.reload.replace(Some(source));
    }

    pub fn load(&self) {
        let imp = self.imp();
        let generation = imp.generation.get() + 1;
        imp.generation.set(generation);
        imp.stale.set(false);
        let (grid, game) = (self.downgrade(), imp.game.borrow().clone());
        glib::spawn_future_local(async move {
            let rows = backend::call(move |core| async move { core.media(&game).await }).await;
            let Some(grid) = grid.upgrade().filter(|g| g.imp().generation.get() == generation) else { return };
            let Some(app) = grid.app() else { return };
            let imp = grid.imp();
            match rows {
                Ok(rows) => {
                    let same =
                        imp.store.n_items() as usize == rows.len() && imp.store.iter::<MediaItem>().flatten().zip(&rows).all(|(item, row)| *item.row() == *row);
                    if !same {
                        let items: Vec<MediaItem> = rows.into_iter().map(|row| MediaItem::new(row, &app)).collect();
                        imp.store.splice(0, imp.store.n_items(), &items);
                    }
                }
                Err(e) => tracing::warn!("media: {e}"),
            }
            imp.loaded.set(true);
            grid.update_state();
            grid.poll_thumbs();
        });
    }

    fn poll_thumbs(&self) {
        let imp = self.imp();
        if imp.polls.get() > 0 || !imp.store.iter::<MediaItem>().flatten().any(|item| item.awaits()) {
            return;
        }
        imp.polls.set(1);
        let grid = self.downgrade();
        glib::timeout_add_local(THUMB_POLL, move || {
            let Some(grid) = grid.upgrade() else { return glib::ControlFlow::Break };
            let imp = grid.imp();
            let waiting = imp.store.iter::<MediaItem>().flatten().filter(|item| item.awaits() && !item.check_awaited()).count();
            let polls = imp.polls.get() + 1;
            if waiting == 0 || polls > THUMB_POLLS {
                imp.polls.set(0);
                return glib::ControlFlow::Break;
            }
            imp.polls.set(polls);
            glib::ControlFlow::Continue
        });
    }

    fn update_state(&self) {
        let imp = self.imp();
        let kinds = imp.kinds.borrow().clone();
        let (icon, title, description) = match kinds.as_slice() {
            [Kind::Shot] => ("camera-photo-symbolic", gettext("No Screenshots"), gettext("The shots taken while playing show up here")),
            [Kind::Recording] => ("camera-video-symbolic", gettext("No Recordings"), gettext("Sessions recorded by the capture module show up here")),
            [Kind::Journal] => ("text-x-generic-symbolic", gettext("No Journal Entries"), gettext("An entry is written after each session")),
            _ => ("camera-photo-symbolic", gettext("No Media Yet"), gettext("Screenshots, recordings and journal entries of your sessions show up here")),
        };
        imp.empty.set_icon_name(Some(icon));
        imp.empty.set_title(&title);
        imp.empty.set_description(Some(&description));
        let name = if !imp.loaded.get() {
            "loading"
        } else if imp.shown.get().is_some_and(|m| m.n_items() > 0) {
            "grid"
        } else {
            "empty"
        };
        imp.stack.set_visible_child_name(name);
    }

    fn open(&self, position: u32) {
        let imp = self.imp();
        let (Some(shown), Some(win)) = (imp.shown.get(), self.root().and_downcast::<Window>()) else { return };
        let Some(item) = shown.item(position).and_downcast::<MediaItem>() else { return };
        match item.kind() {
            Kind::Shot => {
                let shots: Vec<MediaItem> =
                    shown.iter::<glib::Object>().flatten().filter_map(|o| o.downcast::<MediaItem>().ok()).filter(|i| i.kind() == Kind::Shot).collect();
                let index = shots.iter().position(|i| *i == item).unwrap_or(0);
                viewer::open(&win, shots.iter().map(|i| viewer::Shot::of(&i.row())).collect(), index);
            }
            Kind::Recording => player::open(&win, &item.row()),
            Kind::Journal => {
                let row = item.row();
                journal::open_entry(&win, &row.game, &row.session);
            }
        }
    }
}
