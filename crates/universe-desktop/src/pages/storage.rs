use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use serde_json::Value;

use crate::backend;
use crate::pages::game_data::{act, confirm, home, size};
use crate::window::Window;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

/// A page of its own, so it can sit in Preferences or in a dialog of its own alike.
pub struct StorageView {
    pub page: adw::PreferencesPage,
    groups: RefCell<Vec<adw::PreferencesGroup>>,
    win: glib::WeakRef<Window>,
}

impl StorageView {
    pub fn new(win: &Window) -> Rc<StorageView> {
        let view = Rc::new(StorageView {
            page: adw::PreferencesPage::builder().name("storage").title(gettext("Storage")).icon_name("drive-harddisk-symbolic").build(),
            groups: RefCell::default(),
            win: win.downgrade(),
        });
        view.load();
        view
    }

    fn clear(&self) {
        for group in self.groups.take() {
            self.page.remove(&group);
        }
    }

    fn group(&self, title: &str, description: &str) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::builder().title(title).build();
        if !description.is_empty() {
            group.set_description(Some(description));
        }
        self.page.add(&group);
        self.groups.borrow_mut().push(group.clone());
        group
    }

    pub fn load(self: &Rc<Self>) {
        self.clear();
        self.group("", "").add(&adw::Spinner::builder().height_request(48).margin_top(24).build());
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let storage = backend::call(|core| async move { core.storage().await }).await;
            let Some(view) = weak.upgrade() else { return };
            view.clear();
            match storage {
                Ok(storage) => view.show(&storage, Rc::downgrade(&view)),
                Err(e) => view.group(&gettext("Storage"), &e.to_string()).set_visible(true),
            }
        });
    }

    fn show(&self, storage: &Value, me: Weak<StorageView>) {
        let names = [
            ("games", gettext("Games")),
            ("prefixes", gettext("Wine Prefixes")),
            ("saves", gettext("Save Backups")),
            ("recordings", gettext("Recordings")),
            ("library", gettext("Library Data")),
            ("components", gettext("Runners and Tools")),
            ("logs", gettext("Logs")),
        ];
        let folders = self.group(&gettext("Folders"), "");
        for root in storage["roots"].as_array().into_iter().flatten() {
            let id = text(root, "id");
            let title = names.iter().find(|(k, _)| *k == id).map(|(_, n)| n.clone()).unwrap_or(id);
            let row = crate::rows::plain(adw::ActionRow::builder().build(), title, home(&text(root, "path")));
            let mut used = size(&root["bytes"]);
            if root["free"].as_u64().unwrap_or(0) > 0 {
                used = gettext("{} · {} free").replacen("{}", &used, 1).replacen("{}", &size(&root["free"]), 1);
            }
            row.add_suffix(&gtk::Label::builder().label(used).css_classes(["dimmed"]).build());
            folders.add(&row);
        }

        let games = storage["games"].as_array().cloned().unwrap_or_default();
        let total: u64 = games.iter().map(|g| g["bytes"].as_u64().unwrap_or(0)).sum();
        let list = self.group(&gettext("Games"), &glib::format_size(total));
        for g in &games {
            let parts: Vec<String> =
                [("install", gettext("Install")), ("prefix", gettext("Prefix")), ("saves", gettext("Saves")), ("recordings", gettext("Recordings"))]
                    .iter()
                    .filter(|(k, _)| g[*k].as_u64().unwrap_or(0) > 0)
                    .map(|(k, label)| format!("{label} {}", size(&g[*k])))
                    .collect();
            let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), text(g, "title"), parts.join(" · "));
            row.add_suffix(&gtk::Label::builder().label(size(&g["bytes"])).css_classes(["dimmed"]).build());
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let (id, win) = (text(g, "id"), self.win.clone());
            row.connect_activated(move |row| {
                let Some(win) = win.upgrade() else { return };
                let Some(game) = win.app().library().get(&id) else { return };
                if let Some(dialog) = row.ancestor(adw::Dialog::static_type()).and_downcast::<adw::Dialog>() {
                    dialog.close();
                }
                win.open_game(&game);
            });
            list.add(&row);
        }

        let leftovers = storage["leftovers"].as_array().cloned().unwrap_or_default();
        let described = if leftovers.is_empty() {
            gettext("Nothing: every prefix, recording and folder belongs to a game in the library.")
        } else {
            gettext("{} that no game in the library uses. Nothing goes to the trash unless you ask.").replace("{}", &size(&storage["leftover_bytes"]))
        };
        let group = self.group(&gettext("Leftovers"), &described);
        for item in &leftovers {
            let title = match text(item, "kind").as_str() {
                "prefix" => gettext("Prefix No Game Uses"),
                "recordings" => gettext("Archived Recordings"),
                "game" => gettext("Removed Game"),
                _ => gettext("Removed Game's Logs"),
            };
            let title = if text(item, "title").is_empty() { title } else { format!("{title}: {}", text(item, "title")) };
            let row = crate::rows::plain(adw::ActionRow::builder().build(), title, home(&text(item, "path")));
            row.add_suffix(&gtk::Label::builder().label(size(&item["bytes"])).css_classes(["dimmed"]).build());
            let trash = gtk::Button::builder().icon_name("user-trash-symbolic").tooltip_text(gettext("Move to Trash")).valign(gtk::Align::Center).build();
            trash.add_css_class("flat");
            let (path, me) = (text(item, "path"), me.clone());
            trash.connect_clicked(move |button| {
                let (path, me, anchor) = (path.clone(), me.clone(), button.clone().upcast::<gtk::Widget>());
                let body = gettext("{} goes to the trash, where it can be brought back until the trash is emptied.").replace("{}", &home(&path));
                confirm(button.upcast_ref(), &gettext("Move to the Trash?"), &body, &gettext("_Move to Trash"), move || {
                    let (path, me) = (path.clone(), me.clone());
                    let shown = home(&path);
                    let target = path.clone();
                    act(
                        &anchor,
                        move |core| async move { core.trash_leftover(&target).await },
                        move |()| gettext("{} moved to the trash").replace("{}", &shown),
                        Rc::new(move || {
                            if let Some(view) = me.upgrade() {
                                view.load();
                            }
                        }),
                    );
                });
            });
            row.add_suffix(&trash);
            group.add(&row);
        }
    }
}
