use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use serde_json::Value;
use universe::forms::Form;

use crate::backend;
use crate::dialogs::signin;
use crate::form_view::FormView;
use crate::jobs::{Job, Kind};
use crate::window::Window;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn list(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// A page of rows read from the core, cleared and filled again whenever it changes.
struct ListPage {
    page: adw::PreferencesPage,
    groups: RefCell<Vec<adw::PreferencesGroup>>,
    rows: RefCell<Vec<crate::components::Keyed>>,
}

impl ListPage {
    fn new(name: &str, title: &str, icon: &str) -> Rc<ListPage> {
        Rc::new(ListPage {
            page: adw::PreferencesPage::builder().name(name).title(title).icon_name(icon).build(),
            groups: RefCell::default(),
            rows: RefCell::default(),
        })
    }

    fn clear(&self) {
        self.rows.take();
        for group in self.groups.take() {
            self.page.remove(&group);
        }
    }

    /// `row` found again by `key` after a rebuild, the focus with it.
    fn keep(&self, key: &str, row: &impl IsA<gtk::Widget>) {
        self.rows.borrow_mut().push((key.to_string(), row.clone().upcast()));
    }

    fn group(&self, title: &str, description: &str) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        if !title.is_empty() {
            group.set_title(title);
        }
        if !description.is_empty() {
            group.set_description(Some(description));
        }
        self.page.add(&group);
        self.groups.borrow_mut().push(group.clone());
        group
    }

    fn loading(&self) {
        self.clear();
        let group = self.group("", "");
        group.add(&adw::Spinner::builder().height_request(48).margin_top(24).build());
    }
}

/// A subpage of the dialog around a form: a header bar, the form's page, the view kept alive with the page.
fn push_form(dialog: &adw::PreferencesDialog, title: &str, view: Rc<FormView>) {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&view.page));
    let page = adw::NavigationPage::new(&toolbar, title);
    view.load();
    page.connect_hidden(move |_| {
        let _ = &view;
    });
    dialog.push_subpage(&page);
}

fn chevron() -> gtk::Image {
    gtk::Image::from_icon_name("go-next-symbolic")
}

/// `page`: `launch`, `runners`, `stores`, `modules`, `controller`, `system` (a Steam Deck's), `artwork` or `doctor`; empty
/// for the first.
pub fn present(win: &Window, page: &str) {
    let dialog = adw::PreferencesDialog::builder().search_enabled(true).content_height(720).build();
    let connector = win.connector();

    let launch = FormView::new(Form::Launch, &dialog, connector.clone());
    launch.page.set_name(Some("launch"));
    launch.page.set_title(&gettext("Launch"));
    launch.page.set_icon_name(Some("media-playback-start-symbolic"));
    dialog.add(&launch.page);
    launch.load();

    let runners = ListPage::new("runners", &gettext("Runners"), "system-run-symbolic");
    dialog.add(&runners.page);
    load_runners(&dialog, &runners, win, &connector, false);

    let stores = ListPage::new("stores", &gettext("Stores"), "system-software-install-symbolic");
    dialog.add(&stores.page);
    load_stores(&dialog, &stores, &connector);

    let modules = ListPage::new("modules", &gettext("Modules"), "package-x-generic-symbolic");
    dialog.add(&modules.page);
    load_modules(&dialog, &modules, &connector);

    let controller = crate::dialogs::controller::page(&dialog);
    dialog.add(&controller.page);

    let system = universe::deck::model().is_some().then(|| {
        let page = ListPage::new("system", &gettext("System"), "computer-symbolic");
        dialog.add(&page.page);
        load_system(&dialog, &page);
        page
    });

    let artwork = ListPage::new("artwork", &gettext("Artwork"), "image-x-generic-symbolic");
    dialog.add(&artwork.page);
    load_artwork(&artwork, win);

    let doctor = ListPage::new("doctor", &gettext("Doctor"), "emblem-ok-symbolic");
    dialog.add(&doctor.page);
    load_doctor(&dialog, &doctor);
    let (weak_page, weak_win) = (Rc::downgrade(&artwork), win.downgrade());
    let job = win.app().connect_local("job-changed", false, move |_| {
        if let (Some(page), Some(win)) = (weak_page.upgrade(), weak_win.upgrade()) {
            load_artwork(&page, &win);
        }
        None
    });
    let (weak_dialog, weak_runners, weak_win, runners_connector) = (dialog.downgrade(), Rc::downgrade(&runners), win.downgrade(), connector.clone());
    let component_job = crate::components::on_jobs(&win.app(), move || {
        if let (Some(dialog), Some(runners), Some(win)) = (weak_dialog.upgrade(), weak_runners.upgrade(), weak_win.upgrade()) {
            load_runners(&dialog, &runners, &win, &runners_connector, true);
        }
    });

    let (app, jobs) = (win.app().downgrade(), RefCell::new(vec![job, component_job]));
    dialog.connect_closed(move |_| {
        let _ = (&launch, &runners, &stores, &modules, &controller, &system, &artwork, &doctor);
        if let Some(app) = app.upgrade() {
            for handler in jobs.take() {
                app.disconnect(handler);
            }
        }
    });
    if !page.is_empty() {
        dialog.set_visible_page_name(page);
    }
    dialog.present(Some(win));
}

