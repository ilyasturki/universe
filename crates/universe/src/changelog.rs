use serde::Serialize;

use crate::paths;

const BUNDLED: &str = include_str!("../../../CHANGELOG.md");

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    pub version: String,
    pub date: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub title: String,
    pub items: Vec<String>,
}

/// Every release in the CHANGELOG.md this build was made from, newest first.
pub fn releases() -> Vec<Release> {
    parse(BUNDLED)
}

fn parse(text: &str) -> Vec<Release> {
    let mut out: Vec<Release> = Vec::new();
    for line in text.lines() {
        if let Some((version, date)) = heading(line) {
            out.push(Release { version, date, sections: Vec::new() });
            continue;
        }
        let Some(release) = out.last_mut() else { continue };
        if let Some(title) = line.strip_prefix("### ") {
            release.sections.push(Section { title: title.trim().into(), items: Vec::new() });
        } else if let Some(item) = line.strip_prefix("- ") {
            if let Some(section) = release.sections.last_mut() {
                section.items.push(item.trim().into());
            }
        } else if line.starts_with("  ") && !line.trim().is_empty() {
            if let Some(item) = release.sections.last_mut().and_then(|s| s.items.last_mut()) {
                item.push(' ');
                item.push_str(line.trim());
            }
        }
    }
    out
}

fn heading(line: &str) -> Option<(String, String)> {
    let (version, rest) = line.strip_prefix("## [")?.split_once(']')?;
    semver(version)?;
    Some((version.into(), rest.strip_prefix(" - ").unwrap_or_default().trim().into()))
}

fn semver(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.trim().split('.').map(|p| p.parse::<u32>().ok());
    let triple = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(triple)
}

fn since(releases: Vec<Release>, last: &str, current: &str) -> Vec<Release> {
    let (Some(last), Some(current)) = (semver(last), semver(current)) else { return Vec::new() };
    releases.into_iter().filter(|r| semver(&r.version).is_some_and(|v| v > last && v <= current)).collect()
}

/// Records this version as the last one started, whether `shown` or not; returns, when `shown`, the releases since the one
/// recorded before. None recorded (a first start) or an unreadable one shows nothing.
pub fn whats_new(shown: bool) -> Vec<Release> {
    let file = paths::last_version_file();
    let last = std::fs::read_to_string(&file).ok();
    if last.as_deref().map(str::trim) != Some(crate::VERSION) {
        let written = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(&file, crate::VERSION));
        if let Err(e) = written {
            tracing::warn!("{}: {e}", file.display());
        }
    }
    match last {
        Some(last) if shown => since(releases(), &last, crate::VERSION),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog

## [0.2.0] - 2026-10-02

### Added

- Two.

## [0.1.1] - 2026-10-01

### Fixed

- One `code`,
  wrapped.
- Another.

## [0.1.0] - 2026-09-30

### Added

- Zero.

[0.2.0]: https://github.com/ilyasturki/universe/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/ilyasturki/universe/compare/v0.1.0...v0.1.1
";

    fn versions(releases: &[Release]) -> Vec<&str> {
        releases.iter().map(|r| r.version.as_str()).collect()
    }

    #[test]
    fn the_bundled_changelog_has_every_release_dated_and_this_one_in_it() {
        let releases = releases();
        assert!(versions(&releases).contains(&crate::VERSION), "no section for {}", crate::VERSION);
        for r in &releases {
            let date = chrono::NaiveDate::parse_from_str(&r.date, "%Y-%m-%d");
            assert!(date.is_ok(), "{} is dated '{}'", r.version, r.date);
            assert!(r.sections.iter().any(|s| !s.items.is_empty()), "{} has no entries", r.version);
        }
        let order: Vec<_> = releases.iter().map(|r| semver(&r.version).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] > w[1]), "newest first: {:?}", versions(&releases));
    }

    #[test]
    fn a_section_reads_its_wrapped_entries_and_not_the_links() {
        let releases = parse(SAMPLE);
        assert_eq!(versions(&releases), ["0.2.0", "0.1.1", "0.1.0"]);
        assert_eq!(releases[1].date, "2026-10-01");
        assert_eq!(releases[1].sections, [Section { title: "Fixed".into(), items: vec!["One `code`, wrapped.".into(), "Another.".into()] }]);
        assert_eq!(releases[2].sections[0].items, ["Zero."]);
    }

    #[test]
    fn what_is_new_runs_from_the_last_version_to_this_one() {
        let since = |last, current| since(parse(SAMPLE), last, current).iter().map(|r| r.version.clone()).collect::<Vec<_>>();
        assert_eq!(since("0.1.0", "0.2.0"), ["0.2.0", "0.1.1"]);
        assert_eq!(since("0.0.1", "0.1.1"), ["0.1.1", "0.1.0"], "a release newer than the build is left out");
        assert!(since("0.2.0", "0.2.0").is_empty());
        assert!(since("0.2.0", "0.1.0").is_empty(), "a downgrade");
        assert!(since("garbage", "0.2.0").is_empty());
    }

    #[test]
    fn every_start_records_its_version_and_shows_the_releases_once() {
        let _env = paths::test_env();
        let recorded = || std::fs::read_to_string(paths::last_version_file()).unwrap();
        assert!(whats_new(true).is_empty(), "a first start shows nothing");
        assert_eq!(recorded(), crate::VERSION);

        std::fs::write(paths::last_version_file(), "0.0.0").unwrap();
        assert!(whats_new(false).is_empty());
        assert_eq!(recorded(), crate::VERSION, "recorded with the page off too");

        std::fs::write(paths::last_version_file(), "0.0.0\n").unwrap();
        let shown = whats_new(true);
        assert_eq!(shown.first().map(|r| r.version.as_str()), Some(crate::VERSION));
        assert_eq!(shown.len(), releases().iter().filter(|r| semver(&r.version) <= semver(crate::VERSION)).count());
        assert!(whats_new(true).is_empty(), "shown once");

        std::fs::write(paths::last_version_file(), "garbage").unwrap();
        assert!(whats_new(true).is_empty());
        assert_eq!(recorded(), crate::VERSION);
    }
}
