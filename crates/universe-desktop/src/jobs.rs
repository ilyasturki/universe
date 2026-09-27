use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use crate::backend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Install,
    Update,
    Scan,
    Artwork,
}

/// What a long job ends with: its line for the player, whether it went through, the game an install brought in.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub ok: bool,
    pub text: String,
    pub game: String,
}

enum Note {
    /// The game in hand, by title, and its place in the run from 1.
    Target(String, u32),
    Progress(u64, u64, String),
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties)]
    #[properties(wrapper_type = super::Job)]
    pub struct Job {
        /// The game in hand; empty for a scan or the whole library's art.
        #[property(get)]
        pub title: RefCell<String>,
        #[property(get)]
        pub label: RefCell<String>,
        /// The source's own line: `42%`, the game being fetched.
        #[property(get)]
        pub message: RefCell<String>,
        #[property(get)]
        pub done: Cell<u64>,
        #[property(get)]
        pub total: Cell<u64>,
        #[property(get)]
        pub step: Cell<u32>,
        #[property(get)]
        pub steps: Cell<u32>,
        #[property(get)]
        pub running: Cell<bool>,
        #[property(get)]
        pub cancelled: Cell<bool>,
        pub kind: Cell<Kind>,
        pub source: RefCell<String>,
        /// The store id being worked on, which `cancel` stops: an update of several games moves through them.
        pub current: Arc<Mutex<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Job {
        const NAME: &'static str = "UniverseJob";
        type Type = super::Job;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Job {}
}

glib::wrapper! {
    /// An install, an update, a scan or an art refresh running in this process: it dies with it, an install then paused.
    pub struct Job(ObjectSubclass<imp::Job>);
}

impl Job {
    fn new(kind: Kind, source: &str, targets: &[(String, String)]) -> Job {
        let job: Job = glib::Object::new();
        let imp = job.imp();
        let first = targets.first().map(|(_, title)| title.clone()).unwrap_or_default();
        let n = targets.len() as u32;
        let label = match kind {
            Kind::Install => gettext("Installing {}").replace("{}", &first),
            Kind::Update if n == 1 => gettext("Updating {}").replace("{}", &first),
            Kind::Update => ngettext("Updating {} game", "Updating {} games", n).replace("{}", &n.to_string()),
            Kind::Scan => gettext("Looking for installed games"),
            Kind::Artwork if n == 1 => gettext("Fetching art for {}").replace("{}", &first),
            Kind::Artwork => gettext("Fetching artwork"),
        };
        imp.kind.set(kind);
        imp.source.replace(source.into());
        imp.title.replace(first);
        imp.label.replace(label);
        imp.steps.set(n.max(1));
        imp.step.set(1);
        imp.running.set(true);
        job
    }

    pub fn kind(&self) -> Kind {
        self.imp().kind.get()
    }

    pub fn source(&self) -> String {
        self.imp().source.borrow().clone()
    }

    /// The store id in hand.
    pub fn target(&self) -> String {
        self.imp().current.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn fraction(&self) -> f64 {
        let (done, total) = (self.done(), self.total());
        if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64).clamp(0.0, 1.0)
        }
    }

    fn note(&self, note: Note) {
        let imp = self.imp();
        match note {
            Note::Target(title, step) => {
                imp.title.replace(title);
                imp.step.set(step);
                imp.done.set(0);
                imp.total.set(0);
                imp.message.replace(String::new());
                for name in ["title", "step", "done", "total", "message"] {
                    self.notify(name);
                }
            }
            Note::Progress(done, total, message) => {
                imp.done.set(done);
                imp.total.set(total);
                self.notify_done();
                self.notify_total();
                if *imp.message.borrow() != message {
                    imp.message.replace(message);
                    self.notify_message();
                }
            }
        }
    }

    fn end(&self) {
        self.imp().running.set(false);
        self.notify_running();
    }

    /// A scan runs to its end; the others can be stopped.
    pub fn cancellable(&self) -> bool {
        self.kind() != Kind::Scan
    }

    /// Stopping pauses an install or an update, which resumes where it was.
    pub fn pauses(&self) -> bool {
        matches!(self.kind(), Kind::Install | Kind::Update)
    }

    /// An install or update is SIGTERMed and keeps what it downloaded; an art refresh ends after the game in hand.
    pub fn cancel(&self) {
        if !self.running() || self.cancelled() || !self.cancellable() {
            return;
        }
        self.imp().cancelled.set(true);
        self.notify_cancelled();
        let (kind, source, target) = (self.kind(), self.source(), self.target());
        glib::spawn_future_local(async move {
            backend::run(async move {
                let core = backend::core();
                if kind == Kind::Artwork {
                    core.media_cancel();
                } else {
                    core.source_cancel(&source, &target);
                }
            })
            .await;
        });
    }
}

