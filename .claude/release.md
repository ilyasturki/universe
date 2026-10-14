The apps show the section too (Settings › About, the what's-new page, Universe Desktop's About): one line per entry, plain Markdown, backticks but no links.

Highlights are the 1 to 4 entries a player sees most, picked with AskUserQuestion, each with its image line (the head of `site/media/takes.toml` has the form); a fixes-only hotfix may have none. `tools/changelog check X.Y.Z` passes and `tools/changelog media X.Y.Z` lists the assets.

The site documents the latest release, so its pages and media ship with it, recorded after the entry since `record` stamps them with its version:

- The guide pages under `site/src/content/docs/docs/` agree with every entry, a new screen has its take and its `<Shot>` or `<Clip>` (`site/README.md`), and `just site build` passes.
- `just showcase`, then `UNIVERSE_DEV=.dev-showcase just docs-media record`.
- Every tile of `target/docs-media/contact.png` shows what its page or highlight says; re-record a wrong one alone.
- `just docs-media check --strict X.Y.Z`, then `just docs-media publish X.Y.Z`.

The push also carries `git push --force-with-lease origin site-media`. COPR keeps building for about 50 minutes after the release run; its page is in the run's summary.
