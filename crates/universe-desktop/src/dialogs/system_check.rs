use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use universe::doctor::Check;

use crate::backend;
use crate::dialogs::preferences::{text, ListPage};
use crate::window::Window;

const EXTENSION: &str = "universe-extension";

pub fn present(win: &Window) {
    let dialog = adw::PreferencesDialog::builder().title(gettext("System Check")).content_height(720).build();
    let page = ListPage::new("system-check", &gettext("System Check"), "checkbox-checked-symbolic");
    dialog.add(&page.page);
    load(&dialog, &page);
    let held = RefCell::new(Some(page));
    dialog.connect_closed(move |_| {
        held.take();
    });
    dialog.present(Some(win));
}

/// `(GNOME's extension check, failing, passing)`: the extension's is in neither list.
fn split(checks: &[Check], gnome: bool) -> (Option<&Check>, Vec<&Check>, Vec<&Check>) {
    let setup = checks.iter().find(|c| gnome && c.check == EXTENSION);
    let (passing, failing) = checks.iter().filter(|c| setup.is_none_or(|s| s.check != c.check)).partition(|c| c.ok);
    (setup, failing, passing)
}

/// Left in English for whoever reads the report.
pub fn debug_info(facts: &[(&str, String)], checks: Option<&[Check]>) -> String {
    let mut lines: Vec<String> = facts.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}: {v}")).collect();
    let Some(checks) = checks else {
        lines.push("System check: running".into());
        return lines.join("\n");
    };
    let failing: Vec<&Check> = checks.iter().filter(|c| !c.ok).collect();
    lines.push(String::new());
    lines.push(format!("System check: {} of {} fail", failing.len(), checks.len()));
    for check in failing {
        let module = if check.module.is_empty() { "core" } else { check.module.as_str() };
        lines.push(format!("- {} ({module}): {}", check.check, check.detail));
        if !check.fix.is_empty() {
            lines.push(format!("  fix: {}", check.fix));
        }
    }
    lines.join("\n")
}

fn install_button(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, component: &str) -> gtk::Button {
    let button = gtk::Button::builder().label(gettext("_Install…")).use_underline(true).valign(gtk::Align::Center).build();
    let (weak_dialog, weak_page, id) = (dialog.downgrade(), Rc::downgrade(page), component.to_string());
    button.connect_clicked(move |button| {
        let (weak_dialog, weak_page, weak_button, id) = (weak_dialog.clone(), weak_page.clone(), button.downgrade(), id.clone());
        glib::spawn_future_local(async move {
            let listed = crate::components::listing(false).await.unwrap_or_default();
            let (Some(button), Some(c)) = (weak_button.upgrade(), listed.iter().find(|c| text(c, "id") == id)) else { return };
            let checked: Rc<dyn Fn()> = Rc::new(move || {
                if let (Some(dialog), Some(page)) = (weak_dialog.upgrade(), weak_page.upgrade()) {
                    load(&dialog, &page);
                }
            });
            crate::components::act(&button, c, "install", checked);
        });
    });
    button
}

fn check_row(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>, check: &Check) -> adw::ActionRow {
    let row = crate::rows::plain(adw::ActionRow::builder().build(), check.label.clone(), check.detail.clone());
    let icon = gtk::Image::from_icon_name(if check.ok { "object-select-symbolic" } else { "dialog-warning-symbolic" });
    icon.add_css_class(if check.ok { "success" } else { "warning" });
    row.add_prefix(&icon);
    if check.ok || !(check.detail.starts_with("not installed") || check.detail.contains("not enabled")) {
        return row;
    }
    let install = gtk::Button::builder().label(gettext("_Set Up")).use_underline(true).valign(gtk::Align::Center).build();
    let (weak_dialog, weak_page) = (dialog.downgrade(), Rc::downgrade(page));
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
            load(&dialog, &page);
        });
    });
    row.add_suffix(&install);
    row
}

