---
name: release
description: Write the changelog entry, refresh the docs and their media, then bump, commit and tag the version.
disable-model-invocation: true
argument-hint: <patch|minor|major|X.Y.Z>
allowed-tools: Bash Read Write Edit Grep Glob AskUserQuestion
---

# Release

The tag's GitHub release body is this version's `CHANGELOG.md` section, word for word, the apps bundle the file, and the AUR packages follow the tag: the entry is what users read.

## Context

Current version: !`sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"$/\1/p}' Cargo.toml`

Release readiness: !`just release-ready 2>&1 | head -1`

Commits since the last tag, grouped by conventional type:

!`tools/release-notes HEAD`

Top of CHANGELOG.md:

!`head -40 CHANGELOG.md 2>/dev/null || echo "(no CHANGELOG.md yet)"`

## Steps

0. **Ready.** The release readiness line above reads `ci green on <sha>, secrets set`. Any other line: stop and report it, so the user pushes and waits for ci, fixes it, or sets the secret it names.

1. **Level.** `$ARGUMENTS` if it is `patch`, `minor`, `major` or `X.Y.Z`. Otherwise deduce it from the commits (0.0.x bumps patch even for breaking changes) and confirm with AskUserQuestion. Done when the new version number is fixed.

2. **Changelog entry.** Add `## [X.Y.Z] - YYYY-MM-DD` (today) at the top of `CHANGELOG.md`, under `# Changelog`, and its compare link at the top of the links at the bottom: `[X.Y.Z]: https://github.com/ilyasturki/universe/compare/vPREV...vX.Y.Z`. Keep a Changelog categories, only those that apply, in this order:
    - **Breaking**: what a user must redo or change: config keys gone, data moved, a dependency dropped
    - **Added**: new features
    - **Changed**: changes to existing behaviour
    - **Fixed**: bug fixes
    - **Performance**: speed or resource gains
    - **Removed**: features taken out

    Entries are user-facing: what a player or a packager notices, in plain words, no type prefixes or scopes. Fold related commits into one entry. Build, CI, test, refactor, hook and dev-shell commits stay out unless a user feels them. The apps show the section too (Settings › About, the what's-new page, Universe Desktop's About), so keep to one line per entry and plain Markdown: backticks, no links. Done when every commit above is either in an entry or deliberately left out, and `tools/changelog check X.Y.Z` passes.

3. **README.** Read the entry back and update `README.md` where it no longer matches (features, install paths, runners, commands). Skip for a fixes-only release.

4. **Docs.** The site documents the latest release: it is rebuilt from the tag with the media of the `site-media` branch, so this release's pages and media ship with it. This step follows the changelog entry because `record` stamps the media with the entry's version.
    1. For each entry, find the guide pages under `site/src/content/docs/docs/` that cover it (grep its setting, command or screen name) and fix what it made wrong. A new feature with a screen of its own gets a take at the end of `site/media/takes.toml` and a `<Shot name="…" />` or `<Clip name="…" />` on its page; the head of takes.toml and `site/README.md` have the rules. `just site build` passes, its link validator included.
    2. `just showcase` (rebuilds `.dev-showcase` from `.dev`), then `UNIVERSE_DEV=.dev-showcase just docs-media record`. It films every take on the GPU in a headless compositor, no window, and its gamescope clips launch Dead Cells in the showcase profile.
    3. Read `target/docs-media/contact.png`. A take that failed, or whose tile shows another screen than its page describes or a half-drawn one: fix its keys, record it alone (`UNIVERSE_DEV=.dev-showcase just docs-media record <name>`) and Read the sheet again.
    4. `just docs-media check --strict X.Y.Z`.
    5. `just docs-media publish X.Y.Z`: the media become the single commit of the local `site-media` branch.

    Done when every tile on the contact sheet shows what its page says, the strict check passes, and publish printed `site-media`'s commit.

5. **Bump.** `just bump <level>`. It runs `just release-ready` and `just check` first, then `docs-media check --strict` on the new version, rewrites every copy of the version, writes the entry into the metainfo's `<release>` (`tools/changelog metainfo`), commits it with `CHANGELOG.md`, `README.md`, the pages under `site/src/content/docs` and `site/media/takes.toml` as `chore(release): vX.Y.Z`, and tags. A hotfix the user wants out on the last release's media skips step 4's recording and publishing, and runs `BUMP_SKIP_MEDIA=1 just bump <level>`. If it fails, stop and report its output.

6. **Hand off.** Show the entry, and give `git push --follow-tags origin main && git push --force-with-lease origin site-media` for the user to run (its first push alone after `BUMP_SKIP_MEDIA`): the tag's release run builds the tarball and installs and starts it on Arch, Ubuntu 22.04 and 24.04 and Fedora (the smoke job; `just release-smoke` runs it locally) while it waits for the ci run main's push started on the same commit. Only once both are green does it publish the GitHub release "Universe X.Y.Z" (`tools/changelog notes X.Y.Z` is its body), upload the tarball, publish the `universe` and `universe-bin` AUR packages, rebuild the site from the tag with the media `site-media` holds, and queue the COPR build, whose page the run's summary links: COPR builds it for about 50 minutes after the run ends. A ci failure that is not the code's: `just ci-retry`, then rerun the release run's failed jobs.
