use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use universe::forms::{self, Field, Form, Origin};

use crate::backend;

/// The title a form's card takes; the core names them in English.
pub fn section_title(section: &str) -> String {
    match section {
        "Display" => gettext("Display"),
        "Overlay" => gettext("Overlay"),
        "Scaling" => gettext("Scaling"),
        "Environment" => gettext("Environment"),
        "Launch" => gettext("Launch"),
        "Proton" => gettext("Proton"),
        "Sync" => gettext("Sync"),
        "Upscaling" => gettext("Upscaling"),
        "Logs" => gettext("Logs"),
        "Programs" => gettext("Programs"),
        "Folders" => gettext("Folders"),
        "API keys" => gettext("API Keys"),
        "Desktop" => gettext("Desktop"),
        "Proton builds" => gettext("Proton Builds"),
        "Desktop and library" => gettext("Desktop and Library"),
        "Artwork" => gettext("Artwork"),
        "Runner" => gettext("Runner"),
        "Options" => gettext("Options"),
        "Settings" => gettext("Settings"),
        other => other.to_string(),
    }
}

fn origin_word(origin: Origin) -> String {
    match origin {
        Origin::Game => gettext("This Game"),
        Origin::Runner => gettext("Runner"),
        Origin::Global => gettext("Global"),
        Origin::Default => gettext("Default"),
        Origin::Found => gettext("Found"),
    }
}

