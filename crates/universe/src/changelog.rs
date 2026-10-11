use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use crate::{paths, Error, Result};

const BUNDLED: &str = include_str!("../../../CHANGELOG.md");
/// Where a release's media are uploaded, as assets of its tag.
pub const RELEASES: &str = "https://github.com/ilyasturki/universe/releases/download/";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Release {
    pub version: String,
    pub date: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub text: String,
    pub media: Option<Media>,
}

/// A highlighted entry's screenshot or clip, an asset of its release.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Media {
    /// still | clip
    pub kind: String,
    pub alt: String,
    /// The file the apps show: a still's WebP, a clip's H.264 MP4.
    pub url: String,
}

impl Item {
    fn new(text: &str) -> Self {
        Item { text: text.into(), media: None }
    }
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
                section.items.push(Item::new(item.trim()));
            }
        } else if line.starts_with("  ") && !line.trim().is_empty() {
            let version = release.version.clone();
            let Some(item) = release.sections.last_mut().and_then(|s| s.items.last_mut()) else { continue };
            if line.trim().starts_with("![") {
                item.media = media(line.trim(), &version);
            } else {
                item.text.push(' ');
                item.text.push_str(line.trim());
            }
        }
    }
    out
}

/// `![alt](RELEASES/vVERSION/NAME.EXT)` under an entry: the Markdown shows a still's NAME.webp and a clip's animated NAME.avif,
/// the apps a still's NAME.webp and a clip's NAME.mp4 beside it.
fn media(line: &str, version: &str) -> Option<Media> {
    let (alt, rest) = line.strip_prefix("![")?.split_once("](")?;
    let file = rest.strip_suffix(')')?.strip_prefix(RELEASES)?.strip_prefix(&format!("v{version}/"))?;
    let (name, ext) = file.rsplit_once('.')?;
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
        return None;
    }
    let (kind, shown) = match ext {
        "webp" => ("still", format!("{name}.webp")),
        "avif" => ("clip", format!("{name}.mp4")),
        _ => return None,
    };
    Some(Media { kind: kind.into(), alt: alt.trim().into(), url: format!("{RELEASES}v{version}/{shown}") })
}

/// The cached copy of a release's media (`Media::url`), downloaded on its first ask.
pub async fn media_file(url: &str) -> Result<PathBuf> {
    fetch_media(url, RELEASES).await
}

async fn fetch_media(url: &str, base: &str) -> Result<PathBuf> {
    let file = url.strip_prefix(RELEASES).filter(|f| is_asset(f)).ok_or_else(|| Error::Invalid(format!("{url}: no Universe release's media")))?;
    let dest = paths::cache_home().join("changelog").join(file);
    if dest.is_file() {
        return Ok(dest);
    }
    let dir = dest.parent().unwrap_or(&dest).to_path_buf();
    std::fs::create_dir_all(&dir)?;
    static FETCHES: AtomicU64 = AtomicU64::new(0);
    let part = dir.join(format!(".{}.{}.part", std::process::id(), FETCHES.fetch_add(1, Ordering::Relaxed)));
    let failed = |e: reqwest::Error| Error::Unavailable(format!("{url}: {e}"));
    let resp = crate::components::client()?.get(format!("{base}{file}")).send().await.and_then(reqwest::Response::error_for_status).map_err(failed)?;
    let bytes = resp.bytes().await.map_err(failed)?;
    std::fs::write(&part, &bytes)?;
    std::fs::rename(&part, &dest)?;
    Ok(dest)
}

