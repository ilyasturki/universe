# Extensions

An extension is a module, a source or a theme that does not ship with Universe. The UI calls them
add-ons. A module or a source is the same folder a shipped one is (`docs/api.md`, Module protocol
and Source protocol): a `module.toml` or a `source.toml` at its root, and the programs it names. A
theme is a look for Universe Big Screen: a `theme.toml` and its own QML tree, whose root is
`theme.qml` (`docs/frontends.md`, Themes). An extension runs as you, with your files and your
accounts: nothing sandboxes it, and a theme's QML can reach them as well as a program can.

## Layout

```
my-module/
  module.toml        # or source.toml: one kind's manifest, never two
  bin/…              # the hooks or the source's exe the manifest names, executable

my-theme/
  theme.toml
  theme.qml          # the look's root, loaded in place of a built-in one
  screenshot.png     # what theme.toml's `screenshot` names: shown beside its name
  …                  # the rest of its QML tree, its pictures and sounds
```

Every manifest starts with the same fields: `api`, `id`, `name`, `version`, `description`, and
optionally `author`, `license` and `screenshot` (a picture inside the folder, by its path there).
`examples/theme/` is a theme of a few files to start from:

```toml
api = 2
id = "sample"
name = "Sample"
version = "1.0.0"
description = "The smallest theme: the library as a row of covers, and every look a Start away."
author = "Ilyas Turki"
license = "MIT"
screenshot = "screenshot.png"
```

Universe looks extensions up in three places. On the same id, a later one wins:

1. `$XDG_DATA_HOME/universe/extensions/<module|source|theme>/<id>/`: what `universe extension install`
   put there, with `<id>.json` beside each one (where it came from, which `update` fetches again);
2. the shipped ones (`$UNIVERSE_MODULES_PATH` / `$UNIVERSE_SOURCES_PATH`, then
   `$XDG_DATA_DIRS/universe/{modules,sources}`);
3. your own, `$XDG_CONFIG_HOME/universe/{modules,sources}/<id>/`, the folder to work on one in.

A theme has only the first: the built-in looks (`reprise`, `switch2`, `ps5`) are not folders, and
there is no folder of your own for themes.

Installing an id that ships (a built-in look's, for a theme), or that your own folder holds, is
refused, and so is an id another kind holds: two kinds never share one.

## API number

The manifest's `api` names the extension API it was written for. This Universe reads **api 2**.
An extension whose `api` it does not read, or that names none, is refused at install. Found later,
after an update of Universe, it stays listed but unavailable: its hooks never run, its source
verbs are refused, a theme is listed in Settings › Themes but cannot be picked, and
`universe doctor` shows an `extension-api` line for it, whether it is on or off.

The API grows without a bump while every change is an addition: a new hook, field, setting key,
event or `api` member that an older extension simply does not use. For a theme, the API is the part
of the QML `api` object `docs/frontends.md` lists under What a theme can rely on. A change that
would break an extension written for api 2 bumps the number, and Universe then reads the range it
still supports.

`[requires] core` (comparators on Universe's version) still applies on top, for an extension that
needs a feature added later in the same api.

A source's `[[tools]]` pins only stand in for its own programs: it pins nothing while it is off, a
shipped source's pin wins over an extension's of the same id, and a pin is only fetched for the
source that declares it. Once fetched, its program sits in `$XDG_DATA_HOME/universe/bin` like every
fetched tool.

## Installing

```
universe extension ls                       # the index's extensions and the installed ones
universe extension install now-playing      # an id from the index
universe extension install https://example.org/now-playing-1.2.0.tar.gz
universe extension install ./now-playing.tar.gz
universe extension install ~/src/now-playing    # a folder, copied
universe extension update [id]              # every installed one when no id is given
universe extension remove now-playing
```

An install asks first. It names the extension's kind, says it runs programs as you, and says where
it comes from: the index, or Unlisted for a URL, an archive or a folder. `--yes` accepts without
asking. An archive is a tar (plain, gzip or xz) or a zip, holding the folder at its root or in its
one folder, as a release archive does. URLs are https:// only. A path holds a `/` (`./now-playing`):
a bare word is always an id of the index. Replacing an installed extension waits until no game
runs.

Installing does not switch an extension on: `universe module enable <id>` or
`universe source enable <id>` does, and so do the Modules and Sources pages. A theme is picked in
Settings › Themes. An update replaces the folder in place. An index install takes the index's
newer version. Any other install is fetched again from its URL, archive or folder. Removing
switches a module or a source off, then
deletes its folder; removing the theme in use puts the default look back. What it saved under
`$XDG_DATA_HOME/universe/{modules,sources}/<id>/` stays.

A theme that fails to load puts the default look back, with a notice. One that loads but leaves no
way out, because it breaks once running or offers no other look, is left by holding HOME on the
pad for 5 s: Universe reads that hold itself, whatever the theme's QML does, puts the default look
back with the same notice, and no longer remembers the theme as the look to start on.

In the UI, the Modules and Sources pages of every look end on a "Get more…" row, and Settings ›
Themes has one after the looks. It lists the index's add-ons of that kind and installs one after the
same confirmation. The UI installs from the index only. An unlisted extension installed from the
CLI shows an Unlisted tag in the lists and can be removed there.

## By hand

The CLI is not needed:

1. Put the folder under `$XDG_DATA_HOME/universe/extensions/module/<id>/` (or `source/<id>/`,
   `theme/<id>/`), named after its manifest's `id`. Make its programs executable.
2. Switch it on: `universe module enable <id>`, or `enabled` in `config.toml`'s `[modules]` or
   `[sources]`; a theme shows in Settings › Themes the next time it opens.

Without a `<id>.json` beside it, it reads as unlisted and `update` leaves it alone. To work on an
extension, use `$XDG_CONFIG_HOME/universe/modules/<id>/` instead: that folder wins over an
installed copy. A theme has no such folder: install it from the folder you work in, then
`universe extension update <id>` copies it again, which the UI loads at its next start.

## The index

`extensions.index` in `config.toml`, or `UNIVERSE_EXTENSIONS_INDEX`, is an https:// URL or a file.
When empty it is the one the registry (below) publishes,
`https://raw.githubusercontent.com/ilyasturki/universe-extensions/index/index.json`:

```json
{
  "schema": 1,
  "extensions": [
    {
      "id": "now-playing",
      "kind": "module",
      "name": "Now Playing",
      "version": "1.2.0",
      "description": "Shares the game you play as your chat status.",
      "api": 2,
      "url": "https://example.org/now-playing-1.2.0.tar.gz",
      "sha256": "…",
      "size": 48000,
      "homepage": "https://example.org/now-playing"
    }
  ]
}
```

`kind` is `module`, `source` or `theme`. `url` is the archive and `sha256` its digest: an archive
whose digest differs is refused, and so is one whose manifest holds another id, kind or version than
its entry. A `url` that names a file is only read from an index that is a file itself. An update that
the index lists for another `api` is not offered. The index is HTTPS plus these pins, with no
signature. Universe reads schema 1 and refuses a newer one.

The registry is [ilyasturki/universe-extensions](https://github.com/ilyasturki/universe-extensions).
To list an extension there, open a pull request adding `extensions/<id>.toml` to its `main` branch.
CI writes `index.json` to the `index` branch from those entries, so nobody edits it by hand. The
registry's README says what an entry holds and what a pull request's check covers.
