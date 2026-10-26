---
title: Install
description: Install Universe on Arch, Fedora, SteamOS, NixOS or any other distribution.
sidebar:
  order: 1
---

Universe runs on x86_64 Linux with systemd 250 or newer, on cgroups v2. You get two apps on the same library: **Universe Big Screen**, which you drive with a controller, and **Universe Desktop**, for the mouse and the keyboard.

Install gamescope too. Without it, games open on your desktop, there's no dock over a game, and the [Universe session](/docs/getting-started/universe-session/) can't start.

## Arch Linux

From the AUR: `universe` builds the latest release, `universe-bin` installs it prebuilt, and `universe-git` builds `main`. Add `universe-desktop` (or its `-bin` or `-git`) for Universe Desktop.

```sh
paru -S universe universe-desktop
```

gamescope, MangoHud, gpu-screen-recorder and `game-devices-udev` are optional dependencies: install the ones you want.

## Fedora

Fedora 44 and newer, from COPR:

```sh
sudo dnf copr enable ilyasturki/universe
sudo dnf install universe universe-desktop
```

## SteamOS, Bazzite and other distributions

The installer puts Universe in `~/.local`. It needs Python 3.11 to 3.14, and SDL3 for your controller: SteamOS 3.8 and Bazzite have it, Ubuntu 24.04 and older don't.

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh
```

It asks for your password once, to set up your controllers and the **Universe** entry on the login screen. On a Steam Deck, [set a password first](/docs/getting-started/steam-deck/#installing-on-steamos).

From a git checkout, `./install.sh` builds it instead, with cargo.

## Nix

The flake has a NixOS module for the system side and a Home Manager module for the apps:

```nix
# flake.nix
inputs.universe.url = "github:ilyasturki/universe";

# NixOS configuration
imports = [ inputs.universe.nixosModules.default ];
programs.universe.enable = true;
programs.universe.session.enable = true; # optional: the Universe login session

# Home Manager configuration
imports = [ inputs.universe.homeModules.default ];
programs.universe.enable = true;
```

With `programs.universe.settings` set, your settings come from Nix, and the app can't change them. [Nix options](/docs/reference/nix/) lists every option.

## After installing

On GNOME, run this once, then log out and back in. The installer runs it for you; the Arch and Fedora packages don't.

```sh
universe setup
```

It installs and turns on the Universe Shell extension, which screenshots, window recording and the on-screen messages use.

Then open **Universe Big Screen** from your app menu. On an empty library, the [first run](/docs/getting-started/first-run/) finds your games.
