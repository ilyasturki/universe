mod sheet;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene, gsk, pango};

/// The sheet every pad is drawn on, in its own units.
const VIEW: (f32, f32) = (1000.0, 720.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn delta(self) -> (f32, f32) {
        match self {
            Dir::Up => (0.0, -1.0),
            Dir::Down => (0.0, 1.0),
            Dir::Left => (-1.0, 0.0),
            Dir::Right => (1.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Face { x: f32, y: f32, r: f32 },
    Stick { x: f32, y: f32, r: f32, axes: [&'static str; 2], readout_below: bool },
    Arm { cx: f32, cy: f32, l: f32, a: f32, dir: Dir, split: bool },
    Small { x: f32, y: f32, w: f32, h: f32, angle: f32, round: bool },
    Tab { x: f32, y: f32, w: f32, h: f32 },
    Paddle { x: f32, y: f32, w: f32, h: f32 },
    Trigger { path: &'static str, b: [f32; 4] },
    Bumper { path: &'static str, b: [f32; 4] },
}

/// One button on the sheet; a ghost is behind the pad (a back paddle), drawn see-through.
#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub slot: &'static str,
    pub shape: Shape,
    pub ghost: bool,
}

impl Spec {
    /// `[x, y, w, h]` of the button on the sheet.
    fn frame(&self) -> [f32; 4] {
        match self.shape {
            Shape::Face { x, y, r, .. } | Shape::Stick { x, y, r, .. } => [x - r, y - r, r * 2.0, r * 2.0],
            Shape::Arm { cx, cy, l, .. } => [cx - l, cy - l, l * 2.0, l * 2.0],
            Shape::Paddle { x, y, w, h } => [x - w / 2.0, y, w, h],
            Shape::Trigger { b, .. } | Shape::Bumper { b, .. } => [b[0], b[1], b[2] - b[0], b[3] - b[1]],
            Shape::Small { x, y, w, h, .. } | Shape::Tab { x, y, w, h } => {
                let s = w.max(h);
                [x - s / 2.0, y - s / 2.0, s, s]
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Detail {
    Shape { path: &'static str, alpha: f32 },
    Line { path: &'static str, alpha: f32, width: f32, glow: bool },
    Dots { x: f32, y: f32, cols: u32, rows: u32, gap: f32, r: f32 },
    Dish { x: f32, y: f32, r: f32, alpha: f32, ring: bool },
    Cross { x: f32, y: f32, l: f32, a: f32 },
}

#[derive(Debug, Clone)]
pub struct Geometry {
    body: &'static str,
    details: &'static [Detail],
    buttons: Vec<Spec>,
}

/// A family's pad: the Sony ones on the DualSense's body, the Xbox ones on the Xbox's, the Deck on its own, the rest on the
/// generic one.
pub fn geometry(family: &str) -> Geometry {
    let extra = |slot, shape, ghost| Spec { slot, shape, ghost };
    if family == "steam-deck" {
        let mut buttons = sheet::DECK_SHOULDERS.to_vec();
        buttons.extend_from_slice(sheet::DECK_LAYOUT);
        return Geometry { body: sheet::DECK_BODY, details: sheet::DECK_DETAILS, buttons };
    }
    if matches!(family, "dualsense-edge" | "dualsense" | "dualshock4") {
        let mut buttons = sheet::DUALSENSE_SHOULDERS.to_vec();
        buttons.extend(sheet::DUALSENSE_LAYOUT.iter().filter(|s| s.slot != "mute" || family != "dualshock4").copied());
        if family == "dualsense-edge" {
            buttons.push(extra("fn_left", Shape::Tab { x: 444.0, y: 432.0, w: 34.0, h: 15.0 }, false));
            buttons.push(extra("fn_right", Shape::Tab { x: 556.0, y: 432.0, w: 34.0, h: 15.0 }, false));
            buttons.push(extra("paddle_left", Shape::Paddle { x: 150.0, y: 500.0, w: 54.0, h: 96.0 }, true));
            buttons.push(extra("paddle_right", Shape::Paddle { x: 850.0, y: 500.0, w: 54.0, h: 96.0 }, true));
        }
        return Geometry { body: sheet::DUALSENSE_BODY, details: sheet::DUALSENSE_DETAILS, buttons };
    }
    let kind = if matches!(family, "xbox-elite" | "xbox" | "switch-pro" | "8bitdo-pro-3") { family } else { "generic" };
    let xbox = matches!(kind, "xbox" | "xbox-elite");
    let (body, details, shoulders, layout) = if xbox {
        (sheet::XBOX_BODY, sheet::XBOX_DETAILS, sheet::XBOX_SHOULDERS, sheet::XBOX_LAYOUT)
    } else {
        (sheet::GENERIC_BODY, sheet::GENERIC_DETAILS, sheet::GENERIC_SHOULDERS, sheet::GENERIC_LAYOUT)
    };
    let mut buttons = shoulders.to_vec();
    buttons.extend(layout.iter().filter(|s| s.slot != "share" || kind == "xbox").copied());
    match kind {
        "xbox-elite" => {
            buttons.push(extra("paddle_p3", Shape::Paddle { x: 128.0, y: 520.0, w: 52.0, h: 96.0 }, true));
            buttons.push(extra("paddle_p4", Shape::Paddle { x: 196.0, y: 548.0, w: 46.0, h: 70.0 }, true));
            buttons.push(extra("paddle_p1", Shape::Paddle { x: 872.0, y: 520.0, w: 52.0, h: 96.0 }, true));
            buttons.push(extra("paddle_p2", Shape::Paddle { x: 804.0, y: 548.0, w: 46.0, h: 70.0 }, true));
        }
        "switch-pro" => buttons.push(extra("capture", Shape::Small { x: 430.0, y: 318.0, w: 28.0, h: 28.0, angle: 0.0, round: false }, false)),
        "8bitdo-pro-3" => {
            buttons.extend(sheet::PRO3_BUMPERS);
            buttons.push(extra("star", Shape::Small { x: 500.0, y: 300.0, w: 26.0, h: 26.0, angle: 0.0, round: true }, false));
            buttons.push(extra("paddle_pl", Shape::Paddle { x: 150.0, y: 500.0, w: 54.0, h: 96.0 }, true));
            buttons.push(extra("paddle_pr", Shape::Paddle { x: 850.0, y: 500.0, w: 54.0, h: 96.0 }, true));
        }
        _ => {}
    }
    Geometry { body, details, buttons }
}

/// What a button has printed on it: its text, or the mark drawn instead.
fn glyph(family: &str, slot: &str) -> (&'static str, &'static str) {
    let style = match family {
        "dualsense-edge" | "dualsense" | "dualshock4" => "sony",
        "switch-pro" => "nintendo",
        "8bitdo-pro-3" => "eightbitdo",
        "steam-deck" => "deck",
        _ => "xbox",
    };
    let face: Option<(&str, &str)> = match (style, slot) {
        ("sony", "south") => Some(("", "cross")),
        ("sony", "east") => Some(("", "circle")),
        ("sony", "north") => Some(("", "triangle")),
        ("sony", "west") => Some(("", "square")),
        ("sony", "lb") => Some(("L1", "")),
        ("sony", "rb") => Some(("R1", "")),
        ("sony", "lt") => Some(("L2", "")),
        ("sony", "rt") => Some(("R2", "")),
        ("sony", "select") => Some(("", "create")),
        ("sony", "start") => Some(("", "menu")),
        ("sony", "guide") => Some(("PS", "")),
        ("sony", "ls") => Some(("L3", "")),
        ("sony", "rs") => Some(("R3", "")),
        ("xbox" | "deck", "south") => Some(("A", "")),
        ("xbox" | "deck", "east") => Some(("B", "")),
        ("xbox" | "deck", "north") => Some(("Y", "")),
        ("xbox" | "deck", "west") => Some(("X", "")),
        ("xbox" | "deck", "select") => Some(("", "view")),
        ("xbox" | "deck", "start") => Some(("", "menu")),
        ("deck", "lb") => Some(("L1", "")),
        ("deck", "rb") => Some(("R1", "")),
        ("deck", "lt") => Some(("L2", "")),
        ("deck", "rt") => Some(("R2", "")),
        ("deck", "guide") => Some(("", "steam")),
        ("deck", "ls") => Some(("L3", "")),
        ("deck", "rs") => Some(("R3", "")),
        ("xbox", "lb") => Some(("LB", "")),
        ("xbox", "rb") => Some(("RB", "")),
        ("xbox", "lt") => Some(("LT", "")),
        ("xbox", "rt") => Some(("RT", "")),
        ("xbox", "guide") => Some(("", "xbox")),
        ("xbox", "ls") => Some(("LS", "")),
        ("xbox", "rs") => Some(("RS", "")),
        ("nintendo" | "eightbitdo", "south") => Some(("B", "")),
        ("nintendo" | "eightbitdo", "east") => Some(("A", "")),
        ("nintendo" | "eightbitdo", "north") => Some(("X", "")),
        ("nintendo" | "eightbitdo", "west") => Some(("Y", "")),
        ("nintendo" | "eightbitdo", "select") => Some(("", "minus")),
        ("nintendo" | "eightbitdo", "start") => Some(("", "plus")),
        ("nintendo" | "eightbitdo", "guide") => Some(("", "home")),
        ("nintendo", "lb") => Some(("L", "")),
        ("nintendo", "rb") => Some(("R", "")),
        ("nintendo", "lt") => Some(("ZL", "")),
        ("nintendo", "rt") => Some(("ZR", "")),
        ("nintendo", "ls") => Some(("LS", "")),
        ("nintendo", "rs") => Some(("RS", "")),
        ("eightbitdo", "lb") => Some(("L1", "")),
        ("eightbitdo", "rb") => Some(("R1", "")),
        ("eightbitdo", "lt") => Some(("L2", "")),
        ("eightbitdo", "rt") => Some(("R2", "")),
        ("eightbitdo", "ls") => Some(("L3", "")),
        ("eightbitdo", "rs") => Some(("R3", "")),
        _ => None,
    };
    let extra = match slot {
        "fn_left" | "fn_right" => ("Fn", ""),
        "paddle_left" => ("LB", ""),
        "paddle_right" => ("RB", ""),
        "paddle_p1" => ("P1", ""),
        "paddle_p2" => ("P2", ""),
        "paddle_p3" => ("P3", ""),
        "paddle_p4" => ("P4", ""),
        "paddle_l4" => ("L4", ""),
        "paddle_r4" => ("R4", ""),
        "paddle_pl" => ("PL", ""),
        "paddle_pr" => ("PR", ""),
        "share" => ("", "share"),
        "capture" => ("", "capture"),
        "mute" => ("", "mic"),
        "star" => ("", "star"),
        "grip_l4" => ("L4", ""),
        "grip_l5" => ("L5", ""),
        "grip_r4" => ("R4", ""),
        "grip_r5" => ("R5", ""),
        "quick" => ("", "more"),
        "pad_left" | "pad_right" => ("", "pad"),
        _ => ("", ""),
    };
    face.unwrap_or(extra)
}

/// The marks printed on buttons, on a unit radius about the origin; stroked but for `FILLED_SYMBOLS`.
fn symbol_path(name: &str) -> Option<&'static str> {
    Some(match name {
        "cross" => "M-1 -1L1 1M1 -1L-1 1",
        "circle" => "M1 0A1 1 0 1 1 -1 0A1 1 0 1 1 1 0",
        "triangle" => "M0 -1.0976L1.12 0.7392L-1.12 0.7392Z",
        "square" => "M-0.94 -0.94H0.94V0.94H-0.94Z",
        "menu" => "M-1 -0.75H1M-1 0H1M-1 0.75H1",
        "create" => "M-1 -0.75H0.2M-1 0H1M-1 0.75H-0.2",
        "view" => "M-0.8 -0.44H0.28V0.52H-0.8ZM-0.28 -0.76H0.8V0.2H-0.28Z",
        "minus" => "M-1 0H1",
        "plus" => "M-1 0H1M0 -1V1",
        "home" => "M-1 0L0 -1L1 0M-0.7 -0.25V0.9H0.7V-0.25",
        "xbox" => "M1.1 0A1.1 1.1 0 1 1 -1.1 0A1.1 1.1 0 1 1 1.1 0M-0.55 -0.55Q0 0 0.55 0.55M0.55 -0.55Q0 0 -0.55 0.55",
        "share" => "M0 0.5V-1M-0.55 -0.45L0 -1L0.55 -0.45M-0.9 0V1H0.9V0",
        "capture" => "M-1 -1H1V1H-1ZM0.4 0A0.4 0.4 0 1 1 -0.4 0A0.4 0.4 0 1 1 0.4 0",
        "mic" => "M-0.35 -0.65A0.35 0.35 0 0 1 0.35 -0.65V-0.05A0.35 0.35 0 0 1 -0.35 -0.05ZM0.6237 0.3178A0.7 0.7 0 0 1 -0.6237 0.3178M0 0.7V1.05",
        "steam" => "M1.1 0A1.1 1.1 0 1 1 -1.1 0A1.1 1.1 0 1 1 1.1 0M0.72 -0.28A0.3 0.3 0 1 1 0.12 -0.28A0.3 0.3 0 1 1 0.72 -0.28M0.2 -0.08L-0.52 0.42M-0.35 0.42A0.17 0.17 0 1 1 -0.69 0.42A0.17 0.17 0 1 1 -0.35 0.42",
        "more" => "M-0.5 0A0.22 0.22 0 1 1 -0.94 0A0.22 0.22 0 1 1 -0.5 0M0.22 0A0.22 0.22 0 1 1 -0.22 0A0.22 0.22 0 1 1 0.22 0M0.94 0A0.22 0.22 0 1 1 0.5 0A0.22 0.22 0 1 1 0.94 0",
        "pad" => "M-0.9 -0.9H0.9V0.9H-0.9ZM-0.3 0H0.3M0 -0.3V0.3",
        _ => return None,
    })
}

const FILLED_SYMBOLS: [&str; 1] = ["more"];

fn star() -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    for k in 0..10 {
        let (r, a) = (if k % 2 == 0 { 1.05 } else { 0.45 }, -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0);
        if k == 0 {
            builder.move_to(a.cos() * r, a.sin() * r);
        } else {
            builder.line_to(a.cos() * r, a.sin() * r);
        }
    }
    builder.close();
    builder.to_path()
}

fn circle(x: f32, y: f32, r: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.add_circle(&graphene::Point::new(x, y), r);
    builder.to_path()
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    let r = r.min(w / 2.0).min(h / 2.0);
    builder.add_rounded_rect(&gsk::RoundedRect::from_rect(graphene::Rect::new(x, y, w, h), r));
    builder.to_path()
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.add_rect(&graphene::Rect::new(x, y, w, h));
    builder.to_path()
}

/// A cross of arm width `a` and half-length `l`.
fn cross(cx: f32, cy: f32, l: f32, a: f32) -> gsk::Path {
    let h = a / 2.0;
    let builder = gsk::PathBuilder::new();
    builder.move_to(cx - h, cy - l);
    for (x, y) in [(h, -l), (h, -h), (l, -h), (l, h), (h, h), (h, l), (-h, l), (-h, h), (-l, h), (-l, -h), (-h, -h)] {
        builder.line_to(cx + x, cy + y);
    }
    builder.close();
    builder.to_path()
}

fn paddle(cx: f32, y: f32, w: f32, h: f32) -> gsk::Path {
    let builder = gsk::PathBuilder::new();
    builder.move_to(cx - w * 0.32, y);
    builder.line_to(cx + w * 0.32, y);
    builder.cubic_to(cx + w * 0.5, y + h * 0.25, cx + w * 0.5, y + h * 0.75, cx + w * 0.34, y + h);
    builder.line_to(cx - w * 0.34, y + h);
    builder.cubic_to(cx - w * 0.5, y + h * 0.75, cx - w * 0.5, y + h * 0.25, cx - w * 0.32, y);
    builder.close();
    builder.to_path()
}

fn stroke(width: f32) -> gsk::Stroke {
    let stroke = gsk::Stroke::new(width);
    stroke.set_line_cap(gsk::LineCap::Round);
    stroke.set_line_join(gsk::LineJoin::Round);
    stroke
}

fn mix(a: gdk::RGBA, b: gdk::RGBA, t: f32) -> gdk::RGBA {
    let at = |x: f32, y: f32| x + (y - x) * t;
    gdk::RGBA::new(at(a.red(), b.red()), at(a.green(), b.green()), at(a.blue(), b.blue()), at(a.alpha(), b.alpha()))
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct PadArt {
        pub family: RefCell<String>,
        pub geometry: RefCell<Option<Geometry>>,
        /// The sheet's paths parsed once, by the address of their text.
        pub paths: RefCell<HashMap<usize, gsk::Path>>,
        pub pressed: RefCell<HashSet<String>>,
        pub axes: RefCell<HashMap<String, f32>>,
        pub unbound: RefCell<HashSet<String>>,
        pub learning: RefCell<String>,
        pub readouts: Cell<bool>,
        pub pulse: Cell<f32>,
        pub tick: RefCell<Option<gtk::TickCallbackId>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PadArt {
        const NAME: &'static str = "UniversePadArt";
        type Type = super::PadArt;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PadArt {
        fn constructed(&self) {
            self.parent_constructed();
            let art = self.obj().downgrade();
            adw::StyleManager::default().connect_dark_notify(move |_| {
                art.upgrade().inspect(|art| art.queue_draw());
            });
        }
    }

    impl WidgetImpl for PadArt {
        // A height that follows the width breaks a scrolled page's measure; the drawing scales into what it gets.
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match orientation {
                gtk::Orientation::Horizontal => (200, 560, -1, -1),
                _ => (400, 400, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            self.obj().draw(snapshot);
        }
    }
}

glib::wrapper! {
    /// A drawing of the connected pad: its buttons lit as they are pressed, the sticks leaning and the triggers filling.
    pub struct PadArt(ObjectSubclass<imp::PadArt>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for PadArt {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl PadArt {
    pub fn set_family(&self, family: &str) {
        let imp = self.imp();
        if *imp.family.borrow() == family {
            return;
        }
        imp.family.replace(family.to_string());
        imp.geometry.replace(Some(geometry(family)));
        self.clear_input();
    }

    pub fn set_pressed(&self, slot: &str, down: bool) {
        let changed = if down { self.imp().pressed.borrow_mut().insert(slot.to_string()) } else { self.imp().pressed.borrow_mut().remove(slot) };
        if changed {
            self.queue_draw();
        }
    }

    pub fn set_axis(&self, role: &str, value: f32) {
        self.imp().axes.borrow_mut().insert(role.to_string(), value);
        self.queue_draw();
    }

    pub fn clear_input(&self) {
        self.imp().pressed.borrow_mut().clear();
        self.imp().axes.borrow_mut().clear();
        self.queue_draw();
    }

    pub fn set_unbound(&self, slots: impl IntoIterator<Item = String>) {
        self.imp().unbound.replace(slots.into_iter().collect());
        self.queue_draw();
    }

    pub fn set_readouts(&self, on: bool) {
        self.imp().readouts.set(on);
        self.queue_draw();
    }

    /// The slot waiting for a press pulses until another is set, or none.
    pub fn set_learning(&self, slot: &str) {
        let imp = self.imp();
        imp.learning.replace(slot.to_string());
        if slot.is_empty() {
            if let Some(tick) = imp.tick.take() {
                tick.remove();
            }
        } else if imp.tick.borrow().is_none() {
            let tick = self.add_tick_callback(|art, clock| {
                let t = clock.frame_time() as f64 / 1_000_000.0;
                art.imp().pulse.set((0.3 + 0.2 * (t * std::f64::consts::TAU / 1.2).sin()) as f32);
                art.queue_draw();
                glib::ControlFlow::Continue
            });
            imp.tick.replace(Some(tick));
        }
        self.queue_draw();
    }

    fn path(&self, text: &'static str) -> Option<gsk::Path> {
        let key = text.as_ptr() as usize;
        if let Some(path) = self.imp().paths.borrow().get(&key) {
            return Some(path.clone());
        }
        let path = gsk::Path::parse(text).map_err(|e| tracing::warn!("pad path: {e}")).ok()?;
        self.imp().paths.borrow_mut().insert(key, path.clone());
        Some(path)
    }

    fn draw(&self, snapshot: &gtk::Snapshot) {
        let Some(geo) = self.imp().geometry.borrow().clone() else { return };
        let (w, h) = (self.width() as f32, self.height() as f32);
        let k = (w / VIEW.0).min(h / VIEW.1);
        if k <= 0.01 {
            return;
        }
        let fg = self.color();
        let ink = |a: f32| gdk::RGBA::new(fg.red(), fg.green(), fg.blue(), a);
        let ground = if adw::StyleManager::default().is_dark() { gdk::RGBA::new(0.133, 0.133, 0.149, 1.0) } else { gdk::RGBA::new(0.98, 0.98, 0.98, 1.0) };
        snapshot.save();
        snapshot.translate(&graphene::Point::new((w - VIEW.0 * k) / 2.0, (h - VIEW.1 * k) / 2.0));
        snapshot.scale(k, k);

        let Some(body) = self.path(geo.body) else {
            snapshot.restore();
            return;
        };
        let bounds = body.bounds().unwrap_or_else(|| graphene::Rect::new(0.0, 0.0, VIEW.0, VIEW.1));
        let (top, bottom) = (bounds.y(), bounds.y() + bounds.height());
        snapshot.push_fill(&body, gsk::FillRule::Winding);
        snapshot.append_linear_gradient(
            &bounds,
            &graphene::Point::new(0.0, top),
            &graphene::Point::new(0.0, bottom),
            &[gsk::ColorStop::new(0.0, ink(0.20)), gsk::ColorStop::new(0.5, ink(0.13)), gsk::ColorStop::new(1.0, ink(0.08))],
        );
        snapshot.append_linear_gradient(
            &bounds,
            &graphene::Point::new(0.0, top),
            &graphene::Point::new(0.0, bottom),
            &[gsk::ColorStop::new(0.5, gdk::RGBA::new(0.0, 0.0, 0.0, 0.0)), gsk::ColorStop::new(1.0, gdk::RGBA::new(0.0, 0.0, 0.0, 0.25))],
        );
        snapshot.pop();
        let outline = |snapshot: &gtk::Snapshot| snapshot.append_stroke(&body, &stroke(2.0 / k), &ink(0.30));
        outline(snapshot);

        for detail in geo.details {
            match *detail {
                Detail::Shape { path, alpha } => {
                    if let Some(p) = self.path(path) {
                        snapshot.append_fill(&p, gsk::FillRule::Winding, &ink(alpha));
                        snapshot.append_stroke(&p, &stroke(1.6 / k), &ink(0.2));
                    }
                }
                Detail::Line { path, alpha, width, glow } => {
                    if let Some(p) = self.path(path) {
                        if glow {
                            snapshot.append_stroke(&p, &stroke(6.0 / k), &ink(0.12));
                        }
                        snapshot.append_stroke(&p, &stroke(width / k), &ink(alpha));
                    }
                }
                Detail::Dots { x, y, cols, rows, gap, r } => {
                    for row in 0..rows {
                        for col in 0..cols {
                            let dx = (col as f32 - (cols - 1) as f32 / 2.0) * gap;
                            let dy = (row as f32 - (rows - 1) as f32 / 2.0) * gap;
                            snapshot.append_fill(&circle(x + dx, y + dy, r), gsk::FillRule::Winding, &ink(0.3));
                        }
                    }
                }
                Detail::Dish { x, y, r, alpha, ring } => {
                    let p = circle(x, y, r);
                    snapshot.append_fill(&p, gsk::FillRule::Winding, &ink(alpha));
                    snapshot.append_stroke(&p, &stroke(if ring { 1.6 } else { 1.4 } / k), &ink(if ring { 0.3 } else { 0.14 }));
                }
                Detail::Cross { x, y, l, a } => {
                    let p = cross(x, y, l, a);
                    snapshot.append_fill(&p, gsk::FillRule::Winding, &ink(0.14));
                    snapshot.append_stroke(&p, &stroke(1.6 / k), &ink(0.36));
                }
            }
        }
        for spec in &geo.buttons {
            if let Shape::Stick { x, y, r, .. } = spec.shape {
                snapshot.append_fill(&circle(x, y, r + 16.0), gsk::FillRule::Winding, &gdk::RGBA::new(0.0, 0.0, 0.0, 0.22));
            }
        }
        let family = self.imp().family.borrow().clone();
        for spec in &geo.buttons {
            self.button(snapshot, spec, &family, &body, k, fg, ground);
        }
        outline(snapshot);
        snapshot.restore();
    }

    #[allow(clippy::too_many_arguments)]
    fn button(&self, snapshot: &gtk::Snapshot, spec: &Spec, family: &str, body: &gsk::Path, k: f32, fg: gdk::RGBA, ground: gdk::RGBA) {
        let imp = self.imp();
        let ink = |a: f32| gdk::RGBA::new(fg.red(), fg.green(), fg.blue(), a);
        let down = imp.pressed.borrow().contains(spec.slot);
        let learning = *imp.learning.borrow() == spec.slot;
        let missing = imp.unbound.borrow().contains(spec.slot);
        let axes = imp.axes.borrow();
        let g = if down { 1.0 } else { 0.0 };
        let pulse = imp.pulse.get();
        let base = if spec.ghost { 0.10 } else { 0.14 };
        let fill_a = if missing {
            0.05
        } else if learning {
            pulse
        } else {
            base
        };
        let stroke_a = if learning {
            0.95
        } else if spec.ghost {
            0.5
        } else {
            0.40
        };
        let fill = mix(ink(fill_a), ink(1.0), g);
        let edge = mix(ink(stroke_a), ink(1.0), g);
        let line = stroke(1.6 / k);
        if missing && !down {
            line.set_dash(&[4.0 / k, 4.0 / k]);
        }
        let paint = |path: &gsk::Path| {
            if !spec.ghost {
                snapshot.append_fill(path, gsk::FillRule::Winding, &ground);
            }
            snapshot.append_fill(path, gsk::FillRule::Winding, &fill);
            snapshot.append_stroke(path, &line, &edge);
        };
        let frame = spec.frame();
        let (mut cx, mut cy) = (frame[0] + frame[2] / 2.0, frame[1] + frame[3] / 2.0);
        let (text, symbol) = glyph(family, spec.slot);
        let mut mark = mix(ink(0.9), ground, g);
        let mut symbol_radius = 0.0;
        let mut text_size = 0.0;
        match spec.shape {
            Shape::Face { x, y, r, .. } => {
                paint(&circle(x, y, r));
                symbol_radius = r;
                text_size = r * 1.05;
            }
            Shape::Stick { x, y, r, axes: [ax, ay], readout_below } => {
                let (lx, ly) = (axes.get(ax).copied().unwrap_or(0.0), axes.get(ay).copied().unwrap_or(0.0));
                (cx, cy) = (x + lx * r * 0.45, y + ly * r * 0.45);
                paint(&circle(cx, cy, r));
                text_size = r * 0.42;
                if imp.readouts.get() {
                    let sign = |v: f32| if v >= 0.0 { "+" } else { "−" };
                    let lines = format!("x {}{} %\ny {}{} %", sign(lx), (lx.abs() * 100.0).round(), sign(ly), (ly.abs() * 100.0).round());
                    self.readout(snapshot, &lines, x, y, r, x < VIEW.0 / 2.0, fg, readout_below);
                }
            }
            Shape::Arm { cx: ax, cy: ay, l, a, dir, split } => {
                let (dx, dy) = dir.delta();
                if split {
                    let gap = a * 0.28;
                    let len = l - gap - a * 0.5;
                    let (bx, by) = (ax + dx * (gap + a * 0.5 + len / 2.0), ay + dy * (gap + a * 0.5 + len / 2.0));
                    let (bw, bh) = (if dx != 0.0 { len } else { a }, if dy != 0.0 { len } else { a });
                    paint(&rounded(bx - bw / 2.0, by - bh / 2.0, bw, bh, a * 0.22));
                } else if down || learning || missing {
                    let h = a / 2.0;
                    let (x, y) = (ax + dx * (h + l) / 2.0, ay + dy * (h + l) / 2.0);
                    let (w2, h2) = (if dx != 0.0 { l - h } else { a }, if dy != 0.0 { l - h } else { a });
                    let arm = rect(x - w2 / 2.0, y - h2 / 2.0, w2, h2);
                    let tint = if down {
                        ink(1.0)
                    } else if learning {
                        ink(pulse)
                    } else {
                        ink(0.05)
                    };
                    snapshot.append_fill(&arm, gsk::FillRule::Winding, &tint);
                    if missing {
                        snapshot.append_stroke(&arm, &line, &edge);
                    }
                }
                if !split && !down {
                    mark = ink(0.9);
                }
                let (tx, ty, v) = (ax + dx * l * 0.72, ay + dy * l * 0.72, a * 0.16);
                let chevron = gsk::PathBuilder::new();
                chevron.move_to(tx - dx * v + dy * v * 1.4, ty - dy * v + dx * v * 1.4);
                chevron.line_to(tx + dx * v * 0.6, ty + dy * v * 0.6);
                chevron.line_to(tx - dx * v - dy * v * 1.4, ty - dy * v - dx * v * 1.4);
                snapshot.append_stroke(&chevron.to_path(), &stroke(1.8 / k), &mark);
            }
            Shape::Trigger { path, b } => {
                let Some(p) = self.path(path) else { return };
                let pull = axes.get(spec.slot).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                snapshot.append_fill(&p, gsk::FillRule::Winding, &ground);
                snapshot.append_fill(&p, gsk::FillRule::Winding, &ink(fill_a));
                if pull > 0.005 {
                    let height = b[3] - b[1];
                    snapshot.push_fill(&p, gsk::FillRule::Winding);
                    snapshot.append_color(&ink(0.92), &graphene::Rect::new(b[0], b[3] - height * pull, b[2] - b[0], height * pull + 2.0));
                    snapshot.pop();
                }
                let width = if down { 2.5 } else { 1.6 };
                let outline = stroke(width / k);
                if missing && !down {
                    outline.set_dash(&[4.0 / k, 4.0 / k]);
                }
                snapshot.append_stroke(&p, &outline, &edge);
                cy = b[1] + 15.0;
                text_size = 18.0;
                if down || pull >= 0.5 {
                    mark = ground;
                }
                if imp.readouts.get() {
                    self.readout(snapshot, &format!("{} %", (pull * 100.0).round()), cx, cy, (b[2] - b[0]) / 2.0, cx < VIEW.0 / 2.0, fg, false);
                }
            }
            Shape::Bumper { path, .. } => {
                let Some(p) = self.path(path) else { return };
                snapshot.push_fill(body, gsk::FillRule::Winding);
                let tint = mix(
                    ink(if missing {
                        0.05
                    } else if learning {
                        pulse
                    } else {
                        0.12
                    }),
                    ink(1.0),
                    g,
                );
                snapshot.append_fill(&p, gsk::FillRule::Winding, &tint);
                snapshot.append_stroke(&p, &line, &edge);
                snapshot.pop();
                text_size = 16.0;
            }
            Shape::Paddle { x, y, w, h } => {
                paint(&paddle(x, y, w, h));
                text_size = w * 0.42;
            }
            Shape::Small { x, y, w, h, angle, round } => {
                let radius = if round { w.min(h) / 2.0 } else { w.min(h) * 0.3 };
                snapshot.save();
                if angle != 0.0 {
                    snapshot.translate(&graphene::Point::new(x, y));
                    snapshot.rotate(angle);
                    snapshot.translate(&graphene::Point::new(-x, -y));
                }
                paint(&rounded(x - w / 2.0, y - h / 2.0, w, h, radius));
                snapshot.restore();
                symbol_radius = if angle != 0.0 { 0.0 } else { w.min(h) / 2.0 };
                text_size = w.min(h) * if text.chars().count() > 1 { 0.62 } else { 0.8 };
            }
            Shape::Tab { x, y, w, h } => {
                paint(&rounded(x - w / 2.0, y - h / 2.0, w, h, w.min(h) * 0.3));
                text_size = w.min(h) * if text.chars().count() > 1 { 0.62 } else { 0.8 };
            }
        }
        if !symbol.is_empty() {
            if symbol_radius == 0.0 && !matches!(spec.shape, Shape::Small { .. }) {
                symbol_radius = frame[2].min(frame[3]) / 2.0;
            }
            if symbol_radius > 0.0 {
                self.symbol(snapshot, symbol, cx, cy, symbol_radius * 0.56, k, mark);
            }
        } else if !text.is_empty() && !matches!(spec.shape, Shape::Arm { .. }) && text_size > 0.0 {
            self.text(snapshot, text, cx, cy, text_size, mark);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn symbol(&self, snapshot: &gtk::Snapshot, name: &str, cx: f32, cy: f32, scale: f32, k: f32, color: gdk::RGBA) {
        let path = if name == "star" { Some(star()) } else { symbol_path(name).and_then(|text| self.path(text)) };
        let Some(path) = path else { return };
        snapshot.save();
        snapshot.translate(&graphene::Point::new(cx, cy));
        snapshot.scale(scale, scale);
        if FILLED_SYMBOLS.contains(&name) {
            snapshot.append_fill(&path, gsk::FillRule::Winding, &color);
        } else {
            snapshot.append_stroke(&path, &stroke(1.8 / k / scale), &color);
        }
        snapshot.restore();
    }

    fn text(&self, snapshot: &gtk::Snapshot, text: &str, cx: f32, cy: f32, size: f32, color: gdk::RGBA) {
        let layout = self.create_pango_layout(Some(text));
        let mut font = pango::FontDescription::new();
        font.set_weight(pango::Weight::Semibold);
        font.set_absolute_size(f64::from(size) * f64::from(pango::SCALE));
        layout.set_font_description(Some(&font));
        let (w, h) = layout.pixel_size();
        snapshot.save();
        snapshot.translate(&graphene::Point::new(cx - w as f32 / 2.0, cy - h as f32 / 2.0));
        snapshot.append_layout(&layout, &color);
        snapshot.restore();
    }

    /// Numbers beside a stick or a trigger, on the side away from the pad's middle, or under it.
    #[allow(clippy::too_many_arguments)]
    fn readout(&self, snapshot: &gtk::Snapshot, text: &str, cx: f32, cy: f32, reach: f32, left: bool, color: gdk::RGBA, below: bool) {
        let layout = self.create_pango_layout(Some(text));
        let mut font = pango::FontDescription::new();
        font.set_weight(pango::Weight::Semibold);
        font.set_absolute_size(22.0 * f64::from(pango::SCALE));
        layout.set_font_description(Some(&font));
        layout.set_alignment(if below {
            pango::Alignment::Center
        } else if left {
            pango::Alignment::Right
        } else {
            pango::Alignment::Left
        });
        let (w, h) = layout.pixel_size();
        let at = if below {
            graphene::Point::new(cx - w as f32 / 2.0, cy + reach + 4.0)
        } else {
            graphene::Point::new(if left { cx - reach - 24.0 - w as f32 } else { cx + reach + 24.0 }, cy - h as f32 / 2.0)
        };
        snapshot.save();
        snapshot.translate(&at);
        snapshot.append_layout(&layout, &color);
        snapshot.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sheet_path_and_mark_parses() {
        let mut texts: Vec<&str> = vec![sheet::DUALSENSE_BODY, sheet::XBOX_BODY, sheet::DECK_BODY, sheet::GENERIC_BODY];
        for details in [sheet::DUALSENSE_DETAILS, sheet::XBOX_DETAILS, sheet::DECK_DETAILS, sheet::GENERIC_DETAILS] {
            for d in details {
                if let Detail::Shape { path, .. } | Detail::Line { path, .. } = d {
                    texts.push(*path);
                }
            }
        }
        for family in ["steam-deck", "dualsense-edge", "dualsense", "dualshock4", "xbox", "xbox-elite", "switch-pro", "8bitdo-pro-3", "generic"] {
            for spec in geometry(family).buttons {
                if let Shape::Trigger { path, .. } | Shape::Bumper { path, .. } = spec.shape {
                    texts.push(path);
                }
                let (_, symbol) = glyph(family, spec.slot);
                if let Some(path) = symbol_path(symbol) {
                    texts.push(path);
                }
            }
        }
        for text in texts {
            assert!(gsk::Path::parse(text).is_ok(), "{}", &text[..text.len().min(60)]);
        }
    }

    #[test]
    fn families_get_their_extra_buttons() {
        let slots = |family: &str| geometry(family).buttons.iter().map(|s| s.slot).collect::<Vec<_>>();
        assert!(slots("dualsense-edge").contains(&"paddle_left") && slots("dualsense-edge").contains(&"mute"));
        assert!(!slots("dualshock4").contains(&"mute"));
        assert!(slots("xbox-elite").contains(&"paddle_p4") && !slots("xbox-elite").contains(&"share"));
        assert!(slots("xbox").contains(&"share"));
        assert!(slots("8bitdo-pro-3").contains(&"paddle_l4") && slots("8bitdo-pro-3").contains(&"star"));
        assert_eq!(glyph("switch-pro", "east"), ("A", ""));
        assert_eq!(glyph("dualsense", "south"), ("", "cross"));
        let deck = slots("steam-deck");
        assert!(["grip_l5", "grip_r4", "quick", "pad_left", "pad_right"].iter().all(|s| deck.contains(s)), "{deck:?}");
        assert_eq!((glyph("steam-deck", "guide"), glyph("steam-deck", "lb"), glyph("steam-deck", "quick")), (("", "steam"), ("L1", ""), ("", "more")));
        assert!(
            geometry("steam-deck").buttons.iter().any(|s| matches!(s.shape, Shape::Stick { readout_below: true, .. })),
            "the Deck's numbers go under its sticks"
        );
    }
}
