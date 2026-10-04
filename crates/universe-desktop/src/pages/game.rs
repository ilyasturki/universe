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

/// What the page shows past the library row, read once it opens: the library's at once, the screenshots' count and whether
/// the source lists achievements after.
#[derive(Debug, Clone, Default)]
pub struct Details {
    description: String,
    developers: Vec<String>,
    publishers: Vec<String>,
    genres: Vec<String>,
    screenshots: Vec<String>,
    achievements: (usize, usize),
    /// The game's source lists achievements for it, whether or not it has yet.
    achievable: bool,
    shots: usize,
    recordings: usize,
    journal: usize,
    sessions: usize,
}

impl Details {
    /// `known`: what the page had, kept for the parts read after.
    fn of(r: &universe::library::Resolved, known: &Details) -> Details {
        let m = &r.game.metadata;
        Details {
            description: if m.description.trim().is_empty() { m.summary.trim().to_string() } else { m.description.trim().to_string() },
            developers: m.developers.clone(),
            publishers: m.publishers.clone(),
            genres: m.genres.clone(),
            screenshots: r.screenshots.clone(),
            achievements: (r.achievements.unlocked, r.achievements.total),
            achievable: known.achievable,
            shots: known.shots,
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
        pub header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub scroller: TemplateChild<gtk::ScrolledWindow>,
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
        #[template_child]
        pub data_group: TemplateChild<adw::PreferencesGroup>,
        #[template_child]
        pub data_list: TemplateChild<gtk::ListBox>,
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
            obj.follow_scroll();
            let shortcuts = gtk::ShortcutController::new();
            shortcuts.add_shortcut(gtk::Shortcut::new(
                Some(gtk::KeyvalTrigger::new(gtk::gdk::Key::Delete, gtk::gdk::ModifierType::empty())),
                Some(gtk::NamedAction::new("game.remove")),
            ));
            obj.add_controller(shortcuts);
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
        let page = self.downgrade();
        handlers.push(game.connect_art_changed(move |_| {
            let Some(page) = page.upgrade() else { return };
            page.imp().cover.reload();
            page.load_details();
        }));
        imp.handlers.replace(handlers);
        self.refresh();
        self.load_details();
        self.load_data();
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
        self.load_data();
    }

    // A task of its own: sizes walk the folders and the saves ask ludusavi, which the details must not wait on.
    fn load_data(&self) {
        let Some(game) = self.game() else { return };
        let id = game.id();
        let page = self.downgrade();
        glib::spawn_future_local(async move {
            let data = backend::call(move |core| async move { core.game_data(&id).await }).await;
            let Some(page) = page.upgrade() else { return };
            let imp = page.imp();
            let Ok(data) = data else {
                imp.data_group.set_visible(false);
                return;
            };
            let again = page.downgrade();
            crate::pages::game_data::fill(
                &imp.data_list,
                &data,
                std::rc::Rc::new(move || {
                    if let Some(page) = again.upgrade() {
                        page.load_data();
                    }
                }),
            );
            imp.data_group.set_description(Some(&crate::pages::game_data::size(&data["total"])));
            imp.data_group.set_visible(true);
        });
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

        imp.facts.set_label(&facts(&row, details.achievements, game.playing(), chrono::Local::now()).join(" · "));

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

    /// Each part shows the moment it lands: the library's details and the screenshots' count, then whether the source lists
    /// achievements (`sources()` may ask the stores), and apart from both the backdrop, faded in once decoded.
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
            let texture = covers::texture(&backdrop, 64, 36).await;
            let Some(page) = page.upgrade() else { return };
            let picture = &page.imp().backdrop;
            let fresh = picture.paintable().is_none();
            picture.set_paintable(texture.as_ref());
            if fresh && texture.is_some() {
                picture.set_opacity(0.0);
                adw::TimedAnimation::new(&**picture, 0.0, 1.0, 400, adw::PropertyAnimationTarget::new(&**picture, "opacity")).play();
            }
        });
        let page = self.downgrade();
        glib::spawn_future_local(async move {
            let resolved = backend::pinned(move |core| async move {
                let r = core.get(&id).await?;
                let shots = core.screenshots(&id).await.map(|s| s.len()).unwrap_or(0);
                Ok::<_, universe::Error>((r, shots))
            })
            .await;
            let Ok((r, shots)) = resolved else { return };
            let Some(this) = page.upgrade() else { return };
            let mut known = this.imp().details.borrow().clone();
            known.shots = shots;
            this.imp().details.replace(Details::of(&r, &known));
            this.show_details();
            this.refresh();
            let source = (r.game.source.id.clone(), r.game.source.kind.clone());
            let achievable = backend::pinned(move |core| async move {
                !source.0.is_empty()
                    && core
                        .sources()
                        .await
                        .iter()
                        .any(|s| s["id"] == source.1.as_str() && s["capabilities"].as_array().is_some_and(|c| c.iter().any(|c| c == "achievements")))
            })
            .await;
            let Some(this) = page.upgrade() else { return };
            this.imp().details.borrow_mut().achievable = achievable;
            this.show_details();
        });
    }

    /// The header names the game once its title has scrolled away under it.
    fn follow_scroll(&self) {
        let imp = self.imp();
        let page = self.downgrade();
        imp.scroller.vadjustment().connect_value_changed(move |adjustment| {
            let Some(page) = page.upgrade() else { return };
            let imp = page.imp();
            let Some(content) = imp.scroller.child().and_downcast::<gtk::Viewport>().and_then(|v| v.child()) else { return };
            let bottom = imp.title.compute_point(&content, &gtk::graphene::Point::new(0.0, imp.title.height() as f32)).map_or(0.0, |p| p.y());
            imp.header.set_show_title(adjustment.value() > f64::from(bottom));
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

/// The line under the byline: the machine, the runner, when last played, the hours, then the achievements once read.
fn facts(row: &crate::game::Row, achievements: (usize, usize), playing: bool, now: chrono::DateTime<chrono::Local>) -> Vec<String> {
    let mut facts = vec![library::platform_name(&row.platform)];
    if !facts.contains(&row.runner_name) {
        facts.push(row.runner_name.clone());
    }
    if playing {
        facts.push(gettext("Playing now"));
    } else if row.last_played > 0 {
        facts.push(gettext("Last played {}").replace("{}", &format::relative(row.last_played, now)));
    }
    facts.push(format::played(row.hours));
    if achievements.1 > 0 {
        facts.push(gettext("{} of {} achievements").replacen("{}", &achievements.0.to_string(), 1).replacen("{}", &achievements.1.to_string(), 1));
    }
    facts.retain(|f| !f.is_empty());
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_facts_name_the_runner_before_the_details_land() {
        let row = crate::game::Row { platform: "windows".into(), runner_name: "Proton".into(), hours: 2.0, ..Default::default() };
        let now = chrono::Local::now();
        let before = facts(&row, (0, 0), false, now);
        assert_eq!(before[..2], ["Windows", "Proton"], "the library's row has the runner: nothing waits on the details");
        let after = facts(&row, (3, 40), false, now);
        assert_eq!(after[..before.len()], before[..], "the details only add to the end of the line");
        assert_eq!(after.len(), before.len() + 1);
    }
}
