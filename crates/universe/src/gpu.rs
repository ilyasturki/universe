use std::path::Path;
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    Amd,
    Nvidia,
    Intel,
}

impl Vendor {
    fn of(pci: u16) -> Option<Vendor> {
        match pci {
            0x1002 => Some(Vendor::Amd),
            0x10de => Some(Vendor::Nvidia),
            0x8086 => Some(Vendor::Intel),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Vendor::Amd => "AMD",
            Vendor::Nvidia => "NVIDIA",
            Vendor::Intel => "Intel",
        }
    }
}

/// amdgpu's ip_discovery GC major.minor: 10.0–10.2 RDNA 1, 10.3 RDNA 2, 11 RDNA 3, 12 RDNA 4.
fn rdna_of(major: u32, minor: u32) -> Option<u8> {
    match (major, minor) {
        (12, _) => Some(4),
        (11, _) => Some(3),
        (10, m) if m >= 3 => Some(2),
        (10, _) => Some(1),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gpu {
    pub vendor: Vendor,
    pub name: String,
    pub rdna: Option<u8>,
    pub label: String,
}

impl Gpu {
    fn new(vendor: Vendor, rdna: Option<u8>) -> Gpu {
        let name = vendor.name().to_string();
        let label = match rdna {
            Some(r) => format!("{name} · RDNA {r}"),
            None => name.clone(),
        };
        Gpu { vendor, name, rdna, label }
    }

    /// `None` for a key that is not an upscaler upgrade.
    pub fn fits(&self, key: &str) -> Option<bool> {
        match key {
            "dlss_upgrade" => Some(self.vendor == Vendor::Nvidia),
            "fsr4_upgrade" => Some(matches!(self.rdna, Some(3 | 4))),
            "xess_upgrade" => Some(true),
            "optiscaler" => Some(self.vendor != Vendor::Nvidia),
            _ => None,
        }
    }

    /// What an upgrade left on `auto` turns into: on where it is a plain win — DLSS on NVIDIA, FSR 4 on RDNA 4
    /// (RDNA 3 pays for it), XeSS on Intel. `None` for a key that is not an upscaler upgrade.
    pub fn wants(&self, key: &str) -> Option<bool> {
        match key {
            "dlss_upgrade" => Some(self.vendor == Vendor::Nvidia),
            "fsr4_upgrade" => Some(self.rdna == Some(4)),
            "xess_upgrade" => Some(self.vendor == Vendor::Intel),
            "optiscaler" => Some(false),
            _ => None,
        }
    }

    /// FSR 4 on RDNA 3 goes through Proton's RDNA 3 variant of the upgrade, not the generic one.
    pub fn needs_fsr4_rdna3(&self) -> bool {
        self.rdna == Some(3)
    }

    pub fn to_json(&self) -> serde_json::Value {
        let keys = ["dlss_upgrade", "fsr4_upgrade", "xess_upgrade", "optiscaler"];
        let map = |f: &dyn Fn(&str) -> Option<bool>| {
            serde_json::Value::Object(keys.into_iter().filter_map(|k| f(k).map(|b| (k.to_string(), serde_json::Value::Bool(b)))).collect())
        };
        let mut v = serde_json::to_value(self).unwrap_or_default();
        v["fits"] = map(&|k| self.fits(k));
        v["auto"] = map(&|k| self.wants(k));
        v
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Card {
    vendor: Vendor,
    vram: u64,
    gc: Option<(u32, u32)>,
}

fn read_hex(path: &Path) -> Option<u16> {
    let s = std::fs::read_to_string(path).ok()?;
    u16::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
}

fn read_u64(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn cards(drm: &Path) -> Vec<Card> {
    let Ok(rd) = std::fs::read_dir(drm) else { return vec![] };
    let mut names: Vec<_> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.starts_with("card") && !n.contains('-')).collect();
    names.sort();
    names
        .iter()
        .filter_map(|n| {
            let dev = drm.join(n).join("device");
            let vendor = Vendor::of(read_hex(&dev.join("vendor"))?)?;
            let gc = dev.join("ip_discovery/die/0/GC/0");
            Some(Card {
                vendor,
                vram: read_u64(&dev.join("mem_info_vram_total")).unwrap_or(0),
                gc: read_u64(&gc.join("major")).zip(read_u64(&gc.join("minor"))).map(|(a, b)| (a as u32, b as u32)),
            })
        })
        .collect()
}

/// NVIDIA shows no VRAM in sysfs: ranked above any iGPU by hand.
fn pick(cards: Vec<Card>) -> Option<Gpu> {
    let card = cards.into_iter().min_by_key(|c| std::cmp::Reverse(if c.vendor == Vendor::Nvidia { u64::MAX } else { c.vram }))?;
    let rdna = if card.vendor == Vendor::Amd { card.gc.and_then(|(a, b)| rdna_of(a, b)) } else { None };
    Some(Gpu::new(card.vendor, rdna))
}

/// `None` when sysfs shows no card of a known vendor.
pub fn detected() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| pick(cards(Path::new("/sys/class/drm")))).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(dir: &Path, n: &str, vendor: &str, vram: Option<&str>, gc: Option<(&str, &str)>) {
        let dev = dir.join(n).join("device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("vendor"), vendor).unwrap();
        if let Some(v) = vram {
            std::fs::write(dev.join("mem_info_vram_total"), v).unwrap();
        }
        if let Some((a, b)) = gc {
            let g = dev.join("ip_discovery/die/0/GC/0");
            std::fs::create_dir_all(&g).unwrap();
            std::fs::write(g.join("major"), a).unwrap();
            std::fs::write(g.join("minor"), b).unwrap();
        }
    }

    #[test]
    fn the_card_with_the_most_vram_wins_and_the_vendor_names_it() {
        let dir = tempfile::tempdir().unwrap();
        card(dir.path(), "card0", "0x1002", Some("536870912"), Some(("10", "3")));
        card(dir.path(), "card1", "0x1002", Some("17163091968"), Some(("11", "0")));
        std::fs::create_dir_all(dir.path().join("card1-DP-1")).unwrap();
        let cards = cards(dir.path());
        assert_eq!(cards.len(), 2, "connectors are not cards");
        let g = pick(cards).unwrap();
        assert_eq!((g.vendor, g.rdna), (Vendor::Amd, Some(3)));
        assert_eq!(g.label, "AMD · RDNA 3");
        assert_eq!(g.to_json()["fits"], serde_json::json!({ "dlss_upgrade": false, "fsr4_upgrade": true, "xess_upgrade": true, "optiscaler": true }));
        assert_eq!(g.to_json()["rdna"], 3);
        assert!(g.needs_fsr4_rdna3());
    }

    #[test]
    fn nvidia_beats_an_igpu_without_vram_and_rdna4_needs_no_override() {
        let dir = tempfile::tempdir().unwrap();
        card(dir.path(), "card0", "0x8086", None, None);
        card(dir.path(), "card1", "0x10de", None, None);
        let g = pick(cards(dir.path())).unwrap();
        assert_eq!((g.vendor, g.rdna, g.label.as_str()), (Vendor::Nvidia, None, "NVIDIA"));
        assert_eq!(g.to_json()["fits"], serde_json::json!({ "dlss_upgrade": true, "fsr4_upgrade": false, "xess_upgrade": true, "optiscaler": false }));
        let rdna4 = Gpu::new(Vendor::Amd, Some(4));
        assert!(rdna4.fits("fsr4_upgrade") == Some(true) && !rdna4.needs_fsr4_rdna3() && rdna4.fits("prefix").is_none());
        assert_eq!(rdna4.to_json()["auto"], serde_json::json!({ "dlss_upgrade": false, "fsr4_upgrade": true, "xess_upgrade": false, "optiscaler": false }));
        assert_eq!(g.to_json()["auto"]["dlss_upgrade"], serde_json::json!(true), "auto is on where the upgrade is a plain win");
        assert_eq!(rdna_of(10, 1), Some(1));
        assert!(rdna_of(9, 4).is_none());
        assert!(pick(cards(&dir.path().join("nope"))).is_none());
    }
}
