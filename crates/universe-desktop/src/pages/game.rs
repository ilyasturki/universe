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
use crate::media::Kind;
use crate::widgets::Cover;
use crate::window::Window;

type Opener = fn(&Window, &str);

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
    /// The game's source lists achievements for it, whether or not it has yet.
    achievable: bool,
    shots: usize,
    recordings: usize,
    journal: usize,
    sessions: usize,
}

impl Details {
    fn of(r: &universe::library::Resolved, shots: usize, achievable: bool) -> Details {
        let m = &r.game.metadata;
        Details {
            description: if m.description.trim().is_empty() { m.summary.trim().to_string() } else { m.description.trim().to_string() },
            developers: m.developers.clone(),
            publishers: m.publishers.clone(),
            genres: m.genres.clone(),
            screenshots: r.screenshots.clone(),
            runner_name: r.effective.runner_name.clone(),
            achievements: (r.achievements.unlocked, r.achievements.total),
            achievable,
            shots,
            recordings: r.sessions.iter().filter(|s| s.recording.as_deref().is_some_and(|p| !p.is_empty())).count(),
            journal: r.journal_count,
            sessions: r.sessions.iter().filter(|s| !s.unit.is_empty()).count(),
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
        #[template_child]
        pub play_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub achievements_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub achievements_count: TemplateChild<gtk::Label>,
        #[template_child]
        pub shots_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub shots_count: TemplateChild<gtk::Label>,
        #[template_child]
        pub recordings_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub recordings_count: TemplateChild<gtk::Label>,
        #[template_child]
        pub journal_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub journal_count: TemplateChild<gtk::Label>,
        #[template_child]
        pub sessions_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub sessions_count: TemplateChild<gtk::Label>,
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
            obj.setup_page_actions();
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

    /// The rows under "Your Play" open their pages over this one.
    fn setup_page_actions(&self) {
        let group = gio::SimpleActionGroup::new();
        let open: [(&str, Opener); 5] = [
            ("achievements", |win, id| crate::pages::achievements::open(win, id)),
            ("screenshots", |win, id| crate::pages::media::open_game(win, id, Kind::Shot)),
            ("recordings", |win, id| crate::pages::media::open_game(win, id, Kind::Recording)),
            ("journal", |win, id| crate::pages::journal::open_list(win, id)),
            ("sessions", |win, id| crate::pages::sessions::open(win, id)),
        ];
        for (name, act) in open {
            let action = gio::SimpleAction::new(name, None);
            let page = self.downgrade();
            action.connect_activate(move |_, _| {
                let Some(page) = page.upgrade() else { return };
                if let (Some(game), Some(win)) = (page.game(), page.root().and_downcast::<Window>()) {
                    act(&win, &game.id());
                }
            });
            group.add_action(&action);
        }
        self.insert_action_group("page", Some(&group));
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
            let details = backend::pinned(move |core| async move {
                let r = core.get(&id).await?;
                let shots = core.screenshots(&id).await.map(|s| s.len()).unwrap_or(0);
                let listed =
                    !r.game.source.gog_id.is_empty()
                        && core.sources().await.iter().any(|s| {
                            s["id"] == r.game.source.kind.as_str() && s["capabilities"].as_array().is_some_and(|c| c.iter().any(|c| c == "achievements"))
                        });
                Ok::<_, universe::Error>(Details::of(&r, shots, listed))
            })
            .await;
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

        let count = |n: usize| n.to_string();
        let (unlocked, total) = details.achievements;
        let rows: [(&adw::ActionRow, &gtk::Label, bool, String); 5] = [
            (
                &imp.achievements_row,
                &imp.achievements_count,
                total > 0 || details.achievable,
                if total > 0 { gettext("{} of {}").replacen("{}", &unlocked.to_string(), 1).replacen("{}", &total.to_string(), 1) } else { String::new() },
            ),
            (&imp.shots_row, &imp.shots_count, details.shots > 0, count(details.shots)),
            (&imp.recordings_row, &imp.recordings_count, details.recordings > 0, count(details.recordings)),
            (&imp.journal_row, &imp.journal_count, details.journal > 0 || details.sessions > 0, count(details.journal)),
            (&imp.sessions_row, &imp.sessions_count, details.sessions > 0, count(details.sessions)),
        ];
        let mut any = false;
        for (row, label, shown, text) in rows {
            row.set_visible(shown);
            label.set_label(&text);
            any |= shown;
        }
        imp.play_group.set_visible(any);
    }
}
