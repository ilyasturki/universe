---
title: Virtual pads
description: The Virtual pads module gives an emulator one PlayStation-class pad per player, with gyro, touchpad and rumble, whatever pads you hold.
sidebar:
  order: 6
---

Virtual pads (`pads`) gives an emulator one virtual pad per player, named **Universe Pad 1**, **Universe Pad 2** and so on, whatever pads you hold. The emulator sees a PlayStation-class pad with gyro, touchpad and rumble, and never your real pads, so you can swap one for another without it noticing. It serves emulated games only. Turn it on in Settings › **Features** › **Virtual pads**.

## What it needs

- `/dev/uhid` open to your user. The controller rules that come with the AUR and COPR packages and with the installer open it and load `uhid` at boot ([Install](/docs/getting-started/install/)). On NixOS, `programs.universe.controller.enable` does it, and it's on by default.
- SDL3.
- systemd's `systemd-run` and `systemctl`.

Without `/dev/uhid` or SDL3, the emulator gets your real pads as they are. System check tests neither, so the sign is the emulator listing your own pads instead of Universe's.

## Players

A pad takes the lowest player that has none, up to eight; a ninth pad is left out. A pad that leaves keeps its player's virtual pad in place, holding nothing, until another pad takes it. Each virtual pad keeps its name and its SDL identity from one session to the next, so the emulator's bindings hold whichever pad you pick up.

Universe's own menus, **HOME** and your macros keep reading the real pads.

## Rumble, gyro and touchpad

The emulator's rumble goes back to the real pad behind that player. Each virtual pad carries its real pad's sticks, triggers and buttons, its gyro and accelerometer when it has them, its touchpad (two fingers and the click) and its battery level.

## HOME

**HOME** stays Universe's. Turn on **Share HOME with the emulator** and the virtual pads send it as well, for the console's own HOME or PS button.

While **HOME** has the game paused, the virtual pads report nothing held, so what you press in the dock doesn't reach the game.

## Reconnect from the dock

In the Universe look, the dock over a game has a **Controllers** button listing each pad. **A** on a pad reconnects it. A Bluetooth pad's link is dropped and brought back; a pad on a cable has to be pulled out and plugged in again. Virtual pads then makes every virtual pad anew, with the same names and identities, and each real pad that returns gets its player back. **Find controllers** looks again for a pad Universe doesn't show.

The PS5 look has the same panel under **Controllers** in its Control Center, and the Switch 2 look a **Reconnect** row per pad on its Controllers page. From a terminal, `universe controller reconnect` does every pad, or one pad by the id `universe controller ls` prints first ([`universe controller`](/docs/reference/cli/controller/)).

## Settings

- **Hand the pads to the emulator as virtual pads** (on) belongs to each emulated game, on its **Game settings** page under **Virtual pads**. Off, that game gets your real pads.
- **Share HOME with the emulator** (off) is on the module's page, for every game.

## Virtual pads and Emulator controls

The two never both serve a game. While Virtual pads can serve one (the module is on, on for that game, and `/dev/uhid` is open), [Emulator controls](/docs/controllers/emulator-controls/) leaves the emulator's controller settings as they are, and its Nintendo button layout doesn't apply. Turn Virtual pads off for a game, and Emulator controls, if it's on, takes over.
