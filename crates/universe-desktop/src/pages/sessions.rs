use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};
use universe::sessions::SessionRow;

use crate::backend;
use crate::format;
use crate::media;
use crate::pages::player;
use crate::window::Window;

// The lines a log page shows: a crash's last words, not a whole evening of gamescope.
const TAIL: usize = 400;

fn end_text(row: &SessionRow) -> String {
    match universe::sessions::end_of(&row.session) {
        "quit" => gettext("Quit"),
        "stopped" => gettext("Stopped"),
        "crashed" => gettext("Crashed (exit {})").replace("{}", &row.session.exit.to_string()),
        "killed" => gettext("Killed"),
        _ => gettext("Ended"),
    }
}

fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(child).build()
}

/// A game's sessions, the running one first, each with how it ended; one opens its log.
pub fn open(win: &Window, game: &str) {
    let group = adw::PreferencesGroup::new();
    let column =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(24).margin_bottom(36).margin_start(12).margin_end(12).build();
    column.append(&group);
    let empty = adw::StatusPage::builder()
        .icon_name("document-open-recent-symbolic")
        .title(gettext("No Sessions"))
        .description(gettext("Sessions played from here and from the command line show up here"))
        .visible(false)
        .vexpand(true)
        .build();
    column.append(&empty);
    let toolbar = adw::ToolbarView::builder().content(&scrolled(&adw::Clamp::builder().maximum_size(760).child(&column).build())).build();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("Sessions and Logs")).tag(format!("sessions:{game}")).build();

    let rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::default();
    let load: Rc<dyn Fn()> = {
        let (win, game, group, empty, rows) = (win.downgrade(), game.to_string(), group.downgrade(), empty.downgrade(), rows.clone());
        Rc::new(move || {
            let (win, game, group, empty, rows) = (win.clone(), game.clone(), group.clone(), empty.clone(), rows.clone());
            glib::spawn_future_local(async move {
                let id = game.clone();
                let listed = backend::call(move |core| async move { core.sessions(&id).await }).await;
                let (Some(win), Some(group), Some(empty)) = (win.upgrade(), group.upgrade(), empty.upgrade()) else { return };
                for row in rows.take() {
                    group.remove(&row);
                }
                let listed: Vec<SessionRow> = listed.unwrap_or_default().into_iter().filter(|r| !r.session.unit.is_empty()).collect();
                let mut added = Vec::new();
                if let Some(current) = win.app().current().filter(|c| c.id == game) {
                    let row = adw::ActionRow::builder()
                        .title(media::moment(&current.started_at))
                        .subtitle(gettext("Playing now"))
                        .activatable(true)
                        .use_markup(false)
                        .build();
                    row.add_prefix(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
                    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                    let (weak, game) = (win.downgrade(), game.clone());
                    row.connect_activated(move |_| {
                        if let Some(win) = weak.upgrade() {
                            open_log(&win, &game, "", &gettext("Playing now"), None);
                        }
                    });
                    added.push(row);
                }
                for session in listed {
                    let bad = matches!(universe::sessions::end_of(&session.session), "crashed" | "killed");
                    let subtitle = [format::duration(session.session.duration_s), end_text(&session)].join(" · ");
                    let when = media::moment(&session.session.started_at);
                    let row = adw::ActionRow::builder().title(&when).subtitle(&subtitle).activatable(true).use_markup(false).build();
                    let icon = gtk::Image::from_icon_name(if bad { "dialog-warning-symbolic" } else { "object-select-symbolic" });
                    if bad {
                        icon.add_css_class("error");
                    } else {
                        icon.add_css_class("dimmed");
                    }
                    row.add_prefix(&icon);
                    if session.recording.as_ref().is_some_and(|r| r.exists) {
                        let camera = gtk::Image::builder().icon_name("camera-video-symbolic").tooltip_text(gettext("Recorded")).css_classes(["dimmed"]).build();
                        row.add_suffix(&camera);
                    }
                    if session.journal.is_some() {
                        let entry =
                            gtk::Image::builder().icon_name("text-x-generic-symbolic").tooltip_text(gettext("Journal entry")).css_classes(["dimmed"]).build();
                        row.add_suffix(&entry);
                    }
                    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                    let (weak, game, row_data) = (win.downgrade(), game.clone(), session.clone());
                    row.connect_activated(move |_| {
                        if let Some(win) = weak.upgrade() {
                            open_log(&win, &game, &row_data.session.session, &when, Some(row_data.clone()));
                        }
                    });
                    added.push(row);
                }
                for row in &added {
                    group.add(row);
                }
                empty.set_visible(added.is_empty());
                group.set_visible(!added.is_empty());
                rows.replace(added);
            });
        })
    };
    let app = win.app();
    let (weak_load, id) = (Rc::downgrade(&load), game.to_string());
    let followed = app.connect_changed(move |event| {
        let moved = match event {
            universe::changes::Event::Library(ids) => ids.is_empty() || ids.contains(&id),
            universe::changes::Event::SessionStarted(current) => current.id == id,
            universe::changes::Event::SessionEnded(ended) => ended.id == id,
            _ => false,
        };
        if moved {
            weak_load.upgrade().inspect(|load| load());
        }
    });
    load();
    let (held, weak_app) = (RefCell::new(Some((load, followed))), app.downgrade());
    page.connect_destroy(move |_| {
        if let (Some((_, followed)), Some(app)) = (held.take(), weak_app.upgrade()) {
            app.disconnect(followed);
        }
    });
    win.push_page(&page);
}

