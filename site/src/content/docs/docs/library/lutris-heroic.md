---
title: Lutris and Heroic
description: Bring in the games Lutris and Heroic installed, Lutris's play time included.
sidebar:
  order: 5
---

Universe takes over the games other launchers installed, without moving a file. Lutris games come with their play time. Heroic's GOG and Epic games, Steam's and the itch app's go to Universe's store of the same name, which updates them from then on.

## Bringing them in

The first run's **What's on this machine** step lists each launcher with **Add** ([First run](/docs/getting-started/first-run/)). A store you aren't signed in to finds nothing until you sign in.

Later, **Add a game** › **Import from Lutris**, at the end of the Library, imports the Lutris games new to Universe.

## What comes along from Lutris

- The play time, the hidden flag and the year.
- A Wine game's launch settings, on the Proton or Wine build Lutris ran it with. Its prefix stays where Lutris made it, until [**Move into Universe's Prefixes**](/docs/library/disk-space/#what-one-game-takes) moves it.
- A GOG game Lutris installed becomes a GOG game, which the [GOG store](/docs/library/gog/) then updates.
- Every other game counts as added by hand: Lutris is where it came from, not its store.
- An emulated game keeps its emulator.

Only installed games come over, and games already in your library stay as they are.

## Heroic

- Heroic's GOG games go to the GOG store, and Heroic's Epic games to the Epic Games store once you're signed in ([Epic Games](/docs/library/epic/#heroics-epic-games)).
- Heroic's Amazon games stay in Heroic, since they launch through it.

No play time comes over from Heroic, and no prefix: each game gets a new one, and its saves in Heroic's prefix stay there. Even with the store's [cloud saves](/docs/library/saves/#cloud-saves) on, the first launch starts without the cloud's copy.
