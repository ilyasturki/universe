use std::cell::{Ref, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use universe::timeline::MediaRow;

use crate::app::Application;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Shot,
    Recording,
    Journal,
}

impl Kind {
    fn of(kind: &str) -> Kind {
        match kind {
            "recording" => Kind::Recording,
            "journal" => Kind::Journal,
            _ => Kind::Shot,
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::MediaItem)]
    pub struct MediaItem {
        /// What the card shows: a thumbnail or a frame once it is there, empty meanwhile.
        #[property(get, set)]
        pub picture: RefCell<String>,
        pub row: RefCell<Option<MediaRow>>,
        /// The thumbnail the core is still making.
        pub awaited: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaItem {
        const NAME: &'static str = "UniverseMediaItem";
        type Type = super::MediaItem;
    }

    #[glib::derived_properties]
    impl ObjectImpl for MediaItem {}
}

glib::wrapper! {
    /// A shot, a recording or a journal entry of the timeline, with the picture that stands for it.
    pub struct MediaItem(ObjectSubclass<imp::MediaItem>);
}

impl MediaItem {
    /// A recording's picture is its thumbnail frame, asked of the sampler when it is not cached; a shot's and an entry's the
    /// core's thumbnail once made.
    pub fn new(row: MediaRow, app: &Application) -> MediaItem {
        let item: MediaItem = glib::Object::new();
        let imp = item.imp();
        let picture = match Kind::of(&row.kind) {
            Kind::Recording => app.frame(&row.path, universe::frames::THUMB, row.duration_s).unwrap_or_default(),
            _ if row.thumb.is_empty() => row.path.clone(),
            _ if row.thumb_ready => row.thumb.clone(),
            _ => {
                imp.awaited.replace(row.thumb.clone());
                String::new()
            }
        };
        imp.picture.replace(picture);
        imp.row.replace(Some(row));
        item
    }

    pub fn row(&self) -> Ref<'_, MediaRow> {
        Ref::map(self.imp().row.borrow(), |row| row.as_ref().expect("a media item holds its row"))
    }

    pub fn kind(&self) -> Kind {
        Kind::of(&self.row().kind)
    }

    /// What a deferred delete names it by.
    pub fn key(&self) -> String {
        let row = self.row();
        match self.kind() {
            Kind::Shot => shot_key(&row.path),
            Kind::Recording => recording_key(&row.game, &row.session),
            Kind::Journal => journal_key(&row.game, &row.session),
        }
    }

    /// Takes the awaited thumbnail once it is on the disk; true when it came.
    pub fn check_awaited(&self) -> bool {
        let awaited = self.imp().awaited.borrow().clone();
        if awaited.is_empty() || !std::path::Path::new(&awaited).is_file() {
            return false;
        }
        self.imp().awaited.replace(String::new());
        self.set_picture(awaited);
        true
    }

    pub fn awaits(&self) -> bool {
        !self.imp().awaited.borrow().is_empty()
    }
}

pub fn shot_key(path: &str) -> String {
    format!("shot:{path}")
}

pub fn recording_key(game: &str, session: &str) -> String {
    format!("recording:{game}:{session}")
}

pub fn journal_key(game: &str, session: &str) -> String {
    format!("journal:{game}:{session}")
}

/// `Tuesday 22 September at 21:30`, or `Today at 21:30`.
pub fn moment(rfc3339: &str) -> String {
    let Ok(at) = chrono::DateTime::parse_from_rfc3339(rfc3339) else { return String::new() };
    let at = at.with_timezone(&chrono::Local);
    let now = chrono::Local::now();
    let clock = at.format("%H:%M").to_string();
    match (now.date_naive() - at.date_naive()).num_days() {
        0 => gettext("Today at {}").replace("{}", &clock),
        1 => gettext("Yesterday at {}").replace("{}", &clock),
        _ if at.format("%Y").to_string() == now.format("%Y").to_string() => {
            gettext("{} at {}").replacen("{}", &at.format("%A %-d %B").to_string(), 1).replacen("{}", &clock, 1)
        }
        _ => gettext("{} at {}").replacen("{}", &at.format("%-d %B %Y").to_string(), 1).replacen("{}", &clock, 1),
    }
}

/// The day alone, as a card says it: `today`, `3 days ago`, `22 September`.
pub fn day(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339).map(|at| crate::format::relative(at.timestamp(), chrono::Local::now())).unwrap_or_default()
}
