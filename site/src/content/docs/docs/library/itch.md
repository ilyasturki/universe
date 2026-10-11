---
title: itch.io
description: Sign in to itch.io with an API key, install your games and tools through butler, and take over the games the itch app installed.
sidebar:
  order: 4
---

Universe installs and updates what you own on itch.io through butler, itch.io's own tool, which it fetches the first time it needs it. It also takes over the games the itch app installed. The store is off until you turn it on. Press **Y** on **itch.io** in Settings › **Stores**, or run [`universe source enable itch`](/docs/reference/cli/source/).

## Signing in with an API key

1. On Settings › Stores › itch.io, pick **Get an API key** in the **Sign-in** card. It shows itch.io's API keys page, `itch.io/user/settings/api-keys`, as a QR code and as an address.
2. Generate a key there.
3. Pick **Enter the key** and enter it.

From a terminal, [`universe login itch`](/docs/reference/cli/login/) prints the address and asks for the code; give it the key. `universe login itch <key>` does it in one go.

## Your itch.io games

Universe lists the games and tools you own, bought or claimed. A bundle's games show up once you've claimed them on itch.io. Soundtracks, assets and books are left out.

## Installing and updating

The rows and menus of Settings › Downloads work as for every store ([Installing from the store](/docs/library/gog/#installing-from-the-store)), and butler picks what to install:

- A full build for Linux or Windows, never a demo or a preorder, and never a `.deb`, `.rpm`, `.pkg` or `.dmg` package. When a game has both, **Build to install when a game has both** decides, Linux by default. 64-bit builds and `.zip` files come first.
- A Linux build runs with the Linux runner, a Windows build through Proton.
- A browser game has nothing to download, so **Install** fails, saying the game has no Linux or Windows build.

A cancelled download resumes where it stopped. Updates are what butler calls direct updates, a newer build on the same channel, so a file the developer replaced under the same upload doesn't show as one. **Uninstall…** and **Delete files…** go through butler.

## The itch app's games

Each time Universe looks for itch.io installs on disk, it finds the games in the install folders the itch app's database lists, and leaves them where they are. The database is the one **Adopt installs from** names, `~/.config/itch/db/butler.db` by default. Those games then show under **On this PC**, ready for **Add to library**. The first run's **itch** row adds them all at once.

## No achievements or cloud saves

itch.io keeps neither, so the store's page has no **Game defaults** card.

## itch.io's settings

The **Settings** card holds the **Install folder**, where empty installs into the library's games folder, and Build to install when a game has both, **Linux** or **Windows**. **Y** shows **butler's database**, where empty keeps the store's own, Adopt installs from, and **Install time limit**, which stops an install or update that runs longer than 7200 seconds.
