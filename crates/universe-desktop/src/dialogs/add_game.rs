use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use serde_json::{json, Value};

use crate::backend;
use crate::dialogs::signin;
use crate::window::Window;

const LINUX_EXTENSIONS: [&str; 5] = ["", "sh", "x86_64", "x86", "appimage"];

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// The runners that could start `path`, likeliest first: its extension, then found on this machine, then Proton and Wine.
fn candidates(runners: Vec<Value>, path: &Path) -> Vec<Value> {
    let ext = if path.is_dir() { String::new() } else { path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default() };
    let takes = |r: &Value| {
        if text(r, "kind") == "linux" {
            LINUX_EXTENSIONS.contains(&ext.as_str())
        } else {
            strings(r, "extensions").iter().any(|e| e.to_lowercase() == ext)
        }
    };
    let found = |r: &Value| text(r, "kind") == "linux" || !text(r, "path").is_empty();
    let rank = |r: &Value| match text(r, "kind").as_str() {
        "proton" => 0,
        "wine" => 1,
        _ => 2,
    };
    let mut runners = runners;
    runners.sort_by_key(|r| (!takes(r), !found(r), rank(r), text(r, "name").to_lowercase()));
    runners
}

fn plural_games(n: usize) -> String {
    ngettext("{} game", "{} games", n as u32).replace("{}", &n.to_string())
}

struct AddGame {
    dialog: adw::Dialog,
    nav: adw::NavigationView,
    toasts: adw::ToastOverlay,
    win: glib::WeakRef<Window>,
}

impl AddGame {
    fn say(&self, text: &str) {
        self.toasts.add_toast(crate::dialogs::toast(text));
    }
}

/// Everything that brings games in: a file picked by hand, the Lutris and emulator imports, the stores.
pub fn present(win: &Window) {
    let dialog = adw::Dialog::builder().title(gettext("Add Games")).content_width(560).content_height(640).build();
    let toasts = adw::ToastOverlay::new();
    let nav = adw::NavigationView::new();
    toasts.set_child(Some(&nav));
    dialog.set_child(Some(&toasts));
    let this = Rc::new(AddGame { dialog: dialog.clone(), nav: nav.clone(), toasts, win: win.downgrade() });
    nav.add(&home(&this));
    let held = std::cell::RefCell::new(Some(this));
    dialog.connect_closed(move |_| {
        held.take();
    });
    dialog.present(Some(win));
}

fn page(title: &str, content: &impl IsA<gtk::Widget>) -> (adw::NavigationPage, adw::HeaderBar) {
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(content));
    (adw::NavigationPage::new(&toolbar, title), header)
}

fn home(this: &Rc<AddGame>) -> adw::NavigationPage {
    let prefs = adw::PreferencesPage::new();
    let pick = adw::PreferencesGroup::new();
    let file = adw::ActionRow::builder()
        .title(gettext("Game File…"))
        .subtitle(gettext("A program or a ROM: the runner and the title are guessed from it"))
        .activatable(true)
        .build();
    file.add_prefix(&gtk::Image::from_icon_name("document-open-symbolic"));
    file.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let weak = Rc::downgrade(this);
    file.connect_activated(move |_| {
        if let Some(this) = weak.upgrade() {
            pick_file(&this);
        }
    });
    pick.add(&file);
    prefs.add(&pick);

    let imports = adw::PreferencesGroup::builder().title(gettext("Import")).build();
    imports.add(&import_row(this, Import::Lutris));
    imports.add(&import_row(this, Import::Roms));
    prefs.add(&imports);

    let stores = adw::PreferencesGroup::builder().title(gettext("Stores")).build();
    prefs.add(&stores);
    let weak = Rc::downgrade(this);
    glib::spawn_future_local(async move {
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let Some(this) = weak.upgrade() else { return };
        for source in sources {
            let ready = source["enabled"].as_bool() == Some(true) && source["logged_in"].as_bool() == Some(true);
            let row = crate::rows::plain(adw::ActionRow::builder().build(), text(&source, "name"), signin::status(&source));
            let button = gtk::Button::builder().label(if ready { gettext("Browse") } else { gettext("Set Up") }).valign(gtk::Align::Center).build();
            let weak = Rc::downgrade(&this);
            button.connect_clicked(move |_| {
                let Some(this) = weak.upgrade() else { return };
                this.dialog.close();
                let Some(win) = this.win.upgrade() else { return };
                if ready {
                    let _ = WidgetExt::activate_action(&win, "win.view", Some(&"store".to_variant()));
                } else {
                    let _ = WidgetExt::activate_action(&win, "app.preferences-page", Some(&"stores".to_variant()));
                }
            });
            row.add_suffix(&button);
            stores.add(&row);
        }
        stores.set_visible(stores.first_child().is_some());
    });
    page(&gettext("Add Games"), &prefs).0
}

