use gettextrs::gettext;
use gtk::gio;
use gtk::prelude::*;

fn section(items: &[(String, &str)]) -> gio::Menu {
    let menu = gio::Menu::new();
    for (label, action) in items {
        let item = gio::MenuItem::new(Some(label), Some(action));
        if *action == "app.big-screen" {
            item.set_attribute_value("hidden-when", Some(&"action-missing".to_variant()));
        }
        menu.append_item(&item);
    }
    menu
}

/// The primary menu every page's header holds, `own` (a page's verbs) on top.
pub fn main(own: Option<&gio::Menu>) -> gio::Menu {
    let menu = gio::Menu::new();
    if let Some(own) = own {
        menu.append_section(None, own);
    }
    menu.append_section(None, &section(&[(gettext("_Rescan Library"), "win.rescan")]));
    menu.append_section(None, &section(&[(gettext("Open _Big Screen"), "app.big-screen")]));
    menu.append_section(
        None,
        &section(&[
            (gettext("_Preferences"), "app.preferences"),
            (gettext("_Keyboard Shortcuts"), "app.shortcuts"),
            (gettext("_About Universe Desktop"), "app.about"),
        ]),
    );
    menu
}

/// The Store page's menu: its own verbs, then the main menu's.
pub fn store() -> gio::Menu {
    main(Some(&section(&[(gettext("_Find Installed Games"), "store.scan")])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(menu: &impl IsA<gio::MenuModel>) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0..menu.n_items() {
            if let Some(action) = menu.item_attribute_value(i, "action", None).and_then(|v| v.get::<String>()) {
                out.push(action);
            }
            if let Some(section) = menu.item_link(i, "section") {
                out.extend(actions(&section));
            }
        }
        out
    }

    #[test]
    fn every_page_holds_the_same_main_menu_under_its_own_verbs() {
        let shared = actions(&main(None));
        assert_eq!(shared, ["win.rescan", "app.big-screen", "app.preferences", "app.shortcuts", "app.about"]);
        assert_eq!(actions(&store()), [&["store.scan".to_string()][..], &shared].concat(), "the store's refresh is its header button");
    }
}
