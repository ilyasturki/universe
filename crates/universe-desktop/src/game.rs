use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;
use universe::library::Resolved;

/// What the library shows of a game, taken off the core's runtime in one piece.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub sort_title: String,
    pub developer: String,
    pub publisher: String,
    pub platform: String,
    pub source: String,
    pub runner: String,
    pub runner_name: String,
    pub installed: bool,
    pub hidden: bool,
    pub favorite: bool,
    pub hours: f64,
    pub play_count: u64,
    /// Unix seconds; 0 when never.
    pub last_played: i64,
    pub added: i64,
    pub release_year: u32,
    pub cover: String,
    pub square: String,
    pub banner: String,
    pub background: String,
    pub logo: String,
}

fn unix(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339).map(|t| t.timestamp()).unwrap_or(0)
}

impl Row {
    pub fn of(r: &Resolved) -> Row {
        let slot = |name: &str| r.media.iter().find(|(s, _)| s == name).map(|(_, p)| p.clone()).unwrap_or_default();
        let g = &r.game;
        Row {
            id: g.id.clone(),
            title: g.title.clone(),
            sort_title: if g.sort_title.is_empty() { g.title.clone() } else { g.sort_title.clone() },
            developer: g.metadata.developers.first().cloned().unwrap_or_default(),
            publisher: g.metadata.publishers.first().cloned().unwrap_or_default(),
            platform: r.effective.platform.clone(),
            source: g.source.kind.clone(),
            runner: r.effective.runner.clone(),
            runner_name: r.effective.runner_name.clone(),
            installed: g.is_installed(),
            hidden: g.hidden,
            favorite: g.favorite,
            hours: r.stats.hours,
            play_count: r.stats.play_count,
            last_played: r.stats.last_played.as_deref().map(unix).unwrap_or(0),
            added: unix(&g.added_at),
            release_year: g.release_year,
            cover: slot("box_front"),
            square: slot("square"),
            banner: slot("banner"),
            background: slot("background"),
            logo: slot("logo"),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::GameObject)]
    pub struct GameObject {
        #[property(get)]
        pub id: RefCell<String>,
        #[property(get)]
        pub title: RefCell<String>,
        #[property(get)]
        pub developer: RefCell<String>,
        #[property(get)]
        pub platform: RefCell<String>,
        #[property(get)]
        pub source: RefCell<String>,
        #[property(get)]
        pub installed: Cell<bool>,
        #[property(get)]
        pub hidden: Cell<bool>,
        #[property(get)]
        pub favorite: Cell<bool>,
        #[property(get)]
        pub cover: RefCell<String>,
        #[property(get, set)]
        pub playing: Cell<bool>,
        pub row: RefCell<Row>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GameObject {
        const NAME: &'static str = "UniverseGame";
        type Type = super::GameObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for GameObject {}
}

glib::wrapper! {
    pub struct GameObject(ObjectSubclass<imp::GameObject>);
}

impl GameObject {
    pub fn new(row: Row) -> Self {
        let game: Self = glib::Object::new();
        game.update(row);
        game
    }

    pub fn row(&self) -> std::cell::Ref<'_, Row> {
        self.imp().row.borrow()
    }

    /// Takes the row and notifies what changed, so a bound widget repaints only then.
    pub fn update(&self, row: Row) {
        let imp = self.imp();
        if *imp.row.borrow() == row {
            return;
        }
        imp.row.replace(row.clone());
        let text = |cell: &RefCell<String>, value: &str, name: &str| {
            if *cell.borrow() != value {
                cell.replace(value.to_string());
                self.notify(name);
            }
        };
        text(&imp.id, &row.id, "id");
        text(&imp.title, &row.title, "title");
        text(&imp.developer, &row.developer, "developer");
        text(&imp.platform, &row.platform, "platform");
        text(&imp.source, &row.source, "source");
        text(&imp.cover, &row.cover, "cover");
        let flag = |cell: &Cell<bool>, value: bool, name: &str| {
            if cell.replace(value) != value {
                self.notify(name);
            }
        };
        flag(&imp.installed, row.installed, "installed");
        flag(&imp.hidden, row.hidden, "hidden");
        flag(&imp.favorite, row.favorite, "favorite");
    }
}
