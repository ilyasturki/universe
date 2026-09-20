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

Qt's `offscreen` platform, audio unreachable (`PIPEWIRE_REMOTE=/nonexistent`), keys posted after load, `Shot:name.png` saved under `<dir>`, then it quits. Use the scratchpad as `<dir>` and Read the PNGs. Keys are Pegasus names (`Right`, `Return`, `Escape`, `I`, `F1`, `Wait:5`…): `--help` on `.venv/bin/universe-ui` lists the script phases, `universe-ui.1.scd` the bindings. `UNIVERSE_UI_ARGS` replaces the default `--fake --no-gamepad` (`UNIVERSE_UI_ARGS="--no-gamepad"` runs the real core on `.dev/`, where `Return` on a game launches it for real — a visible run, see below; `--fake-launch` a fake session; `--theme switch2` the other look).

Offscreen covers everything drawn in the main window, both themes, the fake and real core, key-driven navigation, and with `UNIVERSE_UI_ARGS="--fake"` the controller screens' fake watcher (`Press:`, `Axis:`) and pad sticks (`Stick:`).

## Visible run

Not reachable offscreen: the gamescope re-exec and nested overlay path, a real game or `just sample` launch, the physical gamepad, the Recordings player's video. When the change lives there, run it visibly (`just ui`, `just ui --windowed`) — but tell the user first, in the sentence before the command, that a window and sound are coming.
