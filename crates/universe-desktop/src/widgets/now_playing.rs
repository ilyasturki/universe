use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use universe::session::Current;

use crate::backend;
use crate::format;
use crate::widgets::Cover;

/// What the popover's switches read when it opens: the running game's own keys, as the launch took them.
#[derive(Debug, Clone, Default)]
struct Controls {
    hud: bool,
    fps: String,
    fps_choices: Vec<String>,
    recording: bool,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/ilyasturki/UniverseDesktop/ui/now-playing.ui")]
    pub struct NowPlaying {
        #[template_child]
        pub button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        pub thumb: TemplateChild<Cover>,
        #[template_child]
        pub title: TemplateChild<gtk::Label>,
        #[template_child]
        pub rec: TemplateChild<gtk::Label>,
        #[template_child]
        pub clock: TemplateChild<gtk::Label>,
        #[template_child]
        pub popover: TemplateChild<gtk::Popover>,
        #[template_child]
        pub popover_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub hud: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub fps: TemplateChild<adw::ComboRow>,
        pub session: RefCell<Option<Current>>,
        pub started: Cell<i64>,
        pub ticker: RefCell<Option<glib::SourceId>>,
        pub fps_values: RefCell<Vec<String>>,
        pub syncing: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NowPlaying {
        const NAME: &'static str = "UniverseNowPlaying";
        type Type = super::NowPlaying;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Cover::ensure_type();
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NowPlaying {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_visible(false);
            let this = obj.downgrade();
            self.popover.connect_show(move |_| {
                this.upgrade().inspect(|n| n.load_controls());
            });
            let this = obj.downgrade();
            self.hud.connect_active_notify(move |row| {
                let Some(n) = this.upgrade().filter(|n| !n.imp().syncing.get()) else { return };
                n.set_hud(row.is_active());
            });
            let this = obj.downgrade();
            self.fps.connect_selected_notify(move |row| {
                let Some(n) = this.upgrade().filter(|n| !n.imp().syncing.get()) else { return };
                let value = n.imp().fps_values.borrow().get(row.selected() as usize).cloned();
                if let Some(value) = value {
                    n.set_fps(value);
                }
            });
        }

        fn dispose(&self) {
            if let Some(ticker) = self.ticker.take() {
                ticker.remove();
            }
        }
    }

    impl WidgetImpl for NowPlaying {}
    impl BinImpl for NowPlaying {}
}

glib::wrapper! {
    /// The sidebar's foot while a game runs: its cover, title and clock; a popover to show it, shoot it, tune it or quit it.
    pub struct NowPlaying(ObjectSubclass<imp::NowPlaying>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn fps_label(value: &str) -> String {
    match value {
        "" => gettext("Game Default"),
        "auto" => gettext("Match the Display"),
        "none" => gettext("No Limit"),
        n => gettext("{} FPS").replace("{}", n),
    }
}

impl NowPlaying {
    pub fn set_session(&self, session: Option<&Current>, cover: &str) {
        let imp = self.imp();
        imp.session.replace(session.cloned());
        if let Some(ticker) = imp.ticker.take() {
            ticker.remove();
        }
        let Some(session) = session else {
            self.set_visible(false);
            imp.popover.popdown();
            return;
        };
        self.set_visible(true);
        imp.title.set_label(&session.title);
        imp.popover_title.set_label(&session.title);
        imp.thumb.set_path(cover.to_string());
        imp.rec.set_visible(false);
        imp.started.set(chrono::DateTime::parse_from_rfc3339(&session.started_at).map(|t| t.timestamp()).unwrap_or_else(|_| chrono::Local::now().timestamp()));
        self.tick();
        let this = self.downgrade();
        imp.ticker.replace(Some(glib::timeout_add_seconds_local(1, move || match this.upgrade() {
            Some(n) => {
                n.tick();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        })));
        self.load_controls();
    }

    fn tick(&self) {
        let imp = self.imp();
        imp.clock.set_label(&format::clock(chrono::Local::now().timestamp() - imp.started.get()));
    }

    fn load_controls(&self) {
        let Some(session) = self.imp().session.borrow().clone() else { return };
        let this = self.downgrade();
        glib::spawn_future_local(async move {
            let (id, screen) = (session.id.clone(), session.screen.clone());
            let controls = backend::run(async move {
                let core = backend::core();
                let r = core.get(&id).await.ok()?;
                let mode = universe::desktop::screen_mode(&screen).await;
                let fps_choices = core.launch_keys("game", mode).ok()?.into_iter().find(|k| k.key == "fps_limit").map(|k| k.choices).unwrap_or_default();
                let capture = r.effective.modules.get("capture");
                Some(Controls {
                    hud: r.effective.mangohud,
                    fps: r.game.launch.fps_limit.clone(),
                    fps_choices,
                    recording: capture.is_some_and(|m| m.get("enabled").and_then(|v| v.as_bool()) != Some(false)),
                })
            })
            .await;
            let (Some(n), Some(c)) = (this.upgrade(), controls) else { return };
            n.show_controls(c);
        });
    }

    fn show_controls(&self, c: Controls) {
        let imp = self.imp();
        imp.syncing.set(true);
        imp.hud.set_active(c.hud);
        let mut values = vec![String::new()];
        values.extend(c.fps_choices.iter().cloned());
        if !values.contains(&c.fps) {
            values.push(c.fps.clone());
        }
        let labels: Vec<String> = values.iter().map(|v| fps_label(v)).collect();
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        imp.fps.set_model(Some(&gtk::StringList::new(&labels)));
        imp.fps.set_selected(values.iter().position(|v| *v == c.fps).unwrap_or(0) as u32);
        imp.fps_values.replace(values);
        imp.rec.set_visible(c.recording);
        imp.syncing.set(false);
    }

    fn toast(&self, text: &str) {
        if let Some(win) = self.root().and_downcast::<crate::window::Window>() {
            win.toast(adw::Toast::new(text));
        }
    }

    fn set_hud(&self, on: bool) {
        let this = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.set_mangohud(Some(on)).await }).await;
            let Some(n) = this.upgrade() else { return };
            if let Err(e) = result {
                n.toast(&e.to_string());
                n.load_controls();
            }
        });
    }

    fn set_fps(&self, value: String) {
        let Some(id) = self.imp().session.borrow().as_ref().map(|s| s.id.clone()) else { return };
        let this = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move {
                core.set(&id, "launch.fps_limit", &value).await?;
                core.set_fps_limit().await
            })
            .await;
            let Some(n) = this.upgrade() else { return };
            if let Err(e) = result {
                n.toast(&e.to_string());
                n.load_controls();
            }
        });
    }
}
