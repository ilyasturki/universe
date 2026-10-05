use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

use super::Bus;

const ADAPTER: &str = "/org/bluez/hci0";
const HID: &str = "00001124-0000-1000-8000-00805f9b34fb";

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub enum BluezError {
    #[zbus(error)]
    ZBus(zbus::Error),
    AuthenticationRejected(String),
    AuthenticationCanceled(String),
    AlreadyExists(String),
    DoesNotExist(String),
    NotReady(String),
}

/// How a device pairs: on its own, or once the agent confirms a passkey.
#[derive(Clone, Copy, Debug)]
pub enum Pairing {
    JustWorks,
    Confirm(u32),
}

#[derive(Default)]
struct Agents {
    registered: HashMap<String, OwnedObjectPath>,
    default: Option<(String, OwnedObjectPath)>,
    asked: Option<(String, OwnedObjectPath)>,
}

type Shared = Arc<Mutex<Agents>>;

struct AgentManager {
    agents: Shared,
}

fn sender(hdr: &Header<'_>) -> String {
    hdr.sender().map(|s| s.to_string()).unwrap_or_default()
}

#[zbus::interface(name = "org.bluez.AgentManager1")]
impl AgentManager {
    fn register_agent(&self, #[zbus(header)] hdr: Header<'_>, agent: OwnedObjectPath, _capability: String) -> Result<(), BluezError> {
        let mut agents = self.agents.lock().unwrap();
        if agents.registered.contains_key(&sender(&hdr)) {
            return Err(BluezError::AlreadyExists("Already Exists".into()));
        }
        agents.registered.insert(sender(&hdr), agent);
        Ok(())
    }

    fn unregister_agent(&self, #[zbus(header)] hdr: Header<'_>, _agent: OwnedObjectPath) -> Result<(), BluezError> {
        let mut agents = self.agents.lock().unwrap();
        agents.registered.remove(&sender(&hdr)).ok_or_else(|| BluezError::DoesNotExist("No such agent".into()))?;
        agents.default = agents.default.take().filter(|(s, _)| *s != sender(&hdr));
        Ok(())
    }

    fn request_default_agent(&self, #[zbus(header)] hdr: Header<'_>, agent: OwnedObjectPath) -> Result<(), BluezError> {
        let mut agents = self.agents.lock().unwrap();
        if agents.registered.get(&sender(&hdr)) != Some(&agent) {
            return Err(BluezError::DoesNotExist("No such agent".into()));
        }
        agents.default = Some((sender(&hdr), agent));
        Ok(())
    }
}

struct Adapter {
    powered: bool,
    discovering: bool,
}

#[zbus::interface(name = "org.bluez.Adapter1")]
impl Adapter {
    async fn start_discovery(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> Result<(), BluezError> {
        if !self.powered {
            return Err(BluezError::NotReady("Resource Not Ready".into()));
        }
        self.discovering = true;
        self.discovering_changed(&emitter).await?;
        Ok(())
    }

    async fn stop_discovery(&mut self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> Result<(), BluezError> {
        self.discovering = false;
        self.discovering_changed(&emitter).await?;
        Ok(())
    }

    async fn remove_device(&self, #[zbus(object_server)] server: &zbus::ObjectServer, device: OwnedObjectPath) -> Result<(), BluezError> {
        let _ = server.remove::<Battery, _>(&device).await;
        if !server.remove::<Device, _>(&device).await? {
            return Err(BluezError::DoesNotExist("Does Not Exist".into()));
        }
        Ok(())
    }

    #[zbus(property)]
    fn address(&self) -> String {
        "00:1A:7D:DA:71:13".into()
    }

    #[zbus(property)]
    fn powered(&self) -> bool {
        self.powered
    }

    #[zbus(property)]
    fn set_powered(&mut self, on: bool) {
        self.powered = on;
        if !on {
            self.discovering = false;
        }
    }

    #[zbus(property)]
    fn discovering(&self) -> bool {
        self.discovering
    }
}

struct Flags {
    paired: bool,
    trusted: bool,
    connected: bool,
}

struct Device {
    address: String,
    name: Option<String>,
    icon: String,
    pairing: Pairing,
    flags: Mutex<Flags>,
    agents: Shared,
}

#[zbus::interface(name = "org.bluez.Device1")]
impl Device {
    async fn pair(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), BluezError> {
        if self.flags.lock().unwrap().paired {
            return Err(BluezError::AlreadyExists("Already Exists".into()));
        }
        if let Pairing::Confirm(passkey) = self.pairing {
            let agent = {
                let mut agents = self.agents.lock().unwrap();
                let caller = sender(&hdr);
                let agent = agents.registered.get(&caller).map(|p| (caller, p.clone())).or_else(|| agents.default.clone());
                agents.asked = agent.clone();
                agent
            };
            let (dest, path) = agent.ok_or_else(|| BluezError::AuthenticationRejected("no agent".into()))?;
            let me = hdr.path().map(|p| p.to_owned()).unwrap();
            let asked = conn.call_method(Some(dest.as_str()), &path, Some("org.bluez.Agent1"), "RequestConfirmation", &(me, passkey)).await;
            match asked {
                Ok(_) => {}
                Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.bluez.Error.Canceled" => {
                    return Err(BluezError::AuthenticationCanceled("Authentication Canceled".into()));
                }
                Err(_) => return Err(BluezError::AuthenticationRejected("Authentication Rejected".into())),
            }
        }
        self.flags.lock().unwrap().paired = true;
        self.paired_changed(&emitter).await?;
        Ok(())
    }

    async fn cancel_pairing(&self, #[zbus(connection)] conn: &zbus::Connection) -> Result<(), BluezError> {
        let asked = self.agents.lock().unwrap().asked.clone();
        let (dest, path) = asked.ok_or_else(|| BluezError::DoesNotExist("No pairing in progress".into()))?;
        conn.call_method(Some(dest.as_str()), &path, Some("org.bluez.Agent1"), "Cancel", &()).await?;
        Ok(())
    }

    async fn connect(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> Result<(), BluezError> {
        self.flags.lock().unwrap().connected = true;
        self.connected_changed(&emitter).await?;
        Ok(())
    }

    async fn disconnect(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> Result<(), BluezError> {
        self.flags.lock().unwrap().connected = false;
        self.connected_changed(&emitter).await?;
        Ok(())
    }

    #[zbus(property)]
    fn address(&self) -> String {
        self.address.clone()
    }

    #[zbus(property)]
    fn alias(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.address.replace(':', "-"))
    }

    #[zbus(property)]
    fn name(&self) -> String {
        self.name.clone().unwrap_or_default()
    }

    #[zbus(property)]
    fn icon(&self) -> String {
        self.icon.clone()
    }

    #[zbus(property)]
    fn paired(&self) -> bool {
        self.flags.lock().unwrap().paired
    }

    #[zbus(property)]
    fn connected(&self) -> bool {
        self.flags.lock().unwrap().connected
    }

    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.flags.lock().unwrap().trusted
    }

    #[zbus(property)]
    fn set_trusted(&mut self, on: bool) {
        self.flags.lock().unwrap().trusted = on;
    }

    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        ObjectPath::from_static_str_unchecked(ADAPTER).into()
    }
}

