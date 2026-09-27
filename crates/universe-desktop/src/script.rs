use std::path::{Path, PathBuf};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

/// `UNIVERSE_DESKTOP_SCRIPT`: steps run once the library is up, then the app quits; `~` in an action stands for a
/// space. The app runs apart from a running one then, adopts no scope and sweeps no journal.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Wait(Duration),
    Shot(PathBuf),
    Action(String),
    Size(i32, i32),
}

pub fn from_env() -> Option<Vec<Step>> {
    let text = std::env::var("UNIVERSE_DESKTOP_SCRIPT").ok()?;
    Some(parse(&text))
}

fn parse(text: &str) -> Vec<Step> {
    text.split_whitespace()
        .filter_map(|word| {
            let (kind, arg) = word.split_once(':')?;
            match kind {
                "wait" => arg.parse().ok().map(|ms| Step::Wait(Duration::from_millis(ms))),
                "shot" => Some(Step::Shot(arg.into())),
                "action" => Some(Step::Action(arg.replace('~', " "))),
                "size" => arg.split_once('x').and_then(|(w, h)| Some(Step::Size(w.parse().ok()?, h.parse().ok()?))),
                _ => None,
            }
        })
        .collect()
}

pub async fn run(window: gtk::Window, steps: Vec<Step>) {
    for step in steps {
        match step {
            Step::Wait(d) => glib::timeout_future(d).await,
            Step::Size(w, h) => window.set_default_size(w, h),
            Step::Action(detailed) => match gio::Action::parse_detailed_name(&detailed) {
                Ok((name, target)) => {
                    if window.activate_action(&name, target.as_ref()).is_err() {
                        tracing::warn!("script: no action {name}");
                    }
                }
                Err(e) => tracing::warn!("script: {detailed}: {e}"),
            },
            Step::Shot(path) => match shot(&window, &path) {
                Ok(()) => println!("shot: {}", path.display()),
                Err(e) => tracing::warn!("script: shot {}: {e}", path.display()),
            },
        }
    }
    window.application().inspect(|app| app.quit());
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
    texture.save_to_png(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_reads_its_steps_and_skips_what_it_cannot() {
        let steps = parse("size:1280x800 wait:300 action:win.view::platform:Nintendo~Switch shot:/tmp/a.png bogus nope:1 wait:x");
        assert_eq!(
            steps,
            [
                Step::Size(1280, 800),
                Step::Wait(Duration::from_millis(300)),
                Step::Action("win.view::platform:Nintendo Switch".into()),
                Step::Shot("/tmp/a.png".into())
            ]
        );
    }
}
