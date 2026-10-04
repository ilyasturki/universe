use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use universe::journal::Entry;
use universe::sessions::SessionRow;

use crate::backend;
use crate::format;
use crate::media;
use crate::pages::{player, viewer};
use crate::widgets::Cover;
use crate::window::Window;

fn list_item(text: &str) -> bool {
    let t = text.trim_start();
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") || (digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") ")))
}

/// The paragraphs as the reader shows them: a list's items, one paragraph each on disk, in one block.
pub fn blocks(paragraphs: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paragraphs {
        match out.last_mut() {
            Some(last) if list_item(p) && last.rsplit('\n').next().is_some_and(list_item) => {
                last.push('\n');
                last.push_str(p);
            }
            _ => out.push(p.clone()),
        }
    }
    out
}

/// A link's text and address at the head of `text`, `[text](address)`, and how long it runs.
fn link(text: &str) -> Option<(&str, &str, usize)> {
    let close = text.find("](")?;
    let end = close + 2 + text[close + 2..].find(')')?;
    Some((&text[1..close], &text[close + 2..end], end + 1))
}

/// A paragraph's Markdown as Pango markup: bold, italic, code and links, the rest as written. `None` when it would not parse.
pub fn markup(block: &str) -> Option<String> {
    let mut out = String::new();
    for (n, line) in block.lines().enumerate() {
        if n > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let (line, heading) = match trimmed.trim_start_matches('#') {
            rest if rest.len() < trimmed.len() && rest.starts_with(' ') => (rest.trim_start(), true),
            _ => (line, false),
        };
        let line = match line.trim_start().strip_prefix("- ").or_else(|| line.trim_start().strip_prefix("* ")).or_else(|| line.trim_start().strip_prefix("+ "))
        {
            Some(item) => format!("•  {item}"),
            None => line.to_string(),
        };
        let text = glib::markup_escape_text(&line).to_string();
        let (mut bold, mut italic, mut code) = (false, false, false);
        let mut rest = text.as_str();
        let mut prev = ' ';
        if heading {
            out.push_str("<b>");
        }
        while let Some(c) = rest.chars().next() {
            let next = rest[c.len_utf8()..].chars().next().unwrap_or(' ');
            if code {
                if c == '`' {
                    out.push_str("</tt>");
                    code = false;
                } else {
                    out.push(c);
                }
            } else if rest.starts_with("**") || rest.starts_with("__") {
                out.push_str(if bold { "</b>" } else { "<b>" });
                bold = !bold;
                rest = &rest[2..];
                prev = c;
                continue;
            } else if (c == '*' || c == '_') && (italic || !next.is_whitespace()) && !(c == '_' && prev.is_alphanumeric() && next.is_alphanumeric()) {
                out.push_str(if italic { "</i>" } else { "<i>" });
                italic = !italic;
            } else if c == '`' {
                out.push_str("<tt>");
                code = true;
            } else if let Some((label, address, len)) = (c == '[').then(|| link(rest)).flatten() {
                out.push_str(&format!("<a href=\"{address}\">{label}</a>"));
                rest = &rest[len..];
                prev = ')';
                continue;
            } else {
                out.push(c);
            }
            prev = c;
            rest = &rest[c.len_utf8()..];
        }
        for (open, tag) in [(code, "</tt>"), (italic, "</i>"), (bold, "</b>")] {
            if open {
                out.push_str(tag);
            }
        }
        if heading {
            out.push_str("</b>");
        }
    }
    gtk::pango::parse_markup(&without_links(&out), '\0').ok().map(|_| out)
}

/// The markup with its links' tags taken out: a label reads `<a>` itself, Pango does not know it.
fn without_links(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(at) = rest.find("<a href=\"") {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        rest = rest.find("\">").map_or("", |end| &rest[end + 2..]);
    }
    out.push_str(rest);
    out.replace("</a>", "")
}

fn label(text: &str) -> gtk::Label {
    let label =
        gtk::Label::builder().wrap(true).wrap_mode(gtk::pango::WrapMode::WordChar).xalign(0.0).selectable(true).css_classes(["body", "journal-text"]).build();
    match markup(text) {
        Some(m) => label.set_markup(&m),
        None => label.set_label(text),
    }
    label
}

/// A session the player played, `YYYYMMDD-HHMMSS`: an imported one has no journal to write.
fn played(session: &str) -> bool {
    let b = session.as_bytes();
    b.len() == 15 && b[8] == b'-' && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
}

