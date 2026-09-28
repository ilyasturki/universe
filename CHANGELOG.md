# Changelog

## [0.0.7]

### Breaking

- Art you picked for a game now lives in `games/<id>/media/picked/` and `paths.overrides` is gone. Picks under the old `~/.config/universe/overrides/<id>/` are no longer read: move them over by hand, or pick them again.
- The `[lutris]` config section is gone. `universe migrate` reads Lutris's own folders, and no longer copies art from a Pegasus library.
- The journal stays as entries in each game's folder: the Markdown note export, `paths.journal_root`, `universe journal --render` and the `UNIVERSE_JOURNAL_ROOT` hook variable are gone. Notes already written to `~/Documents/universe/journal` are left alone.
- InputPlumber is no longer used: the controls module sets up each emulator's pads itself. The NixOS option `programs.universe.inputplumber.enable` is removed and emulator runners lose their `inputplumber` option.
- `[proton]` ships with no entries. A Proton build that isn't found is downloaded by umu instead of failing the launch.
- The GNOME Shell extension supports GNOME 45 to 50 and hides the pointer itself: `cursor_extension` now defaults to empty, set it to another extension's uuid to keep using that one.

### Added

- A third look, PS5: home row and game hub, library, store and downloads, search, media and journal, trophies, a control center over a running game, settings and controller setup.
- Universe Desktop, a GTK app for mouse and keyboard: library, game pages, store, add games, game settings and preferences, controller setup with live pad art, artwork, media, journal and achievements. It adds a GNOME Shell search provider for your games and an "Open Big Screen" action.
- Steam, Epic (through legendary) and itch.io (through butler) stores next to GOG, several at once, a game from two stores kept as two games. Steam games run through Proton. Onboarding adopts the Steam, Epic and itch installs it finds, and uninstalling goes through the store when it removes the files.
- Achievements: a page per game and unlock banners in game, GOG's through its Galaxy service. `universe achievements --replay` shows unlocks again.
- Components: runner builds, the tools Universe needs (umu-run, gogdl, legendary, butler) and distribution packages such as gamescope and MangoHud are installed, updated, rolled back and uninstalled from settings, and offered before a launch that needs one.
- Works on KDE, Cinnamon, Sway, Hyprland, Niri and X11 besides GNOME: game windows, on-screen volume, screenshots, cursor hiding, keyboard layout and screen mode.
- Steam Deck: its controller with glyphs and pad art, a 16:10 layout, runs in Steam's Game Mode, and power limit, GPU clock and fan controls.
- Touch on the Reprise and Switch 2 looks.
- A power menu (hold B): suspend, reboot, power off.
- Each emulator gets one virtual pad per player, with its button bindings written before launch; a shoulders setting swaps bumpers and triggers, and a Wiimote setting holds the remote per game.
- A hybrid laptop's games can run on its discrete GPU.
- Screenshots and the volume level work inside gamescope.
- "Apply to all games" on a game's settings.
- Switch 2 look: keyboard sheet, launch and page transitions, per-action sounds with a folder to replace them, Wi-Fi and battery in the top bar.
- Settings mark changed values and name where each one comes from, with a reset.
- Doctor fixes are worded for your distribution.
- Packages: `universe`, `universe-bin` and `universe-git` on the AUR, each with a `universe-desktop` counterpart; a release tarball with `install.sh`; a `universe-fhs` environment and a Home Manager `desktop` option on Nix.

### Changed

- Keep-awake also holds a logind idle lock.
- The launcher falls back to the desktop when gamescope fails to start.
- MangoHud's frame limit works without uinput, and the HUD without mangoapp installed.
- Settings are found from the tab bar search; the Reprise, Switch 2 and add-a-game pages are trimmed.

### Fixed

- The UI restarts in place instead of freezing when its event loop gets stuck.
- `universe rm --purge` trashes a game's default Wine prefix too.
- Steam's copy of a real pad no longer shows as a second controller.
- Dolphin players past a game's own profile get a profile too.
- Azahar ROM folders are read from azahar-emu's config.
- Games show under Steam's Game Mode and the launcher survives its preload.
- GOG achievement icons are full size.
- Uruntime AppImages unpack, and the runner environment adds the libraries they miss.
- Switch 2 and PS5 dialogs scroll long text and elide long titles.
- AUR packages: the GOG Galaxy service stub ships, dependencies and versions pinned.

### Performance

- The PS5 home loads a game's hub ahead of the hero fade.

### Removed

- Reprise favourites.
