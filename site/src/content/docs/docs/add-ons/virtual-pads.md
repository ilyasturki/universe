---
title: Virtual pads
description: The Virtual pads feature gives an emulator one virtual pad per player, whatever pads you hold.
sidebar:
  order: 6
---

Virtual pads gives an emulator one virtual pad per player, **Universe Pad 1**, **Universe Pad 2** and so on, whatever pads you hold. The emulator sees a PlayStation-class pad with gyro, touchpad and rumble, never your real pads, so you can swap one pad for another without it noticing. It serves emulated games only. Turn it on in Settings › **Features** › **Virtual pads**; [Emulator controls](/docs/controllers/emulator-controls/#which-one-to-pick) compares the two.

## Players

A pad takes the lowest player that has none, up to eight. A pad that leaves keeps its player's virtual pad plugged in, holding nothing, until another pad takes it. After a [reconnect](/docs/using/playing/#controllers), each pad gets its player back. The emulator's bindings hold whichever pad you pick up.

The emulator's rumble goes to the real pad behind that player. Universe's own menus, **HOME** and your macros keep reading the real pads.

## HOME

**HOME** stays Universe's. Turn on **Share HOME with the emulator**, on the feature's page, and the virtual pads send it too, for the console's own HOME or PS button. While **HOME** has the game paused, the virtual pads press nothing.

## When the emulator lists your own pads

That means Virtual pads couldn't start, and the emulator got your real pads as they are. It needs SDL3 and the controller [system setup](/docs/controllers/supported/#system-setup) your install does. System check doesn't test either.

To give one game your real pads, turn off **Hand the pads to the emulator as virtual pads** in its **Game settings**.