fn state_line(entry: &Entry) -> String {
    match entry.state.as_str() {
        "pending" => gettext("Being written"),
        "deferred" => {
            let when = chrono::DateTime::parse_from_rfc3339(&entry.retry_at).map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string()).ok();
            match when {
                Some(at) => gettext("Put off until {}").replace("{}", &at),
                None => gettext("Put off"),
            }
        }
        "failed" => gettext("Could not be written"),
        _ => String::new(),
    }
}

fn title_of(entry: &Entry) -> String {
    match entry.state.as_str() {
        _ if !entry.title.is_empty() => entry.title.clone(),
        "pending" => gettext("Being Written"),
        "deferred" => gettext("Put Off"),
        "failed" => gettext("Journal Failed"),
        _ => gettext("Untitled"),
    }
}

/// What a journal page shows of one game: its entries and its sessions, each with the recording when there is one.
#[derive(Debug, Clone, Default)]
struct Journal {
    entries: Vec<Entry>,
    sessions: Vec<SessionRow>,
}

impl Journal {
    async fn load(game: &str) -> Result<Journal, String> {
        let game = game.to_string();
        backend::call(move |core| async move {
            let entries = core.journal(&game).await?;
            let sessions = core.sessions(&game).await?;
            Ok(Journal { entries, sessions })
        })
        .await
        .map_err(|e| e.to_string())
    }

    fn session(&self, session: &str) -> Option<&SessionRow> {
        self.sessions.iter().find(|s| s.session.session == session)
    }

    fn recording(&self, session: &str) -> Option<player::Recording> {
        self.session(session).and_then(player::Recording::of_session)
    }
}

fn say(win: &Window, text: &str) {
    win.toast(adw::Toast::new(text));
}

/// Starts the journal module on one session: a first entry, another try, or a new entry over a written one.
pub fn write(win: &Window, game: &str, session: &str, rewrite: bool) {
    let (weak, game, session) = (win.downgrade(), game.to_string(), session.to_string());
    glib::spawn_future_local(async move {
        let result = backend::call(move |core| async move { core.journal_write(&game, &session, rewrite).await }).await;
        let Some(win) = weak.upgrade() else { return };
        match result {
            Ok(_) => say(&win, &gettext("Writing the journal entry…")),
            Err(e) => say(&win, &e.to_string()),
        }
    });
}

/// The entry goes at once, for good once the toast is gone.
fn delete(win: &Window, game: &str, session: &str) {
    let (game, session) = (game.to_string(), session.to_string());
    win.app().defer(&media::journal_key(&game, &session), &gettext("Journal entry deleted"), move || async move {
        if let Err(e) = backend::call(move |core| async move { core.remove_journal_entry(&game, &session).await }).await {
            tracing::warn!("remove journal entry: {e}");
        }
    });
}

/// A pending entry's writing is stopped at once: there is no undoing a stopped unit.
fn stop(win: &Window, game: &str, session: &str, then: Rc<dyn Fn()>) {
    let dialog = adw::AlertDialog::new(Some(&gettext("Stop Writing This Entry?")), Some(&gettext("The journal module stops, and the entry is not written.")));
    dialog.add_responses(&[("cancel", &gettext("_Keep Writing")), ("stop", &gettext("_Stop"))]);
    dialog.set_response_appearance("stop", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let (weak, game, session) = (win.downgrade(), game.to_string(), session.to_string());
    dialog.connect_response(Some("stop"), move |_, _| {
        let (weak, game, session, then) = (weak.clone(), game.clone(), session.clone(), then.clone());
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.remove_journal_entry(&game, &session).await }).await;
            if let (Err(e), Some(win)) = (result, weak.upgrade()) {
                say(&win, &e.to_string());
            }
            then();
        });
    });
    dialog.present(Some(win));
}

fn page(title: &str, tag: &str, header: &adw::HeaderBar) -> (adw::NavigationPage, gtk::Box, adw::ViewStack) {
    let column =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(24).margin_bottom(36).margin_start(12).margin_end(12).build();
    let clamp = adw::Clamp::builder().maximum_size(760).child(&column).build();
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&clamp).build();
    let stack = adw::ViewStack::new();
    stack.add_named(
        &adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build(),
        Some("loading"),
    );
    stack.add_named(&scroll, Some("content"));
    let toolbar = adw::ToolbarView::builder().content(&stack).build();
    toolbar.add_top_bar(header);
    (adw::NavigationPage::builder().child(&toolbar).title(title).tag(tag).build(), column, stack)
}