#[derive(Clone, Copy)]
enum Import {
    Lutris,
    Roms,
}

/// What an import would bring, read first; the import itself asks before it writes.
fn import_row(this: &Rc<AddGame>, kind: Import) -> adw::ActionRow {
    let (title, subtitle) = match kind {
        Import::Lutris => (gettext("Lutris"), gettext("Its games with their hours; games already here are kept")),
        Import::Roms => (gettext("Emulator Folders"), gettext("The games under the folders your emulators list")),
    };
    let row = adw::ActionRow::builder().title(title).subtitle(subtitle).build();
    row.add_prefix(&gtk::Image::from_icon_name(match kind {
        Import::Lutris => "source-lutris-symbolic",
        Import::Roms => "input-gaming-symbolic",
    }));
    let spinner = adw::Spinner::new();
    row.add_suffix(&spinner);
    let button = gtk::Button::builder().label(gettext("Import")).valign(gtk::Align::Center).visible(false).css_classes(["suggested-action"]).build();
    row.add_suffix(&button);
    let (weak, row_ref, button_ref) = (Rc::downgrade(this), row.clone(), button.clone());
    glib::spawn_future_local(async move {
        let preview = preview(kind).await;
        spinner.set_visible(false);
        let Some(_this) = weak.upgrade() else { return };
        match preview {
            Ok(0) => row_ref.set_subtitle(&gettext("Nothing new")),
            Ok(n) => {
                row_ref.set_subtitle(&gettext("{} to import").replace("{}", &plural_games(n)));
                button_ref.set_visible(true);
            }
            Err(e) => row_ref.set_subtitle(&e),
        }
    });
    let weak = Rc::downgrade(this);
    button.connect_clicked(move |button| {
        let Some(this) = weak.upgrade() else { return };
        button.set_sensitive(false);
        let (weak, button) = (Rc::downgrade(&this), button.clone());
        glib::spawn_future_local(async move {
            let result = apply(kind).await;
            let Some(this) = weak.upgrade() else { return };
            match result {
                Ok((n, art)) => {
                    button.set_visible(false);
                    this.say(&gettext("Added {}").replace("{}", &plural_games(n)));
                    if let Some(win) = this.win.upgrade() {
                        win.app().library().refresh(&[]).await;
                        win.app().fetch_art(art);
                    }
                }
                Err(e) => {
                    button.set_sensitive(true);
                    this.say(&e);
                }
            }
        });
    });
    row
}

async fn preview(kind: Import) -> Result<usize, String> {
    backend::pinned(move |core| async move {
        match kind {
            Import::Lutris => core.import_lutris(false).await.map(|r| r.imported.len()),
            Import::Roms => core.import_roms(false).await.map(|r| r.imported.len()),
        }
    })
    .await
    .map_err(|e| match kind {
        Import::Lutris => gettext("Lutris is not set up here"),
        Import::Roms => e.to_string(),
    })
}

