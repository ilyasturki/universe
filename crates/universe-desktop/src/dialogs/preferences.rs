use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use serde_json::Value;
use universe::forms::Form;

use crate::backend;
use crate::form_view::FormView;
use crate::qr;
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
}

impl ListPage {
    fn new(name: &str, title: &str, icon: &str) -> Rc<ListPage> {
        Rc::new(ListPage { page: adw::PreferencesPage::builder().name(name).title(title).icon_name(icon).build(), groups: RefCell::default() })
    }

    fn clear(&self) {
        for group in self.groups.take() {
            self.page.remove(&group);
        }
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

/// `page`: `launch`, `runners`, `stores` or `modules`; empty for the first.
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
    load_runners(&dialog, &runners, win, &connector);

    let stores = ListPage::new("stores", &gettext("Stores"), "system-software-install-symbolic");
    dialog.add(&stores.page);
    load_stores(&dialog, &stores, &connector);

    let modules = ListPage::new("modules", &gettext("Modules"), "package-x-generic-symbolic");
    dialog.add(&modules.page);
    load_modules(&dialog, &modules, &connector);

    dialog.connect_closed(move |_| {
        let _ = (&launch, &runners, &stores, &modules);
    });
    if !page.is_empty() {
        dialog.set_visible_page_name(page);
    }
    dialog.present(Some(win));
}

fn load_runners(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, win: &Window, connector: &str) {
    page.loading();
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
        let (Some(dialog), Some(win)) = (dialog.upgrade(), win.upgrade()) else { return };
        page.clear();
        let found = |r: &Value| text(r, "kind") == "linux" || !text(r, "path").is_empty();
        let mut sorted = runners;
        sorted.sort_by(|a, b| {
            let (ua, ub) = (usage.get(&text(a, "id")).copied().unwrap_or_default(), usage.get(&text(b, "id")).copied().unwrap_or_default());
            found(b).cmp(&found(a)).then(ub.0.cmp(&ua.0)).then(ub.1.total_cmp(&ua.1)).then(text(a, "name").to_lowercase().cmp(&text(b, "name").to_lowercase()))
        });
        let (installed, missing) = (page.group("", &gettext("The programs your games start through")), page.group(&gettext("Not Found"), ""));
        for runner in sorted {
            let id = text(&runner, "id");
            let count = usage.get(&id).map(|u| u.0).unwrap_or(0);
            let platforms = list(&runner, "platforms").iter().map(|p| crate::library::platform_name(p)).collect::<Vec<_>>().join(", ");
            let row = adw::ActionRow::builder().title(text(&runner, "name")).subtitle(platforms).use_markup(false).activatable(true).build();
            if count > 0 {
                row.add_suffix(
                    &gtk::Label::builder()
                        .label(ngettext("{} game", "{} games", count as u32).replace("{}", &count.to_string()))
                        .css_classes(["dimmed"])
                        .build(),
                );
            }
            row.add_suffix(&chevron());
            let is_found = found(&runner);
            let (dialog, runner, list, win, connector) =
                (dialog.downgrade(), runner.clone(), games.remove(&id).unwrap_or_default(), win.downgrade(), connector.clone());
            row.connect_activated(move |_| {
                let (Some(dialog), Some(win)) = (dialog.upgrade(), win.upgrade()) else { return };
                open_runner(&dialog, &runner, &list, &win, &connector);
            });
            if is_found {
                installed.add(&row);
            } else {
                missing.add(&row);
            }
        }
        missing.set_visible(missing.first_child().is_some());
    });
}

