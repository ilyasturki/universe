use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use serde_json::Value;
use universe::discover::{InstallDir, Launcher};

use crate::backend;
use crate::config;
use crate::dialogs::signin;
use crate::state::State;
use crate::window::Window;

const IMPORTERS: [&str; 2] = ["lutris", "roms"];

fn plural_games(n: usize) -> String {
    ngettext("{} game", "{} games", n as u32).replace("{}", &n.to_string())
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
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

/// Under a read-only config, what the player has to write for a launcher's games to be adopted: the source turned on,
/// or Heroic's GOG folders in its `scan_dirs`. `None` when nothing stands in the way.
fn blocked(via: &str, sources: &[Value], gog_scan_dirs: &str, gog_dirs: &[String], owner: &str) -> Option<String> {
    let source = sources.iter().find(|s| s["id"] == via).filter(|_| !IMPORTERS.contains(&via))?;
    if source["enabled"].as_bool() != Some(true) {
        return Some(
            gettext("{name} is off: add {id} to sources.enabled in {owner}.")
                .replace("{name}", &text(source, "name"))
                .replace("{id}", via)
                .replace("{owner}", owner),
        );
    }
    let current: Vec<&str> = gog_scan_dirs.split(',').map(str::trim).filter(|d| !d.is_empty()).collect();
    let missing: Vec<&str> = gog_dirs.iter().map(String::as_str).filter(|d| via == "gog" && !current.contains(d)).collect();
    (!missing.is_empty()).then(|| gettext("Add {dirs} to sources.gog.scan_dirs in {owner}.").replace("{dirs}", &missing.join(", ")).replace("{owner}", owner))
}

/// The pages after the first, as the big screen's setup has them: the stores to sign in to, then — only where config.toml
/// takes writes — the install folder and the preferences, then the end.
fn steps(stores: bool, writable: bool) -> Vec<&'static str> {
    let mut steps = Vec::new();
    if stores {
        steps.push("stores");
    }
    if writable {
        steps.extend(["install", "preferences"]);
    }
    steps.push("done");
    steps
}

/// The install folders on offer, `(folder, why)`: the one the setup found in use, the other launchers', then one picked
/// by hand; each once.
fn install_choices(first: &str, dirs: &[InstallDir], current: &str) -> Vec<(String, String)> {
    let mut choices: Vec<(String, String)> = Vec::new();
    let found = dirs.iter().map(|d| (d.dir.clone(), gettext("Where {} installs").replace("{}", &d.by)));
    for (dir, why) in
        std::iter::once((first.to_string(), gettext("Where games install now"))).chain(found).chain(std::iter::once((current.to_string(), String::new())))
    {
        if !dir.is_empty() && !choices.iter().any(|(d, _)| *d == dir) {
            choices.push((dir, why));
        }
    }
    choices
}

/// What discovery and the core said, once the first page has looked.
#[derive(Default)]
struct Look {
    gog_dirs: Vec<String>,
    install_dirs: Vec<InstallDir>,
    sources: Vec<Value>,
    config: Value,
    gpu: Value,
    families: Vec<Value>,
    first_root: String,
}

struct Onboarding {
    dialog: adw::Dialog,
    nav: adw::NavigationView,
    toasts: adw::ToastOverlay,
    win: glib::WeakRef<Window>,
    look: RefCell<Look>,
    ready: Cell<bool>,
    /// The stores the stores page lists: signed out, or off with their launcher here.
    stores: RefCell<Vec<Value>>,
    /// What the steps brought in, for the last page: `(what, how much)`.
    summary: RefCell<Vec<(String, String)>>,
    /// The launchers whose games are on their way, by name, while they are.
    adding: RefCell<Vec<String>>,
    /// Launchers whose store found nothing it could call owned while signed out: `(via, launcher)`, adopted again at its sign-in.
    waiting: RefCell<Vec<(String, String)>>,
    /// The found page's Add buttons, which Add Everything presses.
    adds: RefCell<Vec<gtk::Button>>,
    /// The importers write the library one at a time.
    importer: Rc<tokio::sync::Mutex<()>>,
    done: RefCell<Option<(gtk::Label, gtk::Label, adw::PreferencesGroup)>>,
    done_rows: RefCell<Vec<adw::ActionRow>>,
}