struct Battery {
    percentage: u8,
}

#[zbus::interface(name = "org.bluez.Battery1")]
impl Battery {
    #[zbus(property)]
    fn percentage(&self) -> u8 {
        self.percentage
    }
}

/// bluetoothd on the test bus: one powered adapter, the devices a test adds.
#[derive(Clone)]
pub struct Bluez {
    conn: zbus::Connection,
    agents: Shared,
}

fn device_path(address: &str) -> OwnedObjectPath {
    ObjectPath::try_from(format!("{ADAPTER}/dev_{}", address.replace(':', "_"))).unwrap().into()
}

impl Bluez {
    pub async fn serve(bus: &Bus) -> Bluez {
        let conn = bus.connect().await;
        let agents = Shared::default();
        let server = conn.object_server();
        server.at("/", zbus::fdo::ObjectManager).await.unwrap();
        server.at("/org/bluez", AgentManager { agents: agents.clone() }).await.unwrap();
        server.at(ADAPTER, Adapter { powered: true, discovering: false }).await.unwrap();
        conn.request_name("org.bluez").await.unwrap();
        Bluez { conn, agents }
    }

    /// A device as discovery finds it, or paired already. A name of None is a device that tells none, which BlueZ shows as no Name.
    pub async fn add(&self, address: &str, name: Option<&str>, icon: &str, paired: bool, pairing: Pairing) -> OwnedObjectPath {
        let path = device_path(address);
        let device = Device {
            address: address.into(),
            name: name.map(str::to_string),
            icon: icon.into(),
            pairing,
            flags: Mutex::new(Flags { paired, trusted: paired, connected: false }),
            agents: self.agents.clone(),
        };
        self.conn.object_server().at(&path, device).await.unwrap();
        path
    }

    pub async fn battery(&self, address: &str, percentage: u8) {
        self.conn.object_server().at(device_path(address), Battery { percentage }).await.unwrap();
    }

    pub fn default_agent(&self) -> Option<String> {
        self.agents.lock().unwrap().default.as_ref().map(|(sender, _)| sender.clone())
    }

    /// What the sixaxis plugin does for a Sony pad plugged in by cable: the default agent is asked to let it use HID.
    pub async fn authorize_service(&self, device: &OwnedObjectPath) -> zbus::Result<()> {
        let (dest, path) = self.agents.lock().unwrap().default.clone().ok_or_else(|| zbus::Error::Failure("no default agent".into()))?;
        self.conn.call_method(Some(dest.as_str()), &path, Some("org.bluez.Agent1"), "AuthorizeService", &(device, HID)).await.map(drop)
    }

    /// BlueZ giving up on the question it put to an agent.
    pub async fn cancel_agent(&self) {
        let (dest, path) = self.agents.lock().unwrap().asked.clone().expect("no agent was asked");
        self.conn.call_method(Some(dest.as_str()), &path, Some("org.bluez.Agent1"), "Cancel", &()).await.unwrap();
    }
}
