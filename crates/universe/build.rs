use std::path::Path;
use std::process::Command;

// The short rev behind the semver: `UNIVERSE_GIT_REV` from the flake (no .git under nix), else git; plain semver when neither answers.
fn main() {
    println!("cargo:rerun-if-env-changed=UNIVERSE_GIT_REV");
    let rev = std::env::var("UNIVERSE_GIT_REV").ok().filter(|r| !r.is_empty()).or_else(git_rev).unwrap_or_default();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let build = if rev.is_empty() { version } else { format!("{version} ({rev})") };
    println!("cargo:rustc-env=UNIVERSE_BUILD={build}");
}

fn git_rev() -> Option<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().ok()?;
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(&root).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    // A source tarball unpacked inside another repo (an AUR package's clone) would take that repo's rev
    if Path::new(&git(&["rev-parse", "--show-toplevel"])?).canonicalize().ok()? != root {
        return None;
    }
    // Cargo reads a relative path from the crate, and counts a missing one, such as a packed ref's, as changed on every build.
    let watch = |path: &str| {
        if let Some(p) = git(&["rev-parse", "--git-path", path]).map(|p| root.join(p)).filter(|p| p.exists()) {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    };
    for path in ["HEAD", "index", "packed-refs"] {
        watch(path);
    }
    if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch(&head);
    }
    let short = git(&["rev-parse", "--short", "HEAD"])?;
    // Without --no-optional-locks, status refreshes the index watched above and reruns this script.
    let dirty = git(&["--no-optional-locks", "status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty()).unwrap_or(false);
    Some(if dirty { format!("{short}-dirty") } else { short })
}
