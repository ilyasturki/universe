use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use crate::format;
use crate::media::{self, Kind, MediaItem};
use crate::widgets::Cover;

pub const WIDTH: i32 = 256;
pub const HEIGHT: i32 = 144;

mod imp {
    use super::*;

    #[derive(Debug)]
    pub struct MediaCard {
        pub cover: Cover,
        pub badge: gtk::Box,
        pub badge_icon: gtk::Image,
        pub badge_label: gtk::Label,
        pub title: gtk::Label,
        pub subtitle: gtk::Label,
        pub excerpt: gtk::Label,
        pub item: RefCell<Option<(MediaItem, glib::SignalHandlerId)>>,
    }

    impl Default for MediaCard {
        fn default() -> Self {
            let label = |classes: &[&str]| {
                gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).max_width_chars(28).css_classes(classes.to_vec()).build()
            };
            MediaCard {
                cover: Cover::new(WIDTH, HEIGHT),
                badge: gtk::Box::builder().spacing(4).halign(gtk::Align::End).valign(gtk::Align::End).margin_end(8).margin_bottom(8).build(),
                badge_icon: gtk::Image::builder().pixel_size(12).build(),
                badge_label: gtk::Label::new(None),
                title: label(&["media-title"]),
                subtitle: label(&["dimmed", "caption"]),
                excerpt: gtk::Label::builder()
                    .xalign(0.0)
                    .wrap(true)
                    .lines(2)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(28)
                    .css_classes(["dimmed", "caption"])
                    .build(),
                item: RefCell::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MediaCard {
        const NAME: &'static str = "UniverseMediaCard";
        type Type = super::MediaCard;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for MediaCard {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_orientation(gtk::Orientation::Vertical);
            obj.set_spacing(4);
            obj.set_halign(gtk::Align::Center);
            obj.set_width_request(WIDTH);
            self.badge.add_css_class("media-badge");
            self.badge.append(&self.badge_icon);
            self.badge.append(&self.badge_label);
            let overlay = gtk::Overlay::builder().child(&self.cover).margin_bottom(6).build();
            overlay.add_overlay(&self.badge);
            obj.append(&overlay);
            obj.append(&self.title);
            obj.append(&self.subtitle);
            obj.append(&self.excerpt);
        }
    }

    impl WidgetImpl for MediaCard {}
    impl BoxImpl for MediaCard {}
}

glib::wrapper! {
    /// A shot, a recording or a journal entry in a grid: its picture, then what it is.
    pub struct MediaCard(ObjectSubclass<imp::MediaCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for MediaCard {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl MediaCard {
    /// `alone`: the grid shows one game's media, so the card names the moment rather than the game.
    pub fn bind(&self, item: &MediaItem, alone: bool) {
        self.unbind();
        let imp = self.imp();
        let row = item.row().clone();
        imp.cover.set_placeholder(match item.kind() {
            Kind::Shot => "camera-photo-symbolic",
            Kind::Recording => "camera-video-symbolic",
            Kind::Journal => "text-x-generic-symbolic",
        });
        imp.cover.set_path(item.picture());
        let cover = imp.cover.clone();
        let handler = item.connect_picture_notify(move |item| cover.set_path(item.picture()));
        imp.item.replace(Some((item.clone(), handler)));
        let (title, subtitle) = match (item.kind(), alone) {
            (Kind::Journal, true) => (if row.heading.is_empty() { gettext("Untitled") } else { row.heading.clone() }, media::moment(&row.date)),
            (Kind::Journal, false) => {
                let heading = if row.heading.is_empty() { gettext("Untitled") } else { row.heading.clone() };
                (heading, format!("{} · {}", row.title, media::day(&row.date)))
            }
            (_, true) => (media::moment(&row.date), String::new()),
            (_, false) => (row.title.clone(), media::day(&row.date)),
        };
        imp.subtitle.set_visible(!subtitle.is_empty());
        imp.title.set_label(&title);
        imp.subtitle.set_label(&subtitle);
        imp.excerpt.set_label(&row.excerpt);
        imp.excerpt.set_visible(item.kind() == Kind::Journal && !row.excerpt.is_empty());
        let (icon, label) = match item.kind() {
            Kind::Shot => ("", String::new()),
            Kind::Recording => ("media-playback-start-symbolic", format::clock(row.duration_s as i64)),
            Kind::Journal => ("text-x-generic-symbolic", String::new()),
        };
        imp.badge.set_visible(!icon.is_empty());
        imp.badge_icon.set_icon_name(Some(icon));
        imp.badge_label.set_label(&label);
        imp.badge_label.set_visible(!label.is_empty());
    }

    pub fn unbind(&self) {
        if let Some((item, handler)) = self.imp().item.take() {
            item.disconnect(handler);
        }
    }
}
