---
title: Network
description: "Settings › Network shows the connection, tests the internet, and joins the Wi-Fi networks in range."
sidebar:
  order: 9
---

Settings › **Network** shows how the machine is connected and joins Wi-Fi networks, through NetworkManager. It shows only when NetworkManager lists a Wi-Fi device, and never under [Steam's Game Mode](/docs/getting-started/steam-deck/#steams-game-mode). A PC on a cable alone has no Network section.

## Status

- **Connection**: **Cable**, the name of the Wi-Fi network joined, or **Offline**. The detail line says how the internet answered last: "The internet answers", "Connected, the internet does not answer", "The network wants a sign-in first", "No internet", or "Not checked yet".
- **Test connection** (A: **Test**) asks NetworkManager whether the internet answers. The row reads "Testing…", and the answer comes as a notice.

## Wi-Fi

**Wi-Fi** turns the machine's Wi-Fi on or off. There is nothing to set up for a cable.

## Networks

While Wi-Fi is on, this card lists the networks in range, with their number on its line, or reads "Looking for networks…" until one shows up. The section keeps scanning while it's open.

- The value reads **Connected**, **Connecting…** or **Saved**.
- The detail gives the signal (weak, fair or strong) and the security, such as Open, WPA2, WPA3 or WPA Enterprise. A lock marks a secured network.
- **A** (**Connect**) joins a network. A secured one Universe has no password for asks for it, and a wrong password offers **Try Again**. The notice names the network joined, and adds when it wants a sign-in or has no internet yet.
- **A** (**Forget**) on the network in use asks "Forget …?" first. Universe then stops joining it on its own, and joining it again takes its password.

Universe doesn't set up WEP or WPA Enterprise networks, and dims them. Join one once from a desktop, and Universe can join it from then on.

## From the command line

[`universe network`](/docs/reference/cli/network/) lists the networks in range, and its `scan`, `connect <ssid>`, `forget <ssid>`, `wifi on` or `wifi off`, `check` and `watch` do the rest.

## In the other looks

The Switch 2 look calls it **Internet** (System Settings › Internet), with **Test Connection**, **Connection Status** and the networks under **Internet Settings**; there **X** forgets a saved network. The PS5 look keeps the name, Settings › **Network**, with the parts **Settings**, **Set Up Internet Connection** and **Connection Status**. Universe Desktop has no Network page, and leaves Wi-Fi to the desktop's own settings.
