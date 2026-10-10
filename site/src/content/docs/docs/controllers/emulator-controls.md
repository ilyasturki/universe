---
title: Emulator controls
description: The two modules that set emulators up for your pads, which to pick, player order, Nintendo button layouts, and what they do to your emulator files.
sidebar:
  order: 4
---

Two optional modules set emulators up for the pads you hold. **Emulator controls** writes each emulator's own controller settings at every launch. **Virtual pads** hands the emulator one virtual pad per player instead. Both are off until you turn one on, and both serve emulator games alone. A Proton, Wine or Linux game keeps your pads as they are.

## Turn one on

In Settings › **Features**, open **Emulator controls** or **Virtual pads** and turn on **Enabled**, or press **Y** on its row. From a terminal, [`universe module enable`](/docs/reference/cli/module/) does it:

```sh
universe module enable controls
universe module enable pads
```

Once a module is on, an emulator game's **Game settings** show a card named after it, with a switch for that game alone. It reads **Set up controllers** for Emulator controls ("Off keeps the controller settings made in the emulator."), and **Hand the pads to the emulator as virtual pads** for Virtual pads. The module's own page in [Features](/docs/settings/features/) holds its settings for every game, and `universe module set` changes them from a terminal.

## Which one to pick

| | Emulator controls | Virtual pads |
|---|---|---|
| What the emulator sees | Your pads, bound in its own settings files | One "Universe Pad" per player, a PlayStation-class pad with gyro, touchpad and rumble |
| A pad changed mid-game | The bindings were written at launch, for the pads held then | The emulator notices nothing |
| Player order | Player 1 is the pad you last pressed | Each pad takes the lowest free player |
| Nintendo button layout, Wii controller, Triggers and bumpers | Yes | No |
| Needs | SDL3 | SDL3, `systemd-run`, and `/dev/uhid` open to your session |

