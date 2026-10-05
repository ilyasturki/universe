use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use serde_json::Value;

use crate::backend;
use crate::components::{toast, Ask};
use crate::dialogs::preferences::text;

fn kind_line(kind: &str) -> String {
    if kind == "source" {
        gettext("A source")
    } else {
        gettext("A module")
    }
}

/// What a row says beside its name: installed, an update, or the version on offer and its size.
pub fn state(row: &Value) -> String {
    let (update, installed) = (text(row, "update"), row["installed"] == true);
    match () {
        _ if installed && !update.is_empty() => gettext("Update {}").replace("{}", &update),
        _ if installed && text(row, "origin") == "unlisted" => gettext("Installed · Unlisted"),
        _ if installed => gettext("Installed"),
        _ if !text(row, "incompatible").is_empty() => gettext("Needs another Universe"),
        _ => {
            let size = row["size"].as_u64().filter(|s| *s > 0).map(|s| glib::format_size(s).to_string()).unwrap_or_default();
            [text(row, "version"), size].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ")
        }
    }
}

/// `https://host/…` as its host, a file as itself.
fn host(url: &str) -> String {
    match url.split_once("://") {
        Some(("https", rest)) => rest.split('/').next().unwrap_or(rest).to_string(),
        _ => url.strip_prefix("file://").unwrap_or(url).to_string(),
    }
}

/// The confirmation before `action` (`install`, `update`, `remove`): an install names the kind, that it runs programs as
/// you and where it comes from.
pub fn ask(row: &Value, action: &str, index: &str) -> Ask {
    let name = text(row, "name");
    if action == "remove" {
        let body = gettext("Universe deletes it and turns it off. {} cannot run until it is installed again.").replace("{}", &name);
        return Ask { heading: gettext("Remove {}?").replace("{}", &name), body, yes: gettext("_Remove"), destructive: true };
    }
    let listed = row["listed"] == true && text(row, "origin") != "unlisted";
    let origin = if listed { gettext("from the index at {}").replace("{}", &host(index)) } else { gettext("Unlisted") };
    let version = if action == "update" { text(row, "update") } else { text(row, "version") };
    let head =
        [kind_line(&text(row, "kind")), version.clone(), row["size"].as_u64().filter(|s| *s > 0).map(|s| glib::format_size(s).to_string()).unwrap_or_default()];
    let head = head.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ");
    let body = format!("{head}\n{}\n\n{}", text(row, "description"), gettext("Runs programs as you · {}").replace("{}", &origin));
    let (heading, yes) = if action == "update" {
        (gettext("Update {} to {}?").replacen("{}", &name, 1).replacen("{}", &version, 1), gettext("_Update"))
    } else {
        (gettext("Install {}?").replace("{}", &name), gettext("_Install"))
    };
    Ask { heading, body, yes, destructive: false }
}

/// The index's add-ons of `kind` (`module` or `source`) on a subpage of `dialog`; `changed` hears each install, update and
/// removal.
pub fn present(dialog: &adw::PreferencesDialog, kind: &'static str, changed: Rc<dyn Fn()>) {
    let page = adw::PreferencesPage::builder().name("addons").build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    toolbar.set_content(Some(&page));
    dialog.push_subpage(&adw::NavigationPage::new(&toolbar, &gettext("Add-ons")));
    let shown: Rc<RefCell<Option<adw::PreferencesGroup>>> = Rc::default();
    fill(&page, &shown, kind, changed);
}

fn fill(page: &adw::PreferencesPage, shown: &Rc<RefCell<Option<adw::PreferencesGroup>>>, kind: &'static str, changed: Rc<dyn Fn()>) {
    if shown.borrow().is_none() {
        let group = adw::PreferencesGroup::new();
        group.add(&adw::Spinner::builder().height_request(48).margin_top(24).build());
        page.add(&group);
        shown.replace(Some(group));
    }
    let (page, shown) = (page.downgrade(), shown.clone());
    glib::spawn_future_local(async move {
        let listing = backend::call(|core| async move { core.extensions().await }).await;
        let Some(page) = page.upgrade() else { return };
        if let Some(old) = shown.take() {
            page.remove(&old);
        }
        let group = adw::PreferencesGroup::builder().description(gettext("Modules and sources others made: each one runs programs as you")).build();
        page.add(&group);
        shown.replace(Some(group.clone()));
        let listing = match listing {
            Ok(listing) => listing,
            Err(e) => return group.set_description(Some(&e.to_string())),
        };
        let error = text(&listing["index"], "error");
        if !error.is_empty() {
            group.set_description(Some(&gettext("The add-on index could not be reached: {}").replace("{}", &error)));
        }
        let index = text(&listing["index"], "url");
        for entry in listing["extensions"].as_array().into_iter().flatten().filter(|r| text(r, "kind") == kind).cloned() {
            let open = entry["installed"] == true || text(&entry, "incompatible").is_empty();
            let subtitle = if open { text(&entry, "description") } else { text(&entry, "incompatible") };
            let row = crate::rows::marked(adw::ActionRow::builder().activatable(open).build(), text(&entry, "name"), subtitle);
            row.add_suffix(&gtk::Label::builder().label(state(&entry)).valign(gtk::Align::Center).css_classes(["dimmed"]).build());
            let (weak_page, weak_shown, index, changed) = (page.downgrade(), Rc::downgrade(&shown), index.clone(), changed.clone());
            row.connect_activated(move |row| {
                let (Some(page), Some(shown)) = (weak_page.upgrade(), weak_shown.upgrade()) else { return };
                let changed = changed.clone();
                let again: Rc<dyn Fn()> = Rc::new(move || {
                    changed();
                    fill(&page, &shown, kind, changed.clone());
                });
                pick(row, &entry, &index, again);
            });
            group.add(&row);
        }
    });
}