/// How many games came in, and the `(id, title)` of those whose art is still to fetch.
async fn apply(kind: Import) -> Result<(usize, Vec<(String, String)>), String> {
    backend::pinned(move |core| async move {
        match kind {
            Import::Lutris => core.import_lutris(true).await.map(|r| (r.imported.len(), Vec::new())),
            Import::Roms => core.import_roms(true).await.map(|r| (r.imported.len(), r.imported.into_iter().map(|f| (f.id, f.title)).collect())),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

fn pick_file(this: &Rc<AddGame>) {
    let dialog = gtk::FileDialog::builder().title(gettext("Pick a Game File")).modal(true).build();
    let weak = Rc::downgrade(this);
    let root = this.win.upgrade();
    dialog.open(root.as_ref(), gio::Cancellable::NONE, move |result| {
        let (Some(this), Ok(file)) = (weak.upgrade(), result) else { return };
        let Some(path) = file.path() else { return };
        let weak = Rc::downgrade(&this);
        glib::spawn_future_local(async move {
            let runners = backend::run(async { backend::core().runners().await }).await;
            if let Some(this) = weak.upgrade() {
                let page = file_page(&this, path.clone(), candidates(runners, &path));
                this.nav.push(&page);
            }
        });
    });
}

/// The picked file as a game: its title, the runner that starts it, the platform when the runner has several.
fn file_page(this: &Rc<AddGame>, path: PathBuf, runners: Vec<Value>) -> adw::NavigationPage {
    let prefs = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::new();
    let title = adw::EntryRow::builder().title(gettext("Title")).text(universe::core::title_of(&path)).build();
    group.add(&title);
    let names: Vec<String> = runners.iter().map(|r| text(r, "name")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let runner = adw::ComboRow::builder().title(gettext("Runner")).model(&gtk::StringList::new(&names)).build();
    group.add(&runner);
    let platform = adw::ComboRow::builder().title(gettext("Platform")).build();
    group.add(&platform);
    let file = crate::rows::plain(adw::ActionRow::builder().subtitle_selectable(true).build(), gettext("File"), path.to_string_lossy());
    group.add(&file);
    prefs.add(&group);

    let runners = Rc::new(runners);
    let platforms_of = {
        let runners = runners.clone();
        move |index: u32| runners.get(index as usize).map(|r| strings(r, "platforms")).unwrap_or_default()
    };
    let sync_platforms = {
        let (platform, platforms_of) = (platform.clone(), platforms_of.clone());
        move |index: u32| {
            let list = platforms_of(index);
            let shown: Vec<String> = list.iter().map(|p| crate::library::platform_name(p)).collect();
            let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
            platform.set_model(Some(&gtk::StringList::new(&shown)));
            platform.set_visible(list.len() > 1);
        }
    };
    sync_platforms(0);
    runner.connect_selected_notify(move |row| sync_platforms(row.selected()));

    let (page, header) = page(&gettext("Add a Game File"), &prefs);
    let add = gtk::Button::builder().label(gettext("_Add")).use_underline(true).css_classes(["suggested-action"]).build();
    header.pack_end(&add);
    let weak = Rc::downgrade(this);
    add.connect_clicked(move |button| {
        let Some(this) = weak.upgrade() else { return };
        let Some(chosen) = runners.get(runner.selected() as usize) else { return };
        let platform = platforms_of(runner.selected()).get(platform.selected() as usize).cloned().unwrap_or_default();
        let spec = json!({"runner": text(chosen, "id"), "exe": path.to_string_lossy(), "title": title.text().trim(), "platform": platform});
        button.set_sensitive(false);
        let (weak, button) = (Rc::downgrade(&this), button.clone());
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.add_game(&spec).await }).await;
            let Some(this) = weak.upgrade() else { return };
            button.set_sensitive(true);
            match result {
                Ok(id) => {
                    this.dialog.close();
                    let Some(win) = this.win.upgrade() else { return };
                    win.app().library().refresh(std::slice::from_ref(&id)).await;
                    if let Some(game) = win.app().library().get(&id) {
                        win.toast(crate::dialogs::toast(&gettext("Added “{}”").replace("{}", &game.title())));
                        win.open_game(&game);
                    }
                }
                Err(e) => this.say(&e.to_string()),
            }
        });
    });
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_proposes_the_runners_that_take_its_extension_first() {
        let runner = |id: &str, kind: &str, path: &str, exts: &[&str]| json!({"id": id, "name": id, "kind": kind, "path": path, "extensions": exts});
        let runners = vec![
            runner("dolphin", "emulator", "/bin/dolphin", &["iso", "rvz"]),
            runner("wine", "wine", "/bin/wine", &["exe"]),
            runner("proton", "proton", "/bin/umu-run", &["exe"]),
            runner("linux", "linux", "", &[]),
            runner("pcsx2", "emulator", "", &["iso"]),
        ];
        let ids = |p: &str| candidates(runners.clone(), Path::new(p)).iter().map(|r| text(r, "id")).collect::<Vec<_>>();
        assert_eq!(ids("/g/Hades.exe")[..2], ["proton", "wine"]);
        assert_eq!(ids("/g/Melee.ISO")[..2], ["dolphin", "pcsx2"], "found first, the case of the extension aside");
        assert_eq!(ids("/g/start.sh")[0], "linux");
    }
}
