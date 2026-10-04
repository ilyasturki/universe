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
use crate::window::Window;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn list(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// A page of rows read from the core, cleared and filled again whenever it changes.
pub struct ListPage {
    pub page: adw::PreferencesPage,
    groups: RefCell<Vec<adw::PreferencesGroup>>,
    rows: RefCell<Vec<crate::components::Keyed>>,
}

impl ListPage {
    pub fn new(name: &str, title: &str, icon: &str) -> Rc<ListPage> {
        Rc::new(ListPage {
            page: adw::PreferencesPage::builder().name(name).title(title).icon_name(icon).build(),
            groups: RefCell::default(),
            rows: RefCell::default(),
        })
    }

    pub fn clear(&self) {
        self.rows.take();
        for group in self.groups.take() {
            self.page.remove(&group);
        }
    }

    /// `row` found again by `key` after a rebuild, the focus with it.
    fn keep(&self, key: &str, row: &impl IsA<gtk::Widget>) {
        self.rows.borrow_mut().push((key.to_string(), row.clone().upcast()));
    }

    pub fn group(&self, title: &str, description: &str) -> adw::PreferencesGroup {
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

    pub fn loading(&self) {
        self.clear();
        let group = self.group("", "");
        group.add(&adw::Spinner::builder().height_request(48).margin_top(24).build());
    }
}

/// A subpage of the dialog around a form: a header bar, the form's page, the view kept alive with the page.
fn push_form(dialog: &adw::PreferencesDialog, title: &str, view: Rc<FormView>) -> adw::NavigationPage {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&view.page));
    let page = adw::NavigationPage::new(&toolbar, title);
    view.load();
    page.connect_hidden(move |_| {
        let _ = &view;
    });
    dialog.push_subpage(&page);
    page
}

/// A list row that opens a subpage: its state beside the chevron when it is off, the switch on the subpage.
fn list_row(title: &str, subtitle: &str, enabled: bool) -> adw::ActionRow {
    let row = crate::rows::marked(adw::ActionRow::builder().activatable(true).build(), title, subtitle);
    if !enabled {
        row.add_suffix(&gtk::Label::builder().label(gettext("Off")).valign(gtk::Align::Center).css_classes(["dimmed"]).build());
    }
    row.add_suffix(&chevron());
    row
}

fn chevron() -> gtk::Image {
    gtk::Image::from_icon_name("go-next-symbolic")
}