impl Onboarding {
    fn say(&self, text: &str) {
        self.toasts.add_toast(crate::dialogs::toast(text));
    }

    fn writable(&self) -> bool {
        self.look.borrow().config["config_writable"].as_bool() != Some(false)
    }

    fn owner(&self) -> &'static str {
        if self.look.borrow().config["config_owner"] == "home-manager" {
            "home-manager"
        } else {
            "config.toml"
        }
    }

    fn source_name(&self, id: &str) -> String {
        self.look.borrow().sources.iter().find(|s| s["id"] == id).map(|s| text(s, "name")).unwrap_or_else(|| id.to_string())
    }

    /// After an adoption that found nothing: whether `via`'s store is signed out, then listed on the stores step and
    /// `launcher` adopted again at its sign-in.
    async fn waits_for_sign_in(&self, via: &str, launcher: &str) -> bool {
        let sources = backend::pinned(|core| async move { core.sources().await }).await;
        let store = sources.iter().find(|s| s["id"] == via && signed_out(s)).cloned();
        self.look.borrow_mut().sources = sources;
        let Some(store) = store else { return false };
        if !self.stores.borrow().iter().any(|s| s["id"] == via) {
            self.stores.borrow_mut().push(store);
        }
        self.waiting.borrow_mut().push((via.to_string(), launcher.to_string()));
        true
    }

    fn next(self: &Rc<Self>, after: &str) {
        let stores = !self.stores.borrow().is_empty();
        let all = steps(stores, self.writable());
        let at = all.iter().position(|s| *s == after).map_or(0, |i| i + 1);
        let page = match all.get(at).copied().unwrap_or("done") {
            "stores" => stores_page(self),
            "install" => install_page(self),
            "preferences" => preferences_page(self),
            _ => done_page(self),
        };
        self.nav.push(&page);
    }

    /// The last page's head and summary, again as imports end.
    fn refresh_done(&self) {
        let Some((heading, body, group, rows)) = self.done.borrow().as_ref().map(|(h, b, g)| (h.clone(), b.clone(), g.clone(), self.done_rows.take())) else {
            return;
        };
        for row in rows {
            group.remove(&row);
        }
        let (adding, summary) = (self.adding.borrow().clone(), self.summary.borrow().clone());
        let mut kept = Vec::new();
        for (what, how) in adding.iter().map(|name| (name.clone(), gettext("Adding…"))).chain(summary.iter().cloned()) {
            let row = crate::rows::plain(adw::ActionRow::builder().build(), what, how);
            group.add(&row);
            kept.push(row);
        }
        self.done_rows.replace(kept);
        group.set_visible(!adding.is_empty() || !summary.is_empty());
        let (title, line) = if !adding.is_empty() {
            (gettext("Still Adding Games"), gettext("They keep arriving in the library after you close this."))
        } else if summary.is_empty() {
            (gettext("Nothing Added Yet"), gettext("Add games any time from Add Games, with Ctrl+N."))
        } else if !self.writable() && self.owner() == "home-manager" {
            (gettext("You’re All Set"), gettext("Settings come from home-manager on this machine."))
        } else if !self.writable() {
            (gettext("You’re All Set"), gettext("config.toml is read-only here, so settings stay as they are."))
        } else {
            (gettext("You’re All Set"), gettext("Everything here can be changed later in Preferences."))
        };
        heading.set_label(&title);
        body.set_label(&line);
    }
}

