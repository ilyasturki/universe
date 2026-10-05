use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::Bus;

const ROOT: &str = "/org/freedesktop/NetworkManager";
const SETTINGS: &str = "/org/freedesktop/NetworkManager/Settings";
const ETHERNET: &str = "/org/freedesktop/NetworkManager/Devices/1";
const WIFI: &str = "/org/freedesktop/NetworkManager/Devices/2";

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager")]
pub enum NmError {
    #[zbus(error)]
    ZBus(zbus::Error),
    PermissionDenied(String),
    UnknownConnection(String),
}

/// The access points' side: the password they take, and whether NM lets this user save a connection for everyone.
#[derive(Clone)]
pub struct Router {
    pub password: String,
    pub system: bool,
}

impl Default for Router {
    fn default() -> Router {
        Router { password: "hunter22".into(), system: false }
    }
}

#[derive(Debug)]
pub struct Saved {
    pub ssid: String,
    pub permissions: Vec<String>,
    pub psk: String,
    pub psk_flags: Option<u32>,
}

#[derive(Default)]
struct State {
    router: Router,
    aps: Vec<(OwnedObjectPath, String, &'static str)>,
    connections: BTreeMap<u32, Settings>,
    next: u32,
    scans: u32,
    interactive: bool,
}

type Shared = Arc<Mutex<State>>;

fn object(path: String) -> OwnedObjectPath {
    ObjectPath::try_from(path).unwrap().into()
}

fn none() -> OwnedObjectPath {
    ObjectPath::from_static_str_unchecked("/").into()
}

fn ssid_in(settings: &Settings) -> String {
    let bytes = settings.get("802-11-wireless").and_then(|w| w.get("ssid")).and_then(|v| <Vec<u8>>::try_from(v.try_clone().ok()?).ok());
    String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned()
}

fn psk_in(settings: &Settings) -> String {
    settings.get("802-11-wireless-security").and_then(|s| s.get("psk")).and_then(|v| <&str>::try_from(v).ok()).unwrap_or("").to_string()
}

fn noted(state: &Shared, hdr: &Header<'_>) {
    if hdr.primary().flags().contains(zbus::message::Flags::AllowInteractiveAuth) {
        state.lock().unwrap().interactive = true;
    }
}

struct Manager {
    state: Shared,
    wireless_enabled: bool,
    connectivity: u32,
}

impl Manager {
    /// The join plays out a moment later, as on a real link: up when the password is the router's, else no secrets.
    fn activate(&self, conn: &zbus::Connection, connection: OwnedObjectPath, ap: OwnedObjectPath, ok: bool) -> OwnedObjectPath {
        let id = {
            let mut s = self.state.lock().unwrap();
            s.next += 1;
            s.next
        };
        let active = object(format!("{ROOT}/ActiveConnection/{id}"));
        let conn = conn.clone();
        let path = active.clone();
        tokio::spawn(async move {
            let server = conn.object_server();
            server.at(&path, Active { state: 1, connection }).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
            let wifi = server.interface::<_, Device>(WIFI).await.unwrap();
            if ok {
                let ac = server.interface::<_, Active>(&path).await.unwrap();
                ac.get_mut().await.state = 2;
                ac.get().await.state_changed(ac.signal_emitter()).await.unwrap();
                wifi.get_mut().await.state = 100;
                wifi.get().await.state_changed(wifi.signal_emitter()).await.unwrap();
                let radio = server.interface::<_, Wireless>(WIFI).await.unwrap();
                radio.get_mut().await.active = ap;
                radio.get().await.active_access_point_changed(radio.signal_emitter()).await.unwrap();
                let manager = server.interface::<_, Manager>(ROOT).await.unwrap();
                manager.get_mut().await.connectivity = 4;
                manager.get().await.connectivity_changed(manager.signal_emitter()).await.unwrap();
            } else {
                wifi.get_mut().await.reason = (30, 7);
                server.remove::<Active, _>(&path).await.unwrap();
            }
        });
        active
    }

