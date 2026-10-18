---
title: Themes
description: Install a theme others made, pick it, and get back to the Universe look.
sidebar:
  order: 7
---

A theme is a look for Universe Big Screen that someone else made. It's listed after the three built-in [looks](/docs/using/looks/) and picked the same way.

## Installing and picking one

Settings › **Appearance** › **Get more…** lists the themes in Universe's index ([Installing add-ons](/docs/add-ons/installing/)). Installing one keeps the look you have.

Then press **A** on **Theme**, in the same section, and pick the theme. It shows **Add-on** under its name, and **Can't be used** when it can't run. The switch happens at once.

## What a theme can't do

A theme draws Universe's main window and nothing else. It has no dock over a running game: holding **HOME** brings Universe back instead.

## Back to the Universe look

When a theme stops working, Universe puts its own look back and says why.

If a theme loads but leaves you no way out, hold **HOME** for 5 seconds. Universe puts its own look back and starts on it next time.

## Working on a theme

Install it from the folder you work in, then update it after each change. Big Screen loads the new copy at its next start.

```sh
universe extension install ~/src/my-theme
universe extension update my-theme
```

A checkout of Universe has a small theme to start from in `examples/theme`. [Frontends](/docs/developers/frontends/#themes) explains how a theme is loaded.
