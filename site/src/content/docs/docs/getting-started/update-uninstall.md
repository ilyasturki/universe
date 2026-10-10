---
title: Update and uninstall
description: Updating Universe on each install path, what it keeps current by itself, and removing it with or without your data.
sidebar:
  order: 5
---

Universe never checks for updates to itself. You update it the way you installed it, with that tool's usual command. Your library and settings live in your home folder, apart from the program, and an update leaves them alone.

## Updating

### Arch Linux

Each release reaches `universe` and `universe-bin` on the AUR, Universe Desktop's packages included, and your AUR helper's upgrade picks them up:

```sh
paru -Syu
```

`universe-git` builds `main`, which has no release number to compare. Ask paru to check development packages and it rebuilds whenever `main` has moved:

```sh
paru -Syu --devel
```

### Fedora

COPR builds `universe` and `universe-desktop` from each release:

```sh
sudo dnf upgrade
```

### The installer

Run it again:

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh
```

It replaces the previous install with the latest release, or with the one `--version` names, and reuses the Python environment it made the first time. Pass the same `--prefix` if you used one. It asks for your password only when one of its files outside your home folder is missing or changed. It ends with `universe setup`, which on GNOME refreshes the Shell extension; GNOME loads the new one at your next login.

From a checkout, pull and run it again:

```sh
git pull
./install.sh
```

### Nix

Update the flake input, then rebuild:

```sh
nix flake update universe
sudo nixos-rebuild switch --flake .
```

`nixos-rebuild` covers the NixOS module, and Home Manager too when it runs as a NixOS module. A standalone Home Manager needs `home-manager switch --flake .` as well.

## What Universe keeps current

Universe updates what it installed for your games: runner builds such as Proton, Wine and the emulators, and tools such as umu-launcher, gogdl, legendary and butler. Universe Big Screen looks for new builds once a day while it runs, the first time 90 seconds after it starts, and never during a game. The build before the newest one stays, so Settings › [Emulators & Proton](/docs/settings/emulators-proton/) can roll back an update that misbehaves. What came from your distribution, Steam, Lutris or Heroic is left as it is.

To update them by hand only, turn it off in [config.toml](/docs/reference/config/):

```sh
universe config set components.auto_update false
```

Add-ons update separately, with [`universe extension update`](/docs/reference/cli/extension/).

## What's new

The first time you open Universe after an update, **What's new** opens by itself with what changed in every release since the one you last ran. Whichever app you open first shows it, and the other then doesn't. The notes are built into the app, so they need no network. They don't open on a first start, or after going back to an older version.

Settings › About › **What's new** reopens them any time, every release newest first, and so does **Version**, the row above it. Turning off **Show what's new after an update**, in About and in Game defaults › Updates, keeps the page from opening by itself. Universe still notes each version you start, so turning the switch back on later shows only the releases that come after. Universe Desktop shows the same notes in a dialog, and its main menu has **What's New**.

## Uninstalling

Each path removes the program and leaves your data, listed under [What stays](/docs/getting-started/update-uninstall/#what-stays).

On Arch Linux, remove the packages you installed, with the `-bin` or `-git` names if those are yours:

```sh
sudo pacman -Rs universe universe-desktop
```

On Fedora, remove the packages, and the COPR repository if you want it gone too:

```sh
sudo dnf remove universe universe-desktop
sudo dnf copr remove ilyasturki/universe
```

With the installer, run it with `--uninstall`, and the same `--prefix` if you used one. From a checkout, that's `./install.sh --uninstall`.

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh -s -- --uninstall
```

It removes `~/.local/lib/universe` with its Python environment, and the commands, app entries and man pages it put under `~/.local`. Through sudo, it also removes the controller rules, its login session entry and the system tools helper with its policy. Without sudo, it names each of those files that stays for you to remove as root.

On Nix, delete the `programs.universe` lines, the module imports and the `universe` input, then rebuild. If you set `programs.universe.settings`, the config.toml that Home Manager wrote goes with them.

## What stays

Whatever the path, Universe's own folders stay where they are:

| Folder | What it holds |
| --- | --- |
| `~/.config/universe` | config.toml, and your own modules and stores |
| `~/.local/share/universe` | the library (each game's settings, artwork, sessions, journal and screenshots), store sign-ins, Wine prefixes, save backups, the runner builds and tools Universe installed, add-ons |
| `~/.local/state/universe` | the first-run and What's new markers, what each app remembers between runs, game logs |
| `~/.cache/universe` | caches Universe rebuilds when it needs them |

These follow `XDG_CONFIG_HOME` and the other XDG variables when you set them, and `UNIVERSE_CONFIG_HOME`, `UNIVERSE_DATA_HOME`, `UNIVERSE_STATE_HOME` and `UNIVERSE_CACHE_HOME` replace them.

The games Universe installed stay too, in `~/Games` unless you chose another folder, and so do your recordings, in `~/Videos/universe`. The installer's closing message names only `~/.config/universe` and `~/.local/share/universe`, but every folder above stays.

## Removing everything

:::caution
This deletes your play time, your save backups, your store sign-ins and the Wine prefixes, where many Windows games keep their saves. Cloud saves stay with their store.
:::

After uninstalling, delete Universe's four folders:

```sh
rm -rf ~/.config/universe ~/.local/share/universe ~/.local/state/universe ~/.cache/universe
```

Then, if you want them gone too:

- The games Universe installed, in `~/Games` or the folders you chose. Look before you delete, since another launcher may install into the same folder.
- Your recordings, in `~/Videos/universe`.
- On GNOME, the Shell extension that `universe setup` copied into your home folder, which `gnome-extensions uninstall universe@ilyasturki.github.io` removes.
- The Universe Big Screen shortcut in Steam, if you added it as a non-Steam game.
