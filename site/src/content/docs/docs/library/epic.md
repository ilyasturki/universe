---
title: Epic Games
description: Sign in to Epic Games, install its games, and take over Heroic's Epic installs.
sidebar:
  order: 3
---

Universe installs and updates your Epic Games library itself. It takes Epic's Windows builds, so every Epic game runs through Proton or Wine. The store is off until you press **Y** on **Epic Games** in Settings › **Stores**.

## Signing in

1. On Settings › Stores › Epic Games, pick **Get a sign-in link**, and open it from its QR code or its address.
2. Sign in to Epic. You end on a page of text that holds an `authorizationCode`.
3. Pick **Enter the code** and enter that code, or the whole page.

Signing in again with another account switches to it.

## Your Epic games

The first listing can take up to 15 minutes. It leaves out the games that install through another store, such as EA's or Ubisoft's.

Installing, updating and uninstalling work as for [every store](/docs/library/gog/#installing-from-the-store). **Install owned DLCs**, on by default, installs the DLCs you own with each game.

## Heroic's Epic games

Once you're signed in, the Epic games Heroic installed show under **On this PC**, ready for **Add to library**. Universe updates them from then on. Each one gets a new Wine prefix, so its saves in Heroic's prefix stay there ([Lutris and Heroic](/docs/library/lutris-heroic/#heroic)).

## Cloud saves and achievements

Epic's page in Settings › Stores turns both on or off for every Epic game, and a game's **Game settings** › **Epic Games** card for that game alone. **Achievements**, on by default, show once a session has ended. [Cloud saves](/docs/library/saves/#cloud-saves) are off by default.

A game with Easy Anti-Cheat or BattlEye gets the runtime Proton needs for it, while **Anti-cheat runtimes** is on.
