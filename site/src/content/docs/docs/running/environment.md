---
title: Environment variables and wrappers
description: Environment variables, a wrapper command, arguments and commands around a game's launch.
sidebar:
  order: 4
---

These rows change how a game launches. They're advanced: press **Y** on Settings › **Game defaults** or on a game's **Game settings** to show them. Only environment variables also have a default for every game; the other rows belong to one game.

## Environment variables

The variables every game gets are in Settings › Game defaults › **Environment**. **Add a variable…** asks for a **Variable** and a **Value**, and **X** (**Remove**) deletes one.

A game's own are under **Environment** in the **Launch** card of its Game settings. To change a default for one game, add a variable of the same name there: the game's wins.

## Wrapper command

**Wrapper command**, in the game's **Launch** card, is a command the game runs through, such as `gamemoderun`.

## Arguments and working directory

**Arguments** are added to the game's command line, split as a shell splits words. **Working directory** is the folder the game starts in, by default the one holding its program.

## Before and after the game

**Before the game** is a shell command run before the game starts, and **After the game** one run after it ends. A command before the game that runs past two minutes fails the launch.

## DLL overrides

**DLL overrides**, in the **Proton** or **Wine** card of a game's settings, makes Wine load the copy of a DLL that a game or a mod ships, instead of its own. **Add an override…** asks for the **DLL**, its name without `.dll`, and the **Override**, such as `n,b` (the game's copy first, then Wine's).

## Check what a launch got

A game's log starts with the command its last session ran ([A game's logs](/docs/running/troubleshooting/#a-games-logs)). For every variable it started with, turn on the [Debug log](/docs/running/troubleshooting/#debug-log).