fn clear(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

/// One entry to read: its title and day, the prose, the pictures it was written from and what comes next.
pub fn open_entry(win: &Window, game: &str, session: &str) {
    let menu = gtk::MenuButton::builder().icon_name("view-more-symbolic").tooltip_text(gettext("More")).build();
    let header = adw::HeaderBar::new();
    header.pack_end(&menu);
    let (page, column, stack) = page(&gettext("Journal Entry"), "entry", &header);
    let actions = gio::SimpleActionGroup::new();
    page.insert_action_group("entry", Some(&actions));
    let state = Rc::new(RefCell::new(Journal::default()));
    let (game, session) = (game.to_string(), session.to_string());

    let fill: Rc<dyn Fn()> = {
        let (weak_win, weak_page, game, session, state) = (win.downgrade(), page.downgrade(), game.clone(), session.clone(), state.clone());
        let (column, stack, menu) = (column.downgrade(), stack.downgrade(), menu.downgrade());
        Rc::new(move || {
            let (Some(win), Some(page), Some(column), Some(stack), Some(menu)) =
                (weak_win.upgrade(), weak_page.upgrade(), column.upgrade(), stack.upgrade(), menu.upgrade())
            else {
                return;
            };
            let journal = state.borrow().clone();
            let title = win.app().library().get(&game).map(|g| g.title()).unwrap_or_else(|| game.clone());
            clear(&column);
            let Some(entry) = journal.entries.iter().find(|e| e.session == session).cloned() else {
                column.append(&adw::StatusPage::builder().icon_name("text-x-generic-symbolic").title(gettext("No Entry")).build());
                menu.set_visible(false);
                stack.set_visible_child_name("content");
                return;
            };
            page.set_title(&title_of(&entry));
            let heading = gtk::Label::builder().label(title_of(&entry)).wrap(true).xalign(0.0).css_classes(["title-1"]).build();
            let when = media::moment(if entry.started_at.is_empty() { &entry.written_at } else { &entry.started_at });
            let mut meta = vec![title.clone(), when];
            if entry.duration_s > 0 {
                meta.push(format::duration(entry.duration_s as u64));
            }
            let head = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
            head.append(&heading);
            head.append(&gtk::Label::builder().label(meta.join(" · ")).wrap(true).xalign(0.0).css_classes(["dimmed"]).build());
            column.append(&head);
            match entry.state.as_str() {
                "written" => {
                    for block in blocks(&entry.paragraphs) {
                        column.append(&label(&block));
                    }
                    if !entry.images.is_empty() {
                        column.append(&pictures(&win, &entry, &title));
                    }
                    if !entry.next_up.trim().is_empty() {
                        let group = adw::PreferencesGroup::builder().title(gettext("Next Up")).build();
                        let next = label(entry.next_up.trim());
                        next.add_css_class("card");
                        next.add_css_class("journal-next");
                        group.add(&next);
                        column.append(&group);
                    }
                }
                state => {
                    let reason = entry.paragraphs.first().cloned().unwrap_or_default();
                    let status = adw::StatusPage::builder()
                        .icon_name(match state {
                            "pending" => "document-edit-symbolic",
                            "deferred" => "alarm-symbolic",
                            _ => "dialog-warning-symbolic",
                        })
                        .title(state_line(&entry))
                        .description(glib::markup_escape_text(&reason).as_str())
                        .css_classes(["compact"])
                        .build();
                    if state != "pending" {
                        let again = gtk::Button::builder()
                            .label(gettext("_Try Again Now"))
                            .use_underline(true)
                            .halign(gtk::Align::Center)
                            .action_name("entry.write")
                            .css_classes(["pill", "suggested-action"])
                            .build();
                        status.set_child(Some(&again));
                    } else {
                        status.set_child(Some(&adw::Spinner::builder().width_request(24).height_request(24).build()));
                    }
                    column.append(&status);
                }
            }
            let model = gio::Menu::new();
            match entry.state.as_str() {
                "written" => model.append(Some(&gettext("_Write It Again")), Some("entry.rewrite")),
                "pending" => {}
                _ => model.append(Some(&gettext("_Try Again Now")), Some("entry.write")),
            }
            if journal.recording(&session).is_some() {
                model.append(Some(&gettext("Watch the _Recording")), Some("entry.recording"));
            }
            let end = gio::Menu::new();
            end.append(Some(&if entry.state == "pending" { gettext("_Stop Writing…") } else { gettext("_Delete Entry") }), Some("entry.delete"));
            model.append_section(None, &end);
            menu.set_menu_model(Some(&model));
            menu.set_visible(true);
            stack.set_visible_child_name("content");
        })
    };

    let load: Rc<dyn Fn()> = {
        let (game, state, fill) = (game.clone(), state.clone(), fill.clone());
        Rc::new(move || {
            let (game, state, fill) = (game.clone(), state.clone(), fill.clone());
            glib::spawn_future_local(async move {
                match Journal::load(&game).await {
                    Ok(journal) => {
                        state.replace(journal);
                        fill();
                    }
                    Err(e) => tracing::warn!("journal: {e}"),
                }
            });
        })
    };

    let add = |name: &str, act: Box<dyn Fn(&Window)>| {
        let action = gio::SimpleAction::new(name, None);
        let weak = win.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(win) = weak.upgrade() {
                act(&win);
            }
        });
        actions.add_action(&action);
    };
    let (g, s) = (game.clone(), session.clone());
    add("write", Box::new(move |win| write(win, &g, &s, false)));
    let (g, s) = (game.clone(), session.clone());
    add("rewrite", Box::new(move |win| write(win, &g, &s, true)));
    let (st, s) = (state.clone(), session.clone());
    add(
        "recording",
        Box::new(move |win| {
            if let Some(recording) = st.borrow().recording(&s) {
                player::open_recording(win, recording);
            }
        }),
    );
    let (g, s, st, page_ref, reload) = (game.clone(), session.clone(), state.clone(), page.downgrade(), load.clone());
    add(
        "delete",
        Box::new(move |win| {
            let pending = st.borrow().entries.iter().any(|e| e.session == s && e.state == "pending");
            if pending {
                stop(win, &g, &s, reload.clone());
            } else {
                delete(win, &g, &s);
                if let Some(page) = page_ref.upgrade() {
                    let _ = page.activate_action("navigation.pop", None);
                }
            }
        }),
    );

    let (weak_load, id) = (Rc::downgrade(&load), game.clone());
    let followed = win.app().connect_changed(move |event| {
        if let universe::changes::Event::Journal(changed) = event {
            if *changed == id {
                weak_load.upgrade().inspect(|load| load());
            }
        }
    });
    load();
    let (held, app) = (RefCell::new(Some((load, fill, followed))), win.app().downgrade());
    page.connect_destroy(move |_| {
        if let (Some((_, _, followed)), Some(app)) = (held.take(), app.upgrade()) {
            app.disconnect(followed);
        }
    });
    win.push_page(&page);
}

