---
title: Game fixes
description: What to change when a Windows game misbehaves, in the order to try it.
sidebar:
  order: 6
---

When a Windows game misbehaves, read its log first ([Troubleshooting](/docs/running/troubleshooting/)). Then work down this list, in the **Proton** card of the game's **Game settings**, most rows behind **Y**:

1. [Check its umu id](/docs/running/game-fixes/#gameid-and-protonfixes).
2. [Try another Proton build](/docs/running/game-fixes/#another-proton-build).
3. [Turn the sync modes off](/docs/running/game-fixes/#sync-modes), one at a time.
4. [Turn Wayland off](/docs/running/game-fixes/#wayland), for a game on the desktop.
5. [Add the DLL overrides or runtimes](/docs/running/game-fixes/#dll-overrides-and-runtimes) the game needs.
6. [Turn off an upscaler upgrade](/docs/running/game-fixes/#upscaler-upgrades) you turned on.
7. [Reset the prefix](/docs/running/troubleshooting/#prefix-tools) last, since the next launch builds a fresh one.

When a PlayStation pad does nothing in a game, see [PlayStation controller](/docs/running/game-fixes/#playstation-controller).

## GAMEID and protonfixes

umu-run applies the fixes it knows for a game by the game's **umu id**, which Universe fills in from its store or its Steam app id. When the fixes are missing because the game got the wrong id, set **Desktop and library** › **Steam app id**, or the **umu id** itself.

## Another Proton build

Pick another **Proton build**. To get one you don't have, install it from Settings › Emulators & Proton › **Proton** › **Builds**, whose menu offers older releases under **Install another version…**.

## Sync modes

**Esync**, **Fsync** and **NTSync** are on by default, under **Sync**. A game that hangs or stutters may do better with one off. Turn them off one at a time, NTSync first.

## Wayland

**Wayland**, on by default, runs Proton's own Wayland driver. Universe leaves it out inside gamescope, unless the game's gamescope arguments include `--expose-wayland`, so it matters for a game on the desktop. Turn it off when such a game misbehaves.

## DLL overrides and runtimes

A game or a mod that ships its own DLL, such as `dxgi.dll`, may need a [DLL override](/docs/running/environment/#dll-overrides) with `n,b`. A runtime the game asks for, such as a Visual C++ redistributable, comes from **Winetricks**, on the game's **Data and saves** page.

## Upscaler upgrades

Under **Upscaling**, four rows swap a game's upscaler for a newer one, and each says whether it works on your GPU: **DLSS upgrade**, **FSR 4 upgrade**, **XeSS upgrade** and **OptiScaler**, which adds FSR 4 or XeSS to games that only offer DLSS. All four are off by default. A swapped upscaler can upset a game or its anti-cheat, so if a game broke after you turned one on, turn it off first.

## PlayStation controller

**PlayStation controller** sets how a PlayStation pad reaches a Windows game: **native**, **as a DualShock 4** or **as an Xbox pad**. **auto**, the default, goes by what the game's Steam page says it supports, and the row says which one the game gets. When a game made for Xbox pads doesn't see your pad, pick **as an Xbox pad**.
