use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result};

/// SteamOS's polkit helpers: `steamos-priv-write <path> <value>` writes the backlight, the APU's power cap and the GPU's
/// clock files (and leaves them group-writable after), `jupiter-fan-control --enable|--disable` runs its fan curve.
const HELPERS: &str = "usr/bin/steamos-polkit-helpers";

/// Every control's id, as `Core::system_controls` lists the ones this machine has.
pub const CONTROLS: [&str; 5] = ["brightness", "refresh", "tdp", "gpu", "fan"];

/// One of the machine's own controls. `range` steps from `min` to `max`; `choice` picks one of `choices`; `toggle` is "on" or "off".
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Control {
    pub id: &'static str,
    pub label: &'static str,
    pub detail: &'static str,
    pub kind: &'static str,
    pub value: String,
    pub min: u32,
    pub max: u32,
    pub step: u32,
    pub unit: &'static str,
    pub choices: Vec<String>,
}

impl Control {
    pub fn range(id: &'static str, label: &'static str, detail: &'static str, value: u32, (min, max, step): (u32, u32, u32), unit: &'static str) -> Control {
        Control { id, label, detail, kind: "range", value: value.to_string(), min, max, step, unit, choices: vec![] }
    }
}

/// The sysfs the controls read and write, `/` on the machine and a tempdir in tests.
pub struct Machine {
    root: PathBuf,
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_u64(path: &Path) -> Option<u64> {
    read(path)?.parse().ok()
}

fn writable(path: &Path) -> bool {
    std::fs::OpenOptions::new().write(true).open(path).is_ok()
}

fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir).map(|it| it.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    out.sort();
    out
}

impl Default for Machine {
    fn default() -> Self {
        Machine { root: PathBuf::from("/") }
    }
}

impl Machine {
    pub fn at(root: &Path) -> Machine {
        Machine { root: root.to_path_buf() }
    }

    fn helper(&self, name: &str) -> Option<PathBuf> {
        Some(self.root.join(HELPERS).join(name)).filter(|p| p.is_file())
    }

    /// The panel's backlight: amdgpu's first, as the Deck names it.
    pub fn backlight(&self) -> Option<PathBuf> {
        let all = sorted_dirs(&self.root.join("sys/class/backlight"));
        all.iter().find(|d| d.file_name().is_some_and(|n| n.to_string_lossy().starts_with("amdgpu_bl"))).or(all.first()).cloned()
    }

    /// amdgpu's hwmon with a power cap: the APU's sustained power limit (TDP), in µW.
    pub fn power_cap(&self) -> Option<PathBuf> {
        sorted_dirs(&self.root.join("sys/class/hwmon"))
            .into_iter()
            .find(|d| read(&d.join("name")).as_deref() == Some("amdgpu") && d.join("power1_cap").is_file())
    }

    /// An amdgpu card with overdrive: its clock is set by hand through pp_od_clk_voltage.
    pub fn gpu(&self) -> Option<PathBuf> {
        sorted_dirs(&self.root.join("sys/class/drm"))
            .into_iter()
            .map(|d| d.join("device"))
            .find(|d| d.join("pp_od_clk_voltage").is_file() && d.join("power_dpm_force_performance_level").is_file())
    }

    fn can_write(&self, path: &Path) -> bool {
        writable(path) || self.helper("steamos-priv-write").is_some()
    }

    /// Straight in when the file lets us (SteamOS's helper leaves it so after its first write), else through the helper.
    async fn write(&self, path: &Path, value: &str) -> Result<()> {
        if writable(path) {
            return std::fs::write(path, value).map_err(|e| Error::Io(format!("{}: {e}", path.display())));
        }
        let Some(helper) = self.helper("steamos-priv-write") else {
            return Err(Error::Unavailable(format!("{} is not writable", path.display())));
        };
        let status = tokio::process::Command::new(helper).arg(path).arg(value).status().await?;
        match status.success() {
            true => Ok(()),
            false => Err(Error::Unavailable(format!("steamos-priv-write refused {}", path.display()))),
        }
    }

    pub fn brightness(&self) -> Option<Control> {
        let dir = self.backlight()?;
        let max = read_u64(&dir.join("max_brightness")).filter(|m| *m > 0)?;
        let now = read_u64(&dir.join("brightness"))?;
        let percent = ((now * 100 + max / 2) / max) as u32;
        Some(Control::range("brightness", "Brightness", "The screen's backlight.", percent.clamp(5, 100), (5, 100, 5), "%"))
    }

