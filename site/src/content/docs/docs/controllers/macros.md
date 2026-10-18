---
title: Macros and paddles
description: Bind volume, screenshots, MangoHud, key combos or commands to a pad's paddles and spare buttons.
sidebar:
  order: 3
---

A macro is an action Universe runs when you press or hold a button, such as a back paddle that turns the volume up. A button can carry one macro on press and another on hold. Universe ships a few for pads with spare buttons, and you bind your own in Settings › **Controller**.

## Actions

| Action | What it does |
|---|---|
| Volume up, Volume down | Changes the volume, and repeats while held. |
| Mute | Mutes or unmutes the sound. |
| Screenshot | Takes a screenshot, once the [Screenshots](/docs/add-ons/screenshots/) feature is on. |
| Toggle MangoHud | Shows or hides the running game's MangoHud until it ends. |
| Stop the game | Rumbles the pad, then quits the game. On hold only. |
| Key combo… | Types a key combination, such as `Super_L+F12`. |
| Command… | Runs a shell command. |

A game that reads the same button sees the press too. Key combos need the [system setup](/docs/controllers/supported/#system-setup) your install did.

## Shipped macros

All on a press:

| Controller | Button | Action |
|---|---|---|
| DualSense Edge | Left Fn | Screenshot |
| DualSense Edge | Right Fn | Toggle MangoHud |
| DualSense Edge | Left back button | Volume down |
| DualSense Edge | Right back button | Volume up |
| Xbox Elite Series 2 | P1 (right upper) | Toggle MangoHud |
| Xbox Elite Series 2 | P2 (right lower) | Volume up |
| Xbox Elite Series 2 | P3 (left upper) | Screenshot |
| Xbox Elite Series 2 | P4 (left lower) | Volume down |
| 8BitDo Pro 3 | R4 | Toggle MangoHud |
| 8BitDo Pro 3 | PR | Volume up |
| 8BitDo Pro 3 | PL | Volume down |
| Xbox | Share | Screenshot |
| Switch Pro, Switch 2 Pro | Capture | Screenshot |

The Steam Deck's back buttons ship empty, ready for yours.

## Bind a macro

1. Connect the pad and open Settings › Controller.
2. Press **A** on the button's row, then **On press…** or **On hold…**.
3. Pick the action. **Key combo…** and **Command…** open a text field.

The macro holds for every pad of that model, and **Clear press** or **Clear hold** removes it. HOME takes no macro. To bind for a model with no pad connected, use [Universe Desktop](/docs/using/universe-desktop/)'s Preferences › Controller.

## Press and hold

A button with only a press macro fires the moment you press it. With a hold macro too, a short press fires the press macro when you let go, and a long one fires the hold macro. **Hold length**, in Settings › Controller's **Advanced** rows, sets how long a hold is.

## When macros don't fire

- With the dock open over a game, only the volume, mute and MangoHud macros fire.
- While Settings › Controller has the focus, none fire.
- In [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode), the volume, mute, MangoHud and Screenshot macros do nothing.

## Get the shipped macros back

Your first change to a macro saves the whole set, the shipped ones included. To go back to the shipped set, run:

```sh
universe config set controller.macros
```
