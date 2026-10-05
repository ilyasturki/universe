use std::collections::HashMap;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::{Error, Result};

const NM: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const MODIFY_SYSTEM: &str = "org.freedesktop.NetworkManager.settings.modify.system";
const CALL: Duration = Duration::from_secs(5);
// A join: the 4-way handshake, then DHCP on a slow router.
const JOINING: Duration = Duration::from_secs(45);
const POLL: Duration = Duration::from_millis(250);
const SETTLE: Duration = Duration::from_millis(300);
// NM refuses a scan right after the last one; its own background scans fill in between.
const SCAN_EVERY: Duration = Duration::from_secs(20);

// NMDeviceType, NMDeviceState, NMActiveConnectionState, NMDeviceStateReason, NM80211ApFlags and NM80211ApSecurityFlags.
const TYPE_ETHERNET: u32 = 1;
const TYPE_WIFI: u32 = 2;
const DEVICE_ACTIVATED: u32 = 100;
const ACTIVE_ACTIVATED: u32 = 2;
const ACTIVE_DEACTIVATED: u32 = 4;
const REASON_NO_SECRETS: u32 = 7;
const REASON_SUPPLICANT_DISCONNECT: u32 = 8;
const AP_PRIVACY: u32 = 0x1;
const SEC_PSK: u32 = 0x100;
const SEC_8021X: u32 = 0x200;
const SEC_SAE: u32 = 0x400;
const SEC_OWE: u32 = 0x800 | 0x1000;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
    pub available: bool,
    pub enabled: bool,
    /// The Wi-Fi interface, empty on a machine with none.
    pub device: String,
    /// The link up: `wifi`, `wired` (a cable wins), or empty offline.
    pub link: String,
    pub ssid: String,
    /// The joined network's signal, 0-100.
    pub strength: u8,
    /// `full`, `limited`, `portal`, `none` or `unknown`, as NM last checked.
    pub connectivity: String,
    /// The networks in range, one per name at its strongest, the joined one first then by signal.
    pub networks: Vec<Wifi>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wifi {
    pub ssid: String,
    pub strength: u8,
    /// `open`, `owe`, `wep`, `psk`, `sae` or `enterprise`: Universe joins the first two and the WPA ones.
    pub security: String,
    pub saved: bool,
    pub active: bool,
}

type Props = HashMap<String, OwnedValue>;
type Settings = HashMap<String, HashMap<String, OwnedValue>>;

fn get<'a, T: TryFrom<&'a Value<'a>>>(props: &'a Props, key: &str) -> Option<T> {
    props.get(key).and_then(|v| T::try_from(v).ok())
}

fn path(props: &Props, key: &str) -> Option<OwnedObjectPath> {
    get::<ObjectPath>(props, key).filter(|p| p.as_str() != "/").map(OwnedObjectPath::from)
}

fn ssid_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn security_of(flags: u32, wpa: u32, rsn: u32) -> &'static str {
    let any = wpa | rsn;
    if any & SEC_8021X != 0 {
        "enterprise"
    } else if any & SEC_PSK != 0 {
        "psk"
    } else if any & SEC_SAE != 0 {
        "sae"
    } else if any & SEC_OWE != 0 {
        "owe"
    } else if flags & AP_PRIVACY != 0 {
        "wep"
    } else {
        "open"
    }
}

pub fn connectivity_of(state: u32) -> &'static str {
    match state {
        1 => "none",
        2 => "portal",
        3 => "limited",
        4 => "full",
        _ => "unknown",
    }
}

fn timeout() -> zbus::Error {
    zbus::Error::InputOutput(std::io::Error::from(std::io::ErrorKind::TimedOut).into())
}

async fn timed<T>(limit: Duration, call: impl std::future::Future<Output = zbus::Result<T>>) -> zbus::Result<T> {
    tokio::time::timeout(limit, call).await.unwrap_or_else(|_| Err(timeout()))
}

