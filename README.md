# Universe

A gamepad-first game launcher for Linux. A Rust core (`universed` daemon + `universe` CLI) owns the library, launches games through [umu-run](https://github.com/Open-Wine-Components/umu-launcher) in systemd scopes, records sessions and playtime, and exposes everything over D-Bus (`io.github.ilyasturki.Universe`). A Qt 6 / PySide6 host (`universe-ui`) renders the [Reprise](https://github.com/ilyasturki/pegasus-theme-reprise) interface on top of it. Everything else — GOG installs, dialog-free recording, an AI play journal, a Markdown tracker — is a module.

Plain files are the truth: one `game.toml`, one `sessions.jsonl` and a `journal/` per game under `$XDG_DATA_HOME/universe/games/<id>/`. SQLite is only a rebuildable index.

## Install (Nix flake)

```nix
# flake.nix
inputs.universe.url = "github:ilyasturki/universe";
```

NixOS side (KMS capture helper, D-Bus service, packages):

```nix
imports = [ universe.nixosModules.default ];
programs.universe.enable = true;          # programs.gpu-screen-recorder + dbus service
```

Home-manager side (config.toml, `universed` user service, enabled modules):

```nix
imports = [ universe.homeManagerModules.default ];
programs.universe = {
  enable = true;
  modules.enabled = [ "gog" "capture" "journal" "tracker-md" ];
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

Without home-manager: `nix profile install github:ilyasturki/universe`, then write `~/.config/universe/config.toml` (defaults in `docs/api.md`). The CLI starts `universed` on demand.

Packages: `universe` (default: core wrapped with the shipped modules and their runtime on `PATH`), `universe-ui`, `core`, `modules`, `modules-<id>`. `nix run .#universe-ui` starts the host.

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
universe media technomancer refresh   # SteamGridDB + RAWG + Steam screenshots
universe journal technomancer --render
universe module ls · enable capture · settings journal
universe doctor                # prerequisites of the core and every enabled module
universe ls --json | jq '.[] | select(.stats.hours > 10) | .title'
```

## Modules and their prerequisites

`universe doctor` checks these for every enabled module.

| Module | Kind | Needs | Notes |
|---|---|---|---|
| `gog` | source | `gogdl` | login via `universe gog login`; a dedicated `GOGDL_CONFIG_PATH` under the module's data dir |
| `capture` | hooks | `gpu-screen-recorder` + its setcap `gsr-kms-server` (`programs.gpu-screen-recorder.enable` on NixOS) | KMS capture of the output the game runs on, no portal dialog; `cursor` per game |
| `journal` | hooks | `ffmpeg`, `codex` (or `provider = "claude"` / `"stub"`) | one Markdown entry per session from frames and screenshots |
| `tracker-md` | hooks | — | keeps a Markdown tracker's Hours column and journal links in sync |
| metadata | core | SteamGridDB and RAWG keys in `[keys]` | artwork slots `box_front`, `tile`, `background`, `logo`, screenshots |

Cursor hiding on GNOME toggles the `hide-cursor@elcste.com` shell extension around the session.

Third-party modules: drop a directory with a `module.toml` under `~/.local/share/universe/modules/` (user modules override shipped ones). The manifest, the hook environment and the source protocol (JSON lines) are frozen in `docs/api.md`.

## Develop

```sh
nix develop                    # cargo, PySide6, Qt 6 QML paths, module runtime
cargo test
python -m pytest               # ui/tests + modules/*/tests
nix build .#universe && nix flake check
```

`docs/plan-v3.html` is the design; `docs/api.md` the D-Bus and module contract; `docs/progress.md` the state of the work.
