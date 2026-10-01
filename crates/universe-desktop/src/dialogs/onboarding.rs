use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;
use serde_json::Value;
use universe::discover::Launcher;

use crate::backend;
use crate::config;
use crate::dialogs::signin;
use crate::window::Window;

const IMPORTERS: [&str; 2] = ["lutris", "roms"];

fn plural_games(n: usize) -> String {
    ngettext("{} game", "{} games", n as u32).replace("{}", &n.to_string())
}

fn signed_out(source: &Value) -> bool {
    source["enabled"].as_bool() == Some(true) && source["available"].as_bool() != Some(false) && source["logged_in"].as_bool() != Some(true)
}

/// Why the store a launcher's `via` names cannot adopt its games; `None` when it can, turned on first if it is off.
fn unusable(via: &str, sources: &[Value]) -> Option<String> {
    if IMPORTERS.contains(&via) {
        return None;
    }
    match sources.iter().find(|s| s["id"] == via) {
        None => Some(gettext("no {} source").replace("{}", via)),
        Some(s) if s["available"].as_bool() == Some(false) => {
            let missing: Vec<&str> = s["missing"].as_array().map(|m| m.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            Some(gettext("needs {}").replace("{}", &if missing.is_empty() { gettext("its programs") } else { missing.join(", ") }))
        }
        Some(_) => None,
    }
}

struct Onboarding {
    dialog: adw::Dialog,
    nav: adw::NavigationView,
    toasts: adw::ToastOverlay,
    win: glib::WeakRef<Window>,
    /// What the steps brought in, for the last page: `(what, how much)`.
    summary: RefCell<Vec<(String, String)>>,
    gog_dirs: RefCell<Vec<String>>,
    sources: RefCell<Vec<Value>>,
    stores: RefCell<Vec<Value>>,
    /// Launchers whose store found nothing it could call owned while signed out: `(via, launcher)`, adopted again at its sign-in.
    waiting: RefCell<Vec<(String, String)>>,
}

impl Onboarding {
    fn say(&self, text: &str) {
        self.toasts.add_toast(crate::dialogs::toast(text));
    }

    /// After an adoption that found nothing: whether `via`'s store is signed out, then listed on the stores step and
    /// `launcher` adopted again at its sign-in.
    async fn waits_for_sign_in(&self, via: &str, launcher: &str) -> bool {
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let store = sources.iter().find(|s| s["id"] == via && signed_out(s)).cloned();
        self.sources.replace(sources);
        let Some(store) = store else { return false };
        if !self.stores.borrow().iter().any(|s| s["id"] == via) {
            self.stores.borrow_mut().push(store);
        }
        self.waiting.borrow_mut().push((via.to_string(), launcher.to_string()));
        true
    }
}

/// The first run with an empty library: what other launchers hold here, the stores to sign in to, then off to play.
pub fn present(win: &Window) {
    let dialog = adw::Dialog::builder().title(gettext("Welcome")).content_width(560).content_height(680).build();
    let toasts = adw::ToastOverlay::new();
    let nav = adw::NavigationView::new();
    toasts.set_child(Some(&nav));
    dialog.set_child(Some(&toasts));
    let this = Rc::new(Onboarding {
        dialog: dialog.clone(),
        nav: nav.clone(),
        toasts,
        win: win.downgrade(),
        summary: RefCell::default(),
        gog_dirs: RefCell::default(),
        sources: RefCell::default(),
        stores: RefCell::default(),
        waiting: RefCell::default(),
    });
    nav.add(&found_page(&this));
    let (weak, held) = (win.downgrade(), RefCell::new(Some(this)));
    dialog.connect_closed(move |_| {
        held.take();
        if let Some(win) = weak.upgrade() {
            win.set_onboarded();
        }
    });
    dialog.present(Some(win));
}

/// A page of the flow: a header bar, a title and a line over the content, a button to go on at the foot.
fn step(title: &str, icon: Option<&str>, heading: &str, body: &str, content: &gtk::Widget, next: &str, go: impl Fn() + 'static) -> adw::NavigationPage {
    let column =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(24).margin_top(12).margin_bottom(24).margin_start(12).margin_end(12).build();
    let head = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
    if let Some(icon) = icon {
        head.append(&gtk::Image::builder().icon_name(icon).pixel_size(96).margin_bottom(12).build());
    }
    head.append(&gtk::Label::builder().label(heading).wrap(true).justify(gtk::Justification::Center).css_classes(["title-1"]).build());
    head.append(&gtk::Label::builder().label(body).wrap(true).justify(gtk::Justification::Center).css_classes(["body", "dimmed"]).build());
    column.append(&head);
    column.append(content);
    let button = gtk::Button::builder().label(next).use_underline(true).halign(gtk::Align::Center).css_classes(["pill", "suggested-action"]).build();
    button.connect_clicked(move |_| go());
    column.append(&button);
    let clamp = adw::Clamp::builder().maximum_size(480).child(&column).build();
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&clamp).build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    toolbar.set_content(Some(&scroll));
    adw::NavigationPage::new(&toolbar, title)
}

