---
title: Bluetooth
description: "Settings › Bluetooth turns Bluetooth on or off, and pairs controllers, headphones and keyboards."
sidebar:
  order: 10
---

Settings › **Bluetooth** pairs and connects controllers, headphones, keyboards and mice, through BlueZ. It shows only when BlueZ has an adapter, and never under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode).

## Bluetooth

**Bluetooth** turns the adapter on or off. A controller plugged in by USB works at once; pair it here to go without the cable.

## Paired

The devices paired with this machine, each reading **Connected** or **Not connected**, with its battery level when the device reports one.

**A** (**Options**) offers **Connect** or **Disconnect**, and **Forget**.

## Found

The devices nearby in pairing mode. The adapter searches for as long as the section is open. To show up, a device needs its pairing button held until its light blinks.

**A** (**Pair**) pairs a device.

## Pairing questions

Some devices ask a question while they pair, such as a code to confirm, or a PIN to type (often 0000 or 1234). It comes up over whatever page is open. While a game holds the screen, Universe answers no, and a notice says to pair the device, or plug it in, again from HOME.

## From the command line

[`universe bluetooth`](/docs/reference/cli/bluetooth/) lists the paired and found devices. `scan`, `pair <addr>`, `connect <addr>`, `disconnect <addr>`, `remove <addr>` (which forgets), `power on` or `power off`, and `watch` do the rest.

[Bluetooth pairing](/docs/controllers/bluetooth/) lists every [pairing question](/docs/controllers/bluetooth/#pairing-questions), and covers pairing a controller, a Sony pad over its cable included.

## In the other looks

The Switch 2 look has System Settings › **Bluetooth**, with **Paired Devices** and **Devices Found**; its Controllers page has **Pair New Controller**, which lists controllers only. The PS5 look calls it Settings › **Accessories**, with **Registered Accessories** and **Found Accessories**. Universe Desktop has no Bluetooth page.