/// The pictures an entry was written from, in a row that opens the viewer.
fn pictures(win: &Window, entry: &Entry, title: &str) -> gtk::Widget {
    let flow =
        gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).column_spacing(12).row_spacing(12).max_children_per_line(4).homogeneous(true).build();
    let shots: Vec<viewer::Shot> = entry
        .images
        .iter()
        .map(|path| viewer::Shot {
            path: path.clone(),
            game: entry.game.clone(),
            title: title.to_string(),
            date: entry.started_at.clone(),
            ..viewer::Shot::default()
        })
        .collect();
    for path in &entry.images {
        let cover = Cover::new(176, 99);
        cover.set_path(path.clone());
        flow.append(&cover);
    }
    let weak = win.downgrade();
    flow.connect_child_activated(move |_, child| {
        if let Some(win) = weak.upgrade() {
            viewer::open(&win, shots.clone(), child.index().max(0) as usize);
        }
    });
    flow.upcast()
}

/// A game's journal: every entry, the ones being written and put off among them, and the sessions still without one.
pub fn open_list(win: &Window, game: &str) {
    let (page, column, stack) = page(&gettext("Journal"), &format!("journal:{game}"), &crate::pages::game_header(win, &gettext("Journal"), game));
    let game = game.to_string();
    let fill: Rc<dyn Fn(Journal)> = {
        let (weak_win, column, stack, game) = (win.downgrade(), column.downgrade(), stack.downgrade(), game.clone());
        Rc::new(move |journal: Journal| {
            let (Some(win), Some(column), Some(stack)) = (weak_win.upgrade(), column.upgrade(), stack.upgrade()) else { return };
            clear(&column);
            let group = adw::PreferencesGroup::new();
            let mut sessions: Vec<(String, Option<Entry>, u64)> =
                journal.entries.iter().map(|e| (e.session.clone(), Some(e.clone()), e.duration_s.max(0) as u64)).collect();
            for row in &journal.sessions {
                if played(&row.session.session) && !sessions.iter().any(|(s, _, _)| *s == row.session.session) {
                    sessions.push((row.session.session.clone(), None, row.session.duration_s));
                }
            }
            sessions.sort_by(|a, b| b.0.cmp(&a.0));
            let app = win.app();
            for (session, entry, duration) in sessions {
                if app.is_deferred(&media::journal_key(&game, &session)) {
                    continue;
                }
                let when = entry
                    .as_ref()
                    .map(|e| if e.started_at.is_empty() { e.written_at.clone() } else { e.started_at.clone() })
                    .or_else(|| journal.session(&session).map(|s| s.session.started_at.clone()))
                    .unwrap_or_default();
                let mut line = vec![media::moment(&when)];
                if duration > 0 {
                    line.push(format::duration(duration));
                }
                let row = crate::rows::plain(adw::ActionRow::builder().title_lines(1).subtitle_lines(1).build(), "", "");
                let art = Cover::new(80, 45);
                art.add_css_class("thumb");
                art.set_valign(gtk::Align::Center);
                row.add_prefix(&art);
                match &entry {
                    Some(entry) => {
                        row.set_title(&title_of(entry));
                        let state = state_line(entry);
                        if !state.is_empty() {
                            line.push(state);
                        }
                        art.set_path(entry.images.first().cloned().unwrap_or_default());
                        if entry.state == "pending" {
                            row.add_suffix(&adw::Spinner::new());
                        }
                        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                        row.set_activatable(true);
                        let (weak, game, session) = (win.downgrade(), game.clone(), session.clone());
                        row.connect_activated(move |_| {
                            if let Some(win) = weak.upgrade() {
                                open_entry(&win, &game, &session);
                            }
                        });
                    }
                    None => {
                        row.set_title(&gettext("No Entry"));
                        let write_button = gtk::Button::builder().label(gettext("_Write")).use_underline(true).valign(gtk::Align::Center).build();
                        let (weak, game, session) = (win.downgrade(), game.clone(), session.clone());
                        write_button.connect_clicked(move |_| {
                            if let Some(win) = weak.upgrade() {
                                write(&win, &game, &session, false);
                            }
                        });
                        row.add_suffix(&write_button);
                    }
                }
                row.set_subtitle(&line.join(" · "));
                group.add(&row);
            }
            if group.row(0).is_some() {
                column.append(&group);
            } else {
                column.append(
                    &adw::StatusPage::builder()
                        .icon_name("text-x-generic-symbolic")
                        .title(gettext("No Journal Entries"))
                        .description(gettext("An entry is written after each session"))
                        .build(),
                );
            }
            stack.set_visible_child_name("content");
        })
    };
    let load: Rc<dyn Fn()> = {
        let (game, fill) = (game.clone(), fill.clone());
        Rc::new(move || {
            let (game, fill) = (game.clone(), fill.clone());
            glib::spawn_future_local(async move {
                match Journal::load(&game).await {
                    Ok(journal) => fill(journal),
                    Err(e) => tracing::warn!("journal: {e}"),
                }
            });
        })
    };
    let app = win.app();
    let (weak_load, id) = (Rc::downgrade(&load), game.clone());
    let followed = app.connect_changed(move |event| {
        let moved = match event {
            universe::changes::Event::Journal(changed) => *changed == id,
            universe::changes::Event::Library(ids) => ids.is_empty() || ids.contains(&id),
            _ => false,
        };
        if moved {
            weak_load.upgrade().inspect(|load| load());
        }
    });
    let weak_load = Rc::downgrade(&load);
    let deferred = app.connect_deferred_changed(move || {
        weak_load.upgrade().inspect(|load| load());
    });
    load();
    let (held, weak_app) = (RefCell::new(Some((load, fill, followed, deferred))), app.downgrade());
    page.connect_destroy(move |_| {
        if let (Some((_, _, followed, deferred)), Some(app)) = (held.take(), weak_app.upgrade()) {
            app.disconnect(followed);
            app.disconnect(deferred);
        }
    });
    win.push_page(&page);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_stays_one_block_and_prose_its_own() {
        let paragraphs: Vec<String> = ["We sailed.", "- the boat", "- the storm", "Then home.", "1. one", "2) two"].map(String::from).to_vec();
        assert_eq!(blocks(&paragraphs), ["We sailed.", "- the boat\n- the storm", "Then home.", "1. one\n2) two"]);
    }

    #[test]
    fn markdown_turns_to_markup_escaped_first() {
        assert_eq!(markup("**Bold** and *soft* & `code`").as_deref(), Some("<b>Bold</b> and <i>soft</i> &amp; <tt>code</tt>"));
        assert_eq!(markup("see [the wiki](https://a.b/?x=1&y=2)").as_deref(), Some("see <a href=\"https://a.b/?x=1&amp;y=2\">the wiki</a>"));
        assert_eq!(markup("snake_case_name stays").as_deref(), Some("snake_case_name stays"));
        assert_eq!(markup("- an item").as_deref(), Some("•  an item"));
        assert_eq!(markup("## A heading").as_deref(), Some("<b>A heading</b>"));
        assert_eq!(markup("*unclosed").as_deref(), Some("<i>unclosed</i>"));
    }
}
