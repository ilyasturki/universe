---
title: Video capture
description: The Video capture feature records each session with gpu-screen-recorder.
sidebar:
  order: 3
---

Video capture records each session with gpu-screen-recorder, from the moment the game starts until it ends. Turn it on in Settings › **Features** › **Video capture**.

## What it needs

gpu-screen-recorder 6.1 or newer, installed from your distribution or its project rather than as a Flatpak, and ffmpeg. While the feature is on, [System check](/docs/settings/system-check/) has a line for each part it needs, with the fix for your system. Without the recorder's helper set up, each recording asks for your password.

## What it records

It records every session of a game whose **Record the session** is on: the whole screen, or the game's window alone with **Capture** set to **One window**. A recording shorter than **Shortest recording kept**, four minutes at first, goes to the trash.

While **Pause on HOME** has the game frozen, the recording pauses too. When a recording can't start, a message says "Recording failed".

On GNOME, recording one window needs the Universe Shell extension ([After installing](/docs/getting-started/install/#after-installing)). At the game's first launch, GNOME asks you to pick the game's window, and later launches find it by themselves.

## Where recordings land

In `~/Videos/universe`, unless you move the folder in Settings › **Game defaults** › **Folders** › **Recordings**, shown with **Y**. The [Media](/docs/using/media/) tab and a game's Recordings page play them.

## Settings

**Record the session**, **Capture** and **Show the cursor in the recording** belong to each game, in its **Game settings** under **Video capture**. The feature's page holds the rest:

- **Video codec**: **Auto** takes the best one your GPU encodes.
- **Quality**: **Very high** by default, about 7 to 8 GB an hour at 4K on an AMD card.
- **Frame rate**: 60 by default.
- **Video size**: **Native** by default, or a smaller size.
- **Audio**: **Output**, what the speakers play, **Output and microphone**, or **None**.

**Y** shows the advanced rows, such as the container and **Shortest recording kept**.