    fn ap_ok(&self, ap: &OwnedObjectPath, psk: &str) -> bool {
        let s = self.state.lock().unwrap();
        let open = s.aps.iter().any(|(p, _, security)| p == ap && *security == "open");
        open || psk == s.router.password
    }
}

#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![object(ETHERNET.into()), object(WIFI.into())]
    }

    fn get_permissions(&self, #[zbus(header)] hdr: Header<'_>) -> HashMap<String, String> {
        noted(&self.state, &hdr);
        let system = if self.state.lock().unwrap().router.system { "yes" } else { "auth" };
        HashMap::from([
            ("org.freedesktop.NetworkManager.settings.modify.system".into(), system.into()),
            ("org.freedesktop.NetworkManager.settings.modify.own".into(), "yes".into()),
        ])
    }

    async fn add_and_activate_connection(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
        settings: Settings,
        _device: OwnedObjectPath,
        ap: OwnedObjectPath,
    ) -> Result<(OwnedObjectPath, OwnedObjectPath), NmError> {
        noted(&self.state, &hdr);
        let own = settings
            .get("connection")
            .and_then(|c| c.get("permissions"))
            .and_then(|v| <Vec<String>>::try_from(v.try_clone().ok()?).ok())
            .is_some_and(|p| !p.is_empty());
        if !own && !self.state.lock().unwrap().router.system {
            return Err(NmError::PermissionDenied("Insufficient privileges".into()));
        }
        let ok = self.ap_ok(&ap, &psk_in(&settings));
        let saved = Nm::add(conn, &self.state, settings).await;
        Ok((saved.clone(), self.activate(conn, saved, ap, ok)))
    }

    fn activate_connection(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
        connection: OwnedObjectPath,
        _device: OwnedObjectPath,
        ap: OwnedObjectPath,
    ) -> Result<OwnedObjectPath, NmError> {
        noted(&self.state, &hdr);
        let id: u32 = connection.as_str().rsplit('/').next().and_then(|n| n.parse().ok()).unwrap_or(0);
        let psk = self.state.lock().unwrap().connections.get(&id).map(psk_in).ok_or_else(|| NmError::UnknownConnection("no such connection".into()))?;
        let ok = self.ap_ok(&ap, &psk);
        Ok(self.activate(conn, connection, ap, ok))
    }

    fn check_connectivity(&self) -> u32 {
        self.connectivity
    }

    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.wireless_enabled
    }

    #[zbus(property)]
    fn set_wireless_enabled(&mut self, on: bool) {
        self.wireless_enabled = on;
    }

    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        self.connectivity
    }
}

struct Device {
    kind: u32,
    interface: &'static str,
    state: u32,
    reason: (u32, u32),
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        self.kind
    }

    #[zbus(property)]
    fn interface(&self) -> String {
        self.interface.into()
    }

    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }

    #[zbus(property)]
    fn state_reason(&self) -> (u32, u32) {
        self.reason
    }
}

struct Wireless {
    state: Shared,
    active: OwnedObjectPath,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    fn get_all_access_points(&self) -> Vec<OwnedObjectPath> {
        self.state.lock().unwrap().aps.iter().map(|(p, _, _)| p.clone()).collect()
    }

    fn request_scan(&self, _options: HashMap<String, OwnedValue>) {
        self.state.lock().unwrap().scans += 1;
    }

    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        self.active.clone()
    }
}

struct AccessPoint {
    ssid: String,
    strength: u8,
    security: &'static str,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        self.ssid.as_bytes().to_vec()
    }

    #[zbus(property)]
    fn strength(&self) -> u8 {
        self.strength
    }

    #[zbus(property)]
    fn flags(&self) -> u32 {
        u32::from(self.security != "open")
    }

    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }

    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        match self.security {
            "psk" => 0x100,
            "sae" => 0x400,
            _ => 0,
        }
    }
}

struct SettingsIface {
    state: Shared,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings")]
impl SettingsIface {
    fn list_connections(&self) -> Vec<OwnedObjectPath> {
        self.state.lock().unwrap().connections.keys().map(|id| object(format!("{SETTINGS}/{id}"))).collect()
    }

    #[zbus(signal)]
    async fn connection_removed(emitter: &SignalEmitter<'_>, connection: ObjectPath<'_>) -> zbus::Result<()>;
}

struct Connection {
    state: Shared,
    id: u32,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Connection {
    fn get_settings(&self) -> Settings {
        let state = self.state.lock().unwrap();
        let mut settings: Settings =
            state.connections[&self.id].iter().map(|(k, v)| (k.clone(), v.iter().map(|(k, v)| (k.clone(), v.try_clone().unwrap())).collect())).collect();
        // NM hands out no secret through GetSettings.
        if let Some(security) = settings.get_mut("802-11-wireless-security") {
            security.remove("psk");
        }
        settings
    }

    fn update(&self, #[zbus(header)] hdr: Header<'_>, settings: Settings) {
        noted(&self.state, &hdr);
        self.state.lock().unwrap().connections.insert(self.id, settings);
    }

    async fn delete(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &zbus::ObjectServer,
        #[zbus(connection)] conn: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        noted(&self.state, &hdr);
        self.state.lock().unwrap().connections.remove(&self.id);
        let path = object(format!("{SETTINGS}/{}", self.id));
        let emitter = SignalEmitter::new(conn, SETTINGS)?;
        SettingsIface::connection_removed(&emitter, path.as_ref()).await?;
        let server = server.clone();
        tokio::spawn(async move { server.remove::<Connection, _>(&path).await });
        Ok(())
    }
}

struct Active {
    state: u32,
    connection: OwnedObjectPath,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.state
    }

