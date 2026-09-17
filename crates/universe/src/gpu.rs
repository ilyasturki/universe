//! The GPU the games run on: vendor and generation from sysfs, the name from vulkaninfo when it is on PATH.

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

/// AMD's graphics IP generation, from amdgpu's ip_discovery (GC major.minor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Rdna {
    Rdna1,
    Rdna2,
    Rdna3,
    Rdna4,
}

impl Rdna {
    fn of(major: u32, minor: u32) -> Option<Rdna> {
        match (major, minor) {
            (12, _) => Some(Rdna::Rdna4),
            (11, _) => Some(Rdna::Rdna3),
            (10, m) if m >= 3 => Some(Rdna::Rdna2),
            (10, _) => Some(Rdna::Rdna1),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Rdna::Rdna1 => "RDNA 1",
            Rdna::Rdna2 => "RDNA 2",
            Rdna::Rdna3 => "RDNA 3",
            Rdna::Rdna4 => "RDNA 4",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gpu {
    pub vendor: Vendor,
    pub name: String,
    pub rdna: Option<Rdna>,
    /// `AMD Radeon RX 7900 GRE · RDNA 3`, what a settings card shows.
    pub label: String,
}

impl Gpu {
    fn new(vendor: Vendor, name: String, rdna: Option<Rdna>) -> Gpu {
        let name = if name.is_empty() { vendor.name().to_string() } else { name };
        let label = match rdna {
            Some(r) => format!("{name} · {}", r.name()),
            None => name.clone(),
        };
        Gpu { vendor, name, rdna, label }
    }

    /// Whether a Proton upscaler upgrade does anything on this card; `None` for a key that is not one.
    pub fn fits(&self, key: &str) -> Option<bool> {
        match key {
            "dlss_upgrade" => Some(self.vendor == Vendor::Nvidia),
            "fsr4_upgrade" => Some(matches!(self.rdna, Some(Rdna::Rdna3 | Rdna::Rdna4))),
            "xess_upgrade" => Some(true),
            "optiscaler" => Some(self.vendor != Vendor::Nvidia),
            _ => None,
        }
    }

    /// FSR 4 on RDNA 3 goes through Proton's RDNA 3 variant of the upgrade, not the generic one.
    pub fn needs_fsr4_rdna3(&self) -> bool {
        self.rdna == Some(Rdna::Rdna3)
    }

    /// `{vendor, name, rdna, label, fits: {key: bool}}`.
    pub fn to_json(&self) -> serde_json::Value {
        let fits: serde_json::Map<String, serde_json::Value> = ["dlss_upgrade", "fsr4_upgrade", "xess_upgrade", "optiscaler"]
            .into_iter()
            .filter_map(|k| self.fits(k).map(|b| (k.to_string(), serde_json::Value::Bool(b))))
            .collect();
        let mut v = serde_json::to_value(self).unwrap_or_default();
        v["fits"] = serde_json::Value::Object(fits);
        v
    }
}

/// One DRM card as sysfs describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Card {
    vendor: Vendor,
    device: u16,
    vram: u64,
    gc: Option<(u32, u32)>,
}

/// One device as `vulkaninfo --summary` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct VkDevice {
    vendor: u16,
    device: u16,
    name: String,
    discrete: bool,
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
                device: read_hex(&dev.join("device")).unwrap_or(0),
                vram: read_u64(&dev.join("mem_info_vram_total")).unwrap_or(0),
                gc: read_u64(&gc.join("major")).zip(read_u64(&gc.join("minor"))).map(|(a, b)| (a as u32, b as u32)),
            })
        })
        .collect()
}

fn parse_vulkaninfo(out: &str) -> Vec<VkDevice> {
    let mut devices = vec![];
    let mut cur: Option<VkDevice> = None;
    for line in out.lines() {
        let t = line.trim();
        if t.starts_with("GPU") && t.ends_with(':') {
            devices.extend(cur.take());
            cur = Some(VkDevice { vendor: 0, device: 0, name: String::new(), discrete: false });
            continue;
        }
        let Some(d) = cur.as_mut() else { continue };
        let Some((k, v)) = t.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "vendorID" => d.vendor = u16::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0),
            "deviceID" => d.device = u16::from_str_radix(v.trim_start_matches("0x"), 16).unwrap_or(0),
            "deviceType" => d.discrete = v == "PHYSICAL_DEVICE_TYPE_DISCRETE_GPU",
            "deviceName" => d.name = v.to_string(),
            _ => {}
        }
    }
    devices.extend(cur);
    devices
}

