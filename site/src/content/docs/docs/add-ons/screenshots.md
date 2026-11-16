---
title: Screenshots
description: The Screenshots feature takes the screenshots you ask for, from the dock or a pad button.
sidebar:
  order: 5
---

The Screenshots feature takes the screenshots you ask for. It starts off: turn it on in Settings › **Features** › **Screenshots**.

## Taking one

- In the dock over a game, the **Screenshot** button, or **X** anywhere in the dock.
- A pad button with the **Screenshot** macro, such as an Xbox pad's **Share** or the Switch Pro's **Capture** ([Macros and paddles](/docs/controllers/macros/)).
- [`universe screenshot`](/docs/reference/cli/screenshot/) in a terminal.

On a GNOME desktop, when Big Screen isn't full screen, Universe takes the shot through its Shell extension ([After installing](/docs/getting-started/install/#after-installing)). In [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode), Steam takes the screenshots.

## Settings

Each game has these in its **Game settings**, under **Screenshots**, and the feature's page sets the last two for every game:

- **Take screenshots**, on.
- **The game's window alone, not every monitor**, on. When Big Screen runs full screen, that's the game alone, without overlays such as MangoHud.
- **Show the cursor in screenshots**, off.

## Where they go

The [Media](/docs/using/media/) tab and the game's Screenshots page show them. A shot taken with no game running lands in `~/.local/state/universe/screenshots/`, which no list shows.