/// The first run with an empty library: what other launchers hold here, the stores to sign in to, where games install
/// and a few preferences, then off to play.
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
        look: RefCell::default(),
        ready: Cell::new(false),
        stores: RefCell::default(),
        summary: RefCell::default(),
        adding: RefCell::default(),
        waiting: RefCell::default(),
        adds: RefCell::default(),
        importer: Rc::default(),
        done: RefCell::default(),
        done_rows: RefCell::default(),
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

struct Step {
    page: adw::NavigationPage,
    button: gtk::Button,
    heading: gtk::Label,
    body: gtk::Label,
}

/// A page of the flow: a header bar, a title and a line over the content, the button to go on in a bar of its own at
/// the foot, in view whatever the content's length.
fn step(title: &str, icon: Option<&str>, heading: &str, body: &str, content: &gtk::Widget, next: &str, go: impl Fn() + 'static) -> Step {
    let column =
        gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(24).margin_top(12).margin_bottom(24).margin_start(12).margin_end(12).build();
    let head = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(8).build();
    if let Some(icon) = icon {
        head.append(&gtk::Image::builder().icon_name(icon).pixel_size(96).margin_bottom(12).build());
    }
    let heading = gtk::Label::builder().label(heading).wrap(true).justify(gtk::Justification::Center).css_classes(["title-1"]).build();
    let body = gtk::Label::builder().label(body).wrap(true).justify(gtk::Justification::Center).css_classes(["body", "dimmed"]).build();
    head.append(&heading);
    head.append(&body);
    column.append(&head);
    column.append(content);
    let button = gtk::Button::builder().label(next).use_underline(true).halign(gtk::Align::Center).css_classes(["pill", "suggested-action"]).build();
    button.connect_clicked(move |_| go());
    let bar = gtk::Box::builder().halign(gtk::Align::Center).margin_top(12).margin_bottom(12).build();
    bar.append(&button);
    let clamp = adw::Clamp::builder().maximum_size(480).child(&column).build();
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&clamp).build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::builder().show_title(false).build());
    toolbar.set_content(Some(&scroll));
    toolbar.add_bottom_bar(&bar);
    Step { page: adw::NavigationPage::new(&toolbar, title), button, heading, body }
}

fn found_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
    let group = adw::PreferencesGroup::builder().title(gettext("On This Machine")).build();
    let looking = adw::ActionRow::builder().title(gettext("Looking for other launchers…")).build();
    looking.add_suffix(&adw::Spinner::new());
    group.add(&looking);
    content.append(&group);
    let weak = Rc::downgrade(this);
    let step = step(
        &gettext("Welcome"),
        Some(config::APP_ID),
        &gettext("Welcome to Universe"),
        &gettext("Every game you own, from every launcher and store, in one library. Start with what is already here: adding it moves nothing."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade().filter(|t| t.ready.get()) {
                this.next("found");
            }
        },
    );
    // Whether the stores page comes next is known once the look ends: Continue waits for it.
    step.button.set_sensitive(false);
    let (weak, button) = (Rc::downgrade(this), step.button.clone());
    glib::spawn_future_local(async move {
        let report = backend::run(async { backend::core().discover().await }).await;
        let (sources, config, gpu, controller, gog) = backend::pinned(|core| async move {
            let gog = core.source_settings("gog", "").await.unwrap_or_default();
            (core.sources().await, core.settings().await, core.gpu().await, core.controller_state().await, gog)
        })
        .await;
        let Some(this) = weak.upgrade() else { return };
        group.remove(&looking);
        let writable = config["config_writable"].as_bool() != Some(false);
        let found: Vec<&Launcher> = report.launchers.iter().filter(|l| l.found && l.games > 0).collect();
        let found_via: Vec<&str> = found.iter().map(|l| l.via.as_str()).collect();
        let offered = |s: &Value| {
            writable
                && s["enabled"].as_bool() == Some(false)
                && s["available"].as_bool() != Some(false)
                && found_via.contains(&s["id"].as_str().unwrap_or_default())
        };
        this.stores.replace(sources.iter().filter(|s| signed_out(s) || offered(s)).cloned().collect());
        this.look.replace(Look {
            gog_dirs: report.gog_dirs.clone(),
            install_dirs: report.install_dirs.clone(),
            first_root: config["paths"]["games_root"].as_str().unwrap_or_default().to_string(),
            families: controller["families"].as_array().cloned().unwrap_or_default(),
            sources,
            config,
            gpu,
        });
        if found.is_empty() {
            group
                .add(&adw::ActionRow::builder().title(gettext("No other launchers found")).subtitle(gettext("Games can join any time from Add Games")).build());
        }
        let gog_scan = gog["scan_dirs"].as_str().unwrap_or_default().to_string();
        for launcher in found {
            group.add(&launcher_row(&this, launcher, &gog_scan));
        }
        if this.adds.borrow().len() > 1 {
            let all = gtk::Button::builder().label(gettext("Add _Everything")).use_underline(true).valign(gtk::Align::Center).css_classes(["flat"]).build();
            let weak = Rc::downgrade(&this);
            all.connect_clicked(move |all| {
                let Some(this) = weak.upgrade() else { return };
                all.set_visible(false);
                let adds: Vec<gtk::Button> = this.adds.borrow().iter().filter(|b| b.is_sensitive()).cloned().collect();
                for add in adds {
                    add.emit_clicked();
                }
            });
            group.set_header_suffix(Some(&all));
        }
        this.ready.set(true);
        button.set_sensitive(true);
    });
    step.page
}

