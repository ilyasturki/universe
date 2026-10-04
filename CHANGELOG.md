# Changelog

## [0.0.10] - 2026-10-04

### Breaking

- RAWG is no longer used: `keys.rawg`, `keys.rawg_file` and a game's `metadata.rawg_id` are ignored.

### Added

- A Universe session on the login screen: the launcher alone on the screen, like SteamOS's Game Mode, and Log out returns to the login screen. The AUR and Fedora packages and `install.sh` install it; on NixOS, `programs.universe.session.enable`.
- Save backups through ludusavi: after each session (`[saves] auto_backup`, `keep`), before a purge or a prefix reset, and on demand, with restore and export to a zip.
- A game's data on its page: install, Wine prefix, saves, recordings and logs with their sizes, a prefix moved under `prefixes_root` or reset, winecfg and winetricks in it.
- A storage page: free space per folder, games by size, and leftovers of removed games to trash. `universe data`, `universe storage` and `universe saves` do the same from the CLI.
- Settings › About shows the changelog in every look and in Universe Desktop; with `desktop.whats_new` on, the first start after an update opens what changed.
- A startup animation of the Universe mark with a quiet chime, then the look's home; Settings › Themes turns it off.
- HOME pressed while another app has the focus brings Universe up; `controller.home_summons = false` drops the press instead.
- A game's setting can go to every game it reaches (all games, all Dolphin games, all Proton and Wine games) from its page.
- Settings › Runners offers the system tools an enabled module needs, such as gpu-screen-recorder for recordings.
- Fedora 44 and newer: `universe` and `universe-desktop` from the `ilyasturki/universe` COPR.
- Installing Eden shows a notice and asks first; `universe component install` takes `--yes` to accept it.
- Settings › Themes says the looks are not affiliated with Nintendo or Sony.

### Changed

- Art and details need no API key: they come from the game's store, Steam, GOG GamesDB and libretro, and a scan, install or Lutris import fetches them. A SteamGridDB key adds its art to the picker, and `keys.prefer_sgdb` puts it first.
- First run happens once per machine, in either app: one row adds every game other launchers hold, then it asks where games, prefixes and save backups go, offering other launchers' folders. Universe Desktop's installs the runners your games need.
- Quick settings changed during a game (brightness, refresh rate, power limit, GPU clock, fan) are that game's and come back at its next launch; Y gives them to every game.
- While another app has the focus, the pad, held B and HOME leave it alone, and the looks pause their animations and videos.
- Settings › Runners installs, updates and switches each runner's builds, in every look and in Universe Desktop, in place of a separate components list.
- Module settings say what they do under their name, a game's page shows only the ones that apply to it, and the settings search finds them by their own keywords.
- A module or source made for another Universe version is unavailable, and `universe doctor` says which version it needs.
- The achievements pages are one compact list in every look, hidden locked achievements folded into a last row.
- The AUR and Fedora packages and `install.sh` install the controller udev rule and load uhid at boot; `install.sh` asks for sudo once.
- The GNOME Shell extension supports GNOME 45 to 51.

### Fixed

- `universe rm --purge` keeps a Wine prefix another library game shares.
- On a screen of its own, gamescope drives the display it read the mode from, and hides the idle cursor.
- A store scan counts only the games it added, not the ones it already had.
- The Proton builds card offers every Proton found as the default.
- Universe Desktop's runners page keeps its scroll and focus while builds install.
- A read-only `config.toml` is put down to Home Manager only when it links into the Nix store.
- A system where the HTTPS client cannot start gets an error instead of a crash.
- A recording frame cut off by leaving its page or quitting is extracted again, no longer left blank.

### Performance

- Emulator games are added first on a rescan, and their art fetched after.

## [0.0.9] - 2026-09-30

### Added

- `universe doctor` runs the checks a module brings through its new `check` hook. The journal's checks that Codex is signed in when it writes with Codex.

### Changed

- With Advanced hidden, a settings card still shows the advanced settings you changed on it.

### Fixed

- A journal entry that finds Codex signed out waits for `codex login` and is tried again every 15 minutes, without using up one of its tries.
- Dolphin: a port past your connected pads that was left bound to one is unplugged, so one pad no longer drives two players.
- Recording thumbnails are decoded on the strongest GPU that has a hardware decoder, not always the first render node, and in software when there is none.

## [0.0.8] - 2026-09-28

### Fixed

- The prebuilt release (`universe-x86_64-linux.tar.gz`) and the `universe-bin` AUR package, both missing from 0.0.7.

## [0.0.7] - 2026-09-28

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

## [0.0.6] - 2026-09-24

### Breaking

- Screenshots come from a new `screenshot` module instead of capture, and with modules opt-in nothing takes them until you turn it on with `universe module enable screenshot` or in Settings › Modules. It grabs the game's window alone by default, and its own per-game `window` and `cursor` settings replace capture's `source` and `cursor` for screenshots.
- The journal writes nothing until it has a writing model: set `provider` with `universe module set journal provider=codex` (or `openai`), or pick it on the journal's page, which takes you there when you turn the journal on. `universe doctor` says it is waiting. `model` is empty by default, so Codex uses its own default model instead of `gpt-5.6-sol`.
- `universe info --json` puts a game's merged module settings under `effective.modules`. The top-level `modules` is now the game's own `game.toml` table.

