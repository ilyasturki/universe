use gettextrs::gettext;
use gtk::gio;
use gtk::prelude::*;

/// The game menu's verbs that only show when they apply.
const SHOWN_WHEN_ENABLED: [&str; 7] = ["game.favorite", "game.unfavorite", "game.hide", "game.unhide", "game.update", "game.uninstall", "game.remove-purge"];

fn section(items: &[(String, &str)]) -> gio::Menu {
    let menu = gio::Menu::new();
    for (label, action) in items {
        let item = gio::MenuItem::new(Some(label), Some(action));
        let hidden = if *action == "app.big-screen" { Some("action-missing") } else { SHOWN_WHEN_ENABLED.contains(action).then_some("action-disabled") };
        if let Some(hidden) = hidden {
            item.set_attribute_value("hidden-when", Some(&hidden.to_variant()));
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
    menu.append_section(None, &section(&[(gettext("_Rescan Library"), "win.rescan"), (gettext("Library _Artwork…"), "app.artwork")]));
    menu.append_section(None, &section(&[(gettext("Open _Big Screen"), "app.big-screen")]));
    menu.append_section(None, &section(&[(gettext("System _Check"), "app.system-check"), (gettext("_Storage"), "app.storage")]));
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

/// A game's menu over the `game.*` actions: a card's ⋮, and a right click or a long press on whatever stands for a game.
pub fn game() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &section(&[(gettext("_Play"), "game.play"), (gettext("_Details"), "game.details")]));
    menu.append_section(None, &section(&[(gettext("Game _Settings"), "game.settings"), (gettext("_Artwork"), "game.artwork")]));
    menu.append_section(
        None,
        &section(&[
            (gettext("Add to _Favourites"), "game.favorite"),
            (gettext("Remove From _Favourites"), "game.unfavorite"),
            (gettext("_Hide"), "game.hide"),
            (gettext("_Unhide"), "game.unhide"),
        ]),
    );
    menu.append_section(
        None,
        &section(&[
            (gettext("_Update"), "game.update"),
            (gettext("U_ninstall…"), "game.uninstall"),
            (gettext("_Remove From Library"), "game.remove"),
            (gettext("Remove With Wine _Prefix…"), "game.remove-purge"),
        ]),
    );
    menu
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(menu: &impl IsA<gio::MenuModel>) -> Vec<(String, Option<String>)> {
        let mut out = Vec::new();
        for i in 0..menu.n_items() {
            if let Some(action) = menu.item_attribute_value(i, "action", None).and_then(|v| v.get::<String>()) {
                out.push((action, menu.item_attribute_value(i, "hidden-when", None).and_then(|v| v.get::<String>())));
            }
            if let Some(section) = menu.item_link(i, "section") {
                out.extend(items(&section));
            }
        }
        out
    }

    fn actions(menu: &impl IsA<gio::MenuModel>) -> Vec<String> {
        items(menu).into_iter().map(|(action, _)| action).collect()
    }

    #[test]
    fn every_page_holds_the_same_main_menu_under_its_own_verbs() {
        let shared = actions(&main(None));
        assert_eq!(shared, ["win.rescan", "app.artwork", "app.big-screen", "app.system-check", "app.storage", "app.preferences", "app.shortcuts", "app.about"]);
        assert_eq!(actions(&store()), [&["store.scan".to_string()][..], &shared].concat(), "the store's refresh is its header button");
    }

    #[test]
    fn the_game_menu_runs_the_game_actions_and_hides_what_does_not_apply() {
        let listed = items(&game());
        for (action, hidden) in &listed {
            let name = action.strip_prefix("game.").expect("a game.* action");
            assert!(crate::actions::GAME.contains(&name), "{action} is one the card installs");
            let toggled = !matches!(name, "play" | "details" | "settings" | "artwork" | "remove");
            assert_eq!(hidden.as_deref(), toggled.then_some("action-disabled"), "{action}");
        }
        assert_eq!(listed.len(), 12);
    }
}
