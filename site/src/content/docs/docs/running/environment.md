---
title: Environment variables and wrappers
description: Environment variables, a wrapper command, arguments and shell lines around a game's launch, and how to check what a launch got.
sidebar:
  order: 4
---

Every row on this page is an advanced one, shown once you press **Y** (Show advanced) on Settings › **Game defaults** or on a game's **Game settings**. Only environment variables have a default for every game; the rest belong to one game.

## Environment variables

The variables every game gets are in Settings › Game defaults › **Environment**, one row per variable. **Add a variable…** opens a sheet with **Variable** and **Value**, and **X** (**Remove**) deletes one.

One game's are in the **Launch** card of its Game settings, under **Environment**. The defaults show there too, tagged **Global**, and only Game defaults removes them. To change one for this game, add a variable of the same name, and the game's wins. When you add a variable from a game, Universe asks whether it's for **This game** or **All games**. A name keeps only letters, digits, `_` and `-`.

From a terminal, an empty value deletes the variable:

```sh
universe set <game> env.DXVK_HUD=fps
universe set <game> env.DXVK_HUD=
universe config set launch.env.DXVK_HUD fps
```

### Which value wins

A launch builds the game's environment in this order, each step over the one before:

1. the variables a feature's pre-launch hook adds;
2. the defaults from Game defaults;
3. the runner's own variables: for Proton `WINEPREFIX`, `PROTONPATH`, `GAMEID`, `STORE`, the `PROTON_*` variables of its switches and `WINEDLLOVERRIDES`; for Wine `WINEPREFIX`, `WINEARCH`, `WINEESYNC`, `WINEFSYNC` and `WINEDLLOVERRIDES`;
4. the game's own variables, which win over all of the above.

Some variables don't follow these steps. The **Wayland** and **HDR** switches decide over a `PROTON_ENABLE_WAYLAND` or `PROTON_ENABLE_HDR` set in the defaults. `PROTON_ENABLE_WAYLAND` is dropped on the fullscreen app's gamescope, and inside a game's own gamescope unless its arguments include `--expose-wayland`. A `PROTON_SONY_*` variable you set takes over from the **PlayStation controller** setting for that game. The variables for **Discrete GPU**, the debug log and Proton's crash reports are only added where you haven't set them.

## Wrapper command

**Wrapper command**, in the game's **Launch** card, is a command the game runs through, such as `gamemoderun` or `taskset -c 0-7`. Universe splits it like a shell line and puts it in front of the game's program as the innermost wrapper, inside gamescope and MangoHud. For a Proton game that program is umu-run, so the launch becomes `gamemoderun umu-run <the game's .exe>`. There is no wrapper for every game.

```sh
universe set <game> wrapper="gamemoderun"
```

## Arguments and working directory

**Arguments** are appended to the game's command line. They're split as a shell splits words, so `-windowed -nolauncher` is two arguments and `--name 'My Save'` keeps its space inside one. For an emulator they come last, after the runner page's **Arguments**, the flags of its options, the game's file and any a feature's hook adds.

**Working directory** is the folder the game starts in. It defaults to the folder holding the game's program, since some games quit when started from anywhere else.

```sh
universe set <game> args="-windowed -nolauncher" working_dir=~/Games/MyGame
```

## Before and after the game

**Before the game** is a shell line run before the game starts, and **After the game** one run once it has ended. Both run through `sh -c`, with the game's environment, in its working directory, and get 120 seconds.

Universe ignores what the line before returns, so the game starts either way, but a line before that runs past 120 seconds fails the launch. The line after also runs when a launch fails once the line before has run.

```sh
universe set <game> pre_command="systemctl --user stop syncthing" post_command="systemctl --user start syncthing"
```

## DLL overrides

**DLL overrides**, in the **Proton** or **Wine** card of a game's settings, tells Wine to load a DLL's native copy, the one a game or a mod ships, instead of its own builtin. **Add an override…** asks for the **DLL**, its name without `.dll`, and the **Override**, such as `n,b` (native first, then Wine's builtin). Universe joins them into `WINEDLLOVERRIDES`, as in `d3d11=n,b;dxgi=n`.

```sh
universe set <game> dll_overrides.dxgi=n,b
```

## Check what a launch got

[`universe logs <game>`](/docs/reference/cli/logs/) starts with the command the last session ran. [`universe info <game>`](/docs/reference/cli/info/) shows the runner and its program, the prefix and Proton build, the switches, the wrapper, gamescope, the frame rate limit and the variables from Game defaults and the game merged.

For the whole environment, turn on **Debug log** for the game, in its **Proton** card (its **Logs** card for a Wine game) or with `universe set <game> debug_log=true`, which also works for a Linux game or an emulator. Each launch then writes `launch.txt` in `~/.local/state/universe/logs/<id>/<session>/`, holding the full command line, a blank line, then every variable the game started with. Proton's and DXVK's logs land in the same folder and grow large, so turn it off once you're done ([Troubleshooting](/docs/running/troubleshooting/#debug-log)).
