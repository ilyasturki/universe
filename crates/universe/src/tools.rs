use std::path::PathBuf;
use std::time::Duration;

use crate::paths;

pub struct Tool {
    pub bin: &'static str,
    pub package: &'static str,
    pub version: &'static str,
    url: &'static str,
    sha256: &'static str,
    /// The program's path inside a tar download; `None` when the download is the program itself.
    member: Option<&'static str>,
    x86_64_only: bool,
}

pub const TOOLS: &[Tool] = &[
    Tool {
        bin: "umu-run",
        package: "umu-launcher",
        version: "1.4.4",
        url: "https://github.com/Open-Wine-Components/umu-launcher/releases/download/1.4.4/umu-launcher-1.4.4-zipapp.tar",
        sha256: "eb590691841f7fad3fc3ad8fd5db4ccb87849fe7948e62b28ece7a4ee48cc851",
        member: Some("umu/umu-run"),
        x86_64_only: false,
    },
    Tool {
        bin: "gogdl",
        package: "heroic-gogdl",
        version: "1.3.0",
        url: "https://github.com/Heroic-Games-Launcher/heroic-gogdl/releases/download/v1.3.0/gogdl_linux_x86_64",
        sha256: "cba013d42767c808237c437335ab1d56f58405d07e8f37b3324d264ea5c49655",
        member: None,
        x86_64_only: true,
    },
];

/// Searched after PATH (`runners::on_path`, `search_path`): an installed program wins over a fetched one.
pub fn dir() -> PathBuf {
    paths::data_home().join("bin")
}

pub fn find(bin: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|t| t.bin == bin && (!t.x86_64_only || cfg!(target_arch = "x86_64")))
}

pub fn search_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.push(dir());
    std::env::join_paths(dirs).unwrap_or_default()
}

/// `Ok` for a missing program that is not one of `TOOLS`: its caller reports it.
pub async fn ensure(bin: &str) -> crate::Result<()> {
    let Some(tool) = find(bin).filter(|_| crate::runners::on_path(bin).is_none()) else { return Ok(()) };
    static FETCHING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _one = FETCHING.lock().await;
    if crate::runners::on_path(bin).is_some() {
        return Ok(());
    }
    tracing::info!("{bin} not installed: fetching {} {}", tool.package, tool.version);
    fetch(tool).await
}

async fn fetch(tool: &Tool) -> crate::Result<()> {
    let failed =
        |e: reqwest::Error| crate::Error::Unavailable(format!("{} is not installed and {} {} could not be fetched: {e}", tool.bin, tool.package, tool.version));
    let body = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent("universe")
        .build()
        .map_err(failed)?
        .get(tool.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(failed)?
        .bytes()
        .await
        .map_err(failed)?;
    install(tool, &body)
}

fn install(tool: &Tool, body: &[u8]) -> crate::Result<()> {
    use sha2::Digest;
    use std::os::unix::fs::PermissionsExt;
    let digest: String = sha2::Sha256::digest(body).iter().map(|b| format!("{b:02x}")).collect();
    if digest != tool.sha256 {
        return Err(crate::Error::Io(format!("{}: sha256 {digest}, not the pinned {}", tool.url, tool.sha256)));
    }
    let program = match tool.member {
        None => body.to_vec(),
        Some(member) => {
            let mut archive = tar::Archive::new(body);
            let mut entry = archive
                .entries()?
                .flatten()
                .find(|e| e.path().is_ok_and(|p| p.as_os_str() == member))
                .ok_or_else(|| crate::Error::Io(format!("{}: no {member} inside", tool.url)))?;
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut out)?;
            out
        }
    };
    let dir = dir();
    std::fs::create_dir_all(&dir)?;
    let part = dir.join(format!(".{}.part", tool.bin));
    std::fs::write(&part, program)?;
    std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&part, dir.join(tool.bin))?;
    tracing::info!("{} {} installed as {}", tool.package, tool.version, dir.join(tool.bin).display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_of(name: &str, data: &[u8]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o744);
        h.set_cksum();
        b.append_data(&mut h, name, data).unwrap();
        b.into_inner().unwrap()
    }

    fn sha256_of(body: &[u8]) -> &'static str {
        use sha2::Digest;
        Box::leak(sha2::Sha256::digest(body).iter().map(|b| format!("{b:02x}")).collect::<String>().into_boxed_str())
    }

    #[test]
    fn a_fetch_lands_executable_and_only_with_its_pinned_digest() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("UNIVERSE_DATA_HOME", home.path());
        let body = tar_of("umu/umu-run", b"#!/usr/bin/env python3\n");
        let tool =
            Tool { bin: "umu-run", package: "umu-launcher", version: "0", url: "u", sha256: sha256_of(&body), member: Some("umu/umu-run"), x86_64_only: false };
        install(&tool, &body).unwrap();
        let installed = home.path().join("bin/umu-run");
        assert_eq!(std::fs::read(&installed).unwrap(), b"#!/usr/bin/env python3\n", "the member alone, out of the tar");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&installed).unwrap().permissions().mode() & 0o777, 0o755);

        let tampered = Tool { sha256: sha256_of(b"other"), bin: "gogdl", member: None, ..tool };
        assert!(install(&tampered, b"#!/bin/sh\n").is_err());
        assert!(!home.path().join("bin/gogdl").exists(), "a download that is not the pinned one is never written");
        std::env::remove_var("UNIVERSE_DATA_HOME");
    }

    #[test]
    #[ignore = "live: downloads every pinned tool from GitHub"]
    fn live_the_pinned_tools_download_and_match() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("UNIVERSE_DATA_HOME", home.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        for tool in TOOLS.iter().filter(|t| find(t.bin).is_some()) {
            rt.block_on(fetch(tool)).unwrap_or_else(|e| panic!("{}: {e}", tool.bin));
            let installed = home.path().join("bin").join(tool.bin);
            assert!(std::fs::metadata(&installed).is_ok_and(|m| m.len() > 0), "{}", installed.display());
        }
        std::env::remove_var("UNIVERSE_DATA_HOME");
    }

    #[test]
    fn fetched_tools_are_searched_after_path() {
        let _lock = crate::paths::ENV_LOCK.lock().unwrap();
        assert!(find("umu-run").is_some() && find("wine").is_none());
        let path = search_path();
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(dirs.last(), Some(&dir()), "an installed tool wins over a fetched one");
    }
}
