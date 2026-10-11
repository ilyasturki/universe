---
title: Saves and cloud saves
description: Save backups after every session through ludusavi, restoring and exporting them, and GOG and Epic cloud saves, conflicts included.
sidebar:
  order: 9
---

Universe backs up a game's saves after each session through ludusavi, which knows where PC games keep them from PCGamingWiki's data. Universe fetches it the first time it needs it. GOG and Epic games can also sync their saves with the store's cloud.

## A backup after every session

**Back up after playing**, in Settings › **Game defaults** › **Saves** and the first run's **Where game data goes** step, is on by default. A backup is skipped when the saves haven't changed since the last one.

Each game keeps its last five backups; **Backups kept**, behind **Y**, takes 1 to 255. They go in the **Save backups** folder (Game defaults › **Folders**), by default `~/.local/share/universe/saves/<id>`. Universe's ludusavi keeps its settings and cache in `~/.local/share/universe/ludusavi/`, apart from a ludusavi you run yourself.

A Windows game's backup restores into whatever prefix the game has now, so it outlives **Move into Universe's Prefixes** and **Reset the Prefix…**, which backs up first.

ludusavi finds a game by its Steam id, its GOG id, then its title. When it knows no game by that title, the game isn't backed up, and its Data and saves page says why. Set the title ludusavi knows it by (`ludusavi find --fuzzy` finds it) from a terminal; the app has no field for it:

```sh
universe set hollow-knight saves.name="Hollow Knight"
```

### Emulated games

Universe backs up one game's saves at a time for RPCS3, shadPS4, Vita3K, Cemu and Dolphin, and the `.sav` beside a melonDS or mGBA ROM. Eden, Ryujinx, PCSX2, DuckStation, PPSSPP, Azahar, Mupen64Plus, ScummVM and xemu keep every game's saves in one shared folder, which the Data and saves page shows as **Every game's**. Nothing backs that folder up. A backup never holds keys, firmware or BIOS files.

## Backing up, restoring and exporting

The **Saves** section of a game's Data and saves page (game menu › **Manage** › **Data and saves**) shows the files ludusavi found, or **Not found** with the reason, then:

- **Back Up Now**, with the last backup's date or **None yet**;
- **Restore the Latest Backup**, which replaces the saves on disk now;
- **Export the Backups**, which writes every backup into one zip in your home folder, laid out as ludusavi lays out backups, so any ludusavi restores from it.

The **Backups** section lists every backup by date, and **A** on one restores it, after asking. Restoring is refused while the game runs.

From a terminal, [`universe saves`](/docs/reference/cli/saves/) does the same. `list` shows the saves found and the backups kept, `backup` takes one, `restore` puts back a backup by its id (the latest without one), and `export` writes the zip into a folder, the current one by default.

## Cloud saves

Cloud saves are off by default, and only a Windows build run through Proton or Wine syncs. Turn them on:

- for every game of the store, in Settings › Stores › **GOG** (or **Epic Games**) › **Game defaults** › **Cloud saves**;
- for one game, in the **GOG** (or **Epic Games**) card of its Game settings;
- from a terminal, with `universe source set gog cloud_saves=true`, and `--game <name>` for one game.

The saves then come down before each session and go up after it. The download gets about 34 seconds, after which the game starts on the saves on this device. Before a game's first sync, Universe backs its saves up, and a failed backup stops the sync.

The **Cloud Saves** row of the Data and saves page says how the last sync went:

- **Synced**, with its date;
- **Not synced yet**;
- **Changed on both sides**, a conflict;
- **Store out of reach**;
- **Not synced**, with the reason;
- **Not kept**, when the store keeps no cloud saves for the game or the build isn't a Windows one;
- **Off**.

The app has no button that syncs now. `universe saves cloud <name> --download` brings the cloud's saves down and `--upload` sends them up, as a session would. Neither runs while the game does.

### Settling a conflict

When the saves changed both on this device and in the cloud since their last sync, Universe touches neither. The game starts on the saves on this device, and the row reads Changed on both sides until you keep one side:

- **Keep the Saves on This Device**: the cloud's saves are replaced by these.
- **Keep the Cloud's Saves**: the saves here are backed up, then replaced.

`universe saves cloud <name> --keep-local` and `--keep-cloud` do the same from a terminal.

## Steam Cloud

Universe doesn't sync Steam Cloud, and Steam doesn't sync around a Universe session. The [Steam](/docs/library/steam/#steam-cloud) page says where those saves stay and when they reach the cloud.

## In the other looks

The Switch 2 look has a game's saves under its Software Options › **Data Management**, and the PS5 look under **Saved Data and Storage**.
