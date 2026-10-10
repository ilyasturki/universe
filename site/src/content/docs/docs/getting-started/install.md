---
title: Install
description: Every way to install Universe, and what each one puts on your system.
sidebar:
  order: 1
---

Universe runs on x86_64 Linux with a systemd user session (systemd 250 or newer, on cgroups v2). gamescope is optional but on by default: without it, games open on your desktop. The Universe login session needs it.

You get two apps. **Universe Big Screen** is the one you drive with a controller. **Universe Desktop** is a GTK app for mouse and keyboard on the same library. Both come with the `universe` command.

## Arch Linux

Three packages on the AUR:

- `universe` builds the latest release.
- `universe-bin` installs its prebuilt build.
- `universe-git` builds `main`.

`universe-desktop`, `universe-desktop-bin` and `universe-desktop-git` add Universe Desktop to each.

```sh
paru -S universe
```

gamescope, MangoHud, gpu-screen-recorder and the controller rules (`game-devices-udev`) are optional dependencies: install the ones you want. The store tools (umu-launcher, gogdl, legendary, butler, ludusavi) are fetched on first use when they're missing.

## Fedora

Fedora 44 and newer, from COPR. Both packages are built from each release against Fedora's PySide6, GTK and libadwaita.

```sh
sudo dnf copr enable ilyasturki/universe
sudo dnf install universe universe-desktop
```

## SteamOS, Bazzite and other distributions

The installer puts the latest release under `~/.local`, for your user:

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh
```

It needs Python 3.11 to 3.14 (on Fedora 45, whose `python3` is 3.15, run `sudo dnf install python3.14` first). The first install downloads PySide6, which is large.

It then asks for your password once, for three things that live outside your home folder:

- the controller rules, so the virtual pads and the key macros can open `/dev/uhid` and `/dev/uinput`;
- the **Universe** entry on the login screen;
- the helper that installs gamescope, MangoHud and gpu-screen-recorder from inside that session, with no password prompt.

On a Steam Deck, set a password with `passwd` first. Without sudo, the install still finishes and prints the commands to run as root.

Universe Desktop needs GTK 4.22 and libadwaita 1.9. Where the system's are older, the installer leaves it out and says so.

Two flags: `--prefix DIR` installs somewhere other than `~/.local`, and `--version vX.Y.Z` installs that release. Pass them after `sh -s --`:

```sh
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh -s -- --version v0.0.11
```

## From a checkout

The same installer builds a git checkout. It needs cargo, and the network for the build's downloads:

```sh
git clone https://github.com/ilyasturki/universe
cd universe
./install.sh
```

Universe Desktop is built when GTK 4.22, libadwaita 1.9 and `blueprint-compiler` are there.

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

The NixOS module sets up gpu-screen-recorder for recordings, `/dev/uinput` and `/dev/uhid` with the controller rules, and gamescope. The Home Manager module installs the CLI, both apps and the GNOME Shell extension. Set `programs.universe.settings` and your `config.toml` comes from Nix: the settings pages can then read it but not change it.

Every option is listed in [Nix options](/docs/reference/nix/).

## After installing

On GNOME, run this once, then log out and back in:

```sh
universe setup
```

It installs and turns on the Universe Shell extension, which window recording, screenshots and the on-screen messages use, then checks your system. The installer runs it for you; the Arch and Fedora packages don't. Universe Desktop has the same step under System Check.

Then open **Universe Big Screen** from your app menu. With an empty library, a short [first-run setup](/docs/getting-started/first-run/) finds your games.
