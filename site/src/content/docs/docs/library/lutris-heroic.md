---
title: Lutris and Heroic
description: See what Lutris, Heroic and other launchers hold with universe discover, import Lutris games with their play time, and adopt Heroic's installs.
sidebar:
  order: 5
---

Universe takes over the games other launchers installed without moving a file. Lutris games come with their play time. Heroic's GOG and Epic games, Steam's and the itch app's go to Universe's store of the same name, which updates them from then on.

## What's on this machine

[`universe discover`](/docs/reference/cli/discover/) reports what Lutris, Steam, Heroic's stores (**Heroic · GOG**, **Heroic · Epic Games** and **Heroic · Amazon Games**), the itch app and the emulators' game folders hold, and writes nothing. Each line gives how many games there are to import, or how many Universe can't take over, such as Heroic's Amazon games, which launch through Heroic.

The first run's **What's on this machine** step lists the same launchers, each with **Add** ([First run](/docs/getting-started/first-run/)):

- The **Steam**, **Heroic · Epic Games** and **itch** rows turn their store on in Universe if it was off. For Heroic's GOG games, Universe first adds Heroic's GOG folders to the GOG store's **Scanned folders**.
- A store you haven't signed in to may find nothing yet. Its row then reads **Waiting for a sign-in**, and runs again once you sign in on the **Your stores** step.

## Importing Lutris games

The Library's last tile, **Add a game**, opens a page with **Import from Lutris**. It asks to import the games new to Universe, then fetches their art.

[`universe migrate`](/docs/reference/cli/migrate/) reports what it would import, skip and update, and `universe migrate --apply` writes it. Only the games Lutris has installed are read.

## What comes along from Lutris

- The play time, as one session ended on the day Lutris last saw the game played. A second import doesn't count it twice.
- The game's hidden flag and its year.
- For a Wine game, the launch as Lutris had it: the program, the prefix, the working folder, the architecture, the arguments, the environment variables, the commands before and after, Esync and Fsync, HDR, the DLL overrides, MangoHud, the frame rate limit, and Lutris's prefix command, split into variables and a wrapper. It becomes a Proton game on the Proton build Lutris named, or a Wine game when Lutris ran it on a Wine build or on the system's Wine.
- A GOG game Lutris installed through its GOG service becomes a GOG game, which the [GOG store](/docs/library/gog/) then updates.
- An emulator game keeps its emulator, under Universe's name for it.
- A runner program from Lutris that Universe can't find by itself is written to `[runners.<id>] exe` in config.toml.

## What stays behind

Games already in your library are kept as they are, and Lutris games that aren't installed stay in Lutris.

A Wine game's prefix stays where Lutris made it, until [**Move into Universe's Prefixes**](/docs/library/disk-space/#what-one-game-takes) moves it. Lutris then still points at the old path.

## Heroic

Universe has no Heroic importer. It sends Heroic's games to its own stores:

- Heroic's GOG games go to the GOG store: their folders join GOG's Scanned folders, where a scan finds them. From a terminal, `universe discover` prints those folders, then `universe source set gog scan_dirs=<folders>` and `universe scan gog` do the rest.
- Heroic's Epic games go to the Epic Games store, which imports them through legendary once you're signed in ([Epic Games](/docs/library/epic/#heroics-epic-games)).
- Heroic's Amazon games stay in Heroic.

No play time comes over from Heroic, and no prefix: a game adopted from Heroic runs in a new prefix of its own under Windows game folders, and the saves in Heroic's prefix stay there. Even with the store's [cloud saves](/docs/library/saves/#cloud-saves) on, the first launch starts without the cloud's copy, since the new prefix doesn't exist yet when the saves would come down.
