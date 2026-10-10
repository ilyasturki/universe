---
title: Performance
description: "Settings › Performance sets the brightness, a Steam Deck's refresh rate, the power limit, the GPU clock and SteamOS's fan curve."
sidebar:
  order: 12
---

Settings › **Performance** sets the machine's own controls, such as the screen's brightness and the APU's power limit. It shows only when the machine has at least one control Universe can set, and never under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode).

## Performance and display

One card, whose line reads **Steam Deck** or **Steam Deck OLED** on a Deck. Each row is there only when the machine has the control:

| Row | What it sets | Values | When it shows |
| --- | --- | --- | --- |
| Brightness | The screen's backlight | 5 to 100 %, in steps of 5 | Any machine with a backlight, laptops included |
| Refresh rate | The panel's refresh; lower saves battery | the panel's range, in Hz | A Steam Deck, when the launcher runs its own gamescope on the panel |
| Power limit | The most the APU may draw, sustained | watts, in the APU's range | An AMD GPU whose power cap Universe can write |
| GPU clock | Auto lets the driver choose; a fixed clock trades power for steady frame times | Auto, or steps of 100 MHz | An AMD GPU whose overdrive Universe can write |
| SteamOS fan curve | Off leaves the fan to the firmware | on or off | SteamOS |

**A** toggles the fan curve and opens a picker on the others. The change applies at once, and a refusal comes back as a notice.

## Keys

**X** (**Refresh**) reads the controls again.

## Where it's kept

The system keeps the brightness itself. The other four go to `[system]` in config.toml (`refresh`, `tdp`, `gpu`, `fan`), and the launcher sets them again each time it starts outside Game Mode, since the power limit and the clocks don't outlive a reboot. If the running game has its own value for a control, a change here waits for the game's session to end.

Over a game, the dock's **System** button holds the same controls, and a change made there belongs to that game. [Playing](/docs/using/playing/) covers the dock.

## In the other looks

The Switch 2 look has System Settings › **Performance**, and the PS5 look Settings › **Performance**, both refreshed with **Y**. Universe Desktop shows Preferences › **Performance** on any Steam Deck, without the brightness and the refresh rate.
