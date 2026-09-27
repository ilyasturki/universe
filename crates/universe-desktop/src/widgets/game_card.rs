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
            let group = actions::install_game(&*obj, move || weak.upgrade().and_then(|card| card.game()));
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
