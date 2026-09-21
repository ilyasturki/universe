use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use evdev::{Device, EventSummary, EventType, InputEvent, KeyCode};
use tokio::io::unix::AsyncFd;
use tokio::io::AsyncBufReadExt;
use tokio::sync::{mpsc, Mutex};

use super::engine::{Binding, Engine, Fire};
use super::keys::{self, Source};
use super::{axis_roles, detect_family, resolve_slots, sdl_axis, sdl_element, ControllerConfig, Family};
use crate::core::Core;
use crate::paths;

const BTN_GAMEPAD: u16 = 0x130;
const SCAN_EVERY: Duration = Duration::from_secs(2);
const COMBO_HOLD: Duration = Duration::from_millis(40);
const LEARN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Default)]
pub struct WatchOptions {
    pub json: bool,
    pub wait: bool,
}

#[derive(Clone, Copy)]
struct Out {
    json: bool,
}

impl Out {
    /// A closed stdout means the launcher is gone; the watcher goes with it.
    fn emit(&self, v: serde_json::Value) -> bool {
        if self.json {
            let mut o = std::io::stdout().lock();
            writeln!(o, "{v}").and_then(|_| o.flush()).is_ok()
        } else {
            tracing::info!("{v}");
            true
        }
    }
}

pub fn lock_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(paths::state_home)
        .join("universe")
        .join("controller.lock")
}

async fn take_lock(wait: bool, out: &Out) -> crate::Result<Option<std::fs::File>> {
    let path = lock_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path)?;
    match file.try_lock() {
        Ok(()) => return Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => {}
        Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
    }
    if !wait {
        return Ok(None);
    }
    out.emit(serde_json::json!({"event": "waiting"}));
    let file = tokio::task::spawn_blocking(move || file.lock().map(|()| file)).await.map_err(|e| crate::Error::Io(e.to_string()))??;
    Ok(Some(file))
}

enum PadCmd {
    Rumble,
    Close,
}

#[derive(Clone, PartialEq, Eq)]
enum Learn {
    Slot(String),
    Axis(String),
}

enum DevEvent {
    Input(String, InputEvent),
    Battery(String, u8, bool),
    Gone(String),
}

struct Pad {
    id: String,
    path: PathBuf,
    name: String,
    family: &'static Family,
    bus: String,
    vendor: u16,
    product: u16,
    keys: Vec<u16>,
    axes: Vec<u16>,
    ranges: BTreeMap<u16, (i32, i32)>,
    roles: BTreeMap<u16, (&'static str, bool)>,
    slots: BTreeMap<String, Option<Source>>,
    by_source: BTreeMap<Source, String>,
    bindings: BTreeMap<String, Binding>,
    axis_down: BTreeSet<(u16, bool)>,
    axis_last: BTreeMap<&'static str, i32>,
    battery: Option<(u8, bool)>,
    cmd: mpsc::Sender<PadCmd>,
}

const EIGHTBITDO: u16 = 0x2dc8;

/// An 8BitDo's charge, from its own HID report (the kernel keeps no power supply for it): byte 14 is the percent under a charging bit.
fn battery_of(vendor: u16, report: &[u8]) -> Option<(u8, bool)> {
    if vendor != EIGHTBITDO || report.len() < 15 || !matches!(report[0], 1 | 3 | 4) {
        return None;
    }
    let level = (report[14] & 0x7f).min(100);
    Some((level, report[14] & 0x80 != 0 || level == 100))
}

/// The hidraw node of the HID device an evdev node hangs off, opened for reading without blocking; none for a pad without one or one out of reach.
fn hidraw_of(path: &Path, vendor: u16) -> Option<AsyncFd<std::fs::File>> {
    if vendor != EIGHTBITDO {
        return None;
    }
    let sysfs = Path::new("/sys/class/input").join(path.file_name()?).join("device/device/hidraw");
    let node = std::fs::read_dir(sysfs).ok()?.flatten().next()?.file_name();
    let fd = rustix::fs::open(Path::new("/dev").join(node), rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK, rustix::fs::Mode::empty()).ok()?;
    AsyncFd::new(std::fs::File::from(fd)).ok()
}

async fn next_report(hid: &mut Option<AsyncFd<std::fs::File>>) -> Vec<u8> {
    loop {
        let Some(fd) = hid.as_mut() else {
            std::future::pending::<()>().await;
            unreachable!()
        };
        let Ok(mut guard) = fd.readable_mut().await else {
            *hid = None;
            continue;
        };
        let mut buf = [0u8; 64];
        match guard.try_io(|inner| rustix::io::read(inner.get_ref(), &mut buf).map_err(std::io::Error::from)) {
            Ok(Ok(n)) => return buf[..n].to_vec(),
            Ok(Err(_)) => *hid = None,
            Err(_) => {}
        }
    }
}

fn axis_value(name: &str, value: i32, (min, max): (i32, i32)) -> f64 {
    let span = f64::from(max) - f64::from(min);
    if span <= 0.0 {
        return 0.0;
    }
    let v = if name == "lt" || name == "rt" {
        (f64::from(value) - f64::from(min)) / span
    } else {
        (f64::from(value) - (f64::from(min) + f64::from(max)) / 2.0) / (span / 2.0)
    };
    v.clamp(-1.0, 1.0)
}

/// A stick past 40% of its throw from rest, a trigger past 40% of its pull: what a learn takes as meant.
const LEARN_THROW: f64 = 0.4;

impl Pad {
    fn resolve(&mut self, cfg: &ControllerConfig) {
        self.slots = resolve_slots(cfg, self.family, &self.keys, &self.axes);
        self.roles = axis_roles(cfg, self.family, &self.axes);
        self.by_source = self.slots.iter().filter_map(|(s, src)| src.map(|src| (src, s.clone()))).collect();
        self.bindings = self.slots.keys().map(|s| (s.clone(), Binding::of(&cfg.macros_for(self.family.id, s)))).collect();
    }

