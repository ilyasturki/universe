use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};
use serde_json::Value;

use crate::app::Application;
use crate::backend;
use crate::jobs::{ComponentJob, Job, Kind};
use crate::window::Window;

const PROTON_TOOL: &str = "umu-run";

/// A group's load, held weakly by what reloads it.
type Again = RefCell<Option<Weak<dyn Fn()>>>;

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn app() -> Option<Application> {
    gio::Application::default().and_downcast()
}

pub async fn listing(refresh: bool) -> universe::Result<Vec<Value>> {
    let listed = backend::call(move |core| async move { core.components(refresh).await }).await?;
    Ok(listed["components"].as_array().cloned().unwrap_or_default())
}

fn origin(build: &Value) -> String {
    match build["origin"].as_str().unwrap_or_default() {
        "universe" => "Universe".into(),
        "nix" => "Nix".into(),
        "system" => gettext("system"),
        "lutris" => "Lutris".into(),
        "steam" => "Steam".into(),
        "heroic" => "Heroic".into(),
        "umu" => "umu".into(),
        "local" => gettext("your proton folder"),
        "config" => "config.toml".into(),
        other => other.into(),
    }
}

pub fn build_text(build: &Value) -> String {
    let version = text(build, "version");
    format!("{} · {}", if version.is_empty() { gettext("unknown version") } else { version }, origin(build))
}

fn builds(c: &Value) -> impl Iterator<Item = &Value> {
    c["builds"].as_array().into_iter().flatten()
}

fn managed(c: &Value) -> Vec<&Value> {
    builds(c).filter(|b| b["managed"] == true).collect()
}

fn latest(c: &Value) -> String {
    text(&c["latest"], "version")
}

fn size(bytes: u64) -> String {
    glib::format_size(bytes).to_string()
}

/// An update, a newer build than the one found, a game waiting on it.
pub fn wants(c: &Value) -> bool {
    !text(c, "update").is_empty() || !text(c, "proposal").is_empty()
}

/// What to do about it, `Updated` after an update landed: empty when nothing.
pub fn tag(c: &Value) -> String {
    let update = text(c, "update");
    match text(c, "proposal").as_str() {
        _ if !update.is_empty() => gettext("Update {}").replace("{}", &update),
        "newer" => gettext("{} available").replace("{}", &latest(c)),
        "install" => gettext("Needed"),
        _ if !c["recent"].is_null() => gettext("Updated"),
        _ => String::new(),
    }
}

pub fn state(c: &Value) -> String {
    if !c["in_use"].is_null() {
        build_text(&c["in_use"])
    } else if !c["latest"].is_null() || text(c, "kind") == "system" {
        gettext("Not installed")
    } else {
        gettext("No download")
    }
}

/// What a runner's page installs: Proton's builds and umu-run, which starts them; another runner its own. The ones in use
/// first.
pub fn of_runner(ident: &str, kind: &str, listed: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = if kind == "proton" {
        listed.iter().filter(|c| text(c, "kind") == "proton").chain(listed.iter().filter(|c| text(c, "id") == PROTON_TOOL)).cloned().collect()
    } else {
        listed.iter().filter(|c| text(c, "runner") == ident).cloned().collect()
    };
    out.sort_by_key(|c| (c["in_use"].is_null(), text(c, "kind") != "proton", c["builds"].as_array().is_none_or(Vec::is_empty), text(c, "name").to_lowercase()));
    out
}

/// The components that are no runner's, what waits on them first.
pub fn tools(listed: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> =
        listed.iter().filter(|c| matches!(text(c, "kind").as_str(), "tool" | "system") && text(c, "id") != PROTON_TOOL).cloned().collect();
    out.sort_by_key(|c| (text(c, "proposal") != "install", !wants(c), c["in_use"].is_null(), text(c, "name").to_lowercase()));
    out
}

/// A runner's components at a glance: the build in use, the first tag, whether something waits.
pub fn summary(own: &[Value]) -> (String, String, bool) {
    let in_use = own.iter().find(|c| !c["in_use"].is_null()).map(|c| build_text(&c["in_use"])).unwrap_or_default();
    let first = own.iter().map(tag).find(|t| !t.is_empty()).unwrap_or_default();
    (in_use, first, own.iter().any(wants))
}

