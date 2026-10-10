---
title: Virtual pads
description: The Virtual pads module gives an emulator one PlayStation-class pad per player, with gyro, touchpad and rumble, whatever pads you hold.
sidebar:
  order: 6
---

Virtual pads (`pads`) gives an emulator one virtual pad per player, named **Universe Pad 1**, **Universe Pad 2** and so on, whatever pads you hold. The emulator sees a PlayStation-class pad with gyro, touchpad and rumble. Your real pads stay hidden from it, so you can swap one for another without the emulator noticing. The module serves emulated games only. It starts off until you turn it on in Settings › **Features** › **Virtual pads**.

## What it needs

- `/dev/uhid` open to your user. The controller rules that come with the AUR and COPR packages and with the installer open it and load `uhid` at boot ([Install](/docs/getting-started/install/)). On NixOS, `programs.universe.controller.enable` does it, and it's on by default.
- SDL3.
- systemd's `systemd-run` and `systemctl`.

Without `/dev/uhid` or SDL3, the module steps aside and the emulator gets your real pads as they are. System check tests neither, so the sign is the emulator listing your own pads instead of Universe's.

## Players

A pad takes the lowest player that has none. A pad that leaves keeps its player's virtual pad in place, holding nothing, until another pad takes it, so the emulator's bindings survive a change of pad. There are up to eight players, and a ninth pad is left out.

Each virtual pad keeps its name and its SDL identity from one session to the next. The emulator maps them as it maps any PlayStation pad, and those bindings hold whichever pad you pick up next time.

Universe's own menus, **HOME** and your macros keep reading the real pads, as they do without the module.

## Rumble, gyro and touchpad

The emulator's rumble goes back to the real pad behind that player. The other way, each virtual pad carries its real pad's sticks, triggers and buttons, its gyro and accelerometer when it has them, its touchpad (two fingers and the click) and its battery level.

## HOME

**HOME** stays Universe's. Turn on **Share HOME with the emulator** and the virtual pads send it as well, for the console's own HOME or PS button; Universe's menu still opens on it.

While **HOME** has the game paused, the virtual pads report nothing held, so what you press in the dock doesn't reach the game afterwards.

## Reconnect from the dock

In the Universe look, the dock over a game has a **Controllers** button. Its card lists each pad by name, with **Bluetooth** or **USB** and the charge when Universe knows it. **A** on a pad reconnects it. A Bluetooth pad's link is dropped and brought back; a pad on a cable has to be pulled out and plugged in again.

Either way, Virtual pads then makes every virtual pad anew, with the same names and identities. The emulator sees them unplugged and back, and each real pad that returns gets the player it held. The card's last row, **Find controllers**, looks again for a pad Universe doesn't show.

The PS5 look has the same panel under **Controllers** in its Control Center. The Switch 2 look gives each pad a **Reconnect** row on its Controllers page. From a terminal, `universe controller reconnect` does every pad, or one pad by the id `universe controller ls` prints first ([`universe controller`](/docs/reference/cli/controller/)).

## Settings

- **Hand the pads to the emulator as virtual pads** (on) belongs to each emulated game, on its **Game settings** page under **Virtual pads**. Off, that game gets your real pads.
- **Share HOME with the emulator** (off) is on the module's page, for every game.

## Virtual pads and Emulator controls

The two never both serve a game. While Virtual pads can serve one (the module is on, on for that game, and `/dev/uhid` is open), [Emulator controls](/docs/controllers/emulator-controls/) leaves the emulator's controller settings as they are, since what it would write is for pads the emulator no longer sees. Its Nintendo button layout doesn't apply to virtual pads either.

Turn Virtual pads off for a game, and Emulator controls, if it's on, writes that emulator's controller settings for the pads you hold at each launch.
