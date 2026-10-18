---
title: Saves and cloud saves
description: Save backups after every session, restoring them, and GOG and Epic cloud saves.
sidebar:
  order: 9
---

Universe backs up a game's saves after each session. GOG and Epic games can also sync their saves with the store's cloud.

## A backup after every session

**Back up after playing**, in Settings › **Game defaults**, is on by default. Each game keeps its last five backups.

When Universe can't find a game's saves, its **Data and saves** page says why. When the save database, ludusavi, knows the game under another title, set that title from a terminal, as in `universe set hollow-knight saves.name="Hollow Knight"`.

Some emulators, such as Eden, Ryujinx and PCSX2, keep every game's saves in one shared folder. The Data and saves page shows it as **Every game's**, and nothing backs it up.

## Backing up and restoring

The game menu (**Start** on a game) › **Manage** › **Data and saves** has:

- **Back Up Now**;
- **Restore the Latest Backup**, which replaces the saves on disk;
- **Export the Backups**, which writes every backup into one zip in your home folder;
- the list of backups, where **A** restores one, after asking.

Restoring is refused while the game runs.

## Cloud saves

Cloud saves are off by default, and only a Windows build run through Proton or Wine syncs. Turn them on for every game of a store in Settings › Stores › **GOG** (or **Epic Games**), or for one game in the **GOG** or **Epic Games** card of its Game settings.

The saves then come down before each session and go up after it. The **Cloud Saves** row of the game's Data and saves page says how the last sync went.

### Settling a conflict

When the saves changed both here and in the cloud, Universe touches neither: the game starts on the saves here, and the row reads **Changed on both sides** until you pick one side.

- **Keep the Saves on This Device** replaces the cloud's saves with these.
- **Keep the Cloud's Saves** backs up the saves here, then replaces them.

## Steam Cloud

Universe doesn't sync Steam Cloud. The [Steam](/docs/library/steam/#steam-cloud) page says where those saves stay.