pub struct Action {
    pub id: String,
    pub label: String,
}

pub fn quick(own: &[Value]) -> Option<(&Value, Action)> {
    own.iter().filter(|c| wants(c)).find_map(|c| actions(c).into_iter().next().filter(|a| a.id == "update" || a.id == "install").map(|a| (c, a)))
}

fn action(id: impl Into<String>, label: impl Into<String>) -> Action {
    Action { id: id.into(), label: label.into() }
}

/// `install`, `update`, `use:<build>`, `versions`, `rollback`, `remove:<version>`, `uninstall`: the useful one first.
pub fn actions(c: &Value) -> Vec<Action> {
    let kind = text(c, "kind");
    if kind == "system" {
        let installable = c["installable"] == true && c["in_use"].is_null();
        return if installable { vec![action("install", gettext("Install From the Distribution"))] } else { vec![] };
    }
    let mut out = Vec::new();
    let managed = managed(c);
    let newest = latest(c);
    let runner = kind == "emulator" || kind == "wine";
    let switchable = kind != "tool";
    let update = text(c, "update");
    if !update.is_empty() {
        out.push(action("update", gettext("Update to {}").replace("{}", &update)));
    } else if !newest.is_empty() && !managed.iter().any(|b| text(b, "version") == newest) {
        let label = if switchable && !c["in_use"].is_null() { gettext("Install {} and Use It") } else { gettext("Install {}") };
        out.push(action("install", label.replace("{}", &newest)));
    }
    if switchable {
        for b in builds(c).filter(|b| b["in_use"] != true) {
            let target = match (b["managed"] == true, runner, text(b, "name")) {
                (true, ..) => text(b, "version"),
                (false, true, _) => "system".into(),
                (false, false, name) if name.is_empty() => text(b, "version"),
                (false, false, name) => name,
            };
            out.push(action(format!("use:{target}"), gettext("Use {}").replace("{}", &build_text(b))));
        }
    }
    let follows = if kind == "proton" {
        text(c, "family")
    } else if runner {
        "latest".into()
    } else {
        String::new()
    };
    let setting = text(c, "setting");
    if !follows.is_empty() && !setting.is_empty() && setting != follows && !managed.is_empty() {
        out.push(action("use:latest", gettext("Follow the Newest {}").replace("{}", &text(c, "name"))));
    }
    if !version_actions(c).is_empty() {
        out.push(action("versions", gettext("Install Another Version")));
    }
    if let Some(first) = managed.first() {
        if (managed.len() > 1 || builds(c).any(|b| b["managed"] != true)) && first["pinned"] != true {
            out.push(action("rollback", gettext("Roll Back {}").replace("{}", &text(first, "version"))));
        }
    }
    for b in managed.iter().filter(|b| b["pinned"] != true && b["in_use"] != true) {
        let version = text(b, "version");
        let disk = b["disk"].as_u64().unwrap_or(0);
        let label = if disk > 0 { format!("{version} · {}", size(disk)) } else { version.clone() };
        out.push(action(format!("remove:{version}"), gettext("Remove {}").replace("{}", &label)));
    }
    if !managed.is_empty() {
        let disk: u64 = managed.iter().map(|b| b["disk"].as_u64().unwrap_or(0)).sum();
        out.push(action("uninstall", if disk > 0 { gettext("Uninstall · {}").replace("{}", &size(disk)) } else { gettext("Uninstall") }));
    }
    out
}

/// The catalogue's other builds, `install:<version>`.
pub fn version_actions(c: &Value) -> Vec<Action> {
    let newest = latest(c);
    c["available"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|a| a["installed"] != true && text(a, "version") != newest)
        .map(|a| {
            let version = text(a, "version");
            let date: String = text(a, "date").chars().take(10).collect();
            let bytes = a["size"].as_u64().unwrap_or(0);
            let parts = [
                version.clone(),
                if bytes > 0 { size(bytes) } else { String::new() },
                date,
                if a["skipped"] == true { gettext("rolled back") } else { String::new() },
            ];
            action(format!("install:{version}"), parts.iter().filter(|p| !p.is_empty()).cloned().collect::<Vec<_>>().join(" · "))
        })
        .collect()
}

