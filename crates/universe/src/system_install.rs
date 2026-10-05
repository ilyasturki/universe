use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

use crate::components::{system_tool, SystemTool};
use crate::core::Progress;
use crate::{Error, Result};

pub const ACTION: &str = "io.github.ilyasturki.universe.system-install";

// The paths the polkit policy's exec.path names: the distribution packages', then install.sh's.
const PLACES: [&str; 2] = ["/usr/lib/universe/universe-system-install", "/usr/local/lib/universe/universe-system-install"];

/// The helper's arguments: exactly one id of `components::SYSTEM`. It runs as root, so what it installs never comes from the caller.
pub fn requested(args: &[String]) -> Result<&'static SystemTool> {
    match args {
        [id] => system_tool(id).ok_or_else(|| Error::Invalid(format!("{id} is not a system tool Universe installs"))),
        _ => Err(Error::Invalid(format!("one system tool expected, got {} arguments", args.len()))),
    }
}

/// The helper and pkexec, in the Universe session only: no polkit agent answers there, and the helper's action is
/// allow_active=yes, while a desktop keeps PackageKit's own prompt.
pub fn helper() -> Option<PathBuf> {
    if !crate::nest::session() || crate::runners::on_system_path("pkexec").is_none() {
        return None;
    }
    PLACES.iter().map(PathBuf::from).find(|p| p.is_file())
}

/// The helper prints each percent on a line of its own; its last stderr line is the reason it failed.
pub async fn install(helper: &Path, tool: &SystemTool, mut progress: Option<Progress<'_, '_>>) -> Result<()> {
    let failed = |e: String| Error::Unavailable(format!("{}: {e}", tool.name));
    let mut child = tokio::process::Command::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(helper)
        .arg(tool.id)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| failed(format!("pkexec: {e}")))?;
    let mut lines = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut stderr = child.stderr.take().expect("piped");
    let errors = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });
    while let Ok(Some(line)) = lines.next_line().await {
        if let (Some(p), Ok(percent)) = (progress.as_mut(), line.trim().parse::<u64>()) {
            p(percent.min(100), 100, &format!("Installing {} · {percent}%", tool.name));
        }
    }
    let status = child.wait().await.map_err(|e| failed(e.to_string()))?;
    let errors = errors.await.unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let reason = errors.lines().map(str::trim).rfind(|l| !l.is_empty()).map(str::to_string);
    Err(failed(reason.unwrap_or_else(|| format!("the install helper ended with {status}"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_helper_takes_one_listed_system_tool_and_nothing_else() {
        for tool in crate::components::SYSTEM {
            assert_eq!(requested(&args(&[tool.id])).unwrap().id, tool.id);
        }
        for refused in [
            &[][..],
            &["bash"],
            &["MangoHud"],
            &["mangohud "],
            &["lib32-mangohud"],
            &["../mangohud"],
            &["/usr/bin/gamescope"],
            &["mangohud", "gamescope"],
            &["mangohud", "--", "bash"],
            &[""],
        ] {
            assert!(matches!(requested(&args(refused)), Err(Error::Invalid(_))), "{refused:?} is refused");
        }
    }

    #[test]
    fn outside_the_session_there_is_no_helper_to_go_through() {
        let _env = crate::paths::test_env();
        assert_eq!(helper(), None);
    }

    #[test]
    fn the_shipped_policy_lets_active_sessions_run_the_helper_where_the_packages_put_it() {
        let policy = include_str!("../../../packaging/system/io.github.ilyasturki.universe.policy");
        assert!(policy.contains(&format!("<action id=\"{ACTION}\">")));
        assert!(policy.contains("<allow_any>no</allow_any>") && policy.contains("<allow_inactive>no</allow_inactive>"));
        assert!(policy.contains("<allow_active>yes</allow_active>"));
        assert!(policy.contains(&format!("<annotate key=\"org.freedesktop.policykit.exec.path\">{}</annotate>", PLACES[0])));
    }
}
