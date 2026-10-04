use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use crate::backend;
use crate::dialogs::preferences::ListPage;
use crate::jobs::{Job, Kind};
use crate::window::Window;

pub fn present(win: &Window) {
    let dialog = adw::PreferencesDialog::builder().title(gettext("Library Artwork")).content_height(720).build();
    let page = ListPage::new("artwork", &gettext("Library Artwork"), "image-x-generic-symbolic");
    dialog.add(&page.page);
    load(&page, win);
    let (weak_page, weak_win) = (Rc::downgrade(&page), win.downgrade());
    let job = win.app().connect_local("job-changed", false, move |_| {
        if let (Some(page), Some(win)) = (weak_page.upgrade(), weak_win.upgrade()) {
            load(&page, &win);
        }
        None
    });
    let (app, held) = (win.app().downgrade(), RefCell::new(Some((page, job))));
    dialog.connect_closed(move |_| {
        if let (Some((_, job)), Some(app)) = (held.take(), app.upgrade()) {
            app.disconnect(job);
        }
    });
    dialog.present(Some(win));
}

fn load(page: &Rc<ListPage>, win: &Window) {
    page.loading();
    let (page, win) = (page.clone(), win.downgrade());
    glib::spawn_future_local(async move {
        let status = backend::pinned(|core| async move { core.media_status("").await }).await;
        let Some(win) = win.upgrade() else { return };
        page.clear();
        let app = win.app();
        let library = app.library();
        let slots = crate::dialogs::artwork::slots();
        let mut missing: Vec<(String, String, Vec<String>)> = status
            .unwrap_or_default()
            .into_iter()
            .filter(|game| library.get(&game.id).is_some_and(|g| !g.hidden()))
            .filter_map(|game| {
                let gaps: Vec<String> = slots
                    .iter()
                    .filter(|(slot, ..)| game.slots.iter().find(|s| s.slot == *slot).is_none_or(|s| s.kind == "missing"))
                    .map(|(_, label, ..)| label.clone())
                    .collect();
                (!gaps.is_empty()).then_some((game.id, game.title, gaps))
            })
            .collect();
        missing.sort_by_key(|(_, title, _)| crate::library::fold(title));
        let about = if missing.is_empty() {
            gettext("Every game has its art")
        } else {
            ngettext("{} game misses some art", "{} games miss some art", missing.len() as u32).replace("{}", &missing.len().to_string())
        };
        let library_group = page.group(&gettext("Library"), &about);
        match app.job().filter(|j| j.kind() == Kind::Artwork) {
            Some(job) => library_group.add(&job_row(&job)),
            None => {
                let fetch = adw::ButtonRow::builder().title(gettext("Fetch Missing Art")).start_icon_name("folder-download-symbolic").build();
                let weak = app.downgrade();
                fetch.connect_activated(move |_| {
                    if let Some(app) = weak.upgrade() {
                        app.start_job(Kind::Artwork, "", Vec::new(), false);
                    }
                });
                library_group.add(&fetch);
                let again = adw::ButtonRow::builder().title(gettext("Fetch All Again…")).start_icon_name("view-refresh-symbolic").build();
                let weak = win.downgrade();
                again.connect_activated(move |_| {
                    if let Some(win) = weak.upgrade() {
                        confirm_fetch_all(&win);
                    }
                });
                library_group.add(&again);
            }
        }
        if !missing.is_empty() {
            let games = page.group(&gettext("Missing Art"), "");
            for (id, title, gaps) in missing {
                let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), title, gettext("No {}").replace("{}", &gaps.join(", ")));
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                let weak = win.downgrade();
                row.connect_activated(move |_| {
                    if let Some(win) = weak.upgrade() {
                        if let Some(game) = win.app().library().get(&id) {
                            crate::dialogs::artwork::present(&win, &game);
                        }
                    }
                });
                games.add(&row);
            }
        }
    });
}

fn job_row(job: &Job) -> adw::ActionRow {
    let row = crate::rows::plain(adw::ActionRow::builder().build(), job.label(), "");
    crate::components::follow(&row, job, |job| {
        if job.cancelled() {
            return gettext("Stopping after this game…");
        }
        let mut line = vec![job.message()];
        if job.total() > 0 {
            line.push(gettext("{} of {}").replacen("{}", &(job.done() + 1).to_string(), 1).replacen("{}", &job.total().to_string(), 1));
        }
        line.retain(|part| !part.is_empty());
        line.join(" · ")
    });
    row
}

fn confirm_fetch_all(win: &Window) {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Fetch All the Art Again?")),
        Some(&gettext("Every fetched picture and description is fetched again from the stores, GOG GamesDB and libretro, and SteamGridDB with your key. The pictures you picked stay.")),
    );
    dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("fetch", &gettext("_Fetch All"))]);
    dialog.set_response_appearance("fetch", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let weak = win.app().downgrade();
    dialog.connect_response(Some("fetch"), move |_, _| {
        if let Some(app) = weak.upgrade() {
            app.start_job(Kind::Artwork, "", Vec::new(), true);
        }
    });
    dialog.present(Some(win));
}