fn found_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
    let group = adw::PreferencesGroup::builder().title(gettext("On This Machine")).build();
    let looking = adw::ActionRow::builder().title(gettext("Looking for other launchers…")).build();
    looking.add_suffix(&adw::Spinner::new());
    group.add(&looking);
    content.append(&group);
    let weak = Rc::downgrade(this);
    glib::spawn_future_local(async move {
        let report = backend::run(async { backend::core().discover().await }).await;
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let Some(this) = weak.upgrade() else { return };
        group.remove(&looking);
        this.gog_dirs.replace(report.gog_dirs.clone());
        this.stores.replace(sources.iter().filter(|s| signed_out(s)).cloned().collect());
        this.sources.replace(sources);
        let found: Vec<&Launcher> = report.launchers.iter().filter(|l| l.found).collect();
        if found.is_empty() {
            group
                .add(&adw::ActionRow::builder().title(gettext("No other launchers found")).subtitle(gettext("Games can join any time from Add Games")).build());
        }
        for launcher in found {
            group.add(&launcher_row(&this, launcher));
        }
    });
    let weak = Rc::downgrade(this);
    step(
        &gettext("Welcome"),
        Some(config::APP_ID),
        &gettext("Welcome to Universe"),
        &gettext("Every game you own, from every launcher and store, in one library. Start with what is already here."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade() {
                let next = if this.stores.borrow().is_empty() { done_page(&this) } else { stores_page(&this) };
                this.nav.push(&next);
            }
        },
    )
}

