use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};
use universe::media::{CandidatePage, Hit, MediaStatus, SlotStatus};

use crate::backend;
use crate::covers;
use crate::game::GameObject;
use crate::widgets::Cover;
use crate::window::Window;

/// The art slots a game shows, with the width over height its pictures come in.
pub fn slots() -> [(&'static str, String, f64, String); 5] {
    [
        ("box_front", gettext("Box Front"), 2.0 / 3.0, gettext("The library and the game’s page")),
        ("square", gettext("Square"), 1.0, gettext("Big Screen’s home")),
        ("banner", gettext("Banner"), 920.0 / 430.0, gettext("Cards, and backdrops without a background")),
        ("background", gettext("Background"), 16.0 / 9.0, gettext("Behind the game’s page")),
        ("logo", gettext("Logo"), 3.0, gettext("Over the background in Big Screen")),
    ]
}

pub fn slot_label(slot: &str) -> String {
    slots().into_iter().find(|(s, ..)| *s == slot).map(|(_, label, ..)| label).unwrap_or_else(|| slot.to_string())
}

fn origin_label(origin: &str) -> String {
    match origin {
        "picked" => gettext("Your pick"),
        "sgdb" => "SteamGridDB".into(),
        "steam" => "Steam".into(),
        "gamesdb" => "GOG GamesDB".into(),
        "libretro" => "libretro".into(),
        "epic" => "Epic Games Store".into(),
        "gog" => "GOG".into(),
        "itch" => "itch.io".into(),
        universe::media::GENERATED => gettext("Made from the box front"),
        "" => gettext("Fetched"),
        other => other.into(),
    }
}

fn entry_title(entry: Option<&Hit>) -> String {
    match entry {
        None => gettext("Not matched yet"),
        Some(e) if e.name.is_empty() => gettext("Entry {}").replace("{}", &e.id.to_string()),
        Some(e) if e.year > 0 => format!("{} ({})", e.name, e.year),
        Some(e) => e.name.clone(),
    }
}

fn catalogue(sgdb_key: bool) -> &'static str {
    if sgdb_key {
        "SteamGridDB"
    } else {
        "GOG GamesDB"
    }
}

/// A picture of the slot's shape `height` pixels tall.
fn sized(aspect: f64, height: i32) -> Cover {
    let cover = Cover::new((f64::from(height) * aspect).round() as i32, height);
    cover.add_css_class("thumb");
    cover.set_valign(gtk::Align::Center);
    cover
}

struct Artwork {
    nav: adw::NavigationView,
    toasts: adw::ToastOverlay,
    win: glib::WeakRef<Window>,
    game: String,
    title: String,
    source: adw::PreferencesGroup,
    entry: adw::ActionRow,
    key: adw::ActionRow,
    rows: Vec<(String, adw::ActionRow, Cover, gtk::Button)>,
    status: RefCell<MediaStatus>,
}

impl Artwork {
    fn say(&self, text: &str) {
        self.toasts.add_toast(crate::dialogs::toast(text));
    }

