---
title: Game fixes
description: What to change when a Windows game misbehaves, in the order to try it, from its umu id and Proton build to sync, Wayland, DLL overrides and upscalers.
sidebar:
  order: 6
---

The fixes for a Windows game are in the **Proton** card of its **Game settings**, most of them under Show advanced (**Y**). All but **umu id**, **Store** and **DLL overrides** also have a default for every Proton game, on Settings › **Emulators & Proton** › **Proton**. Read the game's log first ([Troubleshooting](/docs/running/troubleshooting/)), then work down this list, which starts with the changes that touch the least:

1. [Check its umu id](/docs/running/game-fixes/#gameid-and-protonfixes), so umu-run applies the fixes it knows for the game.
2. [Try another Proton build](/docs/running/game-fixes/#another-proton-build).
3. [Turn the sync modes off](/docs/running/game-fixes/#sync-modes), one at a time.
4. [Turn Wayland off](/docs/running/game-fixes/#wayland), for a game on the desktop.
5. [Add the DLL overrides or runtimes](/docs/running/game-fixes/#dll-overrides-and-runtimes) the game needs.
6. [Turn off an upscaler upgrade](/docs/running/game-fixes/#upscaler-upgrades) you turned on.
7. [Reset the prefix](/docs/running/troubleshooting/#prefix-tools) last, since the next launch builds a fresh one.

When a PlayStation pad does nothing in a game, see [PlayStation controller](/docs/running/game-fixes/#playstation-controller).

## GAMEID and protonfixes

umu-run finds the fixes it has for a game (protonfixes, and GE-Proton's switches for that game) by `GAMEID` and `STORE`. Universe sets `GAMEID` from the game's **umu id**. Left empty, it's `umu-` followed by the game's Steam app id when it has one, else `umu-default`. `universe info <game> --json` shows what it comes to, as `effective.umu_id`.

The stores fill **umu id** and **Store** only while they're empty, so a value you set stays. GOG and Epic games ask the umu database for theirs, and ask again a week later when it doesn't know the game yet. Steam games get `umu-` and their app id, itch.io games only their store. `STORE` is `gog`, `egs`, `itchio` or `steam`.

Universe looks up a game's Steam app id while fetching its artwork. When it got it wrong or found none, set **Desktop and library** › **Steam app id** (Show advanced) or pin it from a terminal. You can also set both variables yourself:

```sh
universe media <game> pin steam <app id>
universe set <game> umu_id=umu-<app id> store=gog
```

## Another Proton build

Pick another **Proton build** in the game's Proton card; [Proton builds](/docs/running/runners/#proton-builds) explains how a name finds its build. To get one you don't have, install it from the GE-Proton, Proton-CachyOS, Proton-EM or UMU-Proton row of Settings › Emulators & Proton › **Proton** › **Builds**, whose menu offers older releases under **Install another version…**.

```sh
universe component install ge-proton GE-Proton9-27
universe set <game> proton=GE-Proton9-27
```

## Sync modes

**Esync**, **Fsync** and, for Proton, **NTSync** are on by default, under **Sync** with Show advanced. A game that hangs or stutters may do better with one off. Turn them off one at a time, NTSync first. NTSync needs Linux 6.14 and a Proton built for it, Fsync needs the kernel's futex2, and each falls back when it can't run.

## Wayland

**Wayland**, on by default, runs Proton's own Wayland driver instead of Xwayland. Universe drops it on the fullscreen app's gamescope, and in a game's own gamescope unless its arguments include `--expose-wayland`, so it matters mostly for a game on the desktop. Turn it off when such a game misbehaves.

## DLL overrides and runtimes

A game or a mod that ships its own DLL, such as `dxgi.dll` or `dinput8.dll`, may need Wine to load that copy. Add it under [DLL overrides](/docs/running/environment/#dll-overrides) in the Proton card, with `n,b`.

A runtime the game asks for, such as a Visual C++ redistributable, comes from **Winetricks**, on the game's **Data and saves** page under **Wine Prefix**:

```sh
universe data <game> winetricks vcrun2022
```

## Upscaler upgrades

Under **Upscaling**, with Show advanced, four rows swap a game's upscaler for a newer one, and each says whether it works on your GPU.

- **DLSS upgrade** gives games with DLSS the newest DLSS from the NVIDIA driver, on GeForce RTX only.
- **FSR 4 upgrade** moves games with FSR 3.1 to FSR 4, on a Radeon RX 9000, or on an RX 7000 at a cost.
- **XeSS upgrade** gives games with XeSS the newest XeSS, on any GPU and best on Intel Arc.
- **OptiScaler** adds FSR 4 or XeSS to games that only offer DLSS, on AMD and Intel. It needs a Proton that ships it, CachyOS or GE.

All four are off by default. The first three also take `auto`, which turns DLSS on with an NVIDIA GPU, FSR 4 on a Radeon RX 9000 and XeSS on an Intel GPU. A swapped DLL can upset a game or its anti-cheat, so if a game broke after you turned one on, turn that one off first.

## PlayStation controller

**PlayStation controller**, in the Proton card, sets how GE-Proton 11-4 and later hands a PlayStation pad to a Windows game:

- **native** passes the pad as it is, buttons and haptics kept, but a game made for Xbox pads doesn't see it.
- **as a DualShock 4** makes a DualSense look like a DualShock 4.
- **as an Xbox pad** works in any game that takes an Xbox pad.
- **auto**, the default, goes by the game's Steam page: native when it lists DualSense support, a DualShock 4 when it lists DualShock support but not DualSense, and an Xbox pad when it lists no PlayStation support or Universe knows no Steam page for the game. The row says which one the game gets, and why.

A `PROTON_SONY_*` variable you set takes over, and the row says so. Universe knows a game's Steam page once it has a Steam app id ([above](/docs/running/game-fixes/#gameid-and-protonfixes)).