/// `page`: `launch`, `runners`, `stores`, `modules`, `controller` or `system` (a Steam Deck's); empty for the first. `storage`,
/// `artwork` and `doctor` open the dialogs those pages became.
pub fn present(win: &Window, page: &str) {
    match page {
        "storage" => return crate::pages::storage::present(win),
        "artwork" => return crate::dialogs::library_artwork::present(win),
        "doctor" => return crate::dialogs::system_check::present(win),
        _ => {}
    }
    let dialog = adw::PreferencesDialog::builder().search_enabled(true).content_width(800).content_height(720).build();
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
    load_stores(&dialog, &stores, &connector, false);

    let modules = ListPage::new("modules", &gettext("Modules"), "package-x-generic-symbolic");
    dialog.add(&modules.page);
    load_modules(&dialog, &modules, &connector, false);

    let controller = crate::dialogs::controller::page(&dialog);
    dialog.add(&controller.page);

    let system = universe::deck::model().is_some().then(|| {
        let page = ListPage::new("system", &gettext("System"), "computer-symbolic");
        dialog.add(&page.page);
        load_system(&dialog, &page);
        page
    });

    let (weak_dialog, weak_runners, weak_win, runners_connector) = (dialog.downgrade(), Rc::downgrade(&runners), win.downgrade(), connector.clone());
    let component_job = crate::components::on_jobs(&win.app(), move || {
        if let (Some(dialog), Some(runners), Some(win)) = (weak_dialog.upgrade(), weak_runners.upgrade(), weak_win.upgrade()) {
            load_runners(&dialog, &runners, &win, &runners_connector, true);
        }
    });

    let (app, job) = (win.app().downgrade(), RefCell::new(Some(component_job)));
    dialog.connect_closed(move |_| {
        let _ = (&launch, &runners, &stores, &modules, &controller, &system);
        if let (Some(app), Some(handler)) = (app.upgrade(), job.take()) {
            app.disconnect(handler);
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
            if let Some((component, action)) = crate::components::quick(&own) {
                let label = if action.id == "update" { gettext("_Update") } else { gettext("_Install…") };
                let button = gtk::Button::builder().label(label).use_underline(true).valign(gtk::Align::Center).tooltip_text(&action.label).build();
                let (component, again) = (component.clone(), again.clone());
                button.connect_clicked(move |button| crate::components::act(button, &component, &action.id, again.clone()));
                row.add_suffix(&button);
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

/// `reload`: the rows rebuilt in one go where they were, the focus kept, no spinner; their switch is on their page.
fn load_stores(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, connector: &str, reload: bool) {
    if !reload {
        page.loading();
    }
    let (dialog, page, connector) = (dialog.downgrade(), page.clone(), connector.to_string());
    glib::spawn_future_local(async move {
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let Some(dialog) = dialog.upgrade() else { return };
        let focus = crate::components::take_focus(&page.rows.borrow());
        page.clear();
        let group = page.group("", &gettext("Where your games come from: sign in to list them and install them from Store"));
        for source in sources {
            let enabled = source["enabled"].as_bool() == Some(true);
            let line = if enabled || source["available"].as_bool() == Some(false) { signin::status(&source) } else { text(&source, "description") };
            let row = list_row(&text(&source, "name"), &line, enabled);
            page.keep(&text(&source, "id"), &row);
            let (weak_dialog, weak_page, source, conn) = (dialog.downgrade(), Rc::downgrade(&page), source.clone(), connector.clone());
            row.connect_activated(move |_| {
                let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) else { return };
                open_store(&dialog, &page, &source, &conn);
            });
            group.add(&row);
        }
        crate::components::refocus(&page.rows.borrow(), focus);
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
                load_stores(&dialog, &stores, &connector_owned, true);
            }
        });
        view.add_tail(&signin::account_group(source, say, reload));
    }
    let (weak, stores, connector) = (dialog.downgrade(), Rc::downgrade(stores), connector.to_string());
    push_form(dialog, &text(source, "name"), view).connect_hidden(move |_| {
        if let (Some(dialog), Some(stores)) = (weak.upgrade(), stores.upgrade()) {
            load_stores(&dialog, &stores, &connector, true);
        }
    });
}

/// What a module's row says: why it does nothing, else what it does. A setting it waits on goes by its label.
fn module_status(module: &Value) -> String {
    let (enabled, available) = (module["enabled"].as_bool() == Some(true), module["available"].as_bool() != Some(false));
    let missing = list(module, "missing").join(", ");
    let settings = module["settings"].as_array().cloned().unwrap_or_default();
    let label = |key: &String| settings.iter().find(|s| text(s, "key") == *key).map(|s| text(s, "label")).filter(|l| !l.is_empty()).unwrap_or(key.clone());
    let unset: Vec<String> = list(module, "unset").iter().map(label).collect();
    let incompatible = text(module, "incompatible");
    match (enabled, available) {
        (true, false) if !incompatible.is_empty() => gettext("On, but skipped: {}").replace("{}", &incompatible),
        (false, false) if !incompatible.is_empty() => incompatible,
        (true, false) => gettext("On, but skipped: missing {}").replace("{}", &missing),
        (false, false) => gettext("Needs {}").replace("{}", &missing),
        (true, true) if !unset.is_empty() => gettext("Does nothing until it is set: {}").replace("{}", &unset.join(", ")),
        _ => text(module, "description"),
    }
}

/// `reload`: the rows rebuilt in one go where they were, the focus kept, no spinner; their switch is on their page.
fn load_modules(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, connector: &str, reload: bool) {
    if !reload {
        page.loading();
    }
    let (dialog, page, connector) = (dialog.downgrade(), page.clone(), connector.to_string());
    glib::spawn_future_local(async move {
        let modules = backend::run(async { backend::core().modules().await }).await;
        let Some(dialog) = dialog.upgrade() else { return };
        let focus = crate::components::take_focus(&page.rows.borrow());
        page.clear();
        let group = page.group("", &gettext("What runs around your games: recording, the journal, screenshots and more"));
        for module in modules {
            let row = list_row(&text(&module, "name"), &module_status(&module), module["enabled"].as_bool() == Some(true));
            page.keep(&text(&module, "id"), &row);
            let (weak_dialog, weak_page, module, conn) = (dialog.downgrade(), Rc::downgrade(&page), module.clone(), connector.clone());
            row.connect_activated(move |_| {
                let Some(dialog) = weak_dialog.upgrade() else { return };
                let view = FormView::new(Form::Module(text(&module, "id")), &dialog, conn.clone());
                view.page.set_description(&text(&module, "description").replace('`', ""));
                let (weak_dialog, weak_page, conn) = (dialog.downgrade(), weak_page.clone(), conn.clone());
                push_form(&dialog, &text(&module, "name"), view).connect_hidden(move |_| {
                    if let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) {
                        load_modules(&dialog, &page, &conn, true);
                    }
                });
            });
            group.add(&row);
        }
        crate::components::refocus(&page.rows.borrow(), focus);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_waiting_on_a_setting_names_it_by_its_label() {
        let module = serde_json::json!({
            "id": "journal", "enabled": true, "available": true, "unset": ["provider", "other"], "description": "Writes the `journal`.",
            "settings": [{"key": "enabled", "label": "Enabled"}, {"key": "provider", "label": "Writing model"}],
        });
        let status = module_status(&module);
        assert!(status.contains("Writing model") && !status.contains("provider"), "{status}");
        assert!(status.contains("other"), "a key without a label stays as written");
        let set = serde_json::json!({"enabled": true, "available": true, "unset": [], "description": "Writes the `journal`."});
        assert_eq!(module_status(&set), "Writes the `journal`.", "the row draws its Markdown");
    }
}
