<p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="brand/universe-lockup-dark.svg"><img src="brand/universe-lockup.svg" width="420" alt="Universe"></picture></p>

A gamepad-first game launcher for Linux. A Rust core (a library, the `universe` CLI and a Python module for the host) owns the library, launches games through [umu-run](https://github.com/Open-Wine-Components/umu-launcher) as transient systemd units, and records sessions and playtime. There is no daemon: systemd runs `universe session-end` when the game's cgroup empties, whatever happened to the process that launched it. A Qt 6 / PySide6 host (`universe-ui`) renders the [Reprise](https://github.com/ilyasturki/pegasus-theme-reprise) interface on top of the core, in-process. Everything else — GOG installs, dialog-free recording, an AI play journal — is a module.

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
universe gog login             # prints the URL, takes the code
universe gog scan              # cross installed folders with the owned library
universe install 1434554947    # GOG id
universe update                # pending GOG updates (build id vs GOG builds endpoint)
universe uninstall technomancer   # trashes the install folder, keeps the hours and the journal
universe media technomancer refresh   # SteamGridDB + RAWG + Steam screenshots
universe journal technomancer --render
universe module ls · enable capture · settings journal
universe doctor                # prerequisites of the core and every enabled module
universe controller ls         # connected pads, every button and what it does
universe controller bind xbox-elite paddle_p1 hold stop    # a macro; `learn` when a paddle is not recognised
universe ls --json | jq '.[] | select(.stats.hours > 10) | .title'
```

## Controller macros

The spare buttons of a pad — the Edge's paddles and Fn buttons, the Elite's paddles, the Pro 3's back buttons — carry macros: volume, mute, a screenshot, the MangoHud toggle, stopping the game on a long hold, any key combo, any command. The Settings tab has a Controller section that draws the pad and lets you bind each button by pressing it. Nothing is grabbed: the engine reads the pad over evdev next to the game, so the game keeps rumble, the lightbar and every button it already saw. It runs only while the launcher is open or a session is running (`universe controller watch`, one instance at a time through a lock), never on the bare desktop. Pads can come and go while it runs, several pads each fire their family's macros, and a bind made anywhere reaches the running instance within its next scan. Paddle codes are not trusted: they differ between USB and Bluetooth and between drivers, so a slot is checked against what the pad advertises on every connect, and a button the seeds got wrong is fixed by pressing it (`universe controller learn xbox-elite paddle_p1`). The DualSense Edge's paddles reach evdev from kernel 7.2. An Elite Series 2 only reports its paddles on profile slot 0 (LED off) over Bluetooth (xpadneo) and on the in-tree xpad driver over USB; xone has no such gating. Key macros type through uinput: enable `hardware.uinput` and put your user in the `uinput` group (the NixOS module does the first).

## Modules and their prerequisites

`universe doctor` checks these for every enabled module.

| Module | Kind | Needs | Notes |
|---|---|---|---|
| `gog` | source | `gogdl` | login via `universe gog login`; a dedicated `GOGDL_CONFIG_PATH` under the module's data dir |
| `capture` | hooks | `gpu-screen-recorder` + its setcap `gsr-kms-server`, both from the host system and the same nixpkgs (the NixOS module enables and pins `programs.gpu-screen-recorder`) | KMS capture of the output the game runs on, no portal dialog; `cursor` per game; `fps = "auto"` follows the output's refresh rate (read from Mutter, 60 elsewhere) |
| `journal` | hooks | `ffmpeg`, `codex` (or `provider = "claude"` / `"stub"`) | one Markdown entry per session from frames and screenshots |
| metadata | core | SteamGridDB and RAWG keys in `[keys]` | artwork slots `box_front`, `tile`, `background`, `logo`, screenshots |

Cursor hiding on GNOME toggles the `hide-cursor@elcste.com` shell extension around the session.

Third-party modules: drop a directory with a `module.toml` under `~/.local/share/universe/modules/` (user modules override shipped ones). The manifest, the hook environment and the source protocol (JSON lines) are frozen in `docs/api.md`.

## Develop

A `justfile` wraps everything in `nix develop` and points the core at an isolated `.dev/` (its own config, data, recordings, journal), so nothing touches `~/.config/universe`:

```sh
just setup                 # build, create .dev/config/config.toml, run doctor
just cli migrate --apply   # any CLI command against .dev/ (gog login, gog scan, media <id> refresh, launch <id>…)
just cli play <game>       # a game is a transient systemd unit; `just logs` follows them
just ui                    # PySide6 host on the in-process core (add --windowed)
just ui-fake               # host on a fixture library, no core
just seed [id…]            # copy real games (journal, media, recording refs) into .dev/ to test the player and the journal
just test / just check     # cargo + pytest / flake packages + sandboxed checks
just clean                 # trash .dev/
```

`docs/api.md` is the core API, the process model and the module contract; `docs/frontends.md` is what a frontend binds to.