Only one serves a game. With both on, Virtual pads takes the game whenever it can, and Emulator controls leaves the emulator's files alone. When Virtual pads is off for that game, or can't open `/dev/uhid` ([System setup](/docs/controllers/supported/#system-setup)), Emulator controls takes over.

Pick Emulator controls for the layout settings below. Pick Virtual pads when you swap pads while you play, or want bindings you set once in the emulator to survive any change of pad. [Virtual pads](/docs/add-ons/virtual-pads/) has more on that module.

## Player order

Emulator controls makes player 1 the pad you last pressed a button on, the one you drive Universe with. The other pads follow in the order they connected.

Virtual pads gives each pad the lowest free player, up to 8. A pad that leaves keeps its player's virtual pad in place, reporting nothing pressed, for the next pad to take, so the emulator's bindings survive a change of pad.

The numbers on the battery badges are the pads' own, from their lights. Neither module follows them.

## Where A sits in Nintendo games

A Nintendo pad has A on the right and B at the bottom; an Xbox pad has A at the bottom. Two settings decide which button is A in Nintendo emulators.

The first is Emulator controls' **Nintendo button layout**, set per game, whose description begins "Where A sits in Nintendo games, GameCube and Wii included. B, X and Y follow it." It has three choices:

- **Each controller's own** (the default) leaves it to each pad.
- **A on the right, every controller**
- **A at the bottom, every controller**

The second is the **Nintendo games** row in Settings › **Controller**, set per model, which shows while one of that model's pads is connected:

- **As printed** (the default) goes by the pad's print, which puts A on the right on a Switch Pro, a Switch 2 Pro and an 8BitDo Pro 3 in D-input or Switch mode, and at the bottom on every other pad, a PlayStation pad's Cross counting as A.
- **A on the right**
- **A at the bottom**

At launch, a game set to A on the right or A at the bottom puts every pad that way. On Each controller's own, each pad takes its model's row, and on As printed, its print. The model is the one Universe detects. An 8BitDo Pro 3 in X-input mode reads as an Xbox pad, so the Xbox row applies to it, and its print counts as A at the bottom.

The layout reaches Eden, Ryujinx, Dolphin, Cemu, Azahar, melonDS, mGBA and snes9x, where the game page offers it. A real GameCube controller keeps its printed letters in Dolphin, and N64 games keep Mupen64Plus's own layout. Virtual pads ignores both settings, since the emulator maps its PlayStation-class pads itself.

The **Nintendo games** row shows whether Emulator controls is on or not. Only that module reads it, so with the module off the row changes nothing.

From a terminal:

```sh
universe module set controls layout=positional
universe module set controls layout=xbox --game <game>
universe config set controller.layout.8bitdo-pro-3 xbox
universe config set controller.layout.8bitdo-pro-3
```

The game setting is `layout`, one of `pad`, `positional` (A on the right) or `xbox` (A at the bottom), for every game or, with `--game`, for one. A model's row is `controller.layout.<family>` in `config.toml`, set with [`universe config`](/docs/reference/cli/config/), and no value goes back to As printed.

## Triggers and bumpers, and the Wii controller

**Triggers and bumpers** is per game, **As on the console** or **Swapped**, and its description reads "Swapped exchanges the two. GameCube L and R then sit on the bumpers, Z on the triggers." As on the console, L and R sit on the bumpers and ZL and ZR (or L2 and R2) on the triggers, except on the GameCube, whose L and R go on the triggers and Z on the bumpers. The row shows for Eden, Ryujinx, Dolphin, Cemu, Azahar, Mupen64Plus, RPCS3, PCSX2 and DuckStation. A console with only L and R (GBA, DS, SNES, PSP) gets them on both, so there is nothing to swap.

**Wii controller**, per Wii game, picks **Remote and Nunchuk** (the default), **Remote sideways** for 2D games like New Super Mario Bros. Wii, or **Classic Controller**.

The settings search finds Nintendo button layout and Triggers and bumpers under "swap".

## What Emulator controls writes

At each launch, Emulator controls writes these emulators' controller settings for the pads you hold:

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

Vita3K, shadPS4, DOSBox and MAME have no settings to write, so they only get a pad mapping that leaves out HOME, Capture and the back paddles. Xenia runs through Proton, whose own SDL reads the pads, and Emulator controls does nothing for it. With no pad held at launch, it writes nothing.

## Your emulator files

- A file is written only when something in it changes, and the rest of it stays as you left it.
- The first time Emulator controls changes a file, it keeps the original beside it as `<file>.before-universe`.
- A file kept as a link (to a dotfiles repository, say) stays a link, and its target gets the change.
- A file the emulator makes on its first start is never created. Start the emulator once by itself, and the next launch writes its controls. Mupen64Plus, snes9x and ScummVM are the exception, since their files are written whole, missing or not.
- A game's own input profile in the emulator keeps its bindings, and only follows the pad you hold.

To keep the emulator's own settings for one game, turn **Set up controllers** off in its Game settings.

## HOME in emulators

Neither module gives HOME to the emulator, so HOME opens Universe's menu over an emulator as over any game. When a game needs the console's own HOME or PS button, turn on **Share HOME with the emulator** in the module's settings. The emulator then gets HOME too, and Universe's menu still opens on it. Under Emulator controls, Capture and the back paddles stay out of the emulator either way, free for your [macros](/docs/controllers/macros/).

## When a pad stops answering

Over a game, press **HOME** and pick the dock's **Controllers** button. It lists the pads Universe reads, each with its connection and, for an 8BitDo or a Bluetooth pad whose battery BlueZ reads, its charge ("Bluetooth · 80%"), then **Find controllers**.

- **A** on a pad reconnects it. A Bluetooth pad is disconnected and connected again, and Universe waits up to 5 seconds for it. A pad on a cable can't be replugged from software, so pull it out and plug it back in.
- Then, whatever came of the pad, Virtual pads unplugs its virtual pads and plugs them back with the same identity, so the emulator takes them up afresh. Each pad that comes back gets its old player. A cable pad triggers this too.
- **Find controllers** closes and reopens every pad, tries again any input device it passed over, and has the menus list the pads again.

The notice reads "DualSense Wireless Controller is back", or says the pad "switched itself off: press its PS or HOME button", "is on a cable: pull it out and plug it back in" or "could not be reconnected". Find controllers answers with the count, as in "2 controllers found" or "No controller found".

The dock has no Controllers button while the game is still loading. From a terminal, [`universe controller reconnect`](/docs/reference/cli/controller/) does the same for every pad, or for one by the id `universe controller ls` prints first, such as `event30`.

The PS5 look has the same list as **Controllers** in its Control Center. In the Switch 2 look, the **Controllers** page lists each pad with **Reconnect**, then **Find Controllers**.

## Upgrading an older config.toml

Before **Each controller's own** existed, the layout setting offered A on the right or A at the bottom for every pad, and its default put A on the right everywhere. The first time this Universe opens a `config.toml` without `schema = 2`, it removes an all-games `layout = "xbox"` from `[modules.controls]`, writes `schema = 2`, and leaves everything else as written, comments included. It does this once.

For a config that said `xbox`, the result is each controller's own layout. Xbox, PlayStation, Steam Deck and generic pads keep A at the bottom, from their print. Nintendo-printed pads (Switch Pro, Switch 2 Pro, an 8BitDo Pro 3 in D-input or Switch mode) now get A on the right, unless their model's **Nintendo games** row says A at the bottom. To have A at the bottom on every pad again, pick **A at the bottom, every controller**. A choice made after the migration sticks.

The migration leaves alone a `layout = "positional"` (still A on the right everywhere), a game's own `layout` in its `game.toml`, and a `config.toml` Universe can't write, such as the one Home Manager makes from `programs.universe.settings`. There, `modules.controls.layout = "xbox"` still means A at the bottom on every pad; remove it to give each pad its own.

If you never set the layout, the old default put A on the right on every pad, and the new one puts A at the bottom on Xbox, PlayStation, Steam Deck and generic pads. **A on the right, every controller** brings the old behaviour back.