    pub async fn set_brightness(&self, percent: u32) -> Result<()> {
        let dir = self.backlight().ok_or_else(|| Error::Unavailable("no backlight".into()))?;
        let max = read_u64(&dir.join("max_brightness")).unwrap_or(0);
        let raw = (u64::from(percent.clamp(5, 100)) * max / 100) as u32;
        let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match crate::logind::set_brightness(&name, raw).await {
            Ok(()) => Ok(()),
            Err(why) => {
                tracing::debug!("logind SetBrightness: {why}");
                self.write(&dir.join("brightness"), &raw.to_string()).await
            }
        }
    }

    pub fn tdp(&self) -> Option<Control> {
        let dir = self.power_cap()?;
        let cap = dir.join("power1_cap");
        if !self.can_write(&cap) {
            return None;
        }
        let watts = |f: &str, fallback: u32| read_u64(&dir.join(f)).map_or(fallback, |uw| (uw / 1_000_000) as u32);
        let (min, max) = (watts("power1_cap_min", 3).max(3), watts("power1_cap_max", 15));
        let now = watts("power1_cap", max).clamp(min, max);
        Some(Control::range("tdp", "Power limit", "The most the APU may draw, sustained: lower runs cooler and longer.", now, (min, max, 1), "W"))
    }

    pub async fn set_tdp(&self, watts: u32) -> Result<()> {
        let tdp = self.tdp().ok_or_else(|| Error::Unavailable("no power limit to set".into()))?;
        let cap = self.power_cap().unwrap().join("power1_cap");
        self.write(&cap, &(u64::from(watts.clamp(tdp.min, tdp.max)) * 1_000_000).to_string()).await
    }

    pub fn gpu_clock(&self) -> Option<Control> {
        let dir = self.gpu()?;
        let (od, level) = (dir.join("pp_od_clk_voltage"), dir.join("power_dpm_force_performance_level"));
        if !self.can_write(&od) || !self.can_write(&level) {
            return None;
        }
        let text = read(&od)?;
        let (min, max) = od_range(&text)?;
        let value = match read(&level).as_deref() {
            Some("manual") => od_sclk(&text).map_or("auto".into(), |mhz| mhz.to_string()),
            _ => "auto".into(),
        };
        let first = min.div_ceil(100) * 100;
        let choices = std::iter::once("auto".to_string()).chain((first..=max).step_by(100).map(|m| m.to_string())).collect();
        Some(Control {
            id: "gpu",
            label: "GPU clock",
            detail: "Auto lets the driver choose; a fixed clock trades power for steady frame times.",
            kind: "choice",
            value,
            min,
            max,
            step: 100,
            unit: "MHz",
            choices,
        })
    }

    pub async fn set_gpu_clock(&self, value: &str) -> Result<()> {
        let dir = self.gpu().ok_or_else(|| Error::Unavailable("no GPU clock to set".into()))?;
        let level = dir.join("power_dpm_force_performance_level");
        if value == "auto" {
            return self.write(&level, "auto").await;
        }
        let mhz: u32 = value.parse().map_err(|_| Error::Invalid(format!("GPU clock: auto or MHz, not '{value}'")))?;
        let control = self.gpu_clock().ok_or_else(|| Error::Unavailable("no GPU clock to set".into()))?;
        let mhz = mhz.clamp(control.min, control.max);
        let od = dir.join("pp_od_clk_voltage");
        self.write(&level, "manual").await?;
        // Steam pins the clock the same way: the low and the high state to one value, then commits.
        for line in [format!("s 0 {mhz}"), format!("s 1 {mhz}"), "c".into()] {
            self.write(&od, &line).await?;
        }
        Ok(())
    }

    pub async fn fan(&self) -> Option<Control> {
        self.helper("jupiter-fan-control")?;
        let out = tokio::process::Command::new("systemctl").args(["is-active", "jupiter-fan-control.service"]).output().await.ok()?;
        let on = String::from_utf8_lossy(&out.stdout).trim() == "active";
        Some(Control {
            id: "fan",
            label: "SteamOS fan curve",
            detail: "Off leaves the fan to the firmware.",
            kind: "toggle",
            value: if on { "on" } else { "off" }.into(),
            min: 0,
            max: 0,
            step: 0,
            unit: "",
            choices: vec![],
        })
    }

    pub async fn set_fan(&self, on: bool) -> Result<()> {
        let helper = self.helper("jupiter-fan-control").ok_or_else(|| Error::Unavailable("no SteamOS fan control".into()))?;
        let status = tokio::process::Command::new(helper).arg(if on { "--enable" } else { "--disable" }).status().await?;
        match status.success() {
            true => Ok(()),
            false => Err(Error::Unavailable("jupiter-fan-control refused".into())),
        }
    }
}

