use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;
use serde_json::{json, Value};

use crate::backend;
use crate::state::State;
use crate::watcher::Watcher;
use crate::widgets::PadArt;

/// Big Screen's HOME: its macros are not the player's to set.
const HOME_SLOT: &str = "guide";
const WALK_STEP_S: u32 = 8;
const WALK_ORDER: [&str; 17] =
    ["south", "east", "west", "north", "lb", "rb", "lt", "rt", "select", "start", "guide", "ls", "rs", "dpad_up", "dpad_down", "dpad_left", "dpad_right"];
const RESTART_MS: u64 = 2000;
const RESTART_MAX_MS: u64 = 30_000;
const HOLDS: [u64; 4] = [400, 600, 800, 1000];
const STEPS: [u64; 4] = [1, 2, 5, 10];

fn text(v: &Value, key: &str) -> String {
    match &v[key] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

struct Step {
    slot: String,
    axis: String,
    art: String,
    label: String,
    prompt: String,
}

enum Record {
    Missed(String),
    Found { slot: String, axis: String, code: String, from: String, stolen: String },
}

struct Walk {
    id: String,
    family: String,
    name: String,
    steps: Vec<Step>,
    index: usize,
    found: Vec<String>,
    missed: Vec<String>,
    axes: Vec<String>,
    history: Vec<Record>,
    seconds: u32,
    prompt: gtk::Label,
    count: gtk::Label,
    back: gtk::Button,
    timer: Option<glib::SourceId>,
}

/// A button's page: learning its code, its press and hold macros.
struct Editor {
    slot: String,
    learn: adw::ActionRow,
    learn_button: gtk::Button,
    rows: [(String, adw::ComboRow, adw::EntryRow, adw::EntryRow); 2],
}

pub struct Controller {
    pub page: adw::PreferencesPage,
    dialog: glib::WeakRef<adw::PreferencesDialog>,
    arts: RefCell<Vec<glib::WeakRef<PadArt>>>,
    status: gtk::Label,
    device_row: adw::ComboRow,
    family_row: adw::ComboRow,
    test: adw::SwitchRow,
    walk_button: gtk::Button,
    buttons: adw::PreferencesGroup,
    rows: RefCell<Vec<adw::ActionRow>>,
    hold: adw::ComboRow,
    step: adw::ComboRow,
    home: adw::SwitchRow,
    /// The window this page shows in, watched while the page is open: losing the focus stops what reads the pad.
    active_watch: RefCell<Option<(gtk::Window, glib::SignalHandlerId)>>,
    watcher: RefCell<Option<Watcher>>,
    state: RefCell<Value>,
    devices: RefCell<Vec<Value>>,
    current: RefCell<String>,
    status_kind: RefCell<String>,
    passive: Cell<bool>,
    learning: RefCell<String>,
    walk: RefCell<Option<Walk>>,
    editor: RefCell<Option<Editor>>,
    restart_ms: Cell<u64>,
    open: Cell<bool>,
    syncing: Cell<bool>,
    remembered: RefCell<String>,
}

impl Controller {
    fn say(&self, line: &str) {
        if let Some(dialog) = self.dialog.upgrade() {
            dialog.add_toast(crate::dialogs::toast(line));
        }
    }

    fn families(&self) -> Vec<Value> {
        self.state.borrow()["families"].as_array().cloned().unwrap_or_default()
    }

    fn device(&self) -> Option<Value> {
        let current = self.current.borrow().clone();
        self.devices.borrow().iter().find(|d| text(d, "id") == current).cloned()
    }

    /// The family whose buttons show: the picked pad's, else the one remembered.
    fn family(&self) -> String {
        self.device().map(|d| text(&d, "family")).unwrap_or_else(|| self.remembered.borrow().clone())
    }

    fn family_spec(&self) -> Option<Value> {
        let family = self.family();
        self.families().into_iter().find(|f| text(f, "id") == family)
    }

    fn live(&self) -> bool {
        self.device().is_some() && !self.passive.get() && *self.status_kind.borrow() == "ready"
    }

    fn send(&self, command: Value) {
        if let Some(watcher) = self.watcher.borrow().as_ref() {
            watcher.send(command);
        }
    }

    fn art(&self) -> PadArt {
        let art = PadArt::default();
        art.set_family(&self.family());
        self.arts.borrow_mut().push(art.downgrade());
        art
    }

    fn each_art(&self, f: impl Fn(&PadArt)) {
        self.arts.borrow_mut().retain(|a| a.upgrade().is_some());
        for art in self.arts.borrow().iter().filter_map(|a| a.upgrade()) {
            f(&art);
        }
    }

    fn preset_label(&self, id: &str) -> String {
        self.state.borrow()["presets"]
            .as_array()
            .and_then(|p| p.iter().find(|p| text(p, "id") == id).map(|p| text(p, "label")))
            .unwrap_or_else(|| id.to_string())
    }

    fn macro_of(&self, slot: &str, trigger: &str) -> Option<Value> {
        let family = self.family();
        let state = self.state.borrow();
        state["macros"]
            .as_array()?
            .iter()
            .find(|m| text(m, "button") == slot && text(m, "trigger") == trigger && (text(m, "family") == family || text(m, "family") == "*"))
            .cloned()
    }

    fn macro_label(&self, m: &Value) -> String {
        match text(m, "action").as_str() {
            "keys" => text(m, "keys"),
            "command" => gettext("A command"),
            action => self.preset_label(action),
        }
    }

    fn bound(&self, slot: &str) -> bool {
        match self.device() {
            Some(device) => device["slots"][slot]["bound"].as_bool() == Some(true),
            None => self
                .family_spec()
                .and_then(|f| f["slots"].as_array().cloned())
                .unwrap_or_default()
                .iter()
                .any(|s| text(s, "id") == slot && s["codes"].as_array().is_some_and(|c| !c.is_empty())),
        }
    }

    fn display(&self, slot: &str) -> String {
        if slot == HOME_SLOT {
            return gettext("Big Screen’s home button");
        }
        if !self.bound(slot) {
            return gettext("Unbound");
        }
        let parts: Vec<String> = [("press", gettext("Press")), ("hold", gettext("Hold"))]
            .into_iter()
            .filter_map(|(trigger, word)| self.macro_of(slot, trigger).map(|m| format!("{word} · {}", self.macro_label(&m))))
            .collect();
        if parts.is_empty() {
            gettext("No macro")
        } else {
            parts.join(" / ")
        }
    }

    fn slots(&self) -> Vec<Value> {
        let slots = self.family_spec().and_then(|f| f["slots"].as_array().cloned()).unwrap_or_default();
        let (extra, standard): (Vec<Value>, Vec<Value>) = slots.into_iter().partition(|s| s["extra"].as_bool() == Some(true));
        extra.into_iter().chain(standard).collect()
    }

    fn unbound(&self) -> Vec<String> {
        self.slots().iter().map(|s| text(s, "id")).filter(|slot| !self.bound(slot)).collect()
    }

    fn rebuild(self: &Rc<Self>) {
        for row in self.rows.take() {
            self.buttons.remove(&row);
        }
        let family = self.family();
        self.each_art(|art| art.set_family(&family));
        let unbound = self.unbound();
        self.each_art(|art| art.set_unbound(unbound.clone()));
        let mut rows = Vec::new();
        for slot in self.slots() {
            let id = text(&slot, "id");
            let row = crate::rows::plain(adw::ActionRow::builder().activatable(true).build(), text(&slot, "label"), self.display(&id));
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let (this, label) = (Rc::downgrade(self), text(&slot, "label"));
            row.connect_activated(move |_| {
                this.upgrade().inspect(|t| t.edit(&id, &label));
            });
            self.buttons.add(&row);
            rows.push(row);
        }
        self.buttons.set_title(&self.family_spec().map(|f| text(&f, "name")).unwrap_or_default());
        self.rows.replace(rows);
        self.sync();
        self.refresh_editor();
    }

    /// The status line and what can be done now.
    fn sync(&self) {
        let device = self.device();
        let line = match (self.status_kind.borrow().as_str(), &device) {
            _ if self.passive.get() => gettext("Big Screen is reading the controllers: the test, learning and setup wait until it closes"),
            ("off", _) => gettext("The controller watcher stopped: starting it again"),
            ("missing", _) => gettext("The universe command is not installed: live presses cannot show here"),
            (_, Some(device)) => {
                let mut parts = vec![text(device, "name")];
                let bus = text(device, "bus");
                if !bus.is_empty() {
                    parts.push(match bus.as_str() {
                        "bluetooth" => "Bluetooth".into(),
                        "usb" => "USB".into(),
                        other => other.to_string(),
                    });
                }
                if let Some(percent) = device["battery"]["percent"].as_u64() {
                    parts.push(format!("{percent} %"));
                }
                parts.join(" · ")
            }
            ("ready", None) => gettext("No controller connected: its buttons can still be set up below"),
            _ => gettext("Looking for controllers…"),
        };
        self.status.set_label(&line);
        let live = self.live();
        self.test.set_sensitive(live);
        if !live && self.test.is_active() {
            self.test.set_active(false);
        }
        self.walk_button.set_sensitive(live && self.walk.borrow().is_none());
        let devices = self.devices.borrow().clone();
        self.syncing.set(true);
        self.device_row.set_visible(devices.len() > 1);
        let names: Vec<String> = devices.iter().map(|d| text(d, "name")).collect();
        self.device_row.set_model(Some(&gtk::StringList::new(&names.iter().map(String::as_str).collect::<Vec<_>>())));
        let current = self.current.borrow().clone();
        if let Some(index) = devices.iter().position(|d| text(d, "id") == current) {
            self.device_row.set_selected(index as u32);
        }
        let families = self.families();
        self.family_row.set_visible(devices.is_empty() && !families.is_empty());
        let family_names: Vec<String> = families.iter().map(|f| text(f, "name")).collect();
        self.family_row.set_model(Some(&gtk::StringList::new(&family_names.iter().map(String::as_str).collect::<Vec<_>>())));
        let remembered = self.remembered.borrow().clone();
        if let Some(index) = families.iter().position(|f| text(f, "id") == remembered) {
            self.family_row.set_selected(index as u32);
        }
        let state = self.state.borrow().clone();
        if let Some(index) = HOLDS.iter().position(|h| state["hold_ms"].as_u64() == Some(*h)) {
            self.hold.set_selected(index as u32);
        }
        if let Some(index) = STEPS.iter().position(|s| state["volume_step"].as_f64().map(|v| v.round() as u64) == Some(*s)) {
            self.step.set_selected(index as u32);
        }
        self.home.set_active(state["home_summons"].as_bool() != Some(false));
        self.syncing.set(false);
    }

    fn remember(&self, family: &str) {
        if family.is_empty() || *self.remembered.borrow() == family {
            return;
        }
        self.remembered.replace(family.to_string());
        let scripted = gtk::gio::Application::default().and_downcast::<crate::app::Application>().is_some_and(|app| app.scripted());
        if !scripted {
            let mut state = State::load();
            state.controller_family = family.to_string();
            state.save();
        }
    }

    /// `controller_state` again, `config.toml` reread first when the watcher wrote it from its own process.
    fn reload(self: &Rc<Self>, reread: bool) {
        let this = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let state = backend::run(async move {
                let core = backend::core();
                if reread {
                    if let Err(e) = core.reload_settings().await {
                        tracing::warn!("controller: {e}");
                    }
                }
                core.controller_state().await
            })
            .await;
            let Some(this) = this.upgrade() else { return };
            this.state.replace(state);
            this.rebuild();
        });
    }

    /// Something was written to `config.toml` from here: this state and the watcher's both follow it.
    fn changed(self: &Rc<Self>) {
        self.send(json!({"cmd": "reload"}));
        self.reload(false);
    }

    fn start(self: &Rc<Self>) {
        self.open.set(true);
        if self.active_watch.borrow().is_none() {
            if let Some(window) = self.page.root().and_downcast::<gtk::Window>() {
                let this = Rc::downgrade(self);
                let id = window.connect_is_active_notify(move |window| {
                    if !window.is_active() {
                        this.upgrade().inspect(|t| t.focus_lost());
                    }
                });
                self.active_watch.replace(Some((window, id)));
            }
        }
        if self.watcher.borrow().is_some() {
            return;
        }
        let this = Rc::downgrade(self);
        let watcher = Watcher::start(move |event| {
            if let Some(this) = this.upgrade() {
                this.on_event(&event);
            }
        });
        if watcher.is_none() {
            self.status_kind.replace("missing".into());
        }
        self.watcher.replace(watcher);
        self.sync();
    }

    fn stop(&self) {
        self.open.set(false);
        if let Some((window, id)) = self.active_watch.take() {
            window.disconnect(id);
        }
        self.stop_walk();
        self.stop_learning();
        self.test.set_active(false);
        self.watcher.take();
        self.status_kind.replace(String::new());
        self.devices.borrow_mut().clear();
        self.current.replace(String::new());
        self.each_art(|art| art.clear_input());
    }

    fn on_event(self: &Rc<Self>, event: &Value) {
        let id = text(event, "id");
        match text(event, "event").as_str() {
            "device" => {
                let mut devices = self.devices.borrow_mut();
                match devices.iter_mut().find(|d| text(d, "id") == id) {
                    Some(known) => *known = event.clone(),
                    None => devices.push(event.clone()),
                }
                drop(devices);
                if self.device().is_none() {
                    self.current.replace(id);
                }
                self.remember(&self.family());
                self.rebuild();
            }
            "gone" => {
                self.devices.borrow_mut().retain(|d| text(d, "id") != id);
                if *self.current.borrow() == id {
                    let next = self.devices.borrow().first().map(|d| text(d, "id")).unwrap_or_default();
                    self.current.replace(next);
                    if self.walk.borrow().as_ref().is_some_and(|w| w.id == id) {
                        self.stop_walk();
                        self.say(&gettext("The controller is gone: setup stopped"));
                    }
                }
                self.rebuild();
            }
            "battery" => {
                if let Some(device) = self.devices.borrow_mut().iter_mut().find(|d| text(d, "id") == id) {
                    device["battery"] = json!({"percent": event["percent"], "charging": event["charging"]});
                }
                self.sync();
            }
            "button" if id == *self.current.borrow() => {
                let (slot, down) = (text(event, "slot"), event["pressed"].as_bool() == Some(true));
                self.each_art(|art| art.set_pressed(&slot, down));
            }
            "axis" if id == *self.current.borrow() => {
                let (axis, value) = (text(event, "axis"), event["value"].as_f64().unwrap_or(0.0) as f32);
                self.each_art(|art| art.set_axis(&axis, value));
            }
            "learned" => self.learned(event),
            "learn_timeout" => {
                if self.walk.borrow().is_some() {
                    self.walk_skip();
                } else if self.stop_learning() {
                    self.say(&gettext("No button was pressed: learning stopped"));
                }
            }
            "error" => {
                self.stop_learning();
                self.stop_walk();
                self.say(&text(event, "message"));
            }
            kind @ ("waiting" | "ready" | "off") => {
                self.status_kind.replace(kind.to_string());
                match kind {
                    "waiting" => {
                        self.passive.set(true);
                        self.test.set_active(false);
                        let this = Rc::downgrade(self);
                        glib::spawn_future_local(async move {
                            let pads = backend::run(async { backend::core().controller_pads().await }).await;
                            let Some(this) = this.upgrade() else { return };
                            this.devices.replace(pads);
                            let first = this.devices.borrow().first().map(|d| text(d, "id")).unwrap_or_default();
                            this.current.replace(first);
                            this.rebuild();
                        });
                    }
                    "ready" => {
                        self.restart_ms.set(RESTART_MS);
                        if self.passive.replace(false) {
                            self.devices.borrow_mut().clear();
                            self.current.replace(String::new());
                        }
                    }
                    _ => {
                        self.passive.set(false);
                        self.stop_learning();
                        self.stop_walk();
                        self.devices.borrow_mut().clear();
                        self.current.replace(String::new());
                        self.watcher.take();
                        if self.open.get() {
                            let delay = self.restart_ms.get();
                            self.restart_ms.set((delay * 2).min(RESTART_MAX_MS));
                            let this = Rc::downgrade(self);
                            glib::timeout_add_local_once(Duration::from_millis(delay), move || {
                                if let Some(this) = this.upgrade().filter(|t| t.open.get()) {
                                    this.start();
                                }
                            });
                        }
                    }
                }
                self.rebuild();
            }
            _ => {}
        }
    }

    /// Another window has the focus: a press made there is not for the test, a learn or the walk.
    fn focus_lost(&self) {
        if self.walk.borrow().is_some() || !self.learning.borrow().is_empty() {
            self.send(json!({"cmd": "cancel"}));
        }
        self.stop_walk();
        self.stop_learning();
        self.test.set_active(false);
        self.each_art(|art| art.clear_input());
    }

    fn stop_learning(&self) -> bool {
        if self.learning.borrow().is_empty() {
            return false;
        }
        self.learning.replace(String::new());
        self.each_art(|art| art.set_learning(""));
        self.refresh_editor();
        true
    }

    fn learn(self: &Rc<Self>, slot: &str) {
        let Some(device) = self.device().filter(|_| self.live()) else { return };
        if !self.learning.borrow().is_empty() {
            self.send(json!({"cmd": "cancel"}));
            self.stop_learning();
            return;
        }
        self.learning.replace(slot.to_string());
        self.each_art(|art| art.set_learning(slot));
        self.send(json!({"cmd": "learn", "id": text(&device, "id"), "slot": slot}));
        self.refresh_editor();
    }

    fn learned(self: &Rc<Self>, event: &Value) {
        let (family, slot, axis, code) = (text(event, "family"), text(event, "slot"), text(event, "axis"), text(event, "code"));
        let from = text(event, "from");
        for device in self.devices.borrow_mut().iter_mut().filter(|d| text(d, "family") == family) {
            if !axis.is_empty() {
                device["axes"][&axis] = json!(code);
                continue;
            }
            device["slots"][&slot] = json!({"code": code, "bound": true});
            if !from.is_empty() && from != slot {
                device["slots"][&from] = json!({"code": null, "bound": false});
            }
        }
        self.stop_learning();
        self.reload(true);
        if self.walk.borrow().is_some() {
            self.walk_learned(&slot, &axis, &from, &code);
        } else if !slot.is_empty() {
            self.say(&gettext("Learned: {}").replace("{}", &code));
        }
    }

    fn set_macro(self: &Rc<Self>, slot: &str, trigger: &str, action: &str, keys: &str, command: &str) {
        let (this, family, slot, trigger) = (Rc::downgrade(self), self.family(), slot.to_string(), trigger.to_string());
        let (action, keys, command) = (action.to_string(), keys.to_string(), command.to_string());
        glib::spawn_future_local(async move {
            let result = if action.is_empty() {
                let (f, s, t) = (family.clone(), slot.clone(), trigger.clone());
                backend::call(move |core| async move { core.remove_controller_macro(&f, &s, &t).await }).await.or_else(|e| match e {
                    universe::Error::NotFound(_) => Ok(()),
                    e => Err(e),
                })
            } else {
                let m = universe::controller::Macro { family, button: slot, trigger, action, keys, command };
                backend::call(move |core| async move { core.set_controller_macro(m).await }).await
            };
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(()) => this.changed(),
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn restore_codes(self: &Rc<Self>, slot: &str) {
        let (this, family, slot) = (Rc::downgrade(self), self.family(), slot.to_string());
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.set_controller_button(&family, &slot, None).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(()) => {
                    this.say(&gettext("The button’s default codes are back"));
                    this.changed();
                }
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    fn set_config(self: &Rc<Self>, key: &str, value: String) {
        let (this, key) = (Rc::downgrade(self), key.to_string());
        glib::spawn_future_local(async move {
            let result = backend::call(move |core| async move { core.set_setting(&key, &value).await }).await;
            let Some(this) = this.upgrade() else { return };
            match result {
                Ok(()) => this.changed(),
                Err(e) => this.say(&e.to_string()),
            }
        });
    }

    /// The choices a macro trigger offers: none, the presets that fire on it, typed keys, a command.
    fn choices(&self, trigger: &str) -> Vec<(String, String)> {
        let mut out = vec![(String::new(), gettext("Nothing"))];
        for preset in self.state.borrow()["presets"].as_array().cloned().unwrap_or_default() {
            if trigger == "hold" || preset["hold_only"].as_bool() != Some(true) {
                out.push((text(&preset, "id"), text(&preset, "label")));
            }
        }
        out.push(("keys".into(), gettext("Type Keys")));
        out.push(("command".into(), gettext("Run a Command")));
        out
    }

    fn edit(self: &Rc<Self>, slot: &str, label: &str) {
        let Some(dialog) = self.dialog.upgrade() else { return };
        let prefs = adw::PreferencesPage::new();
        let art_group = adw::PreferencesGroup::new();
        let art = self.art();
        art.set_unbound(self.unbound());
        art_group.add(&art);
        prefs.add(&art_group);

        let button = adw::PreferencesGroup::builder().title(gettext("Button")).build();
        let learn = crate::rows::plain(adw::ActionRow::builder().build(), gettext("Learn the Button"), "");
        let learn_button = gtk::Button::builder().valign(gtk::Align::Center).build();
        learn.add_suffix(&learn_button);
        button.add(&learn);
        let restore = adw::ButtonRow::builder().title(gettext("Restore the Default Codes")).build();
        button.add(&restore);
        prefs.add(&button);

        let macros = adw::PreferencesGroup::builder().title(gettext("Macros")).build();
        let make = |trigger: &str, title: &str| {
            let combo = adw::ComboRow::builder().title(title).build();
            let keys = adw::EntryRow::builder().title(gettext("Keys, as Super_L+F12")).show_apply_button(true).visible(false).build();
            let command = adw::EntryRow::builder().title(gettext("Command")).show_apply_button(true).visible(false).build();
            macros.add(&combo);
            macros.add(&keys);
            macros.add(&command);
            (trigger.to_string(), combo, keys, command)
        };
        let rows = [make("press", &gettext("Press")), make("hold", &gettext("Hold"))];
        if slot == HOME_SLOT {
            macros.set_description(Some(&gettext("Big Screen keeps this button: a press opens its menu, a hold goes home.")));
            macros.set_sensitive(false);
        }
        prefs.add(&macros);

        let toolbar = adw::ToolbarView::builder().content(&prefs).build();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let page = adw::NavigationPage::new(&toolbar, label);

        let (this, s) = (Rc::downgrade(self), slot.to_string());
        learn_button.connect_clicked(move |_| {
            this.upgrade().inspect(|t| t.learn(&s));
        });
        let (this, s) = (Rc::downgrade(self), slot.to_string());
        restore.connect_activated(move |_| {
            this.upgrade().inspect(|t| t.restore_codes(&s));
        });
        for (trigger, combo, keys, command) in &rows {
            let (this, s, t, keys_row, command_row) = (Rc::downgrade(self), slot.to_string(), trigger.clone(), keys.clone(), command.clone());
            combo.connect_selected_notify(move |combo| {
                let Some(this) = this.upgrade().filter(|t| !t.syncing.get()) else { return };
                let Some((action, _)) = this.choices(&t).get(combo.selected() as usize).cloned() else { return };
                keys_row.set_visible(action == "keys");
                command_row.set_visible(action == "command");
                if action != "keys" && action != "command" {
                    this.set_macro(&s, &t, &action, "", "");
                }
            });
            let (this, s, t) = (Rc::downgrade(self), slot.to_string(), trigger.clone());
            keys.connect_apply(move |row| {
                let value = row.text().trim().to_string();
                if let (Some(this), false) = (this.upgrade(), value.is_empty()) {
                    this.set_macro(&s, &t, "keys", &value, "");
                }
            });
            let (this, s, t) = (Rc::downgrade(self), slot.to_string(), trigger.clone());
            command.connect_apply(move |row| {
                let value = row.text().trim().to_string();
                if let (Some(this), false) = (this.upgrade(), value.is_empty()) {
                    this.set_macro(&s, &t, "command", "", &value);
                }
            });
        }
        self.editor.replace(Some(Editor { slot: slot.to_string(), learn, learn_button, rows }));
        self.refresh_editor();
        let this = Rc::downgrade(self);
        page.connect_hidden(move |_| {
            let Some(this) = this.upgrade() else { return };
            if this.learning.borrow().is_empty() {
                return;
            }
            this.send(json!({"cmd": "cancel"}));
            this.stop_learning();
        });
        dialog.push_subpage(&page);
    }

    fn refresh_editor(&self) {
        let editor = self.editor.borrow();
        let Some(editor) = editor.as_ref() else { return };
        let learning = *self.learning.borrow() == editor.slot;
        let code = self.device().map(|d| text(&d["slots"][&editor.slot], "code")).unwrap_or_default();
        editor.learn.set_subtitle(&match (learning, code.is_empty()) {
            (true, _) => gettext("Press it on the controller…"),
            (false, true) if self.bound(&editor.slot) => gettext("Press the button to see its code"),
            (false, true) => gettext("Unbound"),
            (false, false) => code,
        });
        editor.learn_button.set_label(&if learning { gettext("Cancel") } else { gettext("Learn") });
        editor.learn_button.set_sensitive(self.live());
        self.syncing.set(true);
        for (trigger, combo, keys, command) in &editor.rows {
            let choices = self.choices(trigger);
            let labels: Vec<&str> = choices.iter().map(|(_, l)| l.as_str()).collect();
            combo.set_model(Some(&gtk::StringList::new(&labels)));
            let current = self.macro_of(&editor.slot, trigger);
            let action = current.as_ref().map(|m| text(m, "action")).unwrap_or_default();
            combo.set_selected(choices.iter().position(|(id, _)| *id == action).unwrap_or(0) as u32);
            keys.set_visible(action == "keys");
            command.set_visible(action == "command");
            keys.set_text(&current.as_ref().map(|m| text(m, "keys")).unwrap_or_default());
            command.set_text(&current.as_ref().map(|m| text(m, "command")).unwrap_or_default());
        }
        self.syncing.set(false);
    }

    /// Each button pressed in turn, then the sticks thrown: whatever the pad sends becomes the button asked for.
    fn start_walk(self: &Rc<Self>) {
        let (Some(device), Some(family), Some(dialog)) = (self.device().filter(|_| self.live()), self.family_spec(), self.dialog.upgrade()) else { return };
        self.test.set_active(false);
        let specs = family["slots"].as_array().cloned().unwrap_or_default();
        let label = |slot: &str| specs.iter().find(|s| text(s, "id") == slot).map(|s| text(s, "label")).unwrap_or_else(|| slot.to_string());
        let mut order: Vec<String> = WALK_ORDER.iter().filter(|slot| specs.iter().any(|s| text(s, "id") == **slot)).map(|s| s.to_string()).collect();
        order.extend(specs.iter().filter(|s| s["extra"].as_bool() == Some(true)).map(|s| text(s, "id")));
        let mut steps: Vec<Step> = order
            .iter()
            .map(|slot| {
                let verb = match slot.as_str() {
                    "lt" | "rt" => gettext("Pull {}"),
                    "ls" | "rs" => gettext("Click {}"),
                    _ => gettext("Press {}"),
                };
                Step { slot: slot.clone(), axis: String::new(), art: slot.clone(), label: label(slot), prompt: verb.replace("{}", &label(slot)) }
            })
            .collect();
        for (role, stick, throw) in [
            ("lx", "ls", gettext("the left stick right")),
            ("ly", "ls", gettext("the left stick down")),
            ("rx", "rs", gettext("the right stick right")),
            ("ry", "rs", gettext("the right stick down")),
        ] {
            if device["axes"].get(role).is_some() {
                steps.push(Step {
                    slot: String::new(),
                    axis: role.into(),
                    art: stick.into(),
                    label: throw.clone(),
                    prompt: gettext("Hold {}").replace("{}", &throw),
                });
            }
        }
        if steps.is_empty() {
            self.say(&gettext("Nothing to set up on this controller"));
            return;
        }
        let art = self.art();
        let prompt = gtk::Label::builder().css_classes(["title-2"]).wrap(true).justify(gtk::Justification::Center).build();
        let count = gtk::Label::builder().css_classes(["dimmed"]).build();
        let back = gtk::Button::builder().label(gettext("_Back")).use_underline(true).build();
        let skip = gtk::Button::builder().label(gettext("_Skip")).use_underline(true).build();
        let stop = gtk::Button::builder().label(gettext("_Stop")).use_underline(true).css_classes(["destructive-action"]).build();
        let controls = gtk::Box::builder().spacing(12).halign(gtk::Align::Center).build();
        controls.append(&back);
        controls.append(&skip);
        controls.append(&stop);
        let column =
            gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(12).margin_bottom(24).margin_start(24).margin_end(24).build();
        column.append(&art);
        column.append(&prompt);
        column.append(&count);
        column.append(&controls);
        let toolbar = adw::ToolbarView::builder().content(&column).build();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        let page = adw::NavigationPage::new(&toolbar, &gettext("Set Up the Buttons"));
        let this = Rc::downgrade(self);
        back.connect_clicked(move |_| {
            this.upgrade().inspect(|t| t.walk_back());
        });
        let this = Rc::downgrade(self);
        skip.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.send(json!({"cmd": "cancel"}));
                this.walk_skip();
            }
        });
        let this = Rc::downgrade(self);
        stop.connect_clicked(move |_| {
            if let Some(this) = this.upgrade() {
                this.send(json!({"cmd": "cancel"}));
                this.stop_walk();
                this.say(&gettext("Setup stopped"));
            }
        });
        let this = Rc::downgrade(self);
        page.connect_hidden(move |_| {
            if let Some(this) = this.upgrade().filter(|t| t.walk.borrow().is_some()) {
                this.send(json!({"cmd": "cancel"}));
                this.stop_walk();
            }
        });
        self.walk.replace(Some(Walk {
            id: text(&device, "id"),
            family: text(&device, "family"),
            name: text(&device, "name"),
            steps,
            index: 0,
            found: Vec::new(),
            missed: Vec::new(),
            axes: Vec::new(),
            history: Vec::new(),
            seconds: WALK_STEP_S,
            prompt,
            count,
            back,
            timer: None,
        }));
        dialog.push_subpage(&page);
        self.walk_send();
        self.sync();
    }

    fn walk_send(self: &Rc<Self>) {
        let command = {
            let mut walk = self.walk.borrow_mut();
            let Some(walk) = walk.as_mut() else { return };
            let step = &walk.steps[walk.index];
            walk.seconds = WALK_STEP_S;
            walk.prompt.set_label(&step.prompt);
            walk.back.set_sensitive(walk.index > 0);
            self.learning.replace(step.art.clone());
            if step.axis.is_empty() {
                json!({"cmd": "learn", "id": walk.id, "slot": step.slot})
            } else {
                json!({"cmd": "learn", "id": walk.id, "axis": step.axis, "except": walk.axes})
            }
        };
        let art = self.learning.borrow().clone();
        self.each_art(|a| a.set_learning(&art));
        self.send(command);
        self.walk_count();
        let this = Rc::downgrade(self);
        let timer = glib::timeout_add_seconds_local(1, move || {
            let Some(this) = this.upgrade() else { return glib::ControlFlow::Break };
            let left = {
                let mut walk = this.walk.borrow_mut();
                let Some(walk) = walk.as_mut() else { return glib::ControlFlow::Break };
                walk.seconds = walk.seconds.saturating_sub(1);
                walk.seconds
            };
            if left == 0 {
                if let Some(walk) = this.walk.borrow_mut().as_mut() {
                    walk.timer = None;
                }
                this.send(json!({"cmd": "cancel"}));
                this.walk_skip();
                return glib::ControlFlow::Break;
            }
            this.walk_count();
            glib::ControlFlow::Continue
        });
        if let Some(old) = self.walk.borrow_mut().as_mut().and_then(|w| w.timer.replace(timer)) {
            old.remove();
        }
    }

    fn walk_count(&self) {
        if let Some(walk) = self.walk.borrow().as_ref() {
            walk.count.set_label(
                &gettext("Step {} of {} · {} s").replacen("{}", &(walk.index + 1).to_string(), 1).replacen("{}", &walk.steps.len().to_string(), 1).replacen(
                    "{}",
                    &walk.seconds.to_string(),
                    1,
                ),
            );
        }
    }

    fn walk_stop_timer(&self) {
        if let Some(timer) = self.walk.borrow_mut().as_mut().and_then(|w| w.timer.take()) {
            timer.remove();
        }
    }

    fn walk_skip(self: &Rc<Self>) {
        self.walk_stop_timer();
        {
            let mut walk = self.walk.borrow_mut();
            let Some(walk) = walk.as_mut() else { return };
            let label = walk.steps[walk.index].label.clone();
            walk.missed.push(label.clone());
            walk.history.push(Record::Missed(label));
        }
        self.walk_advance();
    }

    fn walk_learned(self: &Rc<Self>, slot: &str, axis: &str, from: &str, code: &str) {
        let stolen = {
            let mut walk = self.walk.borrow_mut();
            let Some(walk) = walk.as_mut() else { return };
            let step = &walk.steps[walk.index];
            if (slot, axis) != (step.slot.as_str(), step.axis.as_str()) {
                return;
            }
            self.walk_stop_timer_in(walk);
            if !axis.is_empty() {
                walk.axes.push(code.trim_end_matches('-').to_string());
            }
            let mut stolen = String::new();
            if !from.is_empty() && walk.found.iter().any(|f| f == from) {
                stolen = walk.steps.iter().find(|s| s.slot == from).map(|s| s.label.clone()).unwrap_or_else(|| from.to_string());
                walk.found.retain(|f| f != from);
                walk.missed.push(stolen.clone());
            }
            walk.found.push(if slot.is_empty() { axis.to_string() } else { slot.to_string() });
            walk.history.push(Record::Found { slot: slot.into(), axis: axis.into(), code: code.into(), from: from.into(), stolen: stolen.clone() });
            stolen
        };
        if !stolen.is_empty() {
            let label = self.walk.borrow().as_ref().map(|w| w.steps[w.index].label.clone()).unwrap_or_default();
            self.say(&gettext("That button was {}: it is {} now").replacen("{}", &stolen, 1).replacen("{}", &label, 1));
        }
        self.walk_advance();
    }

    fn walk_stop_timer_in(&self, walk: &mut Walk) {
        if let Some(timer) = walk.timer.take() {
            timer.remove();
        }
    }

    fn walk_advance(self: &Rc<Self>) {
        let done = {
            let mut walk = self.walk.borrow_mut();
            let Some(walk) = walk.as_mut() else { return };
            walk.index += 1;
            walk.index >= walk.steps.len()
        };
        if !done {
            self.walk_send();
            return;
        }
        let Some(walk) = self.walk.take() else { return };
        self.stop_learning();
        let mut tally = gettextrs::ngettext("{} set up", "{} set up", walk.found.len() as u32).replace("{}", &walk.found.len().to_string());
        if !walk.missed.is_empty() {
            tally.push_str(&gettext(", {} skipped: {}").replacen("{}", &walk.missed.len().to_string(), 1).replacen("{}", &walk.missed.join(", "), 1));
        }
        self.say(&format!("{}: {tally}", walk.name));
        if let Some(dialog) = self.dialog.upgrade() {
            dialog.pop_subpage();
        }
        self.sync();
    }

    /// Back one step: what it took is given back, the code to the button it came from, and the step asked again.
    fn walk_back(self: &Rc<Self>) {
        let record = {
            let mut walk = self.walk.borrow_mut();
            let Some(walk) = walk.as_mut() else { return };
            if walk.index == 0 {
                return;
            }
            self.walk_stop_timer_in(walk);
            walk.index -= 1;
            let Some(record) = walk.history.pop() else { return };
            match &record {
                Record::Missed(label) => {
                    if let Some(at) = walk.missed.iter().position(|m| m == label) {
                        walk.missed.remove(at);
                    }
                }
                Record::Found { slot, axis, stolen, from, .. } => {
                    let key = if slot.is_empty() { axis } else { slot };
                    walk.found.retain(|f| f != key);
                    if !axis.is_empty() {
                        walk.axes.pop();
                    }
                    if !stolen.is_empty() {
                        walk.missed.retain(|m| m != stolen);
                        walk.found.push(from.clone());
                    }
                }
            }
            Some((record, walk.family.clone()))
        };
        self.send(json!({"cmd": "cancel"}));
        if let Some((Record::Found { slot, axis, code, from, .. }, family)) = record {
            if axis.is_empty() {
                let codes = |slot: &str| -> Vec<String> {
                    self.families()
                        .iter()
                        .find(|f| text(f, "id") == family)
                        .and_then(|f| f["slots"].as_array().cloned())
                        .unwrap_or_default()
                        .iter()
                        .find(|s| text(s, "id") == slot)
                        .and_then(|s| s["codes"].as_array().cloned())
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|c| c.as_str().map(String::from))
                        .collect()
                };
                let given_back: Vec<String> = codes(&slot).into_iter().filter(|c| *c != code).collect();
                let mut returned: Vec<String> = vec![code.clone()];
                returned.extend(codes(&from).into_iter().filter(|c| *c != code));
                let this = Rc::downgrade(self);
                glib::spawn_future_local(async move {
                    let (f, s) = (family.clone(), slot.clone());
                    let _ = backend::call(move |core| async move { core.set_controller_button(&f, &s, Some(given_back)).await }).await;
                    if !from.is_empty() {
                        let _ = backend::call(move |core| async move { core.set_controller_button(&family, &from, Some(returned)).await }).await;
                    }
                    if let Some(this) = this.upgrade() {
                        this.changed();
                    }
                });
            }
        }
        self.walk_send();
    }

    fn stop_walk(&self) {
        self.walk_stop_timer();
        if self.walk.take().is_some() {
            self.stop_learning();
            self.sync();
        }
    }
}