### Added

- The journal writes through any OpenAI-compatible endpoint: `provider = "openai"` with `base_url` and `api_key` or `api_key_file`.
- Journal settings for web lookups, reasoning effort, tries per session, the request timeout, the frame size and a `prompt_file` that replaces the built-in prompt.
- `universe journal <name> --write <session>` writes an entry on the spot for a session that has none or whose entry failed, and `--force` writes a written one again. The journal page lists every session, with Write the entry, Try again now and Write it again.
- Settings › Sound in both looks, the dock's Output row and `universe output` pick the audio output.
- A running game keeps the desktop awake, so a session played on the pad alone no longer blanks or suspends. `desktop.keep_awake` turns it off.
- Your keyboard layout reaches gamescope, so AZERTY and other layouts type right inside it, and the on-screen keyboards follow it. The UI needs the `xkbcommon` Python package.
- Mouse and keyboard in the Reprise look: hovering brightens, a click picks then presses, a right click is back, the wheel and a touchpad scroll, the keyboard types in text sheets, and the hints show keys once a keyboard or mouse was used last.
- A button walk sets up a pad's buttons and sticks one at a time, with a step back. It is offered the first time a pad with nothing learned connects, and Settings › Controller › Set up the buttons starts it again.
- Battery levels for 8BitDo pads and Bluetooth LE pads, and the battery badge tints the pad in use.
- The controller test screen draws each pad's own outline and shows the sticks' and triggers' readings and the last inputs.
- A store install under way heads the home row in both looks with its progress and turns into the game when it ends. On the store page an installed game's menu offers Play and Details.
- HOME during a launch raises the dock over the poster with Home and Quit, even before the game's session has started, and the dock gains a Sharpness row.
- Reprise shows a scrollbar on a journal entry, a game's details, a session log and settings cards.

### Changed

- A journal entry that fails for a passing reason (a quota, a timeout, an unmounted recording) is put off and tried again after 15 minutes, 1 hour, then 4 hours, or once the quota lifts. Entries are retried one at a time and never while a game runs, and a later try reuses the frames and the answer an earlier one got.
- The journal needs `codex` installed only when Codex is its writing model.
- Game settings show a sidebar of cards beside the picked card's rows in both looks, and module, source and runner pages are laid out the same way.
- Y shows the advanced settings on the settings pages, folded into the cards they belong to, instead of an Advanced row. X on a value set on the game or the runner resets it so it inherits again, and no longer opens the game's details.
- A value set on the game, the runner or globally is tagged THIS GAME, THIS RUNNER or GLOBAL, and a default reads plain.
- A setting on `auto` shows what it comes to here (`auto · 144`). On a game's page, the choice that clears the game's own value names the global value it falls back to.
- Environment variables and DLL overrides are one row each, and Add a variable takes the name and the value in one sheet.
- Notices queue and show one at a time, in Reprise's hint bar, the dock's status row and the Switch 2 top corner. A failure is marked red and stays longer.
- `universe doctor` and the Doctor page give each check a plain name, list the failing ones first and say how to fix each.
- Media tab cards show their kind: a screenshot whole, a recording with a play disc and its length, a journal entry by its title and opening lines.
- Recent games count a game added or installed as well as one played, and reinstalling a removed game from its store brings it back.
- In Reprise, focus rings sit a gap off the art and captions clear them, and the home rail holds the last twelve games, with the Library tile holding the rest.
- A screenshot taken with no game running uses the screenshot module's global settings, and a failed one says why.

### Fixed

- A launch and its recording go to a screen that is being drawn on, not one whose cable is in but whose monitor is off, and a recording moves off a monitor turned off mid-session.
- A recorder that fails to start says "Recording failed" on screen.
- 8BitDo Pro 3 in D-input mode: its lettered buttons, L4, R4 and back paddles map correctly, its sticks and triggers are told apart, and its right-hand A confirms.
- The text caret sits at the insertion point in both looks.
- A pad button sent as a key with no keysym no longer switches the hints to the keyboard's.
- The UI quits cleanly when its X display goes away, instead of crashing on the way out.
- The UI plays its sounds through PulseAudio, because Qt's PipeWire backend could freeze it for good.
- The Nix package's desktop entry claims gamescope's window class, so the GNOME overview shows Universe's icon for the fullscreen launcher.

## [0.0.5] - 2026-09-21

### Added

- `universe logs <name>` prints what a game's session wrote, and each game's Sessions page (Reprise: the Sessions pill on its details; Switch 2: Software Options › Play Log) lists its sessions and opens their logs. `universe doctor` warns when a reboot would drop them.
- Each session records how it ended (quit, stopped, crashed or killed), shown by `universe sessions`, both looks and the end-of-session toast.
- `launch.debug_log` keeps Proton's, Wine's and DXVK's full logs for each session of a Proton or Wine game.
- An empty home offers Set up beside Add a game, which opens the first-run setup again.
- In Reprise, X on a game's settings opens its details, and B brings the settings back on the same row.

