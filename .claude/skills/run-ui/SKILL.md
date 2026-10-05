---
name: run-ui
description: Run, drive or screenshot the Universe UI to see a change working. Use when confirming a UI or core change in the real app, when asked to run or screenshot it, or before opening any universe-ui window.
---

# Running the UI

`just ui`, `just ui-fake` and `just fresh ui` open a window on the user's desktop (fullscreen re-execs into gamescope) and play the UI's own sounds. That steals focus and disturbs the user: the default run is offscreen.

## Offscreen run

```
just ui-shot <dir> [keys…]
```

Qt's `offscreen` platform, audio unreachable (`PIPEWIRE_REMOTE=/nonexistent`), keys posted after load, `Shot:name.png` saved under `<dir>`, then it quits. `UNIVERSE_UI_SIZE=1280x800` is the Steam Deck's screen (the default is 1920x1080); `UNIVERSE_DECK=oled` plays one to the fake core (its System controls, its glyphs). Use the scratchpad as `<dir>` and Read the PNGs. `UNIVERSE_UI_ARGS` replaces the default `--fake --no-gamepad` (`--no-gamepad` alone runs the real core on `.dev/`; `--fake-launch` a fake session; `--theme switch2` the other look).

Offscreen covers everything drawn in the main window, both themes, the fake and real core, key-driven navigation, and with `UNIVERSE_UI_ARGS="--fake"` the controller screens' fake watcher (`Press:`, `Axis:`) and pad sticks (`Stick:`). It draws with the software scenegraph (`Theme.software`): every ShaderEffect is dropped, so covers come out square and masks, blurs and glows are missing. Judge layout and text from a shot; judge the look from a recording.

## Keys

The names are `KEY_NAMES` in `ui/universe_ui/gamepad.py` (`Right`, `Return`, `Esc`, `I`, `F1`…), and `--help` on `.venv/bin/universe-ui` lists the script phases (`Wait:5`, `Shot:`, `Hold:`…). An unknown name is skipped with an `unknown key` log line while the rest of the script runs on, so after one wrong name every later key lands on a screen the script never planned for.

On the real core, a launching key (`Return` or `A` on a game, a pad's `Press:`) starts the game for real, fullscreen on the user's screen. Send one on the real core only in a gamescope take (Recording), right after `Expect:<game id>`, which quits the script when Return would launch anything else.

## Recording

```
just ui-record <out.mp4> [keys…]
```

The UI in a headless sway rendering on the boot GPU, recorded by wf-recorder: no window, no sound, the real scenegraph with every effect. Out is H.264 at 60 fps, supersampled 2x and scaled back, ready to post. Same keys and `UNIVERSE_UI_*` variables as `ui-shot`, 500 ms between keys; `tools/ui-record --help` has the rest. Check it with `ffmpeg -ss <t> -i <out> -frames:v 1 <png>` and Read the frame.

A gamescope take, `UNIVERSE_RECORD_GAMESCOPE=1` with `UNIVERSE_UI_ARGS=--no-gamepad`, films what runs only inside gamescope: a real launch, HOME over the game, MangoHud, the volume level (`Volume:up`). The UI goes fullscreen in its own gamescope inside the headless sway, on a copy of the profile, the game's sound into a private sink, and the take stops the game it started. `Mark:NAME` keys land as cut points in `<out>.marks`; `tools/readme-video` is a worked take.

Anything shown outside the repo (README, posts) is recorded on `UNIVERSE_DEV=.dev-showcase`, which `just showcase` rebuilds from `.dev`: PC games with full artwork only, no emulated or hand-added games. The Media tab there still holds dev test sessions; leave it out of public footage.

## Visible run

Not reachable offscreen: the physical gamepad and the Recordings player's video; the gamescope re-exec, the overlay over a game and a real launch only in a gamescope take. When the change lives there, run it visibly (`just ui`, `just ui --windowed`) — but tell the user first, in the sentence before the command, that a window and sound are coming.