/// The Deck's own controls but the backlight, which the desktop sets.
fn load_system(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>) {
    page.loading();
    let (weak_dialog, weak) = (dialog.downgrade(), Rc::downgrade(page));
    glib::spawn_future_local(async move {
        let controls = backend::pinned(|core| async move { core.system_controls().await }).await;
        let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak.upgrade()) else { return };
        page.clear();
        let controls: Vec<_> = controls.into_iter().filter(|c| c.id != "brightness").collect();
        let about = if controls.is_empty() {
            gettext("None of its controls can be set from here")
        } else {
            gettext("Set at once, and set again each time Universe starts: the power limit and the clocks do not outlive a reboot")
        };
        let group = page.group(&gettext("Steam Deck"), &about);
        for control in controls {
            group.add(&system_row(&dialog, control));
        }
    });
}

fn system_row(dialog: &adw::PreferencesDialog, control: universe::hardware::Control) -> adw::ActionRow {
    let (id, weak) = (control.id, dialog.downgrade());
    let set = move |value: String| {
        let weak = weak.clone();
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.set_system(id, &value).await }).await;
            if let (Err(e), Some(dialog)) = (result, weak.upgrade()) {
                dialog.add_toast(crate::dialogs::toast(&e.to_string()));
            }
        });
    };
    let shown = |value: &str| if value == "auto" { gettext("Auto") } else { format!("{value} {}", control.unit).trim_end().to_string() };
    let row: adw::ActionRow = match control.kind {
        "choice" => {
            let names: Vec<String> = control.choices.iter().map(|c| shown(c)).collect();
            let row = adw::ComboRow::builder().model(&gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>())).build();
            row.set_selected(control.choices.iter().position(|c| *c == control.value).unwrap_or(0) as u32);
            let choices = control.choices.clone();
            row.connect_selected_notify(move |row| {
                if let Some(value) = choices.get(row.selected() as usize) {
                    set(value.clone());
                }
            });
            row.upcast()
        }
        "toggle" => {
            let row = adw::SwitchRow::builder().active(control.value == "on").build();
            row.connect_active_notify(move |row| set(if row.is_active() { "on" } else { "off" }.into()));
            row.upcast()
        }
        _ => {
            let row = adw::SpinRow::with_range(f64::from(control.min), f64::from(control.max), f64::from(control.step.max(1)));
            row.set_value(control.value.parse().unwrap_or(f64::from(control.min)));
            row.add_suffix(&gtk::Label::builder().label(control.unit).css_classes(["dimmed"]).build());
            // A held button steps many times a second: the value is written once it rests.
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::default();
            row.connect_value_notify(move |row| {
                if let Some(source) = pending.take() {
                    source.remove();
                }
                let (value, set, done) = ((row.value().round() as u32).to_string(), set.clone(), pending.clone());
                pending.replace(Some(glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                    done.replace(None);
                    set(value);
                })));
            });
            row.upcast()
        }
    };
    crate::rows::plain(row, control.label, control.detail)
}