fn is_asset(file: &str) -> bool {
    let Some((tag, name)) = file.split_once('/') else { return false };
    let Some((stem, ext)) = name.rsplit_once('.') else { return false };
    tag.strip_prefix('v').and_then(semver).is_some()
        && matches!(ext, "webp" | "mp4")
        && !stem.is_empty()
        && stem.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
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
  ![Two, played](https://github.com/ilyasturki/universe/releases/download/v0.2.0/two.avif)
- Three.
  ![Three](https://github.com/ilyasturki/universe/releases/download/v0.2.0/three.webp)
- Copied from the last release.
  ![Old](https://github.com/ilyasturki/universe/releases/download/v0.1.1/old.webp)
- Elsewhere.
  ![Hosted](https://example.com/v0.2.0/x.webp)

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
        let shown = releases.iter().flat_map(|r| &r.sections).flat_map(|s| &s.items).filter(|i| i.media.is_some()).count();
        assert_eq!(shown, BUNDLED.lines().filter(|l| l.trim_start().starts_with("![")).count(), "an image line the apps read as no media");
        let order: Vec<_> = releases.iter().map(|r| semver(&r.version).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] > w[1]), "newest first: {:?}", versions(&releases));
    }

    #[test]
    fn a_section_reads_its_wrapped_entries_and_not_the_links() {
        let releases = parse(SAMPLE);
        assert_eq!(versions(&releases), ["0.2.0", "0.1.1", "0.1.0"]);
        assert_eq!(releases[1].date, "2026-10-01");
        assert_eq!(releases[1].sections, [Section { title: "Fixed".into(), items: vec![Item::new("One `code`, wrapped."), Item::new("Another.")] }]);
        assert_eq!(releases[2].sections[0].items, [Item::new("Zero.")]);
    }

    #[test]
    fn an_entry_takes_its_release_s_media_out_of_its_text() {
        let items = &parse(SAMPLE)[0].sections[0].items;
        let texts: Vec<_> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, ["Two.", "Three.", "Copied from the last release.", "Elsewhere."], "an image line is never text");
        let at = |file: &str| format!("{RELEASES}v0.2.0/{file}");
        assert_eq!(items[0].media, Some(Media { kind: "clip".into(), alt: "Two, played".into(), url: at("two.mp4") }), "a clip plays its MP4");
        assert_eq!(items[1].media, Some(Media { kind: "still".into(), alt: "Three".into(), url: at("three.webp") }));
        assert_eq!(items[2].media, None, "another release's assets");
        assert_eq!(items[3].media, None, "another host");
    }

    #[test]
    fn a_release_s_media_downloads_once_into_the_cache() {
        let _env = paths::test_env();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let base = crate::components::tests::serve(vec![("/v0.2.0/three.webp", b"webp".to_vec())]);
        let base = format!("{base}/");
        let url = format!("{RELEASES}v0.2.0/three.webp");
        let file = rt.block_on(fetch_media(&url, &base)).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"webp");
        assert!(file.starts_with(paths::cache_home()));
        assert_eq!(rt.block_on(fetch_media(&url, "http://127.0.0.1:9/")).unwrap(), file, "cached: no second download");

        let missing = rt.block_on(fetch_media(&format!("{RELEASES}v0.2.0/gone.webp"), &base));
        assert!(matches!(missing, Err(Error::Unavailable(_))), "{missing:?}");
        assert_eq!(std::fs::read_dir(file.parent().unwrap()).unwrap().count(), 1, "a failed download leaves no part behind");
        for foreign in ["https://example.com/v0.2.0/x.webp", &format!("{RELEASES}v0.2.0/../../x.webp"), &format!("{RELEASES}v0.2.0/x.sh")] {
            assert!(matches!(rt.block_on(fetch_media(foreign, &base)), Err(Error::Invalid(_))), "{foreign}");
        }
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

    #[tokio::test]
    async fn a_fresh_config_shows_what_is_new_once_after_an_update() {
        let _env = paths::test_env();
        let core = crate::core::Core::open_with(crate::config::Config::load().unwrap(), crate::host::Host::memory().0).await.unwrap();
        std::fs::create_dir_all(paths::last_version_file().parent().unwrap()).unwrap();
        std::fs::write(paths::last_version_file(), "0.0.0").unwrap();
        assert!(!core.whats_new().await.is_empty(), "on until turned off");
        assert!(core.whats_new().await.is_empty(), "once");
    }
}