    /// The axis as the page reads it: -1..1 for a stick, 0..1 for a trigger, a backwards axis turned round.
    fn axis_read(&self, code: u16, value: i32) -> Option<(&'static str, f64)> {
        let (name, backwards) = *self.roles.get(&code)?;
        let range = self.ranges.get(&code).copied().unwrap_or((-1, 1));
        let v = axis_value(name, value, range);
        Some((name, if backwards && !matches!(name, "lt" | "rt") { -v } else { v }))
    }

    fn axis_sample(&mut self, code: u16, value: i32) -> Option<serde_json::Value> {
        let (name, v) = self.axis_read(code, value)?;
        let q = (v * 100.0).round() as i32;
        (self.axis_last.insert(name, q) != Some(q)).then(|| serde_json::json!({"event": "axis", "id": self.id, "axis": name, "value": f64::from(q) / 100.0}))
    }

    /// An axis thrown past `LEARN_THROW` off its rest, as a learn names it: `ABS_Z` thrown the way asked, `ABS_Z-` the other way.
    fn axis_thrown(&self, code: u16, value: i32) -> Option<String> {
        if (16..=23).contains(&code) {
            return None;
        }
        let (min, max) = self.ranges.get(&code).copied().unwrap_or((-1, 1));
        let trigger_like = min >= 0 && self.roles.get(&code).is_some_and(|(r, _)| matches!(*r, "lt" | "rt"));
        let v = axis_value(if trigger_like { "lt" } else { "lx" }, value, (min, max));
        (v.abs() > LEARN_THROW).then(|| format!("{:?}{}", evdev::AbsoluteAxisCode(code), if v < 0.0 { "-" } else { "" }))
    }

    fn json(&self) -> serde_json::Value {
        let axes: serde_json::Map<String, serde_json::Value> = self
            .roles
            .iter()
            .map(|(code, (role, backwards))| {
                ((*role).to_string(), serde_json::json!(format!("{:?}{}", evdev::AbsoluteAxisCode(*code), if *backwards { "-" } else { "" })))
            })
            .collect();
        let sdl: serde_json::Map<String, serde_json::Value> =
            self.slots.iter().filter_map(|(s, src)| src.map(|src| (s.clone(), serde_json::json!(sdl_element(&src, &self.keys, &self.axes))))).collect();
        let sdl_axes: serde_json::Map<String, serde_json::Value> = self
            .roles
            .iter()
            .filter_map(|(code, (role, backwards))| {
                sdl_axis(*code, &self.axes).map(|a| ((*role).to_string(), serde_json::json!(format!("{a}{}", if *backwards { "~" } else { "" }))))
            })
            .collect();
        serde_json::json!({
            "event": "device", "id": self.id, "name": self.name, "family": self.family.id, "family_name": self.family.name, "bus": self.bus,
            "vendor": self.vendor, "product": self.product, "slots": slots_json(&self.slots), "axes": axes, "sdl": sdl, "sdl_axes": sdl_axes,
            "battery": self.battery.map(|(percent, charging)| serde_json::json!({"percent": percent, "charging": charging})),
        })
    }

