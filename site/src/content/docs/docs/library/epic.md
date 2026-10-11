---
title: Epic Games
description: Sign in to Epic Games, install and update its games through legendary, take over Heroic's Epic installs, and what each Epic setting does.
sidebar:
  order: 3
---

Universe installs and updates your Epic Games library through legendary, which it fetches the first time it needs it. It takes Epic's Windows builds only, so every Epic game runs through Proton or Wine. The store is off until you turn it on. Press **Y** on **Epic Games** in Settings › **Stores**, or run [`universe source enable epic`](/docs/reference/cli/source/).

## Signing in

1. On Settings › Stores › Epic Games, pick **Get a sign-in link** in the **Sign-in** card, and open the link from its QR code or its address.
2. Sign in to Epic. You end on a page of text that holds an `authorizationCode`.
3. Pick **Enter the code** and enter that code, or the whole page.

A new code drops the session Universe had, which is also how you switch accounts. [`universe login epic`](/docs/reference/cli/login/) does the same from a terminal.

By default Universe shares legendary's own folder, `~/.config/legendary` (the **legendary's folder** setting). A legendary you run by hand sees the same sign-in and the same installs.

## Your Epic games

The first listing fetches every title's details, and can take up to 15 minutes. It leaves out the titles that install through another store, such as EA's or Ubisoft's, and those with no Windows build.

## Installing and updating

The rows and menus of Settings › Downloads work as for every store ([Installing from the store](/docs/library/gog/#installing-from-the-store)), and legendary does the work behind them:

- **Install** downloads the Windows build into the **Install folder**, with the DLCs you own while **Install owned DLCs** is on. A cancelled download resumes where it stopped.
- legendary tells which installed games are behind, and **Update** runs its update.
- **Uninstall…** and **Delete files…** go through legendary (**Uninstall through Epic Games**), which removes the files and its record of the install.

## Heroic's Epic games

Heroic installs Epic games with a legendary of its own. Each time Universe looks for Epic installs on disk (opening Settings › Downloads on Epic Games, a scan, the first run's **Heroic · Epic Games** row), it imports Heroic's into its own legendary from the folder **Adopt installs from** names, Heroic's by default. They then show under **On this PC**, ready for **Add to library**.

The import needs you signed in to Epic; a failed one is tried again an hour later. Such a game gets a prefix of Universe's own ([Lutris and Heroic](/docs/library/lutris-heroic/#heroic)).

## Launching an Epic game

Before each launch, Universe gets the game's launch arguments from legendary, with a one-time sign-in to Epic, or the offline ones when offline, so a game that plays offline still starts.

For a game that ships Easy Anti-Cheat or BattlEye, **Anti-cheat runtimes** fetches the runtime Proton needs from Lutris.

## Cloud saves and achievements

The **Game defaults** card of Epic's page sets two settings for every Epic game, and a game's Game settings › **Epic Games** card for that one:

- **Achievements**, on by default. They're fetched from Epic after each session, so an unlock shows once the session has ended.
- **Cloud saves**, off by default ([Saves and cloud saves](/docs/library/saves/#cloud-saves)).

## Epic's settings

The **Settings** card of Settings › Stores › Epic Games holds the Install folder, where empty installs into the library's games folder, and Install owned DLCs, on by default.

**Y** shows the rest: Anti-cheat runtimes, on; legendary's folder, `~/.config/legendary`; Adopt installs from, Heroic's `~/.config/heroic/legendaryConfig/legendary`; and **Install time limit**, which stops an install or update that runs longer than 7200 seconds.
