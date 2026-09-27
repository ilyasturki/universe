use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};
use universe::timeline::MediaRow;

use crate::backend;
use crate::covers;
use crate::media;
use crate::window::Window;

// The picture decodes at most this big: a 4K screen at scale 1, or a 1080p one at 2.
const FULL: (u32, u32) = (3840, 2160);

/// One picture of the viewer. A shot of the player's can be deleted; a picture of a journal entry cannot from here.
#[derive(Debug, Clone, Default)]
pub struct Shot {
    pub path: String,
    pub thumb: String,
    pub game: String,
    pub title: String,
    pub date: String,
    pub deletable: bool,
}

impl Shot {
    pub fn of(row: &MediaRow) -> Shot {
        Shot {
            path: row.path.clone(),
            thumb: if row.thumb_ready { row.thumb.clone() } else { String::new() },
            game: row.game.clone(),
            title: row.title.clone(),
            date: row.date.clone(),
            deletable: row.kind == "shot",
        }
    }
}

struct Viewer {
    page: glib::WeakRef<adw::NavigationPage>,
    win: glib::WeakRef<Window>,
    picture: gtk::Picture,
    heading: adw::WindowTitle,
    previous: gtk::Button,
    next: gtk::Button,
    delete: gtk::Button,
    shots: RefCell<Vec<Shot>>,
    index: Cell<usize>,
    generation: Cell<u64>,
}

impl Viewer {
    fn current(&self) -> Option<Shot> {
        self.shots.borrow().get(self.index.get()).cloned()
    }

    fn show(self: &Rc<Self>, index: usize) {
        let count = self.shots.borrow().len();
        let Some(shot) = self.shots.borrow().get(index).cloned() else { return };
        self.index.set(index);
        self.heading.set_title(&shot.title);
        let when = media::moment(&shot.date);
        self.heading.set_subtitle(&if count > 1 { format!("{when} · {} / {count}", index + 1) } else { when });
        self.previous.set_visible(index > 0);
        self.next.set_visible(index + 1 < count);
        self.delete.set_visible(shot.deletable);
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        let this = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            if !shot.thumb.is_empty() {
                let quick = covers::texture(&shot.thumb, 960, 540).await;
                let Some(viewer) = this.upgrade().filter(|v| v.generation.get() == generation) else { return };
                if quick.is_some() {
                    viewer.picture.set_paintable(quick.as_ref());
                }
            }
            let full = covers::full(&shot.path, FULL.0, FULL.1).await;
            if let Some(viewer) = this.upgrade().filter(|v| v.generation.get() == generation) {
                viewer.picture.set_paintable(full.as_ref());
            }
        });
    }

    fn step(self: &Rc<Self>, by: isize) {
        let index = self.index.get() as isize + by;
        if index >= 0 && (index as usize) < self.shots.borrow().len() {
            self.show(index as usize);
        }
    }

    fn open_with(&self) {
        let (Some(shot), Some(win)) = (self.current(), self.win.upgrade()) else { return };
        gtk::FileLauncher::new(Some(&gio::File::for_path(&shot.path))).launch(Some(&win), gio::Cancellable::NONE, |_| {});
    }

    /// The shot goes at once, for good once the toast is gone.
    fn delete(self: &Rc<Self>) {
        let (Some(shot), Some(win)) = (self.current(), self.win.upgrade()) else { return };
        if !shot.deletable {
            return;
        }
        let name = std::path::Path::new(&shot.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let game = shot.game.clone();
        win.app().defer(&media::shot_key(&shot.path), &gettext("Screenshot deleted"), move || async move {
            if let Err(e) = backend::call(move |core| async move { core.remove_screenshot(&game, &name).await }).await {
                tracing::warn!("remove screenshot: {e}");
            }
        });
        let left = {
            let mut shots = self.shots.borrow_mut();
            shots.remove(self.index.get());
            shots.len()
        };
        if left == 0 {
            if let Some(page) = self.page.upgrade() {
                let _ = page.activate_action("navigation.pop", None);
            }
        } else {
            self.show(self.index.get().min(left - 1));
        }
    }
}

