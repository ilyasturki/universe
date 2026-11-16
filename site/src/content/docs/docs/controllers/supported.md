---
title: Supported controllers
description: The controllers Universe knows, the buttons it draws for each, and what your system needs.
sidebar:
  order: 1
---

Universe works with any controller Linux reads as a gamepad. Nine models get their own button names, glyphs and extra buttons; any other pad is a plain **Controller** with Xbox letters.

## Controller families

| Controller | Extra buttons |
|---|---|
| Steam Deck | L4, L5, R4, R5, Quick access, the two trackpads |
| DualSense Edge | Left and Right Fn, the two back buttons |
| DualSense, DualShock 4 | none |
| Xbox Elite Series 2 | the paddles P1 to P4 |
| Xbox | Share |
| Switch 2 Pro | GL, GR, C, Capture |
| Switch Pro | Capture |
| 8BitDo Pro 3 | L4, R4, PL, PR, Star |

Extra buttons take [macros](/docs/controllers/macros/). A button that reads **Unbound** in Settings › **Controller** sends nothing on this connection: [learn it](/docs/controllers/button-setup/#learn-one-button) if the pad has it. The DualSense Edge's extra buttons need Linux 7.2 or newer, and no mode of the 8BitDo Pro 3 seen so far sends its Star to Linux.

## Button glyphs

The hints draw each button as your pad prints it:

| Controller | Face buttons | Shoulders | Select, Start, HOME |
|---|---|---|---|
| DualSense, DualSense Edge | ✕ ○ △ □ | L1 R1 L2 R2 | Create, Options, PS |
| DualShock 4 | ✕ ○ △ □ | L1 R1 L2 R2 | Share, Options, PS |
| Xbox, Xbox Elite Series 2, Controller | A B X Y | LB RB LT RT | View, Menu, Xbox |
| Switch Pro, Switch 2 Pro | B A X Y | L R ZL ZR | Minus, Plus, Home |
| 8BitDo Pro 3 | B A X Y | L1 R1 L2 R2 | Minus, Plus, Home |
| Steam Deck | A B X Y | L1 R1 L2 R2 | View, Menu, Steam |

On a Nintendo-style pad, the A on the right confirms and the B at the bottom goes back, as printed. Until Universe sees a pad, the hints follow the last one it saw or the one picked in the first run, else Xbox letters (the Deck's buttons on a Deck). Pick up the keyboard or the mouse and they show key caps.

## Switch 2 Pro Controller

Linux can't read the Switch 2 Pro yet. The menus don't move with it, and HOME, macros, button setup and its battery level don't work. Emulators still read it over USB. While one is plugged in, Settings › System check shows a failing **Switch 2 Pro Controller** entry.

## System setup

Key-combo macros and [Virtual pads](/docs/add-ons/virtual-pads/) need access that only root can give. The packages, the installer and the NixOS module set it up.

In emulators, a pad's gyro and touchpad need the pad opened to you too. `game-devices-udev` does it for Sony and Nintendo pads, and `steam-devices` for an Xbox pad on Bluetooth.

Settings › [System check](/docs/settings/system-check/) checks the key-combo and gyro access under **Controller**, with a fix for each. It can't tell whether [Virtual pads](/docs/add-ons/virtual-pads/#when-the-emulator-lists-your-own-pads) will start.

## Battery levels

In the Universe look, the batteries sit next to the clock: the machine's own, and one per pad whose charge Universe can read. A pad at 15% or less and not charging turns red, and a charging one carries a bolt. With several pads, each also gets a number, the player its lights show.

## A pad Universe reads wrong

Record it, then attach the file to a bug report:

```sh
universe controller ls
universe controller dump <id> --walk --json > my-pad.json
```

`--walk` asks you to press each button and move each stick. [Controllers by model](/docs/developers/api/#controllers-by-model) lists the pads already recorded.
