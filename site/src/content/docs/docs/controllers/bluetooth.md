---
title: Bluetooth pairing
description: Pair, connect and forget pads and other devices in Settings › Bluetooth, answer pairing questions, and pair a Sony pad over its cable.
sidebar:
  order: 5
---

Settings › **Bluetooth** pairs pads, headphones and keyboards through BlueZ. It shows only when the machine has a Bluetooth adapter, and never in Steam's Game Mode, where Steam handles Bluetooth.

## Settings › Bluetooth

- **Bluetooth** turns the adapter on or off.
- **Paired** lists the devices paired with this machine.
- **Found** lists what a search found. Universe searches as long as the page is open.

Each row shows the device's kind, its state (**Pairing…**, **Connected** or **Not connected**) and its charge when BlueZ knows it. Nothing pairs until you pick it under Found.

## Pair a controller

1. With no pad connected yet, plug one in by USB to get around the page.
2. Hold the pairing button of the pad you're pairing until its light blinks: SYNC on a Switch Pro, PS and Create together on a DualSense, the pair button on an Xbox pad.
3. Open Settings › Bluetooth and wait for the pad under **Found**.
4. Press **A** on it (**Pair**). Universe pairs it, trusts it so it reconnects by itself later, and connects it.
5. Answer any question the pad asks ([Pairing questions](/docs/controllers/bluetooth/#pairing-questions)).

**A** on a paired device (**Options**) offers **Connect** or **Disconnect**, and **Forget**, which unpairs it. A failed pairing or connection shows a notice with BlueZ's reason.

## Pairing questions

| The device | Universe shows | You |
|---|---|---|
| Shows a code to confirm | `Pair <name>?` and `Check that <name> shows <code>.` | Pick **Pair** if the codes match, else **Cancel** |
| Asks to be let in | `Pair <name>?` and "It asks to connect to this machine." | **Pair** or **Cancel** |
| Wants its passkey | A keyboard titled `Pair <name>` | Type the code the device shows |
| Wants a PIN | A keyboard titled `Pair <name>` | Type its PIN, often 0000 or 1234 |
| Wants a code typed on it | `Type <code> on <name>, then press Enter.` | Type it on the device; **Cancel** gives up |

A question left unanswered for 60 seconds is refused. One that comes while a game holds the screen is refused too, with a notice to pair the device again from HOME.

## Pairing a DualSense or DualShock 4 by cable

In the [Universe session](/docs/getting-started/universe-session/), a DualSense or DualShock 4 plugged in by USB can pair over the cable. Universe asks `Pair <name>?`, and after **Pair** the pad connects without the cable. On a desktop, the desktop's own Bluetooth agent asks instead.

If a game holds the screen when you plug the pad in, Universe refuses with `Plug <name> in again from HOME`. Press HOME, then plug it in again.

## In the other looks

The Switch 2 look pairs pads from **Controllers** › **Pair New Controller**, which lists pads alone, under Paired Controllers and Controllers Found. Headphones, speakers and keyboards go through System Settings › **Bluetooth**. The PS5 look calls the section Settings › **Accessories**, and its Controllers page has **Pair a Controller**.

## From the command line

[`universe bluetooth`](/docs/reference/cli/bluetooth/) lists the adapter's state, then the paired and found devices with their address, kind, state and charge.

```sh
universe bluetooth scan --seconds 20
universe bluetooth pair AA:BB:CC:DD:EE:FF
universe bluetooth remove AA:BB:CC:DD:EE:FF
```

`scan` searches for 10 seconds by default, then lists what it found. `pair` pairs, trusts and connects the device, asking any question on the terminal. `connect`, `disconnect`, `remove` (unpair and forget) and `power on` or `power off` do the rest.

## A pad that misbehaves

When a paired pad stops answering during a game, press **HOME**, pick the dock's **Controllers** button, and press **A** on the pad. Universe drops its Bluetooth link and brings it back. `universe controller reconnect` does the same from a terminal, and [Emulator controls](/docs/controllers/emulator-controls/#when-a-pad-stops-answering) has the details.