fn open_runner(dialog: &adw::PreferencesDialog, runner: &Value, games: &[(String, String, f64)], win: &Window, connector: &str) {
    let id = text(runner, "id");
    let view = FormView::new(Form::Runner(id), dialog, connector.to_string());
    let path = text(runner, "path");
    let description = match (text(runner, "kind").as_str(), path.is_empty(), text(runner, "source").as_str()) {
        ("linux", _, _) => gettext("Starts native Linux games as they are."),
        (_, true, _) => gettext("Not found: set its program below."),
        (_, false, "config") => gettext("Set to {}").replace("{}", &path),
        (_, false, _) => gettext("Found at {}").replace("{}", &path),
    };
    view.page.set_description(&description);
    if !games.is_empty() {
        let group = adw::PreferencesGroup::builder().title(gettext("Games")).build();
        let mut games = games.to_vec();
        games.sort_by_key(|(_, title, _)| crate::library::fold(title));
        for (id, title, hours) in games {
            let row = adw::ActionRow::builder().title(title).subtitle(crate::format::played(hours)).use_markup(false).activatable(true).build();
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

fn source_status(source: &Value) -> String {
    let missing = list(source, "missing");
    if source["available"].as_bool() == Some(false) {
        return gettext("Missing {}").replace("{}", &missing.join(", "));
    }
    if source["enabled"].as_bool() != Some(true) {
        return gettext("Off");
    }
    match (source["logged_in"].as_bool() == Some(true), text(source, "user")) {
        (true, user) if !user.is_empty() => gettext("Signed in as {}").replace("{}", &user),
        (true, _) => gettext("Signed in"),
        (false, _) => gettext("Not signed in"),
    }
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
            let row = adw::ActionRow::builder().title(text(&source, "name")).subtitle(source_status(&source)).use_markup(false).activatable(true).build();
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
                        dialog.add_toast(adw::Toast::new(&e.to_string()));
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
        view.add_tail(&account_group(dialog, stores, source, connector));
    }
    push_form(dialog, &text(source, "name"), view);
}

/// Sign-in: the store's link to open here or on a phone, and the code it shows after.
fn account_group(dialog: &adw::PreferencesDialog, stores: &Rc<ListPage>, source: &Value, connector: &str) -> adw::PreferencesGroup {
    let id = text(source, "id");
    let group = adw::PreferencesGroup::builder().title(gettext("Account")).build();
    let status = adw::ActionRow::builder().title(source_status(source)).use_markup(false).build();
    status.add_prefix(&gtk::Image::from_icon_name("avatar-default-symbolic"));
    group.add(&status);
    let get_link = adw::ButtonRow::builder().title(gettext("Get a Sign-In Link")).start_icon_name("web-browser-symbolic").build();
    group.add(&get_link);
    let link = adw::ActionRow::builder().title(gettext("Open the link, sign in, then enter the code it shows")).use_markup(false).visible(false).build();
    let open = gtk::Button::builder().label(gettext("Open")).valign(gtk::Align::Center).build();
    link.add_suffix(&open);
    group.add(&link);
    let qr = gtk::Picture::builder().can_shrink(false).halign(gtk::Align::Center).margin_top(12).margin_bottom(12).visible(false).build();
    qr.set_tooltip_text(Some(&gettext("Scan it to sign in from a phone")));
    group.add(&qr);
    let code = adw::EntryRow::builder().title(gettext("Code")).show_apply_button(true).visible(false).build();
    group.add(&code);

    let url = Rc::new(RefCell::new(String::new()));
    let (weak_dialog, sid) = (dialog.downgrade(), id.clone());
    let (link_row, qr_pic, code_row, url_cell) = (link.clone(), qr.clone(), code.clone(), url.clone());
    get_link.connect_activated(move |row| {
        row.set_sensitive(false);
        let (dialog, sid, row) = (weak_dialog.clone(), sid.clone(), row.clone());
        let (link_row, qr_pic, code_row, url_cell) = (link_row.clone(), qr_pic.clone(), code_row.clone(), url_cell.clone());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.source_login_url(&sid).await }).await;
            row.set_sensitive(true);
            match result {
                Ok(url) => {
                    link_row.set_subtitle(&url);
                    link_row.set_visible(true);
                    qr_pic.set_paintable(qr::texture(&url, 4).as_ref());
                    qr_pic.set_visible(true);
                    code_row.set_visible(true);
                    url_cell.replace(url);
                }
                Err(e) => {
                    if let Some(dialog) = dialog.upgrade() {
                        dialog.add_toast(adw::Toast::new(&e.to_string()));
                    }
                }
            }
        });
    });
    open.connect_clicked(move |button| {
        let launcher = gtk::UriLauncher::new(&url.borrow());
        launcher.launch(button.root().and_downcast::<gtk::Window>().as_ref(), gio::Cancellable::NONE, |_| {});
    });
    let (weak_dialog, weak_stores, conn) = (dialog.downgrade(), Rc::downgrade(stores), connector.to_string());
    code.connect_apply(move |row| {
        let (entered, sid) = (row.text().trim().to_string(), id.clone());
        if entered.is_empty() {
            return;
        }
        row.set_sensitive(false);
        let (dialog, stores, row, status, conn) = (weak_dialog.clone(), weak_stores.clone(), row.clone(), status.clone(), conn.clone());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.source_login(&sid, &entered).await }).await;
            row.set_sensitive(true);
            let Some(dialog) = dialog.upgrade() else { return };
            match result {
                Ok(user) => {
                    let title = if user.is_empty() { gettext("Signed in") } else { gettext("Signed in as {}").replace("{}", &user) };
                    status.set_title(&title);
                    dialog.add_toast(adw::Toast::new(&title));
                    if let Some(stores) = stores.upgrade() {
                        load_stores(&dialog, &stores, &conn);
                    }
                }
                Err(e) => dialog.add_toast(adw::Toast::new(&e.to_string())),
            }
        });
    });
    group
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
            let row = adw::ActionRow::builder()
                .title(text(&module, "name"))
                .subtitle(module_status(&module))
                .subtitle_lines(2)
                .use_markup(false)
                .activatable(true)
                .build();
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
                        dialog.add_toast(adw::Toast::new(&e.to_string()));
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
