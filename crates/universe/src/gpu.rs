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
    /// The render node VAAPI decodes on: the strongest card not on NVIDIA's own driver, which ships no VAAPI.
    pub vaapi: Option<String>,
}

impl Gpu {
    fn new(vendor: Vendor, rdna: Option<u8>) -> Gpu {
        let name = vendor.name().to_string();
        let label = match rdna {
            Some(r) => format!("{name} · RDNA {r}"),
            None => name.clone(),
        };
        Gpu { vendor, name, rdna, label, vaapi: None }
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Card {
    vendor: Option<Vendor>,
    vendor_id: u16,
    device_id: u16,
    vram: u64,
    gc: Option<(u32, u32)>,
    /// `0000:03:00.0`
    slot: String,
    driver: String,
    boot_vga: bool,
    /// `renderD129`
    render: String,
}

impl Card {
    /// NVIDIA shows no VRAM in sysfs: ranked above any iGPU by hand.
    fn rank(&self) -> u64 {
        if self.vendor == Some(Vendor::Nvidia) {
            u64::MAX
        } else {
            self.vram
        }
    }
}

/// A render GPU stronger than the one the firmware drives the screen with: a hybrid laptop's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offload {
    /// gamescope's `--prefer-vk-device`: `vendor:device` in hex.
    pub vk_device: String,
    pub env: Vec<(String, String)>,
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
    let link_name = |p: &Path| std::fs::read_link(p).ok().and_then(|l| l.file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_default();
    names
        .iter()
        .filter_map(|n| {
            let dev = drm.join(n).join("device");
            let vendor_id = read_hex(&dev.join("vendor"))?;
            let gc = dev.join("ip_discovery/die/0/GC/0");
            Some(Card {
                vendor: Vendor::of(vendor_id),
                vendor_id,
                device_id: read_hex(&dev.join("device")).unwrap_or(0),
                vram: read_u64(&dev.join("mem_info_vram_total")).unwrap_or(0),
                gc: read_u64(&gc.join("major")).zip(read_u64(&gc.join("minor"))).map(|(a, b)| (a as u32, b as u32)),
                slot: link_name(&dev),
                driver: link_name(&dev.join("driver")),
                boot_vga: read_u64(&dev.join("boot_vga")) == Some(1),
                render: std::fs::read_dir(dev.join("drm"))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .find(|n| n.starts_with("renderD"))
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn strongest<'a>(cards: impl Iterator<Item = &'a Card>) -> Option<&'a Card> {
    cards.filter(|c| c.vendor.is_some()).min_by_key(|c| std::cmp::Reverse(c.rank()))
}

fn pick(cards: Vec<Card>) -> Option<Gpu> {
    let card = strongest(cards.iter())?;
    let vendor = card.vendor?;
    let rdna = if vendor == Vendor::Amd { card.gc.and_then(|(a, b)| rdna_of(a, b)) } else { None };
    let decoder = strongest(cards.iter().filter(|c| c.driver != "nvidia" && !c.render.is_empty()));
    Some(Gpu { vaapi: decoder.map(|c| format!("/dev/dri/{}", c.render)), ..Gpu::new(vendor, rdna) })
}

/// Mesa and NVIDIA's own driver render on the firmware's display GPU (`boot_vga`) unless told otherwise.
fn offload_of(cards: &[Card]) -> Option<Offload> {
    let display = cards.iter().find(|c| c.boot_vga)?;
    let best = strongest(cards.iter())?;
    if best.rank() <= display.rank() || best.slot.is_empty() {
        return None;
    }
    let env: Vec<(&str, String)> = if best.driver == "nvidia" {
        vec![("__NV_PRIME_RENDER_OFFLOAD", "1".into()), ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".into()), ("__VK_LAYER_NV_optimus", "NVIDIA_only".into())]
    } else {
        vec![("DRI_PRIME", format!("pci-{}", best.slot.replace([':', '.'], "_")))]
    };
    Some(Offload { vk_device: format!("{:04x}:{:04x}", best.vendor_id, best.device_id), env: env.into_iter().map(|(k, v)| (k.to_string(), v)).collect() })
}

/// `None` when sysfs shows no card of a known vendor.
pub fn detected() -> Option<&'static Gpu> {
    static GPU: OnceLock<Option<Gpu>> = OnceLock::new();
    GPU.get_or_init(|| pick(cards(Path::new("/sys/class/drm")))).as_ref()
}

/// `None` off a hybrid machine: one GPU, or the display's is already the strongest.
pub fn offload() -> Option<&'static Offload> {
    static OFFLOAD: OnceLock<Option<Offload>> = OnceLock::new();
    OFFLOAD.get_or_init(|| offload_of(&cards(Path::new("/sys/class/drm")))).as_ref()
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
        for (n, render) in [("card0", "renderD128"), ("card1", "renderD129")] {
            std::fs::create_dir_all(dir.path().join(n).join("device/drm").join(render)).unwrap();
        }
        let cards = cards(dir.path());
        assert_eq!(cards.len(), 2, "connectors are not cards");
        let g = pick(cards).unwrap();
        assert_eq!(g.vaapi.as_deref(), Some("/dev/dri/renderD129"), "the strongest card decodes, whatever renderD128 is this boot");
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
        assert_eq!(pick(cards(dir.path())).unwrap().vaapi, None, "no render node, no VAAPI");
        for (n, render) in [("card0", "renderD129"), ("card1", "renderD128")] {
            std::fs::create_dir_all(dir.path().join(n).join("device/drm").join(render)).unwrap();
        }
        std::os::unix::fs::symlink(dir.path().join("nvidia"), dir.path().join("card1/device/driver")).unwrap();
        let g = pick(cards(dir.path())).unwrap();
        assert_eq!(g.vaapi.as_deref(), Some("/dev/dri/renderD129"), "NVIDIA's own driver has no VAAPI: the iGPU decodes");
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

    fn gpu(vendor_id: u16, device_id: u16, vram: u64, slot: &str, driver: &str, boot_vga: bool) -> Card {
        Card { vendor: Vendor::of(vendor_id), vendor_id, device_id, vram, slot: slot.into(), driver: driver.into(), boot_vga, ..Default::default() }
    }

    #[test]
    fn a_hybrid_laptop_renders_on_its_discrete_gpu_and_a_desktop_is_left_alone() {
        let intel = gpu(0x8086, 0xa7a0, 0, "0000:00:02.0", "i915", true);
        let nvidia = gpu(0x10de, 0x28e0, 0, "0000:01:00.0", "nvidia", false);
        let o = offload_of(&[intel.clone(), nvidia]).unwrap();
        assert_eq!(o.vk_device, "10de:28e0");
        assert_eq!(
            o.env.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            ["__NV_PRIME_RENDER_OFFLOAD", "__GLX_VENDOR_LIBRARY_NAME", "__VK_LAYER_NV_optimus"]
        );
        let apu = gpu(0x1002, 0x1681, 536_870_912, "0000:06:00.0", "amdgpu", true);
        let radeon = gpu(0x1002, 0x7480, 8_573_157_376, "0000:03:00.0", "amdgpu", false);
        let o = offload_of(&[apu.clone(), radeon.clone()]).unwrap();
        assert_eq!(
            (o.vk_device.as_str(), o.env.clone()),
            ("1002:7480", vec![("DRI_PRIME".to_string(), "pci-0000_03_00_0".to_string())]),
            "Mesa's GL and Vulkan both take the pci tag"
        );
        let nouveau = gpu(0x10de, 0x28e0, 0, "0000:01:00.0", "nouveau", false);
        assert_eq!(offload_of(&[intel, nouveau]).unwrap().env[0].0, "DRI_PRIME", "nouveau and NVK are Mesa's");
        let display_dgpu = gpu(0x1002, 0x744c, 25_753_026_560, "0000:03:00.0", "amdgpu", true);
        let igpu = gpu(0x1002, 0x164e, 536_870_912, "0000:17:00.0", "amdgpu", false);
        assert_eq!(offload_of(&[igpu, display_dgpu]), None, "the screen's GPU is already the strongest");
        assert_eq!(offload_of(std::slice::from_ref(&radeon)), None, "no firmware display GPU, nothing to steer away from");
        assert_eq!(offload_of(&[Card { boot_vga: true, ..radeon }]), None);
    }
}
