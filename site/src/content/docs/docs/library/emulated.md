---
title: Emulated games
description: How Universe finds your ROMs in the emulators' own game folders, which emulators it reads, adding new ROMs later, and their artwork.
sidebar:
  order: 6
---

Universe doesn't ask where your ROMs are. It reads the game folders each emulator lists in its own settings, and adds what it finds there, each game with the emulator whose folder held it. That happens in the first run's **Emulator folders** row, and again each time you run [`universe rescan`](/docs/reference/cli/rescan/).

## The folders Universe reads

| Emulator | Read from |
|---|---|
| Eden, and Citron, Sudachi, Suyu, yuzu | `~/.config/<name>/qt-config.ini`, its game folders |
| Azahar (Lime3DS, Citra) | `~/.config/azahar-emu/qt-config.ini`, or under `lime3ds-emu` or `citra-emu` |
| Dolphin | `~/.config/dolphin-emu/Dolphin.ini`, its ISO paths |
| Ryujinx | `~/.config/Ryujinx/Config.json`, `game_dirs` |
| RPCS3 | `~/.config/rpcs3/games.yml`, and its `dev_hdd0/game` folder |
| PCSX2 | `~/.config/PCSX2/inis/PCSX2.ini`, `[GameList]` |
| DuckStation | `settings.ini` in `~/.local/share/duckstation` or `~/.config/duckstation`, `[GameList]` |
| Cemu | `~/.config/Cemu/settings.xml`, `GamePaths` |
| shadPS4 | `~/.config/shadPS4/config.toml`, `installDirs` |
| Flycast | `~/.config/flycast/emu.cfg`, `Dreamcast.ContentPath` |

`~/.config` and `~/.local/share` follow `$XDG_CONFIG_HOME` and `$XDG_DATA_HOME` when you set them. Universe goes into subfolders where the emulator's own setting does, such as Eden's deep scan or Dolphin's recursive paths.

Universe reads an emulator's folders only when it finds the emulator itself, on your PATH, installed by Universe or set as the runner's program. Flatpak emulators aren't looked for. melonDS and mGBA keep no list of folders, and Universe has no reader for PPSSPP, xemu, Xenia, Vita3K, Mupen64Plus, Snes9x, ScummVM, DOSBox or MAME. Their games go in [by hand](/docs/library/add-a-game/).

## What counts as a game

A game is a file with one of its emulator's extensions, as [Runners](/docs/reference/runners/) lists them. Three emulators take folders instead:

- RPCS3, a folder holding `PS3_GAME/USRDIR/EBOOT.BIN`, titled from its `PARAM.SFO`;
- shadPS4, a folder holding `eboot.bin`, whose `-UPDATE` and `-PATCH` folders are patches and not games;
- Cemu, besides its other files, the `.rpx` under `code/`, titled from `meta/meta.xml`.

Universe passes over:

- folders named `updates`, `dlc`, `mods`, `firmware`, `amiibo`, `backups`, `downloads`, `prefixes`, `saves`, `textures` or `shaders`, and hidden folders;
- a `.bin`, `.img` or `.raw` beside a `.cue`, `.gdi` or `.m3u`, which is one of its tracks;
- a file already in your library, even reached through another path;
- a file whose title another file or a library game already has. Of a plain name and a tagged one, the plain name wins.

A title is the file's name without its `[tags]`, its `(tags)`, its underscores and a version such as `v1.1`.

## Adding new ROMs

Universe Big Screen doesn't watch the folders, and has no rescan button. After you add ROMs, any of these brings them in:

- `universe rescan`, which also fetches their art;
- **Rescan Library** in Universe Desktop's main menu;
- the first run again, from Settings › About › **First-run setup** › **Run again**, then **Add** on its Emulator folders row.

[`universe discover`](/docs/reference/cli/discover/) shows how many are waiting, on its Emulator folders line.

## Artwork for emulated games

libretro's thumbnails give an emulated game its box front and its logo. They're matched by the ROM's file name and the game's platform, on these platforms: Game Boy, Game Boy Color and Game Boy Advance, SNES, Nintendo 64, DS, 3DS, GameCube, Wii, Wii U, PlayStation 1 to 4, PSP, Vita, Dreamcast, Xbox, Xbox 360, ScummVM, MS-DOS and arcade games. The Switch has none.

GOG GamesDB is searched by title, preferring a release on the game's platform. A game still titled after its file takes GamesDB's name when the match is sure. The rest works as for any game ([Artwork](/docs/library/artwork/)).