fn vulkan_devices() -> Vec<VkDevice> {
    let Some(bin) = crate::runners::on_path("vulkaninfo") else { return vec![] };
    let out = std::process::Command::new(bin).arg("--summary").output().ok();
    out.filter(|o| o.status.success()).map(|o| parse_vulkaninfo(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default()
}

/// RADV's `AMD Radeon RX 7900 GRE (RADV NAVI31)` without the driver's suffix.
fn clean_name(name: &str) -> String {
    match name.rfind(" (") {
        Some(i) if name.ends_with(')') => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// The card the games run on: what Vulkan calls discrete, else the one with the most VRAM (an NVIDIA card shows none and wins over an AMD or Intel iGPU).
fn pick(cards: Vec<Card>, vk: &[VkDevice]) -> Option<Gpu> {
    let mut ranked: Vec<(u64, Card)> = cards
        .into_iter()
        .map(|c| {
            let discrete = vk.iter().any(|d| d.vendor == pci_of(c.vendor) && d.device == c.device && d.discrete);
            let score = if discrete { u64::MAX } else if c.vendor == Vendor::Nvidia { u64::MAX / 2 } else { c.vram };
            (score, c)
        })
        .collect();
    ranked.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    let (_, card) = ranked.into_iter().next()?;
    let name = vk.iter().find(|d| d.vendor == pci_of(card.vendor) && d.device == card.device).map(|d| clean_name(&d.name)).unwrap_or_default();
    let rdna = if card.vendor == Vendor::Amd { card.gc.and_then(|(a, b)| Rdna::of(a, b)) } else { None };
    Some(Gpu::new(card.vendor, name, rdna))
}

fn pci_of(v: Vendor) -> u16 {
    match v {
        Vendor::Amd => 0x1002,
        Vendor::Nvidia => 0x10de,
        Vendor::Intel => 0x8086,
    }
}

/// Probed once per process; `None` when sysfs shows no card of a known vendor.
pub fn detected() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| pick(cards(Path::new("/sys/class/drm")), &vulkan_devices())).as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMMARY: &str = "Devices:\n========\nGPU0:\n\tvendorID           = 0x1002\n\tdeviceID           = 0x744c\n\tdeviceType         = PHYSICAL_DEVICE_TYPE_DISCRETE_GPU\n\tdeviceName         = AMD Radeon RX 7900 GRE (RADV NAVI31)\nGPU1:\n\tvendorID           = 0x1002\n\tdeviceID           = 0x164e\n\tdeviceType         = PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU\n\tdeviceName         = AMD Ryzen 9 7900X 12-Core Processor (RADV RAPHAEL_MENDOCINO)\nGPU2:\n\tvendorID           = 0x10005\n\tdeviceID           = 0x0000\n\tdeviceType         = PHYSICAL_DEVICE_TYPE_CPU\n\tdeviceName         = llvmpipe (LLVM 21.1.8, 256 bits)\n";

    fn card(dir: &Path, n: &str, vendor: &str, device: &str, vram: Option<&str>, gc: Option<(&str, &str)>) {
        let dev = dir.join(n).join("device");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("vendor"), vendor).unwrap();
        std::fs::write(dev.join("device"), device).unwrap();
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
    fn vulkaninfo_summary_parses() {
        let d = parse_vulkaninfo(SUMMARY);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].vendor, d[0].device, d[0].discrete), (0x1002, 0x744c, true));
        assert_eq!(d[0].name, "AMD Radeon RX 7900 GRE (RADV NAVI31)");
        assert!(!d[1].discrete);
        assert_eq!(clean_name(&d[0].name), "AMD Radeon RX 7900 GRE");
        assert_eq!(clean_name("NVIDIA GeForce RTX 4080"), "NVIDIA GeForce RTX 4080");
    }

    #[test]
    fn the_discrete_card_wins_and_is_named() {
        let dir = tempfile::tempdir().unwrap();
        card(dir.path(), "card0", "0x1002", "0x164e", Some("536870912"), Some(("10", "3")));
        card(dir.path(), "card1", "0x1002", "0x744c", Some("17163091968"), Some(("11", "0")));
        std::fs::create_dir_all(dir.path().join("card1-DP-1")).unwrap();
        let cards = cards(dir.path());
        assert_eq!(cards.len(), 2, "connectors are not cards");
        let g = pick(cards.clone(), &parse_vulkaninfo(SUMMARY)).unwrap();
        assert_eq!((g.vendor, g.rdna), (Vendor::Amd, Some(Rdna::Rdna3)));
        assert_eq!(g.label, "AMD Radeon RX 7900 GRE · RDNA 3");
        assert_eq!(g.to_json()["fits"], serde_json::json!({ "dlss_upgrade": false, "fsr4_upgrade": true, "xess_upgrade": true, "optiscaler": true }));
        assert!(g.needs_fsr4_rdna3());
        let g = pick(cards, &[]).unwrap();
        assert_eq!(g.label, "AMD · RDNA 3", "without vulkaninfo the VRAM decides and the vendor names the card");
    }

    #[test]
    fn nvidia_beats_an_igpu_without_vram_and_rdna4_needs_no_override() {
        let dir = tempfile::tempdir().unwrap();
        card(dir.path(), "card0", "0x8086", "0xa780", None, None);
        card(dir.path(), "card1", "0x10de", "0x2704", None, None);
        let g = pick(cards(dir.path()), &[]).unwrap();
        assert_eq!((g.vendor, g.rdna, g.label.as_str()), (Vendor::Nvidia, None, "NVIDIA"));
        assert_eq!(g.to_json()["fits"], serde_json::json!({ "dlss_upgrade": true, "fsr4_upgrade": false, "xess_upgrade": true, "optiscaler": false }));
        let rdna4 = Gpu::new(Vendor::Amd, "AMD Radeon RX 9070 XT".into(), Some(Rdna::Rdna4));
        assert!(rdna4.fits("fsr4_upgrade") == Some(true) && !rdna4.needs_fsr4_rdna3() && rdna4.fits("prefix").is_none());
        assert_eq!(Rdna::of(10, 1), Some(Rdna::Rdna1));
        assert!(Rdna::of(9, 4).is_none());
        assert!(pick(cards(&dir.path().join("nope")), &[]).is_none());
    }
}
