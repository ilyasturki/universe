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

A page tells a player what to press and what happens. It opens with what it's for in one or two sentences, names things as the Universe look shows them, and stays short: about 250 words for a settings page, 500 for the others.

- Leave out what a player doesn't need: timings, file paths, config keys, ids, detection orders, kernel and driver notes, former names, and the commands the CLI reference covers. They belong in the generated reference, the developer docs (`docs/*.md`) or nowhere.
- The Switch 2 and PS5 looks get one line pointing to Looks (`using/looks.mdx`), which has a short section for each.
- An edge case stays only when a player would otherwise get stuck.
- Each fact lives on one page, and the others link to it.
- Every sentence is true of the code, but a page doesn't cover all the code does. A change adds a sentence where its fact lives, and cuts one when the page grows past its length.

A user-facing change updates its page, and its take when the screen changed, in the same commit. The copy follows the writing skill: no em or en dashes, no filler.
