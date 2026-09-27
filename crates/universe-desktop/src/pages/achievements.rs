use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use serde_json::Value;

use crate::backend;
use crate::format;
use crate::widgets::Cover;
use crate::window::Window;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn rarity(v: &Value) -> String {
    match v["rarity"].as_f64() {
        Some(r) if r < 10.0 => gettext("{}% of players").replace("{}", &format!("{r:.1}")),
        Some(r) => gettext("{}% of players").replace("{}", &format!("{}", r.round())),
        None => String::new(),
    }
}

/// Unlocked first, the newest on top; then the locked ones, the most common first; a hidden one closes the list.
fn order(items: &mut [Value]) {
    let unlocked = |v: &Value| !text(v, "unlocked_at").is_empty();
    let masked = |v: &Value| v["hidden"].as_bool() == Some(true) && !unlocked(v);
    items.sort_by(|a, b| {
        unlocked(b).cmp(&unlocked(a)).then_with(|| {
            if unlocked(a) {
                text(b, "unlocked_at").cmp(&text(a, "unlocked_at"))
            } else {
                masked(a).cmp(&masked(b)).then(b["rarity"].as_f64().unwrap_or(-1.0).total_cmp(&a["rarity"].as_f64().unwrap_or(-1.0)))
            }
        })
    });
}

fn row(item: &Value) -> adw::ActionRow {
    let unlocked = !text(item, "unlocked_at").is_empty();
    let masked = item["hidden"].as_bool() == Some(true) && !unlocked;
    let (name, description) = if masked {
        (gettext("Hidden Achievement"), gettext("Keep playing to find out"))
    } else {
        (if text(item, "name").is_empty() { text(item, "key") } else { text(item, "name") }, text(item, "description"))
    };
    let row = crate::rows::plain(adw::ActionRow::builder().build(), name, description);
    let icon = Cover::new(48, 48);
    icon.set_placeholder("trophy-symbolic");
    icon.add_css_class("thumb");
    icon.set_valign(gtk::Align::Center);
    let locked_icon = text(item, "icon_locked");
    icon.set_path(if unlocked || locked_icon.is_empty() { text(item, "icon") } else { locked_icon });
    if !unlocked {
        icon.add_css_class("locked");
    }
    row.add_prefix(&icon);
    let side = if unlocked {
        chrono::DateTime::parse_from_rfc3339(&text(item, "unlocked_at")).map(|t| format::relative(t.timestamp(), chrono::Local::now())).unwrap_or_default()
    } else {
        rarity(item)
    };
    if !side.is_empty() {
        row.add_suffix(&gtk::Label::builder().label(side).css_classes(["dimmed", "caption"]).build());
    }
    row
}

