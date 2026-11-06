use std::path::{Path, PathBuf};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

// An AdwDialog presented during one frame is drawn from the next.
const SETTLE_FRAMES: u32 = 2;
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);
const LOADING_TIMEOUT: Duration = Duration::from_secs(10);

/// `UNIVERSE_DESKTOP_SCRIPT`: steps run once the library is up, animations off so each lands in a frame, then the app
/// quits, with status 1 when a step failed; `~` in an action stands for a space. The app runs apart from a running one
/// then, unless started as a service, adopts no scope and sweeps no journal.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(Duration),
    Shot(PathBuf),
    Action(String),
    Size(i32, i32),
    Button(String),
    Has(String),
    CloseToast,
    View(String),
    Dialog(String),
    Copy(PathBuf, PathBuf),
    Unreadable(String),
}

pub fn from_env() -> Option<Vec<Step>> {
    let text = std::env::var("UNIVERSE_DESKTOP_SCRIPT").ok()?;
    Some(parse(&text))
}

fn parse(text: &str) -> Vec<Step> {
    text.split_whitespace().map(|word| step(word).unwrap_or_else(|| Step::Unreadable(word.into()))).collect()
}

fn step(word: &str) -> Option<Step> {
    let (kind, arg) = word.split_once(':')?;
    match kind {
        "wait" => arg.parse().ok().map(|ms| Step::Wait(Duration::from_millis(ms))),
        "shot" => Some(Step::Shot(arg.into())),
        "action" => Some(Step::Action(arg.replace('~', " "))),
        "size" => arg.split_once('x').and_then(|(w, h)| Some(Step::Size(w.parse().ok()?, h.parse().ok()?))),
        "button" => Some(Step::Button(arg.into())),
        "has" => Some(Step::Has(arg.replace('~', " "))),
        "toast" => (arg == "close").then_some(Step::CloseToast),
        "view" => Some(Step::View(arg.into())),
        "dialog" => Some(Step::Dialog(arg.into())),
        "copy" => arg.split_once(':').map(|(from, to)| Step::Copy(from.into(), to.into())),
        _ => None,
    }
}

pub async fn run(window: gtk::Window, steps: Vec<Step>) {
    window.settings().set_gtk_enable_animations(false);
    let mut failed = false;
    for step in steps {
        if let Err(e) = run_step(&window, step).await {
            eprintln!("script: {e}");
            failed = true;
        }
    }
    if let Some(failure) = window.application().and_downcast::<crate::app::Application>().and_then(|app| app.failure()) {
        eprintln!("script: the core did not open: {failure}");
        failed = true;
    }
    if failed {
        std::process::exit(1);
    }
    window.application().inspect(|app| app.quit());
}

async fn run_step(window: &gtk::Window, step: Step) -> Result<(), String> {
    match step {
        Step::Wait(d) => glib::timeout_future(d).await,
        Step::Size(w, h) => window.set_default_size(w, h),
        Step::Action(detailed) => {
            drawn(window).await?;
            let (name, target) = gio::Action::parse_detailed_name(&detailed).map_err(|e| format!("{detailed}: {e}"))?;
            if !activate(window.upcast_ref(), &name, target.as_ref()) {
                return Err(format!("no action {name}"));
            }
        }
        Step::Button(action) => {
            let button = until(|| loading(window.upcast_ref()).is_none().then(|| button(window, &action, None)).flatten())
                .await
                .ok_or_else(|| format!("no visible button for {action}"))?;
            drawn(window).await?;
            if !button.is_sensitive() {
                return Err(format!("button for {action} is disabled"));
            }
            button.emit_clicked();
        }
        Step::Has(detailed) => {
            let (name, target) = gio::Action::parse_detailed_name(&detailed).map_err(|e| format!("{detailed}: {e}"))?;
            until(|| button(window, &name, target.as_ref()).filter(|b| b.is_sensitive()))
                .await
                .ok_or_else(|| format!("no visible button that can run {detailed}"))?;
            println!("has: {detailed}");
        }
        Step::CloseToast => {
            let close = until(|| {
                widgets(window.upcast_ref())
                    .into_iter()
                    .filter(|widget| widget.type_().name() == "AdwToastWidget")
                    .find_map(|toast| widgets(&toast).into_iter().rev().find_map(|widget| widget.downcast::<gtk::Button>().ok()))
            })
            .await
            .ok_or("no toast shown")?;
            drawn(window).await?;
            close.emit_clicked();
            println!("toast: close");
        }
        Step::View(name) => {
            until(|| {
                (loading(window.upcast_ref()).is_none()
                    && widgets(window.upcast_ref())
                        .iter()
                        .any(|widget| widget.downcast_ref::<adw::ViewStack>().is_some_and(|stack| stack.visible_child_name().as_deref() == Some(&name))))
                .then_some(())
            })
            .await
            .ok_or_else(|| format!("no visible view {name} after {} s", LOADING_TIMEOUT.as_secs()))?;
            println!("view: {name}");
        }
        Step::Dialog(response) => {
            let started = std::time::Instant::now();
            loop {
                if widgets(window.upcast_ref())
                    .iter()
                    .any(|widget| widget.downcast_ref::<adw::AlertDialog>().is_some_and(|dialog| dialog.default_response().as_deref() == Some(&response)))
                {
                    println!("dialog: {response}");
                    break;
                }
                if started.elapsed() >= LOADING_TIMEOUT {
                    return Err(format!("no visible dialog with response {response}"));
                }
                glib::timeout_future(Duration::from_millis(50)).await;
            }
        }
        Step::Copy(from, to) => {
            // Renamed into place: writing into a FIFO there would feed whoever is stuck reading it.
            let part = to.with_extension("part");
            std::fs::copy(&from, &part).and_then(|_| std::fs::rename(&part, &to)).map_err(|e| format!("copy {} to {}: {e}", from.display(), to.display()))?;
        }
        Step::Shot(path) => {
            loaded(window).await?;
            drawn(window).await?;
            shot(window, &path).map_err(|e| format!("shot {}: {e}", path.display()))?;
            println!("shot: {}", path.display());
        }
        Step::Unreadable(word) => return Err(format!("no step reads {word:?}")),
    }
    Ok(())
}

