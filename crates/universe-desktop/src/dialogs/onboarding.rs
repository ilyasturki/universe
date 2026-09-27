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

fn plural_games(n: usize) -> String {
    ngettext("{} game", "{} games", n as u32).replace("{}", &n.to_string())
}

struct Onboarding {
    dialog: adw::Dialog,
    nav: adw::NavigationView,
    toasts: adw::ToastOverlay,
    win: glib::WeakRef<Window>,
    /// What the steps brought in, for the last page: `(what, how much)`.
    summary: RefCell<Vec<(String, String)>>,
    gog_dirs: RefCell<Vec<String>>,
    stores: RefCell<Vec<Value>>,
}

impl Onboarding {
    fn say(&self, text: &str) {
        self.toasts.add_toast(adw::Toast::new(text));
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
        stores: RefCell::default(),
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
        let gog = sources.iter().any(|s| s["id"] == "gog" && s["enabled"].as_bool() == Some(true) && s["available"].as_bool() != Some(false));
        this.stores.replace(
            sources
                .into_iter()
                .filter(|s| s["enabled"].as_bool() == Some(true) && s["available"].as_bool() != Some(false) && s["logged_in"].as_bool() != Some(true))
                .collect(),
        );
        let found: Vec<&Launcher> = report.launchers.iter().filter(|l| l.found).collect();
        if found.is_empty() {
            group
                .add(&adw::ActionRow::builder().title(gettext("No other launchers found")).subtitle(gettext("Games can join any time from Add Games")).build());
        }
        for launcher in found {
            group.add(&launcher_row(&this, launcher, gog));
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

fn launcher_row(this: &Rc<Onboarding>, launcher: &Launcher, gog: bool) -> adw::ActionRow {
    let importable = launcher.importable && launcher.games > 0 && (launcher.via != "gog" || gog);
    let subtitle = if launcher.games == 0 {
        gettext("No games")
    } else if importable {
        plural_games(launcher.games)
    } else if launcher.via == "gog" {
        gettext("{} · needs the GOG source").replace("{}", &plural_games(launcher.games))
    } else {
        gettext("{} · not importable yet").replace("{}", &plural_games(launcher.games))
    };
    let row = adw::ActionRow::builder().title(launcher.name.clone()).subtitle(subtitle).use_markup(false).build();
    if !launcher.titles.is_empty() {
        row.set_tooltip_text(Some(&launcher.titles.join(", ")));
    }
    if !importable {
        return row;
    }
    let label = if launcher.via == "gog" { gettext("Adopt") } else { gettext("Import") };
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
        glib::spawn_future_local(async move {
            let result = bring_in(&via, dirs).await;
            row.remove(&spinner);
            let Some(this) = weak.upgrade() else { return };
            match result {
                Ok(n) => {
                    row.remove(&button);
                    row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
                    row.set_subtitle(&if n == 0 { gettext("Nothing new") } else { gettext("{} added").replace("{}", &plural_games(n)) });
                    if n > 0 {
                        this.summary.borrow_mut().push((name, plural_games(n)));
                    }
                    if let Some(win) = this.win.upgrade() {
                        win.app().library().refresh(&[]).await;
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

/// Lutris and the emulators' folders are imported; another launcher's GOG installs are adopted by the GOG source's scan.
async fn bring_in(via: &str, gog_dirs: Vec<String>) -> Result<usize, String> {
    let via = via.to_string();
    backend::pinned(move |core| async move {
        match via.as_str() {
            "lutris" => core.import_lutris(true).await.map(|r| r.imported.len()),
            "roms" => core.import_roms(true).await.map(|r| r.imported.len()),
            "gog" => {
                let settings = core.source_settings("gog", "").await?;
                let mut dirs: Vec<String> =
                    settings["scan_dirs"].as_str().unwrap_or_default().split(',').map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
                let before = dirs.len();
                dirs.extend(gog_dirs.into_iter().filter(|d| !dirs.contains(d)).collect::<Vec<_>>());
                if dirs.len() != before {
                    core.set_source_setting("gog", "", "scan_dirs", &dirs.join(",")).await?;
                }
                core.source_scan("gog", None).await
            }
            other => Err(universe::Error::Invalid(format!("{other} is not importable"))),
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
        let weak = Rc::downgrade(this);
        let signed_in: signin::Say = Rc::new(move |_: &str| {
            if let Some(this) = weak.upgrade() {
                this.summary.borrow_mut().push((name.clone(), gettext("Signed in")));
            }
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
        group.add(&adw::ActionRow::builder().title(what).subtitle(how).use_markup(false).build());
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