fn load(dialog: &adw::PreferencesDialog, page: &Rc<ListPage>) {
    page.loading();
    let (dialog, page) = (dialog.downgrade(), page.clone());
    glib::spawn_future_local(async move {
        let (modules, sources, checks, desktop) =
            backend::pinned(|core| async move { tokio::join!(core.modules(), core.sources(), core.doctor(), core.desktop()) }).await;
        let names: HashMap<String, String> = modules.iter().chain(&sources).map(|e| (text(e, "id"), text(e, "name"))).collect();
        let Some(dialog) = dialog.upgrade() else { return };
        page.clear();
        let area = |module: &str| match module {
            "" | "core" => gettext("Core"),
            "runners" => gettext("Runners"),
            "media" => gettext("Media"),
            "controller" => gettext("Controller"),
            other => names.get(other).cloned().unwrap_or_else(|| other.to_string()),
        };
        let fails = checks.iter().filter(|c| !c.ok).count();
        let top = page.group(
            "",
            &if fails == 0 {
                gettext("Every check passes")
            } else {
                ngettext("{} of {} checks fails", "{} of {} checks fail", fails as u32).replacen("{}", &fails.to_string(), 1).replacen(
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
                load(&dialog, &page);
            }
        });
        top.add(&again);
        let (setup, failing, passing) = split(&checks, desktop == universe::desktop::Profile::Gnome);
        if let Some(check) = setup {
            let group = page.group(&gettext("System Setup"), &gettext("The GNOME Shell extension finds, focuses and captures game windows"));
            group.add(&check_row(&dialog, &page, check));
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
        let mut seen = HashSet::new();
        let mut areas: Vec<String> = passing.iter().map(|c| area(&c.module)).filter(|name| seen.insert(name.clone())).collect();
        let core = gettext("Core");
        areas.sort_by_key(|name| *name != core);
        for name in areas {
            let total = checks.iter().filter(|c| area(&c.module) == name).count();
            let these: Vec<&&Check> = passing.iter().filter(|c| area(&c.module) == name).collect();
            let group = page.group(&name, &gettext("{} of {} pass").replacen("{}", &these.len().to_string(), 1).replacen("{}", &total.to_string(), 1));
            for check in these {
                group.add(&check_row(&dialog, &page, check));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, ok: bool) -> Check {
        Check {
            check: id.into(),
            label: id.into(),
            ok,
            detail: if ok { String::new() } else { format!("{id} is missing") },
            fix: if ok { String::new() } else { format!("install {id}") },
            module: String::new(),
            component: String::new(),
        }
    }

    fn ids(checks: &[&Check]) -> Vec<String> {
        checks.iter().map(|c| c.check.clone()).collect()
    }

    #[test]
    fn the_extension_shows_once_in_its_own_group_on_gnome() {
        let checks = [check("systemd", true), check(EXTENSION, false), check("gamescope", false)];
        let (setup, failing, passing) = split(&checks, true);
        assert_eq!(setup.map(|c| c.check.as_str()), Some(EXTENSION));
        assert_eq!(ids(&failing), ["gamescope"], "not under Needs Attention as well");
        assert_eq!(ids(&passing), ["systemd"]);

        let (setup, failing, _) = split(&checks, false);
        assert!(setup.is_none());
        assert_eq!(ids(&failing), [EXTENSION, "gamescope"], "elsewhere it is a check like the others");
    }

    #[test]
    fn the_debug_info_names_the_failing_checks_and_their_fixes() {
        let facts = [("Universe Desktop", "0.0.10".to_string()), ("Steam Deck", String::new())];
        let checks = [check("systemd", true), check("gamescope", false)];
        let info = debug_info(&facts, Some(&checks));
        assert!(info.contains("0.0.10") && !info.contains("Steam Deck"), "a fact the machine lacks is left out");
        assert!(info.contains("gamescope is missing") && info.contains("install gamescope"));
        assert!(!info.contains("systemd"));
    }
}