async fn proxy<'a>(conn: &zbus::Connection, path: impl Into<OwnedObjectPath>, iface: &'static str) -> zbus::Result<zbus::Proxy<'a>> {
    zbus::proxy::Builder::<zbus::Proxy>::new(conn)
        .destination(NM)?
        .path(path.into())?
        .interface(iface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

async fn all(conn: &zbus::Connection, path: &OwnedObjectPath, iface: &'static str) -> zbus::Result<Props> {
    let p = zbus::fdo::PropertiesProxy::builder(conn).destination(NM)?.path(path.clone())?.build().await?;
    Ok(p.get_all(zbus::names::InterfaceName::from_static_str_unchecked(iface)).await?)
}

fn root() -> OwnedObjectPath {
    ObjectPath::from_static_str_unchecked(NM_PATH).into()
}

struct Device {
    path: OwnedObjectPath,
    kind: u32,
    interface: String,
    up: bool,
}

async fn devices(conn: &zbus::Connection) -> zbus::Result<Vec<Device>> {
    let paths: Vec<OwnedObjectPath> = proxy(conn, root(), NM).await?.call("GetDevices", &()).await?;
    let mut out = vec![];
    for path in paths {
        let props = all(conn, &path, DEVICE).await?;
        out.push(Device {
            kind: get(&props, "DeviceType").unwrap_or(0),
            interface: get::<&str>(&props, "Interface").unwrap_or("").to_string(),
            up: get::<u32>(&props, "State") == Some(DEVICE_ACTIVATED),
            path,
        });
    }
    out.sort_by(|a, b| a.interface.cmp(&b.interface));
    Ok(out)
}

struct Ap {
    path: OwnedObjectPath,
    ssid: String,
    strength: u8,
    security: &'static str,
}

async fn access_points(conn: &zbus::Connection, device: &OwnedObjectPath) -> zbus::Result<Vec<Ap>> {
    let paths: Vec<OwnedObjectPath> = proxy(conn, device.clone(), WIRELESS).await?.call("GetAllAccessPoints", &()).await?;
    let mut out = vec![];
    for path in paths {
        // An access point can go between the listing and the read.
        let Ok(props) = all(conn, &path, ACCESS_POINT).await else { continue };
        let ssid = ssid_of(&props.get("Ssid").and_then(|v| <Vec<u8>>::try_from(v.try_clone().ok()?).ok()).unwrap_or_default());
        if ssid.is_empty() {
            continue;
        }
        let flags = |k| get::<u32>(&props, k).unwrap_or(0);
        out.push(Ap {
            path,
            ssid,
            strength: get(&props, "Strength").unwrap_or(0),
            security: security_of(flags("Flags"), flags("WpaFlags"), flags("RsnFlags")),
        });
    }
    Ok(out)
}

async fn saved(conn: &zbus::Connection) -> zbus::Result<Vec<(String, OwnedObjectPath)>> {
    let paths: Vec<OwnedObjectPath> = proxy(conn, ObjectPath::from_static_str_unchecked(SETTINGS_PATH), SETTINGS).await?.call("ListConnections", &()).await?;
    let mut out = vec![];
    for path in paths {
        let Ok(settings) = proxy(conn, path.clone(), CONNECTION).await?.call::<_, _, Settings>("GetSettings", &()).await else { continue };
        if let Some(ssid) = settings.get("802-11-wireless").and_then(|w| w.get("ssid")).and_then(|v| <Vec<u8>>::try_from(v.try_clone().ok()?).ok()) {
            out.push((ssid_of(&ssid), path));
        }
    }
    Ok(out)
}

/// The networks in range merged by name, the strongest of each kept.
fn merge(aps: &[Ap], saved: &[String], active: &str) -> Vec<Wifi> {
    let mut by_name: Vec<Wifi> = vec![];
    for ap in aps {
        match by_name.iter_mut().find(|w| w.ssid == ap.ssid) {
            Some(w) if w.strength >= ap.strength => {}
            Some(w) => {
                w.strength = ap.strength;
                w.security = ap.security.into();
            }
            None => by_name.push(Wifi {
                ssid: ap.ssid.clone(),
                strength: ap.strength,
                security: ap.security.into(),
                saved: saved.contains(&ap.ssid),
                active: ap.ssid == active,
            }),
        }
    }
    by_name.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)).then(a.ssid.cmp(&b.ssid)));
    by_name
}