fn launcher_row(this: &Rc<Onboarding>, launcher: &Launcher, gog_scan: &str) -> adw::ActionRow {
    let (why, block) = {
        let look = this.look.borrow();
        let via = Some(launcher.via.as_str()).filter(|via| !via.is_empty());
        let why = via.and_then(|via| unusable(via, &look.sources));
        let block = via.filter(|_| why.is_none() && !this.writable()).and_then(|via| blocked(via, &look.sources, gog_scan, &look.gog_dirs, this.owner()));
        (why, block)
    };
    let importable = launcher.importable && why.is_none() && block.is_none();
    let subtitle = if let Some(block) = &block {
        format!("{} · {block}", plural_games(launcher.games))
    } else if importable {
        plural_games(launcher.games)
    } else if let Some(why) = &why {
        format!("{} · {why}", plural_games(launcher.games))
    } else {
        gettext("{} · not importable yet").replace("{}", &plural_games(launcher.games))
    };
    let row = crate::rows::plain(adw::ActionRow::builder().subtitle_lines(0).build(), launcher.name.clone(), subtitle);
    if !launcher.titles.is_empty() {
        row.set_tooltip_text(Some(&launcher.titles.join(", ")));
    }
    if !importable {
        return row;
    }
    let button = gtk::Button::builder().label(gettext("Add")).valign(gtk::Align::Center).css_classes(["suggested-action"]).build();
    row.add_suffix(&button);
    this.adds.borrow_mut().push(button.clone());
    let (weak, via, name, row_ref) = (Rc::downgrade(this), launcher.via.clone(), launcher.name.clone(), row.clone());
    button.connect_clicked(move |button| {
        let Some(this) = weak.upgrade() else { return };
        button.set_sensitive(false);
        let spinner = adw::Spinner::new();
        row_ref.add_suffix(&spinner);
        let (weak, via, name, row, button) = (Rc::downgrade(&this), via.clone(), name.clone(), row_ref.clone(), button.clone());
        let dirs = this.look.borrow().gog_dirs.clone();
        let enable = this.look.borrow().sources.iter().any(|s| s["id"] == via.as_str() && s["enabled"].as_bool() != Some(true));
        let importer = this.importer.clone();
        this.adding.borrow_mut().push(name.clone());
        this.refresh_done();
        glib::spawn_future_local(async move {
            let turn = if IMPORTERS.contains(&via.as_str()) {
                row.set_subtitle(&gettext("Waiting…"));
                Some(importer.lock().await)
            } else {
                None
            };
            row.set_subtitle(&gettext("Adding…"));
            let result = bring_in(&via, dirs, enable).await;
            drop(turn);
            row.remove(&spinner);
            let Some(this) = weak.upgrade() else { return };
            this.adding.borrow_mut().retain(|n| *n != name);
            match result {
                Ok((0, _)) if !IMPORTERS.contains(&via.as_str()) && this.waits_for_sign_in(&via, &name).await => {
                    row.remove(&button);
                    row.set_subtitle(&gettext("Sign in to {} on the next page to add them").replace("{}", &this.source_name(&via)));
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
                    row.set_subtitle(&gettext("Not added"));
                    this.say(&e);
                }
            }
            this.refresh_done();
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
        if source["enabled"].as_bool() == Some(true) {
            content.append(&account(this, source));
            continue;
        }
        // Off, with its launcher here: a switch turns it on, its sign-in below once it is.
        let name = text(source, "name");
        let group = adw::PreferencesGroup::builder().title(&name).build();
        let switch = adw::SwitchRow::builder().title(gettext("Use {}").replace("{}", &name)).build();
        group.add(&switch);
        content.append(&group);
        let (weak, source, slot) = (Rc::downgrade(this), source.clone(), content.clone());
        switch.connect_active_notify(move |switch| {
            let Some(this) = weak.upgrade().filter(|_| switch.is_active() && switch.is_sensitive()) else { return };
            switch.set_sensitive(false);
            let (weak, source, slot, group, switch) = (Rc::downgrade(&this), source.clone(), slot.clone(), group.clone(), switch.clone());
            glib::spawn_future_local(async move {
                let id = text(&source, "id");
                let result = backend::pinned(move |core| async move { core.enable_source(&id, true).await }).await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(()) => slot.insert_child_after(&account(&this, &source), Some(&group)),
                    Err(e) => {
                        switch.set_active(false);
                        switch.set_sensitive(true);
                        this.say(&e.to_string());
                    }
                }
            });
        });
    }
    let weak = Rc::downgrade(this);
    step(
        &gettext("Stores"),
        None,
        &gettext("Your Stores"),
        &gettext("Sign in to see and install the games you own."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade() {
                this.next("stores");
            }
        },
    )
    .page
}

/// A store's sign-in, which adopts the launchers waiting on it once it holds.
fn account(this: &Rc<Onboarding>, source: &Value) -> adw::PreferencesGroup {
    let name = text(source, "name");
    let weak = Rc::downgrade(this);
    let say: signin::Say = Rc::new(move |text: &str| {
        if let Some(this) = weak.upgrade() {
            this.say(text);
        }
    });
    let (weak, id, store) = (Rc::downgrade(this), text(source, "id"), name.clone());
    let signed_in: signin::Say = Rc::new(move |_: &str| {
        let Some(this) = weak.upgrade() else { return };
        this.summary.borrow_mut().push((store.clone(), gettext("Signed in")));
        this.refresh_done();
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
                    this.refresh_done();
                    if let Some(win) = this.win.upgrade() {
                        win.app().library().refresh(&[]).await;
                    }
                }
                Err(e) => this.say(&e),
            }
        });
    });
    let group = signin::account_group(source, say, signed_in);
    group.set_title(&name);
    group
}

