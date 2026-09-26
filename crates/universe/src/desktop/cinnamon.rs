use std::collections::HashMap;

use zbus::zvariant::Value;

/// org.Cinnamon.ShowOSD takes `level` in 0–100, as GNOME's did.
pub async fn show_osd(icon: &str, label: &str, level: Option<f64>) -> Result<(), String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(&conn, "org.Cinnamon", "/org/Cinnamon", "org.Cinnamon").await.map_err(|e| e.to_string())?;
    let mut params: HashMap<&str, Value> = HashMap::from([("icon", Value::from(icon)), ("label", Value::from(label))]);
    if let Some(level) = level {
        params.insert("level", Value::from((level.clamp(0.0, 1.0) * 100.0).round() as i32));
    }
    proxy.call::<_, _, ()>("ShowOSD", &(params,)).await.map_err(|e| format!("Cinnamon OSD: {e}"))
}