pub struct Ask {
    pub heading: String,
    pub body: String,
    pub yes: String,
    pub destructive: bool,
}

fn uninstall_body(c: &Value) -> String {
    let managed = managed(c);
    let versions: Vec<String> = managed.iter().map(|b| text(b, "version")).collect();
    let disk: u64 = managed.iter().map(|b| b["disk"].as_u64().unwrap_or(0)).sum();
    let gone = if versions.len() < 3 { versions.join(gettext(" and ").as_str()) } else { gettext("its {} builds").replace("{}", &versions.len().to_string()) };
    let what = gettext("Universe removes {}").replace("{}", &gone) + &if disk > 0 { format!(" ({})", size(disk)) } else { String::new() } + ".";
    let found: Vec<&Value> = builds(c).filter(|b| b["managed"] != true).collect();
    let used = c["used_by"].as_u64().unwrap_or(0);
    let after = if !managed.iter().any(|b| b["in_use"] == true) {
        gettext("Universe can install it again later.")
    } else if found.len() > 1 {
        gettext("The newest of the other {} builds runs in its place.").replace("{}", &found.len().to_string())
    } else if let Some(other) = found.first() {
        gettext("{} runs in its place.").replace("{}", &build_text(other))
    } else if used > 0 {
        gettext("The games on it won’t start until it is installed again.")
    } else {
        gettext("Universe can install it again later.")
    };
    format!("{what} {after}")
}

/// What to ask before `action`, the install's notice included; none for what is undone as easily.
pub fn ask(c: &Value, action: &str) -> Option<Ask> {
    let name = text(c, "name");
    let (verb, arg) = action.split_once(':').unwrap_or((action, ""));
    let ask = |heading: String, body: String, yes: String, destructive: bool| Some(Ask { heading, body, yes, destructive });
    match verb {
        "install" if text(c, "kind") == "system" => {
            let packages: Vec<&str> = c["packages"].as_array().into_iter().flatten().filter_map(|p| p.as_str()).collect();
            let body = gettext("{} from your distribution’s packages. It asks for your password.").replace("{}", &packages.join(", "));
            ask(gettext("Install {}?").replace("{}", &name), body, gettext("_Install"), false)
        }
        "install" => {
            let version = if arg.is_empty() { latest(c) } else { arg.to_string() };
            let bytes = c["available"].as_array().into_iter().flatten().find(|a| text(a, "version") == version).and_then(|a| a["size"].as_u64()).unwrap_or(0);
            let download = if bytes > 0 { gettext("{} to download").replace("{}", &size(bytes)) } else { String::new() };
            let body = [text(c, "notice"), download].into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join("\n\n");
            ask(gettext("Install {}?").replace("{}", &format!("{name} {version}")), body, gettext("_Install"), false)
        }
        "remove" => {
            ask(gettext("Remove {}?").replace("{}", &format!("{name} {arg}")), gettext("Universe can install it again later."), gettext("_Remove"), true)
        }
        "uninstall" => ask(gettext("Uninstall {}?").replace("{}", &name), uninstall_body(c), gettext("_Uninstall"), true),
        "rollback" => ask(
            gettext("Roll Back {}?").replace("{}", &format!("{name} {}", managed(c).first().map(|b| text(b, "version")).unwrap_or_default())),
            gettext("Universe removes it and skips that version; the next one updates as usual."),
            gettext("_Roll Back"),
            true,
        ),
        _ => None,
    }
}

/// On the preferences dialog the widget sits in, else the window.
pub(crate) fn toast(anchor: &gtk::Widget, line: &str) {
    if let Some(dialog) = anchor.ancestor(adw::PreferencesDialog::static_type()).and_downcast::<adw::PreferencesDialog>() {
        dialog.add_toast(crate::dialogs::toast(line));
    } else if let Some(win) = anchor.root().and_downcast::<Window>() {
        win.toast(crate::dialogs::toast(line));
    }
}

