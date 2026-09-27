use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Lcd,
    Oled,
}

impl Model {
    pub fn id(self) -> &'static str {
        match self {
            Model::Lcd => "lcd",
            Model::Oled => "oled",
        }
    }

    /// The panel's refresh range, as SteamOS's gamescope display scripts set it.
    pub fn refresh_range(self) -> (u32, u32) {
        match self {
            Model::Lcd => (40, 60),
            Model::Oled => (45, 90),
        }
    }
}

/// Valve's board names: Jupiter is the LCD Deck, Galileo the OLED one.
pub fn model_in(dmi: &Path) -> Option<Model> {
    let read = |f: &str| std::fs::read_to_string(dmi.join(f)).map(|s| s.trim().to_string()).unwrap_or_default();
    if read("sys_vendor") != "Valve" && read("board_vendor") != "Valve" {
        return None;
    }
    match read("product_name").as_str() {
        "Jupiter" => Some(Model::Lcd),
        "Galileo" => Some(Model::Oled),
        _ => None,
    }
}

/// `UNIVERSE_DECK=lcd|oled|none` stands in for the DMI read, so a desktop can play one.
pub fn model() -> Option<Model> {
    static MODEL: OnceLock<Option<Model>> = OnceLock::new();
    *MODEL.get_or_init(|| match std::env::var("UNIVERSE_DECK").as_deref() {
        Ok("lcd") => Some(Model::Lcd),
        Ok("oled") => Some(Model::Oled),
        Ok("none") => None,
        _ => model_in(Path::new("/sys/class/dmi/id")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dmi(vendor: &str, product: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("sys_vendor"), format!("{vendor}\n")).unwrap();
        std::fs::write(dir.path().join("product_name"), format!("{product}\n")).unwrap();
        dir
    }

    #[test]
    fn valve_boards_are_decks_by_their_product_name() {
        assert_eq!(model_in(dmi("Valve", "Jupiter").path()), Some(Model::Lcd));
        assert_eq!(model_in(dmi("Valve", "Galileo").path()), Some(Model::Oled));
        assert_eq!(model_in(dmi("Valve", "Something new").path()), None);
        assert_eq!(model_in(dmi("LENOVO", "Jupiter").path()), None);
        assert_eq!(model_in(Path::new("/nonexistent")), None);
        assert_eq!(Model::Oled.refresh_range(), (45, 90));
    }
}
