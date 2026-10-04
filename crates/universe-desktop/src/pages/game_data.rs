use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use serde_json::Value;

use crate::backend;
use crate::window::Window;

type Reload = Rc<dyn Fn()>;

fn text(v: &Value, key: &str) -> String {
    match &v[key] {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(crate) fn size(v: &Value) -> String {
    glib::format_size(v.as_u64().unwrap_or(0)).to_string()
}

pub(crate) fn home(path: &str) -> String {
    let home = glib::home_dir().to_string_lossy().into_owned();
    match path.strip_prefix(&home) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

fn day(when: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(when)
        .map(|t| t.with_timezone(&chrono::Local).format("%-d %b %Y, %H:%M").to_string())
        .unwrap_or_else(|_| when.to_string())
}

pub(crate) fn toast(anchor: &gtk::Widget, line: &str) {
    if let Some(dialog) = anchor.ancestor(adw::PreferencesDialog::static_type()).and_downcast::<adw::PreferencesDialog>() {
        dialog.add_toast(crate::dialogs::toast(line));
    } else if let Some(win) = anchor.root().and_downcast::<Window>() {
        win.toast(crate::dialogs::toast(line));
    }
}

pub(crate) fn act<T, F, Fut>(anchor: &gtk::Widget, work: F, said: impl Fn(T) -> String + 'static, reload: Reload)
where
    T: Send + 'static,
    F: FnOnce(Arc<universe::core::Core>) -> Fut + Send + 'static,
    Fut: Future<Output = universe::Result<T>> + Send + 'static,
{
    let anchor = anchor.downgrade();
    glib::spawn_future_local(async move {
        let result = backend::call(work).await;
        let Some(anchor) = anchor.upgrade() else { return };
        toast(&anchor, &result.map(said).unwrap_or_else(|e| e.to_string()));
        reload();
    });
}

pub(crate) fn confirm(anchor: &gtk::Widget, heading: &str, body: &str, yes: &str, then: impl Fn() + 'static) {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("yes", yes)]);
    dialog.set_response_appearance("yes", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(Some("yes"), move |_, _| then());
    dialog.present(Some(anchor));
}

pub(crate) fn open_folder(anchor: &gtk::Widget, path: &str) {
    let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
    let weak = anchor.downgrade();
    launcher.launch(anchor.root().and_downcast_ref::<gtk::Window>(), gio::Cancellable::NONE, move |result| {
        if let (Some(anchor), Err(e)) = (weak.upgrade(), result) {
            toast(&anchor, &e.to_string());
        }
    });
}

fn folder_button(path: &str) -> gtk::Button {
    let button = gtk::Button::builder().icon_name("folder-open-symbolic").tooltip_text(gettext("Open Folder")).valign(gtk::Align::Center).build();
    button.add_css_class("flat");
    let path = path.to_string();
    button.connect_clicked(move |b| open_folder(b.upcast_ref(), &path));
    button
}

fn size_label(v: &Value) -> gtk::Label {
    let label = gtk::Label::new(Some(&size(v)));
    label.add_css_class("dimmed");
    label
}

fn files(n: usize) -> String {
    gettextrs::ngettext("{} file", "{} files", n as u32).replace("{}", &n.to_string())
}

fn backed_up(done: universe::saves::Outcome) -> String {
    match done.change.as_str() {
        "same" => gettext("Saves unchanged since the last backup"),
        "none" => gettext("No saves found"),
        _ => gettext("Backed up {}").replace("{}", &files(done.files.len())),
    }
}

pub fn fill(list: &gtk::ListBox, data: &Value, reload: Reload) {
    list.remove_all();
    let id = text(data, "id");
    let saves = &data["saves"];
    let backups = saves["backups"].as_array().cloned().unwrap_or_default();
    if text(saves, "engine").is_empty() {
        let row = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Save Folder"), home(&text(saves, "folder")));
        row.set_subtitle_selectable(true);
        row.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
        row.add_suffix(&gtk::Label::builder().label(gettext("Every game's")).css_classes(["dimmed"]).build());
        row.add_suffix(&folder_button(&text(saves, "folder")));
        list.append(&row);
    } else {
        let found = saves["files"].as_array().map(Vec::len).unwrap_or(0);
        let subtitle = if text(saves, "error").is_empty() { format!("{} · {}", files(found), size(&saves["bytes"])) } else { text(saves, "error") };
        let row = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Saves"), subtitle);
        row.add_prefix(&gtk::Image::from_icon_name("document-save-symbolic"));
        if !text(saves, "name").is_empty() {
            row.set_tooltip_text(Some(&text(saves, "name")));
        }
        let button = gtk::Button::builder().label(gettext("Back Up")).valign(gtk::Align::Center).build();
        let (gid, again) = (id.clone(), reload.clone());
        button.connect_clicked(move |b| {
            let gid = gid.clone();
            act(b.upcast_ref(), move |core| async move { core.saves_backup(&gid).await }, backed_up, again.clone());
        });
        row.add_suffix(&button);
        list.append(&row);

        let kept = adw::ExpanderRow::builder()
            .title(gettext("Backups"))
            .subtitle(match backups.first() {
                Some(b) => gettext("{} kept · last {}").replacen("{}", &backups.len().to_string(), 1).replacen("{}", &day(&text(b, "when")), 1),
                None => gettext("None yet"),
            })
            .sensitive(!backups.is_empty())
            .build();
        kept.add_prefix(&gtk::Image::from_icon_name("document-open-recent-symbolic"));
        for b in &backups {
            let entry = crate::rows::plain(adw::ActionRow::builder().build(), day(&text(b, "when")), size(&b["bytes"]));
            let restore = gtk::Button::builder().label(gettext("Restore")).valign(gtk::Align::Center).build();
            let (gid, backup, again) = (id.clone(), text(b, "id"), reload.clone());
            restore.connect_clicked(move |button| {
                let (gid, backup, again, anchor) = (gid.clone(), backup.clone(), again.clone(), button.clone().upcast::<gtk::Widget>());
                let body = gettext("The saves on disk now are replaced by this backup's.");
                confirm(button.upcast_ref(), &gettext("Restore This Backup?"), &body, &gettext("_Restore"), move || {
                    let (gid, backup) = (gid.clone(), backup.clone());
                    act(
                        &anchor,
                        move |core| async move { core.saves_restore(&gid, &backup).await },
                        |done| gettext("Restored {}").replace("{}", &files(done.files.len())),
                        again.clone(),
                    );
                });
            });
            entry.add_suffix(&restore);
            kept.add_row(&entry);
        }
        let export = adw::ButtonRow::builder().title(gettext("Export the Backups to Your Home Folder")).build();
        let (gid, again) = (id.clone(), reload.clone());
        export.connect_activated(move |row| {
            let gid = gid.clone();
            act(
                row.upcast_ref(),
                move |core| async move { core.saves_export(&gid, "~").await },
                |path| gettext("Exported to {}").replace("{}", &home(&path.to_string_lossy())),
                again.clone(),
            );
        });
        kept.add_row(&export);
        list.append(&kept);
    }

    let prefix = &data["prefix"];
    if prefix.is_object() {
        list.append(&prefix_row(list, &id, prefix, reload.clone()));
    }

    let install = &data["install"];
    if install.is_object() {
        let row = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Install Folder"), home(&text(install, "path")));
        row.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
        row.add_suffix(&size_label(&install["bytes"]));
        row.add_suffix(&folder_button(&text(install, "path")));
        list.append(&row);
    }
    let universe = &data["universe"];
    let parts = &universe["parts"];
    let detail = [("media", gettext("Artwork")), ("screenshots", gettext("Screenshots")), ("journal", gettext("Journal"))]
        .iter()
        .map(|(k, label)| format!("{label} {}", size(&parts[*k])))
        .collect::<Vec<_>>()
        .join(" · ");
    let row = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Universe's Files"), detail);
    row.add_prefix(&gtk::Image::from_icon_name("folder-pictures-symbolic"));
    row.add_suffix(&size_label(&universe["bytes"]));
    row.add_suffix(&folder_button(&text(universe, "path")));
    list.append(&row);
    let recordings = &data["recordings"];
    if recordings["bytes"].as_u64().unwrap_or(0) > 0 {
        let row = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Recordings"), home(&text(recordings, "path")));
        row.add_prefix(&gtk::Image::from_icon_name("camera-video-symbolic"));
        row.add_suffix(&size_label(&recordings["bytes"]));
        row.add_suffix(&folder_button(&text(recordings, "path")));
        list.append(&row);
    }
}

fn prefix_row(list: &gtk::ListBox, id: &str, prefix: &Value, reload: Reload) -> adw::ActionRow {
    let owner = text(prefix, "owner");
    let title = match owner.as_str() {
        "universe" => gettext("Universe's Wine Prefix"),
        "steam" => gettext("Steam's Wine Prefix"),
        "lutris" => gettext("Lutris's Wine Prefix"),
        "wine" => gettext("Wine's Default Prefix"),
        _ => gettext("A Wine Prefix of Its Own"),
    };
    let shared: Vec<String> = prefix["shared_with"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
    let mut subtitle = home(&text(prefix, "path"));
    if !shared.is_empty() {
        subtitle = gettext("{} · shared with {}").replacen("{}", &subtitle, 1).replacen("{}", &shared.join(", "), 1);
    }
    let row = crate::rows::plain(adw::ActionRow::builder().build(), title, subtitle);
    row.add_prefix(&gtk::Image::from_icon_name("application-x-executable-symbolic"));
    row.add_suffix(&size_label(&prefix["bytes"]));

    let menu = gio::Menu::new();
    let section = gio::Menu::new();
    section.append(Some(&gettext("_Open Folder")), Some("data.open-prefix"));
    if prefix["movable"] == true {
        section.append(Some(&gettext("_Move Into Universe's Prefixes…")), Some("data.move"));
    }
    menu.append_section(None, &section);
    let tools = gio::Menu::new();
    tools.append(Some(&gettext("Wine _Configuration")), Some("data.tool::winecfg"));
    tools.append(Some(&gettext("_Winetricks")), Some("data.tool::winetricks"));
    tools.append(Some(&gettext("_Run a Program in the Prefix…")), Some("data.run"));
    tools.append(Some(&gettext("_Stop the Prefix's Programs")), Some("data.tool::kill"));
    menu.append_section(None, &tools);
    if owner == "universe" && shared.is_empty() {
        let reset = gio::Menu::new();
        reset.append(Some(&gettext("R_eset the Prefix…")), Some("data.reset"));
        menu.append_section(None, &reset);
    }
    let button =
        gtk::MenuButton::builder().icon_name("view-more-symbolic").menu_model(&menu).valign(gtk::Align::Center).tooltip_text(gettext("Prefix Tools")).build();
    button.add_css_class("flat");
    row.add_suffix(&button);

    let group = gio::SimpleActionGroup::new();
    let anchor = list.clone().upcast::<gtk::Widget>();
    let path = text(prefix, "path");
    let open = gio::SimpleAction::new("open-prefix", None);
    let (a, p) = (anchor.downgrade(), path.clone());
    open.connect_activate(move |_, _| {
        if let Some(a) = a.upgrade() {
            open_folder(&a, &p);
        }
    });
    group.add_action(&open);

    let moving = gio::SimpleAction::new("move", None);
    let (a, gid, again, target) = (anchor.downgrade(), id.to_string(), reload.clone(), text(prefix, "target"));
    moving.connect_activate(move |_, _| {
        let Some(anchor) = a.upgrade() else { return };
        let (gid, again, at) = (gid.clone(), again.clone(), anchor.clone());
        let body =
            gettext("It moves to {}. Every game using it follows, and across drives it is copied, which can take a while.").replace("{}", &home(&target));
        confirm(&anchor, &gettext("Move the Wine Prefix?"), &body, &gettext("_Move"), move || {
            let gid = gid.clone();
            act(
                &at,
                move |core| async move { core.move_prefix(&gid).await },
                |moved| gettext("Moved to {}").replace("{}", &home(&text(&moved, "to"))),
                again.clone(),
            );
        });
    });
    group.add_action(&moving);

    let tool = gio::SimpleAction::new("tool", Some(glib::VariantTy::STRING));
    let (a, gid, again) = (anchor.downgrade(), id.to_string(), reload.clone());
    tool.connect_activate(move |_, which| {
        let (Some(anchor), Some(which)) = (a.upgrade(), which.and_then(|v| v.get::<String>())) else { return };
        let gid = gid.clone();
        let said = if which == "kill" { gettext("The prefix's programs are stopping") } else { gettext("Started") };
        act(&anchor, move |core| async move { core.prefix_tool(&gid, &which, &[]).await }, move |_| said.clone(), again.clone());
    });
    group.add_action(&tool);

    let run = gio::SimpleAction::new("run", None);
    let (a, gid, again) = (anchor.downgrade(), id.to_string(), reload.clone());
    run.connect_activate(move |_, _| {
        let Some(anchor) = a.upgrade() else { return };
        let chooser = gtk::FileDialog::builder().title(gettext("Run a Program in the Prefix")).modal(true).build();
        let (gid, again, at) = (gid.clone(), again.clone(), anchor.clone());
        chooser.open(anchor.root().and_downcast_ref::<gtk::Window>(), gio::Cancellable::NONE, move |picked| {
            let Some(path) = picked.ok().and_then(|f| f.path()) else { return };
            let (gid, exe) = (gid.clone(), path.to_string_lossy().into_owned());
            act(&at, move |core| async move { core.prefix_tool(&gid, "run", &[exe]).await }, |_| gettext("Started"), again.clone());
        });
    });
    group.add_action(&run);

    let reset = gio::SimpleAction::new("reset", None);
    let (a, gid, again) = (anchor.downgrade(), id.to_string(), reload);
    reset.connect_activate(move |_, _| {
        let Some(anchor) = a.upgrade() else { return };
        let (gid, again, at) = (gid.clone(), again.clone(), anchor.clone());
        let body = gettext("Its saves are backed up first, then it goes to the trash. The next launch makes a fresh prefix.");
        confirm(&anchor, &gettext("Reset the Wine Prefix?"), &body, &gettext("_Reset"), move || {
            let gid = gid.clone();
            act(&at, move |core| async move { core.reset_prefix(&gid).await }, |_| gettext("Prefix reset"), again.clone());
        });
    });
    group.add_action(&reset);
    list.insert_action_group("data", Some(&group));
    row
}