/// Does `action` to `c`, asking first when it costs something; `changed` once the listing changed, after its job for an
/// install or an update.
pub fn act(anchor: &impl IsA<gtk::Widget>, c: &Value, action: &str, changed: Rc<dyn Fn()>) {
    let anchor = anchor.upcast_ref::<gtk::Widget>().clone();
    let Some(question) = ask(c, action) else {
        run(&anchor, c, action, changed);
        return;
    };
    let dialog = adw::AlertDialog::new(Some(&question.heading), Some(&question.body));
    let no = if question.destructive { gettext("_Keep It") } else { gettext("_Not Now") };
    dialog.add_responses(&[("no", &no), ("yes", &question.yes)]);
    dialog.set_response_appearance("yes", if question.destructive { adw::ResponseAppearance::Destructive } else { adw::ResponseAppearance::Suggested });
    dialog.set_default_response(Some(if question.destructive { "no" } else { "yes" }));
    dialog.set_close_response("no");
    let (weak, c, action) = (anchor.downgrade(), c.clone(), action.to_string());
    dialog.connect_response(Some("yes"), move |_, _| {
        if let Some(anchor) = weak.upgrade() {
            run(&anchor, &c, &action, changed.clone());
        }
    });
    dialog.present(Some(&anchor));
}

fn run(anchor: &gtk::Widget, c: &Value, action: &str, changed: Rc<dyn Fn()>) {
    let (id, name) = (text(c, "id"), text(c, "name"));
    let (verb, arg) = action.split_once(':').map(|(v, a)| (v.to_string(), a.to_string())).unwrap_or((action.to_string(), String::new()));
    if verb == "install" || verb == "update" {
        let follow = verb == "install" && text(c, "kind") != "tool" && !c["in_use"].is_null() && arg.is_empty();
        let version = if arg.is_empty() && verb == "install" { latest(c) } else { arg };
        let Some(app) = app() else { return };
        if app.start_component(ComponentJob { id, name, version, update: verb == "update", follow }) {
            if let Some(job) = app.job() {
                job.connect_running_notify(move |job| {
                    if !job.running() {
                        changed();
                    }
                });
            }
        }
        return;
    }
    let weak = anchor.downgrade();
    glib::spawn_future_local(async move {
        let (who, target) = (name.clone(), arg.clone());
        let result = backend::call(move |core| async move {
            match verb.as_str() {
                "use" => core.component_use(&id, &target).await.map(|_| match target.as_str() {
                    "latest" => gettext("{}: the newest build").replace("{}", &who),
                    "system" => gettext("{}: the system’s").replace("{}", &who),
                    build => format!("{who}: {build}"),
                }),
                "remove" => core.component_remove(&id, &target).await.map(|_| gettext("Removed {}").replace("{}", &format!("{who} {target}"))),
                "uninstall" => core.component_uninstall(&id).await.map(|_| gettext("Uninstalled {}").replace("{}", &who)),
                "rollback" => core.component_rollback(&id).await.map(|_| gettext("Rolled back {}").replace("{}", &who)),
                other => Err(universe::Error::Invalid(format!("{id}: no action {other}"))),
            }
        })
        .await;
        if let Some(anchor) = weak.upgrade() {
            toast(&anchor, &result.unwrap_or_else(|e| e.to_string()));
        }
        changed();
    });
}

/// A row and the key a rebuild finds it again by.
pub type Keyed = (String, gtk::Widget);

/// The key of the row among `rows` that holds the focus, let go of first: a dialog whose focused row is taken away takes no
/// focus again.
pub fn take_focus(rows: &[Keyed]) -> Option<String> {
    let (_, first) = rows.first()?;
    let dialog = first.ancestor(adw::Dialog::static_type()).and_downcast::<adw::Dialog>();
    let focus = match &dialog {
        Some(dialog) => dialog.focus(),
        None => first.root().and_then(|root| root.focus()),
    }?;
    let (key, _) = rows.iter().find(|(_, row)| focus == *row || focus.is_ancestor(row))?;
    match (dialog, first.root()) {
        (Some(dialog), _) => dialog.set_focus(None::<&gtk::Widget>),
        (None, Some(root)) => root.set_focus(None::<&gtk::Widget>),
        (None, None) => {}
    }
    Some(key.clone())
}

