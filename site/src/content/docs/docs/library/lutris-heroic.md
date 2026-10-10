---
title: Lutris and Heroic
description: See what Lutris, Heroic and other launchers hold with universe discover, import Lutris games with their play time, and adopt Heroic's installs.
sidebar:
  order: 5
---

Universe takes over the games other launchers installed without moving a file. Lutris games are imported with their play time. Heroic's GOG and Epic games, Steam's, and the itch app's go to Universe's store of the same name, which updates them from then on.

## What's on this machine

[`universe discover`](/docs/reference/cli/discover/) reports what other launchers hold, and writes nothing. It covers Lutris, Steam, Heroic's stores (**Heroic · GOG**, **Heroic · Epic Games** and **Heroic · Amazon Games**), the itch app and the emulators' game folders. Each line says "not installed", "no games", how many games there are to import, or how many it found that Universe can't take over, then names the first few. Heroic's Amazon games are of that last kind, since they launch through Heroic.

The first run's **What's on this machine** step lists the same launchers ([First run](/docs/getting-started/first-run/)):

- **Add** on a launcher takes its games in, and **Add everything** runs every launcher, one after the other.
- The **Steam**, **Heroic · Epic Games** and **itch** rows turn their store on in Universe if it was off. For Heroic's GOG games, Universe first adds Heroic's GOG folders to the GOG store's **Scanned folders**.
- A store you haven't signed in to may find nothing yet. Its row then reads **Waiting for a sign-in**, and runs again once you sign in on the **Your stores** step.
- **Runners your games need** lists the runners the games found need and Universe can install, each with **Install**.

Settings › About › **First-run setup** › **Run again** brings the step back later.

## Importing Lutris games

On the Library, the last tile, **Add a game**, opens a page with **Import from Lutris**. Universe reads Lutris's library, then asks to import the games that are new to it, and fetches their art once they're in.

From a terminal, [`universe migrate`](/docs/reference/cli/migrate/) reports what it would import, skip and update, and `universe migrate --apply` writes it. Only the games Lutris has installed are read.

## What comes along from Lutris

- The play time. Lutris's hours become one session, ended on the day Lutris last saw the game played. They're imported once, so a second import doesn't count them twice.
- The game's hidden flag and its year.
- For a Wine game, the launch as Lutris had it: the program, the prefix, the working folder, the architecture, the arguments, the environment variables, the commands before and after, Esync and Fsync, HDR, the DLL overrides, MangoHud, the frame rate limit, and Lutris's prefix command, split into variables and a wrapper. It becomes a Proton game on the Proton build Lutris named, or a Wine game when Lutris ran it on a Wine build or on the system's Wine.
- A GOG game Lutris installed through its GOG service becomes a GOG game, which the [GOG store](/docs/library/gog/) then updates.
- An emulator game keeps its emulator, under Universe's name for it.
- A runner program from Lutris that Universe can't find by itself is written to `[runners.<id>] exe` in config.toml.

## What stays behind

Games already in your library are kept as they are, and Lutris games that aren't installed stay in Lutris.

A Wine game's prefix stays where Lutris made it. Its Data and saves page calls it **Lutris's prefix** and offers **Move into Universe's Prefixes**, which moves it into **Windows game folders**. After the move, Lutris still points at the old path.

## Heroic

Universe has no Heroic importer. It sends Heroic's games to its own stores:

- Heroic's GOG games go to the GOG store. Their folders join GOG's Scanned folders, and a scan finds the games there. `universe discover` prints those folders, and from a terminal `universe source set gog scan_dirs=<folders>` then `universe scan gog` does the rest.
- Heroic's Epic games go to the Epic Games store, which imports them through legendary once you're signed in ([Epic Games](/docs/library/epic/#heroics-epic-games)).
- Heroic's Amazon games stay in Heroic.

No play time comes over from Heroic. Neither store hands Universe a prefix either, so a game adopted from Heroic runs in a new prefix of its own under Windows game folders, and the saves it kept in Heroic's prefix stay there. With the store's [cloud saves](/docs/library/saves/#cloud-saves) on, the first launch still starts without the cloud's copy: the new prefix isn't made yet when the saves would come down.
