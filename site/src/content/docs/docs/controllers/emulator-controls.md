---
title: Emulator controls
description: The two modules that set emulators up for your pads, which to pick, player order, Nintendo button layouts, and what they do to your emulator files.
sidebar:
  order: 4
---

Two optional modules set emulators up for the pads you hold. **Emulator controls** writes each emulator's own controller settings at every launch. **Virtual pads** hands the emulator one virtual pad per player instead. Both are off until you turn one on. Neither touches a Proton, Wine or Linux game.

## Turn one on

In Settings › **Features**, open **Emulator controls** or **Virtual pads** and turn on **Enabled**, or press **Y** on its row. From a terminal, use [`universe module enable`](/docs/reference/cli/module/):

```sh
universe module enable controls
universe module enable pads
```

An emulator game's **Game settings** then show a card for the module, with a switch for that game alone: **Set up controllers** for Emulator controls, **Hand the pads to the emulator as virtual pads** for Virtual pads. The module's page in [Features](/docs/settings/features/) holds its settings for every game, and `universe module set` changes them from a terminal.

## Which one to pick

| | Emulator controls | Virtual pads |
|---|---|---|
| What the emulator sees | Your pads, bound in its own settings files | One "Universe Pad" per player, a PlayStation-class pad with gyro, touchpad and rumble |
| A pad changed mid-game | The bindings were written at launch, for the pads held then | The emulator notices nothing |
| Player order | Player 1 is the pad you last pressed | Each pad takes the lowest free player |
| Nintendo button layout, Wii controller, Triggers and bumpers | Yes | No |
| Needs | SDL3 | SDL3, `systemd-run`, and `/dev/uhid` open to your session |