/// The focus back on the row of `key`, once rebuilt.
pub fn refocus(rows: &[Keyed], key: Option<String>) {
    if let Some((_, row)) = key.and_then(|key| rows.iter().find(|(k, _)| *k == key)) {
        row.grab_focus();
    }
}

/// Whether a `job-changed` started or ended a component's job: a store's or the art's leaves the components as they were.
#[derive(Default)]
pub struct JobWatch {
    running: Cell<bool>,
}

impl JobWatch {
    pub fn changed(&self, job: Option<Kind>) -> bool {
        let now = job == Some(Kind::Component);
        self.running.replace(now) || now
    }
}

/// `f` when a component's job starts or ends.
pub fn on_jobs(app: &Application, f: impl Fn() + 'static) -> glib::SignalHandlerId {
    let watch = JobWatch::default();
    app.connect_local("job-changed", false, move |values| {
        let job = values[0].get::<Application>().ok().and_then(|app| app.job()).map(|j| j.kind());
        if watch.changed(job) {
            f();
        }
        None
    })
}

fn running(id: &str) -> Option<Job> {
    app()?.job().filter(|j| j.kind() == Kind::Component && j.target() == id)
}

/// A component's row: the build that runs, what to do about it, its options; its progress and Stop while its job runs.
pub fn row(c: &Value, changed: Rc<dyn Fn()>) -> adw::ActionRow {
    let row = crate::rows::plain(adw::ActionRow::new(), text(c, "name"), state(c));
    if let Some(job) = running(&text(c, "id")) {
        follow(&row, &job, |job| {
            let mut line = vec![if job.cancelled() { gettext("Stopping…") } else { job.label() }];
            if job.total() > 0 && !job.cancelled() {
                line.push(gettext("{} of {}").replacen("{}", &size(job.done()), 1).replacen("{}", &size(job.total()), 1));
            }
            line.join(" · ")
        });
        return row;
    }
    let tag = tag(c);
    if !tag.is_empty() {
        let class = if wants(c) { "accent" } else { "dimmed" };
        row.add_suffix(&gtk::Label::builder().label(&tag).css_classes([class, "caption-heading"]).valign(gtk::Align::Center).build());
    }
    let actions = actions(c);
    if actions.is_empty() {
        return row;
    }
    let menu = gio::Menu::new();
    let item = |a: &Action| {
        let item = gio::MenuItem::new(Some(&a.label), None);
        item.set_action_and_target_value(Some("component.act"), Some(&a.id.to_variant()));
        item
    };
    for a in &actions {
        if a.id == "versions" {
            let versions = gio::Menu::new();
            for v in version_actions(c) {
                versions.append_item(&item(&v));
            }
            menu.append_submenu(Some(&a.label), &versions);
        } else {
            menu.append_item(&item(a));
        }
    }
    let group = gio::SimpleActionGroup::new();
    let act_on = gio::SimpleAction::new("act", Some(glib::VariantTy::STRING));
    let (weak, c) = (row.downgrade(), c.clone());
    act_on.connect_activate(move |_, value| {
        let (Some(row), Some(action)) = (weak.upgrade(), value.and_then(|v| v.str().map(String::from))) else { return };
        act(&row, &c, &action, changed.clone());
    });
    group.add_action(&act_on);
    row.insert_action_group("component", Some(&group));
    let button = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .menu_model(&menu)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .tooltip_text(gettext("Options"))
        .build();
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    row
}