/// Preferences › Controller: the pad drawn live, its buttons and their macros; the watcher runs while the page shows.
pub fn page(dialog: &adw::PreferencesDialog) -> Rc<Controller> {
    let page = adw::PreferencesPage::builder().name("controller").title(gettext("Controller")).icon_name("input-gaming-symbolic").build();
    let top = adw::PreferencesGroup::new();
    let status = gtk::Label::builder().wrap(true).justify(gtk::Justification::Center).css_classes(["dimmed"]).margin_top(6).build();
    let device_row = adw::ComboRow::builder().title(gettext("Controller")).visible(false).build();
    let family_row = adw::ComboRow::builder().title(gettext("Controller Type")).subtitle(gettext("Whose buttons to set up")).visible(false).build();
    let test = adw::SwitchRow::builder().title(gettext("Test the Buttons")).subtitle(gettext("Presses light up, the sticks and triggers move")).build();
    let walk_button = gtk::Button::builder().label(gettext("Start")).valign(gtk::Align::Center).build();
    let walk_row = adw::ActionRow::builder().title(gettext("Set Up the Buttons")).subtitle(gettext("Press each one in turn")).build();
    walk_row.add_suffix(&walk_button);
    let pad = adw::PreferencesGroup::new();
    pad.add(&device_row);
    pad.add(&family_row);
    pad.add(&test);
    pad.add(&walk_row);
    let buttons = adw::PreferencesGroup::new();
    let hold = adw::ComboRow::builder().title(gettext("Hold Length")).subtitle(gettext("A press this long is a hold")).build();
    hold.set_model(Some(&gtk::StringList::new(&HOLDS.iter().map(|h| format!("{h} ms")).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>())));
    let step = adw::ComboRow::builder().title(gettext("Volume Step")).subtitle(gettext("How far a volume macro moves the sound")).build();
    step.set_model(Some(&gtk::StringList::new(&STEPS.iter().map(|s| format!("{s} %")).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>())));
    let timing = adw::PreferencesGroup::builder().title(gettext("Timing")).build();
    timing.add(&hold);
    timing.add(&step);
    let home =
        adw::SwitchRow::builder().title(gettext("HOME Opens Universe")).subtitle(gettext("HOME pressed in another app brings Big Screen to the front")).build();
    let big_screen = adw::PreferencesGroup::builder().title(gettext("Big Screen")).build();
    big_screen.add(&home);

    let remembered = {
        let family = State::load().controller_family;
        if family.is_empty() {
            "xbox".to_string()
        } else {
            family
        }
    };
    let this = Rc::new(Controller {
        page: page.clone(),
        dialog: dialog.downgrade(),
        arts: RefCell::default(),
        status: status.clone(),
        device_row: device_row.clone(),
        family_row: family_row.clone(),
        test: test.clone(),
        walk_button: walk_button.clone(),
        buttons: buttons.clone(),
        rows: RefCell::default(),
        hold: hold.clone(),
        step: step.clone(),
        home: home.clone(),
        active_watch: RefCell::default(),
        watcher: RefCell::default(),
        state: RefCell::new(Value::Null),
        devices: RefCell::default(),
        current: RefCell::default(),
        status_kind: RefCell::default(),
        passive: Cell::new(false),
        learning: RefCell::default(),
        walk: RefCell::default(),
        editor: RefCell::default(),
        restart_ms: Cell::new(RESTART_MS),
        open: Cell::new(false),
        syncing: Cell::new(false),
        remembered: RefCell::new(remembered),
    });
    let art = this.art();
    top.add(&art);
    top.add(&status);
    page.add(&top);
    page.add(&pad);
    page.add(&buttons);
    page.add(&timing);
    page.add(&big_screen);

    let weak: Weak<Controller> = Rc::downgrade(&this);
    test.connect_active_notify(move |row| {
        let Some(this) = weak.upgrade() else { return };
        this.send(json!({"cmd": "axes", "on": row.is_active()}));
        this.each_art(|art| art.set_readouts(row.is_active()));
    });
    let weak = Rc::downgrade(&this);
    walk_button.connect_clicked(move |_| {
        weak.upgrade().inspect(|t| t.start_walk());
    });
    let weak = Rc::downgrade(&this);
    device_row.connect_selected_notify(move |row| {
        let Some(this) = weak.upgrade().filter(|t| !t.syncing.get()) else { return };
        let id = this.devices.borrow().get(row.selected() as usize).map(|d| text(d, "id"));
        if let Some(id) = id {
            this.current.replace(id);
            this.each_art(|art| art.clear_input());
            this.remember(&this.family());
            this.rebuild();
        }
    });
    let weak = Rc::downgrade(&this);
    family_row.connect_selected_notify(move |row| {
        let Some(this) = weak.upgrade().filter(|t| !t.syncing.get()) else { return };
        let family = this.families().get(row.selected() as usize).map(|f| text(f, "id"));
        if let Some(family) = family {
            this.remember(&family);
            this.rebuild();
        }
    });
    let weak = Rc::downgrade(&this);
    hold.connect_selected_notify(move |row| {
        if let Some(this) = weak.upgrade().filter(|t| !t.syncing.get()) {
            this.set_config("controller.hold_ms", HOLDS[row.selected() as usize % HOLDS.len()].to_string());
        }
    });
    let weak = Rc::downgrade(&this);
    step.connect_selected_notify(move |row| {
        if let Some(this) = weak.upgrade().filter(|t| !t.syncing.get()) {
            this.set_config("controller.volume_step", STEPS[row.selected() as usize % STEPS.len()].to_string());
        }
    });
    let weak = Rc::downgrade(&this);
    home.connect_active_notify(move |row| {
        if let Some(this) = weak.upgrade().filter(|t| !t.syncing.get()) {
            this.set_config("controller.home_summons", row.is_active().to_string());
        }
    });
    let weak = Rc::downgrade(&this);
    dialog.connect_visible_page_notify(move |dialog| {
        let Some(this) = weak.upgrade() else { return };
        if dialog.visible_page().is_some_and(|p| p == this.page) {
            this.start();
        } else {
            this.stop();
        }
    });
    let weak = Rc::downgrade(&this);
    dialog.connect_closed(move |_| {
        weak.upgrade().inspect(|t| t.stop());
    });
    this.reload(false);
    this
}