/// `OD_RANGE:` / `SCLK:     200Mhz       1600Mhz` in amdgpu's pp_od_clk_voltage.
fn od_range(text: &str) -> Option<(u32, u32)> {
    let after = text.split("OD_RANGE:").nth(1)?;
    let line = after.lines().find(|l| l.trim_start().starts_with("SCLK:"))?;
    let mut mhz = line.split_whitespace().skip(1).filter_map(|t| t.trim_end_matches("Mhz").trim_end_matches("MHz").parse().ok());
    Some((mhz.next()?, mhz.next()?))
}

/// The high state's clock under `OD_SCLK:`, the one a pinned clock sets.
fn od_sclk(text: &str) -> Option<u32> {
    let after = text.split("OD_SCLK:").nth(1)?;
    after
        .lines()
        .take_while(|l| !l.trim_start().starts_with("OD_"))
        .filter_map(|l| l.trim().strip_prefix("1:"))
        .find_map(|v| v.trim().trim_end_matches("Mhz").trim_end_matches("MHz").parse().ok())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const OD: &str = "OD_SCLK:\n0:        1200Mhz\n1:        1200Mhz\nOD_RANGE:\nSCLK:     200Mhz       1600Mhz\nCCLK:     1400Mhz       3500Mhz\n";

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    pub(crate) fn deck() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        put(r, "sys/class/backlight/amdgpu_bl0/brightness", "2048\n");
        put(r, "sys/class/backlight/amdgpu_bl0/max_brightness", "4095\n");
        put(r, "sys/class/hwmon/hwmon0/name", "nvme\n");
        put(r, "sys/class/hwmon/hwmon5/name", "amdgpu\n");
        put(r, "sys/class/hwmon/hwmon5/power1_cap", "15000000\n");
        put(r, "sys/class/hwmon/hwmon5/power1_cap_max", "15000000\n");
        put(r, "sys/class/drm/card0/device/pp_od_clk_voltage", OD);
        put(r, "sys/class/drm/card0/device/power_dpm_force_performance_level", "auto\n");
        dir
    }

    #[test]
    fn the_decks_controls_read_off_sysfs() {
        let dir = deck();
        let m = Machine::at(dir.path());
        assert_eq!(m.brightness().unwrap().value, "50");
        let tdp = m.tdp().unwrap();
        assert_eq!((tdp.value.as_str(), tdp.min, tdp.max), ("15", 3, 15));
        let gpu = m.gpu_clock().unwrap();
        assert_eq!((gpu.value.as_str(), gpu.min, gpu.max), ("auto", 200, 1600));
        assert_eq!(gpu.choices.first().map(String::as_str), Some("auto"));
        assert_eq!(gpu.choices.last().map(String::as_str), Some("1600"));
        put(dir.path(), "sys/class/drm/card0/device/power_dpm_force_performance_level", "manual\n");
        assert_eq!(m.gpu_clock().unwrap().value, "1200");
    }

    #[tokio::test]
    async fn a_writable_file_is_written_straight_and_a_pinned_clock_commits() {
        let dir = deck();
        let m = Machine::at(dir.path());
        m.set_tdp(9).await.unwrap();
        assert_eq!(read(&dir.path().join("sys/class/hwmon/hwmon5/power1_cap")).unwrap(), "9000000");
        m.set_tdp(40).await.unwrap();
        assert_eq!(read(&dir.path().join("sys/class/hwmon/hwmon5/power1_cap")).unwrap(), "15000000", "clamped to the cap's max");
        m.set_gpu_clock("800").await.unwrap();
        let dev = dir.path().join("sys/class/drm/card0/device");
        assert_eq!(read(&dev.join("power_dpm_force_performance_level")).unwrap(), "manual");
        assert_eq!(read(&dev.join("pp_od_clk_voltage")).unwrap(), "c", "the last line written commits");
        m.set_gpu_clock("auto").await.unwrap();
        assert_eq!(read(&dev.join("power_dpm_force_performance_level")).unwrap(), "auto");
        assert!(m.set_gpu_clock("fast").await.is_err());
    }

    #[test]
    fn a_desktop_without_those_files_has_no_controls() {
        let dir = tempfile::tempdir().unwrap();
        let m = Machine::at(dir.path());
        assert!(m.brightness().is_none() && m.tdp().is_none() && m.gpu_clock().is_none());
    }

    #[test]
    fn overdrive_text_parses() {
        assert_eq!(od_range(OD), Some((200, 1600)));
        assert_eq!(od_sclk(OD), Some(1200));
        assert_eq!(od_range("OD_SCLK:\n0: 200Mhz\n"), None);
    }
}
