---
title: Add a game by hand
description: Add a program or a ROM from its file, choose the runner it starts through, and fetch the art a hand-added game doesn't get by itself.
sidebar:
  order: 7
---

A Windows `.exe`, a Linux program, a ROM or a disc image goes into the library by hand. You pick the file, and Universe proposes the runner and the title.

## From the Library

The last tile of the Library grid is **Add a game**, and with an empty library Home offers it too. It opens a page of the same name:

- **Pick a game file…** adds a file;
- a row per store opens Settings › Downloads on that store, or the store's page while it's off or signed out;
- **Import from Lutris** brings in Lutris's games ([Lutris and Heroic](/docs/library/lutris-heroic/)).

To add a file:

1. Press **A** on Pick a game file…, and choose the file in **Game file**.
2. Pick the **Runner** it starts through.
3. Keep or change the **Title of the game**, which Universe proposes from the file's name.

The toast names the runner the game was added through.

A runner's page has a row of its own at the end of its **Games** card. Settings › Emulators & Proton › a runner › **Add a game…** asks for the file and the title, and skips the runner question.

## Which runner a file gets

The Runner list starts with the runners that open the file's extension. In each part of the list, the runners Universe finds come before the missing ones, and Proton comes before Wine. Linux takes a file with no extension, a `.sh`, a `.x86_64`, a `.x86` or an `.AppImage`. [Runners](/docs/reference/runners/) lists every runner's extensions.

Universe Big Screen doesn't ask for a platform. The game takes its runner's first one (Dolphin's is GameCube), and Game settings › **Launch** › **Platform** changes it, next to Runner. A Windows game gets a prefix of its own in **Windows game folders**. [Runners](/docs/running/runners/) covers the rest.

## From a terminal

```sh
universe add ~/Games/roms/melee.iso --runner dolphin
universe add ~/Games/Celeste/Celeste.exe --runner proton --title Celeste --media
```

[`universe add`](/docs/reference/cli/add/) takes the file and a runner id; an alias such as `yuzu` or `citra` works too. `--title` sets the title, `--platform` the platform, and `--media` fetches the art right away. When the runner's program is missing, it says how to install it. A game's id comes from its title, so a title already in the library is refused. Give the second game another `--title`.

## Its artwork

A game you add by hand gets no art by itself, in Universe Big Screen as in Universe Desktop. To fetch it:

- open the game menu (**Start** on the game) › **Manage** › **Artwork**, then **Start** › **Fetch missing art**;
- or Settings › **Artwork** › Fetch missing art, for every game missing some;
- or, from a terminal, [`universe media`](/docs/reference/cli/media/) with the game's name:

```sh
universe media celeste refresh
```

[Artwork](/docs/library/artwork/) covers picking other art.

:::caution
Universe takes the folder holding a Windows game's `.exe` as the game's install folder, and **Uninstall…** later moves that whole folder to the trash. Add a Windows game from a folder of its own, not from a shared one such as Downloads.
:::
