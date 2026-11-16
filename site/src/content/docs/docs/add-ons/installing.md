---
title: Installing add-ons
description: Install, update and remove the modules, stores and themes others made.
sidebar:
  order: 2
---

An add-on is a module, a store or a theme someone else made. Settings installs them from Universe's index.

:::caution
An add-on runs as you, with your files and your accounts. Nothing sandboxes it.
:::

## From Settings

**Get more…**, at the end of Settings › **Features**, **Stores** and **Appearance**, opens the **Add-ons** menu. Each row shows the add-on's version and size, **Installed**, **Update** with the new version, or **Needs another Universe** when this version of Universe can't run it.

Pick an add-on to install it, or, once installed, to update or **Remove** it. Installing turns nothing on: turn a module on in Features and a store in Stores, and pick a theme in Appearance ([Themes](/docs/add-ons/themes/)).

## Updates

Nothing updates an add-on by itself: update it from the Add-ons menu. After you update Universe, an add-on that doesn't accept the new version reads **Unavailable**, and [System check](/docs/settings/system-check/) says to update it or turn it off.

Updating or removing an add-on is refused while a game runs: do it once you've quit the game.

## From a terminal

[`universe extension install`](/docs/reference/cli/extension/) also takes an add-on's URL, an archive or a folder. Settings lists such an add-on as **Unlisted**, and can only remove it.

To write your own, see [Extensions](/docs/developers/extensions/).
