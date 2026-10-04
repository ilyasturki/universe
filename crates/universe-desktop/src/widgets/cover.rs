use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{glib, graphene};

use crate::covers;

mod imp {
    use super::*;

    #[derive(Debug, glib::Properties)]
    #[properties(wrapper_type = super::Cover)]
    pub struct Cover {
        #[property(get, set = Self::set_path)]
        pub path: RefCell<String>,
        #[property(get, set, construct, default = 200)]
        pub art_width: Cell<i32>,
        #[property(get, set, construct, default = 300)]
        pub art_height: Cell<i32>,
        pub frame: gtk::Stack,
        pub picture: gtk::Picture,
        pub placeholder: gtk::Image,
        /// Bumped on every path: a decode that lands after another path was set is dropped.
        pub generation: Cell<u64>,
    }

    impl Default for Cover {
        fn default() -> Self {
            Cover {
                path: RefCell::default(),
                art_width: Cell::new(200),
                art_height: Cell::new(300),
                frame: gtk::Stack::builder().transition_type(gtk::StackTransitionType::Crossfade).build(),
                picture: gtk::Picture::builder().content_fit(gtk::ContentFit::Cover).can_shrink(true).build(),
                placeholder: gtk::Image::builder().icon_name("input-gaming-symbolic").css_classes(["dimmed"]).build(),
                generation: Cell::new(0),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Cover {
        const NAME: &'static str = "UniverseCover";
        type Type = super::Cover;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("cover");
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for Cover {
        fn constructed(&self) {
            self.parent_constructed();
            self.fit_placeholder();
            self.frame.add_named(&self.placeholder, Some("placeholder"));
            self.frame.add_named(&self.picture, Some("art"));
            self.frame.set_visible_child_name("placeholder");
            self.frame.add_css_class("card");
            self.frame.set_overflow(gtk::Overflow::Hidden);
            self.frame.set_parent(&*self.obj());
            self.obj().connect_scale_factor_notify(|cover| cover.imp().load());
        }

        fn dispose(&self) {
            self.frame.unparent();
        }
    }

    impl WidgetImpl for Cover {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let size = if orientation == gtk::Orientation::Horizontal { self.art_width.get() } else { self.art_height.get() };
            (size, size, -1, -1)
        }

        fn size_allocate(&self, width: i32, _height: i32, baseline: i32) {
            let (w, h) = (self.art_width.get(), self.art_height.get());
            self.frame.measure(gtk::Orientation::Horizontal, -1);
            let at = gtk::gsk::Transform::new().translate(&graphene::Point::new(((width - w) / 2) as f32, 0.0));
            self.frame.allocate(w, h, baseline, Some(at));
        }
    }

    impl Cover {
        pub(super) fn fit_placeholder(&self) {
            self.placeholder.set_pixel_size(self.art_width.get().min(self.art_height.get()) * 9 / 25);
        }

        fn set_path(&self, path: String) {
            if *self.path.borrow() == path {
                return;
            }
            self.path.replace(path);
            self.load();
        }

        pub(super) fn load(&self) {
            let path = self.path.borrow().clone();
            let generation = self.generation.get() + 1;
            self.generation.set(generation);
            if path.is_empty() {
                self.show(None);
                return;
            }
            let scale = self.obj().scale_factor().max(1) as u32;
            let (w, h) = (self.art_width.get() as u32 * scale, self.art_height.get() as u32 * scale);
            if let Some(texture) = covers::ready(&path, w, h) {
                self.show(Some(texture));
                return;
            }
            let obj = self.obj().downgrade();
            glib::spawn_future_local(async move {
                let texture = covers::texture(&path, w, h).await;
                if let Some(cover) = obj.upgrade().filter(|c| c.imp().generation.get() == generation) {
                    cover.imp().show(texture);
                }
            });
        }

        fn show(&self, texture: Option<gtk::gdk::Texture>) {
            match texture {
                Some(t) => {
                    self.picture.set_paintable(Some(&t));
                    self.frame.set_visible_child_name("art");
                }
                None => {
                    self.picture.set_paintable(gtk::gdk::Paintable::NONE);
                    self.frame.set_visible_child_name("placeholder");
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct Cover(ObjectSubclass<imp::Cover>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Cover {
    pub fn new(width: i32, height: i32) -> Self {
        glib::Object::builder().property("art-width", width).property("art-height", height).build()
    }

    pub fn set_placeholder(&self, icon: &str) {
        self.imp().placeholder.set_icon_name(Some(icon));
    }

    pub fn set_size(&self, width: i32, height: i32) {
        if (self.art_width(), self.art_height()) == (width, height) {
            return;
        }
        self.set_art_width(width);
        self.set_art_height(height);
        self.imp().fit_placeholder();
        self.queue_resize();
        self.imp().load();
    }

    /// Decodes the file again: the art under the same path was replaced.
    pub fn reload(&self) {
        covers::forget(&self.path());
        self.imp().load();
    }
}