fn osd(icon: &str, tooltip: &str, halign: gtk::Align) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .halign(halign)
        .valign(gtk::Align::Center)
        .margin_start(18)
        .margin_end(18)
        .css_classes(["circular", "osd", "lightbox-step"])
        .build()
}

/// The pictures over a dark page, one at a time, the arrows and the keys to go through them.
pub fn open(win: &Window, shots: Vec<Shot>, index: usize) {
    if shots.is_empty() {
        return;
    }
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Contain).can_shrink(true).hexpand(true).vexpand(true).build();
    let heading = adw::WindowTitle::new("", "");
    let header = adw::HeaderBar::builder().title_widget(&heading).build();
    let delete = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text(gettext("Delete")).build();
    let open_with = gtk::Button::builder().icon_name("document-open-symbolic").tooltip_text(gettext("Open With…")).build();
    header.pack_end(&delete);
    header.pack_end(&open_with);
    let (previous, next) = (osd("go-previous-symbolic", &gettext("Previous"), gtk::Align::Start), osd("go-next-symbolic", &gettext("Next"), gtk::Align::End));
    let overlay = gtk::Overlay::builder().child(&picture).build();
    overlay.add_overlay(&previous);
    overlay.add_overlay(&next);
    let toolbar = adw::ToolbarView::builder().content(&overlay).extend_content_to_top_edge(true).css_classes(["lightbox"]).build();
    toolbar.add_top_bar(&header);
    let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("Screenshot")).tag("viewer").build();

    let viewer = Rc::new(Viewer {
        page: page.downgrade(),
        win: win.downgrade(),
        picture,
        heading,
        previous: previous.clone(),
        next: next.clone(),
        delete: delete.clone(),
        shots: RefCell::new(shots),
        index: Cell::new(0),
        generation: Cell::new(0),
    });
    let weak = Rc::downgrade(&viewer);
    previous.connect_clicked(move |_| {
        weak.upgrade().inspect(|v| v.step(-1));
    });
    let weak = Rc::downgrade(&viewer);
    next.connect_clicked(move |_| {
        weak.upgrade().inspect(|v| v.step(1));
    });
    let weak = Rc::downgrade(&viewer);
    delete.connect_clicked(move |_| {
        weak.upgrade().inspect(|v| v.delete());
    });
    let weak = Rc::downgrade(&viewer);
    open_with.connect_clicked(move |_| {
        weak.upgrade().inspect(|v| v.open_with());
    });
    let keys = gtk::ShortcutController::new();
    for (key, by) in [(gdk::Key::Left, -1), (gdk::Key::Right, 1)] {
        let weak = Rc::downgrade(&viewer);
        keys.add_shortcut(gtk::Shortcut::new(
            Some(gtk::KeyvalTrigger::new(key, gdk::ModifierType::empty())),
            Some(gtk::CallbackAction::new(move |_, _| {
                weak.upgrade().inspect(|v| v.step(by));
                glib::Propagation::Stop
            })),
        ));
    }
    let weak = Rc::downgrade(&viewer);
    keys.add_shortcut(gtk::Shortcut::new(
        Some(gtk::KeyvalTrigger::new(gdk::Key::Delete, gdk::ModifierType::empty())),
        Some(gtk::CallbackAction::new(move |_, _| {
            weak.upgrade().inspect(|v| v.delete());
            glib::Propagation::Stop
        })),
    ));
    keys.add_shortcut(gtk::Shortcut::new(
        Some(gtk::KeyvalTrigger::new(gdk::Key::Escape, gdk::ModifierType::empty())),
        Some(gtk::NamedAction::new("navigation.pop")),
    ));
    page.add_controller(keys);
    viewer.show(index.min(viewer.shots.borrow().len() - 1));
    let held = RefCell::new(Some(viewer));
    page.connect_destroy(move |_| {
        held.take();
    });
    win.push_page(&page);
}
