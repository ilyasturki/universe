use adw::prelude::*;
use gettextrs::gettext;
use universe::forms::Form;

use crate::form_view::FormView;
use crate::game::GameObject;
use crate::window::Window;

/// Every setting one game can set: its launch cards, its runner's program and options, its modules' and source's keys.
pub fn present(win: &Window, game: &GameObject) {
    let dialog = adw::PreferencesDialog::builder().title(gettext("{} Settings").replace("{}", &game.title())).search_enabled(true).content_height(720).build();
    let view = FormView::new(Form::Game(game.id()), &dialog, win.connector());
    view.set_runner_name(&game.row().runner_name);
    view.page.set_title(&gettext("Settings"));
    dialog.add(&view.page);
    view.load();
    dialog.connect_closed(move |_| {
        let _ = &view;
    });
    dialog.present(Some(win));
}
