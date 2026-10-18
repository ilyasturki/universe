---
title: Emulator controls
description: The two features that set emulators up for your pads, which to pick, and Nintendo button layouts.
sidebar:
  order: 4
---

Two optional features set emulators up for the pads you hold. **Emulator controls** writes each emulator's own controller settings at every launch. [Virtual pads](/docs/add-ons/virtual-pads/) hands the emulator one virtual pad per player instead. Both start off, and neither touches a Windows or Linux game.

## Turn one on

In Settings › **Features**, press **Y** on **Emulator controls** or **Virtual pads**. An emulated game's **Game settings** then show a card for it, with a switch for that game alone.

## Which one to pick

| | Emulator controls | Virtual pads |
|---|---|---|
| What the emulator sees | Your pads, bound in its own settings | One "Universe Pad" per player, with gyro, touchpad and rumble |
| A pad changed mid-game | Bound at launch, for the pads held then | The emulator notices nothing |
| Player 1 | The pad you last pressed a button on | Each pad takes the lowest free player |
| Nintendo button layout, Wii controller, triggers and bumpers | Yes | No |

With both on, Virtual pads serves a game whenever it can, and Emulator controls takes over when it can't.

## Where A sits in Nintendo games

Two settings decide which button is A in Nintendo emulators, and B, X and Y follow it:

- **Nintendo button layout**, in a game's Emulator controls card: **Each controller's own** (the default), **A on the right, every controller** or **A at the bottom, every controller**.
- **Nintendo games**, in Settings › **Controller**, for each model: **As printed** (the default), **A on the right** or **A at the bottom**. As printed puts A on the right on a Switch Pro, a Switch 2 Pro and an 8BitDo Pro 3, and at the bottom on every other pad.

On Each controller's own, each pad follows its model's row. An 8BitDo Pro 3 in X-input mode counts as an Xbox pad.

## Triggers and bumpers, and the Wii controller

**Triggers and bumpers**, per game, is **As on the console** or **Swapped**.

**Wii controller**, per Wii game, picks **Remote and Nunchuk** (the default), **Remote sideways** for 2D games, or **Classic Controller**.

## What Emulator controls writes

It writes the controller settings of Eden and its forks, Ryujinx, Cemu, RPCS3, Dolphin, PCSX2, DuckStation, Azahar, melonDS, mGBA, snes9x, Mupen64Plus, PPSSPP, xemu, Flycast and ScummVM.

- The first time it changes a file, it keeps the original beside it as `<file>.before-universe`.
- It doesn't create most emulators' files. Start the emulator once by itself, and the next launch writes its controls.

To keep the emulator's own settings for one game, turn **Set up controllers** off in its Game settings.

## HOME in emulators

Neither feature gives HOME to the emulator. When a game needs the console's own HOME or PS button, turn on **Share HOME with the emulator** in the feature's settings. Universe's menu still opens on it.