/// An installed one asks before its removal, offering its update beside it; any other before its install.
fn pick(anchor: &adw::ActionRow, entry: &Value, index: &str, again: Rc<dyn Fn()>) {
    let installed = entry["installed"] == true;
    let question = ask(entry, if installed { "remove" } else { "install" }, index);
    let dialog = adw::AlertDialog::new(Some(&question.heading), Some(&question.body));
    let no = if question.destructive { gettext("_Keep It") } else { gettext("_Not Now") };
    dialog.add_responses(&[("no", &no), ("yes", &question.yes)]);
    dialog.set_response_appearance("yes", if question.destructive { adw::ResponseAppearance::Destructive } else { adw::ResponseAppearance::Suggested });
    let update = text(entry, "update");
    if installed && !update.is_empty() {
        dialog.add_response("update", &gettext("_Update to {}").replace("{}", &update));
        dialog.set_response_appearance("update", adw::ResponseAppearance::Suggested);
    }
    dialog.set_default_response(Some(if question.destructive { "no" } else { "yes" }));
    dialog.set_close_response("no");
    let (weak, id, name) = (anchor.downgrade(), text(entry, "id"), text(entry, "name"));
    dialog.connect_response(None, move |_, response| {
        let action = match response {
            "yes" if installed => "remove",
            "yes" => "install",
            "update" => "update",
            _ => return,
        };
        if let Some(anchor) = weak.upgrade() {
            run(anchor.upcast_ref(), id.clone(), name.clone(), action, again.clone());
        }
    });
    dialog.present(Some(anchor));
}

fn run(anchor: &gtk::Widget, id: String, name: String, action: &'static str, again: Rc<dyn Fn()>) {
    let busy = if action == "remove" { gettext("Removing {}…") } else { gettext("Installing {}…") };
    toast(anchor, &busy.replace("{}", &name));
    let weak = anchor.downgrade();
    glib::spawn_future_local(async move {
        let result = backend::pinned(move |core| async move {
            match action {
                "install" => {
                    core.extension_install(&id, true, None).await.map(|v| gettext("Installed {}").replace("{}", &format!("{name} {}", text(&v, "version"))))
                }
                "update" => core.extension_update(&id, None).await.map(|_| gettext("Updated {}").replace("{}", &name)),
                _ => core.extension_remove(&id).await.map(|_| gettext("Removed {}").replace("{}", &name)),
            }
        })
        .await;
        if let Some(anchor) = weak.upgrade() {
            toast(&anchor, &result.unwrap_or_else(|e| e.to_string()));
        }
        again();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(installed: bool, origin: &str, update: &str) -> Value {
        json!({"id": "now-playing", "kind": "module", "name": "Now Playing", "description": "Shares what you play.", "version": "1.2.0",
            "size": 48000, "listed": true, "installed": installed, "installed_version": "", "origin": origin, "from": "", "update": update,
            "enabled": false, "incompatible": ""})
    }

    #[test]
    fn an_install_asks_naming_its_kind_that_it_runs_as_you_and_its_origin() {
        let index = "https://raw.githubusercontent.com/ilyasturki/universe-extensions/index/index.json";
        let install = ask(&row(false, "", ""), "install", index);
        assert!(!install.destructive && install.body.contains("A module") && install.body.contains("raw.githubusercontent.com"), "{}", install.body);
        let unlisted = ask(&row(true, "unlisted", ""), "update", index);
        assert!(unlisted.body.contains("Unlisted") && !unlisted.body.contains("raw.githubusercontent.com"), "{}", unlisted.body);
        assert!(ask(&row(true, "registry", ""), "remove", index).destructive);
        assert_eq!(state(&row(true, "registry", "1.3.0")), "Update 1.3.0");
        assert_eq!(state(&row(true, "unlisted", "")), "Installed · Unlisted");
    }
}
