use adw::prelude::*;
use gtk::{gio, glib};

use crate::game::GameObject;
use crate::window::Window;

/// The `game.*` actions of a card or a page, run by the window on the game in hand at activation.
pub const GAME: [&str; 14] = [
    "play",
    "details",
    "settings",
    "artwork",
    "open-folder",
    "favorite",
    "unfavorite",
    "hide",
    "unhide",
    "update",
    "mark-current",
    "uninstall",
    "remove",
    "remove-purge",
];

pub fn install_game(widget: &impl IsA<gtk::Widget>, game: impl Fn() -> Option<GameObject> + Clone + 'static) -> gio::SimpleActionGroup {
    let group = gio::SimpleActionGroup::new();
    for name in GAME {
        let action = gio::SimpleAction::new(name, None);
        let (weak, game) = (widget.upcast_ref::<gtk::Widget>().downgrade(), game.clone());
        action.connect_activate(move |_, _| {
            let (Some(widget), Some(game)) = (weak.upgrade(), game()) else { return };
            if let Some(win) = widget.root().and_downcast::<Window>() {
                win.game_action(name, &game);
            }
        });
        group.add_action(&action);
    }
    widget.insert_action_group("game", Some(&group));
    group
}

/// Which of the actions apply to `game` as it is now.
pub fn sync_game(group: &gio::SimpleActionGroup, game: Option<&GameObject>) {
    let (hidden, favorite) = game.map(|g| (g.hidden(), g.favorite())).unwrap_or_default();
    let idle = game.is_some_and(|g| !g.playing() && !g.launching());
    let playable = idle && game.is_some_and(|g| g.installed());
    let folder = game.is_some_and(|g| !g.row().folder.is_empty());
    let known = game.is_some();
    for (name, on) in [
        ("play", playable),
        ("details", known),
        ("settings", known),
        ("artwork", known),
        ("open-folder", folder),
        ("update", idle && game.is_some_and(|g| g.updatable())),
        ("mark-current", idle && game.is_some_and(|g| g.markable())),
        ("uninstall", idle && game.is_some_and(|g| g.has_install())),
        ("remove", idle),
        ("remove-purge", idle && game.is_some_and(|g| g.has_prefix())),
        ("favorite", known && !favorite),
        ("unfavorite", known && favorite),
        ("hide", known && !hidden),
        ("unhide", known && hidden),
    ] {
        if let Some(action) = group.lookup_action(name).and_downcast::<gio::SimpleAction>() {
            action.set_enabled(on);
        }
    }
}

pub fn game_menu(widget: &impl IsA<gtk::Widget>, game: impl Fn() -> Option<GameObject> + Clone + 'static) -> gio::SimpleActionGroup {
    let group = install_game(widget, game.clone());
    let (weak, held) = (widget.upcast_ref::<gtk::Widget>().downgrade(), group.clone());
    let open = std::rc::Rc::new(move |x: f64, y: f64| {
        let (Some(widget), Some(game)) = (weak.upgrade(), game()) else { return };
        sync_game(&held, Some(&game));
        let popover = gtk::PopoverMenu::from_model(Some(&crate::menus::game()));
        popover.set_parent(&widget);
        popover.set_has_arrow(false);
        popover.set_halign(gtk::Align::Start);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        // The item's action runs after the popover closes, found through its parent: it lets go once that is done.
        popover.connect_closed(|popover| {
            let popover = popover.clone();
            glib::idle_add_local_once(move || popover.unparent());
        });
        popover.popup();
    });
    let click = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
    let on_click = open.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        on_click(x, y);
    });
    widget.add_controller(click);
    let press = gtk::GestureLongPress::builder().touch_only(true).build();
    press.connect_pressed(move |gesture, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        open(x, y);
    });
    widget.add_controller(press);
    group
}

/// Keeps `sync_game` current while `game` is bound; the handlers go with `unbind`.
pub fn follow_game(group: &gio::SimpleActionGroup, game: &GameObject) -> Vec<glib::SignalHandlerId> {
    sync_game(group, Some(game));
    ["hidden", "favorite", "installed", "playing", "launching", "updatable", "markable", "has-install", "has-prefix"]
        .into_iter()
        .map(|prop| {
            let group = group.clone();
            game.connect_notify_local(Some(prop), move |game, _| sync_game(&group, Some(game)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use universe::upstream::Check;

    use super::*;

    /// Which of Update and Mark as Current a game judged by `check` offers.
    fn offered(check: Check) -> Vec<&'static str> {
        let r = universe::library::Resolved { game: universe::game::Game::new("Game"), check, ..Default::default() };
        let row = crate::game::Row::of(&r, std::slice::from_ref(&r), &universe::config::Config::default());
        let group = gio::SimpleActionGroup::new();
        for name in GAME {
            group.add_action(&gio::SimpleAction::new(name, None));
        }
        sync_game(&group, Some(&GameObject::new(row)));
        ["update", "mark-current"].into_iter().filter(|name| group.lookup_action(name).is_some_and(|a| a.is_enabled())).collect()
    }

    #[test]
    fn a_store_update_offers_update_and_one_by_hand_mark_as_current() {
        let none: [&str; 0] = [];
        let store = |status, how, reason| Check { status, how, reason, store: "gog".into(), store_id: "1".into(), ..Default::default() };
        assert_eq!(offered(store("update", "apply", "")), ["update"]);
        assert_eq!(offered(store("current", "apply", "")), none);
        assert_eq!(offered(store("update", "manual", "not-owned")), none, "the store refuses it, and its build is the install's own");
        let steam = |status, markable| Check { status, how: "manual", markable, ..Default::default() };
        assert_eq!(offered(steam("update", true)), ["mark-current"]);
        assert_eq!(offered(steam("unknown", true)), ["mark-current"], "upstream's build gives it a baseline");
        assert_eq!(offered(steam("current", true)), none);
        assert_eq!(offered(steam("untracked", false)), none, "nothing upstream to ask");
    }
}
