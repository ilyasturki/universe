---
title: Installing add-ons
description: Install, update and remove modules, stores and themes that others made, from Settings or with universe extension, and what Universe checks first.
sidebar:
  order: 2
---

An add-on is a module, a store or a theme that doesn't ship with Universe; the code and the command line call them extensions. Settings installs them from Universe's index. The [`universe extension`](/docs/reference/cli/extension/) command also takes a URL, an archive or a folder.

:::caution
An extension runs as you, with your files and your accounts: nothing sandboxes it, and a theme's QML can reach them as well as a program can.
:::

## From Settings

**Get more…** at the end of Settings › **Features**, **Stores** and **Appearance** opens the **Add-ons** menu for modules, stores or themes. It lists the index's add-ons of that kind and the installed ones the index doesn't list. Each row shows:

- its version and download size, when it isn't installed;
- **Installing…** while it installs;
- **Installed**, or **Installed · Unlisted** for one the command line installed from outside the index, which Settings can only remove;
- **Update** and the new version, when the index has a newer one;
- **Needs another Universe**, when it was written for an extension API this Universe doesn't read. It can't be installed.

Pick an add-on to install it, or, once installed, to **Update to** the new version or **Remove** it. Removing turns it off, and a removed theme can't be picked. Universe installs one add-on at a time.

Installing never turns anything on. Turn a module on in Features and a store in Stores, and pick a theme in Appearance ([Themes](/docs/add-ons/themes/)).

In Universe Desktop, Preferences › **Features** and Preferences › **Stores** end on the same **Get more…** row. It has none for themes.

## The index

The index is a JSON file giving each add-on's id, kind, name, version, description, extension API, download URL, size and sha256. Universe reads `https://raw.githubusercontent.com/ilyasturki/universe-extensions/index/index.json` unless config.toml's `extensions.index`, or the `UNIVERSE_EXTENSIONS_INDEX` variable, names another, as an https:// URL or a file ([config.toml](/docs/reference/config/#extensions)).

Before Universe puts an add-on in place, it checks that:

- it came over https://;
- the archive's sha256 is the one the index lists, and the manifest inside holds the id, kind and version of its entry;
- it holds one manifest, `module.toml`, `source.toml` or `theme.toml`, and that manifest parses (a store's must name its program, and a theme needs its `theme.qml`);
- its id is made of lowercase letters, digits, `-` and `_`, and isn't taken by a shipped module, store or look, a folder of your own, or an add-on of another kind;
- it was written for extension API 2, the one this Universe reads;
- a module or a store that only runs on some versions of Universe accepts this one.

The index is HTTPS plus these pins, with no signature.

The version check runs again at every start. After you update Universe, a module or a store that doesn't accept the new version reads **Unavailable**, and [System check](/docs/settings/system-check/) says to update it or Universe, or to turn it off. An add-on written for an API this Universe no longer reads stays listed but can't run, and such a theme can't be picked.

## Updates

Nothing updates an add-on by itself. Update one from the Add-ons menu, or from a terminal:

```sh
universe extension update                  # every installed add-on
universe extension update now-playing
```

An add-on from the index only moves to a newer version, compared as X.Y.Z numbers, and only to one written for this Universe's API. One installed from a URL, an archive or a folder is fetched again from there. One you put in place by hand is left alone.

## From a terminal

`universe extension` (also `universe extensions`) installs from the index, a URL, an archive or a folder:

```sh
universe extension ls                                    # the index's add-ons and the installed ones
universe extension install now-playing                   # an id from the index
universe extension install https://example.org/now-playing-1.2.0.tar.gz
universe extension install ./now-playing.tar.gz          # an archive
universe extension install ~/src/now-playing             # a folder, copied
universe extension remove now-playing
```

A bare word is always an id from the index. A path holds a `/` (`./now-playing`), or starts with `file://`. An archive is a tar (plain, gzip or xz) or a zip, with the add-on's folder at its root or in its one folder. A folder is copied without its `.git`, so later edits need `universe extension update`.

The command shows what it installs and where it comes from, then asks first. `--yes` (`-y`) accepts without asking. Without a terminal, or with `--json`, it refuses unless you pass `--yes`. Then `universe module enable <id>` or `universe source enable <id>` turns the add-on on.

`universe extension remove` turns a module or a store off, then deletes its folder and its `<id>.json`. What it saved under `~/.local/share/universe/modules/<id>/` (or `sources/<id>/`) stays. A shipped module or store can't be removed (turn it off instead), and neither can a built-in look.

## While a game runs

A new add-on installs while a game runs. Replacing an installed one, updating and removing are refused until the game ends; run them again once you've quit it.

## By hand

1. Put the add-on's folder in `~/.local/share/universe/extensions/module/<id>/` (or `source/<id>/`, `theme/<id>/`), named after its manifest's `id`, and make its programs executable.
2. Turn it on with `universe module enable <id>`, in Settings › Features, or in the `enabled` list of `[modules]` or `[sources]` in config.toml. A theme shows in Settings › Appearance the next time you open it.

Without a `<id>.json` beside it, it reads as Unlisted.

To work on a module or a store, use a [folder of your own](/docs/add-ons/#where-modules-stores-and-themes-live) instead. A theme has none ([Working on a theme](/docs/add-ons/themes/#working-on-a-theme)).

## Writing one

A module or a store is a folder with a manifest and the programs it names; a theme is a `theme.toml` and a QML tree. [Extensions](/docs/developers/extensions/) has the layout, the API number, the index's format and how an add-on gets into the index.