async fn try_read(conn: &zbus::Connection) -> zbus::Result<Network> {
    let manager = all(conn, &root(), NM).await?;
    let devices = devices(conn).await?;
    let mut out = Network {
        available: true,
        enabled: get(&manager, "WirelessEnabled").unwrap_or(false),
        connectivity: connectivity_of(get(&manager, "Connectivity").unwrap_or(0)).into(),
        ..Network::default()
    };
    let wifi = devices.iter().find(|d| d.kind == TYPE_WIFI);
    if let Some(wifi) = wifi {
        out.device = wifi.interface.clone();
        let aps = access_points(conn, &wifi.path).await?;
        let current = path(&all(conn, &wifi.path, WIRELESS).await?, "ActiveAccessPoint").filter(|_| wifi.up);
        if let Some(ap) = current.and_then(|p| aps.iter().find(|a| a.path == p)) {
            out.ssid = ap.ssid.clone();
            out.strength = ap.strength;
        }
        let names: Vec<String> = saved(conn).await?.into_iter().map(|(ssid, _)| ssid).collect();
        out.networks = merge(&aps, &names, &out.ssid);
    }
    out.link = if devices.iter().any(|d| d.kind == TYPE_ETHERNET && d.up) {
        "wired".into()
    } else if wifi.is_some_and(|d| d.up) {
        "wifi".into()
    } else {
        String::new()
    };
    Ok(out)
}

pub async fn read(conn: &zbus::Connection) -> Network {
    match tokio::time::timeout(CALL * 2, try_read(conn)).await {
        Ok(Ok(state)) => state,
        _ => Network::default(),
    }
}

pub async fn system() -> Result<zbus::Connection> {
    zbus::Connection::system().await.map_err(|e| Error::Unavailable(format!("the system bus: {e}")))
}

pub async fn state() -> Network {
    match system().await {
        Ok(conn) => read(&conn).await,
        Err(_) => Network::default(),
    }
}

/// Why a join failed: `password` (NM had no secret it took), `permission`, `notfound`, `timeout` or `failed`.
#[derive(Debug)]
pub struct Failure {
    pub reason: &'static str,
    pub message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<zbus::Error> for Failure {
    fn from(e: zbus::Error) -> Failure {
        match e {
            zbus::Error::MethodError(name, message, _) => {
                let denied = name.as_str().ends_with("PermissionDenied");
                Failure { reason: if denied { "permission" } else { "failed" }, message: message.filter(|m| !m.is_empty()).unwrap_or_else(|| name.to_string()) }
            }
            zbus::Error::InputOutput(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                Failure { reason: "timeout", message: "NetworkManager did not answer".into() }
            }
            e => Failure { reason: "failed", message: format!("NetworkManager: {e}") },
        }
    }
}

impl From<Failure> for Error {
    fn from(f: Failure) -> Error {
        match f.reason {
            "notfound" => Error::NotFound(f.message),
            "permission" | "password" => Error::Invalid(f.message),
            _ => Error::Unavailable(f.message),
        }
    }
}

fn failure(reason: &'static str, message: impl Into<String>) -> Failure {
    Failure { reason, message: message.into() }
}

fn user_name() -> String {
    // SAFETY: getpwuid_r fills `pw` from `buf`, both outliving the read of pw_name.
    unsafe {
        let mut pw: libc::passwd = std::mem::zeroed();
        let mut found = std::ptr::null_mut();
        let mut buf = vec![0 as libc::c_char; 4096];
        if libc::getpwuid_r(libc::getuid(), &mut pw, buf.as_mut_ptr(), buf.len(), &mut found) == 0 && !found.is_null() {
            return std::ffi::CStr::from_ptr(pw.pw_name).to_string_lossy().into_owned();
        }
    }
    std::env::var("USER").unwrap_or_default()
}

/// A new connection for `ssid`: the password kept in it (`psk-flags` 0) for a session with no secret agent; owned by `owner`
/// alone when it is set, else system-wide.
pub fn settings_for(ssid: &str, security: &str, password: Option<&str>, owner: Option<&str>) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
    let mut connection: HashMap<&str, Value> = HashMap::from([("type", Value::from("802-11-wireless")), ("id", Value::from(ssid.to_string()))]);
    if let Some(user) = owner {
        connection.insert("permissions", Value::from(vec![format!("user:{user}")]));
    }
    let wireless = HashMap::from([("ssid", Value::from(ssid.as_bytes().to_vec())), ("mode", Value::from("infrastructure"))]);
    let mut out = HashMap::from([("connection", connection), ("802-11-wireless", wireless)]);
    let key_mgmt = match security {
        "psk" => Some("wpa-psk"),
        "sae" => Some("sae"),
        "owe" => Some("owe"),
        _ => None,
    };
    if let Some(key_mgmt) = key_mgmt {
        let mut security = HashMap::from([("key-mgmt", Value::from(key_mgmt))]);
        if let Some(psk) = password.filter(|_| key_mgmt != "owe") {
            security.insert("psk", Value::from(psk.to_string()));
            security.insert("psk-flags", Value::from(0u32));
        }
        out.insert("802-11-wireless-security", security);
    }
    out
}

