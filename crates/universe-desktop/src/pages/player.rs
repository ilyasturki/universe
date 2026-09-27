use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use universe::frames;
use universe::sessions::SessionRow;
use universe::timeline::MediaRow;

use crate::backend;
use crate::format;
use crate::media;
use crate::pages::journal;
use crate::widgets::Cover;
use crate::window::Window;

/// A recording as the player needs it, from the timeline or from a session.
#[derive(Debug, Clone, Default)]
pub struct Recording {
    pub path: String,
    pub duration_s: u64,
    pub game: String,
    pub title: String,
    pub session: String,
    pub date: String,
    pub has_journal: bool,
}

impl Recording {
    pub fn of_media(row: &MediaRow) -> Recording {
        Recording {
            path: row.path.clone(),
            duration_s: row.duration_s,
            game: row.game.clone(),
            title: row.title.clone(),
            session: row.session.clone(),
            date: row.date.clone(),
            has_journal: row.has_journal,
        }
    }

    pub fn of_session(row: &SessionRow) -> Option<Recording> {
        let recording = row.recording.as_ref().filter(|r| r.exists)?;
        Some(Recording {
            path: recording.path.clone(),
            duration_s: if recording.duration_s > 0 { recording.duration_s } else { row.session.duration_s },
            game: row.session.game.clone(),
            title: row.title.clone(),
            session: row.session.session.clone(),
            date: row.session.ended_at.clone(),
            has_journal: row.journal.is_some(),
        })
    }
}

struct Player {
    win: glib::WeakRef<Window>,
    page: glib::WeakRef<adw::NavigationPage>,
    video: gtk::Video,
    frames: Vec<Cover>,
    recording: Recording,
}

impl Player {
    fn seek(&self, index: usize) {
        let Some(stream) = self.video.media_stream() else { return };
        let at = frames::at(index, self.recording.duration_s as f64);
        stream.seek((at * 1_000_000.0) as i64);
        stream.play();
    }

    fn landed(&self, recording: &str, index: usize) {
        if recording == self.recording.path {
            if let Some(cover) = self.frames.get(index) {
                cover.set_path(frames::file(std::path::Path::new(recording), index).to_string_lossy().into_owned());
            }
        }
    }

    fn delete(&self) {
        let Some(win) = self.win.upgrade() else { return };
        let app = win.app();
        let Recording { game, session, path, .. } = self.recording.clone();
        let forget = app.clone();
        app.defer(&media::recording_key(&game, &session), &gettext("Recording deleted"), move || async move {
            match backend::call(move |core| async move { core.remove_recording(&game, &session).await }).await {
                Ok(()) => forget.forget_frames(&path),
                Err(e) => tracing::warn!("remove recording: {e}"),
            }
        });
        if let Some(page) = self.page.upgrade() {
            let _ = page.activate_action("navigation.pop", None);
        }
    }
}

/// The recording plays over the frames it was sampled at; a frame seeks there.
pub fn open(win: &Window, row: &MediaRow) {
    open_recording(win, Recording::of_media(row));
}

pub fn open_recording(win: &Window, recording: Recording) {
    let app = win.app();
    let video = gtk::Video::builder().autoplay(true).hexpand(true).vexpand(true).graphics_offload(gtk::GraphicsOffloadEnabled::Enabled).build();
    video.set_filename(Some(&recording.path));
    let strip = gtk::Box::builder().spacing(8).margin_start(12).margin_end(12).margin_top(12).margin_bottom(12).halign(gtk::Align::Center).build();
    app.select_frames(&recording.path, recording.duration_s);
    let mut covers = Vec::new();
    for index in 0..frames::COUNT {
        let cover = Cover::new(128, 72);
        cover.set_path(app.frame(&recording.path, index, recording.duration_s).unwrap_or_default());
        let button = gtk::Button::builder()
            .child(&cover)
            .tooltip_text(format::clock(frames::at(index, recording.duration_s as f64) as i64))
            .css_classes(["flat", "frame-button"])
            .build();
        strip.append(&button);
        covers.push(cover);
    }
    let scroll = gtk::ScrolledWindow::builder().vscrollbar_policy(gtk::PolicyType::Never).child(&strip).build();
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).build();
    content.append(&video);
    content.append(&scroll);

    let heading = adw::WindowTitle::new(
        &recording.title,
        &[media::moment(&recording.date), format::clock(recording.duration_s as i64)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "),
    );
    let header = adw::HeaderBar::builder().title_widget(&heading).build();
    let delete = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text(gettext("Delete")).build();
    let open_with = gtk::Button::builder().icon_name("document-open-symbolic").tooltip_text(gettext("Open With…")).build();
    header.pack_end(&delete);
    header.pack_end(&open_with);
    if recording.has_journal {
        let entry = gtk::Button::builder().icon_name("text-x-generic-symbolic").tooltip_text(gettext("Journal Entry")).build();
        let (weak, game, session) = (win.downgrade(), recording.game.clone(), recording.session.clone());
        entry.connect_clicked(move |_| {
            if let Some(win) = weak.upgrade() {
                journal::open_entry(&win, &game, &session);
            }
        });
        header.pack_end(&entry);
    }
    let toolbar = adw::ToolbarView::builder().content(&content).build();
    toolbar.add_top_bar(&header);
    let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("Recording")).tag("player").build();

    let path = recording.path.clone();
    let player = Rc::new(Player { win: win.downgrade(), page: page.downgrade(), video: video.clone(), frames: covers, recording });
    for (index, button) in (0..).zip(std::iter::successors(strip.first_child(), |w| w.next_sibling())) {
        let weak = Rc::downgrade(&player);
        if let Some(button) = button.downcast_ref::<gtk::Button>() {
            button.connect_clicked(move |_| {
                weak.upgrade().inspect(|p| p.seek(index));
            });
        }
    }
    let weak = Rc::downgrade(&player);
    let landed = app.connect_frame_landed(move |recording, index| {
        weak.upgrade().inspect(|p| p.landed(recording, index));
    });
    let weak = Rc::downgrade(&player);
    delete.connect_clicked(move |_| {
        weak.upgrade().inspect(|p| p.delete());
    });
    let weak = win.downgrade();
    open_with.connect_clicked(move |_| {
        let Some(win) = weak.upgrade() else { return };
        gtk::FileLauncher::new(Some(&gio::File::for_path(&path))).launch(Some(&win), gio::Cancellable::NONE, |_| {});
    });
    let shown = video.downgrade();
    page.connect_hidden(move |_| {
        if let Some(stream) = shown.upgrade().and_then(|v| v.media_stream()) {
            stream.pause();
        }
    });
    let held = RefCell::new(Some((player, landed)));
    let weak_app = app.downgrade();
    page.connect_destroy(move |_| {
        if let (Some((_, landed)), Some(app)) = (held.take(), weak_app.upgrade()) {
            app.disconnect(landed);
        }
    });
    win.push_page(&page);
}
