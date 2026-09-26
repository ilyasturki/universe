use serde_json::Value;

use super::{run, CursorUndo, Toplevel, CURSOR_IDLE_S};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

pub async fn list_windows() -> Result<Vec<Toplevel>, String> {
    parse_tree(&run("swaymsg", &args(&["-r", "-t", "get_tree"])).await?)
}

pub fn parse_tree(json: &str) -> Result<Vec<Toplevel>, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("swaymsg get_tree: {e}"))?;
    let mut windows = Vec::new();
    walk(&root, false, &mut windows);
    Ok(windows)
}

fn walk(node: &Value, scratchpad: bool, out: &mut Vec<Toplevel>) {
    let scratchpad = scratchpad || (node["type"] == "workspace" && node["name"] == "__i3_scratch");
    if let Some(pid) = node["pid"].as_i64().filter(|_| matches!(node["type"].as_str(), Some("con" | "floating_con"))) {
        let rect = &node["rect"];
        out.push(Toplevel {
            id: node["id"].as_i64().unwrap_or(0).to_string(),
            pid,
            wm_class: node["app_id"].as_str().or(node["window_properties"]["class"].as_str()).map(String::from),
            title: node["name"].as_str().map(String::from),
            focused: node["focused"].as_bool().unwrap_or(false),
            x: rect["x"].as_i64().unwrap_or(0),
            y: rect["y"].as_i64().unwrap_or(0),
            width: rect["width"].as_i64().unwrap_or(0),
            height: rect["height"].as_i64().unwrap_or(0),
            hidden: scratchpad,
            minimized: false,
        });
    }
    for child in ["nodes", "floating_nodes"].iter().filter_map(|k| node[*k].as_array()).flatten() {
        walk(child, scratchpad, out);
    }
}

pub async fn activate_window(id: &str) -> Result<bool, String> {
    let id: u64 = id.parse().map_err(|_| format!("not a sway container id: {id}"))?;
    if !list_windows().await?.iter().any(|w| w.id == id.to_string()) {
        return Ok(false);
    }
    run("swaymsg", &[format!("[con_id={id}]"), "focus".into()]).await?;
    Ok(true)
}

/// `seat * hide_cursor` cannot be read back: a timeout the config file sets is left alone, and put back as found.
pub async fn hide_cursor() -> Result<CursorUndo, String> {
    let config = run("swaymsg", &args(&["-r", "-t", "get_config"])).await?;
    let configured = configured_hide_ms(&config);
    if configured > 0 {
        return Ok(CursorUndo::Nothing);
    }
    run("swaymsg", &args(&["seat", "*", "hide_cursor", &(CURSOR_IDLE_S * 1000).to_string()])).await?;
    Ok(CursorUndo::Sway { ms: configured })
}

pub async fn restore_cursor(ms: u32) -> Result<(), String> {
    run("swaymsg", &args(&["seat", "*", "hide_cursor", &ms.to_string()])).await.map(drop)
}

/// The last `hide_cursor <ms>` of the main config file; its includes are not in GET_CONFIG.
pub fn configured_hide_ms(json: &str) -> u32 {
    let text = serde_json::from_str::<Value>(json).ok().and_then(|v| v["config"].as_str().map(String::from)).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let mut words = l.split_whitespace().skip_while(|w| *w != "hide_cursor");
            words.next()?;
            words.next()?.parse().ok()
        })
        .next_back()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = r#"{"id":1,"type":"root","nodes":[
        {"id":3,"type":"output","name":"DP-1","nodes":[
            {"id":4,"type":"workspace","name":"1","nodes":[
                {"id":10,"type":"con","pid":100,"app_id":"foot","name":"shell","focused":true,"visible":true,"rect":{"x":0,"y":0,"width":1920,"height":1080},"nodes":[]},
                {"id":11,"type":"con","nodes":[
                    {"id":12,"type":"con","pid":200,"app_id":null,"window_properties":{"class":"steam_app_1"},"name":"Game","focused":false,"visible":false,"rect":{"x":1920,"y":0,"width":2560,"height":1440},"nodes":[]}
                ]}
            ],"floating_nodes":[
                {"id":13,"type":"floating_con","pid":300,"app_id":"pavucontrol","name":"Volume","rect":{"x":5,"y":6,"width":7,"height":8},"nodes":[]}
            ]}
        ]},
        {"id":2,"type":"output","name":"__i3","nodes":[
            {"id":5,"type":"workspace","name":"__i3_scratch","nodes":[],"floating_nodes":[
                {"id":14,"type":"floating_con","pid":400,"app_id":"notes","name":"n","rect":{"x":0,"y":0,"width":1,"height":1},"nodes":[]}
            ]}
        ]}
    ]}"#;

    #[test]
    fn the_tree_gives_each_window_with_a_pid_and_the_scratchpad_is_hidden() {
        let windows = parse_tree(TREE).unwrap();
        let ids: Vec<&str> = windows.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["10", "12", "13", "14"], "the split container 11 is no window");
        let game = &windows[1];
        assert_eq!((game.pid, game.wm_class.as_deref(), game.x, game.width), (200, Some("steam_app_1"), 1920, 2560), "Xwayland's class stands in for app_id");
        assert!(windows[0].focused && !game.hidden, "another workspace is not hidden: focus goes there");
        assert!(windows[3].hidden);
    }

    #[test]
    fn the_config_files_hide_cursor_is_read_back() {
        let config = |text: &str| serde_json::json!({ "config": text }).to_string();
        assert_eq!(configured_hide_ms(&config("seat * hide_cursor 3000\n")), 3000);
        assert_eq!(configured_hide_ms(&config("seat seat0 {\n    hide_cursor 1500\n    hide_cursor when-typing enable\n}\n")), 1500);
        assert_eq!(configured_hide_ms(&config("# seat * hide_cursor 3000\nbindsym $mod+Return exec foot\n")), 0);
        assert_eq!(configured_hide_ms("not json"), 0);
    }
}
