---
title: Environment variables and wrappers
description: Environment variables, a wrapper command, arguments and shell lines around a game's launch, and how to check what a launch got.
sidebar:
  order: 4
---

Every row on this page is advanced, shown once you press **Y** (Show advanced) on Settings › **Game defaults** or on a game's **Game settings**. Only environment variables also have a default for every game; the other rows belong to one game.

## Environment variables

The variables every game gets are in Settings › Game defaults › **Environment**. **Add a variable…** asks for a **Variable** and a **Value**, and **X** (**Remove**) deletes one.

A game's own are under **Environment** in the **Launch** card of its Game settings. The defaults show there too, tagged **Global**, and only Game defaults removes them. To change one for this game, add a variable of the same name; the game's wins. A name keeps only letters, digits, `_` and `-`.

From a terminal, an empty value deletes the variable:

```sh
universe set <game> env.DXVK_HUD=fps
universe set <game> env.DXVK_HUD=
universe config set launch.env.DXVK_HUD fps
```

### Which value wins

Each step overrides the one before:

1. the variables a feature's pre-launch hook adds;
2. the defaults from Game defaults;
3. the runner's own variables: for Proton `WINEPREFIX`, `PROTONPATH`, `GAMEID`, `STORE`, the `PROTON_*` variables of its switches and `WINEDLLOVERRIDES`; for Wine `WINEPREFIX`, `WINEARCH`, `WINEESYNC`, `WINEFSYNC` and `WINEDLLOVERRIDES`;
4. the game's own variables.

The exceptions: the **Wayland** and **HDR** switches override a `PROTON_ENABLE_WAYLAND` or `PROTON_ENABLE_HDR` set in the defaults, and gamescope can drop `PROTON_ENABLE_WAYLAND` ([Wayland](/docs/running/game-fixes/#wayland)). A `PROTON_SONY_*` variable you set overrides the **PlayStation controller** setting for that game. The variables for **Discrete GPU**, the debug log and Proton's crash reports are added only where you haven't set them.

## Wrapper command

**Wrapper command**, in the game's **Launch** card, is a command the game runs through, such as `gamemoderun` or `taskset -c 0-7`. Universe splits it like a shell line and makes it the innermost wrapper, inside gamescope and MangoHud, so a Proton game launches as `gamemoderun umu-run <the game's .exe>`.

```sh
universe set <game> wrapper="gamemoderun"
```

## Arguments and working directory

**Arguments** are appended to the game's command line, split as a shell splits words: `-windowed -nolauncher` is two arguments, `--name 'My Save'` is one. For an emulator they come last, after the runner page's **Arguments**, the flags of its options, the game's file and any a feature's hook adds.

**Working directory** is the folder the game starts in, by default the one holding the game's program.

```sh
universe set <game> args="-windowed -nolauncher" working_dir=~/Games/MyGame
```

## Before and after the game

**Before the game** is a shell line run before the game starts, **After the game** one run after it ends. Both run through `sh -c` with the game's environment, in its working directory, and get 120 seconds. The game starts whatever the line before returns, but a line before that runs past 120 seconds fails the launch. The line after also runs when a launch fails after the line before ran.

```sh
universe set <game> pre_command="systemctl --user stop syncthing" post_command="systemctl --user start syncthing"
```

## DLL overrides

**DLL overrides**, in the **Proton** or **Wine** card of a game's settings, makes Wine load a DLL's native copy, the one a game or a mod ships, instead of its builtin. **Add an override…** asks for the **DLL**, its name without `.dll`, and the **Override**, such as `n,b` (native first, then Wine's builtin). Universe joins them into `WINEDLLOVERRIDES`, as in `d3d11=n,b;dxgi=n`.

```sh
universe set <game> dll_overrides.dxgi=n,b
```

## Check what a launch got

[`universe logs <game>`](/docs/reference/cli/logs/) starts with the command the last session ran. [`universe info <game>`](/docs/reference/cli/info/) shows what a launch uses, with the variables from Game defaults and the game merged.

For every variable the game started with, turn on the [Debug log](/docs/running/troubleshooting/#debug-log). Each launch then writes `launch.txt` in the session's log folder, the full command line then the environment. `universe set <game> debug_log=true` turns it on for a Linux game or an emulator too.
