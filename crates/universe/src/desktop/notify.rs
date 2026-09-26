use std::collections::HashMap;

use zbus::zvariant::Value;

const OSD_MS: i32 = 1500;

/// A notification standing in for the OSD: mako, dunst, swaync, fnott and xfce4-notifyd draw `value` as a bar. The bubble's id is kept on disk so the next one, from any process, replaces it instead of stacking.
pub async fn show_osd(icon: &str, label: &str, level: Option<f64>) -> Result<(), String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(&conn, "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.freedesktop.Notifications")
        .await
        .map_err(|e| e.to_string())?;
    let last = crate::paths::runtime_dir().join("osd-notification");
    let (server, ..): (String, String, String, String) = proxy.call("GetServerInformation", &()).await.unwrap_or_default();
    // mako 1.11 crashes on a Notify that both replaces an id and carries the synchronous hint, which alone replaces there.
    let replaces: u32 = if server == "mako" { 0 } else { std::fs::read_to_string(&last).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0) };
    let mut hints: HashMap<&str, Value> =
        HashMap::from([("x-canonical-private-synchronous", Value::from("universe-osd")), ("transient", Value::from(true)), ("urgency", Value::from(0u8))]);
    if let Some(level) = level {
        hints.insert("value", Value::from((level.clamp(0.0, 1.0) * 100.0).round() as i32));
    }
    let actions: Vec<&str> = Vec::new();
    let id: u32 =
        tokio::time::timeout(std::time::Duration::from_secs(5), proxy.call("Notify", &("Universe", replaces, icon, label, "", actions, hints, OSD_MS)))
            .await
            .map_err(|_| "the notification daemon did not answer".to_string())?
            .map_err(|e| format!("no notification daemon: {e}"))?;
    let _ = std::fs::write(&last, id.to_string());
    Ok(())
}