fn install_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
    let group = adw::PreferencesGroup::builder().title(gettext("Install Folder")).build();
    content.append(&group);
    let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::default();
    fill_install(this, &group, &rows);
    let weak = Rc::downgrade(this);
    step(
        &gettext("Install Folder"),
        None,
        &gettext("Where Games Install"),
        &gettext("New installs go here. Games already installed stay where they are."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade() {
                this.next("install");
            }
        },
    )
    .page
}

/// The folder in use with a check, the others with a button that makes it the one; "Another Folder" picks any.
fn fill_install(this: &Rc<Onboarding>, group: &adw::PreferencesGroup, rows: &Rc<RefCell<Vec<gtk::Widget>>>) {
    for row in rows.borrow_mut().drain(..) {
        group.remove(&row);
    }
    let (current, choices) = {
        let look = this.look.borrow();
        let current = look.config["paths"]["games_root"].as_str().unwrap_or_default().to_string();
        let choices = install_choices(&look.first_root, &look.install_dirs, &current);
        (current, choices)
    };
    let home = universe::paths::home().to_string_lossy().to_string();
    for (dir, why) in choices {
        let label = dir.strip_prefix(&home).map_or_else(|| dir.clone(), |rest| format!("~{rest}"));
        let row = crate::rows::plain(adw::ActionRow::builder().build(), label, why);
        if dir == current {
            row.add_suffix(&gtk::Image::from_icon_name("object-select-symbolic"));
        } else {
            let button = gtk::Button::builder().label(gettext("Use")).valign(gtk::Align::Center).build();
            let (weak, group, rows) = (Rc::downgrade(this), group.clone(), rows.clone());
            button.connect_clicked(move |_| {
                if let Some(this) = weak.upgrade() {
                    set_games_root(&this, dir.clone(), &group, &rows);
                }
            });
            row.add_suffix(&button);
        }
        group.add(&row);
        rows.borrow_mut().push(row.upcast());
    }
    let other = adw::ButtonRow::builder().title(gettext("Another Folder…")).start_icon_name("folder-open-symbolic").build();
    let (weak, group_ref, rows_ref) = (Rc::downgrade(this), group.clone(), rows.clone());
    other.connect_activated(move |row| {
        let dialog = gtk::FileDialog::builder().title(gettext("Where Games Install")).modal(true).build();
        let (weak, group, rows) = (weak.clone(), group_ref.clone(), rows_ref.clone());
        dialog.select_folder(row.root().and_downcast::<gtk::Window>().as_ref(), gio::Cancellable::NONE, move |result| {
            let (Some(this), Ok(file)) = (weak.upgrade(), result) else { return };
            if let Some(path) = file.path() {
                set_games_root(&this, path.to_string_lossy().into(), &group, &rows);
            }
        });
    });
    group.add(&other);
    rows.borrow_mut().push(other.upcast());
}

