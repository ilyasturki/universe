#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    NixOs,
    Arch,
    Fedora,
    Debian,
    Other,
}

impl Family {
    pub fn as_str(self) -> &'static str {
        match self {
            Family::NixOs => "nixos",
            Family::Arch => "arch",
            Family::Fedora => "fedora",
            Family::Debian => "debian",
            Family::Other => "other",
        }
    }
}

pub fn detect() -> Family {
    static FAMILY: std::sync::OnceLock<Family> = std::sync::OnceLock::new();
    *FAMILY.get_or_init(|| {
        let text = std::fs::read_to_string("/etc/os-release").or_else(|_| std::fs::read_to_string("/usr/lib/os-release")).unwrap_or_default();
        from_os_release(std::path::Path::new("/etc/NIXOS").exists(), &text)
    })
}

/// `ID` first, then each `ID_LIKE` word: SteamOS is `arch`-like, Bazzite `fedora`-like, Mint and Pop!_OS `ubuntu debian`.
fn from_os_release(nixos: bool, text: &str) -> Family {
    if nixos {
        return Family::NixOs;
    }
    let field = |key: &str| {
        text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(|v| v.trim().trim_matches(['"', '\'']).to_ascii_lowercase()).unwrap_or_default()
    };
    let (id, like) = (field("ID"), field("ID_LIKE"));
    std::iter::once(id.as_str())
        .chain(like.split_whitespace())
        .find_map(|word| match word {
            "nixos" => Some(Family::NixOs),
            "arch" => Some(Family::Arch),
            "fedora" | "rhel" => Some(Family::Fedora),
            "debian" | "ubuntu" => Some(Family::Debian),
            _ => None,
        })
        .unwrap_or(Family::Other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_distribution_is_read_by_its_id_then_its_likes() {
        assert_eq!(from_os_release(true, "ID=arch\n"), Family::NixOs, "/etc/NIXOS settles it");
        assert_eq!(from_os_release(false, "NAME=\"CachyOS Linux\"\nID=cachyos\nID_LIKE=arch\n"), Family::Arch);
        assert_eq!(from_os_release(false, "ID=steamos\nID_LIKE=arch\nVARIANT_ID=steamdeck\n"), Family::Arch);
        assert_eq!(from_os_release(false, "ID=bazzite\nID_LIKE=\"fedora\"\n"), Family::Fedora);
        assert_eq!(from_os_release(false, "ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"), Family::Debian);
        assert_eq!(from_os_release(false, "ID=\"debian\"\n"), Family::Debian);
        assert_eq!(from_os_release(false, "ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"\n"), Family::Other);
        assert_eq!(from_os_release(false, ""), Family::Other);
    }
}