/// `reload`: the rows rebuilt in one go where they were, the focus kept, no spinner.
fn load_runners(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, win: &Window, connector: &str, reload: bool) {
    if !reload {
        page.loading();
    }
    let mut usage: HashMap<String, (usize, f64)> = HashMap::new();
    let mut games: HashMap<String, Vec<(String, String, f64)>> = HashMap::new();
    for game in win.app().library().games() {
        let row = game.row();
        let entry = usage.entry(row.runner.clone()).or_default();
        entry.0 += 1;
        entry.1 += row.hours;
        games.entry(row.runner.clone()).or_default().push((row.id.clone(), row.title.clone(), row.hours));
    }
    let (dialog, page, win, connector) = (dialog.downgrade(), page.clone(), win.downgrade(), connector.to_string());
    glib::spawn_future_local(async move {
        let runners = backend::run(async { backend::core().runners().await }).await;
        let listed = crate::components::listing(false).await;
        let (Some(dialog), Some(win)) = (dialog.upgrade(), win.upgrade()) else { return };
        let focus = crate::components::take_focus(&page.rows.borrow());
        page.clear();
        let listed = listed.unwrap_or_else(|e| {
            dialog.add_toast(crate::dialogs::toast(&e.to_string()));
            Vec::new()
        });
        let again: Rc<dyn Fn()> = {
            let (dialog, page, win, connector) = (dialog.downgrade(), Rc::downgrade(&page), win.downgrade(), connector.clone());
            Rc::new(move || {
                if let (Some(dialog), Some(page), Some(win)) = (dialog.upgrade(), page.upgrade(), win.upgrade()) {
                    load_runners(&dialog, &page, &win, &connector, true);
                }
            })
        };
        let found = |r: &Value| text(r, "kind") == "linux" || !text(r, "path").is_empty();
        let mut sorted = runners;
        sorted.sort_by(|a, b| {
            let (ua, ub) = (usage.get(&text(a, "id")).copied().unwrap_or_default(), usage.get(&text(b, "id")).copied().unwrap_or_default());
            ub.0.cmp(&ua.0).then(ub.1.total_cmp(&ua.1)).then(text(a, "name").to_lowercase().cmp(&text(b, "name").to_lowercase()))
        });
        let installed = page.group("", &gettext("The programs your games start through, and what Universe installs for them"));
        let offered = page.group(&gettext("Not Installed"), &gettext("Universe downloads and updates these"));
        let missing = if listed.is_empty() {
            page.group(&gettext("Not Found"), "")
        } else {
            page.group(&gettext("No Download"), &gettext("Install these from your distribution: upstream ships no build Universe can fetch"))
        };
        let tools = page.group(&gettext("Tools"), &gettext("What runs beside the games"));
        let (mut waiting, mut none) = (Vec::new(), 0);
        for runner in sorted {
            let id = text(&runner, "id");
            let own = crate::components::of_runner(&id, &text(&runner, "kind"), &listed);
            let (in_use, tag, wants) = crate::components::summary(&own);
            let count = usage.get(&id).map(|u| u.0).unwrap_or(0);
            let platforms = list(&runner, "platforms").iter().map(|p| crate::library::platform_name(p)).collect::<Vec<_>>().join(", ");
            let row = crate::rows::plain(
                adw::ActionRow::builder().activatable(true).build(),
                text(&runner, "name"),
                if in_use.is_empty() { platforms } else { in_use },
            );
            if !tag.is_empty() {
                let class = if wants { "accent" } else { "dimmed" };
                row.add_suffix(&gtk::Label::builder().label(&tag).css_classes([class, "caption-heading"]).valign(gtk::Align::Center).build());
            }
            if count > 0 {
                row.add_suffix(
                    &gtk::Label::builder()
                        .label(ngettext("{} game", "{} games", count as u32).replace("{}", &count.to_string()))
                        .css_classes(["dimmed"])
                        .build(),
                );
            }
            row.add_suffix(&chevron());
            page.keep(&id, &row);
            let (dialog, runner_ref, list, win, connector) =
                (dialog.downgrade(), runner.clone(), games.remove(&id).unwrap_or_default(), win.downgrade(), connector.clone());
            row.connect_activated(move |_| {
                let (Some(dialog), Some(win)) = (dialog.upgrade(), win.upgrade()) else { return };
                open_runner(&dialog, &runner_ref, &list, &win, &connector);
            });
            if found(&runner) {
                installed.add(&row);
            } else if own.iter().any(|c| !c["latest"].is_null()) {
                waiting.push((!own.iter().any(|c| c["proposal"] == "install"), row));
            } else {
                missing.add(&row);
                none += 1;
            }
        }
        waiting.sort_by_key(|(later, _)| *later);
        for (_, row) in &waiting {
            offered.add(row);
        }
        let tooling = crate::components::tools(&listed);
        for c in &tooling {
            let row = crate::components::row(c, again.clone());
            page.keep(&text(c, "id"), &row);
            tools.add(&row);
        }
        offered.set_visible(!waiting.is_empty());
        missing.set_visible(none > 0);
        tools.set_visible(!tooling.is_empty());
        crate::components::refocus(&page.rows.borrow(), focus);
    });
}