fn set_games_root(this: &Rc<Onboarding>, dir: String, group: &adw::PreferencesGroup, rows: &Rc<RefCell<Vec<gtk::Widget>>>) {
    let (weak, group, rows) = (Rc::downgrade(this), group.clone(), rows.clone());
    glib::spawn_future_local(async move {
        let result = backend::pinned(move |core| async move {
            core.set_setting("paths.games_root", &dir).await?;
            Ok::<_, universe::Error>(core.settings().await)
        })
        .await;
        let Some(this) = weak.upgrade() else { return };
        match result {
            Ok(config) => {
                this.look.borrow_mut().config = config;
                fill_install(&this, &group, &rows);
            }
            Err(e) => this.say(&e.to_string()),
        }
    });
}

fn preferences_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).build();
    let (families, hdr, fits) = {
        let look = this.look.borrow();
        (look.families.clone(), look.config["launch"]["hdr"].as_bool() == Some(true), look.gpu["fits"]["hdr"].as_bool() != Some(false))
    };
    if !families.is_empty() {
        let group = adw::PreferencesGroup::builder().title(gettext("Controller")).build();
        let names: Vec<String> = families.iter().map(|f| text(f, "name")).collect();
        let row = adw::ComboRow::builder()
            .title(gettext("Buttons and Glyphs"))
            .subtitle(gettext("The pad the button hints follow until one is plugged in"))
            .model(&gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>()))
            .build();
        let remembered = State::load().controller_family;
        if let Some(index) = families.iter().position(|f| text(f, "id") == remembered) {
            row.set_selected(index as u32);
        }
        let scripted = this.win.upgrade().is_some_and(|win| win.app().scripted());
        row.connect_selected_notify(move |row| {
            let Some(family) = families.get(row.selected() as usize).map(|f| text(f, "id")) else { return };
            if !scripted {
                let mut state = State::load();
                state.controller_family = family;
                state.save();
            }
        });
        group.add(&row);
        content.append(&group);
    }
    if fits {
        let group = adw::PreferencesGroup::builder().title(gettext("Graphics")).build();
        let row = adw::SwitchRow::builder().title(gettext("HDR")).subtitle(gettext("Games that support HDR send it to an HDR screen")).active(hdr).build();
        let weak = Rc::downgrade(this);
        row.connect_active_notify(move |row| {
            let (weak, on) = (weak.clone(), row.is_active());
            glib::spawn_future_local(async move {
                let result = backend::pinned(move |core| async move { core.set_setting("launch.hdr", if on { "true" } else { "false" }).await }).await;
                if let (Some(this), Err(e)) = (weak.upgrade(), result) {
                    this.say(&e.to_string());
                }
            });
        });
        group.add(&row);
        content.append(&group);
    }
    let weak = Rc::downgrade(this);
    step(
        &gettext("Preferences"),
        None,
        &gettext("Controller and Screen"),
        &gettext("Both can be changed later in Preferences."),
        content.upcast_ref(),
        &gettext("_Continue"),
        move || {
            if let Some(this) = weak.upgrade() {
                this.next("preferences");
            }
        },
    )
    .page
}

