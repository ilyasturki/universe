use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{mpsc, oneshot};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue};

use crate::{Error, Result};

const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const BATTERY: &str = "org.bluez.Battery1";
const AGENT_MANAGER: &str = "org.bluez.AgentManager1";
pub const AGENT_PATH: &str = "/org/universe/agent";
const CALL: Duration = Duration::from_secs(5);
// Pair returns once the device has bonded: the user's answer, then the link keys.
const PAIRING: Duration = Duration::from_secs(90);
// BlueZ cancels an agent request it has waited this long on (src/agent.c REQUEST_TIMEOUT).
const ANSWER: Duration = Duration::from_secs(60);
// A burst of property changes (a scan's RSSI, a pairing's flags) is read once.
const SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bluetooth {
    pub available: bool,
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub address: String,
    pub name: String,
    /// `pad`, `audio`, `keyboard`, `mouse`, `phone` or `other`, from BlueZ's icon.
    pub kind: String,
    pub icon: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub battery: Option<u8>,
}

type Props = HashMap<String, OwnedValue>;

fn text(props: &Props, key: &str) -> Option<String> {
    props.get(key).and_then(|v| <&str>::try_from(&**v).ok()).map(str::to_string)
}

fn flag(props: &Props, key: &str) -> bool {
    props.get(key).and_then(|v| bool::try_from(&**v).ok()).unwrap_or(false)
}

pub fn kind_of(icon: &str) -> &'static str {
    match icon {
        "input-gaming" => "pad",
        "audio-headset" | "audio-headphones" | "audio-card" => "audio",
        "input-keyboard" => "keyboard",
        "input-mouse" | "input-tablet" => "mouse",
        "phone" => "phone",
        _ => "other",
    }
}

/// The first adapter and the devices under it: those paired, and those found that tell a name (a nameless one is a beacon).
pub fn state_of(objects: &zbus::fdo::ManagedObjects) -> Bluetooth {
    let Some((adapter, props)) =
        objects.iter().filter_map(|(path, ifaces)| ifaces.get(ADAPTER).map(|a| (path, a))).min_by(|a, b| a.0.as_str().cmp(b.0.as_str()))
    else {
        return Bluetooth::default();
    };
    let under = format!("{}/", adapter.as_str());
    let mut devices: Vec<Device> = objects
        .iter()
        .filter(|(path, _)| path.as_str().starts_with(&under))
        .filter_map(|(_, ifaces)| {
            let d = ifaces.get(DEVICE)?;
            let paired = flag(d, "Paired");
            let name = text(d, "Name").filter(|n| !n.is_empty());
            if !paired && name.is_none() {
                return None;
            }
            let address = text(d, "Address")?;
            let icon = text(d, "Icon").unwrap_or_default();
            Some(Device {
                name: text(d, "Alias").or(name).unwrap_or_else(|| address.clone()),
                address,
                kind: kind_of(&icon).to_string(),
                icon,
                paired,
                connected: flag(d, "Connected"),
                trusted: flag(d, "Trusted"),
                battery: ifaces.get(BATTERY).and_then(|b| b.get("Percentage")).and_then(|v| u8::try_from(&**v).ok()).map(|p| p.min(100)),
            })
        })
        .collect();
    devices.sort_by_key(|d| (!d.paired, d.name.to_lowercase(), d.address.clone()));
    Bluetooth { available: true, powered: flag(props, "Powered"), discovering: flag(props, "Discovering"), devices }
}

async fn objects(conn: &zbus::Connection) -> zbus::Result<zbus::fdo::ManagedObjects> {
    let manager = zbus::fdo::ObjectManagerProxy::builder(conn).destination(BLUEZ)?.path("/")?.build().await?;
    Ok(manager.get_managed_objects().await?)
}

pub async fn read(conn: &zbus::Connection) -> Bluetooth {
    match tokio::time::timeout(CALL, objects(conn)).await {
        Ok(Ok(objects)) => state_of(&objects),
        _ => Bluetooth::default(),
    }
}