fn open_runner(dialog: &adw::PreferencesDialog, runner: &Value, games: &[(String, String, f64)], win: &Window, connector: &str) {
    let (id, kind) = (text(runner, "id"), text(runner, "kind"));
    let view = FormView::new(Form::Runner(id.clone()), dialog, connector.to_string());
    let path = text(runner, "path");
    let description = match (kind.as_str(), path.is_empty(), text(runner, "source").as_str()) {
        ("linux", _, _) => gettext("Starts native Linux games as they are."),
        (_, true, _) => gettext("Not found: set its program below, or install it under Builds."),
        (_, false, "config") => gettext("Set to {}").replace("{}", &path),
        (_, false, _) => gettext("Found at {}").replace("{}", &path),
    };
    view.page.set_description(&description);
    if kind != "linux" {
        // Proton's form has a Builds card of its own, the default build: its builds go right under it.
        let title = if kind == "proton" { String::new() } else { gettext("Builds") };
        view.add_head(&crate::components::group(&title, move |listed| crate::components::of_runner(&id, &kind, listed)), "Builds");
    }
    if !games.is_empty() {
        let group = adw::PreferencesGroup::builder().title(gettext("Games")).build();
        let mut games = games.to_vec();
        games.sort_by_key(|(_, title, _)| crate::library::fold(title));
        for (id, title, hours) in games {
            let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), title, crate::format::played(hours));
            row.add_suffix(&chevron());
            let (dialog_ref, win) = (dialog.downgrade(), win.downgrade());
            row.connect_activated(move |_| {
                let (Some(dialog), Some(win)) = (dialog_ref.upgrade(), win.upgrade()) else { return };
                dialog.close();
                if let Some(game) = win.app().library().get(&id) {
                    win.open_game(&game);
                }
            });
            group.add(&row);
        }
        view.add_tail(&group);
    }
    push_form(dialog, &text(runner, "name"), view);
}

fn load_stores(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, connector: &str) {
    page.loading();
    let (dialog, page, connector) = (dialog.downgrade(), page.clone(), connector.to_string());
    glib::spawn_future_local(async move {
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let Some(dialog) = dialog.upgrade() else { return };
        page.clear();
        let group = page.group("", &gettext("Where your games come from: sign in to list them and install them from Store"));
        for source in sources {
            let id = text(&source, "id");
            let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), text(&source, "name"), signin::status(&source));
            let switch = gtk::Switch::builder().valign(gtk::Align::Center).active(source["enabled"].as_bool() == Some(true)).build();
            switch.set_sensitive(source["available"].as_bool() != Some(false) || switch.is_active());
            let (weak_dialog, weak_page, sid, conn) = (dialog.downgrade(), Rc::downgrade(&page), id.clone(), connector.clone());
            switch.connect_active_notify(move |switch| {
                let on = switch.is_active();
                let (dialog, page, sid, conn) = (weak_dialog.clone(), weak_page.clone(), sid.clone(), conn.clone());
                glib::spawn_future_local(async move {
                    let result = backend::call(move |core| async move { core.enable_source(&sid, on).await }).await;
                    let (Some(dialog), Some(page)) = (dialog.upgrade(), page.upgrade()) else { return };
                    if let Err(e) = result {
                        dialog.add_toast(crate::dialogs::toast(&e.to_string()));
                    }
                    load_stores(&dialog, &page, &conn);
                });
            });
            row.add_suffix(&switch);
            row.add_suffix(&chevron());
            let (weak_dialog, weak_page, source, conn) = (dialog.downgrade(), Rc::downgrade(&page), source.clone(), connector.clone());
            row.connect_activated(move |_| {
                let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) else { return };
                open_store(&dialog, &page, &source, &conn);
            });
            group.add(&row);
        }
    });
}

