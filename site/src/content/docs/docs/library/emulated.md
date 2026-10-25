---
title: Emulated games
description: How Universe finds your ROMs through your emulators, and adds new ones later.
sidebar:
  order: 6
---

Universe doesn't ask where your ROMs are. It reads the game folders each emulator lists in its own settings, and adds what it finds there, each game with the emulator whose folder held it. The first run's **Emulator folders** row does it.

## The emulators it reads

Eden (and Citron, Sudachi, Suyu and yuzu), Azahar (and Lime3DS and Citra), Dolphin, Ryujinx, RPCS3, PCSX2, DuckStation, Cemu, shadPS4, KytyPS5 and Flycast.

Universe reads an emulator's folders only when it finds the emulator, on your PATH, installed by Universe or set as its program; it doesn't look for Flatpak emulators. The games of other emulators, such as melonDS, mGBA or PPSSPP, go in [by hand](/docs/library/add-a-game/).

A game is a file with one of its emulator's extensions ([Runners](/docs/reference/runners/)), or a folder for RPCS3, shadPS4 and KytyPS5. A PlayStation 4 folder counts only with its `sce_sys/param.sfo` and a PlayStation 5 one with its `sce_sys/param.json`, so one folder can hold games of both. Universe passes over update, DLC and firmware folders, and a file whose game is already in the library.

## Adding new ROMs

Universe doesn't watch the folders. After you add ROMs, any of these brings them in:

- `universe rescan` in a terminal;
- **Rescan Library** in Universe Desktop's main menu;
- the first run again, from Settings › About › **First-run setup** › **Run again**, then **Add** on its Emulator folders row.

## Artwork for emulated games

libretro's thumbnails give an emulated game its box front and its logo, matched by the ROM's file name. Most consoles have them; the Switch doesn't. GOG GamesDB fills in the rest by title ([Artwork](/docs/library/artwork/)).
