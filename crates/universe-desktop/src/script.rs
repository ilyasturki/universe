use std::path::{Path, PathBuf};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

// An AdwDialog presented during one frame is drawn from the next.
const SETTLE_FRAMES: u32 = 2;
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);

/// `UNIVERSE_DESKTOP_SCRIPT`: steps run once the library is up, animations off so each lands in a frame, then the app
/// quits, with status 1 when a step failed; `~` in an action stands for a space. The app runs apart from a running one
/// then, adopts no scope and sweeps no journal.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(Duration),
    Shot(PathBuf),
    Action(String),
    Size(i32, i32),
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
        _ => None,
    }
}

pub async fn run(window: gtk::Window, steps: Vec<Step>) {
    window.settings().set_gtk_enable_animations(false);
    let mut failed = false;
    if let Some(failure) = window.application().and_downcast::<crate::app::Application>().and_then(|app| app.failure()) {
        eprintln!("script: the core did not open: {failure}");
        failed = true;
    }
    for step in steps {
        if let Err(e) = run_step(&window, step).await {
            eprintln!("script: {e}");
            failed = true;
        }
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
        Step::Shot(path) => {
            drawn(window).await?;
            shot(window, &path).map_err(|e| format!("shot {}: {e}", path.display()))?;
            println!("shot: {}", path.display());
        }
        Step::Unreadable(word) => return Err(format!("no step reads {word:?}")),
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_reads_its_steps_and_keeps_what_it_cannot_read_to_fail_on() {
        let steps = parse("size:1280x800 wait:300 action:win.view::platform:Nintendo~Switch shot:/tmp/a.png bogus nope:1 wait:x");
        assert_eq!(
            steps,
            [
                Step::Size(1280, 800),
                Step::Wait(Duration::from_millis(300)),
                Step::Action("win.view::platform:Nintendo Switch".into()),
                Step::Shot("/tmp/a.png".into()),
                Step::Unreadable("bogus".into()),
                Step::Unreadable("nope:1".into()),
                Step::Unreadable("wait:x".into()),
            ]
        );
    }

    #[test]
    fn a_shot_of_a_single_colour_is_blank() {
        assert!(one_colour(&[]));
        assert!(one_colour(&[20, 20, 20, 255, 20, 20, 20, 255]));
        assert!(!one_colour(&[20, 20, 20, 255, 20, 20, 21, 255]));
    }
}
