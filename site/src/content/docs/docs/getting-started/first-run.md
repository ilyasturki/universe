---
title: First run
description: The setup that opens on an empty library, step by step, and how to skip it or run it again.
sidebar:
  order: 2
---

The first time you open Universe Big Screen with nothing in your library, a setup opens over the home screen once the startup animation ends. It brings in the games other launchers already installed, signs you in to your stores, asks where new games go and installs the runners those games need. None of it is required.

It runs once per user, and Universe Desktop shares it. Finishing or skipping it in either app writes `~/.local/state/universe/onboarded`, and after that neither app opens it by itself. A library that already holds games writes the same file without showing anything. Delete the file and the setup comes back at the next start, as long as the library is still empty.

## Moving through it

Steps with nothing to do on your machine are left out, so the counter above each title (`2 / 5`) varies from one PC to the next.

- **X**, or **A** on **Continue**, goes to the next step. On the last step the button reads **Finish**.
- **B**, or **A** on **Back**, goes back a step.
- On the first step that button reads **Skip setup** and asks **Skip setup?** before closing. Once some games came in, or when you run the setup again, it reads **Close** and closes without asking.

Imports run in the background. You can move on, and even finish, while a launcher's games are still coming in.

## The steps

1. **Connect to the internet** comes first when the machine is offline and NetworkManager has a Wi-Fi device. Press **A** on your network's **Connect**, and type its password if it asks for one. On a cable, plug it in and continue.
2. **What's on this machine** lists the games other launchers installed here: Lutris, Steam, Heroic's GOG, Epic Games and Amazon Games libraries, the itch app, and **Emulator folders**, the games your emulators list. The Flatpak versions of Steam, Heroic and itch count too. **Add** on a launcher brings its games into the library, and **Add everything** takes them all. Nothing moves on disk. Lutris games keep their play time, while Steam, GOG, Epic Games and itch games play and update through the matching store in [Settings › Stores](/docs/settings/stores/). Amazon games can't come in, since they launch through Heroic. Under **Runners your games need**, **Install** fetches the emulators and Proton builds the new games run on.
3. **Your stores** shows up when a store is signed out. GOG is the only store on at first, and a store whose launcher turned up on the previous step gets a switch such as **Use Epic Games**. GOG and Epic Games sign you in through a link. **Get a sign-in link** shows it with a QR code for your phone, and **Enter the code** takes the code the page gives you. itch.io and Steam want an API key instead, through **Get an API key** and **Enter the key**. The [GOG](/docs/library/gog/), [Epic Games](/docs/library/epic/), [itch.io](/docs/library/itch/) and [Steam](/docs/library/steam/) pages go through each one.
4. **Where games install** sets the folder new installs go to, `~/Games` by default. The folders Heroic and Lutris install into are offered beside it with **Use**, and **Another folder…** picks any other. Games already installed stay where they are.
5. **Where game data goes** sets **Windows game folders** (`~/.local/share/universe/prefixes`), where each Windows game gets a Wine prefix of its own, and **Save backups** (`~/.local/share/universe/saves`). **Back up saves after playing**, on by default, backs up a game's saves each time you stop playing it.
6. **Controller and screen** has **Buttons and glyphs**, the controller whose buttons the hints and the controller art show until you plug one in, and **HDR**, for games that support it on an HDR screen.
7. **You're set** sums up the games that came in and the stores you signed in to. Games still importing keep arriving on Home after you finish.

Steps 4 to 6 need a config.toml that Universe can write. When Home Manager writes it (`programs.universe.settings`), the setup leaves them out.

## Running it again

Settings › About › **First-run setup** opens it again with **Run again**. On an empty library, the home screen also has a **Set up** button next to **Add a game**.

The Switch 2 and PS5 looks run the same steps, and their empty home screens have the same two buttons (**Set Up** and **Add a Game** in the PS5 look).

## A controller Universe hasn't set up

Apart from this setup, the first time you connect a controller of a model Universe knows but has nothing set up for, it offers to set up its buttons ([Button setup](/docs/controllers/button-setup/)).

## Universe Desktop

Universe Desktop opens its own version on the same terms, a **Welcome** dialog with the same steps except the Wi-Fi one, ending on **Start Playing**. Closing it before any game came in asks **Skip Setup?**, and once skipped it stays closed. **First-Run Setup…** in Add Games (<kbd>Ctrl</kbd>+<kbd>N</kbd>) brings it back.
