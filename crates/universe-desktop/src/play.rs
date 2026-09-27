use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::backend;
use crate::game::GameObject;
use crate::pages::GamePage;
use crate::window::Window;

// A launch that returned but whose unit died at once never shows a session: past this, the game is no longer starting.
const STARTING: std::time::Duration = std::time::Duration::from_secs(6);

impl Window {
    pub(crate) fn setup_play_actions(&self) {
        let stop = gio::ActionEntry::builder("stop").activate(|win: &Self, _, _| win.stop_game()).build();
        let focus = gio::ActionEntry::builder("focus-game").activate(|win: &Self, _, _| win.focus_game()).build();
        let screenshot = gio::ActionEntry::builder("screenshot").activate(|win: &Self, _, _| win.screenshot()).build();
        let open = gio::ActionEntry::builder("open-game")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|win: &Self, _, param| {
                if let Some(game) = param.and_then(|p| p.get::<String>()).and_then(|id| win.app().library().get(&id)) {
                    win.open_game(&game);
                }
            })
            .build();
        let play = gio::ActionEntry::builder("play-game")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(|win: &Self, _, param| {
                if let Some(game) = param.and_then(|p| p.get::<String>()).and_then(|id| win.app().library().get(&id)) {
                    win.play(&game);
                }
            })
            .build();
        self.add_action_entries([stop, focus, screenshot, open, play]);
        self.sync_play_actions();
    }

    pub(crate) fn sync_play_actions(&self) {
        let running = self.app().current().is_some();
        for name in ["stop", "focus-game", "screenshot"] {
            if let Some(action) = self.lookup_action(name).and_downcast::<gio::SimpleAction>() {
                action.set_enabled(running);
            }
        }
    }

    /// A game already on the stack is popped back to rather than pushed twice: a page's tag is the game's id.
    pub fn open_game(&self, game: &GameObject) {
        let navigation = &self.imp().navigation;
        let tag = format!("game:{}", game.id());
        if let Some(page) = navigation.find_page(&tag) {
            navigation.pop_to_page(&page);
            return;
        }
        let page = GamePage::new(game);
        page.set_tag(Some(&tag));
        navigation.push(&page);
    }

    /// The monitor the window is on, as a connector: the game starts there.
    fn connector(&self) -> String {
        self.surface()
            .and_then(|surface| surface.display().monitor_at_surface(&surface))
            .and_then(|monitor| monitor.connector())
            .map(String::from)
            .unwrap_or_default()
    }

    pub fn play(&self, game: &GameObject) {
        let app = self.app();
        let Some(current) = app.current() else {
            self.launch(game);
            return;
        };
        if current.id == game.id() {
            self.focus_game();
            return;
        }
        let dialog = adw::AlertDialog::new(
            Some(&gettext("Quit {} and Start {}?").replacen("{}", &current.title, 1).replacen("{}", &game.title(), 1)),
            Some(&gettext("Only one game runs at a time. {} keeps what it saved itself.").replace("{}", &current.title)),
        );
        dialog.add_responses(&[("cancel", &gettext("_Cancel")), ("switch", &gettext("_Quit and Start"))]);
        dialog.set_response_appearance("switch", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        let (win, game) = (self.downgrade(), game.clone());
        dialog.connect_response(None, move |_, response| {
            let Some(win) = win.upgrade().filter(|_| response == "switch") else { return };
            win.app().set_pending(Some(game.id()));
            win.stop_game();
        });
        dialog.present(Some(self));
    }

    fn launch(&self, game: &GameObject) {
        game.set_launching(true);
        let (id, screen) = (game.id(), self.connector());
        let (win, game) = (self.downgrade(), game.clone());
        glib::spawn_future_local(async move {
            let result = backend::call_pinned(move |core| async move { core.launch(&id, &screen, "").await }).await;
            let Some(win) = win.upgrade() else { return };
            match result {
                Ok(_) => {
                    win.app().check_session();
                    glib::timeout_future(STARTING).await;
                    if game.launching() {
                        game.set_launching(false);
                    }
                }
                Err(e) => {
                    game.set_launching(false);
                    win.alert(&gettext("Could Not Start {}").replace("{}", &game.title()), &e.to_string());
                }
            }
        });
    }

    pub fn stop_game(&self) {
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::call(|core| async move { core.stop("").await }).await;
            let Some(win) = win.upgrade() else { return };
            match result {
                Ok(()) => win.app().check_session(),
                Err(e) => win.toast(adw::Toast::new(&e.to_string())),
            }
        });
    }

    pub fn focus_game(&self) {
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::call(|core| async move { core.focus_session().await }).await;
            if let (Some(win), Err(e)) = (win.upgrade(), result) {
                win.toast(adw::Toast::new(&e.to_string()));
            }
        });
    }

    fn screenshot(&self) {
        let win = self.downgrade();
        glib::spawn_future_local(async move {
            let result = backend::call(|core| async move { core.screenshot().await }).await;
            let Some(win) = win.upgrade() else { return };
            match result {
                Ok(_) => win.toast(adw::Toast::new(&gettext("Screenshot taken"))),
                Err(e) => win.toast(adw::Toast::new(&e.to_string())),
            }
        });
    }

    pub fn open_folder(&self, game: &GameObject) {
        let folder = game.row().folder.clone();
        if folder.is_empty() {
            return;
        }
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&folder)));
        let win = self.downgrade();
        launcher.launch(Some(self), gio::Cancellable::NONE, move |result| {
            if let (Some(win), Err(e)) = (win.upgrade(), result) {
                win.toast(adw::Toast::new(&e.to_string()));
            }
        });
    }

    pub fn alert(&self, heading: &str, body: &str) {
        let dialog = adw::AlertDialog::new(Some(heading), Some(body));
        dialog.add_response("close", &gettext("_Close"));
        dialog.present(Some(self));
    }
}
