use std::path::Path;
use std::time::Duration;

use zbus::fdo::ObjectManagerProxy;

const BUS_NAME: &str = "org.shadowblip.InputPlumber";
const ROOT: &str = "/org/shadowblip/InputPlumber";
const MANAGER: &str = "/org/shadowblip/InputPlumber/Manager";
const MANAGER_IFACE: &str = "org.shadowblip.InputManager";
const UNIT: &str = "inputplumber.service";

async fn manager(conn: &zbus::Connection) -> zbus::Result<zbus::Proxy<'static>> {
    zbus::Proxy::new(conn, BUS_NAME, MANAGER, MANAGER_IFACE).await
}

async fn version(conn: &zbus::Connection) -> Option<String> {
    manager(conn).await.ok()?.get_property::<String>("Version").await.ok()
}

async fn wait_for(tries: u32, every: Duration, mut ready: impl AsyncFnMut() -> bool) -> bool {
    for _ in 0..tries {
        if ready().await {
            return true;
        }
        tokio::time::sleep(every).await;
    }
    ready().await
}

pub async fn reachable() -> bool {
    match zbus::Connection::system().await {
        Ok(conn) => version(&conn).await.is_some(),
        Err(_) => false,
    }
}

async fn composite_present(conn: &zbus::Connection) -> bool {
    let Ok(om) = ObjectManagerProxy::builder(conn).destination(BUS_NAME).and_then(|b| b.path(ROOT)).map(|b| b.build()) else { return false };
    let Ok(om) = om.await else { return false };
    om.get_managed_objects().await.map(|objs| objs.keys().any(|p| p.as_str().contains("/CompositeDevice"))).unwrap_or(false)
}

fn hidden_present() -> bool {
    std::fs::read_dir(Path::new("/dev/inputplumber/by-hidden")).map(|rd| rd.flatten().next().is_some()).unwrap_or(false)
}

async fn restart_daemon(conn: &zbus::Connection) -> zbus::Result<()> {
    let systemd = zbus::Proxy::new(conn, "org.freedesktop.systemd1", "/org/freedesktop/systemd1", "org.freedesktop.systemd1.Manager").await?;
    systemd.call::<_, _, zbus::zvariant::OwnedObjectPath>("RestartUnit", &(UNIT, "replace")).await?;
    Ok(())
}

/// Returns whether the pads were taken; the emulator runs on the raw pads otherwise.
pub async fn engage() -> bool {
    let Ok(conn) = zbus::Connection::system().await else { return false };
    // A second ManageAllDevices in one daemon lifetime builds a duplicate composite.
    if let Err(e) = restart_daemon(&conn).await {
        tracing::warn!("inputplumber: restart refused ({e}); a second session in this daemon's lifetime may get a duplicate composite");
    }
    if !wait_for(20, Duration::from_millis(250), async || version(&conn).await.is_some()).await {
        tracing::warn!("inputplumber: the daemon did not come up");
        return false;
    }
    let Ok(m) = manager(&conn).await else { return false };
    if let Err(e) = m.set_property("ManageAllDevices", true).await {
        tracing::warn!("inputplumber: ManageAllDevices refused: {e}");
        return false;
    }
    // The first composite takes 7-9 s on a fresh daemon; the emulator's startup pad scan misses a later one.
    if !wait_for(30, Duration::from_millis(500), async || composite_present(&conn).await).await {
        tracing::warn!("inputplumber: no composite device appeared; the pads may still be hidden");
    }
    true
}

// No restart: killed before udev restores permissions, a pad stays mode 000 until power-cycled; by-hidden empties once udev has run.
pub async fn release() {
    let Ok(conn) = zbus::Connection::system().await else { return };
    let Ok(m) = manager(&conn).await else { return };
    if let Err(e) = m.set_property("ManageAllDevices", false).await {
        tracing::warn!("inputplumber: ManageAllDevices off refused: {e}");
        return;
    }
    if !wait_for(20, Duration::from_millis(250), async || !hidden_present()).await {
        tracing::warn!("inputplumber: pads still hidden after release");
    }
}