async fn may_save_for_all(conn: &zbus::Connection) -> bool {
    let asked = timed(CALL, async { proxy(conn, root(), NM).await?.call::<_, _, HashMap<String, String>>("GetPermissions", &()).await }).await;
    asked.is_ok_and(|p| p.get(MODIFY_SYSTEM).is_some_and(|v| v == "yes"))
}

async fn wifi_device(conn: &zbus::Connection) -> std::result::Result<OwnedObjectPath, Failure> {
    devices(conn).await?.into_iter().find(|d| d.kind == TYPE_WIFI).map(|d| d.path).ok_or_else(|| failure("notfound", "no Wi-Fi device"))
}

async fn activated(conn: &zbus::Connection, active: &OwnedObjectPath, device: &OwnedObjectPath) -> std::result::Result<(), Failure> {
    let deadline = tokio::time::Instant::now() + JOINING;
    let ac = proxy(conn, active.clone(), ACTIVE).await?;
    loop {
        // The object goes as soon as the activation fails: no state left is a failure too.
        let state = timed(CALL, async { ac.get_property::<u32>("State").await }).await.unwrap_or(ACTIVE_DEACTIVATED);
        if state == ACTIVE_ACTIVATED {
            return Ok(());
        }
        if state == ACTIVE_DEACTIVATED {
            let reason = timed(CALL, async { proxy(conn, device.clone(), DEVICE).await?.get_property::<(u32, u32)>("StateReason").await })
                .await
                .map(|(_, reason)| reason)
                .unwrap_or(0);
            return Err(match reason {
                REASON_NO_SECRETS | REASON_SUPPLICANT_DISCONNECT => failure("password", "the password was not accepted"),
                _ => failure("failed", format!("NetworkManager could not join the network (reason {reason})")),
            });
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(failure("timeout", "the network did not answer in time"));
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Joins `ssid`: a saved network as saved (with `password` it is saved again with it), a new one added with `password`. A
/// password the network refuses leaves no new connection behind. No call may ask polkit for a password: where NM wants an
/// admin, it refuses.
pub async fn connect(conn: &zbus::Connection, ssid: &str, password: Option<&str>) -> std::result::Result<(), Failure> {
    let device = wifi_device(conn).await?;
    let aps = access_points(conn, &device).await?;
    let ap = aps.iter().filter(|a| a.ssid == ssid).max_by_key(|a| a.strength).ok_or_else(|| failure("notfound", format!("{ssid} is not in range")))?;
    let manager = proxy(conn, root(), NM).await?;
    let known = saved(conn).await?.into_iter().find(|(name, _)| name == ssid).map(|(_, path)| path);
    let (active, added) = match known {
        Some(saved) => {
            if let Some(psk) = password {
                let c = proxy(conn, saved.clone(), CONNECTION).await?;
                let mut settings: Settings = timed(CALL, c.call("GetSettings", &())).await?;
                let security = settings.entry("802-11-wireless-security".into()).or_default();
                security.insert("psk".into(), OwnedValue::try_from(Value::from(psk.to_string())).map_err(zbus::Error::from)?);
                security.insert("psk-flags".into(), OwnedValue::from(0u32));
                timed(CALL, c.call::<_, _, ()>("Update", &(settings,))).await?;
            }
            let active: OwnedObjectPath = timed(CALL, manager.call("ActivateConnection", &(&saved, &device, &ap.path))).await?;
            (active, None)
        }
        None => {
            if matches!(ap.security, "psk" | "sae") && password.is_none_or(str::is_empty) {
                return Err(failure("password", format!("{ssid} needs a password")));
            }
            if matches!(ap.security, "wep" | "enterprise") {
                return Err(failure("failed", format!("{ssid} uses {} security, which Universe does not set up", ap.security)));
            }
            let owner = if may_save_for_all(conn).await { None } else { Some(user_name()) };
            let settings = settings_for(ssid, ap.security, password, owner.as_deref());
            let (saved, active): (OwnedObjectPath, OwnedObjectPath) =
                timed(CALL, manager.call("AddAndActivateConnection", &(settings, &device, &ap.path))).await?;
            (active, Some(saved))
        }
    };
    let joined = activated(conn, &active, &device).await;
    if let (Err(f), Some(saved)) = (&joined, added) {
        if f.reason == "password" {
            let _ = timed(CALL, async { proxy(conn, saved, CONNECTION).await?.call::<_, _, ()>("Delete", &()).await }).await;
        }
    }
    joined
}

/// Deletes every saved connection to `ssid`.
pub async fn forget(conn: &zbus::Connection, ssid: &str) -> std::result::Result<(), Failure> {
    let paths: Vec<OwnedObjectPath> = saved(conn).await?.into_iter().filter(|(name, _)| name == ssid).map(|(_, p)| p).collect();
    if paths.is_empty() {
        return Err(failure("notfound", format!("{ssid} is not saved")));
    }
    for path in paths {
        timed(CALL, async { proxy(conn, path, CONNECTION).await?.call::<_, _, ()>("Delete", &()).await }).await?;
    }
    Ok(())
}

pub async fn set_enabled(conn: &zbus::Connection, on: bool) -> std::result::Result<(), Failure> {
    Ok(timed(CALL, async { Ok(proxy(conn, root(), NM).await?.set_property("WirelessEnabled", on).await?) }).await?)
}

pub async fn scan(conn: &zbus::Connection) -> std::result::Result<(), Failure> {
    let device = wifi_device(conn).await?;
    let options: HashMap<&str, Value> = HashMap::new();
    Ok(timed(CALL, async { proxy(conn, device, WIRELESS).await?.call::<_, _, ()>("RequestScan", &(options,)).await }).await?)
}

pub async fn check(conn: &zbus::Connection) -> std::result::Result<String, Failure> {
    let state: u32 = timed(Duration::from_secs(30), async { proxy(conn, root(), NM).await?.call("CheckConnectivity", &()).await }).await?;
    Ok(connectivity_of(state).into())
}

struct Watch<E> {
    conn: zbus::Connection,
    emit: E,
    shown: Option<Network>,
    scanning: bool,
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

    fn join(&self, ssid: String, password: Option<String>) {
        let conn = self.conn.clone();
        let done = self.done.clone();
        tokio::spawn(async move {
            let line = match connect(&conn, &ssid, password.as_deref()).await {
                Ok(()) => json!({"event": "done", "action": "connect", "ssid": ssid, "connectivity": check(&conn).await.unwrap_or_else(|_| "unknown".into())}),
                Err(f) => json!({"event": "failed", "action": "connect", "ssid": ssid, "reason": f.reason, "message": f.message}),
            };
            let _ = done.send(line);
        });
    }

    fn test_link(&self) {
        let conn = self.conn.clone();
        let done = self.done.clone();
        tokio::spawn(async move {
            let line = match check(&conn).await {
                Ok(c) => json!({"event": "done", "action": "check", "connectivity": c}),
                Err(f) => json!({"event": "failed", "action": "check", "reason": f.reason, "message": f.message}),
            };
            let _ = done.send(line);
        });
    }

    async fn command(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return (self.emit)(json!({"event": "error", "message": "bad command"}));
        };
        let ssid = v["ssid"].as_str().unwrap_or("").to_string();
        let on = v["on"].as_bool().unwrap_or(true);
        let (action, outcome) = match v["cmd"].as_str().unwrap_or("") {
            "scan" => {
                self.scanning = on;
                if !on {
                    return true;
                }
                ("scan", scan(&self.conn).await.map(|()| json!({})))
            }
            "wifi" => ("wifi", set_enabled(&self.conn, on).await.map(|()| json!({}))),
            "connect" => {
                self.join(ssid.clone(), v["password"].as_str().map(str::to_string));
                return (self.emit)(json!({"event": "connecting", "ssid": ssid}));
            }
            "forget" => ("forget", forget(&self.conn, &ssid).await.map(|()| json!({"ssid": ssid}))),
            "check" => {
                self.test_link();
                return true;
            }
            "refresh" => {
                self.shown = None;
                return self.refresh().await;
            }
            "quit" => return false,
            other => return (self.emit)(json!({"event": "error", "message": format!("unknown command {other}")})),
        };
        let mut line = match outcome {
            Ok(extra) => extra,
            Err(f) => json!({"event": "failed", "reason": f.reason, "message": f.message, "ssid": ssid}),
        };
        line["action"] = json!(action);
        if line.get("event").is_none() {
            line["event"] = json!("done");
        }
        (self.emit)(line)
    }
}

/// `universe network watch`: the state on start and after every change, each command's outcome (a join's once it is up or
/// has failed); commands in `lines`, one JSON object each. Ends when `lines` does, on `quit`, or once `emit` says no one listens.
pub async fn watch<L, E>(conn: zbus::Connection, mut lines: L, emit: E) -> Result<()>
where
    L: Stream<Item = String> + Unpin,
    E: FnMut(serde_json::Value) -> bool,
{
    let failed = |e: zbus::Error| Error::Unavailable(format!("NetworkManager: {e}"));
    let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).sender(NM).map_err(failed)?.build();
    let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, None).await.map_err(failed)?;
    let (done, mut outcomes) = mpsc::unbounded_channel();
    let mut w = Watch { conn, emit, shown: None, scanning: false, done };
    if !(w.emit)(json!({"event": "ready"})) || !w.refresh().await {
        return Ok(());
    }
    let mut due: Option<tokio::time::Instant> = None;
    let mut rescan = tokio::time::interval(SCAN_EVERY);
    rescan.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
            Some(line) = outcomes.recv() => {
                due = Some(tokio::time::Instant::now());
                (w.emit)(line)
            }
            Some(_) = signals.next() => {
                due.get_or_insert_with(|| tokio::time::Instant::now() + SETTLE);
                true
            }
            _ = rescan.tick(), if w.scanning => {
                let _ = scan(&w.conn).await;
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
    use crate::testbus::nm::{Nm, Router};
    use crate::testbus::Bus;

    struct Session {
        commands: mpsc::UnboundedSender<String>,
        events: mpsc::UnboundedReceiver<serde_json::Value>,
    }

    impl Session {
        async fn start(bus: &Bus) -> Session {
            let (commands, mut lines) = mpsc::unbounded_channel::<String>();
            let (tx, events) = mpsc::unbounded_channel();
            let conn = bus.connect().await;
            let lines = Box::pin(futures_util::stream::poll_fn(move |cx| lines.poll_recv(cx)));
            tokio::spawn(watch(conn, lines, move |v| tx.send(v).is_ok()));
            Session { commands, events }
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

        async fn state(&mut self, want: impl Fn(&Network) -> bool) -> Network {
            let v = self.until(|v| v["event"] == "state" && want(&serde_json::from_value(v.clone()).unwrap())).await;
            serde_json::from_value(v).unwrap()
        }

        async fn outcome(&mut self) -> serde_json::Value {
            self.until(|v| v["event"] == "done" || v["event"] == "failed").await
        }
    }

    fn ap(ssid: &str, strength: u8, security: &'static str) -> Ap {
        Ap { path: ObjectPath::from_static_str_unchecked("/ap").into(), ssid: ssid.into(), strength, security }
    }

    #[test]
    fn security_reads_the_access_points_flags() {
        assert_eq!(security_of(0, 0, 0), "open");
        assert_eq!(security_of(AP_PRIVACY, 0, 0), "wep");
        assert_eq!(security_of(AP_PRIVACY, 0, SEC_PSK), "psk");
        assert_eq!(security_of(AP_PRIVACY, 0, SEC_PSK | SEC_SAE), "psk", "WPA3 transition mode takes a WPA2 password");
        assert_eq!(security_of(AP_PRIVACY, 0, SEC_SAE), "sae");
        assert_eq!(security_of(AP_PRIVACY, SEC_8021X, SEC_8021X), "enterprise");
        assert_eq!(security_of(0, 0, 0x800), "owe");
    }

    #[test]
    fn networks_merge_by_name_at_their_strongest() {
        let aps = [ap("Cafe", 40, "open"), ap("Home", 30, "psk"), ap("Home", 80, "psk"), ap("Attic", 90, "sae")];
        let got: Vec<(String, u8, bool, bool)> = merge(&aps, &["Home".into()], "Home").into_iter().map(|w| (w.ssid, w.strength, w.saved, w.active)).collect();
        assert_eq!(got, vec![("Home".into(), 80, true, true), ("Attic".into(), 90, false, false), ("Cafe".into(), 40, false, false)]);
    }

    #[test]
    fn a_new_connection_keeps_its_password_and_names_its_owner() {
        let s = settings_for("Home", "psk", Some("hunter22"), Some("yasso"));
        assert_eq!(s["connection"]["permissions"], Value::from(vec!["user:yasso".to_string()]));
        assert_eq!(s["802-11-wireless"]["ssid"], Value::from(b"Home".to_vec()));
        assert_eq!(s["802-11-wireless-security"]["key-mgmt"], Value::from("wpa-psk"));
        assert_eq!(s["802-11-wireless-security"]["psk"], Value::from("hunter22"));
        assert_eq!(s["802-11-wireless-security"]["psk-flags"], Value::from(0u32), "no secret agent in the session: the system keeps it");
        let open = settings_for("Cafe", "open", None, None);
        assert!(!open["connection"].contains_key("permissions"), "saved for everyone");
        assert!(!open.contains_key("802-11-wireless-security"));
        assert_eq!(settings_for("Attic", "sae", Some("x"), None)["802-11-wireless-security"]["key-mgmt"], Value::from("sae"));
    }

    #[tokio::test]
    async fn the_networks_in_range_and_the_link() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router::default()).await;
        nm.access_point("Home", 70, "psk").await;
        nm.access_point("Home", 40, "psk").await;
        nm.access_point("Cafe", 55, "open").await;
        nm.access_point("", 90, "psk").await;
        nm.saved("Home", "hunter22").await;
        let state = read(&bus.connect().await).await;
        assert!(state.available && state.enabled);
        assert_eq!((state.device.as_str(), state.link.as_str(), state.connectivity.as_str()), ("wlan0", "", "none"));
        let rows: Vec<(&str, u8, &str, bool)> = state.networks.iter().map(|w| (w.ssid.as_str(), w.strength, w.security.as_str(), w.saved)).collect();
        assert_eq!(rows, vec![("Home", 70, "psk", true), ("Cafe", 55, "open", false)], "a hidden network has no name to list");
        nm.cable(true).await;
        assert_eq!(read(&bus.connect().await).await.link, "wired");
    }

    #[tokio::test]
    async fn no_networkmanager_reads_as_unavailable() {
        let bus = Bus::start();
        let conn = bus.connect().await;
        assert_eq!(read(&conn).await, Network::default());
        assert!(connect(&conn, "Home", None).await.is_err());
    }

    // A new network joins with its password and is saved for this user alone where NM wants an admin for everyone's.
    #[tokio::test]
    async fn a_new_network_joins_and_is_saved_for_the_user() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router { password: "hunter22".into(), system: false }).await;
        nm.access_point("Home", 70, "psk").await;
        let mut s = Session::start(&bus).await;
        s.state(|n| n.networks.len() == 1).await;
        s.send(json!({"cmd": "connect", "ssid": "Home", "password": "hunter22"}));
        assert_eq!(s.until(|v| v["event"] == "connecting").await["ssid"], "Home");
        let done = s.outcome().await;
        assert_eq!((&done["event"], &done["connectivity"]), (&json!("done"), &json!("full")), "{done}");
        let state = s.state(|n| n.link == "wifi").await;
        assert_eq!((state.ssid.as_str(), state.networks[0].saved, state.networks[0].active), ("Home", true, true));
        let saved = nm.connections();
        assert_eq!((saved.len(), saved[0].ssid.as_str()), (1, "Home"));
        assert_eq!(saved[0].permissions, vec![format!("user:{}", user_name())]);
        assert_eq!((saved[0].psk.as_str(), saved[0].psk_flags), ("hunter22", Some(0)));
        assert!(!nm.interactive(), "nothing asked polkit to prompt");
    }

    #[tokio::test]
    async fn where_nm_allows_it_a_network_is_saved_for_everyone() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router { password: "hunter22".into(), system: true }).await;
        nm.access_point("Home", 70, "psk").await;
        connect(&bus.connect().await, "Home", Some("hunter22")).await.unwrap();
        assert_eq!(nm.connections()[0].permissions, Vec::<String>::new());
    }

    // A wrong password is its own failure, and leaves no half-made connection behind.
    #[tokio::test]
    async fn a_wrong_password_fails_as_such_and_saves_nothing() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router { password: "hunter22".into(), system: false }).await;
        nm.access_point("Home", 70, "psk").await;
        let mut s = Session::start(&bus).await;
        s.send(json!({"cmd": "connect", "ssid": "Home", "password": "hunter2"}));
        let failed = s.outcome().await;
        assert_eq!((&failed["event"], &failed["reason"]), (&json!("failed"), &json!("password")), "{failed}");
        assert!(nm.connections().is_empty());
        let missing = connect(&bus.connect().await, "Home", None).await.unwrap_err();
        assert_eq!(missing.reason, "password", "a secured network is not tried without one");
    }

    // A network saved with its secret in a desktop's keyring asks again; the password typed here is saved into it.
    #[tokio::test]
    async fn a_saved_network_joins_as_saved_or_takes_a_new_password() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router { password: "hunter22".into(), system: false }).await;
        nm.access_point("Home", 70, "psk").await;
        nm.saved("Home", "").await;
        let conn = bus.connect().await;
        assert_eq!(connect(&conn, "Home", None).await.unwrap_err().reason, "password");
        connect(&conn, "Home", Some("hunter22")).await.unwrap();
        let saved = nm.connections();
        assert_eq!(saved.len(), 1, "the saved one, not a second");
        assert_eq!((saved[0].psk.as_str(), saved[0].psk_flags), ("hunter22", Some(0)));
    }

    #[tokio::test]
    async fn wifi_switches_scans_and_forgets() {
        let bus = Bus::start();
        let nm = Nm::serve(&bus, Router::default()).await;
        nm.access_point("Home", 70, "psk").await;
        nm.saved("Home", "hunter22").await;
        let mut s = Session::start(&bus).await;
        s.state(|n| n.enabled).await;
        s.send(json!({"cmd": "scan"}));
        assert_eq!(s.outcome().await["action"], "scan");
        assert_eq!(nm.scans(), 1);
        s.send(json!({"cmd": "check"}));
        let checked = s.outcome().await;
        assert_eq!((&checked["action"], &checked["connectivity"]), (&json!("check"), &json!("none")), "{checked}");
        s.send(json!({"cmd": "wifi", "on": false}));
        s.state(|n| !n.enabled).await;
        s.send(json!({"cmd": "forget", "ssid": "Home"}));
        assert_eq!(s.outcome().await["event"], "done");
        s.state(|n| n.networks.iter().all(|w| !w.saved)).await;
        s.send(json!({"cmd": "forget", "ssid": "Home"}));
        assert_eq!(s.outcome().await["reason"], "notfound");
    }
}
