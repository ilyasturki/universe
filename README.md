<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/universe-lockup-dark.svg">
    <img src="brand/universe-lockup.svg" alt="Universe" width="360">
  </picture>
</p>

<p align="center"><b>A game launcher for Linux you drive with a controller.</b><br>Your GOG, Steam, Epic, itch.io and emulated games in one library, played from the couch or on a Steam Deck.</p>

<p align="center"><a href="#install">Install</a> · <a href="https://universelauncher.com/docs/">Docs</a> · <a href="CHANGELOG.md">Changelog</a></p>

<p align="center"><img src="docs/images/home.avif" alt="Moving along the home row, through the library and a game page, searching for Dead Cells, playing it, then quitting from HOME" width="100%"></p>

> [!WARNING]
> Early days (0.0.x). I use it every day, but expect rough edges. [Issues](https://github.com/ilyasturki/universe/issues) are welcome.

## Install

x86_64 Linux with a systemd user session.

```sh
# Arch (AUR). Add -bin to both names for the prebuilt build, -git to follow main.
paru -S universe universe-desktop

# Fedora 44 and newer (COPR)
sudo dnf copr enable ilyasturki/universe
sudo dnf install universe universe-desktop

# SteamOS, Bazzite and others: under ~/.local, with Python 3.11 to 3.14
curl -fsSL https://github.com/ilyasturki/universe/releases/latest/download/install.sh | sh

# From a checkout, with cargo
./install.sh
```

On Nix, import the flake's `nixosModules.default` for the system and `homeModules.default` for the apps, each with `programs.universe.enable = true`.

On GNOME, run `universe setup` once after the Arch or Fedora package. Then open **Universe Big Screen**, and a short setup finds your games.

More: [every way to install](https://universelauncher.com/docs/getting-started/install/), [updating and uninstalling](https://universelauncher.com/docs/getting-started/update-uninstall/).

## Your whole library

GOG, Steam, Epic and itch.io games, installed and updated from Universe. The first run brings in the games Lutris and Heroic installed and your emulators' ROM folders, and moves nothing.

![The library grid](https://raw.githubusercontent.com/ilyasturki/universe/site-media/src/assets/media/library.webp)

## A page for every game

Art, screenshots, play time, achievements, saves and disk use. Its settings are one button away: runner, Proton build, resolution, frame cap.

![A game's page](https://raw.githubusercontent.com/ilyasturki/universe/site-media/src/assets/media/game-page.webp)

## Three looks

Switch live from the settings, or install a theme as an add-on.

<p align="center">
  <img src="https://raw.githubusercontent.com/ilyasturki/universe/site-media/src/assets/media/look-universe.webp" alt="The Universe look" width="32%">
  <img src="https://raw.githubusercontent.com/ilyasturki/universe/site-media/src/assets/media/look-switch2.webp" alt="The Switch 2 look" width="32%">
  <img src="https://raw.githubusercontent.com/ilyasturki/universe/site-media/src/assets/media/look-ps5.webp" alt="The PS5 look" width="32%">
</p>

- **Universe**, the default look, is a port of my [Pegasus theme Reprise](https://github.com/ilyasturki/pegasus-theme-reprise), with the game's art behind everything.
- **Switch 2** puts a row of games over a bar of round buttons, like the Switch 2's HOME menu.
- **PS5** puts the game's world behind a row of tiles, with a hub for each game, like the PS5's home.

<sub>Not affiliated with Nintendo or Sony. Nintendo Switch is a trademark of Nintendo; PlayStation and PS5 are trademarks of Sony Interactive Entertainment.</sub>

## HOME over a running game

HOME pauses the game and opens over it. From there, take a screenshot, turn on MangoHud, cap the frame rate, change the volume, reconnect a controller or quit.

![Dead Cells running, then HOME over it: the dock, MangoHud turned on, back in the game and the volume level](docs/images/home-over-game.avif)

## Couch and Steam Deck

- DualSense, DualShock 4, Xbox, Switch Pro and 8BitDo [controllers](https://universelauncher.com/docs/controllers/supported/), each with its own glyphs, and a guided setup for any other pad. Emulators get the pads you hold, one per player.
- On a [Steam Deck](https://universelauncher.com/docs/getting-started/steam-deck/), it fits the 1280×800 screen. Back buttons run macros; refresh rate, power limit and fan curve are in the settings.
- Added as a non-Steam game, it runs inside [Steam's Game Mode](https://universelauncher.com/docs/getting-started/steam-deck/#steams-game-mode).
- [The Universe session](https://universelauncher.com/docs/getting-started/universe-session/) is a login session with the launcher on the whole screen and no desktop, like a console.

## Also

- Record every session if you like. Videos and screenshots land in the Media tab.
- Achievements from each store, with a banner over the game on unlock.
- Saves backed up after every session, restored or exported from the game's page. GOG and Epic cloud saves synced.
- Proton, Wine and 21 emulators, installed, updated and rolled back from the settings.
- Modules, stores and themes made by others, installed from the settings as add-ons.
- Hold B for the power menu: suspend, reboot or power off.
- Runs on GNOME, KDE, Cinnamon, Sway, Hyprland, Niri, X11, or no desktop at all.
- Universe Desktop, a GTK app for mouse and keyboard on the same library. GNOME's search finds your games.

## From a terminal

`universe` does everything the apps do. Add `--json` to any command.

```sh
universe                            # your library, last played first
universe play "dead cells"          # by id, a word or part of the title
universe login gog                  # sign in to a store
universe search hades --source gog  # then universe install <id> --source gog
universe update                     # games behind, of every kind; --all shows each version
universe saves backup "dead cells"  # also list, restore, export and cloud
universe doctor                     # what this system is missing
universe status --json              # the running game, for a script
```

Every command is in the [CLI reference](https://universelauncher.com/docs/reference/cli/).

## Docs

[universelauncher.com/docs](https://universelauncher.com/docs/) has the guide: install, first run, controllers, every settings section, and a reference of the CLI, the launch settings and config.toml generated from the app.

For development:

- [`docs/api.md`](docs/api.md): the core, the CLI, config, modules and sources
- [`docs/frontends.md`](docs/frontends.md): how the UI is built, and how to write another frontend
- [`docs/extensions.md`](docs/extensions.md): modules, sources and themes that do not ship with Universe, and how to install them
- [`CHANGELOG.md`](CHANGELOG.md): what each release changed, also under Settings › About in both apps

## License

[MIT](LICENSE)
