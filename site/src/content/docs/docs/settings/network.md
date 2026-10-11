---
title: Network
description: "Settings › Network shows the connection, tests the internet, and joins the Wi-Fi networks in range."
sidebar:
  order: 9
---

Settings › **Network** shows how the machine is connected and joins Wi-Fi networks, through NetworkManager. It shows only when NetworkManager lists a Wi-Fi device, and never under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode).

## Status

- **Connection**: **Cable**, the Wi-Fi network joined, or **Offline**. The detail line says how the internet answered last, such as "The network wants a sign-in first" or "No internet".
- **Test connection** (A: **Test**) asks NetworkManager again, and the answer comes as a notice.

## Wi-Fi

**Wi-Fi** turns the machine's Wi-Fi on or off. A cable needs nothing set up.

## Networks

While Wi-Fi is on, this card lists the networks in range, scanning for as long as the section is open. Each reads **Connected**, **Connecting…** or **Saved**, with its signal and security; a lock marks a secured one.

- **A** (**Connect**) joins a network, asking for its password when Universe has none. A wrong password offers **Try Again**.
- **A** (**Forget**) on the network in use asks first. Universe then stops joining it on its own, and joining it again takes its password.

Universe doesn't set up WEP or WPA Enterprise networks, and dims them. Join one once from a desktop, and Universe can join it from then on.

## From the command line

[`universe network`](/docs/reference/cli/network/) lists the networks in range, and its `scan`, `connect <ssid>`, `forget <ssid>`, `wifi on` or `wifi off`, `check` and `watch` do the rest.

## In the other looks

The Switch 2 look calls it **Internet** (System Settings › Internet), with **Test Connection**, **Connection Status** and the networks under **Internet Settings**; there **X** forgets a saved network. The PS5 look keeps the name, Settings › **Network**, with the parts **Settings**, **Set Up Internet Connection** and **Connection Status**. Universe Desktop has no Network page, and leaves Wi-Fi to the desktop's own settings.
