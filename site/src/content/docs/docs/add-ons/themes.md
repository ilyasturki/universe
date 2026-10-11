---
title: Themes
description: Install a theme add-on, pick it in Settings › Appearance, what an installed theme can't do, and how Universe falls back to its own look.
sidebar:
  order: 7
---

A theme is a look for Universe Big Screen that someone else made, installed as an add-on. It's listed after the three built-in looks (Universe, Switch 2 and PS5, see [Looks](/docs/using/looks/)) and picked the same way.

## Installing a theme

Settings › **Appearance** › **Get more…** lists the themes in Universe's index ([Installing add-ons](/docs/add-ons/installing/)). From a terminal, [`universe extension install`](/docs/reference/cli/extension/) takes an id from the index, an https:// URL, an archive or a folder. Either way, the theme lands in `~/.local/share/universe/extensions/theme/<id>/`, and Universe stays on the look you have.

## Picking it

In the Universe look, Settings › Appearance has a **Theme** row in its **Look** card. **A** opens the **Theme** menu, which lists the built-in looks first, then the installed themes, each with its screenshot and **Add-on** under its name. The switch happens at once, and the new look opens on its own Appearance page, where you can pick another.

In the Switch 2 look (System Settings › **Appearance**) and the PS5 look (Settings › **Appearance**), each look is a row of its own, with a theme's screenshot in place of the colour swatch.

A theme written for an extension API this Universe doesn't read, or missing its `theme.qml`, is listed but can't be picked.

Universe starts on the look you picked last, kept as `theme` in `~/.local/state/universe/ui-memory.json`. `universe-ui --theme <id>` starts on another for one run.

## What an installed theme can't do

An installed theme draws in Universe's main window and nowhere else:

- It has no dock or Control Center over a running game. Holding **HOME** brings the launcher back instead.
- The volume level a pad's macro shows over the game is Universe's own pill.
- An achievement unlocked during play shows Universe's banner.

## Back to the Universe look

When a theme stops working, Universe puts its own look back and says why. That happens when the theme's QML doesn't load, when the theme in use is removed or stops being usable (checked whenever you open Appearance, and after an add-on change made from Settings), and at start when the theme you picked last can't be used, which Universe then forgets.

To leave a theme that loads but leaves you no way out, hold **HOME** for 5 seconds. Universe reads that hold itself, whatever the theme's QML does, puts its own look back and starts on it next time. Under a built-in look, the hold doesn't change the look.

## Working on a theme

A theme has no folder of your own under `~/.config/universe`. Install it from the folder you work in, then copy it again after each change:

```sh
universe extension install ~/src/my-theme
universe extension update my-theme
```

Big Screen loads the new copy at its next start.

A checkout of Universe has a small theme to start from in `examples/theme`:

```sh
universe extension install ./examples/theme
```

[Frontends](/docs/developers/frontends/#themes) explains how a theme is loaded and [what it can rely on](/docs/developers/frontends/#what-a-theme-can-rely-on). [Extensions](/docs/developers/extensions/) has the folder layout and the API number.
