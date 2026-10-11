# The site

universe.ilyasturki.com: the landing page and the docs, built with Astro and Starlight. It documents the latest release.

```sh
just site          # build into site/dist: the reference pages are generated first, from this checkout's universe
just site dev      # the same, served with live reload
just site preview  # serve site/dist
just docs-media fetch   # the screenshots and clips, from the site-media branch (placeholders without them)
```

## Where things are

- `src/pages/index.astro`: the landing page.
- `src/content/docs/docs/<section>/<page>.md`: the docs, one folder per sidebar group. `.mdx` only for a page that shows a screenshot or a clip.
- `src/content/docs/docs/reference/`, `developers/` and `changelog.md` are generated at build time and gitignored: `scripts/generate.ts` writes them from `universe __generate --markdown`, `universe --json launch-keys`, `universe --json runner ls`, the config.toml block of `docs/api.md`, the Nix modules, `docs/*.md` and `CHANGELOG.md`. Change those sources, never the pages.
- `media/takes.toml`: every screenshot and clip a page shows, recorded by `just docs-media record` at each release, and the ones a release's notes show (`release = "X.Y.Z"`).

## Rules the build checks

- A page under `settings/` per section of `ui/universe_ui/qml/ui/Sections.js`, titled exactly as the app names it: a renamed or new section fails the build until its page follows.
- Links are absolute (`/docs/running/runners/`) and land on a page and an anchor that exist (starlight-links-validator, and `scripts/built.ts` for the landing page).
- `<Shot name="…" />` and `<Clip name="…" />` name a take of `media/takes.toml`, written literally. A take not recorded yet renders as a placeholder; `DOCS_STRICT=1` (the release deploy) refuses one.

## Writing a page

Name things as the app shows them, the Universe look first, and keep each fact to what the code does. A user-facing change updates its page, and its take when the screen changed, in the same commit. The copy follows the writing skill: no em or en dashes, no filler.
