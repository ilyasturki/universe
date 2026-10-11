---
title: Video capture
description: The Video capture module records each session with gpu-screen-recorder. What it needs, every setting, and where the recordings land.
sidebar:
  order: 3
---

Video capture (`capture`) records each session with gpu-screen-recorder, from the moment the game starts until it ends. The [Play journal](/docs/add-ons/play-journal/) takes its frames from that recording. Turn it on in Settings › **Features** › **Video capture**.

## What it needs

- gpu-screen-recorder 6.1 or newer with its `gsr-cli`, installed natively. The Flatpak can't be driven from outside.
- ffmpeg, ffprobe and `trash`.
- gpu-screen-recorder's KMS helper, `gsr-kms-server`, with the `cap_sys_admin` capability. Without it, each recording asks for your password.

Without gpu-screen-recorder, `gsr-cli`, ffmpeg, ffprobe or `trash`, the module is Unavailable. While the module is on, [System check](/docs/settings/system-check/) has a line each for gpu-screen-recorder, `gsr-cli` and `gsr-kms-server`, with the fix for your system:

- On Arch, install the `gpu-screen-recorder` package, which gives the helper its capability. With PackageKit installed, Settings › **Emulators & Proton** offers to install it.
- Fedora and Debian don't package it. Install gpu-screen-recorder from its project, then give the helper its capability:

  ```sh
  sudo setcap cap_sys_admin+ep "$(command -v gsr-kms-server)"
  ```

- On NixOS, `programs.universe.capture.enable`, on by default, sets up gpu-screen-recorder and its helper at matching versions ([Nix options](/docs/reference/nix/)).

## What it records

It records every session of a game whose **Record the session** is on. It takes the whole screen the game runs on, or the game's window alone with **Capture** set to **One window**. The file grows in `~/.local/share/universe/modules/capture/pending/` while you play. When the session ends it's filed in the recordings folder, or goes to the trash if it's shorter than **Shortest recording kept**.

When gpu-screen-recorder can't start (a screen it won't take, no encoder), a message says "Recording failed" and the session has no recording.

If the screen being recorded goes dark or is unplugged, the recording moves to the screen that's still lit. The parts are joined into one file at the end, or the longest is kept if they can't be.

## Pausing under HOME

With **Pause on HOME** on (the default), the recording pauses while the dock or the launcher covers the game, and the paused stretches are cut from the file, so **Shortest recording kept** measures play. With it off, the game and the recording go on behind the launcher. [Playing](/docs/using/playing/) covers Pause on HOME.

## Where recordings land

Recordings land in `~/Videos/universe/<game id>/<session>.mkv`, or `.mp4` with **Container** set to MP4. The folder is `paths.recordings_root` in config.toml, by default `universe` inside your videos folder (`$XDG_VIDEOS_DIR`). Change it in Settings › **Game defaults** › **Folders** › **Recordings**, behind Advanced, or with [`universe config`](/docs/reference/cli/config/):

```sh
universe config set paths.recordings_root /mnt/recordings/games
```

The [Media](/docs/using/media/) tab, a game's Recordings page and [`universe recordings`](/docs/reference/cli/recordings/) list them. Removing a recording sends the file to the trash; the hours played stay. Removing a game from the library moves its recordings to `.archive/<game id>` in the same folder.

## One window, on GNOME

To record the game's window alone, Universe has to see the desktop's windows, which on GNOME takes the Universe Shell extension. Run [`universe setup`](/docs/reference/cli/setup/), then log out and back in ([Install](/docs/getting-started/install/#after-installing)).

At the game's first launch, GNOME's window picker opens. Pick the game's window. GNOME finds it again by its app id and title, so later launches record without asking. Cancel the picker and that session isn't recorded.

When the game's window doesn't show within **Window wait**, or Universe can't list the desktop's windows, it records the whole screen and says "Recording the screen".

## Settings

**Record the session**, **Capture** and **Show the cursor in the recording** belong to each game, on its **Game settings** page under **Video capture**. The module's page sets the last two for every game in its **Game defaults** card, and holds the rest.

| Setting | Default | |
| --- | --- | --- |
| **Record the session** | On | Per game. |
| **Capture** | The whole screen | Per game: **The whole screen** or **One window**. |
| **Show the cursor in the recording** | Off | Per game. |
| **Video codec** | Auto | Auto, AV1 10-bit, AV1, HEVC 10-bit, HEVC, H.264. Auto takes the first of AV1 10-bit, HEVC 10-bit, HEVC and H.264 that the GPU encodes. |
| **Quality** | Very high | Medium, High, Very high, Ultra. Each is a constant quality held under a bitrate ceiling. |
| **Frame rate** | 60 | Auto (the screen's refresh rate), 120, 90, 60, 30. Rates above the screen's aren't offered. |
| **Video size** | Native | Native, 3840x2160, 2560x1440, 1920x1080. A larger screen is scaled down to fit. |
| **Audio** | Output | Output (what the speakers play), Output and microphone (the microphone on a track of its own), None. |

Behind Advanced:

| Setting | Default | |
| --- | --- | --- |
| **Container** | MKV | MKV survives a crash; MP4 plays anywhere. |
| **Audio codec** | Opus | Opus, AAC, FLAC. gpu-screen-recorder has FLAC turned off, so choosing it records Opus. |
| **Audio bitrate** | Auto | In kbps, 96 to 320. Auto leaves it to gpu-screen-recorder. |
| **Shortest recording kept** | 240 | In seconds of play. A shorter recording goes to the trash. |
| **Window wait** | 60 | In seconds: how long the game's window may take before the whole screen is recorded instead. |
| **FFmpeg video options** | empty | Passed to gpu-screen-recorder as `-ffmpeg-video-opts`, in place of the Quality preset. |
| **Extra gpu-screen-recorder arguments** | empty | Added to gpu-screen-recorder's command line. |

Very high aims at 16 Mbps under a 32 Mbps ceiling, about 7 to 8 GB an hour at 4K on an AMD card. Medium is 6 and 12 Mbps, High 10 and 20, Ultra 24 and 48.

With [`universe module set`](/docs/reference/cli/module/), a setting takes its value as config.toml stores it (`very_high`, `hevc_10bit`, `window`), not its label:

```sh
universe module set capture quality=ultra codec=hevc_10bit
universe module set capture source=window --game "Dead Cells"
universe module set capture enabled=false --game "Dead Cells"   # stop recording that game
```
