<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="brand/universe-lockup-dark.svg"><img src="brand/universe-lockup.svg" width="420" alt="Universe"></picture></p>

A gamepad-first game launcher for Linux. A Rust core (a library, the `universe` CLI and a Python module for the host) owns the library, launches games through their runner — Proton via [umu-run](https://github.com/Open-Wine-Components/umu-launcher), Wine, the program itself, or an emulator — as transient systemd units, and records sessions and playtime. There is no daemon: systemd runs `universe session-end` when the game's cgroup empties, whatever happened to the process that launched it. A launcher that adopts a scope (the UI, `universe play` without `--no-wait`) binds the game to itself, so closing it takes the game down cleanly; `universe play --no-wait` and hooks leave the game to systemd. A Qt 6 / PySide6 host (`universe-ui`) renders the interface on top of the core, in-process, in one of two looks: [Reprise](https://github.com/ilyasturki/pegasus-theme-reprise), dark and cinematic, or the Switch 2 HOME menu, switched live from Settings › Themes; Settings › About shows the build (the version with the short git rev behind it, what `universe --version` prints), and B held anywhere asks to quit the launcher. Everything else is a source — GOG installs — or a module: dialog-free recording, an AI play journal.

Plain files are the truth: one `game.toml`, one `sessions.jsonl` and a `journal/` per game under `$XDG_DATA_HOME/universe/games/<id>/`.

## Install (Nix flake)

```nix
# flake.nix
inputs.universe.url = "github:ilyasturki/universe";
```

NixOS side (what the modules and sources need from the system):

```nix
imports = [ universe.nixosModules.default ];
programs.universe.enable = true;          # gpu-screen-recorder's setcap KMS helper, uinput, InputPlumber, gamescope
```

Home-manager side (the packages, config.toml):

```nix
imports = [ universe.homeModules.default ];
programs.universe = {
  enable = true;
  settings = {
    modules.enabled = [ "capture" "journal" ];
    sources.enabled = [ "gog" ];
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

The home-manager module also installs Universe's GNOME Shell extension (window recording and in-shell screenshots for `capture`, the volume OSD for the controller macros); **log out once** to let GNOME load it (until then `capture`'s window source records the screen, screenshots come from gpu-screen-recorder, and the volume macros show nothing).

`settings = null` installs the packages and leaves `~/.config/universe/config.toml` to you: the core edits that file in place (`universe config set`, `universe module enable`, `universe source enable`, the UI's settings), which a symlink into the store refuses.

Without home-manager: `nix profile install github:ilyasturki/universe`, then write `~/.config/universe/config.toml` (defaults in `docs/api.md`).

Packages: `universe` (default: core wrapped with the shipped modules and sources and their runtime on `PATH`), `universe-ui`, `core`, `universe-core-py` (the `universe_core` Python module), `modules`, `sources`, `universe-shell-extension`. `nix run .#universe-ui` starts the host. `universe` ships Fish completions (game names, modules and sources come from the library); both ship man pages: `man universe`, `man universe-play`, `man universe-ui`.

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
universe set technomancer fps_limit=none   # uncapped; auto (the default) holds a game to the refresh rate it sees through MangoHud's limiter, a number to that
universe set sekiro wayland=false hdr=true wrapper=gamemoderun   # Proton switches (esync fsync ntsync wayland hdr dlss_upgrade fsr4_upgrade xess_upgrade optiscaler; in the UI on Settings › Runners › Proton, the upscaler upgrades annotated for your GPU), a command in front of the program
universe add "/mnt/games/gamecube/F-Zero GX.iso" --runner dolphin   # a ROM, image or folder; --title, --platform, --media
universe runner ls             # every runner, where its program was found
universe runner set dolphin exe=/opt/dolphin/dolphin-emu batch=false   # a runner's program, arguments and options
universe set f-zero-gx options.batch=true platform="Nintendo Wii"      # per game
universe login gog             # prints the URL, takes the code
universe scan gog              # cross installed folders with the owned library
universe install 1434554947    # GOG id
universe update                # pending GOG updates (build id vs GOG builds endpoint)
universe uninstall technomancer   # trashes the install folder, keeps the hours and the journal
universe media technomancer refresh   # SteamGridDB + RAWG + Steam screenshots, into the empty slots
universe media technomancer status    # each slot: what shows, the fetched default, your pick over it
universe media technomancer set logo https://…   # or a file: a pick, kept over what refresh fetches
universe media technomancer search    # SteamGridDB's entries for the name, to `pin sgdb <id>` a wrong match
universe journal technomancer --render
universe recordings technomancer --remove 20260909-213045   # trashes the mkv, keeps the hours; journal --remove for an entry
universe doctor                # prerequisites of the core, every enabled module and source
universe controller ls         # connected pads, every button and what it does
universe controller bind xbox-elite paddle_p1 hold stop    # a macro; `learn` when a paddle is not recognised
universe ls --json | jq '.[] | select(.stats.hours > 10) | .title'
```

## Artwork

Every slot — box front, square (the home rail and the Switch 2 tiles), banner (920×430, shown nowhere yet), background, logo — has two layers: the **default** `refresh` fetches into `games/<id>/media/`, and your **pick**, an override under `paths.overrides/<id>/` that shows over it and survives every refresh. Pegasus's files are read as they are: its `tile` is the square, its `steam` the banner, and a `<id>/sgdb_id` file pins the SteamGridDB entry. A game's menu has an Artwork page in both looks: the five slots as art, each with one label saying what shows — your pick, who fetched the default (SteamGridDB, Steam, Pegasus), or missing — and where the theme uses it. A opens a slot: what shows now (and the default under a pick) above SteamGridDB's candidates for it, A puts one over the slot, X (Reprise) or the options menu (Switch 2) takes the pick off again; the wrong-game search — Y in Reprise, an option on the Switch 2 — lists SteamGridDB's entries for a name and pins the game to the right one (an entry's art is whatever its uploaders gave it); the same menu fetches the missing art and takes a file of your own for a slot. Settings › Artwork (Reprise) lays the library out as one row per game with its five slots across, gaps and picks marked, so what is missing shows at once; A opens the game on that slot, X fetches everything missing.

## Runners

Every game runs inside [gamescope](https://github.com/ValveSoftware/gamescope) unless told otherwise (`launch.gamescope`, per runner `universe runner set dolphin gamescope=false`, per game `universe set … gamescope=false`): one fullscreen window that shows the launch poster until the game draws, so the launcher hands the screen over on it — nothing Proton or umu put up first shows, and a game that swaps windows while it starts never lets the desktop through — and takes it back when it closes. gamescope is told the screen's own mode (read from Mutter, else the connector's preferred mode), so the game renders at the screen's resolution and refresh rate by default; Settings › Launch — and a game's Display card — set a resolution below it (upscaled with the scaler and filter of your choice), the refresh rate, adaptive sync, and any other flag as raw arguments. Every game is held to the refresh rate it sees by MangoHud's limiter inside the game (gamescope's own limiter paces nothing nested on a desktop), overlay or not: `fps_limit` is `auto`, `none`, or a number, global and per game. While a game runs the launcher is home again with the game pinned first, marked PLAYING, A resumes it, its menu quits it, and Play on another game asks first. The NixOS module enables `programs.gamescope`.

A game starts through its runner, as in Lutris: `proton` (umu-run), `wine`, `linux`, or an emulator — Dolphin, Eden (yuzu, Citron, Sudachi), Ryujinx, RPCS3, PCSX2, DuckStation, Cemu, Azahar (Citra), melonDS, mGBA, PPSSPP, xemu, Xenia, shadPS4, Vita3K, Mupen64Plus, Snes9x, Flycast, ScummVM, DOSBox, MAME. The core finds each emulator on `PATH` (or in Lutris's runners directory) and you can point it at another program: `universe runner set <id> exe=…`, or the Runners section of the Settings tab, which shows every runner with its logo, where it was found, its program, arguments and options, and adds a game through it from a file picker. Each emulator ships the flags that start it fullscreen and make it quit with the game, so the session ends when the game does; those are options, global or per game. `platform` on a game (Dolphin: GameCube or Wii) picks the tile icon and the collection. Recordings and the journal work for an emulated game exactly as for a Proton one.

Pads and emulators: every emulator has an `inputplumber` option, on by default, that makes the core take the pads over with [InputPlumber](https://github.com/ShadowBlip/InputPlumber) for the session (one composite device the emulator sees, the raw nodes hidden) and give them back at its end; the macro engine follows the pad onto the composite device; whether the paddles still reach it there is not verified. It needs the InputPlumber daemon on the system (`services.inputplumber` on NixOS), spoken to over the system bus; `universe doctor` says.

`universe migrate` turns Lutris's emulator games into runner games and reads Lutris's runner configs: a wrapper script around a pad tool is seen through, the emulator behind it and its extra arguments (a Ryujinx `--profile`) go to `[runners.<id>]`.

## Controller macros

The spare buttons of a pad — the Edge's paddles and Fn buttons, the Elite's paddles, the Pro 3's back buttons — carry macros: volume, mute, a screenshot, the MangoHud toggle, stopping the game on a long hold, any key combo, any command — on every button but Guide, which is HOME (a press for the menu, a hold to go home). The Settings tab has a Controller section that lists each button with its macros, learns a button by pressing it, and draws the pad live — every press, stick and trigger pull — in its test view. Nothing is grabbed: the engine reads the pad over evdev next to the game, so the game keeps rumble, the lightbar and every button it already saw. It runs only while the launcher is open or a session is running (`universe controller watch`, one instance at a time through a lock), never on the bare desktop. Pads can come and go while it runs, several pads each fire their family's macros, and a bind made anywhere reaches the running instance within its next scan. Paddle codes are not trusted: they differ between USB and Bluetooth and between drivers, so a slot is checked against what the pad advertises on every connect, and a button the seeds got wrong is fixed by pressing it (`universe controller learn xbox-elite paddle_p1`). The DualSense Edge's paddles reach evdev from kernel 7.2. An Elite Series 2 only reports its paddles on profile slot 0 (LED off) over Bluetooth (xpadneo) and on the in-tree xpad driver over USB; xone has no such gating. Volume and mute go straight to the default sink through `wpctl`, by `controller.volume_step` percent per press, so no synthetic key leaks into the game, and volume and mute show GNOME's own OSD (output name and level) through the Universe shell extension; a screenshot flashes the screen and plays the shutter once the pixels are grabbed, and lands in the game's `screenshots/` — the Media tab and the game's Screenshots page show them; the MangoHud macro flips the game's `mangohud` key and shows or hides the HUD in place (mangoapp told over its control queue, no key typed: it works while the game is paused, and the launcher toasts what it became); volume, mute and the MangoHud toggle still fire with the HOME dock over the game, the other macros wait for it to close; key macros type through uinput: enable `hardware.uinput` and put your user in the `uinput` group (the NixOS module does the first).

## Sources, modules and their prerequisites

A source installs and updates games from a store; a module runs hooks around every session. `universe doctor` checks these for every enabled one.

| | Kind | Needs | Notes |
|---|---|---|---|
| `gog` | source | `gogdl` | login via `universe login gog`; a dedicated `GOGDL_CONFIG_PATH` under the source's data dir |
| `capture` | module | `gpu-screen-recorder` ≥ 6.1 with `gsr-cli` and its setcap `gsr-kms-server`, `ffprobe`, `trash`; the `universe@ilyasturki.github.io` shell extension for the window source (GNOME) | the screen by default, or the game's window through GNOME's picker: `docs/api.md` § Recordings |
| `journal` | module | `ffmpeg`, `codex` (or `provider = "stub"`) | one Markdown entry per session from frames and screenshots |
| metadata | core | SteamGridDB and RAWG keys in `[keys]` | artwork slots `box_front`, `square`, `banner`, `background`, `logo`, screenshots |
| runners | core | the emulator on `PATH` (or `[runners.<id>] exe`); `inputplumber` for the pad option | `universe runner ls`; one doctor check per runner in use |

Cursor hiding on GNOME toggles the `hide-cursor@elcste.com` shell extension around the session.

Settings › Modules and Settings › Sources list each one with its switch — flipped in place, or from its page, which holds its settings (and a source's sign-in); the same on the CLI: `universe module ls · enable capture · settings journal`, `universe source ls · enable gog · set gog platform=linux`.

Third-party modules and sources: drop a directory with a `module.toml` under `~/.config/universe/modules/`, or a `source.toml` under `~/.config/universe/sources/` (user ones override shipped ones). The manifests, the hook environment and the source protocol (JSON lines) are frozen in `docs/api.md`.

## Develop

A `justfile` wraps everything in `nix develop` and points the core at an isolated `.dev/` (its own config, data, recordings, journal), so nothing touches `~/.config/universe`. The host runs from `.venv/`, a venv on the dev shell's Python where `maturin develop` installs `universe_core` and `universe_ui` is installed editable (`just develop`, run by `ui` and `test`):

`just setup` first; `just --list` names the rest. `just test` is what CI runs; `just test-live` adds the checks that need this machine — a transient unit through the user systemd, a scope around the test process, an InputPlumber engage/release cycle (the pads vanish for ~10 s), the preferred mode of every connected output. `UNIVERSE_DEV=.dev-empty just ui` runs any recipe on another profile; a new one starts as an empty library.

`docs/api.md` is the core API, the process model and the module and source contracts; `docs/frontends.md` is what a frontend binds to.
