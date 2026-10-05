# Extensions

An extension is a module or a source that does not ship with Universe. The UI calls them
add-ons. It is the same folder a shipped one is (`docs/api.md`, Module protocol and Source
protocol): a `module.toml` or a `source.toml` at its root, and the programs it names. An extension
runs as you, with your files and your accounts: nothing sandboxes it.

## Layout

```
my-module/
  module.toml        # or source.toml: one of the two, never both
  bin/…              # the hooks or the source's exe the manifest names, executable
```

Universe looks extensions up in three places. On the same id, a later one wins:

1. `$XDG_DATA_HOME/universe/extensions/<module|source>/<id>/`: what `universe extension install` put
   there, with `<id>.json` beside each one (where it came from, which `update` fetches again);
2. the shipped ones (`$UNIVERSE_MODULES_PATH` / `$UNIVERSE_SOURCES_PATH`, then
   `$XDG_DATA_DIRS/universe/{modules,sources}`);
3. your own, `$XDG_CONFIG_HOME/universe/{modules,sources}/<id>/`, the folder to work on one in.

Installing an id that ships, or that your own folder holds, is refused.

## API number

The manifest's `api` names the extension API it was written for. This Universe reads **api 2**.
An extension whose `api` it does not read, or that names none, is refused at install. Found later,
after an update of Universe, it stays listed but unavailable: its hooks never run, its source
verbs are refused, and `universe doctor` shows an `extension-api` line for it, whether it is on or
off.

The API grows without a bump while every change is an addition: a new hook, field, setting key or
event that an older extension simply does not use. A change that would break an extension written
for api 2 bumps the number, and Universe then reads the range it still supports.

`[requires] core` (comparators on Universe's version) still applies on top, for an extension that
needs a feature added later in the same api.

A source's `[[tools]]` pins only stand in for its own programs: it pins nothing while it is off, a
shipped source's pin wins over an extension's of the same id, and no source runs another source's
pin.

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
one folder, as a release archive does. URLs are https:// only.

Installing does not switch an extension on: `universe module enable <id>` or
`universe source enable <id>` does, and so do the Modules and Sources pages. An update replaces the
folder in place. An index install takes the index's newer version. Any other install is fetched
again from its URL, archive or folder. Removing switches the extension off, then deletes its folder.
What it saved under `$XDG_DATA_HOME/universe/{modules,sources}/<id>/` stays.

In the UI, the Modules and Sources pages of every look end on a "Get more…" row. It lists the
index's add-ons of that kind and installs one after the same confirmation. The UI installs from the
index only. An unlisted extension installed from the CLI shows an Unlisted tag in the lists and can
be removed there.

## By hand

The CLI is not needed:

1. Put the folder under `$XDG_DATA_HOME/universe/extensions/module/<id>/` (or `source/<id>/`),
   named after its manifest's `id`. Make its programs executable.
2. Switch it on: `universe module enable <id>`, or `enabled` in `config.toml`'s `[modules]` or
   `[sources]`.

Without a `<id>.json` beside it, it reads as unlisted and `update` leaves it alone. To work on an
extension, use `$XDG_CONFIG_HOME/universe/modules/<id>/` instead: that folder wins over an
installed copy.

## The index

`extensions.index` in `config.toml`, or `UNIVERSE_EXTENSIONS_INDEX`, is an https:// URL or a file.
When empty it is the registry's,
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

`kind` is `module` or `source`. `url` is the archive and `sha256` its digest: an archive whose
digest differs is refused, and so is one whose manifest holds another id or kind than its entry.
The index is HTTPS plus these pins, with no signature. Universe reads schema 1 and refuses a
newer one.