/// Error and warning colours for the log, readable on the scheme in use.
fn tones() -> (&'static str, &'static str) {
    if adw::StyleManager::default().is_dark() {
        ("#ff7b63", "#f8e45c")
    } else {
        ("#c01c28", "#9c6e03")
    }
}

/// What a session's processes wrote, its last `TAIL` lines: the running session's when `session` is empty.
pub fn open_log(win: &Window, game: &str, session: &str, when: &str, row: Option<SessionRow>) {
    let buffer = gtk::TextBuffer::new(None);
    let (error, warning) = tones();
    buffer.create_tag(Some("error"), &[("foreground", &error)]);
    buffer.create_tag(Some("warning"), &[("foreground", &warning)]);
    buffer.create_tag(Some("quiet"), &[("foreground-rgba", &gdk::RGBA::new(0.5, 0.5, 0.5, 1.0))]);
    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(12)
        .bottom_margin(24)
        .left_margin(18)
        .right_margin(18)
        .css_classes(["log-view"])
        .build();
    let stack = adw::ViewStack::new();
    stack.add_named(
        &adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build(),
        Some("loading"),
    );
    let status = adw::StatusPage::builder().icon_name("document-open-recent-symbolic").build();
    stack.add_named(&status, Some("status"));
    stack.add_named(&gtk::ScrolledWindow::builder().child(&view).build(), Some("log"));

    let heading = adw::WindowTitle::new(&gettext("Session Log"), when);
    let header = adw::HeaderBar::builder().title_widget(&heading).build();
    let copy = gtk::Button::builder().icon_name("edit-copy-symbolic").tooltip_text(gettext("Copy the Log")).build();
    let refresh = gtk::Button::builder().icon_name("view-refresh-symbolic").tooltip_text(gettext("Read It Again")).build();
    header.pack_end(&copy);
    header.pack_end(&refresh);
    if let Some(dir) = row.as_ref().and_then(|r| r.debug_log.clone()) {
        let folder = gtk::Button::builder().icon_name("folder-open-symbolic").tooltip_text(gettext("Open the Debug Logs")).build();
        let weak = win.downgrade();
        folder.connect_clicked(move |_| {
            if let Some(win) = weak.upgrade() {
                gtk::FileLauncher::new(Some(&gio::File::for_path(&dir))).launch(Some(&win), gio::Cancellable::NONE, |_| {});
            }
        });
        header.pack_start(&folder);
    }
    if let Some(recording) = row.as_ref().and_then(player::Recording::of_session) {
        let watch = gtk::Button::builder().icon_name("camera-video-symbolic").tooltip_text(gettext("Watch the Recording")).build();
        let weak = win.downgrade();
        watch.connect_clicked(move |_| {
            if let Some(win) = weak.upgrade() {
                player::open_recording(&win, recording.clone());
            }
        });
        header.pack_start(&watch);
    }
    let toolbar = adw::ToolbarView::builder().content(&stack).build();
    toolbar.add_top_bar(&header);
    let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("Session Log")).tag("log").build();

    let load: Rc<dyn Fn()> = {
        let (game, session, buffer, stack, status) = (game.to_string(), session.to_string(), buffer.clone(), stack.downgrade(), status.downgrade());
        Rc::new(move || {
            let (game, session, buffer, stack, status) = (game.clone(), session.clone(), buffer.clone(), stack.clone(), status.clone());
            glib::spawn_future_local(async move {
                let lines = backend::call(move |core| async move { core.session_log(&game, &session, TAIL).await }).await;
                let (Some(stack), Some(status)) = (stack.upgrade(), status.upgrade()) else { return };
                buffer.set_text("");
                match lines {
                    Ok(lines) if lines.is_empty() => {
                        status.set_title(&gettext("Nothing Logged"));
                        status.set_description(Some(&gettext("The system journal no longer holds this session's output")));
                        stack.set_visible_child_name("status");
                    }
                    Ok(lines) => {
                        let mut end = buffer.end_iter();
                        for line in lines {
                            let clock =
                                chrono::DateTime::parse_from_rfc3339(&line.time).map(|t| t.with_timezone(&chrono::Local).format("%H:%M:%S ").to_string());
                            buffer.insert_with_tags_by_name(&mut end, &format!("{}{} ", clock.unwrap_or_default(), line.source), &["quiet"]);
                            match line.priority {
                                0..=3 => buffer.insert_with_tags_by_name(&mut end, &line.message, &["error"]),
                                4 => buffer.insert_with_tags_by_name(&mut end, &line.message, &["warning"]),
                                _ => buffer.insert(&mut end, &line.message),
                            }
                            buffer.insert(&mut end, "\n");
                        }
                        stack.set_visible_child_name("log");
                    }
                    Err(e) => {
                        status.set_title(&gettext("The Log Could Not Be Read"));
                        status.set_description(Some(&glib::markup_escape_text(&e.to_string())));
                        stack.set_visible_child_name("status");
                    }
                }
            });
        })
    };
    let weak_load = Rc::downgrade(&load);
    refresh.connect_clicked(move |_| {
        weak_load.upgrade().inspect(|load| load());
    });
    let (weak_buffer, weak) = (buffer.downgrade(), win.downgrade());
    copy.connect_clicked(move |_| {
        let (Some(buffer), Some(win)) = (weak_buffer.upgrade(), weak.upgrade()) else { return };
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        win.clipboard().set_text(&text);
        win.toast(adw::Toast::new(&gettext("Log copied")));
    });
    load();
    let held = RefCell::new(Some(load));
    page.connect_destroy(move |_| {
        held.take();
    });
    win.push_page(&page);
}
