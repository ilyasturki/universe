---
title: Steam
description: How Universe lists, installs and runs your Steam games with the Steam client in the background, and what it leaves to Steam, Steam Cloud included.
sidebar:
  order: 2
---

Universe runs your Steam games itself, a Windows build through Proton, with the Steam client in the background. The downloads stay Steam's. The store is off until you turn it on, and needs Steam installed: Universe doesn't fetch the `steam` program as it does the other stores' tools.

## Turning Steam on

In Settings › **Stores**, press **Y** on **Steam**, or open it and switch **Enabled** on. Without a `steam` program on your PATH, the row reads **Unavailable**. From a terminal: [`universe source enable steam`](/docs/reference/cli/source/).

## Signing in with a Web API key

Universe gets your list of games from Steam's Web API, with a key you register:

1. Sign in to the Steam client on this machine. The key lists the games of the account the client last signed in with.
2. On Settings › Stores › Steam, pick **Get an API key** in the **Sign-in** card. It shows Steam's key page, `steamcommunity.com/dev/apikey`, as a QR code and as an address.
3. Register a key there. Steam asks for a domain name, and any will do.
4. Pick **Enter the key** and enter it, all 32 hexadecimal characters.

[`universe login steam`](/docs/reference/cli/login/) prints the same address and asks for the code; give it the key. `universe login steam <key>` does it in one go. Steam lists no games for an account that keeps its game details private.

## Your Steam games

Settings › **Downloads** then lists every game the account owns. **On this PC** covers every Steam library folder on the machine, without Proton, Steam's runtimes or its redistributables. **Add to library** refuses a game someone shares with you through Steam, since Steam doesn't list it as yours.

## Installing, updating and uninstalling

The rows and menus of Settings › Downloads work as for every store ([Installing from the store](/docs/library/gog/#installing-from-the-store)), except where Steam does the work itself:

- **Install** needs no install folder. Universe starts the Steam client if needed and opens Steam's install window. Confirm the install there, and Universe follows the download until Steam reports the game installed. When nothing starts within **Install confirm time**, 600 seconds, the job stops.
- **Cancel install** only stops Universe watching: the download carries on in Steam, and Resume follows it again.
- An update is one Steam has pending for the game. **Update** waits for the client to run it; when Steam doesn't, Universe says so, and you start it from Steam's Downloads page.
- **Uninstall…** and **Delete files…** open Steam's uninstall dialog, and Steam removes the files once you confirm.

## Share Steam's prefix

With **Share Steam's prefix** on, the default, a Windows game runs in the prefix Steam made for it, `steamapps/compatdata/<appid>`, with the Proton build Steam picked for it, so Steam and Universe see the same saves. The game's Data and saves page names it **Steam's prefix**. Universe never moves it, and backs its saves up from where it is.

The setting applies to games added from then on. With it off, a new game gets a prefix of Universe's own and Universe's Proton build.

Two Proton builds on one prefix rewrite it for each other, and the first launch after a switch may quit. A game added before you changed Steam's Proton pick keeps the build it came with, which its Game settings change.

## Launching a Steam game

A game needs the Steam client running. If none is, Universe starts one in the background (`steam -silent`) and waits up to 70 seconds for it to sign in. A client that never starts cancels the launch; one that doesn't sign in lets the game start in Steam's offline mode.

Universe starts the game's program itself, never through `steam://rungameid/`, so the session keeps its unit, its hooks and its recording. Two kinds of games don't fit that:

- A game wrapped in SteamStub, Steam's DRM, restarts itself through Steam, outside Universe's session.
- A Linux build runs outside Steam's runtime, and one that needs the runtime's libraries doesn't start. On NixOS that's most of them.

**Achievements**, on by default, reads a game's achievements from the Steam client after each session.

## Steam Cloud

Steam Cloud syncs only around the sessions the Steam client launches, not Universe's. No cloud save comes down before a Universe session, and a save file the session created may not go up after it.

With Share Steam's prefix on, nothing is lost: the saves stay in Steam's compatdata, the client syncs them the next time it launches the game itself, and Universe's backups cover them meanwhile. With it off, the saves stay in Universe's prefix and never reach Steam Cloud; only the [backups](/docs/library/saves/) keep them.

## Steam's settings

On Settings › Stores › Steam, the **Settings** card holds Share Steam's prefix, and the **Game defaults** card holds Achievements, both on. **Y** shows three more: **Steam's folder**, Install confirm time (600 seconds) and **Install time limit**, which stops an install or update that runs longer than 14400 seconds.

Steam's folder left empty takes the first of `~/.steam/root`, `~/.local/share/Steam`, `~/.steam/steam` and `~/.steam/debian-installation` that has a `steamapps` folder.