fn launcher_row(this: &Rc<Onboarding>, launcher: &Launcher) -> adw::ActionRow {
    let why = Some(launcher.via.as_str()).filter(|via| !via.is_empty()).and_then(|via| unusable(via, &this.sources.borrow()));
    let importable = launcher.importable && launcher.games > 0 && why.is_none();
    let subtitle = if launcher.games == 0 {
        gettext("No games")
    } else if importable {
        plural_games(launcher.games)
    } else if let Some(why) = &why {
        format!("{} · {why}", plural_games(launcher.games))
    } else {
        gettext("{} · not importable yet").replace("{}", &plural_games(launcher.games))
    };
    let row = crate::rows::plain(adw::ActionRow::builder().build(), launcher.name.clone(), subtitle);
    if !launcher.titles.is_empty() {
        row.set_tooltip_text(Some(&launcher.titles.join(", ")));
    }
    if !importable {
        return row;
    }
    let label = if IMPORTERS.contains(&launcher.via.as_str()) { gettext("Import") } else { gettext("Adopt") };
    let button = gtk::Button::builder().label(label).valign(gtk::Align::Center).css_classes(["suggested-action"]).build();
    row.add_suffix(&button);
    let (weak, via, name, row_ref) = (Rc::downgrade(this), launcher.via.clone(), launcher.name.clone(), row.clone());
    button.connect_clicked(move |button| {
        let Some(this) = weak.upgrade() else { return };
        button.set_sensitive(false);
        let spinner = adw::Spinner::new();
        row_ref.add_suffix(&spinner);
        let (weak, via, name, row, button) = (Rc::downgrade(&this), via.clone(), name.clone(), row_ref.clone(), button.clone());
        let dirs = this.gog_dirs.borrow().clone();
        let enable = this.sources.borrow().iter().any(|s| s["id"] == via.as_str() && s["enabled"].as_bool() != Some(true));
        glib::spawn_future_local(async move {
            let result = bring_in(&via, dirs, enable).await;
            row.remove(&spinner);
            let Some(this) = weak.upgrade() else { return };
            match result {
                Ok((0, _)) if !IMPORTERS.contains(&via.as_str()) && this.waits_for_sign_in(&via, &name).await => {
                    row.remove(&button);
                    let store = this.sources.borrow().iter().find(|s| s["id"] == via.as_str()).and_then(|s| s["name"].as_str()).unwrap_or(&via).to_string();
                    row.set_subtitle(&gettext("Sign in to {} to adopt them").replace("{}", &store));
                }
                Ok((n, art)) => {
                    row.remove(&button);
                    row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
                    row.set_subtitle(&if n == 0 { gettext("Nothing new") } else { gettext("{} added").replace("{}", &plural_games(n)) });
                    if n > 0 {
                        this.summary.borrow_mut().push((name, plural_games(n)));
                    }
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

/// Lutris and the emulators' folders are imported; another launcher's installs are adopted by the scan of the store `via`
/// names, turned on first when `enable` says it is off (the GOG store's scan reads Heroic's GOG folders too). The count,
/// and the `(id, title)` of the games whose art is still to fetch.
async fn bring_in(via: &str, gog_dirs: Vec<String>, enable: bool) -> Result<(usize, Vec<(String, String)>), String> {
    let via = via.to_string();
    backend::pinned(move |core| async move {
        match via.as_str() {
            "lutris" => core.import_lutris(true).await.map(|r| (r.imported.len(), Vec::new())),
            "roms" => core.import_roms(true).await.map(|r| (r.imported.len(), r.imported.into_iter().map(|f| (f.id, f.title)).collect())),
            store => {
                if enable {
                    core.enable_source(store, true).await?;
                }
                if store == "gog" {
                    let settings = core.source_settings("gog", "").await?;
                    let mut dirs: Vec<String> =
                        settings["scan_dirs"].as_str().unwrap_or_default().split(',').map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
                    let before = dirs.len();
                    dirs.extend(gog_dirs.into_iter().filter(|d| !dirs.contains(d)).collect::<Vec<_>>());
                    if dirs.len() != before {
                        core.set_source_setting("gog", "", "scan_dirs", &dirs.join(",")).await?;
                    }
                }
                core.source_scan(store, None).await.map(|n| (n, Vec::new()))
            }
        }
    })
    .await
    .map_err(|e| e.to_string())
}

fn stores_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).build();
    for source in this.stores.borrow().iter() {
        let name = source["name"].as_str().unwrap_or_default().to_string();
        let weak = Rc::downgrade(this);
        let say: signin::Say = Rc::new(move |text: &str| {
            if let Some(this) = weak.upgrade() {
                this.say(text);
            }
        });
        let (weak, id) = (Rc::downgrade(this), source["id"].as_str().unwrap_or_default().to_string());
        let signed_in: signin::Say = Rc::new(move |_: &str| {
            let Some(this) = weak.upgrade() else { return };
            this.summary.borrow_mut().push((name.clone(), gettext("Signed in")));
            let launchers: Vec<String> = this.waiting.borrow().iter().filter(|(via, _)| *via == id).map(|(_, launcher)| launcher.clone()).collect();
            if launchers.is_empty() {
                return;
            }
            this.waiting.borrow_mut().retain(|(via, _)| *via != id);
            let (weak, id) = (Rc::downgrade(&this), id.clone());
            glib::spawn_future_local(async move {
                let result = bring_in(&id, Vec::new(), false).await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok((0, _)) => {}
                    Ok((n, _)) => {
                        this.summary.borrow_mut().push((launchers.join(", "), gettext("{} added").replace("{}", &plural_games(n))));
                        if let Some(win) = this.win.upgrade() {
                            win.app().library().refresh(&[]).await;
                        }
                    }
                    Err(e) => this.say(&e),
                }
            });
        });
        let group = signin::account_group(source, say, signed_in);
        group.set_title(source["name"].as_str().unwrap_or_default());
        content.append(&group);
    }
    let weak = Rc::downgrade(this);
    step(
        &gettext("Stores"),
        None,
        &gettext("Your Stores"),
        &gettext("Sign in to list the games you own and install them from Store."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade() {
                this.nav.push(&done_page(&this));
            }
        },
    )
}

fn done_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
    content.append(&gtk::Image::builder().icon_name("object-select-symbolic").pixel_size(72).css_classes(["success"]).build());
    let group = adw::PreferencesGroup::new();
    let summary = this.summary.borrow().clone();
    if summary.is_empty() {
        group.add(
            &adw::ActionRow::builder().title(gettext("Nothing added yet")).subtitle(gettext("Games can join any time from Add Games, with Ctrl+N")).build(),
        );
    }
    for (what, how) in summary {
        group.add(&crate::rows::plain(adw::ActionRow::builder().build(), what, how));
    }
    content.append(&group);
    let weak = Rc::downgrade(this);
    step(
        &gettext("Done"),
        None,
        &gettext("You’re All Set"),
        &gettext("Everything here can be changed later in Preferences."),
        content.upcast_ref(),
        &gettext("_Start Playing"),
        move || {
            if let Some(this) = weak.upgrade() {
                this.dialog.close();
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launchers_games_go_to_the_store_its_via_names_when_that_store_can_run() {
        let sources = [
            serde_json::json!({"id": "gog", "available": true, "enabled": true}),
            serde_json::json!({"id": "epic", "available": true, "enabled": false}),
            serde_json::json!({"id": "steam", "available": false, "missing": ["steam"]}),
        ];
        assert_eq!(unusable("lutris", &sources), None, "an importer needs no store");
        assert_eq!(unusable("epic", &sources), None, "a store that is off is turned on");
        assert_eq!(unusable("steam", &sources).as_deref(), Some("needs steam"));
        assert_eq!(unusable("itch", &sources).as_deref(), Some("no itch source"));
        assert!(signed_out(&sources[0]) && !signed_out(&sources[1]), "off is not signed out: adopting turns it on");
    }
}
