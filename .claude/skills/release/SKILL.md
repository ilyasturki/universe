---
name: release
description: Write the changelog entry, then bump, commit and tag the version.
disable-model-invocation: true
argument-hint: <patch|minor|major|X.Y.Z>
allowed-tools: Bash Read Write Edit Grep Glob AskUserQuestion
---

# Release

The tag's GitHub release body is this version's `CHANGELOG.md` section, word for word, and the AUR packages follow the tag: the entry is what users read.

## Context

Current version: !`sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"$/\1/p}' Cargo.toml`

Commits since the last tag, grouped by conventional type:

!`tools/release-notes HEAD`

Top of CHANGELOG.md:

!`head -40 CHANGELOG.md 2>/dev/null || echo "(no CHANGELOG.md yet)"`

## Steps

1. **Level.** `$ARGUMENTS` if it is `patch`, `minor`, `major` or `X.Y.Z`. Otherwise deduce it from the commits (0.0.x bumps patch even for breaking changes) and confirm with AskUserQuestion. Done when the new version number is fixed.

2. **Changelog entry.** Add `## [X.Y.Z]` at the top of `CHANGELOG.md`, under `# Changelog` (create the file with that heading if it is missing). Keep a Changelog categories, only those that apply, in this order:
    - **Breaking**: what a user must redo or change: config keys gone, data moved, a dependency dropped
    - **Added**: new features
    - **Changed**: changes to existing behaviour
    - **Fixed**: bug fixes
    - **Performance**: speed or resource gains
    - **Removed**: features taken out

    Entries are user-facing: what a player or a packager notices, in plain words, no type prefixes or scopes. Fold related commits into one entry. Build, CI, test, refactor, hook and dev-shell commits stay out unless a user feels them. Done when every commit above is either in an entry or deliberately left out.

3. **README.** Read the entry back and update `README.md` where it no longer matches (features, install paths, runners, commands). Skip for a fixes-only release.

4. **Bump.** `just bump <level>`. It runs `just check` first, then rewrites every copy of the version, commits it with `CHANGELOG.md` and `README.md` as `chore(release): vX.Y.Z`, and tags. If it fails, stop and report its output.

5. **Hand off.** Show the entry, and give `git push --follow-tags` for the user to run: the push publishes the GitHub release and the `universe` and `universe-bin` AUR packages.
