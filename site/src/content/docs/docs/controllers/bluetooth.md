---
title: Bluetooth pairing
description: Pair, connect and forget pads and other devices in Settings › Bluetooth, answer pairing questions, and pair a Sony pad over its cable.
sidebar:
  order: 5
---

Settings › **Bluetooth** pairs pads, headphones and keyboards through BlueZ. It shows when the machine has a Bluetooth adapter, and not in Steam's Game Mode, where Steam's own settings handle Bluetooth.

## Settings › Bluetooth

- **Bluetooth** turns the adapter on or off. Its detail reads "Plug a controller in by USB to use it at once; pair it here to go without the cable."
- **Paired** lists the devices paired with this machine.
- **Found** lists what a search found. Universe searches as long as the page is open, and the card's header reads "Searching" meanwhile. With nothing found yet, it shows "Searching…" and "Put the device in pairing mode: hold its pairing button until its light blinks."

Each row has the device's name, an icon for its kind (pad, headphones, keyboard, mouse), its state (**Pairing…**, **Connected** or **Not connected**) and its charge ("Battery 80%") when BlueZ knows it.

Nothing pairs by itself. A pad in pairing mode nearby only shows under Found until you pick it.

## Pair a controller

1. With no pad connected yet, plug one in by USB to get around the page.
2. Put the pad you're pairing in pairing mode by holding its pairing button until its light blinks (SYNC on a Switch Pro, PS and Create together on a DualSense, the pair button on an Xbox pad).
3. Open Settings › Bluetooth and wait for the pad under **Found**.
4. Press **A** on it (its action reads **Pair**). Universe pairs it, trusts it so it reconnects by itself later, and connects it.
5. If the pad asks something, answer it ([Pairing questions](/docs/controllers/bluetooth/#pairing-questions)).

**A** on a paired device (its action reads **Options**) offers **Connect** or **Disconnect**, and **Forget**, which unpairs it and drops it from the list. A pairing or connection that fails shows a notice with BlueZ's reason.

## Pairing questions

Some devices ask a question while they pair:

| The device | Universe shows | You |
|---|---|---|
| Shows a code to confirm | `Pair <name>?` and `Check that <name> shows <code>.` | Pick **Pair** if the codes match, else **Cancel** |
| Asks to be let in | `Pair <name>?` and "It asks to connect to this machine." | **Pair** or **Cancel** |
| Wants its passkey | A keyboard titled `Pair <name>` | Type the code the device shows |
| Wants a PIN | A keyboard titled `Pair <name>` | Type its PIN, often 0000 or 1234 |
| Wants a code typed on it | `Type <code> on <name>, then press Enter.` | Type it on the device; **Cancel** gives up |

A question left unanswered for 60 seconds is refused. One that comes while a game holds the screen is refused too, with a notice to pair the device again from HOME.

## Pairing a DualSense or DualShock 4 by cable

In the [Universe session](/docs/getting-started/universe-session/), a DualSense or DualShock 4 plugged in by USB can pair over the cable. Universe asks `Pair <name>?` with "It is plugged in by cable: after this it connects without one." Pick **Pair**, and from then on the pad connects without the cable.

This works only in the Universe session, where Universe answers BlueZ's questions for the whole machine. On a desktop, the desktop's own Bluetooth agent asks instead. If a game holds the screen when you plug the pad in, Universe refuses and says `Plug <name> in again from HOME`. Press HOME, then plug it in again.

## In the other looks

The Switch 2 look pairs pads from **Controllers** › **Pair New Controller**, which lists pads alone, under Paired Controllers and Controllers Found. Headphones, speakers and keyboards go through System Settings › **Bluetooth**. The PS5 look calls the section Settings › **Accessories**, and its Controllers page has **Pair a Controller** ("Hold its pairing button until the light blinks: PS and Create together on a DualSense.").

## From the command line

[`universe bluetooth`](/docs/reference/cli/bluetooth/) lists the adapter's state, then the paired and found devices with their address, kind, state and charge.

```sh
universe bluetooth scan --seconds 20
universe bluetooth pair AA:BB:CC:DD:EE:FF
universe bluetooth remove AA:BB:CC:DD:EE:FF
```

`scan` searches for 10 seconds unless told otherwise, then lists what it found. `pair` pairs, trusts and connects the device, asking any question on the terminal. `connect`, `disconnect`, `remove` (unpair and forget) and `power on` or `power off` do the rest. With no adapter, or BlueZ not running, it says "no Bluetooth adapter, or BlueZ is not running".

## A pad that misbehaves

When a paired pad stops answering during a game, press **HOME**, pick the dock's **Controllers** button, and press **A** on the pad. Universe drops its Bluetooth link and brings it back. If the pad switched itself off, the notice says so, and its PS or HOME button wakes it. [Emulator controls](/docs/controllers/emulator-controls/#when-a-pad-stops-answering) has the details, and `universe controller reconnect` does the same from a terminal.