fn open_store(dialog: &adw::PreferencesDialog, stores: &Rc<ListPage>, source: &Value, connector: &str) {
    let id = text(source, "id");
    let view = FormView::new(Form::Source(id.clone()), dialog, connector.to_string());
    view.page.set_description(&text(source, "description"));
    if source["enabled"].as_bool() == Some(true) {
        let weak = dialog.downgrade();
        let say: signin::Say = Rc::new(move |text: &str| {
            if let Some(dialog) = weak.upgrade() {
                dialog.add_toast(crate::dialogs::toast(text));
            }
        });
        let (weak, stores, connector_owned) = (dialog.downgrade(), Rc::downgrade(stores), connector.to_string());
        let reload: signin::Say = Rc::new(move |_: &str| {
            if let (Some(dialog), Some(stores)) = (weak.upgrade(), stores.upgrade()) {
                load_stores(&dialog, &stores, &connector_owned);
            }
        });
        view.add_tail(&signin::account_group(source, say, reload));
    }
    push_form(dialog, &text(source, "name"), view);
}

fn module_status(module: &Value) -> String {
    let (enabled, available) = (module["enabled"].as_bool() == Some(true), module["available"].as_bool() != Some(false));
    let missing = list(module, "missing").join(", ");
    let unset = list(module, "unset");
    match (enabled, available) {
        (true, false) => gettext("On, but skipped: missing {}").replace("{}", &missing),
        (false, false) => gettext("Needs {}").replace("{}", &missing),
        (true, true) if !unset.is_empty() => gettext("Waiting on {}").replace("{}", &unset.join(", ")),
        _ => text(module, "description"),
    }
}

fn load_modules(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, connector: &str) {
    page.loading();
    let (dialog, page, connector) = (dialog.downgrade(), page.clone(), connector.to_string());
    glib::spawn_future_local(async move {
        let modules = backend::run(async { backend::core().modules().await }).await;
        let Some(dialog) = dialog.upgrade() else { return };
        page.clear();
        let (on, off) =
            (page.group("", &gettext("What runs around your games: recording, the journal, screenshots and more")), page.group(&gettext("Off"), ""));
        for module in modules {
            let id = text(&module, "id");
            let enabled = module["enabled"].as_bool() == Some(true);
            let row = crate::rows::plain(adw::ActionRow::builder().subtitle_lines(2).activatable(true).build(), text(&module, "name"), module_status(&module));
            let switch = gtk::Switch::builder().valign(gtk::Align::Center).active(enabled).build();
            switch.set_sensitive(module["available"].as_bool() != Some(false) || enabled);
            let (weak_dialog, weak_page, mid, conn) = (dialog.downgrade(), Rc::downgrade(&page), id.clone(), connector.clone());
            switch.connect_active_notify(move |switch| {
                let on = switch.is_active();
                let (dialog, page, mid, conn) = (weak_dialog.clone(), weak_page.clone(), mid.clone(), conn.clone());
                glib::spawn_future_local(async move {
                    let result = backend::call(move |core| async move { core.enable_module(&mid, on).await }).await;
                    let (Some(dialog), Some(page)) = (dialog.upgrade(), page.upgrade()) else { return };
                    if let Err(e) = result {
                        dialog.add_toast(crate::dialogs::toast(&e.to_string()));
                    }
                    load_modules(&dialog, &page, &conn);
                });
            });
            row.add_suffix(&switch);
            row.add_suffix(&chevron());
            let (weak_dialog, module, conn) = (dialog.downgrade(), module.clone(), connector.clone());
            row.connect_activated(move |_| {
                let Some(dialog) = weak_dialog.upgrade() else { return };
                let view = FormView::new(Form::Module(text(&module, "id")), &dialog, conn.clone());
                view.page.set_description(&text(&module, "description"));
                push_form(&dialog, &text(&module, "name"), view);
            });
            if enabled {
                on.add(&row);
            } else {
                off.add(&row);
            }
        }
        off.set_visible(off.first_child().is_some());
    });
}

