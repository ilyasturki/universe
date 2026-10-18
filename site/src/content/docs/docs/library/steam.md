---
title: Steam
description: Play your Steam games from Universe, with the Steam client in the background.
sidebar:
  order: 2
---

Universe runs your Steam games itself, with the Steam client in the background, and leaves the downloads to Steam. Steam has to be installed. The store is off until you press **Y** on **Steam** in Settings › **Stores**.

## Signing in with a Web API key

1. Sign in to the Steam client on this machine.
2. On Settings › Stores › Steam, pick **Get an API key**. It shows Steam's key page as a QR code and as an address.
3. Register a key there. Steam asks for a domain name, and any will do.
4. Pick **Enter the key** and enter it.

Settings › Downloads then lists the games the account owns. An account that keeps its game details private lists none.

## Installing, updating and uninstalling

Downloads works as for [every store](/docs/library/gog/#installing-from-the-store), except that Steam does the work:

- **Install** opens Steam's install window. Confirm the install there, and Universe follows the download.
- **Cancel install** only stops Universe watching: the download goes on in Steam.
- **Update** waits for Steam to run the update. When it doesn't, start it from Steam's Downloads page.
- **Uninstall…** opens Steam's uninstall dialog.

A game someone shares with you through Steam can't be added to the library.

## Launching a Steam game

Universe starts the Steam client in the background when it isn't running, then starts the game itself. Two kinds of games don't fit that:

- A game with Steam's own DRM restarts itself through Steam, outside Universe.
- A Linux build that needs Steam's runtime doesn't start. On NixOS, that's most of them.

## Steam Cloud

Steam Cloud only syncs around the sessions the Steam client launches itself. With **Share Steam's prefix** on, the saves stay where Steam keeps them: the client syncs them the next time it launches the game, and Universe's [backups](/docs/library/saves/) cover them until then. With it off, they never reach Steam Cloud.

## Steam's settings

- **Share Steam's prefix**, on by default, runs a Windows game in the prefix and with the Proton build Steam picked for it, so Steam and Universe see the same saves. It applies to games added from then on.
- **Achievements**, on by default, reads a game's achievements from the Steam client after each session.
