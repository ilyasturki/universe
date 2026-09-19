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
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    for path in ["HEAD", "index"] {
        if let Some(p) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    if let Some(head) = git(&["symbolic-ref", "-q", "HEAD"]).and_then(|r| git(&["rev-parse", "--git-path", &r])) {
        println!("cargo:rerun-if-changed={head}");
    }
    let short = git(&["rev-parse", "--short", "HEAD"])?;
    let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).map(|s| !s.is_empty()).unwrap_or(false);
    Some(if dirty { format!("{short}-dirty") } else { short })
}