/// The library's art: how many games miss some, a fetch for what is missing or for everything again, each game to fix.
fn load_artwork(page: &Rc<ListPage>, win: &Window) {
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
                row.add_suffix(&chevron());
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

/// The running art fetch: how far along, and a way to stop it after the game in hand.
fn job_row(job: &Job) -> adw::ActionRow {
    let row = crate::rows::plain(adw::ActionRow::builder().build(), job.label(), "");
    let bar = gtk::ProgressBar::builder().valign(gtk::Align::Center).width_request(120).build();
    let stop = gtk::Button::builder().label(gettext("_Stop")).use_underline(true).valign(gtk::Align::Center).build();
    row.add_suffix(&bar);
    row.add_suffix(&stop);
    let weak = job.downgrade();
    stop.connect_clicked(move |button| {
        if let Some(job) = weak.upgrade() {
            job.cancel();
            button.set_sensitive(false);
        }
    });
    let (weak_row, weak_bar) = (row.downgrade(), bar.downgrade());
    let sync = move |job: &Job| {
        let (Some(row), Some(bar)) = (weak_row.upgrade(), weak_bar.upgrade()) else { return };
        bar.set_fraction(job.fraction());
        let mut line = vec![job.message()];
        if job.total() > 0 {
            line.push(gettext("{} of {}").replacen("{}", &(job.done() + 1).to_string(), 1).replacen("{}", &job.total().to_string(), 1));
        }
        if job.cancelled() {
            line = vec![gettext("Stopping after this game…")];
        }
        line.retain(|part| !part.is_empty());
        row.set_subtitle(&line.join(" · "));
    };
    sync(job);
    for name in ["message", "done", "total", "cancelled"] {
        let sync = sync.clone();
        job.connect_notify_local(Some(name), move |job, _| sync(job));
    }
    row
}

fn confirm_fetch_all(win: &Window) {
    let dialog = adw::AlertDialog::new(
        Some(&gettext("Fetch All the Art Again?")),
        Some(&gettext("Every fetched picture is replaced by SteamGridDB’s best one. The pictures you picked stay.")),
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

/// Installs what fixes a check, asking first, then checks again once the install ends.
fn install_button(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, component: &str) -> gtk::Button {
    let button = gtk::Button::builder().label(gettext("_Install…")).use_underline(true).valign(gtk::Align::Center).css_classes(["suggested-action"]).build();
    let (weak_dialog, weak_page, id) = (dialog.downgrade(), Rc::downgrade(page), component.to_string());
    button.connect_clicked(move |button| {
        let (weak_dialog, weak_page, weak_button, id) = (weak_dialog.clone(), weak_page.clone(), button.downgrade(), id.clone());
        glib::spawn_future_local(async move {
            let listed = crate::components::listing(false).await.unwrap_or_default();
            let (Some(button), Some(c)) = (weak_button.upgrade(), listed.iter().find(|c| text(c, "id") == id)) else { return };
            let checked: Rc<dyn Fn()> = Rc::new(move || {
                if let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) {
                    load_doctor(&dialog, &page);
                }
            });
            crate::components::act(&button, c, "install", checked);
        });
    });
    button
}

/// What Universe needs from this machine, the checks that fail first with what to do about them.
fn load_doctor(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>) {
    page.loading();
    let (dialog, page) = (dialog.downgrade(), page.clone());
    glib::spawn_future_local(async move {
        let (checks, names, gnome) = backend::pinned(|core| async move {
            let mut names: HashMap<String, String> = HashMap::new();
            for entry in core.modules().await.into_iter().chain(core.sources().await) {
                names.insert(text(&entry, "id"), text(&entry, "name"));
            }
            (core.doctor().await, names, core.desktop().await == universe::desktop::Profile::Gnome)
        })
        .await;
        let Some(dialog) = dialog.upgrade() else { return };
        page.clear();
        let area = |module: &str| match module {
            "" | "core" => gettext("Core"),
            "runners" => gettext("Runners"),
            "media" => gettext("Media"),
            "controller" => gettext("Controller"),
            other => names.get(other).cloned().unwrap_or_else(|| other.to_string()),
        };
        let failing: Vec<&universe::doctor::Check> = checks.iter().filter(|c| !c.ok).collect();
        let top = page.group(
            "",
            &if failing.is_empty() {
                gettext("Every check passes")
            } else {
                ngettext("{} of {} checks fails", "{} of {} checks fail", failing.len() as u32).replacen("{}", &failing.len().to_string(), 1).replacen(
                    "{}",
                    &checks.len().to_string(),
                    1,
                )
            },
        );
        let again = adw::ButtonRow::builder().title(gettext("Check Again")).start_icon_name("view-refresh-symbolic").build();
        let (weak_dialog, weak_page) = (dialog.downgrade(), Rc::downgrade(&page));
        again.connect_activated(move |_| {
            if let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) {
                load_doctor(&dialog, &page);
            }
        });
        top.add(&again);
        if gnome {
            if let Some(check) = checks.iter().find(|c| c.check == "universe-extension") {
                let group = page.group(&gettext("System Setup"), &gettext("The GNOME Shell extension finds, focuses and captures game windows"));
                let row = crate::rows::plain(adw::ActionRow::builder().build(), check.label.clone(), check.detail.clone());
                row.add_prefix(&gtk::Image::from_icon_name(if check.ok { "object-select-symbolic" } else { "dialog-warning-symbolic" }));
                if !check.ok && (check.detail.starts_with("not installed") || check.detail.contains("not enabled")) {
                    let install = gtk::Button::builder()
                        .label(gettext("_Set Up"))
                        .use_underline(true)
                        .valign(gtk::Align::Center)
                        .css_classes(["suggested-action"])
                        .build();
                    let (weak_dialog, weak_page) = (dialog.downgrade(), Rc::downgrade(&page));
                    install.connect_clicked(move |button| {
                        button.set_sensitive(false);
                        let (weak_dialog, weak_page) = (weak_dialog.clone(), weak_page.clone());
                        glib::spawn_future_local(async move {
                            let result = backend::run(async {
                                use universe::desktop::gnome::{self, ExtensionCopy};
                                let copied = match gnome::install_extension() {
                                    Ok(ExtensionCopy::Written(_)) => gettext("Installed"),
                                    Ok(ExtensionCopy::Current) => gettext("Up to date"),
                                    Ok(ExtensionCopy::System) => gettext("Provided by the system"),
                                    Err(e) => return Err(e.to_string()),
                                };
                                gnome::enable_extension().await.map(|_| copied)
                            })
                            .await;
                            let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) else { return };
                            let line = match result {
                                Ok(copied) => gettext("{}: the extension loads at the next login").replace("{}", &copied),
                                Err(e) => e,
                            };
                            dialog.add_toast(crate::dialogs::toast(&line));
                            load_doctor(&dialog, &page);
                        });
                    });
                    row.add_suffix(&install);
                }
                group.add(&row);
            }
        }
        if !failing.is_empty() {
            let group = page.group(&gettext("Needs Attention"), "");
            for check in &failing {
                let row = crate::rows::plain_expander(
                    adw::ExpanderRow::builder().build(),
                    check.label.clone(),
                    format!("{} · {}", area(&check.module), check.detail),
                );
                let icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
                icon.add_css_class("warning");
                row.add_prefix(&icon);
                if !check.fix.is_empty() {
                    let fix = crate::rows::plain(adw::ActionRow::builder().subtitle_selectable(true).build(), gettext("What to do"), &check.fix);
                    row.add_row(&fix);
                }
                if !check.component.is_empty() {
                    row.add_suffix(&install_button(&dialog, &page, &check.component));
                }
                group.add(&row);
            }
        }
        let mut areas: Vec<String> = Vec::new();
        for check in &checks {
            let name = area(&check.module);
            if !areas.contains(&name) {
                areas.push(name);
            }
        }
        let core = gettext("Core");
        areas.sort_by_key(|name| *name != core);
        for name in areas {
            let passing: Vec<&universe::doctor::Check> = checks.iter().filter(|c| c.ok && area(&c.module) == name).collect();
            if passing.is_empty() {
                continue;
            }
            let total = checks.iter().filter(|c| area(&c.module) == name).count();
            let group = page.group(&name, &gettext("{} of {} pass").replacen("{}", &passing.len().to_string(), 1).replacen("{}", &total.to_string(), 1));
            for check in passing {
                let row = crate::rows::plain(adw::ActionRow::builder().build(), check.label.clone(), check.detail.clone());
                let icon = gtk::Image::from_icon_name("object-select-symbolic");
                icon.add_css_class("success");
                row.add_prefix(&icon);
                group.add(&row);
            }
        }
    });
}