/// Runs a job on the core over `targets`, `(store id, title)` pairs: one to install, one or more to update, the game whose
/// art to fetch (none: all of them), none for a scan. Progress arrives on the main loop as it comes, the outcome once it ends.
pub fn start(kind: Kind, source: &str, targets: Vec<(String, String)>, force: bool) -> (Job, impl std::future::Future<Output = Outcome>) {
    let job = Job::new(kind, source, &targets);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Note>();
    let pump = job.clone();
    glib::spawn_future_local(async move {
        while let Some(note) = rx.recv().await {
            pump.note(note);
        }
    });
    let (source, current, job_ref) = (source.to_string(), job.imp().current.clone(), job.clone());
    let outcome = async move {
        let result = backend::pinned(move |core| async move {
            let mut progress = |done: u64, total: u64, message: &str| {
                let _ = tx.send(Note::Progress(done, total, message.to_string()));
            };
            let first = targets.first().map(|(id, _)| id.clone()).unwrap_or_default();
            if let Ok(mut c) = current.lock() {
                c.clone_from(&first);
            }
            match kind {
                Kind::Install => core.source_install(&source, &first, Some(&mut progress)).await.map(|id| (1, id)),
                Kind::Update => {
                    for (step, (id, title)) in targets.iter().enumerate() {
                        if let Ok(mut c) = current.lock() {
                            c.clone_from(id);
                        }
                        let _ = tx.send(Note::Target(title.clone(), step as u32 + 1));
                        core.source_update(&source, id, Some(&mut progress)).await?;
                    }
                    Ok((targets.len(), String::new()))
                }
                Kind::Scan => core.source_scan(&source, Some(&mut progress)).await.map(|n| (n, String::new())),
                Kind::Artwork => core.media_refresh(&first, force, Some(&mut progress)).await.map(|(changed, _)| (changed, String::new())),
            }
        })
        .await;
        let cancelled = job_ref.cancelled();
        job_ref.end();
        let title = job_ref.title();
        let failed = |text: String| Outcome { ok: false, text, game: String::new() };
        match result {
            Ok((count, game)) => Outcome {
                ok: true,
                game,
                text: match (kind, count as u32, count.to_string()) {
                    (Kind::Install, ..) => gettext("{} is installed").replace("{}", &title),
                    (Kind::Update, 1, _) => gettext("{} is updated").replace("{}", &title),
                    (Kind::Update, n, text) => ngettext("{} game updated", "{} games updated", n).replace("{}", &text),
                    (Kind::Scan, 0, _) => gettext("No new installed games"),
                    (Kind::Scan, n, text) => ngettext("{} installed game found", "{} installed games found", n).replace("{}", &text),
                    (Kind::Artwork, 0, _) => gettext("No new art"),
                    (Kind::Artwork, n, text) => ngettext("New art for {} game", "New art for {} games", n).replace("{}", &text),
                },
            },
            Err(_) if cancelled && job_ref.pauses() => failed(gettext("{} is paused: it resumes from the Store").replace("{}", &title)),
            Err(_) if cancelled => failed(gettext("Stopped")),
            Err(e) => failed(e.to_string()),
        }
    };
    (job, outcome)
}