    #[zbus(property)]
    fn connection(&self) -> OwnedObjectPath {
        self.connection.clone()
    }
}

/// NetworkManager on the test bus: a cable port and a Wi-Fi card, the access points and saved networks a test adds.
pub struct Nm {
    conn: zbus::Connection,
    state: Shared,
}

impl Nm {
    pub async fn serve(bus: &Bus, router: Router) -> Nm {
        let conn = bus.connect().await;
        let state = Shared::new(Mutex::new(State { router, ..State::default() }));
        let server = conn.object_server();
        server.at(ROOT, Manager { state: state.clone(), wireless_enabled: true, connectivity: 1 }).await.unwrap();
        server.at(ETHERNET, Device { kind: 1, interface: "eth0", state: 20, reason: (0, 0) }).await.unwrap();
        server.at(WIFI, Device { kind: 2, interface: "wlan0", state: 30, reason: (0, 0) }).await.unwrap();
        server.at(WIFI, Wireless { state: state.clone(), active: none() }).await.unwrap();
        server.at(SETTINGS, SettingsIface { state: state.clone() }).await.unwrap();
        conn.request_name("org.freedesktop.NetworkManager").await.unwrap();
        Nm { conn, state }
    }

    async fn add(conn: &zbus::Connection, state: &Shared, settings: Settings) -> OwnedObjectPath {
        let id = {
            let mut s = state.lock().unwrap();
            s.next += 1;
            let id = s.next;
            s.connections.insert(id, settings);
            id
        };
        let path = object(format!("{SETTINGS}/{id}"));
        conn.object_server().at(&path, Connection { state: state.clone(), id }).await.unwrap();
        path
    }

    pub async fn access_point(&self, ssid: &str, strength: u8, security: &'static str) {
        let path = {
            let mut s = self.state.lock().unwrap();
            s.next += 1;
            let path = object(format!("{ROOT}/AccessPoint/{}", s.next));
            s.aps.push((path.clone(), ssid.into(), security));
            path
        };
        self.conn.object_server().at(&path, AccessPoint { ssid: ssid.into(), strength, security }).await.unwrap();
    }

    /// A network saved before: its password kept by the system, or with an empty one, by a desktop's keyring NM cannot reach.
    pub async fn saved(&self, ssid: &str, psk: &str) {
        let value = |v: Value<'_>| OwnedValue::try_from(v).unwrap();
        let mut settings: Settings = HashMap::from([
            ("connection".into(), HashMap::from([("type".into(), value(Value::from("802-11-wireless"))), ("id".into(), value(Value::from(ssid)))])),
            ("802-11-wireless".into(), HashMap::from([("ssid".into(), value(Value::from(ssid.as_bytes().to_vec())))])),
        ]);
        let mut security =
            HashMap::from([("key-mgmt".into(), value(Value::from("wpa-psk"))), ("psk-flags".into(), value(Value::from(u32::from(psk.is_empty()))))]);
        if !psk.is_empty() {
            security.insert("psk".into(), value(Value::from(psk)));
        }
        settings.insert("802-11-wireless-security".into(), security);
        Nm::add(&self.conn, &self.state, settings).await;
    }

    pub async fn cable(&self, up: bool) {
        let device = self.conn.object_server().interface::<_, Device>(ETHERNET).await.unwrap();
        device.get_mut().await.state = if up { 100 } else { 20 };
        device.get().await.state_changed(device.signal_emitter()).await.unwrap();
    }

    pub fn connections(&self) -> Vec<Saved> {
        self.state
            .lock()
            .unwrap()
            .connections
            .values()
            .map(|s| Saved {
                ssid: ssid_in(s),
                permissions: s
                    .get("connection")
                    .and_then(|c| c.get("permissions"))
                    .and_then(|v| <Vec<String>>::try_from(v.try_clone().ok()?).ok())
                    .unwrap_or_default(),
                psk: psk_in(s),
                psk_flags: s.get("802-11-wireless-security").and_then(|c| c.get("psk-flags")).and_then(|v| u32::try_from(v).ok()),
            })
            .collect()
    }

    pub fn scans(&self) -> u32 {
        self.state.lock().unwrap().scans
    }

    /// Whether any call asked to wait on a polkit password prompt.
    pub fn interactive(&self) -> bool {
        self.state.lock().unwrap().interactive
    }
}
