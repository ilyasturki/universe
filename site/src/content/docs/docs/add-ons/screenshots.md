---
title: Screenshots
description: The Screenshots module takes the shots you ask for from the dock, a pad button or universe screenshot. Which tool takes them, its settings, where they go.
sidebar:
  order: 5
---

The Screenshots module (`screenshot`) takes the screenshots you ask for, from the dock, a pad button or the command line. It starts off. Until you turn it on in Settings › **Features**, the dock answers "Screenshot failed" and `universe screenshot` answers "no enabled module takes screenshots".

## Taking one

- In the dock over a game, the **Screenshot** button, or **X** anywhere in the dock. In the PS5 look's Control Center, **Screenshot**, or **X** anywhere in it. The Switch 2 look has no menu over the game, so use a pad button there.
- A pad button with the **Screenshot** macro. Several pads come with one bound, such as the DualSense Edge's **Left Fn**, the Xbox Elite's **P3 (left upper)**, an Xbox pad's **Share** and the Switch Pro's **Capture**. Give the macro to another button in Settings › **Controller** ([Macros and paddles](/docs/controllers/macros/)).
- [`universe screenshot`](/docs/reference/cli/screenshot/), with or without a game running.

When Big Screen takes the shot, a shutter sounds and, over a game inside its own gamescope, the screen flashes white. The dock fades out before the shot and the shutter and flash come after it, so none of them ends up in the picture.

## Which tool takes it

The module asks whatever draws the screen:

- Inside Universe Big Screen's own gamescope (the launcher full screen, or the Universe session), gamescope itself, whatever the desktop.
- On GNOME, the Universe Shell extension, which [`universe setup`](/docs/reference/cli/setup/) installs ([Install](/docs/getting-started/install/#after-installing)).
- On KDE Plasma, spectacle.
- On Sway, Hyprland and niri, grim.
- On X11, Cinnamon included, the X server.
- Anywhere else, or when the desktop takes none, gpu-screen-recorder, if it's installed.

In Steam's Game Mode, Steam takes the screenshots, and Universe answers "Screenshots belongs to Steam in Game Mode" ([Steam Deck and Game Mode](/docs/getting-started/steam-deck/#steams-game-mode)).

## Settings

The three settings belong to each game, on its **Game settings** page under **Screenshots**. The module's page sets the last two for every game in its **Game defaults** card.

- **Take screenshots** (on). Off, that game takes none.
- **The game's window alone, not every monitor** (on). Inside gamescope, that's the game alone at the size it renders, without overlays such as MangoHud; turned off, the overlays are in the shot. On the desktop it's the focused window, or the whole screen when the setting is off. niri gives the whole screen either way.
- **Show the cursor in screenshots** (off). It has no effect inside gamescope or on X11.

## Where they go

Each shot is a PNG named after the moment it was taken, in the game's folder, as in `~/.local/share/universe/games/<id>/screenshots/20261010-201532.png`. Any `YYYYMMDD-HHMMSS.png` or `.jpg` you drop in that folder is listed with them.

A shot belongs to a session when it was taken between 90 seconds before the session started and 120 seconds after it ended. That's how the [Play journal](/docs/add-ons/play-journal/) finds the shots it writes from.

A shot taken with no game running lands in `~/.local/state/universe/screenshots/`, which no list shows.

The [Media](/docs/using/media/) tab and the game's Screenshots page show them, and [`universe screenshots`](/docs/reference/cli/screenshots/) lists them, newest first. Removing a shot sends it to the trash and takes it out of the journal entry that shows it.
