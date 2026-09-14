<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="brand/universe-lockup-dark.svg"><img src="brand/universe-lockup.svg" width="420" alt="Universe"></picture></p>

A gamepad-first game launcher for Linux. A Rust core (a library, the `universe` CLI and a Python module for the host) owns the library, launches games through their runner — Proton via [umu-run](https://github.com/Open-Wine-Components/umu-launcher), Wine, the program itself, or an emulator — as transient systemd units, and records sessions and playtime. There is no daemon: systemd runs `universe session-end` when the game's cgroup empties, whatever happened to the process that launched it. A launcher that adopts a scope (the UI, `universe play` without `--no-wait`) binds the game to itself, so closing it takes the game down cleanly; `universe play --no-wait` and hooks leave the game to systemd. A Qt 6 / PySide6 host (`universe-ui`) renders the interface on top of the core, in-process, in one of two looks: [Reprise](https://github.com/ilyasturki/pegasus-theme-reprise), dark and cinematic, or the Switch 2 HOME menu, switched live from Settings › Themes. Everything else — GOG installs, dialog-free recording, an AI play journal — is a module.

Plain files are the truth: one `game.toml`, one `sessions.jsonl` and a `journal/` per game under `$XDG_DATA_HOME/universe/games/<id>/`. SQLite is only a rebuildable index.

## Install (Nix flake)

```nix
# flake.nix
inputs.universe.url = "github:ilyasturki/universe";
```

NixOS side (KMS capture helper, packages):

```nix
imports = [ universe.nixosModules.default ];
programs.universe.enable = true;          # programs.gpu-screen-recorder + packages
```

Home-manager side (config.toml, enabled modules):

```nix
imports = [ universe.homeModules.default ];
programs.universe = {
  enable = true;
  modules.enabled = [ "gog" "capture" "journal" ];
  settings = {
    paths.games_root = "/mnt/games/PC";
    paths.prefixes_root = "/mnt/games/prefixes";
    paths.recordings_root = "/mnt/recordings/games";
    launch.proton = "proton-ge";
    proton.proton-ge = "/path/to/GE-Proton";
    modules.capture.codec = "av1_10bit";
    keys.sgdb_file = "~/.config/steamgriddb/api_key";
    keys.rawg_file = "~/.config/rawg/api_key";
  };
};
```

The home-manager module also installs Universe's GNOME Shell extension (window recording and in-shell screenshots for `capture`, the volume OSD for the controller macros); **log out once** to let GNOME load it (until then `capture` records the screen, screenshots come from gpu-screen-recorder without a flash, and the volume macros show nothing).

`settings = null` installs the packages and leaves `~/.config/universe/config.toml` to you: the core edits that file in place (`universe config set`, `universe module enable`, the UI's settings), which a symlink into the store refuses.

Without home-manager: `nix profile install github:ilyasturki/universe`, then write `~/.config/universe/config.toml` (defaults in `docs/api.md`).

Packages: `universe` (default: core wrapped with the shipped modules and their runtime on `PATH`), `universe-ui`, `core`, `universe-core-py` (the `universe_core` Python module), `modules`, `modules-<id>`. `nix run .#universe-ui` starts the host. `universe` and `universe-ui` ship Fish completions (game names, modules and sources come from the library) and man pages: `man universe`, `man universe-play`, `man universe-ui`.

## Use

```sh
universe migrate --apply       # import the Lutris library, hours included
universe                       # library, last played first
universe play technomancer     # exact › word › substring › path; interactive pick if ambiguous
universe status                # current session, last sessions
universe info technomancer
universe set technomancer proton=proton-em capture.cursor=true
universe set technomancer gamescope=false   # this game on the desktop, not inside gamescope
universe set technomancer gamescope_resolution=1920x1080 gamescope_filter=fsr   # rendered at 1080p, FSR-upscaled to the screen; gamescope_args="…" for any other flag
universe set sekiro wayland=false hdr=true wrapper=gamemoderun   # Proton switches (esync fsync ntsync wayland hdr dlss_upgrade fsr4_upgrade xess_upgrade optiscaler), a command in front of the program
universe add "/mnt/games/gamecube/F-Zero GX.iso" --runner dolphin   # a ROM, image or folder; --title, --platform, --media
universe runner ls             # every runner, where its program was found
universe runner set dolphin exe=/opt/dolphin/dolphin-emu batch=false   # a runner's program, arguments and options
universe set f-zero-gx options.batch=true platform="Nintendo Wii"      # per game
universe gog login             # prints the URL, takes the code
universe gog scan              # cross installed folders with the owned library
universe install 1434554947    # GOG id
universe update                # pending GOG updates (build id vs GOG builds endpoint)
universe uninstall technomancer   # trashes the install folder, keeps the hours and the journal
universe media technomancer refresh   # SteamGridDB + RAWG + Steam screenshots, into the empty slots
universe media technomancer status    # each slot: what shows, the fetched default, your pick over it
universe media technomancer set logo https://…   # or a file: a pick, kept over what refresh fetches
universe media technomancer search    # SteamGridDB's entries for the name, to `pin sgdb <id>` a wrong match
universe journal technomancer --render
universe recordings technomancer --remove 20260909-213045   # trashes the mkv, keeps the hours; journal --remove for an entry
universe module ls · enable capture · settings journal
universe doctor                # prerequisites of the core and every enabled module
universe controller ls         # connected pads, every button and what it does
universe controller bind xbox-elite paddle_p1 hold stop    # a macro; `learn` when a paddle is not recognised
universe ls --json | jq '.[] | select(.stats.hours > 10) | .title'
```

## Artwork

Every slot — box front, square (the home rail and the Switch 2 tiles), banner (920×430, shown nowhere yet), background, logo — has two layers: the **default** `refresh` fetches into `games/<id>/media/`, and your **pick**, an override under `paths.overrides/<id>/` that shows over it and survives every refresh. Pegasus's files are read as they are: its `tile` is the square, its `steam` the banner, and a `<id>/sgdb_id` file pins the SteamGridDB entry. In Reprise, a game's menu has an Artwork page: the five slots with what each shows (your pick, the default and who fetched it, or nothing) and where the theme uses it, the focused one large with the default under a pick, and SteamGridDB's candidates for it under the entry they come from — A puts one over the slot, X takes the pick off again, Y lists SteamGridDB's entries for the name and pins the game to the right one when the match was wrong (an entry's art is whatever its uploaders gave it). Settings › Artwork is the same across the library: one slot at a time, every game's art as a grid, filtered by state, X fetches everything missing. The Switch 2 look keeps its Refresh artwork row and shows the picks like any other art.

## Runners

Every game runs inside [gamescope](https://github.com/ValveSoftware/gamescope) unless told otherwise (`launch.gamescope`, per runner `universe runner set dolphin gamescope=false`, per game `universe set … gamescope=false`): one fullscreen window that shows the launch poster until the game draws, so the launcher hands the screen over on it — nothing Proton or umu put up first shows, and a game that swaps windows while it starts never lets the desktop through — and takes it back when it closes. gamescope is told the screen's own mode (read from Mutter, else the connector's preferred mode), so the game renders at the screen's resolution and refresh rate by default; Settings › Launch — and a game's Gamescope group — set a resolution below it (upscaled with the scaler and filter of your choice), the refresh rate, a frame rate limit, adaptive sync, and any other flag as raw arguments. While a game runs the launcher is home again with the game pinned first, marked PLAYING, A resumes it, its menu quits it, and Play on another game asks first. The NixOS module enables `programs.gamescope`.

A game starts through its runner, as in Lutris: `proton` (umu-run), `wine`, `linux`, or an emulator — Dolphin, Eden (yuzu, Citron, Sudachi), Ryujinx, RPCS3, PCSX2, DuckStation, Cemu, Azahar (Citra), melonDS, mGBA, PPSSPP, xemu, Xenia, shadPS4, Vita3K, Mupen64Plus, Snes9x, Flycast, ScummVM, DOSBox, MAME. The core finds each emulator on `PATH` (or in Lutris's runners directory) and you can point it at another program: `universe runner set <id> exe=…`, or the Runners section of the Settings tab, which shows every runner with its logo, where it was found, its program, arguments and options, and adds a game through it from a file picker. Each emulator ships the flags that start it fullscreen and make it quit with the game, so the session ends when the game does; those are options, global or per game. `platform` on a game (Dolphin: GameCube or Wii) picks the tile icon and the collection. Recordings and the journal work for an emulated game exactly as for a Proton one.

Pads and emulators: every emulator has an `inputplumber` option, on by default, that makes the core take the pads over with [InputPlumber](https://github.com/ShadowBlip/InputPlumber) for the session (one composite device the emulator sees, the raw nodes hidden) and give them back at its end; the macro engine follows the pad onto the composite device; whether the paddles still reach it there is not verified. It needs the InputPlumber daemon on the system (`services.inputplumber` on NixOS) and its CLI on `PATH`; `universe doctor` says.

`universe migrate` turns Lutris's emulator games into runner games and reads Lutris's runner configs: a wrapper script around a pad tool is seen through, the emulator behind it and its extra arguments (a Ryujinx `--profile`) go to `[runners.<id>]`.

## Controller macros

The spare buttons of a pad — the Edge's paddles and Fn buttons, the Elite's paddles, the Pro 3's back buttons — carry macros: volume, mute, a screenshot, the MangoHud toggle, stopping the game on a long hold, any key combo, any command. The Settings tab has a Controller section that lists each button with its macros, learns a button by pressing it, and draws the pad live — every press, stick and trigger pull — in its test view. Nothing is grabbed: the engine reads the pad over evdev next to the game, so the game keeps rumble, the lightbar and every button it already saw. It runs only while the launcher is open or a session is running (`universe controller watch`, one instance at a time through a lock), never on the bare desktop. Pads can come and go while it runs, several pads each fire their family's macros, and a bind made anywhere reaches the running instance within its next scan. Paddle codes are not trusted: they differ between USB and Bluetooth and between drivers, so a slot is checked against what the pad advertises on every connect, and a button the seeds got wrong is fixed by pressing it (`universe controller learn xbox-elite paddle_p1`). The DualSense Edge's paddles reach evdev from kernel 7.2. An Elite Series 2 only reports its paddles on profile slot 0 (LED off) over Bluetooth (xpadneo) and on the in-tree xpad driver over USB; xone has no such gating. Volume and mute go straight to PulseAudio (PipeWire's server included) through libpulse, by `controller.volume_step` percent per press, so no synthetic key leaks into the game, and volume and mute show GNOME's own OSD (output name and level) through the Universe shell extension; a screenshot flashes the captured area and plays the shutter, at the press; key macros and the MangoHud toggle type through uinput (the launcher toasts the MangoHud fire, since that key never reaches it): enable `hardware.uinput` and put your user in the `uinput` group (the NixOS module does the first).

## Modules and their prerequisites

`universe doctor` checks these for every enabled module.

| Module | Kind | Needs | Notes |
|---|---|---|---|
| `gog` | source | `gogdl` | login via `universe gog login`; a dedicated `GOGDL_CONFIG_PATH` under the module's data dir |
| `capture` | hooks | `gpu-screen-recorder` + its setcap `gsr-kms-server` (screen source); `gst-launch-1.0` + the `universe@ilyasturki.github.io` shell extension (window source, GNOME) | `source = "window"` (default) records just the game's window through Mutter's ScreenCast, dialog-free, following it across workspaces and occlusion; `source = "screen"` (or off GNOME / extension not loaded / no `jeepney` / no window in 60 s) falls back to KMS screen capture and says so on the shell's OSD. `cursor` per game; `fps = "auto"` follows the output's refresh rate (read from Mutter, 60 elsewhere) |
| `journal` | hooks | `ffmpeg`, `codex` (or `provider = "claude"` / `"stub"`) | one Markdown entry per session from frames and screenshots |
| metadata | core | SteamGridDB and RAWG keys in `[keys]` | artwork slots `box_front`, `square`, `banner`, `background`, `logo`, screenshots |
| runners | core | the emulator on `PATH` (or `[runners.<id>] exe`); `inputplumber` for the pad option | `universe runner ls`; one doctor check per runner in use |

Cursor hiding on GNOME toggles the `hide-cursor@elcste.com` shell extension around the session.
Window capture, screenshots and the macro OSD use the `universe@ilyasturki.github.io` shell extension, installed by the
home-manager module; GNOME loads it after the next logout, until then capture records the screen.

Settings › Modules lists the modules, on or off, and opens each one's page with its settings; the same on the CLI: `universe module ls · enable capture · settings journal`.

Third-party modules: drop a directory with a `module.toml` under `~/.local/share/universe/modules/` (user modules override shipped ones). The manifest, the hook environment and the source protocol (JSON lines) are frozen in `docs/api.md`.

## Develop

A `justfile` wraps everything in `nix develop` and points the core at an isolated `.dev/` (its own config, data, recordings, journal), so nothing touches `~/.config/universe`. The host runs from `.venv/`, a venv on the dev shell's Python where `maturin develop` installs `universe_core` and `universe_ui` is installed editable (`just develop`, run by `ui` and `test`):

```sh
just setup                 # build, create .dev/config/config.toml, run doctor
just cli migrate --apply   # any CLI command against .dev/ (gog login, gog scan, media <id> refresh, launch <id>…)
just cli play <game>       # a game is a transient systemd unit; `just logs` follows them
just ui                    # PySide6 host on the in-process core (add --windowed)
just ui-fake               # host on a fixture library, no core
just seed [id…]            # copy real games (journal, media, recording refs) into .dev/ to test the player and the journal
just test / just check     # cargo + pytest / flake packages + sandboxed checks
just clean                 # trash .dev/ and .venv/
just bump patch|minor|major|X.Y.Z   # release: rewrite every version copy from Cargo.toml, commit, tag vX.Y.Z (no push)
```

`docs/api.md` is the core API, the process model and the module contract; `docs/frontends.md` is what a frontend binds to.