    fn axis(&mut self, code: u16, value: i32) -> Vec<(Source, bool)> {
        let (min, max) = self.ranges.get(&code).copied().unwrap_or((-1, 1));
        let positive = if min < 0 { value > 0 } else { value > (min + max) / 2 };
        let negative = min < 0 && value < 0;
        let mut out = vec![];
        for (dir, down) in [(true, positive), (false, negative)] {
            let key = (code, dir);
            if down != self.axis_down.contains(&key) {
                if down {
                    self.axis_down.insert(key);
                } else {
                    self.axis_down.remove(&key);
                }
                out.push((Source::Axis { code, positive: dir }, down));
            }
        }
        out
    }
}

const BTN_JOYSTICK: u16 = 0x120;

/// A joystick-mapped device counts only from a known pad maker (a D-input 8BitDo): dongles claiming ID_INPUT_JOYSTICK stay out.
fn is_pad(dev: &Device) -> bool {
    let Some(keys) = dev.supported_keys() else { return false };
    if keys.contains(KeyCode::new(BTN_GAMEPAD)) {
        return true;
    }
    if !keys.contains(KeyCode::new(BTN_JOYSTICK)) {
        return false;
    }
    let id = dev.input_id();
    detect_family(id.vendor(), id.product(), dev.name().unwrap_or(""), &[]).id != "generic"
}

fn bus_name(dev: &Device) -> String {
    format!("{}", dev.input_id().bus_type()).to_lowercase()
}

struct Caps {
    name: String,
    family: &'static Family,
    vendor: u16,
    product: u16,
    keys: Vec<u16>,
    axes: Vec<u16>,
    ranges: BTreeMap<u16, (i32, i32)>,
}

fn describe(dev: &Device) -> Caps {
    let name = dev.name().unwrap_or("").to_string();
    let id = dev.input_id();
    let keys: Vec<u16> = dev.supported_keys().map(|k| k.iter().map(|c| c.code()).collect()).unwrap_or_default();
    let family = detect_family(id.vendor(), id.product(), &name, &keys);
    let axes: Vec<u16> = dev.supported_absolute_axes().map(|a| a.iter().map(|c| c.0).collect()).unwrap_or_default();
    let ranges = dev.get_absinfo().map(|it| it.map(|(c, i)| (c.0, (i.minimum(), i.maximum()))).collect()).unwrap_or_default();
    Caps { name, family, vendor: id.vendor(), product: id.product(), keys, axes, ranges }
}

fn pad_of(id: String, path: PathBuf, bus: String, c: Caps, cfg: &ControllerConfig, cmd: mpsc::Sender<PadCmd>) -> Pad {
    let mut pad = Pad {
        id,
        path,
        name: c.name,
        family: c.family,
        bus,
        vendor: c.vendor,
        product: c.product,
        keys: c.keys,
        axes: c.axes,
        ranges: c.ranges,
        roles: BTreeMap::new(),
        slots: BTreeMap::new(),
        by_source: BTreeMap::new(),
        bindings: BTreeMap::new(),
        axis_down: BTreeSet::new(),
        axis_last: BTreeMap::new(),
        battery: None,
        cmd,
    };
    pad.resolve(cfg);
    pad
}

fn slots_json(slots: &BTreeMap<String, Option<Source>>) -> serde_json::Map<String, serde_json::Value> {
    slots.iter().map(|(s, src)| (s.clone(), serde_json::json!({"code": src.map(|x| x.to_string()), "bound": src.is_some()}))).collect()
}

fn readable(path: &Path) -> bool {
    rustix::fs::access(path, rustix::fs::Access::READ_OK).is_ok()
}

pub fn enumerate_json(cfg: &ControllerConfig) -> Vec<serde_json::Value> {
    let mut out = vec![];
    for (path, dev) in evdev::enumerate() {
        if !is_pad(&dev) {
            continue;
        }
        let bus = bus_name(&dev);
        let id = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let (ctx, _) = mpsc::channel(1);
        let mut v = pad_of(id, path.clone(), bus, describe(&dev), cfg, ctx).json();
        v["path"] = serde_json::json!(path);
        out.push(v);
    }
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    out
}

fn pad_task(id: String, dev: Device, mut hid: Option<AsyncFd<std::fs::File>>, tx: mpsc::Sender<DevEvent>, mut cmds: mpsc::Receiver<PadCmd>) {
    tokio::spawn(async move {
        let vendor = dev.input_id().vendor();
        let mut stream = match dev.into_event_stream() {
            Ok(s) => s,
            Err(_) => {
                let _ = tx.send(DevEvent::Gone(id)).await;
                return;
            }
        };
        let mut battery = None;
        loop {
            tokio::select! {
                ev = stream.next_event() => match ev {
                    Ok(ev) => { if tx.send(DevEvent::Input(id.clone(), ev)).await.is_err() { return; } }
                    Err(_) => { let _ = tx.send(DevEvent::Gone(id)).await; return; }
                },
                report = next_report(&mut hid) => {
                    if let Some((percent, charging)) = battery_of(vendor, &report).filter(|read| Some(*read) != battery) {
                        battery = Some((percent, charging));
                        if tx.send(DevEvent::Battery(id.clone(), percent, charging)).await.is_err() { return; }
                    }
                }
                cmd = cmds.recv() => match cmd {
                    Some(PadCmd::Rumble) => {
                        let data = evdev::FFEffectData {
                            direction: 0,
                            trigger: evdev::FFTrigger { button: 0, interval: 0 },
                            replay: evdev::FFReplay { length: 180, delay: 0 },
                            kind: evdev::FFEffectKind::Rumble { strong_magnitude: 0x9000, weak_magnitude: 0x5000 },
                        };
                        if let Ok(mut effect) = stream.device_mut().upload_ff_effect(data) {
                            let _ = effect.play(1);
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                    }
                    Some(PadCmd::Close) | None => return,
                },
            }
        }
    });
}

struct Typist {
    dev: Option<evdev::uinput::VirtualDevice>,
}

impl Typist {
    /// Opened at start: a compositor takes a moment to pick up a new keyboard, which would swallow the first combo.
    fn device(&mut self) -> Option<&mut evdev::uinput::VirtualDevice> {
        if self.dev.is_none() {
            let mut keys = evdev::AttributeSet::<KeyCode>::new();
            for c in 1..=0xf7u16 {
                keys.insert(KeyCode::new(c));
            }
            match evdev::uinput::VirtualDevice::builder().and_then(|b| b.name(b"Universe controller macros").with_keys(&keys)).and_then(|b| b.build()) {
                Ok(d) => self.dev = Some(d),
                Err(e) => {
                    tracing::warn!("uinput: {e} (is /dev/uinput writable by your user?)");
                    return None;
                }
            }
        }
        self.dev.as_mut()
    }

    fn set(&mut self, codes: &[u16], down: bool) {
        let Some(dev) = self.device() else { return };
        let mut events: Vec<InputEvent> = codes.iter().map(|c| InputEvent::new(EventType::KEY.0, *c, if down { 1 } else { 0 })).collect();
        events.push(InputEvent::new(EventType::SYNCHRONIZATION.0, 0, 0));
        if let Err(e) = dev.emit(&events) {
            tracing::warn!("uinput emit: {e}");
        }
    }
}

struct Watcher {
    core: Arc<Core>,
    cfg: ControllerConfig,
    out: Out,
    engine: Engine,
    pads: BTreeMap<String, Pad>,
    ignored: BTreeSet<PathBuf>,
    tx: mpsc::Sender<DevEvent>,
    typist: Arc<Mutex<Typist>>,
    suspended: bool,
    // Under the dock: the docked presets still fire.
    docked: bool,
    axes: bool,
    learning: Option<(String, Learn, Instant)>,
    started: Instant,
    config_mtime: Option<std::time::SystemTime>,
}

fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(paths::config_file()).and_then(|m| m.modified()).ok()
}

impl Watcher {
    fn now(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn scan(&mut self) {
        let Ok(rd) = std::fs::read_dir("/dev/input") else { return };
        let present: BTreeSet<PathBuf> =
            rd.flatten().map(|e| e.path()).filter(|p| p.file_name().map(|f| f.to_string_lossy().starts_with("event")).unwrap_or(false)).collect();
        self.ignored.retain(|p| present.contains(p));
        let gone: Vec<String> = self.pads.values().filter(|p| !present.contains(&p.path) || !readable(&p.path)).map(|p| p.id.clone()).collect();
        for id in gone {
            self.drop_pad(&id);
        }
        for path in present {
            if self.ignored.contains(&path) || self.pads.values().any(|p| p.path == path) {
                continue;
            }
            let dev = match Device::open(&path) {
                Ok(d) => d,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => continue,
                Err(_) => {
                    self.ignored.insert(path);
                    continue;
                }
            };
            if !is_pad(&dev) {
                self.ignored.insert(path);
                continue;
            }
            let id = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let bus = bus_name(&dev);
            let (ctx, crx) = mpsc::channel(4);
            let hid = hidraw_of(&path, dev.input_id().vendor());
            let pad = pad_of(id.clone(), path, bus, describe(&dev), &self.cfg, ctx);
            pad_task(id.clone(), dev, hid, self.tx.clone(), crx);
            self.out.emit(pad.json());
            self.pads.insert(id, pad);
        }
    }

    fn battery(&mut self, id: &str, percent: u8, charging: bool) -> bool {
        let Some(pad) = self.pads.get_mut(id) else { return true };
        pad.battery = Some((percent, charging));
        self.out.emit(serde_json::json!({"event": "battery", "id": id, "percent": percent, "charging": charging}))
    }

    fn drop_pad(&mut self, id: &str) {
        if let Some(p) = self.pads.remove(id) {
            let _ = p.cmd.try_send(PadCmd::Close);
            self.engine.forget_device(id);
            self.out.emit(serde_json::json!({"event": "gone", "id": id}));
        }
    }

    async fn reload(&mut self) {
        if let Err(e) = self.core.reload_settings().await {
            tracing::warn!("reload: {e}");
        }
        self.cfg = self.core.config.read().await.controller.clone();
        self.config_mtime = config_mtime();
        self.engine.hold_ms = self.cfg.hold_ms;
        for pad in self.pads.values_mut() {
            pad.resolve(&self.cfg);
            self.out.emit(pad.json());
        }
    }

    async fn input(&mut self, id: String, ev: InputEvent) -> bool {
        let Some(pad) = self.pads.get_mut(&id) else { return true };
        let (transitions, sample): (Vec<(Source, bool)>, Option<serde_json::Value>) = match ev.destructure() {
            EventSummary::Key(_, key, value) => {
                if value == 2 {
                    return true;
                }
                (vec![(Source::Key(key.code()), value != 0)], None)
            }
            EventSummary::AbsoluteAxis(_, axis, value) => {
                let code = axis.0;
                if let Some((_, Learn::Axis(role), _)) = self.learning.clone().filter(|(lid, _, _)| *lid == id) {
                    if let Some(thrown) = pad.axis_thrown(code, value) {
                        let family = pad.family;
                        self.learning = None;
                        return self.learnt(family, Learn::Axis(role), &thrown).await;
                    }
                }
                let sample = if self.axes { pad.axis_sample(code, value) } else { None };
                // A trigger being learned listens to its own axis too: a pull answers before the click at the end of it.
                let trigger_learn = matches!(&self.learning, Some((lid, Learn::Slot(s), _)) if *lid == id && matches!(s.as_str(), "lt" | "rt"))
                    && pad.roles.get(&code).is_some_and(|(r, _)| matches!(*r, "lt" | "rt"));
                let owned = (16..=17).contains(&code) || trigger_learn || pad.by_source.keys().any(|s| matches!(s, Source::Axis { code: c, .. } if *c == code));
                (if owned { pad.axis(code, value) } else { vec![] }, sample)
            }
            _ => return true,
        };
        if let Some(sample) = sample {
            if !self.out.emit(sample) {
                return false;
            }
        }
        for (source, down) in transitions {
            let learnable = down && matches!(source, Source::Key(_) | Source::Axis { .. });
            if let Some((_, Learn::Slot(slot), _)) = self.learning.clone().filter(|(lid, _, _)| *lid == id && learnable) {
                self.learning = None;
                let family = self.pads[&id].family;
                return self.learnt(family, Learn::Slot(slot), &source.to_string()).await;
            }
            let pad = self.pads.get(&id).unwrap();
            let Some(slot) = pad.by_source.get(&source).cloned() else {
                if down && matches!(source, Source::Key(_)) && !self.out.emit(serde_json::json!({"event": "unknown", "id": id, "code": source.to_string()})) {
                    return false;
                }
                continue;
            };
            if !self.out.emit(serde_json::json!({"event": "button", "id": id, "slot": slot, "code": source.to_string(), "pressed": down})) {
                return false;
            }
            let mut binding = pad.bindings.get(&slot).cloned().unwrap_or_default();
            // Suspended, a slot keeps its docked macros under the dock and nothing elsewhere; a release always reaches the engine, so nothing stays held.
            if self.suspended {
                let keep = |m: &super::Macro| self.docked && super::preset(&m.action).is_some_and(|p| p.docked);
                binding.press = binding.press.filter(keep);
                binding.hold = binding.hold.filter(keep);
            }
            let now = self.now();
            let fires = if down { self.engine.press(&id, &slot, binding, now) } else { self.engine.release(&id, &slot, now) };
            for f in fires {
                self.fire(f);
            }
        }
        true
    }

    /// What a learn found, written for the family, told, and taken up by every pad of it.
    async fn learnt(&mut self, family: &'static Family, target: Learn, code: &str) -> bool {
        let written = match &target {
            Learn::Slot(slot) => super::learn_code(&self.cfg, family, slot, code)
                .map(|from| serde_json::json!({"event": "learned", "family": family.id, "slot": slot, "code": code, "from": from})),
            Learn::Axis(role) => {
                super::learn_axis(family, role, code).map(|()| serde_json::json!({"event": "learned", "family": family.id, "axis": role, "code": code}))
            }
        };
        match written {
            Ok(line) => {
                if !self.out.emit(line) {
                    return false;
                }
                self.reload().await;
            }
            Err(e) => {
                self.out.emit(serde_json::json!({"event": "error", "message": e.to_string()}));
            }
        }
        true
    }

    fn fire(&mut self, f: Fire) {
        let (id, slot, m) = (f.device, f.slot, f.action);
        self.out.emit(
            serde_json::json!({"event": "macro", "id": id, "slot": slot, "trigger": f.trigger, "action": m.action, "keys": m.keys, "command": m.command}),
        );
        let core = self.core.clone();
        match m.action.as_str() {
            "volume_up" | "volume_down" | "mute" => {
                let change = match m.action.as_str() {
                    "volume_up" => super::volume::Change::Up,
                    "volume_down" => super::volume::Change::Down,
                    _ => super::volume::Change::ToggleMute,
                };
                let percent = self.cfg.volume_step;
                tokio::spawn(async move {
                    if let Err(e) = super::volume::change(change, percent).await {
                        tracing::warn!("{change:?}: {e}");
                    }
                });
            }
            "mangohud" => {
                let out = self.out;
                tokio::spawn(async move {
                    let shown = core.set_mangohud(None).await.inspect_err(|e| tracing::warn!("mangohud: {e}")).ok();
                    let title = if shown.is_some() { core.current().await.map(|c| c.title).unwrap_or_default() } else { String::new() };
                    out.emit(serde_json::json!({"event": "hud", "shown": shown, "title": title}));
                });
            }
            "keys" => {
                let Ok(codes) = keys::parse_combo(&m.keys) else { return };
                let typist = self.typist.clone();
                tokio::spawn(async move {
                    let mut t = typist.lock().await;
                    t.set(&codes, true);
                    tokio::time::sleep(COMBO_HOLD).await;
                    let up: Vec<u16> = codes.iter().rev().copied().collect();
                    t.set(&up, false);
                });
            }
            // The hook answers once the pixels are grabbed: the launcher's cue (the flash, the shutter) follows the event.
            "screenshot" => {
                let out = self.out;
                tokio::spawn(async move {
                    let path = core.screenshot().await.inspect_err(|e| tracing::warn!("screenshot: {e}")).unwrap_or_default();
                    out.emit(serde_json::json!({"event": "screenshot", "path": path}));
                });
            }
            "stop" => {
                if let Some(p) = self.pads.get(&id) {
                    let _ = p.cmd.try_send(PadCmd::Rumble);
                }
                tokio::spawn(async move {
                    if let Err(e) = core.stop("").await {
                        tracing::warn!("stop: {e}");
                    }
                });
            }
            "command" => {
                let command = m.command.clone();
                tokio::spawn(async move {
                    let mut cmd = tokio::process::Command::new("sh");
                    cmd.arg("-c").arg(&command).stdin(std::process::Stdio::null());
                    if let Some(c) = core.current().await {
                        cmd.env("GAME_ID", &c.id)
                            .env("GAME_TITLE", &c.title)
                            .env("SESSION_ID", &c.session_id)
                            .env("SESSION_UNIT", &c.unit)
                            .env("SESSION_SCREEN", &c.screen);
                    }
                    match cmd.status().await {
                        Ok(s) if !s.success() => tracing::warn!("command exited {s}: {command}"),
                        Err(e) => tracing::warn!("command: {e}"),
                        _ => {}
                    }
                });
            }
            other => tracing::warn!("no such action {other}"),
        }
    }

    async fn command(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return self.out.emit(serde_json::json!({"event": "error", "message": format!("bad command: {line}")}));
        };
        match v["cmd"].as_str().unwrap_or("") {
            "suspend" => {
                self.suspended = true;
                self.docked = v["dock"].as_bool().unwrap_or(false);
                self.engine.clear();
            }
            "resume" => self.suspended = false,
            "axes" => {
                self.axes = v["on"].as_bool().unwrap_or(false);
                for p in self.pads.values_mut() {
                    p.axis_last.clear();
                }
            }
            "reload" => self.reload().await,
            "learn" => {
                let id = v["id"].as_str().unwrap_or("").to_string();
                let slot = v["slot"].as_str().unwrap_or("").to_string();
                let axis = v["axis"].as_str().unwrap_or("").to_string();
                let known = self.pads.get(&id).is_some_and(|p| {
                    if axis.is_empty() {
                        p.family.slots().any(|s| s.id == slot)
                    } else {
                        super::AXIS_ROLES.contains(&axis.as_str())
                    }
                });
                if !known {
                    return self.out.emit(
                        serde_json::json!({"event": "error", "message": format!("cannot learn {} on {id}", if axis.is_empty() { &slot } else { &axis })}),
                    );
                }
                for p in self.pads.values_mut() {
                    p.axis_down.clear();
                }
                self.learning = Some((id, if axis.is_empty() { Learn::Slot(slot) } else { Learn::Axis(axis) }, Instant::now()));
            }
            "cancel" => self.learning = None,
            "rumble" => {
                if let Some(p) = self.pads.get(v["id"].as_str().unwrap_or("")) {
                    let _ = p.cmd.try_send(PadCmd::Rumble);
                }
            }
            "run" => {
                let action = super::Macro {
                    action: v["action"].as_str().unwrap_or("").into(),
                    keys: v["keys"].as_str().unwrap_or("").into(),
                    command: v["command"].as_str().unwrap_or("").into(),
                    ..Default::default()
                };
                self.fire(Fire { device: String::new(), slot: String::new(), trigger: "run", action });
            }
            "quit" => return false,
            other => return self.out.emit(serde_json::json!({"event": "error", "message": format!("unknown command {other}")})),
        }
        true
    }
}

pub async fn watch(core: Arc<Core>, opts: WatchOptions) -> crate::Result<()> {
    let out = Out { json: opts.json };
    let Some(_lock) = take_lock(opts.wait, &out).await? else {
        out.emit(serde_json::json!({"event": "busy"}));
        return Ok(());
    };
    let cfg = core.config.read().await.controller.clone();
    let (tx, mut rx) = mpsc::channel::<DevEvent>(256);
    let mut w = Watcher {
        core,
        engine: Engine::new(cfg.hold_ms),
        cfg,
        out,
        pads: BTreeMap::new(),
        ignored: BTreeSet::new(),
        tx,
        typist: Arc::new(Mutex::new(Typist { dev: None })),
        suspended: false,
        docked: false,
        axes: false,
        learning: None,
        started: Instant::now(),
        config_mtime: config_mtime(),
    };
    if !w.out.emit(serde_json::json!({"event": "ready", "enabled": w.cfg.enabled})) {
        return Ok(());
    }
    w.typist.lock().await.device();
    w.scan();
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    let mut stdin_open = true;
    let mut scan = tokio::time::interval(SCAN_EVERY);
    loop {
        let deadline = w.engine.deadline().map(|d| Duration::from_millis(d.saturating_sub(w.now())));
        let tick = async {
            match deadline {
                Some(d) => tokio::time::sleep(d).await,
                None => std::future::pending::<()>().await,
            }
        };
        let line = async {
            if stdin_open {
                stdin.next_line().await
            } else {
                std::future::pending().await
            }
        };
        let alive = tokio::select! {
            ev = rx.recv() => match ev {
                Some(DevEvent::Input(id, ev)) => w.input(id, ev).await,
                Some(DevEvent::Battery(id, percent, charging)) => w.battery(&id, percent, charging),
                Some(DevEvent::Gone(id)) => { w.drop_pad(&id); true }
                None => false,
            },
            line = line => match line {
                Ok(Some(l)) if !l.trim().is_empty() => w.command(l.trim()).await,
                Ok(Some(_)) => true,
                // The launcher closed its end: stop with it. A unit's /dev/null is read once, then left alone.
                Ok(None) | Err(_) => { stdin_open = false; !opts.json }
            },
            _ = scan.tick() => {
                w.scan();
                // A bind from the launcher or a terminal reaches a watcher it has no pipe to.
                if config_mtime() != w.config_mtime {
                    w.reload().await;
                }
                if w.learning.as_ref().map(|(_, _, since)| since.elapsed() > LEARN_TIMEOUT).unwrap_or(false) {
                    w.learning = None;
                    w.out.emit(serde_json::json!({"event": "learn_timeout"}));
                }
                true
            }
            _ = tick => {
                let now = w.now();
                for f in w.engine.tick(now) {
                    w.fire(f);
                }
                true
            }
        };
        if !alive {
            break;
        }
    }
    for id in w.pads.keys().cloned().collect::<Vec<_>>() {
        w.drop_pad(&id);
    }
    Ok(())
}

pub async fn learn_once(cfg: &ControllerConfig, family: &Family, slot: &str) -> crate::Result<(String, Option<String>)> {
    let (tx, mut rx) = mpsc::channel::<DevEvent>(64);
    let mut senders = vec![];
    for (path, dev) in evdev::enumerate() {
        if !is_pad(&dev) {
            continue;
        }
        if describe(&dev).family.id != family.id {
            continue;
        }
        let (ctx, crx) = mpsc::channel(1);
        pad_task(path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default(), dev, None, tx.clone(), crx);
        senders.push(ctx);
    }
    if senders.is_empty() {
        return Err(crate::Error::NotFound(format!("no {} connected", family.name)));
    }
    let deadline = tokio::time::sleep(LEARN_TIMEOUT);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Some(DevEvent::Input(_, ev)) => {
                    let source = match ev.destructure() {
                        EventSummary::Key(_, key, 1) => Source::Key(key.code()),
                        EventSummary::AbsoluteAxis(_, axis, v) if (16..=17).contains(&axis.0) && v != 0 => Source::Axis { code: axis.0, positive: v > 0 },
                        _ => continue,
                    };
                    let code = source.to_string();
                    let from = super::learn_code(cfg, family, slot, &code)?;
                    return Ok((code, from));
                }
                Some(DevEvent::Battery(..) | DevEvent::Gone(_)) | None => continue,
            },
            _ = &mut deadline => return Err(crate::Error::Io("no button pressed within 30 s".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_normalize_by_their_range() {
        assert_eq!(axis_value("lx", 255, (0, 255)), 1.0);
        assert!((axis_value("ly", 128, (0, 255)) - 0.0039).abs() < 0.001, "a DualSense stick rests a hair off centre");
        assert_eq!(axis_value("rx", -32768, (-32768, 32767)), -1.0);
        assert_eq!(axis_value("lt", 0, (0, 1023)), 0.0);
        assert_eq!(axis_value("rt", 1023, (0, 1023)), 1.0);
        assert_eq!(axis_value("lt", 5, (0, 0)), 0.0, "an empty range is at rest");
    }

    // A Pro 3 report over Bluetooth, at rest, as captured: report 4, byte 14 = 0x50.
    #[test]
    fn an_8bitdo_report_carries_its_charge() {
        let mut report = vec![0x04, 0x0f, 0x80, 0x7f, 0x7f, 0x80, 0, 0, 0, 0, 0, 0, 0x09, 0x60, 0x50, 0xb8];
        assert_eq!(battery_of(EIGHTBITDO, &report), Some((80, false)));
        report[14] = 0x80 | 45;
        assert_eq!(battery_of(EIGHTBITDO, &report), Some((45, true)), "the top bit is the charger");
        report[14] = 100;
        assert_eq!(battery_of(EIGHTBITDO, &report), Some((100, true)), "full on the cable reads as charging, as sysfs's Full does");
        assert_eq!(battery_of(0x054c, &report), None, "another maker's report means something else");
        report[0] = 0x06;
        assert_eq!(battery_of(EIGHTBITDO, &report), None, "only the state reports");
        assert_eq!(battery_of(EIGHTBITDO, &[0x04, 0x0f]), None);
    }
}
