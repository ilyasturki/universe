---
title: Saves and cloud saves
description: Save backups after every session through ludusavi, restoring and exporting them, and GOG and Epic cloud saves, conflicts included.
sidebar:
  order: 9
---

Universe backs up a game's saves each time a session ends. ludusavi does the work. It knows where PC games keep their saves, from PCGamingWiki's data, and Universe fetches it the first time it needs it. GOG and Epic games can also sync their saves with the store's cloud, before and after each session.

## A backup after every session

**Back up after playing**, in Settings › **Game defaults** › **Saves**, is on by default; the first run's **Where game data goes** step has it too. A session's end then backs up the game's saves, unless they haven't changed since the last backup.

Each game keeps its last five backups, the oldest going first. **Backups kept**, which **Y** reveals, takes 1 to 255. They're kept in the **Save backups** folder (Game defaults › **Folders**), by default `~/.local/share/universe/saves/<id>`. ludusavi keeps its own settings and cache in `~/.local/share/universe/ludusavi/`, apart from a ludusavi you run yourself.

A Windows game's backup records the prefix's files under a fixed path, so a restore lands in whatever prefix the game has now. A backup outlives **Move into Universe's Prefixes** and **Reset the Prefix…**, which backs up first.

ludusavi finds a game by its Steam id, its GOG id, then its title. When it knows no game by that title, the backup after each session passes the game over, and its Data and saves page says why. Set the title ludusavi uses (`ludusavi find --fuzzy` finds it) from a terminal, since the app has no field for it:

```sh
universe set hollow-knight saves.name="Hollow Knight"
```

### Emulated games

Universe backs up one game's saves at a time for RPCS3, shadPS4, Vita3K, Cemu and Dolphin, and the `.sav` beside a melonDS or mGBA ROM. Eden, Ryujinx, PCSX2, DuckStation, PPSSPP, Azahar, Mupen64Plus, ScummVM and xemu keep every game's saves in one shared folder, which the Data and saves page shows as **Every game's**. Nothing backs that folder up. A backup never holds keys, firmware or BIOS files.

## Backing up, restoring and exporting

A game's saves are on its Data and saves page, which the game menu (**Start** on the game) opens from **Manage** › **Data and saves**. Its **Saves** section shows the files ludusavi found, or **Not found** with the reason, then:

- **Back Up Now**, valued with the last backup's date, or **None yet**;
- **Restore the Latest Backup**, which replaces the saves on disk now;
- **Export the Backups**, which writes every backup into one zip in your home folder. Unpacked, it's laid out the way ludusavi lays out backups, so any ludusavi restores from it.

The **Backups** section lists every backup by date, and **A** on one restores it, after asking. Restoring is refused while the game runs.

From a terminal, [`universe saves`](/docs/reference/cli/saves/) does the same. `list` shows the saves found and the backups kept, `backup` takes one, `restore` puts back a backup by its id (the latest without one), and `export` writes the zip into a folder, the current one by default.

## Cloud saves

GOG and Epic games can sync their saves with the store. It's off by default, and only a Windows build run through Proton or Wine syncs. Turn it on:

- for every game of the store, in Settings › Stores › **GOG** (or **Epic Games**) › **Game defaults** › **Cloud saves**;
- for one game, in the **GOG** (or **Epic Games**) card of its Game settings;
- from a terminal, with `universe source set gog cloud_saves=true`, and `--game <name>` for one game.

The saves then come down before each session and go up after it. The download gets about 34 seconds; when it hasn't finished by then, the game starts on the saves on this device. Before a game's first sync, Universe backs its saves up, and a failed backup stops the sync.

The **Cloud Saves** row of the Data and saves page says how the last sync went:

- **Synced**, with its date;
- **Not synced yet**;
- **Changed on both sides**, a conflict;
- **Store out of reach**, when GOG or Epic couldn't be reached;
- **Not synced**, with the reason;
- **Not kept**, when the store keeps no cloud saves for the game or the build isn't a Windows one;
- **Off**, while Cloud saves is off for the game.

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