/// `found` polled until it gives something or `LOADING_TIMEOUT` passes.
async fn until<T>(found: impl Fn() -> Option<T>) -> Option<T> {
    let started = std::time::Instant::now();
    loop {
        if let Some(found) = found() {
            return Some(found);
        }
        if started.elapsed() >= LOADING_TIMEOUT {
            return None;
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
}

fn button(window: &gtk::Window, action: &str, target: Option<&glib::Variant>) -> Option<gtk::Button> {
    widgets(window.upcast_ref())
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk::Button>().ok())
        .find(|button| button.action_name().as_deref() == Some(action) && (target.is_none() || button.action_target_value().as_ref() == target))
}

async fn drawn(window: &gtk::Window) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = std::cell::Cell::new(Some(tx));
    let ticks = std::cell::Cell::new(0);
    // A tick runs before its frame's layout and paint, which are over by the time the receiver wakes.
    window.add_tick_callback(move |_, _| {
        ticks.set(ticks.get() + 1);
        if ticks.get() < SETTLE_FRAMES {
            return glib::ControlFlow::Continue;
        }
        if let Some(tx) = tx.take() {
            let _ = tx.send(());
        }
        glib::ControlFlow::Break
    });
    glib::future_with_timeout(FRAME_TIMEOUT, rx).await.map(drop).map_err(|_| format!("the window drew no frames in {} s", FRAME_TIMEOUT.as_secs()))
}

async fn loaded(window: &gtk::Window) -> Result<(), String> {
    let started = std::time::Instant::now();
    loop {
        let Some(stack) = loading(window.upcast_ref()) else { return Ok(()) };
        if started.elapsed() >= LOADING_TIMEOUT {
            let owner = std::iter::successors(stack.parent(), |w| w.parent()).find(|w| w.type_().name().starts_with("Universe"));
            let owner = owner.map_or_else(|| "the window".to_string(), |w| w.type_().name().to_string());
            return Err(format!("{owner} still loading after {} s", LOADING_TIMEOUT.as_secs()));
        }
        glib::timeout_future(Duration::from_millis(50)).await;
    }
}

fn loading(root: &gtk::Widget) -> Option<adw::ViewStack> {
    let mut widgets = vec![root.clone()];
    while let Some(widget) = widgets.pop() {
        if let Some(stack) = widget.downcast_ref::<adw::ViewStack>().filter(|s| s.visible_child_name().as_deref() == Some("loading")) {
            return Some(stack.clone());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if c.is_mapped() {
                widgets.push(c);
            }
        }
    }
    None
}

fn widgets(root: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut widgets = vec![root.clone()];
    let mut index = 0;
    while index < widgets.len() {
        let mut child = widgets[index].first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            if widget.is_mapped() {
                widgets.push(widget);
            }
        }
        index += 1;
    }
    widgets
}

/// Runs the action from the first widget on screen that reaches it: a page's own group (`store.`, a grid's `list.`) sits
/// below the window.
fn activate(root: &gtk::Widget, name: &str, target: Option<&glib::Variant>) -> bool {
    let mut widgets = vec![root.clone()];
    while let Some(widget) = widgets.pop() {
        if widget.activate_action(name, target).is_ok() {
            return true;
        }
        let mut child = widget.last_child();
        while let Some(c) = child {
            child = c.prev_sibling();
            if c.is_mapped() {
                widgets.push(c);
            }
        }
    }
    false
}

fn shot(widget: &impl IsA<gtk::Widget>, path: &Path) -> Result<(), String> {
    let widget = widget.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(widget.width()), f64::from(widget.height()));
    let node = snapshot.to_node().ok_or("nothing was drawn")?;
    let renderer = widget.native().and_then(|n| n.renderer()).ok_or("the window has no renderer")?;
    let texture: gdk::Texture = renderer.render_texture(node, None);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    texture.save_to_png(path).map_err(|e| e.to_string())?;
    let stride = texture.width() as usize * 4;
    let mut pixels = vec![0; stride * texture.height() as usize];
    texture.download(&mut pixels, stride);
    if one_colour(&pixels) {
        return Err("blank: every pixel is the same".into());
    }
    Ok(())
}

fn one_colour(pixels: &[u8]) -> bool {
    let (pixels, _) = pixels.as_chunks::<4>();
    pixels.first().is_none_or(|first| pixels.iter().all(|p| p == first))
}
