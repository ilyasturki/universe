use serde_json::Value;

use super::{run, CursorUndo, Toplevel, CURSOR_IDLE_S};

async fn hyprctl(list: &[&str]) -> Result<String, String> {
    run("hyprctl", &list.iter().map(|s| s.to_string()).collect::<Vec<_>>()).await
}

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    parse_clients(&hyprctl(&["-j", "clients"]).await?)
}

pub fn parse_clients(json: &str) -> Result<Vec<Toplevel>, String> {
    let clients: Vec<Value> = serde_json::from_str(json).map_err(|e| format!("hyprctl clients: {e}"))?;
    Ok(clients
        .iter()
        .map(|c| Toplevel {
            id: c["address"].as_str().unwrap_or_default().to_string(),
            pid: c["pid"].as_i64().unwrap_or(0),
            wm_class: c["class"].as_str().map(String::from),
            title: c["title"].as_str().map(String::from),
            focused: c["focusHistoryID"].as_i64() == Some(0),
            x: c["at"][0].as_i64().unwrap_or(0),
            y: c["at"][1].as_i64().unwrap_or(0),
            width: c["size"][0].as_i64().unwrap_or(0),
            height: c["size"][1].as_i64().unwrap_or(0),
            hidden: c["hidden"].as_bool().unwrap_or(false) || c["mapped"] == false,
            minimized: false,
        })
        .collect())
}

/// hyprctl answers `ok`, else says why, with a zero exit either way.
async fn command(list: &[&str]) -> Result<(), String> {
    let said = hyprctl(list).await?;
    if said.trim() == "ok" {
        Ok(())
    } else {
        Err(format!("hyprctl {}: {}", list.join(" "), said.trim()))
    }
}

pub async fn activate_window(id: &str) -> Result<bool, String> {
    if !id.starts_with("0x") {
        return Err(format!("not a Hyprland window address: {id}"));
    }
    if !list_windows().await?.iter().any(|w| w.id == id) {
        return Ok(false);
    }
    command(&["dispatch", "focuswindow", &format!("address:{id}")]).await?;
    Ok(true)
}

/// A timeout the user set is left alone; a `keyword` lasts until the config reloads.
pub async fn hide_cursor() -> Result<CursorUndo, String> {
    let seconds = option_seconds(&hyprctl(&["-j", "getoption", "cursor:inactive_timeout"]).await?);
    if seconds > 0.0 {
        return Ok(CursorUndo::Nothing);
    }
    command(&["keyword", "cursor:inactive_timeout", &CURSOR_IDLE_S.to_string()]).await?;
    Ok(CursorUndo::Hyprland { seconds })
}

pub async fn restore_cursor(seconds: f64) -> Result<(), String> {
    command(&["keyword", "cursor:inactive_timeout", &seconds.to_string()]).await
}

/// `float` since the option became one, `int` before.
pub fn option_seconds(json: &str) -> f64 {
    let v: Value = serde_json::from_str(json).unwrap_or_default();
    v["float"].as_f64().or(v["int"].as_f64()).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clients_give_their_address_place_and_focus() {
        let json = r#"[
            {"address":"0x55d0a1","mapped":true,"hidden":false,"at":[10,20],"size":[2560,1440],"pid":42,"class":"steam_app_1","title":"Game","focusHistoryID":0},
            {"address":"0x55d0a2","mapped":false,"hidden":false,"at":[0,0],"size":[0,0],"pid":43,"class":"x","title":"","focusHistoryID":1}
        ]"#;
        let windows = parse_clients(json).unwrap();
        assert_eq!((windows[0].id.as_str(), windows[0].pid, windows[0].x, windows[0].y, windows[0].height), ("0x55d0a1", 42, 10, 20, 1440));
        assert!(windows[0].focused && !windows[1].focused);
        assert!(windows[1].hidden, "an unmapped client is not one to focus");
    }

    #[test]
    fn the_timeout_reads_as_a_float_or_an_older_int() {
        assert_eq!(option_seconds(r#"{"option":"cursor:inactive_timeout","float":2.5,"set":true}"#), 2.5);
        assert_eq!(option_seconds(r#"{"option":"cursor:inactive_timeout","int":3,"set":true}"#), 3.0);
        assert_eq!(option_seconds("no such option"), 0.0);
    }
}
