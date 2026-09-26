use serde_json::Value;

use super::{run, Toplevel};

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    parse_windows(&run("niri", &["msg".into(), "-j".into(), "windows".into()]).await?)
}

/// Before 25.05 a window carries no `layout`: its size is unknown, not zero.
pub fn parse_windows(json: &str) -> Result<Vec<Toplevel>, String> {
    let windows: Vec<Value> = serde_json::from_str(json).map_err(|e| format!("niri msg windows: {e}"))?;
    Ok(windows
        .iter()
        .map(|w| {
            let size = &w["layout"]["window_size"];
            Toplevel {
                id: w["id"].as_u64().unwrap_or(0).to_string(),
                pid: w["pid"].as_i64().unwrap_or(0),
                wm_class: w["app_id"].as_str().map(String::from),
                title: w["title"].as_str().map(String::from),
                focused: w["is_focused"].as_bool().unwrap_or(false),
                width: size[0].as_i64().unwrap_or(1),
                height: size[1].as_i64().unwrap_or(1),
                ..Default::default()
            }
        })
        .collect())
}

pub async fn activate_window(id: &str) -> Result<bool, String> {
    let id: u64 = id.parse().map_err(|_| format!("not a niri window id: {id}"))?;
    if !list_windows().await?.iter().any(|w| w.id == id.to_string()) {
        return Ok(false);
    }
    run("niri", &["msg".into(), "action".into(), "focus-window".into(), "--id".into(), id.to_string()]).await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_without_a_layout_still_counts_as_one_to_focus() {
        let json = r#"[
            {"id":7,"title":"Game","app_id":"steam_app_1","pid":42,"workspace_id":1,"is_focused":true,"is_floating":false,"layout":{"window_size":[2560,1440]}},
            {"id":8,"title":null,"app_id":"portal","pid":null,"workspace_id":1,"is_focused":false,"is_floating":false}
        ]"#;
        let windows = parse_windows(json).unwrap();
        assert_eq!((windows[0].id.as_str(), windows[0].pid, windows[0].width, windows[0].height), ("7", 42, 2560, 1440));
        assert!(windows[0].focused);
        assert_eq!((windows[1].pid, windows[1].width, windows[1].height), (0, 1, 1));
    }
}
