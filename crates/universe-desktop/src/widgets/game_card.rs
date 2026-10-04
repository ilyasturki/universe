use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::actions;
use crate::game::GameObject;
use crate::widgets::Cover;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/game-card.ui")]
    pub struct GameCard {
        #[template_child]
        pub motion: TemplateChild<gtk::EventControllerMotion>,
        #[template_child]
        pub cover: TemplateChild<Cover>,
        #[template_child]
        pub options: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub play: TemplateChild<gtk::Button>,
        #[template_child]
        pub playing: TemplateChild<gtk::Label>,
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        pub game: RefCell<Option<GameObject>>,
        pub bindings: RefCell<Vec<glib::Binding>>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub actions: RefCell<Option<gio::SimpleActionGroup>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GameCard {
        const NAME: &'static str = "UniverseGameCard";
        type Type = super::GameCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            Cover::ensure_type();
            klass.bind_template();
            klass.set_css_name("game-card");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for GameCard {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let weak = obj.downgrade();
            let group = actions::game_menu(&*obj, move || weak.upgrade().and_then(|card| card.game()));
            actions::sync_game(&group, None);
            self.actions.replace(Some(group));
            let card = obj.downgrade();
            self.motion.connect_contains_pointer_notify(move |_| {
                card.upgrade().inspect(|card| card.reveal());
            });
            let card = obj.downgrade();
            self.options.connect_active_notify(move |_| {
                card.upgrade().inspect(|card| card.reveal());
            });
            self.options.set_menu_model(Some(&crate::menus::game()));
        }
    }

    impl WidgetImpl for GameCard {}
    impl BoxImpl for GameCard {}
}

glib::wrapper! {
    /// A grid cell: the cover with Play and a menu over it on hover, the title under it.
    pub struct GameCard(ObjectSubclass<imp::GameCard>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Default for GameCard {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl GameCard {
    pub fn game(&self) -> Option<GameObject> {
        self.imp().game.borrow().clone()
    }

    pub fn bind(&self, game: &GameObject) {
        self.unbind();
        let imp = self.imp();
        imp.game.replace(Some(game.clone()));
        let bindings = vec![
            game.bind_property("cover", &*imp.cover, "path").sync_create().build(),
            game.bind_property("title", &*imp.title, "label").sync_create().build(),
            game.bind_property("title", &*imp.title, "tooltip-text").sync_create().build(),
            game.bind_property("playing", &*imp.playing, "visible").sync_create().build(),
        ];
        imp.bindings.replace(bindings);
        if let Some(group) = imp.actions.borrow().as_ref() {
            imp.handlers.replace(actions::follow_game(group, game));
        }
        let cover = imp.cover.downgrade();
        let art = game.connect_art_changed(move |_| {
            cover.upgrade().inspect(|cover| cover.reload());
        });
        imp.handlers.borrow_mut().push(art);
        self.update_property(&[gtk::accessible::Property::Label(&game.title())]);
    }

    pub fn unbind(&self) {
        let imp = self.imp();
        for binding in imp.bindings.take() {
            binding.unbind();
        }
        if let Some(game) = imp.game.take() {
            for handler in imp.handlers.take() {
                game.disconnect(handler);
            }
        }
        if let Some(group) = imp.actions.borrow().as_ref() {
            actions::sync_game(group, None);
        }
    }

    pub fn set_compact(&self, compact: bool) {
        let (width, height) = cover_size(compact);
        self.imp().cover.set_size(width, height);
        self.imp().title.set_max_width_chars(if compact { 14 } else { 20 });
    }

    fn reveal(&self) {
        let imp = self.imp();
        let shown = imp.motion.contains_pointer() || imp.options.is_active();
        for widget in [imp.options.upcast_ref::<gtk::Widget>(), imp.play.upcast_ref()] {
            if shown {
                widget.remove_css_class("hidden");
            } else {
                widget.add_css_class("hidden");
            }
        }
    }
}

pub fn cover_size(compact: bool) -> (i32, i32) {
    if compact {
        (140, 210)
    } else {
        (200, 300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_cards_fit_two_columns_on_a_phone() {
        let (width, height) = cover_size(true);
        assert_eq!(width * 3, height * 2, "box art keeps its shape");
        // The grid's 18 px sides and each child's 6 px: two columns on a 360 px screen.
        assert!(2 * (width + 12) + 36 <= 360);
    }
}
