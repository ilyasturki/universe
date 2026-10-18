---
title: Gamescope, HDR and frame rate
description: The display settings a game runs with, HDR, MangoHud and the frame rate limit.
sidebar:
  order: 3
---

Games run inside gamescope by default. It gives a game the resolution and refresh rate you pick, scales the picture to your screen and draws MangoHud over it. Without gamescope installed, a game opens on the desktop.

## Which settings reach a game

Universe Big Screen, full screen or as [the Universe session](/docs/getting-started/universe-session/), runs every game in its own gamescope. That one takes the defaults alone: Settings › **Game defaults** › **Display**, and **HDR** from Settings › **Emulators & Proton** › **Proton**. A change to them applies the next time Universe starts, and a game's own display settings don't reach it.

A game started from Universe Desktop, a terminal or Big Screen in a window gets a gamescope of its own, built from the game's own settings.

## Display

The **Display** card of Game defaults or of a game's settings has:

- **Resolution**, what the game renders at before it's scaled to your screen;
- **Refresh rate**, the rate the game sees;
- **Adaptive sync**, for FreeSync and G-Sync, on by itself when the screen supports it.

**Y** adds the scaling rows. **Filter** picks how the picture is scaled: `fsr` and `nis` sharpen it, and `nearest` and `pixel` keep pixel art crisp. **Discrete GPU**, on by default, runs games on a laptop's stronger GPU.

## HDR

**HDR**, off by default, sends HDR to an HDR screen in the games that support it. Turn it on in Settings › Emulators & Proton › **Proton**, or for one game in the **Proton** card of its settings.

## MangoHud

**Show MangoHud**, in the **Overlay** card and off by default, draws the frame rate and the GPU and CPU load over the game. Your own MangoHud.conf still sets its layout. During a game, the dock's **Performance** card shows or hides it ([Playing](/docs/using/playing/#mangohud-the-frame-rate-limit-and-scaling)).

## Frame rate limit

**Frame rate limit**, in the **Overlay** card, holds the game to that many frames per second. `auto`, the default, is the refresh rate the game sees, and leaves emulators alone. `none` turns it off.

The limit needs MangoHud. Without it, System check's **Frame rate limit (MangoHud)** fails: install MangoHud, or set the limit to `none` on Game defaults.