pub fn follow(row: &adw::ActionRow, job: &Job, line: impl Fn(&Job) -> String + Clone + 'static) {
    let bar = gtk::ProgressBar::builder().valign(gtk::Align::Center).width_request(120).build();
    let stop = gtk::Button::builder().label(gettext("_Stop")).use_underline(true).valign(gtk::Align::Center).build();
    row.add_suffix(&bar);
    row.add_suffix(&stop);
    let weak = job.downgrade();
    stop.connect_clicked(move |button| {
        if let Some(job) = weak.upgrade() {
            job.cancel();
            button.set_sensitive(false);
        }
    });
    let (weak_row, weak_bar) = (row.downgrade(), bar.downgrade());
    let sync = move |job: &Job| {
        let (Some(row), Some(bar)) = (weak_row.upgrade(), weak_bar.upgrade()) else { return };
        bar.set_fraction(job.fraction());
        row.set_subtitle(&line(job));
    };
    sync(job);
    for name in ["message", "done", "total", "cancelled"] {
        let sync = sync.clone();
        job.connect_notify_local(Some(name), move |job, _| sync(job));
    }
}

/// A group of component rows, `pick` choosing them from the listing: read again when a job starts or ends and after an action.
pub fn group(title: &str, pick: impl Fn(&[Value]) -> Vec<Value> + 'static) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).visible(false).build();
    let rows: Rc<RefCell<Vec<Keyed>>> = Rc::default();
    let me: Rc<Again> = Rc::default();
    let (weak, pick, again) = (group.downgrade(), Rc::new(pick), me.clone());
    let load: Rc<dyn Fn()> = Rc::new(move || {
        let (weak, rows, pick, again) = (weak.clone(), rows.clone(), pick.clone(), again.clone());
        glib::spawn_future_local(async move {
            let listed = listing(false).await;
            let Some(group) = weak.upgrade() else { return };
            let changed: Rc<dyn Fn()> = Rc::new(move || {
                let load = again.borrow().as_ref().and_then(Weak::upgrade);
                load.inspect(|load| load());
            });
            let focus = take_focus(&rows.borrow());
            for (_, row) in rows.take() {
                group.remove(&row);
            }
            let picked = listed.map(|listed| pick(&listed)).unwrap_or_else(|e| {
                toast(group.upcast_ref(), &e.to_string());
                Vec::new()
            });
            group.set_visible(!picked.is_empty());
            for c in &picked {
                let row = row(c, changed.clone());
                group.add(&row);
                rows.borrow_mut().push((text(c, "id"), row.upcast()));
            }
            refocus(&rows.borrow(), focus);
        });
    });
    me.replace(Some(Rc::downgrade(&load)));
    let handler = app().map(|app| {
        let load = Rc::downgrade(&load);
        let handler = on_jobs(&app, move || {
            load.upgrade().inspect(|load| load());
        });
        (app.downgrade(), handler)
    });
    load();
    let held = RefCell::new(Some((load, handler)));
    group.connect_destroy(move |_| {
        if let Some((_, Some((app, handler)))) = held.take() {
            app.upgrade().inspect(|app| app.disconnect(handler));
        }
    });
    group
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ids(actions: Vec<Action>) -> Vec<String> {
        actions.into_iter().map(|a| a.id).collect()
    }

    fn build(version: &str, origin: &str, in_use: bool) -> Value {
        json!({"version": version, "origin": origin, "managed": origin == "universe", "in_use": in_use, "pinned": false, "disk": 64_000_000, "name": ""})
    }

    fn component(id: &str, kind: &str, builds: Vec<Value>, available: &[&str]) -> Value {
        let in_use = builds.iter().find(|b| b["in_use"] == true).cloned().unwrap_or(Value::Null);
        let available: Vec<Value> =
            available.iter().map(|v| json!({"version": v, "size": 21_000_000, "installed": false, "skipped": false, "date": ""})).collect();
        let latest = available.first().cloned().unwrap_or(Value::Null);
        json!({"id": id, "name": id, "kind": kind, "runner": if kind == "emulator" { id } else { "" }, "family": "", "builds": builds, "in_use": in_use,
            "latest": latest, "available": available, "update": "", "proposal": "", "setting": "", "notice": "", "recent": null, "used_by": 0})
    }

    #[test]
    fn the_useful_action_comes_first_and_what_costs_asks_first() {
        let mut xemu = component("xemu", "emulator", vec![build("0.8.135", "universe", true)], &["0.8.136"]);
        xemu["update"] = json!("0.8.136");
        assert_eq!(ids(actions(&xemu)), ["update", "uninstall"]);
        assert!(ask(&xemu, "update").is_none(), "an update runs straight away");
        assert!(ask(&xemu, "uninstall").is_some_and(|a| a.destructive));

        let mut wine = component("wine", "wine", vec![build("11.8", "nix", true)], &["11.18", "11.17"]);
        wine["proposal"] = json!("newer");
        wine["notice"] = json!("Read this first.");
        assert_eq!(ids(actions(&wine)), ["install", "versions"], "a newer build than the system's, then the others");
        assert_eq!(ids(version_actions(&wine)), ["install:11.17"]);
        let question = ask(&wine, "install").unwrap();
        assert!(question.body.starts_with("Read this first.") && !question.destructive, "the notice before every install");

        let mut held = component("eden", "emulator", vec![build("0.2.1", "universe", true), build("0.2.0", "universe", false)], &[]);
        held["setting"] = json!("0.2.1");
        assert_eq!(ids(actions(&held)), ["use:0.2.0", "use:latest", "rollback", "remove:0.2.0", "uninstall"], "held on one build: the newest is a pick away");

        let mut tool = component("gpu-screen-recorder", "system", vec![], &[]);
        tool["installable"] = json!(true);
        tool["packages"] = json!(["gpu-screen-recorder"]);
        assert_eq!(ids(actions(&tool)), ["install"]);
        assert!(ask(&tool, "install").is_some_and(|a| a.body.starts_with("gpu-screen-recorder")));
    }

    #[test]
    fn a_runners_row_offers_the_update_or_the_install_it_waits_on() {
        let mut xemu = component("xemu", "emulator", vec![build("0.8.135", "universe", true)], &["0.8.136"]);
        xemu["update"] = json!("0.8.136");
        let mut eden = component("eden", "emulator", vec![], &["0.2.1"]);
        eden["proposal"] = json!("install");
        let current = component("dolphin", "emulator", vec![build("2606a", "universe", true)], &["2606a"]);
        let pick = |own: &[Value]| quick(own).map(|(c, a)| (text(c, "id"), a.id));
        assert_eq!(pick(std::slice::from_ref(&xemu)), Some(("xemu".into(), "update".into())));
        assert_eq!(pick(&[current.clone(), eden]), Some(("eden".into(), "install".into())), "Needed: its install");
        assert_eq!(pick(&[current]), None, "nothing waits: the row only opens the page");
    }

    #[test]
    fn only_a_components_job_reads_the_runners_again() {
        let watch = JobWatch::default();
        assert!(!watch.changed(Some(Kind::Install)) && !watch.changed(None), "a store's install starts and ends");
        assert!(!watch.changed(Some(Kind::Scan)) && !watch.changed(None), "a scan likewise");
        assert!(watch.changed(Some(Kind::Component)), "a component's starts: its row shows the progress");
        assert!(watch.changed(None), "and ends: the build it brought");
        assert!(!watch.changed(None));
    }

    #[test]
    fn a_runners_page_holds_its_own_components_and_proton_its_builds_and_umu_run() {
        let listed = [
            component("dolphin", "emulator", vec![build("2606a", "nix", true)], &[]),
            component("proton-cachyos", "proton", vec![], &["11.0"]),
            component("ge-proton", "proton", vec![build("GE-Proton11-7", "universe", true)], &[]),
            component("umu-run", "tool", vec![build("1.4.0", "nix", true)], &[]),
            component("gogdl", "tool", vec![], &["1.1"]),
        ];
        let of = |id: &str, kind: &str| of_runner(id, kind, &listed).iter().map(|c| text(c, "id")).collect::<Vec<_>>();
        assert_eq!(of("proton", "proton"), ["ge-proton", "umu-run", "proton-cachyos"], "the ones in use first");
        assert_eq!(of("dolphin", "emulator"), ["dolphin"]);
        assert!(of("linux", "linux").is_empty());
        assert_eq!(tools(&listed).iter().map(|c| text(c, "id")).collect::<Vec<_>>(), ["gogdl"], "umu-run sits on Proton's page");
        assert_eq!(summary(&of_runner("proton", "proton", &listed)).0, "GE-Proton11-7 · Universe");
    }
}