/// A game's achievements as its store lists them: how far along, then each one, the unlocked ones first.
pub fn open(win: &Window, game: &str) {
    let column =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(24).margin_top(24).margin_bottom(36).margin_start(12).margin_end(12).build();
    let stack = adw::ViewStack::new();
    stack.add_named(
        &adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build(),
        Some("loading"),
    );
    let status = adw::StatusPage::builder().icon_name("dialog-information-symbolic").title(gettext("No Achievements")).build();
    stack.add_named(&status, Some("status"));
    let clamp = adw::Clamp::builder().maximum_size(760).child(&column).build();
    stack.add_named(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&clamp).build(), Some("list"));
    let refresh = gtk::Button::builder().icon_name("view-refresh-symbolic").tooltip_text(gettext("Ask the Store Again")).build();
    let header = adw::HeaderBar::new();
    header.pack_end(&refresh);
    let toolbar = adw::ToolbarView::builder().content(&stack).build();
    toolbar.add_top_bar(&header);
    let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("Achievements")).tag(format!("achievements:{game}")).build();

    let load: Rc<dyn Fn(bool)> = {
        let (game, column, stack, status, refresh) = (game.to_string(), column.downgrade(), stack.downgrade(), status.downgrade(), refresh.downgrade());
        Rc::new(move |ask: bool| {
            let (game, column, stack, status, refresh) = (game.clone(), column.clone(), stack.clone(), status.clone(), refresh.clone());
            if let Some(button) = refresh.upgrade() {
                button.set_sensitive(false);
            }
            glib::spawn_future_local(async move {
                let listing = backend::pinned(move |core| async move { core.achievements(&game, ask).await }).await;
                let (Some(column), Some(stack), Some(status)) = (column.upgrade(), stack.upgrade(), status.upgrade()) else { return };
                if let Some(button) = refresh.upgrade() {
                    button.set_sensitive(true);
                }
                let listing = match listing {
                    Ok(listing) => listing,
                    Err(e) => {
                        status.set_description(Some(&glib::markup_escape_text(&e.to_string())));
                        stack.set_visible_child_name("status");
                        return;
                    }
                };
                while let Some(child) = column.first_child() {
                    column.remove(&child);
                }
                let (total, unlocked) = (listing["total"].as_u64().unwrap_or(0), listing["unlocked"].as_u64().unwrap_or(0));
                let mut items: Vec<Value> = listing["items"].as_array().cloned().unwrap_or_default();
                if items.is_empty() {
                    status.set_description(Some(&gettext("The store lists none for this game")));
                    stack.set_visible_child_name("status");
                    return;
                }
                let head = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
                head.append(
                    &gtk::Label::builder()
                        .label(gettext("{} of {} Unlocked").replacen("{}", &unlocked.to_string(), 1).replacen("{}", &total.to_string(), 1))
                        .xalign(0.0)
                        .css_classes(["title-2"])
                        .build(),
                );
                let fraction = if total > 0 { unlocked as f64 / total as f64 } else { 0.0 };
                head.append(&gtk::ProgressBar::builder().fraction(fraction).build());
                let fetched =
                    chrono::DateTime::parse_from_rfc3339(&text(&listing, "fetched_at")).map(|t| format::ago(t.timestamp(), chrono::Local::now())).ok();
                let mut line = vec![format!("{}%", (fraction * 100.0).round())];
                if let Some(fetched) = fetched {
                    line.push(gettext("updated {}").replace("{}", &fetched));
                }
                head.append(&gtk::Label::builder().label(line.join(" · ")).xalign(0.0).css_classes(["dimmed"]).build());
                column.append(&head);
                order(&mut items);
                let group = adw::PreferencesGroup::new();
                for item in &items {
                    group.add(&row(item));
                }
                column.append(&group);
                stack.set_visible_child_name("list");
            });
        })
    };
    let weak = Rc::downgrade(&load);
    refresh.connect_clicked(move |_| {
        weak.upgrade().inspect(|load| load(true));
    });
    let app = win.app();
    let (weak, id) = (Rc::downgrade(&load), game.to_string());
    let followed = app.connect_changed(move |event| {
        if let universe::changes::Event::Library(ids) = event {
            if ids.contains(&id) {
                weak.upgrade().inspect(|load| load(false));
            }
        }
    });
    load(false);
    let (held, weak_app) = (RefCell::new(Some((load, followed))), app.downgrade());
    page.connect_destroy(move |_| {
        if let (Some((_, followed)), Some(app)) = (held.take(), weak_app.upgrade()) {
            app.disconnect(followed);
        }
    });
    win.push_page(&page);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocked_come_first_newest_on_top_then_the_common_locked_ones() {
        let mut items = vec![
            serde_json::json!({"key": "rare", "unlocked_at": "", "rarity": 2.0}),
            serde_json::json!({"key": "old", "unlocked_at": "2026-01-01T00:00:00Z"}),
            serde_json::json!({"key": "secret", "unlocked_at": "", "hidden": true, "rarity": 50.0}),
            serde_json::json!({"key": "common", "unlocked_at": "", "rarity": 40.0}),
            serde_json::json!({"key": "new", "unlocked_at": "2026-09-01T00:00:00Z"}),
        ];
        order(&mut items);
        let keys: Vec<String> = items.iter().map(|v| text(v, "key")).collect();
        assert_eq!(keys, ["new", "old", "common", "rare", "secret"]);
    }
}
