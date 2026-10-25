---
title: Update and uninstall
description: Update Universe, or remove it with or without your data.
sidebar:
  order: 5
---

Universe doesn't update itself: update it the way you installed it. An update keeps your library and settings.

## Updating

On Arch Linux, your AUR helper picks up each release. `universe-git` needs `--devel`:

```sh
paru -Syu
```

On Fedora:

```sh
sudo dnf upgrade
```

With the installer, run it again:

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh
```

On Nix, update the flake input, then rebuild:

```sh
nix flake update universe
sudo nixos-rebuild switch --flake .
```

## What Universe keeps current

Universe updates the runners and tools it installed for your games, such as Proton builds and emulators, once a day while Big Screen runs, never during a game. Settings › [Emulators & Proton](/docs/settings/emulators-proton/) can roll back an update that misbehaves. Add-ons [update separately](/docs/add-ons/installing/).

## What's new

The first time you open Universe after an update, **What's new** shows what changed since the version you last ran. In the Universe look it opens as a window over Home, and **B** closes it. Settings › [About](/docs/settings/about/) opens it again, and its **Show what's new after an update** stops it opening by itself.

## Uninstalling

Each way leaves your data, listed under [What stays](/docs/getting-started/update-uninstall/#what-stays).

```sh
sudo pacman -Rs universe universe-desktop        # Arch Linux
sudo dnf remove universe universe-desktop        # Fedora
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh -s -- --uninstall
```

On Nix, delete the `programs.universe` lines, the module imports and the `universe` input, then rebuild.

## What stays

- `~/.config/universe`: your settings.
- `~/.local/share/universe`: the library, with play time, save backups, store sign-ins and Wine prefixes.
- `~/.local/state/universe` and `~/.cache/universe`: logs and caches.
- The games Universe installed, in `~/Games` unless you chose another folder, and your recordings, in `~/Videos/universe`.

## Removing everything

:::caution
This deletes your play time, your save backups, your store sign-ins and the Wine prefixes, where many Windows games keep their saves. Cloud saves stay with their store.
:::

After uninstalling, delete Universe's folders:

```sh
rm -rf ~/.config/universe ~/.local/share/universe ~/.local/state/universe ~/.cache/universe
```

Delete the games and recordings too if you want them gone, but look first: another launcher may install into the same games folder. On GNOME, `gnome-extensions uninstall universe@ilyasturki.github.io` removes the Shell extension.