pub async fn system() -> Result<zbus::Connection> {
    zbus::Connection::system().await.map_err(|e| Error::Unavailable(format!("the system bus: {e}")))
}

pub async fn state() -> Bluetooth {
    match system().await {
        Ok(conn) => read(&conn).await,
        Err(_) => Bluetooth::default(),
    }
}

fn failed(e: zbus::Error) -> Error {
    match e {
        zbus::Error::MethodError(name, message, _) => {
            let short = name.as_str().trim_start_matches("org.bluez.Error.").to_string();
            Error::Io(message.filter(|m| !m.is_empty()).unwrap_or(short))
        }
        e => Error::Unavailable(format!("BlueZ: {e}")),
    }
}

/// Why BlueZ refused, as the looks word it: `rejected`, `canceled`, `timeout` or `failed`.
pub fn reason(e: &zbus::Error) -> &'static str {
    let zbus::Error::MethodError(name, _, _) = e else { return "failed" };
    match name.as_str().trim_start_matches("org.bluez.Error.") {
        "AuthenticationRejected" | "Rejected" => "rejected",
        "AuthenticationCanceled" | "Canceled" => "canceled",
        "AuthenticationTimeout" | "ConnectionAttemptFailed" => "timeout",
        _ => "failed",
    }
}

async fn proxy<'a>(conn: &zbus::Connection, path: OwnedObjectPath, iface: &'static str) -> zbus::Result<zbus::Proxy<'a>> {
    zbus::proxy::Builder::<zbus::Proxy>::new(conn)
        .destination(BLUEZ)?
        .path(path)?
        .interface(iface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

async fn adapter(conn: &zbus::Connection) -> Result<OwnedObjectPath> {
    let objects = tokio::time::timeout(CALL, objects(conn)).await.map_err(|_| Error::Unavailable("BlueZ did not answer".into()))?.map_err(failed)?;
    objects
        .into_iter()
        .filter(|(_, ifaces)| ifaces.contains_key(ADAPTER))
        .map(|(path, _)| path)
        .min_by(|a, b| a.as_str().cmp(b.as_str()))
        .ok_or_else(|| Error::Unavailable("no Bluetooth adapter".into()))
}

async fn device(conn: &zbus::Connection, address: &str) -> Result<OwnedObjectPath> {
    let objects = tokio::time::timeout(CALL, objects(conn)).await.map_err(|_| Error::Unavailable("BlueZ did not answer".into()))?.map_err(failed)?;
    objects
        .into_iter()
        .find(|(_, ifaces)| ifaces.get(DEVICE).and_then(|d| text(d, "Address")).is_some_and(|a| a.eq_ignore_ascii_case(address)))
        .map(|(path, _)| path)
        .ok_or_else(|| Error::NotFound(format!("no Bluetooth device {address}")))
}

async fn timed<T>(limit: Duration, call: impl std::future::Future<Output = zbus::Result<T>>) -> zbus::Result<T> {
    tokio::time::timeout(limit, call).await.unwrap_or_else(|_| Err(zbus::Error::InputOutput(std::io::Error::from(std::io::ErrorKind::TimedOut).into())))
}

pub async fn set_powered(conn: &zbus::Connection, on: bool) -> Result<()> {
    let path = adapter(conn).await?;
    timed(CALL, async { Ok(proxy(conn, path, ADAPTER).await?.set_property("Powered", on).await?) }).await.map_err(failed)
}

/// Discovery is the caller's own: BlueZ stops it once its connection goes.
pub async fn discover(conn: &zbus::Connection, on: bool) -> Result<()> {
    let path = adapter(conn).await?;
    timed(CALL, async { proxy(conn, path, ADAPTER).await?.call::<_, _, ()>(if on { "StartDiscovery" } else { "StopDiscovery" }, &()).await })
        .await
        .map_err(failed)
}

/// Pair, trust so it reconnects without asking, then connect; whether the connect took. The pairing's questions go to the agent
/// `conn` registered, ahead of the default one: BlueZ routes them by the caller.
pub async fn pair_device(conn: &zbus::Connection, path: OwnedObjectPath) -> zbus::Result<bool> {
    let device = proxy(conn, path, DEVICE).await?;
    match timed(PAIRING, device.call::<_, _, ()>("Pair", &())).await {
        Ok(()) => {}
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.bluez.Error.AlreadyExists" => {}
        Err(e) => return Err(e),
    }
    timed(CALL, async { Ok(device.set_property("Trusted", true).await?) }).await?;
    Ok(timed(PAIRING, device.call::<_, _, ()>("Connect", &())).await.is_ok())
}

pub async fn pair(conn: &zbus::Connection, address: &str) -> Result<bool> {
    pair_device(conn, device(conn, address).await?).await.map_err(failed)
}

pub async fn connect(conn: &zbus::Connection, address: &str) -> Result<()> {
    let path = device(conn, address).await?;
    timed(PAIRING, async { proxy(conn, path, DEVICE).await?.call::<_, _, ()>("Connect", &()).await }).await.map_err(failed)
}

pub async fn disconnect(conn: &zbus::Connection, address: &str) -> Result<()> {
    let path = device(conn, address).await?;
    timed(CALL, async { proxy(conn, path, DEVICE).await?.call::<_, _, ()>("Disconnect", &()).await }).await.map_err(failed)
}

pub async fn remove(conn: &zbus::Connection, address: &str) -> Result<()> {
    let path = device(conn, address).await?;
    let owner = adapter(conn).await?;
    timed(CALL, async { proxy(conn, owner, ADAPTER).await?.call::<_, _, ()>("RemoveDevice", &(path,)).await }).await.map_err(failed)
}

/// A question a pairing puts to the agent; dropping `reply` cancels it.
#[derive(Debug)]
pub enum Ask {
    /// Both ends show `passkey`: the same on each?
    Confirm {
        device: OwnedObjectPath,
        passkey: u32,
        reply: oneshot::Sender<Answer>,
    },
    /// A device asks to pair or to use a service (BlueZ's cable pairing of a Sony pad among them): let it?
    Authorize {
        device: OwnedObjectPath,
        reply: oneshot::Sender<Answer>,
    },
    /// The passkey the device shows, typed here.
    Passkey {
        device: OwnedObjectPath,
        reply: oneshot::Sender<Answer>,
    },
    /// A legacy PIN, typed here.
    Pin {
        device: OwnedObjectPath,
        reply: oneshot::Sender<Answer>,
    },
    /// A code to type on the device (a keyboard); `entered` keys typed so far.
    Display {
        device: OwnedObjectPath,
        code: String,
        entered: u16,
    },
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Yes,
    No,
    Text(String),
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum Refusal {
    Rejected,
    Canceled,
}

struct Agent {
    asks: mpsc::UnboundedSender<Ask>,
}

impl Agent {
    async fn ask(&self, make: impl FnOnce(oneshot::Sender<Answer>) -> Ask) -> std::result::Result<Answer, Refusal> {
        let (tx, rx) = oneshot::channel();
        self.asks.send(make(tx)).map_err(|_| Refusal::Rejected)?;
        match tokio::time::timeout(ANSWER, rx).await {
            Ok(Ok(Answer::No)) => Err(Refusal::Rejected),
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(_)) | Err(_) => Err(Refusal::Canceled),
        }
    }

    async fn text(&self, make: impl FnOnce(oneshot::Sender<Answer>) -> Ask) -> std::result::Result<String, Refusal> {
        match self.ask(make).await? {
            Answer::Text(t) if !t.is_empty() => Ok(t),
            _ => Err(Refusal::Rejected),
        }
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {}

    async fn request_pin_code(&self, device: OwnedObjectPath) -> std::result::Result<String, Refusal> {
        self.text(|reply| Ask::Pin { device, reply }).await
    }

    fn display_pin_code(&self, device: OwnedObjectPath, pincode: String) {
        let _ = self.asks.send(Ask::Display { device, code: pincode, entered: 0 });
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> std::result::Result<u32, Refusal> {
        self.text(|reply| Ask::Passkey { device, reply }).await?.trim().parse().map_err(|_| Refusal::Rejected)
    }

    fn display_passkey(&self, device: OwnedObjectPath, passkey: u32, entered: u16) {
        let _ = self.asks.send(Ask::Display { device, code: format!("{passkey:06}"), entered });
    }

    async fn request_confirmation(&self, device: OwnedObjectPath, passkey: u32) -> std::result::Result<(), Refusal> {
        self.ask(|reply| Ask::Confirm { device, passkey, reply }).await.map(drop)
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> std::result::Result<(), Refusal> {
        self.ask(|reply| Ask::Authorize { device, reply }).await.map(drop)
    }

    async fn authorize_service(&self, device: OwnedObjectPath, _uuid: String) -> std::result::Result<(), Refusal> {
        self.ask(|reply| Ask::Authorize { device, reply }).await.map(drop)
    }

    fn cancel(&self) {
        let _ = self.asks.send(Ask::Cancel);
    }
}

async fn register(conn: &zbus::Connection, default: bool) -> zbus::Result<()> {
    let manager = zbus::Proxy::new(conn, BLUEZ, "/org/bluez", AGENT_MANAGER).await?;
    let path = ObjectPath::from_static_str_unchecked(AGENT_PATH);
    match timed(CALL, manager.call::<_, _, ()>("RegisterAgent", &(&path, "KeyboardDisplay"))).await {
        Ok(()) => {}
        Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.bluez.Error.AlreadyExists" => {}
        Err(e) => return Err(e),
    }
    if default {
        timed(CALL, manager.call::<_, _, ()>("RequestDefaultAgent", &(&path,))).await?;
    }
    Ok(())
}

/// Serves the agent on `conn` and registers it: for `conn`'s own pairings always, for every other request too when `default`
/// (the Universe session, where no desktop has one). Its questions come out of the receiver.
pub async fn agent(conn: &zbus::Connection, default: bool) -> Result<mpsc::UnboundedReceiver<Ask>> {
    let (asks, rx) = mpsc::unbounded_channel();
    conn.object_server().at(AGENT_PATH, Agent { asks }).await.map_err(failed)?;
    register(conn, default).await.map_err(failed)?;
    Ok(rx)
}

async fn describe(conn: &zbus::Connection, device: &OwnedObjectPath) -> (String, String, String) {
    let props = async {
        let p = zbus::fdo::PropertiesProxy::builder(conn).destination(BLUEZ)?.path(device.clone())?.build().await?;
        Ok::<_, zbus::Error>(p.get_all(zbus::names::InterfaceName::from_static_str_unchecked(DEVICE)).await?)
    };
    let props: Props = tokio::time::timeout(CALL, props).await.ok().and_then(|r| r.ok()).unwrap_or_default();
    let address = text(&props, "Address").unwrap_or_default();
    let name = text(&props, "Alias").or_else(|| text(&props, "Name")).unwrap_or_else(|| address.clone());
    (address, name, kind_of(&text(&props, "Icon").unwrap_or_default()).to_string())
}

struct Watch<E> {
    conn: zbus::Connection,
    emit: E,
    default: bool,
    pending: BTreeMap<u64, oneshot::Sender<Answer>>,
    next: u64,
    shown: Option<Bluetooth>,
    done: mpsc::UnboundedSender<serde_json::Value>,
}

impl<E: FnMut(serde_json::Value) -> bool> Watch<E> {
    async fn refresh(&mut self) -> bool {
        let now = read(&self.conn).await;
        if self.shown.as_ref() == Some(&now) {
            return true;
        }
        let mut line = serde_json::to_value(&now).unwrap_or_default();
        line["event"] = json!("state");
        self.shown = Some(now);
        (self.emit)(line)
    }

    async fn ask(&mut self, ask: Ask) -> bool {
        let (kind, device, code, entered, reply) = match ask {
            Ask::Confirm { device, passkey, reply } => ("confirm", device, format!("{passkey:06}"), 0, Some(reply)),
            Ask::Authorize { device, reply } => ("authorize", device, String::new(), 0, Some(reply)),
            Ask::Passkey { device, reply } => ("passkey", device, String::new(), 0, Some(reply)),
            Ask::Pin { device, reply } => ("pin", device, String::new(), 0, Some(reply)),
            Ask::Display { device, code, entered } => ("display", device, code, entered, None),
            Ask::Cancel => {
                self.pending.clear();
                return (self.emit)(json!({"event": "cancel"}));
            }
        };
        let (address, name, device_kind) = describe(&self.conn, &device).await;
        self.next += 1;
        if let Some(reply) = reply {
            self.pending.insert(self.next, reply);
        }
        (self.emit)(
            json!({"event": "request", "id": self.next, "kind": kind, "address": address, "name": name, "device_kind": device_kind, "code": code, "entered": entered}),
        )
    }

    fn spawn(&self, action: &'static str, address: String) -> Result<()> {
        let conn = self.conn.clone();
        let done = self.done.clone();
        tokio::spawn(async move {
            let outcome = match action {
                "pair" => match device(&conn, &address).await {
                    Ok(path) => pair_device(&conn, path).await.map(|connected| json!({"connected": connected})).map_err(|e| (reason(&e), failed(e))),
                    Err(e) => Err(("failed", e)),
                },
                "connect" => connect(&conn, &address).await.map(|()| json!({})).map_err(|e| ("failed", e)),
                "disconnect" => disconnect(&conn, &address).await.map(|()| json!({})).map_err(|e| ("failed", e)),
                _ => remove(&conn, &address).await.map(|()| json!({})).map_err(|e| ("failed", e)),
            };
            let line = match outcome {
                Ok(mut extra) => {
                    extra["event"] = json!("done");
                    extra["action"] = json!(action);
                    extra["address"] = json!(address);
                    extra
                }
                Err((reason, e)) => json!({"event": "failed", "action": action, "address": address, "reason": reason, "message": e.to_string()}),
            };
            let _ = done.send(line);
        });
        Ok(())
    }

    async fn command(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return (self.emit)(json!({"event": "error", "message": format!("bad command: {line}")}));
        };
        let address = v["address"].as_str().unwrap_or("").to_string();
        let on = v["on"].as_bool().unwrap_or(true);
        let outcome = match v["cmd"].as_str().unwrap_or("") {
            "scan" => discover(&self.conn, on).await,
            "power" => set_powered(&self.conn, on).await,
            "pair" => self.spawn("pair", address),
            "connect" => self.spawn("connect", address),
            "disconnect" => self.spawn("disconnect", address),
            "remove" => self.spawn("remove", address),
            "answer" => {
                let Some(reply) = v["id"].as_u64().and_then(|id| self.pending.remove(&id)) else {
                    return true;
                };
                let answer = match (&v["yes"], v["value"].as_str()) {
                    (_, Some(value)) => Answer::Text(value.to_string()),
                    (serde_json::Value::Bool(true), None) => Answer::Yes,
                    _ => Answer::No,
                };
                let _ = reply.send(answer);
                Ok(())
            }
            "refresh" => {
                self.shown = None;
                return self.refresh().await;
            }
            "quit" => return false,
            other => return (self.emit)(json!({"event": "error", "message": format!("unknown command {other}")})),
        };
        match outcome {
            Ok(()) => true,
            Err(e) => (self.emit)(json!({"event": "error", "message": e.to_string()})),
        }
    }
}

/// `universe bluetooth watch`: the state on start and after every change, the agent's questions, each action's outcome; commands
/// in `lines`, one JSON object each. Ends when `lines` does, on `quit`, or once `emit` says no one listens.
pub async fn watch<L, E>(conn: zbus::Connection, default: bool, mut lines: L, emit: E) -> Result<()>
where
    L: Stream<Item = String> + Unpin,
    E: FnMut(serde_json::Value) -> bool,
{
    let (asks_tx, mut asks) = mpsc::unbounded_channel();
    conn.object_server().at(AGENT_PATH, Agent { asks: asks_tx }).await.map_err(failed)?;
    let agent_ok = register(&conn, default).await.is_ok();
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(BLUEZ).map_err(failed)?.build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, None).await.map_err(failed)?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await.map_err(failed)?;
    let mut owners = dbus.receive_name_owner_changed_with_args(&[(0, BLUEZ)]).await.map_err(failed)?;
    let (done, mut outcomes) = mpsc::unbounded_channel();
    let mut w = Watch { conn, emit, default, pending: BTreeMap::new(), next: 0, shown: None, done };
    if !(w.emit)(json!({"event": "ready", "agent": agent_ok, "default": default})) || !w.refresh().await {
        return Ok(());
    }
    let mut due: Option<tokio::time::Instant> = None;
    loop {
        let settle = async {
            match due {
                Some(at) => tokio::time::sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        let alive = tokio::select! {
            line = lines.next() => match line {
                Some(l) if l.trim().is_empty() => true,
                Some(l) => w.command(l.trim()).await,
                None => false,
            },
            Some(ask) = asks.recv() => w.ask(ask).await,
            Some(line) = outcomes.recv() => {
                due = Some(tokio::time::Instant::now());
                (w.emit)(line)
            }
            Some(_) = signals.next() => {
                due.get_or_insert_with(|| tokio::time::Instant::now() + SETTLE);
                true
            }
            Some(change) = owners.next() => {
                // bluetoothd restarted: it forgot the agent.
                if change.args().is_ok_and(|a| a.new_owner().is_some()) {
                    let _ = register(&w.conn, w.default).await;
                }
                due = Some(tokio::time::Instant::now());
                true
            }
            () = settle => {
                due = None;
                w.refresh().await
            }
        };
        if !alive {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testbus::bluez::{Bluez, Pairing};
    use crate::testbus::Bus;

    struct Session {
        commands: mpsc::UnboundedSender<String>,
        events: mpsc::UnboundedReceiver<serde_json::Value>,
        task: tokio::task::JoinHandle<Result<()>>,
    }

    impl Session {
        async fn start(bus: &Bus, default: bool) -> Session {
            let (commands, lines) = mpsc::unbounded_channel::<String>();
            let (tx, events) = mpsc::unbounded_channel();
            let conn = bus.connect().await;
            let lines = lines_of(lines);
            let task = tokio::spawn(watch(conn, default, lines, move |v| tx.send(v).is_ok()));
            Session { commands, events, task }
        }

        fn send(&self, v: serde_json::Value) {
            self.commands.send(v.to_string()).unwrap();
        }

        async fn until(&mut self, want: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
            let wait = async {
                loop {
                    let v = self.events.recv().await.expect("the watch ended");
                    if want(&v) {
                        return v;
                    }
                }
            };
            tokio::time::timeout(Duration::from_secs(5), wait).await.expect("no such event within 5 s")
        }

        async fn state(&mut self, want: impl Fn(&Bluetooth) -> bool) -> serde_json::Value {
            self.until(|v| v["event"] == "state" && want(&serde_json::from_value(v.clone()).unwrap())).await
        }
    }

    fn lines_of(mut rx: mpsc::UnboundedReceiver<String>) -> impl Stream<Item = String> + Unpin {
        Box::pin(futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx)))
    }

    fn has(b: &Bluetooth, address: &str, f: impl Fn(&Device) -> bool) -> bool {
        b.devices.iter().any(|d| d.address == address && f(d))
    }

    const PAD: &str = "AA:BB:CC:00:00:01";
    const KEYBOARD: &str = "AA:BB:CC:00:00:02";
    const HEADSET: &str = "AA:BB:CC:00:00:03";

    // The adapter and the devices worth a row: paired ones, and found ones with a name; a nameless one is a beacon.
    #[tokio::test]
    async fn bluez_lists_the_adapter_and_its_named_devices() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(PAD, Some("Xbox Wireless Controller"), "input-gaming", true, Pairing::JustWorks).await;
        bluez.add(HEADSET, Some("WH-1000XM4"), "audio-headset", false, Pairing::JustWorks).await;
        bluez.add("AA:BB:CC:00:00:09", None, "", false, Pairing::JustWorks).await;
        bluez.battery(PAD, 120).await;
        let conn = bus.connect().await;
        let state = read(&conn).await;
        assert!(state.available && state.powered && !state.discovering);
        let rows: Vec<(&str, &str, bool, Option<u8>)> = state.devices.iter().map(|d| (d.address.as_str(), d.kind.as_str(), d.paired, d.battery)).collect();
        assert_eq!(rows, vec![(PAD, "pad", true, Some(100)), (HEADSET, "audio", false, None)], "paired first, the beacon left out, the battery capped");
    }

    #[tokio::test]
    async fn no_bluez_reads_as_unavailable() {
        let bus = Bus::start();
        let conn = bus.connect().await;
        assert_eq!(read(&conn).await, Bluetooth::default());
        assert!(matches!(set_powered(&conn, true).await, Err(Error::Io(_) | Error::Unavailable(_))));
    }

    // A just-works pad pairs from the list: Pair, Trusted so it comes back on its own, then Connect.
    #[tokio::test]
    async fn a_pad_pairs_trusts_and_connects() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(PAD, Some("Pro Controller"), "input-gaming", false, Pairing::JustWorks).await;
        let mut s = Session::start(&bus, false).await;
        assert_eq!(s.until(|v| v["event"] == "ready").await["agent"], true);
        s.send(json!({"cmd": "scan", "on": true}));
        s.state(|b| b.discovering).await;
        s.send(json!({"cmd": "pair", "address": PAD}));
        let done = s.until(|v| v["event"] == "done" || v["event"] == "failed").await;
        assert_eq!((&done["event"], &done["action"], &done["connected"]), (&json!("done"), &json!("pair"), &json!(true)), "{done}");
        s.state(|b| has(b, PAD, |d| d.paired && d.trusted && d.connected)).await;
        s.send(json!({"cmd": "quit"}));
        s.task.await.unwrap().unwrap();
    }

    // A keyboard's passkey comes to the watch's own agent, not the default one: BlueZ asks the caller's agent first.
    #[tokio::test]
    async fn a_pairing_asks_the_watchs_own_agent_and_takes_its_answer() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(KEYBOARD, Some("K380"), "input-keyboard", false, Pairing::Confirm(4242)).await;
        let mut s = Session::start(&bus, false).await;
        s.until(|v| v["event"] == "ready").await;
        assert_eq!(bluez.default_agent(), None, "a desktop keeps its own default agent");
        s.send(json!({"cmd": "pair", "address": KEYBOARD}));
        let ask = s.until(|v| v["event"] == "request").await;
        assert_eq!((&ask["kind"], &ask["code"], &ask["address"], &ask["name"]), (&json!("confirm"), &json!("004242"), &json!(KEYBOARD), &json!("K380")));
        s.send(json!({"cmd": "answer", "id": ask["id"], "yes": true}));
        let done = s.until(|v| v["event"] == "done" || v["event"] == "failed").await;
        assert_eq!(done["event"], "done", "{done}");
        s.state(|b| has(b, KEYBOARD, |d| d.paired)).await;
    }

    #[tokio::test]
    async fn a_refused_passkey_leaves_the_device_unpaired() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(KEYBOARD, Some("K380"), "input-keyboard", false, Pairing::Confirm(4242)).await;
        let mut s = Session::start(&bus, false).await;
        s.send(json!({"cmd": "pair", "address": KEYBOARD}));
        let ask = s.until(|v| v["event"] == "request").await;
        s.send(json!({"cmd": "answer", "id": ask["id"], "yes": false}));
        let failed = s.until(|v| v["event"] == "failed").await;
        assert_eq!((&failed["action"], &failed["reason"]), (&json!("pair"), &json!("rejected")), "{failed}");
        assert!(!has(&read(&bus.connect().await).await, KEYBOARD, |d| d.paired));
    }

    // BlueZ gives up on a question it sent: the watch says so, and an answer coming after reaches no one.
    #[tokio::test]
    async fn a_question_bluez_cancels_closes() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(KEYBOARD, Some("K380"), "input-keyboard", false, Pairing::Confirm(1)).await;
        let mut s = Session::start(&bus, false).await;
        s.send(json!({"cmd": "pair", "address": KEYBOARD}));
        let ask = s.until(|v| v["event"] == "request").await;
        bluez.cancel_agent().await;
        s.until(|v| v["event"] == "cancel").await;
        s.send(json!({"cmd": "answer", "id": ask["id"], "yes": true}));
        let failed = s.until(|v| v["event"] == "failed").await;
        assert_eq!(failed["reason"], "canceled", "{failed}");
    }

    // In the Universe session the watch is the default agent: BlueZ's cable pairing of a plugged-in DualSense asks it.
    #[tokio::test]
    async fn the_session_agent_answers_a_cable_pairing() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        let pad = bluez.add(PAD, Some("DualSense Wireless Controller"), "input-gaming", false, Pairing::JustWorks).await;
        let mut s = Session::start(&bus, true).await;
        assert_eq!(s.until(|v| v["event"] == "ready").await["default"], true);
        assert!(bluez.default_agent().is_some());
        let asked = tokio::spawn({
            let bluez = bluez.clone();
            async move { bluez.authorize_service(&pad).await }
        });
        let ask = s.until(|v| v["event"] == "request").await;
        assert_eq!((&ask["kind"], &ask["device_kind"], &ask["name"]), (&json!("authorize"), &json!("pad"), &json!("DualSense Wireless Controller")));
        s.send(json!({"cmd": "answer", "id": ask["id"], "yes": true}));
        assert!(asked.await.unwrap().is_ok(), "BlueZ hears the yes");
    }

    #[tokio::test]
    async fn power_connect_disconnect_and_forget() {
        let bus = Bus::start();
        let bluez = Bluez::serve(&bus).await;
        bluez.add(HEADSET, Some("WH-1000XM4"), "audio-headset", true, Pairing::JustWorks).await;
        let mut s = Session::start(&bus, false).await;
        s.send(json!({"cmd": "connect", "address": HEADSET}));
        s.state(|b| has(b, HEADSET, |d| d.connected)).await;
        s.send(json!({"cmd": "disconnect", "address": HEADSET}));
        s.state(|b| has(b, HEADSET, |d| !d.connected)).await;
        s.send(json!({"cmd": "remove", "address": HEADSET}));
        s.state(|b| b.devices.is_empty()).await;
        s.send(json!({"cmd": "power", "on": false}));
        s.state(|b| !b.powered).await;
        s.send(json!({"cmd": "connect", "address": "00:00:00:00:00:00"}));
        let failed = s.until(|v| v["event"] == "failed").await;
        assert_eq!(failed["action"], "connect");
    }

    #[test]
    fn a_device_kind_comes_from_its_icon() {
        assert_eq!(kind_of("input-gaming"), "pad");
        assert_eq!(kind_of("audio-headphones"), "audio");
        assert_eq!(kind_of("input-keyboard"), "keyboard");
        assert_eq!(kind_of(""), "other");
    }
}
