use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::actions;
use crate::backend;
use crate::covers;
use crate::format;
use crate::game::GameObject;
use crate::library;
use crate::widgets::Cover;

/// What the page shows past the library row, read once it opens.
#[derive(Debug, Clone, Default)]
pub struct Details {
    description: String,
    developers: Vec<String>,
    publishers: Vec<String>,
    genres: Vec<String>,
    screenshots: Vec<String>,
    runner_name: String,
    achievements: (usize, usize),
}

impl Details {
    fn of(r: &universe::library::Resolved) -> Details {
        let m = &r.game.metadata;
        Details {
            description: if m.description.trim().is_empty() { m.summary.trim().to_string() } else { m.description.trim().to_string() },
            developers: m.developers.clone(),
            publishers: m.publishers.clone(),
            genres: m.genres.clone(),
            screenshots: r.screenshots.clone(),
            runner_name: r.effective.runner_name.clone(),
            achievements: (r.achievements.unlocked, r.achievements.total),
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/game-page.ui")]
    pub struct GamePage {
        #[template_child]
        pub backdrop: TemplateChild<gtk::Picture>,
        #[template_child]
        pub cover: TemplateChild<Cover>,
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        #[template_child]
        pub byline: TemplateChild<gtk::Label>,
        #[template_child]
        pub facts: TemplateChild<gtk::Label>,
        #[template_child]
        pub play: TemplateChild<gtk::Button>,
        #[template_child]
        pub play_content: TemplateChild<adw::ButtonContent>,
        #[template_child]
        pub stop: TemplateChild<gtk::Button>,
        #[template_child]
        pub about: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub description: TemplateChild<gtk::Label>,
        #[template_child]
        pub shots_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub shots: TemplateChild<gtk::Box>,
        pub game: RefCell<Option<GameObject>>,
        pub details: RefCell<Details>,
        pub handlers: RefCell<Vec<glib::SignalHandlerId>>,
        pub actions: RefCell<Option<gio::SimpleActionGroup>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GamePage {
        const NAME: &'static str = "UniverseGamePage";
        type Type = super::GamePage;
        type ParentType = adw::NavigationPage;

        fn class_init(klass: &mut Self::Class) {
            Cover::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for GamePage {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            let weak = obj.downgrade();
            self.actions.replace(Some(actions::install_game(&*obj, move || weak.upgrade().and_then(|page| page.game()))));
        }

        fn dispose(&self) {
            self.obj().release();
        }
    }

    impl WidgetImpl for GamePage {}
    impl NavigationPageImpl for GamePage {}
}

glib::wrapper! {
    /// One game over its own art: the cover, what is known of it, Play and the rest of its actions.
    pub struct GamePage(ObjectSubclass<imp::GamePage>)
        @extends adw::NavigationPage, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl GamePage {
    pub fn new(game: &GameObject) -> Self {
        let page: Self = glib::Object::new();
        page.show(game);
        page
    }

    pub fn game(&self) -> Option<GameObject> {
        self.imp().game.borrow().clone()
    }

    fn show(&self, game: &GameObject) {
        let imp = self.imp();
        imp.game.replace(Some(game.clone()));
        if let Some(group) = imp.actions.borrow().as_ref() {
            imp.handlers.replace(actions::follow_game(group, game));
        }
        let page = self.downgrade();
        let refresh = move |_: &GameObject, _: &glib::ParamSpec| {
            page.upgrade().inspect(|page| page.refresh());
        };
        let mut handlers = imp.handlers.take();
        for prop in ["title", "cover", "playing", "launching", "installed", "favorite"] {
            handlers.push(game.connect_notify_local(Some(prop), refresh.clone()));
        }
        imp.handlers.replace(handlers);
        self.refresh();
        self.load_details();
    }

    fn release(&self) {
        let imp = self.imp();
        if let Some(game) = imp.game.borrow().as_ref() {
            for handler in imp.handlers.take() {
                game.disconnect(handler);
            }
        }
    }

    /// The row and the details again: the game's files moved.
    pub fn reload(&self) {
        self.refresh();
        self.load_details();
    }

    fn refresh(&self) {
        let imp = self.imp();
        let Some(game) = self.game() else { return };
        let row = game.row().clone();
        self.set_title(&row.title);
        imp.title.set_label(&row.title);
        imp.cover.set_path(row.cover.clone());
        let details = imp.details.borrow().clone();
        let mut byline: Vec<String> = Vec::new();
        byline.extend(details.developers.first().cloned().or_else(|| Some(row.developer.clone())).filter(|d| !d.is_empty()));
        if let Some(publisher) = details.publishers.first().filter(|p| !byline.contains(p)) {
            byline.push(publisher.clone());
        }
        if row.release_year > 0 {
            byline.push(row.release_year.to_string());
        }
        imp.byline.set_label(&byline.join(" · "));
        imp.byline.set_visible(!byline.is_empty());

        let mut facts = vec![library::platform_name(&row.platform)];
        if !details.runner_name.is_empty() && !facts.contains(&details.runner_name) {
            facts.push(details.runner_name.clone());
        }
        if game.playing() {
            facts.push(gettext("Playing now"));
        } else if row.last_played > 0 {
            facts.push(gettext("Last played {}").replace("{}", &format::relative(row.last_played, chrono::Local::now())));
        }
        facts.push(format::played(row.hours));
        if details.achievements.1 > 0 {
            facts.push(gettext("{} of {} achievements").replacen("{}", &details.achievements.0.to_string(), 1).replacen(
                "{}",
                &details.achievements.1.to_string(),
                1,
            ));
        }
        facts.retain(|f| !f.is_empty());
        imp.facts.set_label(&facts.join(" · "));

        let (label, icon) = if game.launching() {
            (gettext("Starting…"), "content-loading-symbolic")
        } else if !row.installed {
            (gettext("Not Installed"), "media-playback-start-symbolic")
        } else if row.hours > 0.0 {
            (gettext("Continue"), "media-playback-start-symbolic")
        } else {
            (gettext("Play"), "media-playback-start-symbolic")
        };
        imp.play_content.set_label(&label);
        imp.play_content.set_icon_name(icon);
        imp.play.set_visible(!game.playing());
        imp.stop.set_visible(game.playing());
    }

    fn load_details(&self) {
        let Some(game) = self.game() else { return };
        let (id, backdrop) = (game.id(), {
            let row = game.row();
            if row.background.is_empty() {
                row.cover.clone()
            } else {
                row.background.clone()
            }
        });
        let page = self.downgrade();
        glib::spawn_future_local(async move {
            let details = backend::run(async move { backend::core().get(&id).await.map(|r| Details::of(&r)) }).await;
            let texture = covers::texture(&backdrop, 64, 36).await;
            let Some(page) = page.upgrade() else { return };
            let imp = page.imp();
            imp.backdrop.set_paintable(texture.as_ref());
            if let Ok(details) = details {
                imp.details.replace(details);
                page.show_details();
                page.refresh();
            }
        });
    }

    fn show_details(&self) {
        let imp = self.imp();
        let details = imp.details.borrow().clone();
        imp.description.set_label(&details.description);
        imp.about.set_visible(!details.description.is_empty());
        if !details.genres.is_empty() {
            imp.about.set_description(Some(&details.genres.join(", ")));
        }
        while let Some(child) = imp.shots.first_child() {
            imp.shots.remove(&child);
        }
        for path in details.screenshots.iter().take(12) {
            let shot = Cover::new(320, 180);
            shot.set_path(path.clone());
            imp.shots.append(&shot);
        }
        imp.shots_group.set_visible(!details.screenshots.is_empty());
    }
}