Only one serves a game. With both on, Virtual pads takes the game whenever it can, and Emulator controls leaves the emulator's files alone. When Virtual pads is off for that game, or can't open `/dev/uhid` ([System setup](/docs/controllers/supported/#system-setup)), Emulator controls takes over.

Pick Emulator controls for the layout settings below, and [Virtual pads](/docs/add-ons/virtual-pads/) when you swap pads while you play, or want bindings you set once in the emulator to survive any change of pad.

## Player order

Under Emulator controls, player 1 is the pad you last pressed a button on, and the other pads follow in the order they connected.

Under Virtual pads, each pad takes the lowest free player, up to 8. When a pad leaves, its player's virtual pad stays plugged in, pressing nothing, for the next pad to take.

## Where A sits in Nintendo games

Two settings decide which button is A in Nintendo emulators. B, X and Y follow it.

Emulator controls' **Nintendo button layout**, set per game, covers GameCube and Wii games too:

- **Each controller's own** (the default) leaves it to each pad.
- **A on the right, every controller**
- **A at the bottom, every controller**

The **Nintendo games** row in Settings › **Controller**, set per model, shows while one of that model's pads is connected:

- **As printed** (the default) follows the pad's print: A on the right on a Switch Pro, a Switch 2 Pro and an 8BitDo Pro 3 in D-input or Switch mode, at the bottom on every other pad, a PlayStation pad's Cross counting as A.
- **A on the right**
- **A at the bottom**

A game set to A on the right or A at the bottom puts every pad that way. On Each controller's own, each pad follows its model's row. The model is the one Universe detects, so an 8BitDo Pro 3 in X-input mode follows the Xbox row, and its print counts as A at the bottom.

The layout reaches Eden, Ryujinx, Dolphin, Cemu, Azahar, melonDS, mGBA and snes9x, whose game pages offer it. A real GameCube controller keeps its printed letters in Dolphin, and N64 games keep Mupen64Plus's own layout. Virtual pads ignores both settings.

From a terminal:

```sh
universe module set controls layout=positional
universe module set controls layout=xbox --game <game>
universe config set controller.layout.8bitdo-pro-3 xbox
universe config set controller.layout.8bitdo-pro-3
```

The game setting is `layout`, one of `pad`, `positional` (A on the right) or `xbox` (A at the bottom), for every game or, with `--game`, for one. A model's row is `controller.layout.<family>` in `config.toml`, set with [`universe config`](/docs/reference/cli/config/), and no value goes back to As printed.

## Triggers and bumpers, and the Wii controller

**Triggers and bumpers**, per game, is **As on the console** or **Swapped**. As on the console puts L and R on the bumpers and ZL and ZR (or L2 and R2) on the triggers, except on the GameCube, whose L and R go on the triggers and Z on the bumpers. Swapped exchanges the two. The row shows for Eden, Ryujinx, Dolphin, Cemu, Azahar, Mupen64Plus, RPCS3, PCSX2 and DuckStation. A console with only L and R (GBA, DS, SNES, PSP) gets them on both.

**Wii controller**, per Wii game, picks **Remote and Nunchuk** (the default), **Remote sideways** for 2D games like New Super Mario Bros. Wii, or **Classic Controller**.

The settings search finds Nintendo button layout and Triggers and bumpers under "swap".

## What Emulator controls writes

At each launch, it writes these emulators' controller settings:

| Emulator | Players |
|---|---|
| Eden, and its forks citron, sudachi, suyu and yuzu | 8 |
| Ryujinx | 8 |
| Cemu | 8 |
| RPCS3 | 7 |
| snes9x | 5, with the multitap from 3 pads |
| Dolphin | 4 GameCube ports and 4 Wii remotes |
| mGBA, Mupen64Plus, xemu, Flycast | 4 |
| PCSX2, DuckStation | 2, more with your own multitap setting |
| Azahar, melonDS, PPSSPP | Player 1 |
| ScummVM | The first pad |

Vita3K, shadPS4, DOSBox and MAME only get a pad mapping that leaves out HOME, Capture and the back paddles. Xenia runs through Proton and gets nothing. With no pad held at launch, nothing is written.

## Your emulator files

- A file is written only when something in it changes, and the rest of it stays as you left it.
- The first time Emulator controls changes a file, it keeps the original beside it as `<file>.before-universe`.
- A file kept as a link (to a dotfiles repository, say) stays a link, and its target gets the change.
- A file the emulator makes on its first start is never created. Start the emulator once by itself, and the next launch writes its controls. Mupen64Plus, snes9x and ScummVM files are written whole, missing or not.
- A game's own input profile in the emulator keeps its bindings, and only follows the pad you hold.

To keep the emulator's own settings for one game, turn **Set up controllers** off in its Game settings.

## HOME in emulators

Neither module gives HOME to the emulator. When a game needs the console's own HOME or PS button, turn on **Share HOME with the emulator** in the module's settings. The emulator then gets HOME too, and Universe's menu still opens on it. Under Emulator controls, Capture and the back paddles stay out of the emulator either way, free for your [macros](/docs/controllers/macros/).

## When a pad stops answering

Over a game, press **HOME** and pick the dock's **Controllers** button ([Playing](/docs/using/playing/#controllers)). It is missing while the game is still loading.

- **A** on a pad reconnects it. A Bluetooth pad is disconnected and connected again, and Universe waits up to 5 seconds for it; one that switched itself off wakes with its PS or HOME button. Pull a pad on a cable out and plug it back in.
- Either way, Virtual pads then unplugs its virtual pads and plugs them back with the same identity, and each pad that comes back gets its old player.
- **Find controllers** closes and reopens every pad, and tries again any input device it passed over.

From a terminal, [`universe controller reconnect`](/docs/reference/cli/controller/) does the same for every pad, or for one by the id `universe controller ls` prints first. The PS5 look has the same list as **Controllers** in its Control Center. In the Switch 2 look, the **Controllers** page lists each pad with **Reconnect**, then **Find Controllers**.

## Upgrading an older config.toml

The first time Universe opens a `config.toml` without `schema = 2`, it removes an all-games `layout = "xbox"` from `[modules.controls]` and writes `schema = 2`, leaving everything else as written, comments included. It does this once.

A config that said `xbox` then gets each controller's own layout, so Nintendo-printed pads (Switch Pro, Switch 2 Pro, an 8BitDo Pro 3 in D-input or Switch mode) move A to the right, unless their model's **Nintendo games** row says A at the bottom. To have A at the bottom on every pad again, pick **A at the bottom, every controller**. A choice made after the migration sticks.

The migration leaves alone a `layout = "positional"`, a game's own `layout` in its `game.toml`, and a `config.toml` Universe can't write, such as the one Home Manager makes from `programs.universe.settings`. There, `modules.controls.layout = "xbox"` still means A at the bottom on every pad; remove it to give each pad its own.

If you never set the layout, the old default put A on the right on every pad, and the new one puts A at the bottom on Xbox, PlayStation, Steam Deck and generic pads. **A on the right, every controller** brings the old behaviour back.
