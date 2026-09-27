use adw::prelude::*;
use gtk::{gio, glib};

use crate::game::GameObject;
use crate::window::Window;

/// The `game.*` actions of a card or a page, run by the window on the game in hand at activation.
pub const GAME: [&str; 10] = ["play", "details", "settings", "artwork", "open-folder", "favorite", "unfavorite", "hide", "unhide", "remove"];

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
    let playable = game.is_some_and(|g| g.installed() && !g.playing() && !g.launching());
    let folder = game.is_some_and(|g| !g.row().folder.is_empty());
    let known = game.is_some();
    for (name, on) in [
        ("play", playable),
        ("details", known),
        ("settings", false),
        ("artwork", false),
        ("open-folder", folder),
        ("remove", false),
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

/// Keeps `sync_game` current while `game` is bound; the handlers go with `unbind`.
pub fn follow_game(group: &gio::SimpleActionGroup, game: &GameObject) -> Vec<glib::SignalHandlerId> {
    sync_game(group, Some(game));
    ["hidden", "favorite", "installed", "playing", "launching"]
        .into_iter()
        .map(|prop| {
            let group = group.clone();
            game.connect_notify_local(Some(prop), move |game, _| sync_game(&group, Some(game)))
        })
        .collect()
}