/// A value as a row shows it: `auto` with what it comes to, words capitalised, units on numbers.
fn show(field: &Field, value: &str) -> String {
    let resolved = if value == field.value || value.is_empty() { field.resolved.as_str() } else { "" };
    let unit = |n: &str| match field.kind.as_str() {
        "refresh" => gettext("{} Hz").replace("{}", n),
        "fps" => gettext("{} FPS").replace("{}", n),
        "resolution" => n.replace('x', " × "),
        _ => n.to_string(),
    };
    let base = match value {
        "" => return resolved.to_string(),
        "auto" => gettext("Auto"),
        "on" | "true" => gettext("On"),
        "off" | "false" => gettext("Off"),
        "none" => gettext("None"),
        v if field.kind == "enum" => {
            let label = field.choices.iter().find(|c| c.value == v).map(|c| c.label.clone()).unwrap_or_else(|| v.to_string());
            let mut chars = label.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
        v => unit(v),
    };
    if resolved.is_empty() || value != "auto" {
        base
    } else {
        format!("{base} · {}", unit(resolved))
    }
}

/// What picking the first entry of an inheriting list comes to: `Global · On`.
fn inherit_label(field: &Field) -> String {
    let word = origin_word(field.fallback.unwrap_or(Origin::Default));
    let shown = show(field, &field.inherited);
    if shown.is_empty() {
        word
    } else {
        format!("{word} · {shown}")
    }
}

/// A list row's value shrinks beside a long subtitle: it shows the description's first clause, when short, the tooltip the rest.
fn first_clause(description: &str) -> String {
    let clause = description.split([';', ':', '.']).next().unwrap_or_default().trim();
    if clause.chars().count() <= 64 {
        clause.to_string()
    } else {
        String::new()
    }
}

/// Folders are picked as folders, the rest as files.
fn wants_folder(key: &str) -> bool {
    let last = key.rsplit('.').next().unwrap_or(key);
    last.ends_with("dir") || last.ends_with("root") || matches!(last, "prefix" | "working_dir")
}

enum Control {
    Switch(adw::SwitchRow),
    Combo(adw::ComboRow, RefCell<Vec<String>>),
    Entry(adw::EntryRow),
    Secret(adw::PasswordEntryRow),
    Map(adw::PreferencesGroup, RefCell<Vec<gtk::Widget>>),
}

struct RowView {
    key: String,
    control: Control,
    origin: gtk::Label,
    reset: gtk::Button,
    group: adw::PreferencesGroup,
}

/// One settings form of the core as libadwaita rows: the basic cards on `page`, the advanced ones a subpage away.
pub struct FormView {
    form: Form,
    dialog: glib::WeakRef<adw::PreferencesDialog>,
    pub page: adw::PreferencesPage,
    advanced: adw::PreferencesPage,
    rows: RefCell<Vec<RowView>>,
    fields: RefCell<Vec<Field>>,
    /// Each group with whether it sits on the advanced page.
    groups: RefCell<Vec<(adw::PreferencesGroup, bool)>>,
    advanced_page: RefCell<Option<adw::NavigationPage>>,
    /// Groups of the page's owner, kept after the form's own on every rebuild.
    tail: RefCell<Vec<adw::PreferencesGroup>>,
    syncing: Cell<bool>,
    screen: String,
    this: RefCell<Weak<FormView>>,
    runner_name: RefCell<String>,
}

impl FormView {
    pub fn new(form: Form, dialog: &adw::PreferencesDialog, screen: String) -> Rc<FormView> {
        let view = Rc::new(FormView {
            form,
            dialog: dialog.downgrade(),
            page: adw::PreferencesPage::new(),
            advanced: adw::PreferencesPage::new(),
            rows: RefCell::default(),
            fields: RefCell::default(),
            groups: RefCell::default(),
            advanced_page: RefCell::default(),
            tail: RefCell::default(),
            syncing: Cell::new(false),
            screen,
            this: RefCell::default(),
            runner_name: RefCell::default(),
        });
        view.this.replace(Rc::downgrade(&view));
        view
    }

    pub fn add_tail(&self, group: &adw::PreferencesGroup) {
        self.page.add(group);
        self.tail.borrow_mut().push(group.clone());
    }

    /// A game's Proton card is titled with its runner.
    pub fn set_runner_name(&self, name: &str) {
        self.runner_name.replace(name.to_string());
    }

    fn weak(&self) -> Weak<FormView> {
        self.this.borrow().clone()
    }

    fn toast(&self, text: &str) {
        if let Some(dialog) = self.dialog.upgrade() {
            dialog.add_toast(crate::dialogs::toast(text));
        }
    }

    pub fn load(&self) {
        let (form, screen, view) = (self.form.clone(), self.screen.clone(), self.weak());
        glib::spawn_future_local(async move {
            let fields = backend::call(move |core| async move {
                let mode = universe::desktop::screen_mode(&universe::desktop::pick_screen(&screen)).await;
                core.form(&form, mode).await
            })
            .await;
            let Some(view) = view.upgrade() else { return };
            match fields {
                Ok(fields) => view.show_fields(fields),
                Err(e) => view.toast(&e.to_string()),
            }
        });
    }

    fn show_fields(&self, fields: Vec<Field>) {
        let same = {
            let old = self.fields.borrow();
            old.len() == fields.len() && old.iter().zip(&fields).all(|(a, b)| a.key == b.key && a.kind == b.kind)
        };
        if same {
            self.syncing.set(true);
            for (row, field) in self.rows.borrow().iter().zip(&fields) {
                self.update(row, field);
            }
            self.syncing.set(false);
            self.fields.replace(fields);
        } else {
            self.fields.replace(fields.clone());
            self.build(&fields);
        }
    }

    fn section_name(&self, section: &str) -> String {
        let runner = self.runner_name.borrow();
        if section == "Proton" && !runner.is_empty() {
            runner.clone()
        } else {
            section_title(section)
        }
    }

    fn build(&self, fields: &[Field]) {
        for (group, advanced) in self.groups.take() {
            if advanced {
                self.advanced.remove(&group);
            } else {
                self.page.remove(&group);
            }
        }
        self.rows.replace(Vec::new());
        let mut groups: Vec<(String, bool, adw::PreferencesGroup)> = Vec::new();
        let mut rows = Vec::new();
        self.syncing.set(true);
        for field in fields {
            let group = if field.kind == "map" {
                let group = adw::PreferencesGroup::builder().title(field.label.clone()).description(field.description.clone()).build();
                groups.push((format!("map:{}", field.key), field.advanced, group.clone()));
                group
            } else {
                match groups.iter().find(|(s, a, _)| *s == field.section && *a == field.advanced) {
                    Some((_, _, g)) => g.clone(),
                    None => {
                        let group = adw::PreferencesGroup::new();
                        let title = self.section_name(&field.section);
                        if !title.is_empty() {
                            group.set_title(&title);
                        }
                        groups.push((field.section.clone(), field.advanced, group.clone()));
                        group
                    }
                }
            };
            let row = self.row(field, &group);
            self.update(&row, field);
            rows.push(row);
        }
        self.syncing.set(false);
        let mut all = Vec::new();
        let has_advanced = groups.iter().any(|(_, a, _)| *a);
        for (_, advanced, group) in &groups {
            if *advanced {
                self.advanced.add(group);
            } else {
                self.page.add(group);
            }
            all.push((group.clone(), *advanced));
        }
        if has_advanced {
            let group = adw::PreferencesGroup::new();
            let row = adw::ActionRow::builder()
                .title(gettext("Advanced"))
                .subtitle(gettext("Sync modes, scaling, upscalers, programs and folders"))
                .activatable(true)
                .build();
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let view = self.weak();
            row.connect_activated(move |_| {
                view.upgrade().inspect(|v| v.open_advanced());
            });
            group.add(&row);
            self.page.add(&group);
            all.push((group, false));
        }
        for group in self.tail.borrow().iter() {
            self.page.remove(group);
            self.page.add(group);
        }
        self.groups.replace(all);
        self.rows.replace(rows);
        self.fetch_dynamic(fields);
    }

    fn open_advanced(&self) {
        let Some(dialog) = self.dialog.upgrade() else { return };
        let page = self
            .advanced_page
            .borrow_mut()
            .get_or_insert_with(|| {
                let toolbar = adw::ToolbarView::new();
                toolbar.add_top_bar(&adw::HeaderBar::new());
                toolbar.set_content(Some(&self.advanced));
                adw::NavigationPage::new(&toolbar, &gettext("Advanced"))
            })
            .clone();
        dialog.push_subpage(&page);
    }

    fn row(&self, field: &Field, group: &adw::PreferencesGroup) -> RowView {
        let origin = gtk::Label::builder().valign(gtk::Align::Center).css_classes(["dimmed", "caption"]).build();
        let reset = gtk::Button::builder().icon_name("edit-undo-symbolic").valign(gtk::Align::Center).css_classes(["flat", "circular"]).build();
        let key = field.key.clone();
        let view = self.weak();
        reset.connect_clicked(move |_| {
            view.upgrade().inspect(|v| v.set(&key, ""));
        });
        let control = match field.kind.as_str() {
            "bool" => {
                let row = adw::SwitchRow::builder().title(field.label.clone()).use_markup(false).build();
                row.set_subtitle(&field.description);
                row.add_suffix(&origin);
                row.add_suffix(&reset);
                let (view, key) = (self.weak(), field.key.clone());
                row.connect_active_notify(move |row| {
                    let Some(v) = view.upgrade().filter(|v| !v.syncing.get()) else { return };
                    v.set(&key, if row.is_active() { "true" } else { "false" });
                });
                group.add(&row);
                Control::Switch(row)
            }
            "map" => Control::Map(group.clone(), RefCell::default()),
            "secret" => {
                let row = adw::PasswordEntryRow::builder().title(field.label.clone()).use_markup(false).show_apply_button(true).build();
                row.set_tooltip_text(Some(&field.description));
                row.add_suffix(&reset);
                let (view, key) = (self.weak(), field.key.clone());
                row.connect_apply(move |row| {
                    view.upgrade().inspect(|v| v.set(&key, &row.text()));
                });
                group.add(&row);
                Control::Secret(row)
            }
            kind if !field.choices.is_empty() || field.dynamic || matches!(kind, "enum" | "toggle" | "proton") => {
                let row = adw::ComboRow::builder().title(field.label.clone()).use_markup(false).build();
                row.set_subtitle(&first_clause(&field.description));
                row.set_tooltip_text(Some(field.description.as_str()).filter(|d| !d.is_empty()));
                row.add_suffix(&reset);
                let (view, key) = (self.weak(), field.key.clone());
                row.connect_selected_notify(move |row| {
                    let Some(v) = view.upgrade().filter(|v| !v.syncing.get()) else { return };
                    let value = v.rows.borrow().iter().find(|r| r.key == key).and_then(|r| match &r.control {
                        Control::Combo(_, values) => values.borrow().get(row.selected() as usize).cloned(),
                        _ => None,
                    });
                    if let Some(value) = value {
                        v.set(&key, &value);
                    }
                });
                group.add(&row);
                Control::Combo(row, RefCell::default())
            }
            kind => {
                let row = adw::EntryRow::builder().title(field.label.clone()).use_markup(false).show_apply_button(true).build();
                row.set_tooltip_text(Some(field.description.as_str()).filter(|d| !d.is_empty()));
                if kind == "int" {
                    row.set_input_purpose(gtk::InputPurpose::Digits);
                }
                row.add_suffix(&origin);
                row.add_suffix(&reset);
                if kind == "path" {
                    let pick = gtk::Button::builder()
                        .icon_name(if wants_folder(&field.key) { "folder-open-symbolic" } else { "document-open-symbolic" })
                        .tooltip_text(gettext("Choose…"))
                        .valign(gtk::Align::Center)
                        .css_classes(["flat"])
                        .build();
                    let (view, key, folder) = (self.weak(), field.key.clone(), wants_folder(&field.key));
                    pick.connect_clicked(move |button| {
                        view.upgrade().inspect(|v| v.pick_path(button, &key, folder));
                    });
                    row.add_suffix(&pick);
                }
                let (view, key) = (self.weak(), field.key.clone());
                row.connect_apply(move |row| {
                    view.upgrade().inspect(|v| v.set(&key, &row.text()));
                });
                group.add(&row);
                Control::Entry(row)
            }
        };
        RowView { key: field.key.clone(), control, origin, reset, group: group.clone() }
    }

    fn update(&self, row: &RowView, field: &Field) {
        let inheriting = field.origin.is_some() && field.own.is_empty();
        row.reset.set_visible(field.resettable);
        row.reset.set_tooltip_text(Some(&gettext("Back to {}").replace("{}", &inherit_label(field))));
        row.origin.set_visible(inheriting && field.origin != Some(Origin::Default));
        row.origin.set_label(&field.origin.map(origin_word).unwrap_or_default());
        match &row.control {
            Control::Switch(r) => r.set_active(field.value == "true"),
            Control::Secret(r) => r.set_text(&field.value),
            Control::Entry(r) => r.set_text(&field.value),
            Control::Combo(r, values) => {
                let mut list: Vec<String> = Vec::new();
                if field.origin.is_some() {
                    list.push(String::new());
                }
                list.extend(field.choices.iter().map(|c| c.value.clone()));
                let current = if inheriting { String::new() } else { field.value.clone() };
                if !list.contains(&current) {
                    list.push(current.clone());
                }
                let labels: Vec<String> =
                    list.iter().map(|v| if v.is_empty() && field.origin.is_some() { inherit_label(field) } else { show(field, v) }).collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                r.set_model(Some(&gtk::StringList::new(&labels)));
                if labels.len() > 10 && r.expression().is_none() {
                    r.set_expression(Some(gtk::PropertyExpression::new(gtk::StringObject::static_type(), None::<gtk::Expression>, "string")));
                    r.set_enable_search(true);
                }
                r.set_selected(list.iter().position(|v| *v == current).unwrap_or(0) as u32);
                values.replace(list);
            }
            Control::Map(group, rows) => {
                for old in rows.take() {
                    group.remove(&old);
                }
                let mut made: Vec<gtk::Widget> = Vec::new();
                for entry in &field.entries {
                    let r = adw::EntryRow::builder().title(entry.name.clone()).use_markup(false).text(entry.value.clone()).show_apply_button(true).build();
                    let key = format!("{}.{}", field.key, entry.name);
                    if entry.origin == Origin::Global && field.key.starts_with("launch.") && matches!(self.form, Form::Game(_)) {
                        r.add_suffix(&gtk::Label::builder().label(gettext("Global")).valign(gtk::Align::Center).css_classes(["dimmed", "caption"]).build());
                    }
                    if entry.resettable {
                        let remove = gtk::Button::builder()
                            .icon_name("user-trash-symbolic")
                            .tooltip_text(gettext("Remove"))
                            .valign(gtk::Align::Center)
                            .css_classes(["flat", "circular"])
                            .build();
                        let (view, key) = (self.weak(), key.clone());
                        remove.connect_clicked(move |_| {
                            view.upgrade().inspect(|v| v.set(&key, ""));
                        });
                        r.add_suffix(&remove);
                    }
                    let view = self.weak();
                    r.connect_apply(move |r| {
                        view.upgrade().inspect(|v| v.set(&key, &r.text()));
                    });
                    group.add(&r);
                    made.push(r.upcast());
                }
                let add = adw::ButtonRow::builder().title(gettext("Add…")).start_icon_name("list-add-symbolic").build();
                let (view, map, label) = (self.weak(), field.key.clone(), field.label.clone());
                add.connect_activated(move |_| {
                    view.upgrade().inspect(|v| v.ask_entry(&map, &label));
                });
                group.add(&add);
                made.push(add.upcast());
                rows.replace(made);
            }
        }
        let _ = &row.group;
    }

    /// Choices the module or source lists at run time: they replace the manifest's once they come.
    fn fetch_dynamic(&self, fields: &[Field]) {
        for field in fields.iter().filter(|f| f.dynamic) {
            let (owner, key) = match (&self.form, field.key.split('.').collect::<Vec<_>>().as_slice()) {
                (Form::Module(id), [key]) => (("module", id.clone()), key.to_string()),
                (Form::Source(id), [key]) => (("source", id.clone()), key.to_string()),
                (_, ["modules", id, key]) => (("module", id.to_string()), key.to_string()),
                (_, ["sources", id, key]) => (("source", id.to_string()), key.to_string()),
                _ => continue,
            };
            let (view, field_key) = (self.weak(), field.key.clone());
            glib::spawn_future_local(async move {
                let choices = backend::call(move |core| async move {
                    match owner.0 {
                        "module" => core.module_setting_choices(&owner.1, &key).await,
                        _ => core.source_setting_choices(&owner.1, &key).await,
                    }
                })
                .await;
                let (Some(view), Ok(choices)) = (view.upgrade(), choices) else { return };
                let field = view.fields.borrow_mut().iter_mut().find(|f| f.key == field_key).map(|f| {
                    f.choices = choices.iter().map(|c| forms::Choice { value: c.clone(), label: c.clone() }).collect();
                    f.clone()
                });
                if let Some(field) = field {
                    view.syncing.set(true);
                    if let Some(row) = view.rows.borrow().iter().find(|r| r.key == field_key) {
                        view.update(row, &field);
                    }
                    view.syncing.set(false);
                }
            });
        }
    }

    fn set(&self, key: &str, value: &str) {
        let (form, key, value, view) = (self.form.clone(), key.to_string(), value.to_string(), self.weak());
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.set_field(&form, &key, &value).await }).await;
            let Some(view) = view.upgrade() else { return };
            if let Err(e) = result {
                view.toast(&e.to_string());
            }
            view.load();
        });
    }

    fn pick_path(&self, button: &gtk::Button, key: &str, folder: bool) {
        let dialog = gtk::FileDialog::builder().modal(true).build();
        let current = self.fields.borrow().iter().find(|f| f.key == key).map(|f| f.value.clone()).unwrap_or_default();
        if !current.is_empty() {
            let file = gio::File::for_path(universe::paths::expand(&current));
            if folder {
                dialog.set_initial_folder(Some(&file));
            } else {
                dialog.set_initial_file(Some(&file));
            }
        }
        let root = button.root().and_downcast::<gtk::Window>();
        let (view, key) = (self.weak(), key.to_string());
        let done = move |result: Result<gio::File, glib::Error>| {
            let (Some(view), Ok(file)) = (view.upgrade(), result) else { return };
            if let Some(path) = file.path() {
                view.set(&key, &path.to_string_lossy());
            }
        };
        if folder {
            dialog.select_folder(root.as_ref(), gio::Cancellable::NONE, done);
        } else {
            dialog.open(root.as_ref(), gio::Cancellable::NONE, done);
        }
    }

    fn ask_entry(&self, map: &str, label: &str) {
        let Some(dialog) = self.dialog.upgrade() else { return };
        let (name, value) = (adw::EntryRow::builder().title(gettext("Name")).build(), adw::EntryRow::builder().title(gettext("Value")).build());
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
        list.append(&name);
        list.append(&value);
        let ask = adw::AlertDialog::new(Some(label), None);
        ask.set_extra_child(Some(&list));
        ask.add_responses(&[("cancel", &gettext("_Cancel")), ("add", &gettext("_Add"))]);
        ask.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        ask.set_default_response(Some("add"));
        let (view, map) = (self.weak(), map.to_string());
        ask.connect_response(None, move |_, response| {
            let Some(view) = view.upgrade().filter(|_| response == "add") else { return };
            match forms::entry_key(&map, &name.text()) {
                Some(key) => view.set(&key, &value.text()),
                None => view.toast(&gettext("A name needs letters or digits")),
            }
        });
        ask.present(Some(&dialog));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(kind: &str, value: &str, resolved: &str) -> Field {
        Field { kind: kind.into(), value: value.into(), resolved: resolved.into(), ..Field::default() }
    }

    #[test]
    fn values_read_with_their_unit_and_what_auto_comes_to() {
        assert_eq!(show(&field("refresh", "auto", "144"), "auto"), "Auto · 144 Hz");
        assert_eq!(show(&field("fps", "auto", "60"), "60"), "60 FPS");
        assert_eq!(show(&field("resolution", "1920x1080", ""), "1920x1080"), "1920 × 1080");
        assert_eq!(show(&field("toggle", "on", ""), "on"), "On");
        let mut filter = field("enum", "", "linear");
        filter.fallback = Some(Origin::Default);
        assert_eq!(inherit_label(&filter), "Default · linear");
    }

    #[test]
    fn folders_are_told_from_files_by_their_key() {
        assert!(wants_folder("paths.games_root") && wants_folder("launch.prefix") && wants_folder("sources.gog.games_dir"));
        assert!(!wants_folder("launch.exe") && !wants_folder("keys.sgdb_file") && !wants_folder("keys.rawg_file"));
    }
}