fn done_page(this: &Rc<Onboarding>) -> adw::NavigationPage {
    let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(12).build();
    content.append(&gtk::Image::builder().icon_name("object-select-symbolic").pixel_size(72).css_classes(["success"]).build());
    let group = adw::PreferencesGroup::new();
    content.append(&group);
    let weak = Rc::downgrade(this);
    let step = step(&gettext("Done"), None, "", "", content.upcast_ref(), &gettext("_Start Playing"), move || {
        if let Some(this) = weak.upgrade() {
            this.dialog.close();
        }
    });
    this.done_rows.take();
    this.done.replace(Some((step.heading.clone(), step.body.clone(), group)));
    this.refresh_done();
    step.page
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

    #[test]
    fn a_read_only_config_names_what_to_write_before_anything_is_tried() {
        let sources =
            [serde_json::json!({"id": "gog", "name": "GOG", "enabled": true}), serde_json::json!({"id": "epic", "name": "Epic Games", "enabled": false})];
        let heroic = ["/mnt/games/PC".to_string()];
        assert_eq!(blocked("lutris", &sources, "", &heroic, "home-manager"), None, "an importer writes no setting");
        assert!(blocked("epic", &sources, "", &heroic, "home-manager").is_some_and(|b| b.contains("sources.enabled") && b.contains("home-manager")));
        assert!(blocked("gog", &sources, "", &heroic, "config.toml").is_some_and(|b| b.contains("/mnt/games/PC") && b.contains("config.toml")));
        assert_eq!(blocked("gog", &sources, "/mnt/games/PC", &heroic, "config.toml"), None, "the folder is there already");
    }

    #[test]
    fn the_install_folder_and_preferences_need_a_config_that_takes_writes() {
        assert_eq!(steps(true, true), ["stores", "install", "preferences", "done"]);
        assert_eq!(steps(false, false), ["done"]);
    }

    #[test]
    fn the_install_folders_list_each_once_the_one_in_use_first() {
        let dirs = [InstallDir { dir: "/h".into(), by: "Heroic".into() }, InstallDir { dir: "/g".into(), by: "Lutris".into() }];
        let folders: Vec<String> = install_choices("/g", &dirs, "/elsewhere").into_iter().map(|(dir, _)| dir).collect();
        assert_eq!(folders, ["/g", "/h", "/elsewhere"]);
    }
}