### Changed

- Reprise's Library is no longer a tab: Home's library tile opens it, and B goes back to Home.
- The first-run setup has Back and Continue buttons.
- Reprise's tab bar search is on every tab, Settings included.
- In the game menu, Right opens an item with a chevron and Left goes back.

### Fixed

- The dock over a running game reads the controller again.
- Stopping a game no longer sends SIGTERM into the session's closing steps once the game has exited.
- On Reprise's Media tab, Guide and the menu key open the highlighted item's menu.
- Reprise's home rail lands on its first game when games arrive behind the setup dialog.
- Scrolling to a settings card's last row shows the card's bottom edge.

## [0.0.4] - 2026-09-21

### Breaking

- Modules are opt-in: with no `[modules] enabled` in `config.toml`, capture and the journal no longer run, while a list you set yourself is kept. Turn them on with `universe module enable capture` and `universe module enable journal`, or in Settings › Modules.
- Two defaults flip: MangoHud's overlay starts hidden (`launch.mangohud = true` shows it, the row is now Show MangoHud and the dock reads Shown or Hidden), and capture no longer records the microphone (`audio = "output+input"` does).
- `universe migrate` no longer rewrites an old `[launch] backend` as `runner`, and it no longer fills games it already imported with Lutris's wrapper, DLL overrides and Proton switches. Set `launch.runner` by hand on a game that still has `backend`.
- Journal entries that name a screenshot as `attachments/<name>` no longer find it in `screenshots/`, so drop the `attachments/` prefix in the entry's `images`. Shots left in `journal/attachments/` are no longer moved for you, and old entries no longer take their session times from `.migrated-sessions.jsonl`.
- The `switch2-white` and `switch2-black` look ids are gone and open Reprise. Pick Switch 2 again in Settings › Themes.
- A per-game module switch that an older `universe module set` wrote as `enabled = ["false"]` reads as on. Set it again.

### Added

- A first-run setup over the home screen finds Lutris, Steam, Heroic and your emulators' game folders, imports what Universe can launch, signs you in to your stores, and asks for your controller and the graphics upgrades your GPU takes. Settings › About › First-run setup opens it again.
- `universe rescan` adds the games in the folders your emulators list (Eden, Dolphin, Ryujinx, RPCS3, PCSX2, DuckStation, Cemu, shadPS4, Flycast), and `universe discover` lists what other launchers and those folders hold.
- A screen recording follows a monitor switch: the recorder moves to the new screen and the parts are joined into one file. Capture needs `ffmpeg`.
- `launch.proton` finds the newest build of a family (`proton-ge` is the latest `GE-Proton`) in Lutris's, Steam's and Heroic's folders, native or Flatpak.
- The DLSS, FSR 4 and XeSS upgrades take `auto`, which turns each one on where the GPU makes it a plain win. They stay off by default, and `true` and `false` still work.
- The banner art shows on the Switch 2 news cards and info pane when they have no picture, and behind a game or its launch when it has no background.

### Changed

- Adaptive sync is `auto` by default, on when the screen takes a variable refresh rate.
- Capture picks the best codec the GPU encodes by default (`codec = "auto"`).
- The Home Manager module leaves `config.toml` to Universe unless `programs.universe.settings` is set. When it is set, settings writes are refused with a message naming the option.
- With capture on, the NixOS module requires gpu-screen-recorder 6.1 or later.
- Settings cards start level with the page title, and store rows show square thumbnails.

### Fixed

- A Proton, Wine or native game finds the controller after an earlier session left InputPlumber holding it.
- Stopping an Eden game sends one SIGTERM, so Eden is no longer killed mid-shutdown, and `universe doctor` flags Eden asking before it closes.
- The launch poster holds until the game's window shows, however long that takes, and Cancel drops it, leaving the game starting behind the launcher.
- An enabled module whose programs are missing reads Unavailable in Settings › Modules, and a notice after the session says it ran nothing. `universe doctor` says `gsr-cli` comes with gpu-screen-recorder 6.1.

### Performance

- Screenshot grids show thumbnails the core makes in the background instead of decoding the full-size shots, and the Media list builds off the UI thread.
- Recording frames are extracted on the GPU, and the recordings page reads no file until a recording is opened.
- Behind a running game the launcher stops animating, and inside gamescope the library loads once at start.

### Removed

- The search row on the Reprise look's store page.

[0.0.10]: https://github.com/ilyasturki/universe/compare/v0.0.9...v0.0.10
[0.0.9]: https://github.com/ilyasturki/universe/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/ilyasturki/universe/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/ilyasturki/universe/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/ilyasturki/universe/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/ilyasturki/universe/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/ilyasturki/universe/compare/v0.0.3...v0.0.4
