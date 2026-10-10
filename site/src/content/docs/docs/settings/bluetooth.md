---
title: Bluetooth
description: "Settings › Bluetooth turns Bluetooth on or off, and pairs controllers, headphones and keyboards."
sidebar:
  order: 10
---

Settings › **Bluetooth** pairs and connects controllers, headphones, keyboards and mice, through BlueZ. It shows only when BlueZ has an adapter, and never under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode).

## Bluetooth

**Bluetooth** turns the adapter on or off. Its detail line reads "Plug a controller in by USB to use it at once; pair it here to go without the cable."

## Paired

This card lists the devices paired with this machine, each with an icon for its kind (controller, headphones, keyboard, mouse). The value reads **Connected** or **Not connected**, and the detail gives the battery level when the device reports one.

**A** (**Options**) offers **Connect** or **Disconnect**, and **Forget**.

## Found

This card lists the devices nearby that are in pairing mode. Its line reads **Searching** while the adapter looks, and the adapter searches for as long as the section is open. With nothing found yet, a **Searching…** row says to hold the device's pairing button until its light blinks.

**A** (**Pair**) pairs a device, and its value reads **Pairing…** meanwhile.

## Pairing questions

Some devices ask a question while they pair, and it comes up over whatever page is open:

- "Pair …?" to check that the device shows the same code, or to let it connect.
- A sheet to type a code the device shows, or its PIN (often 0000 or 1234).
- A code to type on the device itself, then <kbd>Enter</kbd>.

A controller plugged in by cable can ask too, and once you say yes it connects without the cable. While a game holds the screen, Universe answers no, and a notice says to pair the device, or plug it in, again from HOME.

## From the command line

[`universe bluetooth`](/docs/reference/cli/bluetooth/) lists the paired and found devices. `scan`, `pair <addr>`, `connect <addr>`, `disconnect <addr>`, `remove <addr>` (which forgets), `power on` or `power off`, and `watch` do the rest.

[Bluetooth pairing](/docs/controllers/bluetooth/) covers pairing a controller.

## In the other looks

The Switch 2 look has System Settings › **Bluetooth**, with **Paired Devices** and **Devices Found**; its Controllers page has **Pair New Controller**, which lists controllers only. The PS5 look calls it Settings › **Accessories**, with **Registered Accessories** and **Found Accessories**. Universe Desktop has no Bluetooth page.