    fn load(self: &Rc<Self>) {
        let (this, game) = (Rc::downgrade(self), self.game.clone());
        glib::spawn_future_local(async move {
            let status = backend::pinned(move |core| async move { core.media_status(&game).await }).await;
            let Some(this) = this.upgrade() else { return };
            match status {
                Ok(mut all) if !all.is_empty() => this.show(all.remove(0)),
                Ok(_) => {}
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn show(&self, status: MediaStatus) {
        self.entry.set_title(&entry_title(status.entry.as_ref()));
        self.source.set_description(Some(&gettext("The {} game the art comes from").replace("{}", catalogue(status.sgdb_key))));
        self.key.set_visible(!status.sgdb_key);
        let named = status.entry.as_ref().map(|e| e.name.as_str()).unwrap_or("");
        let differs = !named.is_empty() && crate::library::fold(named) != crate::library::fold(&self.title);
        self.entry.set_subtitle(&if differs { gettext("Not named like the game: change it if the art is wrong") } else { String::new() });
        for (slot, row, cover, reset) in &self.rows {
            let found = status.slots.iter().find(|s| s.slot == *slot).cloned().unwrap_or(SlotStatus { kind: "missing".into(), ..SlotStatus::default() });
            let (label, use_) = slots().into_iter().find(|(s, ..)| s == slot).map(|(_, l, _, u)| (l, u)).unwrap_or_default();
            row.set_title(&label);
            let state = match found.kind.as_str() {
                "missing" => gettext("Missing"),
                _ => origin_label(&found.origin),
            };
            row.set_subtitle(&format!("{state} · {use_}"));
            covers::forget(&found.path);
            if cover.path() == found.path {
                cover.reload();
            } else {
                cover.set_path(found.path.clone());
            }
            reset.set_visible(!found.override_path.is_empty());
        }
        self.status.replace(status);
    }

    fn done(self: &Rc<Self>, text: &str) {
        self.say(text);
        self.load();
    }

    fn use_file(self: &Rc<Self>, slot: &str) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Pictures")));
        filter.add_mime_type("image/*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let chooser = gtk::FileDialog::builder().title(gettext("Choose a Picture")).filters(&filters).modal(true).build();
        let (this, slot) = (Rc::downgrade(self), slot.to_string());
        let parent = self.win.upgrade();
        glib::spawn_future_local(async move {
            let Ok(file) = chooser.open_future(parent.as_ref()).await else { return };
            let Some(path) = file.path().map(|p| p.to_string_lossy().into_owned()) else { return };
            let Some(game) = this.upgrade().map(|t| t.game.clone()) else { return };
            let set = slot.clone();
            let result = backend::pinned(move |core| async move { core.media_set_slot(&game, &set, &path).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(_) => this.done(&gettext("{} set").replace("{}", &slot_label(&slot))),
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn reset(self: &Rc<Self>, slot: &str) {
        let (this, game, slot) = (Rc::downgrade(self), self.game.clone(), slot.to_string());
        glib::spawn_future_local(async move {
            let unset = slot.clone();
            let result = backend::pinned(move |core| async move { core.media_unset(&game, &unset).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(_) => this.done(&gettext("{} is the fetched art again").replace("{}", &slot_label(&slot))),
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn fetch_missing(self: &Rc<Self>, button: &adw::ButtonRow) {
        button.set_sensitive(false);
        let (this, game, button) = (Rc::downgrade(self), self.game.clone(), button.downgrade());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.media_refresh(&game, false, None).await }).await;
            if let Some(button) = button.upgrade() {
                button.set_sensitive(true);
            }
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok((0, _)) => this.done(&gettext("Nothing new to fetch")),
                Ok((n, _)) => this.done(&ngettext("{} picture fetched", "{} pictures fetched", n as u32).replace("{}", &n.to_string())),
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    /// The provider's pictures for one slot, a page at a time; one picked replaces what the slot shows.
    fn candidates(self: &Rc<Self>, slot: &str) {
        let aspect = slots().into_iter().find(|(s, ..)| *s == slot).map(|(_, _, a, _)| a).unwrap_or(1.0);
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(12)
            .row_spacing(12)
            .homogeneous(true)
            .max_children_per_line(if aspect < 1.2 { 5 } else { 3 })
            .valign(gtk::Align::Start)
            .build();
        let more =
            gtk::Button::builder().label(gettext("_Load More")).use_underline(true).halign(gtk::Align::Center).visible(false).css_classes(["pill"]).build();
        let column =
            gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(18).margin_bottom(24).margin_start(18).margin_end(18).build();
        column.append(&flow);
        column.append(&more);
        let stack = adw::ViewStack::new();
        stack.add_named(
            &adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build(),
            Some("loading"),
        );
        let status = adw::StatusPage::builder().icon_name("image-missing-symbolic").title(gettext("No Pictures")).build();
        stack.add_named(&status, Some("status"));
        stack.add_named(&gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&column).build(), Some("list"));
        let toolbar = adw::ToolbarView::builder().content(&stack).build();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let page = adw::NavigationPage::builder().child(&toolbar).title(slot_label(slot)).build();

        let urls: Rc<RefCell<Vec<String>>> = Rc::default();
        let following: Rc<Cell<u32>> = Rc::default();
        let next: Rc<dyn Fn(u32)> = {
            let (this, slot, flow, more, stack, status, urls, following) = (
                Rc::downgrade(self),
                slot.to_string(),
                flow.downgrade(),
                more.downgrade(),
                stack.downgrade(),
                status.downgrade(),
                urls.clone(),
                following.clone(),
            );
            Rc::new(move |number: u32| {
                let (this, slot, flow, more, stack, status, urls, following) =
                    (this.clone(), slot.clone(), flow.clone(), more.clone(), stack.clone(), status.clone(), urls.clone(), following.clone());
                let Some(game) = this.upgrade().map(|t| t.game.clone()) else { return };
                if let Some(more) = more.upgrade() {
                    more.set_sensitive(false);
                }
                glib::spawn_future_local(async move {
                    let asked = slot.clone();
                    let listed = backend::pinned(move |core| async move { core.media_candidates(&game, &asked, number).await }).await;
                    let (Some(flow), Some(more), Some(stack), Some(status)) = (flow.upgrade(), more.upgrade(), stack.upgrade(), status.upgrade()) else {
                        return;
                    };
                    more.set_sensitive(true);
                    let listed: CandidatePage = match listed {
                        Ok(listed) => listed,
                        Err(e) => {
                            status.set_title(&gettext("The Pictures Could Not Be Listed"));
                            status.set_description(Some(&glib::markup_escape_text(&e.to_string())));
                            stack.set_visible_child_name("status");
                            return;
                        }
                    };
                    for candidate in &listed.items {
                        let picture = sized(aspect, if aspect < 1.2 { 150 } else { 96 });
                        picture.set_path(if candidate.thumb.is_empty() { candidate.url.clone() } else { candidate.thumb.clone() });
                        let votes = (candidate.score / 1000).max(0) as u32;
                        let tip = if candidate.provider == "sgdb" {
                            format!("SteamGridDB · {}", ngettext("{} vote", "{} votes", votes).replace("{}", &votes.to_string()))
                        } else {
                            origin_label(&candidate.provider)
                        };
                        picture.set_tooltip_text(Some(&tip));
                        flow.append(&picture);
                        urls.borrow_mut().push(candidate.url.clone());
                    }
                    more.set_visible(listed.more);
                    following.set(listed.page + 1);
                    if urls.borrow().is_empty() {
                        let none = gettext("None of this shape for the game");
                        let hint = gettext("A SteamGridDB key adds its community art");
                        status.set_description(Some(&if listed.sgdb_key { none } else { format!("{none}\n{hint}") }));
                        stack.set_visible_child_name("status");
                    } else {
                        stack.set_visible_child_name("list");
                    }
                    if let Some(this) = this.upgrade().filter(|_| number == 0) {
                        this.entry.set_title(&entry_title(listed.entry.as_ref()));
                        this.key.set_visible(!listed.sgdb_key);
                    }
                });
            })
        };
        let weak = Rc::downgrade(&next);
        more.connect_clicked(move |_| {
            weak.upgrade().inspect(|next| next(following.get()));
        });
        let (this, slot_name) = (Rc::downgrade(self), slot.to_string());
        let chosen = urls.clone();
        flow.connect_child_activated(move |_, child| {
            let Some(url) = chosen.borrow().get(child.index().max(0) as usize).cloned() else { return };
            let Some(this) = this.upgrade() else { return };
            this.apply(&slot_name, &url);
        });
        next(0);
        let held = RefCell::new(Some(next));
        page.connect_destroy(move |_| {
            held.take();
        });
        self.nav.push(&page);
    }

    fn apply(self: &Rc<Self>, slot: &str, url: &str) {
        let (this, game, slot, url) = (Rc::downgrade(self), self.game.clone(), slot.to_string(), url.to_string());
        glib::spawn_future_local(async move {
            let set = slot.clone();
            let result = backend::pinned(move |core| async move { core.media_set_url(&game, &set, &url).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(_) => {
                    this.nav.pop();
                    this.done(&gettext("{} set").replace("{}", &slot_label(&slot)));
                }
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    /// The catalogue's games by name, to point the art at the right one.
    fn search(self: &Rc<Self>) {
        let sgdb_key = self.status.borrow().sgdb_key;
        let entry = gtk::SearchEntry::builder().text(&self.title).hexpand(true).search_delay(500).build();
        let header = adw::HeaderBar::builder().title_widget(&adw::Clamp::builder().maximum_size(360).child(&entry).build()).build();
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).valign(gtk::Align::Start).build();
        let column = gtk::Box::builder().orientation(gtk::Orientation::Vertical).margin_top(18).margin_bottom(24).margin_start(12).margin_end(12).build();
        column.append(&list);
        let stack = adw::ViewStack::new();
        stack.add_named(
            &adw::Spinner::builder().width_request(32).height_request(32).halign(gtk::Align::Center).valign(gtk::Align::Center).build(),
            Some("loading"),
        );
        let status = adw::StatusPage::builder().icon_name("edit-find-symbolic").title(gettext("No Results Found")).build();
        stack.add_named(&status, Some("status"));
        stack.add_named(
            &gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).child(&adw::Clamp::builder().child(&column).build()).build(),
            Some("list"),
        );
        let toolbar = adw::ToolbarView::builder().content(&stack).build();
        toolbar.add_top_bar(&header);
        let page = adw::NavigationPage::builder().child(&toolbar).title(gettext("{} Entry").replace("{}", catalogue(sgdb_key))).build();

        let hits: Rc<RefCell<Vec<Hit>>> = Rc::default();
        let (this, list_ref, stack_ref, status_ref, found) = (Rc::downgrade(self), list.downgrade(), stack.downgrade(), status.downgrade(), hits.clone());
        let ask = move |query: String| {
            let (this, list, stack, status, found) = (this.clone(), list_ref.clone(), stack_ref.clone(), status_ref.clone(), found.clone());
            let Some(game) = this.upgrade().map(|t| t.game.clone()) else { return };
            if let Some(stack) = stack.upgrade() {
                stack.set_visible_child_name("loading");
            }
            glib::spawn_future_local(async move {
                let result = backend::pinned(move |core| async move { core.media_search(&game, &query).await }).await;
                let (Some(list), Some(stack), Some(status)) = (list.upgrade(), stack.upgrade(), status.upgrade()) else { return };
                list.remove_all();
                let result = match result {
                    Ok(result) => result,
                    Err(e) => {
                        status.set_title(&gettext("{} Could Not Be Searched").replace("{}", catalogue(sgdb_key)));
                        status.set_description(Some(&glib::markup_escape_text(&e.to_string())));
                        stack.set_visible_child_name("status");
                        return;
                    }
                };
                for hit in &result {
                    let title = if hit.year > 0 { format!("{} ({})", hit.name, hit.year) } else { hit.name.clone() };
                    let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), title, "");
                    if hit.verified {
                        row.add_suffix(
                            &gtk::Image::builder()
                                .icon_name("checkbox-checked-symbolic")
                                .tooltip_text(gettext("Verified by SteamGridDB"))
                                .css_classes(["dimmed"])
                                .build(),
                        );
                    }
                    if hit.current {
                        row.add_suffix(
                            &gtk::Image::builder()
                                .icon_name("object-select-symbolic")
                                .tooltip_text(gettext("The art comes from it now"))
                                .css_classes(["accent"])
                                .build(),
                        );
                    }
                    list.append(&row);
                }
                status.set_title(&gettext("No Results Found"));
                status.set_description(None);
                stack.set_visible_child_name(if result.is_empty() { "status" } else { "list" });
                found.replace(result);
            });
        };
        let asked = ask.clone();
        entry.connect_search_changed(move |entry| {
            let query = entry.text().trim().to_string();
            if !query.is_empty() {
                asked(query);
            }
        });
        let this = Rc::downgrade(self);
        list.connect_row_activated(move |_, row| {
            let (Some(this), Some(hit)) = (this.upgrade(), hits.borrow().get(row.index().max(0) as usize).cloned()) else { return };
            this.pin(&hit);
        });
        ask(self.title.clone());
        self.nav.push(&page);
    }

    fn pin(self: &Rc<Self>, hit: &Hit) {
        let (this, game, provider, id, name) = (Rc::downgrade(self), self.game.clone(), hit.provider.clone(), hit.id.to_string(), hit.name.clone());
        glib::spawn_future_local(async move {
            let result = backend::pinned(move |core| async move { core.media_pin(&game, &provider, &id).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(()) => {
                    this.nav.pop();
                    this.done(&gettext("The art now comes from {}").replace("{}", &name));
                }
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn ask_key(self: &Rc<Self>) {
        let Some(win) = self.win.upgrade() else { return };
        let field = adw::PasswordEntryRow::builder().title(gettext("API Key")).build();
        let list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["boxed-list"]).build();
        list.append(&field);
        let ask = adw::AlertDialog::new(
            Some(&gettext("Add a SteamGridDB Key")),
            Some(&gettext("Make one at steamgriddb.com/profile/preferences/api: its community art joins the picker, square art too.")),
        );
        ask.set_extra_child(Some(&list));
        ask.add_responses(&[("cancel", &gettext("_Cancel")), ("add", &gettext("_Add"))]);
        ask.set_response_appearance("add", adw::ResponseAppearance::Suggested);
        ask.set_default_response(Some("add"));
        let this = Rc::downgrade(self);
        ask.connect_response(Some("add"), move |_, _| {
            let key = field.text().trim().to_string();
            let Some(this) = this.upgrade().filter(|_| !key.is_empty()) else { return };
            let weak = Rc::downgrade(&this);
            glib::spawn_future_local(async move {
                let result = backend::pinned(move |core| async move { core.set_setting("keys.sgdb", &key).await }).await;
                let Some(this) = weak.upgrade() else { return };
                match result {
                    Ok(()) => this.done(&gettext("SteamGridDB key added")),
                    Err(e) => this.say(&e.to_string()),
                }
            });
        });
        ask.present(Some(&win));
    }
}

/// A game's art, slot by slot: what shows and where it came from, the other pictures the stores and catalogues have, a file of the player's.
pub fn present(win: &Window, game: &GameObject) {
    let dialog = adw::Dialog::builder().title(gettext("Artwork")).content_width(640).content_height(720).build();
    let toasts = adw::ToastOverlay::new();
    let nav = adw::NavigationView::new();
    toasts.set_child(Some(&nav));
    dialog.set_child(Some(&toasts));

    let prefs = adw::PreferencesPage::new();
    let source = adw::PreferencesGroup::builder().title(gettext("Fetched From")).build();
    let entry = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Not matched yet"), "");
    entry.add_prefix(&gtk::Image::from_icon_name("image-x-generic-symbolic"));
    let change = gtk::Button::builder().label(gettext("_Change…")).use_underline(true).valign(gtk::Align::Center).build();
    entry.add_suffix(&change);
    source.add(&entry);
    let key = crate::rows::plain(
        adw::ActionRow::builder().visible(false).build(),
        gettext("Add a SteamGridDB key for more art"),
        gettext("Your own key brings its community art and square pictures"),
    );
    key.add_prefix(&gtk::Image::from_icon_name("dialog-password-symbolic"));
    let add_key = gtk::Button::builder().label(gettext("_Add Key…")).use_underline(true).valign(gtk::Align::Center).build();
    key.add_suffix(&add_key);
    source.add(&key);
    prefs.add(&source);

    let group = adw::PreferencesGroup::builder().title(gettext("Pictures")).build();
    let mut rows = Vec::new();
    for (slot, label, aspect, _) in slots() {
        let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), label, "");
        let cover = sized(aspect, 48);
        cover.set_halign(gtk::Align::Center);
        row.add_prefix(&adw::Bin::builder().width_request(144).child(&cover).build());
        let file = gtk::Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text(gettext("Use a File…"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        let reset = gtk::Button::builder()
            .icon_name("edit-undo-symbolic")
            .tooltip_text(gettext("Back to the Fetched Picture"))
            .valign(gtk::Align::Center)
            .visible(false)
            .css_classes(["flat"])
            .build();
        row.add_suffix(&reset);
        row.add_suffix(&file);
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        group.add(&row);
        rows.push((slot.to_string(), row, cover, reset, file));
    }
    prefs.add(&group);
    let fetch = adw::ButtonRow::builder().title(gettext("Fetch Missing Art")).start_icon_name("folder-download-symbolic").build();
    let actions = adw::PreferencesGroup::new();
    actions.add(&fetch);
    prefs.add(&actions);

    let heading = adw::WindowTitle::new(&gettext("Artwork"), &game.title());
    let toolbar = adw::ToolbarView::builder().content(&prefs).build();
    toolbar.add_top_bar(&adw::HeaderBar::builder().title_widget(&heading).build());
    nav.add(&adw::NavigationPage::new(&toolbar, &gettext("Artwork")));

    let this = Rc::new(Artwork {
        nav,
        toasts,
        win: win.downgrade(),
        game: game.id(),
        title: game.title(),
        source,
        entry,
        key,
        rows: rows.iter().map(|(slot, row, cover, reset, _)| (slot.clone(), row.clone(), cover.clone(), reset.clone())).collect(),
        status: RefCell::default(),
    });
    for (slot, row, _, reset, file) in &rows {
        let (weak, s) = (Rc::downgrade(&this), slot.clone());
        row.connect_activated(move |_| {
            weak.upgrade().inspect(|t| t.candidates(&s));
        });
        let (weak, s) = (Rc::downgrade(&this), slot.clone());
        file.connect_clicked(move |_| {
            weak.upgrade().inspect(|t| t.use_file(&s));
        });
        let (weak, s) = (Rc::downgrade(&this), slot.clone());
        reset.connect_clicked(move |_| {
            weak.upgrade().inspect(|t| t.reset(&s));
        });
    }
    let weak = Rc::downgrade(&this);
    change.connect_clicked(move |_| {
        weak.upgrade().inspect(|t| t.search());
    });
    let weak = Rc::downgrade(&this);
    add_key.connect_clicked(move |_| {
        weak.upgrade().inspect(|t| t.ask_key());
    });
    let weak = Rc::downgrade(&this);
    fetch.connect_activated(move |button| {
        weak.upgrade().inspect(|t| t.fetch_missing(button));
    });
    this.load();
    let held = RefCell::new(Some(this));
    dialog.connect_closed(move |_| {
        held.take();
    });
    dialog.present(Some(win));
}
