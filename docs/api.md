# Universe — core API, module and source protocols

`api = 2`. The core is a Rust library (`crates/universe`, `universe::core::Core`). **No Universe
process runs in the background.** There are three ways into it, all in-process:

- **the crate** — `Core::open().await`, every method `async`;
- **`universe_core`** — the Python module built from `crates/universe-py` (PyO3). `Core()` opens the
  core; every method releases the GIL for the duration of the call and hands back plain dicts,
  lists and strings (`None` for "nothing"), taking dicts and lists where the crate takes a struct.
  `data_home()`, `state_home()` and `version()` are methods on it. This is what the PySide6 host
  binds;
- **the `universe` CLI** — the same library, one process per command, `--json` on every command.
  This is what hooks and systemd call.

The crate hands back its structs or `serde_json::Value`s, Python the same as dicts and lists, and
the CLI prints them as JSON under `--json`. Each table below gives all three spellings of the same
operation; a dash means the surface doesn't expose it.

## Process model

- A play session is a **transient systemd unit**, `universe-game-<id>-<session>.service`, with
  `ExitType=cgroup` — it lives as long as any process of the game lives. Its
  `ExecStopPost=universe session-end <id> <session>` runs when the cgroup empties, whatever became
  of the launcher: it appends the `sessions.jsonl` line, undoes what the launch began in reverse
  order (the cursor hiding, `post_command` for `pre_command`, the machine's controls the game's own
  `[system]` changed, see System) as the marker lists them, then runs the `session-end` hooks, then
  the `post-process` hooks.
- **Who owns the game's lifetime** depends on the launcher. One that called `adopt_scope()` — the UI,
  and `universe play` without `--no-wait` — was moved into `universe-launcher-<pid>.scope`, and every
  game it launches carries `BindsTo=` + `After=` that scope: the game goes down with the launcher
  (a crash, a kill, Ctrl-C), `session-end` still runs. `universe play --no-wait`, hooks and anything
  else that never adopted a scope leave the game to systemd alone: it outlives them.
- `state/current-session.json` (written `O_EXCL`) is the marker for the running session: the session
  (`session_id`, `id`, `title`, `unit`, `screen`, `started_at`), the hook environment and the `undo`
  list of the effects `launch` began. The current session is the marker whose unit is still active.
  On every open the core **reconciles**: a marker with no live unit is closed from `journalctl`
  timestamps, one it cannot read is dropped. A launch that fails after an effect began undoes the
  same list.
- Asynchronous hooks (`post-launch`, `post-process`) run as transient units. A `post-launch` hook
  that starts a process meant to last the whole session (the recorder) must put it in its own unit
  with `BindsTo=$SESSION_UNIT After=$SESSION_UNIT`, so it stops with the game even if nothing else
  is watching. The `freeze` and `thaw` hooks run blocking after `freeze(on)` (see Recordings).
- Long jobs (`install`, `update`, `scan`, media refresh) run **in the calling process** with a
  progress callback. Closing the frontend interrupts them.
- One exception to "nothing in the background" is the **controller watcher**
  (`universe controller watch`), which lives exactly as long as the launcher or a session does: the
  UI starts one for its lifetime, `launch` starts one bound to the game's unit
  (`universe-controller-<session>`, `BindsTo=` the game), and a lock (`$XDG_RUNTIME_DIR/universe/
  controller.lock`) hands the pads over between them. Once both are gone nothing runs. It notes the
  pad a button was last pressed on in `$XDG_RUNTIME_DIR/universe/active-pad` (the sysfs device its
  evdev node hangs off), the pad the controls module makes player 1.
- The other is `universe keep-awake`, started by a launch as `universe-awake-<session>` (`BindsTo=`
  the game, `desktop.keep_awake`): it holds every inhibit the session offers, each standing only
  as long as the connection that took it, so it is a unit rather than a step undone at the end:
  `org.freedesktop.ScreenSaver` (the blank and lock; GNOME's suspend with them),
  `org.freedesktop.PowerManagement` (the idle suspend of KDE's PowerDevil and xfce4-power-manager,
  which ScreenSaver does not hold off) and a logind `idle` lock (logind's `IdleAction`, hypridle).
  No `sleep` lock: since systemd 257 it would refuse the player's own Suspend too.
- The CLI that units and hooks call back is `$UNIVERSE_BIN`, else the running `universe`'s argv[0],
  else (the UI's in-process core) `universe` on PATH. The Nix packages and `install.sh`'s wrappers
  (`~/.local/lib/universe`, beside the core's own `~/.local/share/universe`) set it along with
  `UNIVERSE_{MODULES,SOURCES}_PATH`; the AUR package puts the modules and sources under
  `/usr/share/universe`, which `XDG_DATA_DIRS` already names.
- Errors: `Kind ∈ NotFound, Ambiguous, Busy, Invalid, Unavailable, Io`. Python raises
  `universe_core.UniverseError(kind, message)`; the CLI prints `universe: <kind>: <message>` on
  stderr and exits 1.
- Identifiers: `id` is a slug derived from the title and names the `games/<id>/` directory.
  `session_id` is `YYYYMMDD-HHMMSS`.

## Library

| Rust | Python | CLI | Role |
|---|---|---|---|
| `list()` | `list()` | `universe ls [--all]` | `[Game]`; unhidden first, last played first. `--json` always prints every game — `--all` only stops the table from hiding the hidden ones |
| `get(id)` | `get(id)` | `universe info <name> --json` | resolved `Game`: global defaults merged in, session stats, media, active modules |
| `resolve(query)` | `resolve(query)` | — | candidate ids: exact › whole word › substring › path › every word a prefix of a title word or genre. Empty means unknown, more than one means ambiguous |
| `set(id, key, value)` | `set(id, key, value)` | `universe set <name> k=v …` | writes one `game.toml` key |
| `remove(id, purge)` | `remove(id, purge)` | `universe rm <name> [--purge]` | parks recordings under `.archive/` (the journal stays in `games/<id>/journal/`), marks `removed_at`; `purge` also trashes the game's logs and its Wine prefix (`launch.prefix`, else its folder under `paths.prefixes_root`) when it lies under `paths.prefixes_root`: a store's (Steam's `compatdata`) or one imported from Lutris stays. It refuses a prefix another library game uses, and backs the saves up first (see Game data) |
| `uninstall(id)` | `uninstall(id)` | `universe uninstall <name>` | through the game's source when it is active and declares `uninstall` (the store removes the files and forgets the install: Steam, Epic, itch.io), else trashes `source.dir`; then clears `source.dir`, `source.build_id` and `launch.exe`: the game stays in the library, not installed. The trash refuses a root, a home or the games root |
| `uninstall_via(id)` | `uninstall_via(id)` | — | the name of the store that removes the game's files itself (the source `uninstall` goes through), `None` when the folder goes to the trash: the confirmation says which |
| `reload_all()` | `reload()` | — | rereads config and `games/*/game.toml` |
| `rescan()` | `rescan()` | `universe rescan` | `reload_all`, then `import_roms(true)`: its report (the CLI then fetches the new games' art) |
| `reload_game(id)` | `reload_game(id)` | — | rereads one game |
| `import_lutris(apply)` | `import_lutris(apply)` | `universe migrate [--apply]` | a report of what it read from Lutris's own folders (`~/.config/lutris`, `~/.local/share/lutris/pga.db`, under the XDG homes): imported games, per-game env diff (`{id, lutris_env, universe_env, added, removed, changed}`), imported hours and `runners` (what Lutris's runner configs say: a wrapper script is seen through, the program is written to `[runners.<id>] exe` when it is not on PATH, its extra arguments to `args`). Without `apply` it only reports |
| `discover()` | `discover()` | `universe discover [--json]` | what other launchers hold on this machine, nothing written: `{launchers: [{id, name, found, dir, games, titles, importable, via, detail}], gog_dirs, install_dirs: [{dir, by}]}`. `lutris` (`via: "lutris"`: the installed games `import_lutris` would add), `steam` (`via: "steam"`: the `appmanifest_*.acf` of every library in `libraryfolders.vdf`, Proton and the runtimes left out), `heroic-gog` (`via: "gog"`: `goggame-*.info` installs one level under `gog_dirs` and `paths.games_root`), `heroic-epic` (`via: "epic"`: legendary's `installed.json`, the installs whose folder is still there) and `heroic-amazon` (nile's), `itch` (`via: "itch"`: the itch app's installs, from the `caves` of its `db/butler.db`), `roms` (`via: "roms"`: the games `import_roms` would add, `dir` the folders it read). `importable` names what Universe launches itself; `titles` holds the first six. `gog_dirs` are Heroic's GOG install folders (its `gog_store/installed.json` and default install path), for the gog source's `scan_dirs` before a `scan`; `install_dirs` the folders other launchers install into (those and Lutris's `game_path`, `by` naming the launcher), that exist and are not `paths.games_root`: the choices of a first run's install folder |
| `onboarded()` / `mark_onboarded()` | `onboarded()` / `mark_onboarded()` | — | whether the first-run setup ran on this machine, in any frontend: `$XDG_STATE_HOME/universe/onboarded` |
| `changelog::releases()` | `changelog()` | — | every release of the `CHANGELOG.md` the build was made from (embedded, no network), newest first: `[{version, date, sections: [{title, items}]}]`, an item one Markdown line |
| `whats_new()` | `whats_new()` | — | for a frontend's start only: the releases since the version the last start ran, newest first, when `desktop.whats_new` is on; records this version in `$XDG_STATE_HOME/universe/last-version` either way, so the page shows once in whichever app starts first. No file (a first start, or the first after an upgrade from a version without it) records it and shows nothing; a downgrade or an unreadable file shows nothing |
| `import_roms(apply)` | `import_roms(apply)` | — (`universe rescan` applies; `universe discover` reports) | the games under the folders the installed emulators list in their own configuration — Eden and its forks' `Paths\gamedirs` (with `deep_scan`), Dolphin's `ISOPath*` (`RecursiveISOPaths`), Ryujinx's `game_dirs`, RPCS3's `games.yml` entries and `dev_hdd0/game`, PCSX2's and DuckStation's `[GameList]` paths, Cemu's `GamePaths`, shadPS4's `installDirs`, Flycast's `ContentPath`; melonDS and mGBA keep no list — as `{folders: [{runner, dir, recursive}], imported: [{id, title, runner, path}], skipped: [{path, reason}], applied}`. A file is a game by the runner's extensions; RPCS3 and shadPS4 games are folders (`PS3_GAME/USRDIR/EBOOT.BIN`, `<id>/eboot.bin`, titled from `PARAM.SFO`), Cemu's `code/*.rpx` from `meta/meta.xml`. `updates`, `dlc`, `mods`, `firmware`, `amiibo`, `backups`, `downloads`, `prefixes`, `saves`, `textures`, `shaders` and hidden folders are not entered, a `.bin` beside a `.cue`/`.gdi`/`.m3u` is a track, a shadPS4 `-UPDATE` folder a patch. A file already in the library (same inode: a bind mount counts once) is left out, and so is one whose title is taken by another file or by a library game (`skipped` says which); a plain name beats a tagged one for the same title. With `apply`, each is `add_game`d; their art is left to `media_refresh_many`, so a frontend fetches it in the background once the games are in |
| `add_game(spec)` | `add_game(spec)` | `universe add <file> --runner <id> [--title T] [--platform P] [--media]` | `{"runner", "exe", "title"?, "platform"?}` → the new id. The title defaults to the file's name cleaned of release tags; the platform to the runner's first. Refuses an id already in the library |

`set` takes dotted keys: the `launch.*` keys of `universe launch-keys` — the one catalogue
(`launch_keys.rs`) every `[launch]` key is declared in, with its type, default, scope and choices;
a game key left empty takes `[launch]`'s, a value is validated as the key's type says, an unknown
or global-only key is refused — with the maps `launch.dll_overrides.d3d11`, `launch.env.FOO` and
`launch.options.<key>` (validated against the runner's options); then `desktop.hide_cursor`, `hidden`,
`favorite`, `tags`, `sort_title`, `platform`, the pins `metadata.steam_appid`, `metadata.gamesdb_id` and `metadata.sgdb_id`, `saves.name` (ludusavi's title, see Game data), and `capture.cursor` as a
validated shorthand for `modules.capture.cursor`. Values are strings: `true`/`false` for booleans, `auto`/`on`/`off` for a toggle (`true`/`false` read as on/off),
comma-separated for lists, `""` deletes the key. A runner is written under its shipped id (`yuzu` →
`eden`).

`Game` (JSON) is the contents of `game.toml` plus:

```json
{"stats": {"hours": 12.5, "play_count": 7, "last_played": "RFC3339 or null"},
 "media": {"box_front": "path|null", "square": null, "banner": null, "background": null, "logo": null,
           "screenshots": ["path"]},
 "effective": {"runner": "dolphin", "runner_name": "Dolphin", "runner_kind": "emulator",
               "runner_path": "/…/bin/dolphin-emu", "platform": "Nintendo GameCube",
               "options": {"batch": true, "user_directory": ""},
               "proton": "proton-ge", "proton_path": "…", "esync": true, "fsync": true, "ntsync": true,
               "wayland": true, "hdr": false, "discrete_gpu": true, "dlss_upgrade": false, "fsr4_upgrade": false, "xess_upgrade": false,
               "optiscaler": false, "mangohud": false, "gamescope": true, "gamescope_args": "", "gamescope_resolution": "auto", "gamescope_refresh": "auto", "gamescope_scaler": "", "gamescope_filter": "", "gamescope_sharpness": null, "gamescope_adaptive_sync": "auto", "fps_limit": "auto", "hide_cursor": true, "env": {},
               "working_dir": "/…/games/melee", "prefix": "", "modules": {"capture": {"enabled": true, "cursor": false}}},
 "achievements": {"total": 40, "unlocked": 12},
 "removed": false}
```

`effective` is what the launch will use: the game's own keys over the global defaults, the runner
resolved (`runner_path` empty when its program was not found), the platform the runner implies when
the game sets none, `working_dir` and `prefix` as the launch would make them (the program's folder;
`<prefixes_root>/<id>` for a Proton or Wine game, empty otherwise), and each active module's settings
for this game under `modules` — its defaults, then `config.toml`'s, then the game's own. The game's
own `modules` table stays what `game.toml` holds, like `launch`. The upscaler upgrades are the bools their `auto` came to on this GPU (see
`gpu()`); `gamescope_adaptive_sync` stays `auto`, `on` or `off`, since the screen is only known
at launch. `media.screenshots` is the store's promotional shots: `screenshots/` under
`media/picked/`, then under `media/`. The player's own are `screenshots(id)` (see Screenshots).
`added_at` (RFC 3339, in `game.toml`) is when a store install or `add_game` brought the game in — a
frontend's "recently added"; a Lutris import, a ROM import or a source scan leaves it empty, since
those games were already there. A store install of a game `remove`d earlier clears `removed_at` and `hidden` and
re-stamps it; what was parked under `.archive/` stays parked.
`achievements` counts the game's cached list (see Achievements); both are 0 until its source has
given one.

No method notifies a change: the files are the truth, and what the CLI, `session-end` and the hooks
write shows up only there. See Changes.

## Game data

One game's data sits in up to seven places: its install folder (`source.dir`), its Wine prefix,
its saves, the backups of those, Universe's own folder (`games/<id>/`: `game.toml`, media,
screenshots, journal, sessions), its recordings (`<recordings_root>/<id>`, `.archive/<id>` once
removed) and its logs (`<state>/logs/<id>`). The core indexes them where they are and moves nothing
by itself: a new Proton or Wine game gets `<prefixes_root>/<id>` (Steam's `shared_prefix` hands
out its own compatdata instead, which stays Steam's), and each game's backups go under
`<saves_root>/<id>`.

| Rust | Python | CLI | Role |
|---|---|---|---|
| `game_data(id)` | `game_data(id)` | `universe data <name>` | where the game's data is and how big, below |
| `storage()` | `storage()` | `universe storage` | every root with its size and free space, the games by size, the leftovers, below |
| `disk_free()` | `disk_free()` | — | `storage()`'s roots with `free` and `size` and no `bytes`: statvfs alone, no folder walked, for a home screen |
| `trash_leftover(path)` | `trash_leftover(path)` | `universe storage trash <path>` | one of `storage()`'s leftovers to the trash; any other path is `Invalid`. A removed game's folder takes the game out of the library for good |
| `move_prefix(id)` | `move_prefix(id)` | `universe data <name> move-prefix` | the prefix to `<prefixes_root>/<id>`: a rename, or across filesystems a copy (symlinks kept as symlinks: `dosdevices/z:` points at `/`), checked file for file and byte for byte, then the original to the trash (`left` names it when the trash refuses). Every library game on the prefix gets the new `launch.prefix`. Refused for Universe's own, for Steam's compatdata, for Wine's default prefix, for a folder with no `drive_c` (no Wine prefix), a root, a home or the games root, while the game or a sharer runs, and when the target exists. `{from, to, copied, left, games, owner}`: `owner` is the prefix's before the move (`lutris`, `elsewhere`), whose launcher keeps pointing at the old path, which every frontend says before and after the move |
| `reset_prefix(id)` | `reset_prefix(id)` | `universe data <name> reset-prefix` | the game's saves backed up, then its prefix to the trash: the next launch makes a fresh one. Only Universe's own prefix, used by no other game. `{trashed, backup}` |
| `prefix_tool(id, tool, args)` | `prefix_tool(id, tool, args=[])` | `universe data <name> winecfg\|winetricks [verbs]\|run <exe> [args]\|kill` | `winecfg`, `winetricks` and `run`: a transient unit `universe-prefix-<id>-<tool>-<stamp>` running the tool in the game's prefix with the launch's environment, no gamescope: `umu-run winecfg`, `umu-run winetricks …`, `umu-run <exe>` for Proton; `wine winecfg`, `winetricks` with `WINE` set, `wine <exe>` for Wine. Winetricks with no verbs opens its GUI (`--gui`): umu-run refuses it bare; verbs given pass as they are. `{tool, unit}` once the tool's own process runs in the unit (`winecfg.exe`, `winetricks`, the exe's name), or after 60 s at most: umu sets its container up first. A unit that ends before then on a non-zero exit is `Io`: the tool, the exit status and the last line it wrote (winetricks exiting 127); a failure later goes unreported. A program that is not installed (and is no tool Universe fetches, as umu-run is) is `Unavailable`, named, and nothing starts. `kill`: what `wineserver -k` does, from the core itself: every wineserver serving the prefix gets SIGINT (it kills the prefix's programs, saves the registry and exits), SIGKILL after 5 s. A server is found by its working directory, Wine's `server-<dev>-<ino>` named after the prefix, so one in a sandbox's private `/tmp` (umu on NixOS) is found too, whatever started it, and no other prefix's. `{tool, stopped}`, `stopped` the servers stopped (0: nothing ran); `Busy` when one outlives SIGKILL |
| `saves_backup(id)` | `saves_backup(id)` | `universe saves backup <name>` | a backup now, below; `Outcome {change, files: [{path, bytes}], bytes}`, `change` ludusavi's `new`, `different` or `same` (unchanged saves make no backup), `none` when it found nothing |
| `saves_restore(id, backup)` | `saves_restore(id, backup="")` | `universe saves restore <name> [backup]` | a backup's saves put back, the latest when `backup` is empty; refused while the game runs |
| `saves_export(id, to)` | `saves_export(id, to)` | `universe saves export <name> [dir]` | every backup of the game zipped into `<to>/<id>-saves-<stamp>.zip`, laid out as ludusavi lays them out: unpacked, any ludusavi restores from it |
| `saves_cloud(id, action)` | `saves_cloud(id, action="status")` | `universe saves cloud <name> [--download\|--upload\|--keep-local\|--keep-cloud] [-y]` | the game's cloud saves through its source's `cloud-saves` verb (see Cloud saves): `status` reads the last sync's state, `download` and `upload` sync now, `keep-local` and `keep-cloud` settle a conflict. A `cloud` object; `Unavailable` for a game whose source has no `cloud-saves` capability; any action but `status` is refused while the game runs |
| — | — | `universe saves list <name>` | the saves found and the backups kept: `game_data`'s `saves` |

`game_data(id)`:

```json
{"id": "hollow-knight", "title": "Hollow Knight", "runner_kind": "proton", "total": 9300000000,
 "install": {"path": "/mnt/games/gog/Hollow Knight", "bytes": 8300000000, "exists": true, "owner": "universe"},
 "prefix": {"path": "/mnt/games/prefixes/hollow-knight", "bytes": 574000000, "exists": true,
            "owner": "universe", "shared_with": [], "movable": false, "target": "/mnt/games/prefixes/hollow-knight"},
 "saves": {"engine": "ludusavi", "name": "Hollow Knight", "files": [{"path": "…/user1.dat", "bytes": 120000}],
           "bytes": 5100000, "error": "", "folder": "", "title_id": "", "dir": "~/.local/share/universe/saves/hollow-knight",
           "backups": [{"id": "backup-20261004T100051Z", "name": "Hollow Knight", "when": "RFC3339", "bytes": 5100000, "path": "…"}],
           "backups_bytes": 5100000, "auto": true, "keep": 5,
           "cloud": {"enabled": true, "state": "synced", "message": "", "at": "RFC3339", "locations": [{"name": "saves", "path": "…"}]}},
 "universe": {"path": "…/games/hollow-knight", "bytes": 3100000, "parts": {"media": 3000000, "screenshots": 0, "journal": 0, "sessions": 2048}},
 "recordings": {"path": "~/Videos/universe/hollow-knight", "bytes": 0, "exists": false, "archived": false},
 "logs": {"path": "…/logs/hollow-knight", "bytes": 0, "exists": false}}
```

`install` is `null` without a `source.dir`; its `owner` is the store that uninstalls it (see
`uninstall_via`), else `universe`. `prefix` is `null` for a Linux or emulated game; its `owner` is
`universe` under `prefixes_root`, `steam` inside `steamapps/compatdata` and `wine` for Wine's default
prefix (`~/.wine`, `$XDG_DATA_HOME/wine/default`, which plain wine and other launchers share), both
never moved, `lutris` for an import, else `elsewhere`; `shared_with` names the other library games on it, `movable` whether
`move_prefix` would take it. `saves.engine` is `ludusavi` for a PC game, `emulator` for an emulated
title whose saves are known one by one, and empty where only the emulator's `folder` is known;
`error` says why the saves could not be listed (ludusavi not installed yet, a title it does not
know). `saves.cloud` is `saves_cloud(id, "status")`, `null` for a game whose source syncs no cloud
saves. Sizes walk the folders, and the saves ask ludusavi (about a second): a frontend calls it off
its UI thread. `total` adds the parts and the backups.

`storage()`:

```json
{"roots": [{"id": "games", "path": "~/Games", "bytes": 412000000000, "free": 1200000000000, "size": 2000000000000, "exists": true}],
 "games": [{"id": "…", "title": "…", "bytes": 0, "install": 0, "prefix": 0, "universe": 0, "recordings": 0, "saves": 0, "logs": 0}],
 "leftovers": [{"kind": "prefix", "path": "/mnt/games/prefixes/cyberpunk-2077-bak", "id": "cyberpunk-2077-bak", "title": "", "bytes": 9100000000}],
 "leftover_bytes": 9100000000}
```

`roots` are `games`, `prefixes`, `saves`, `recordings`, `library` (`<data>/games`), `components`
and `logs`, each with the free space of the filesystem it is on (its nearest existing folder). The
games come biggest first, a shared prefix counted for each of its games. A leftover is a folder no
library game holds: `prefix` (under `prefixes_root`, no live game's prefix), `recordings` (under
`.archive/`), `game` (the `games/<id>/` of a removed game) and `logs` (a removed game's). Nothing goes
to the trash unless asked. Walking every install takes seconds: off the UI thread, and `disk_free()`
for a home screen.

### Saves

PC saves are ludusavi's (MIT; its manifest is compiled from PCGamingWiki): a fetched tool (see
Fetched tools), with its own config, manifest and cache in `<data>/ludusavi/`, apart from a ludusavi
the player runs. Each call writes `config.yaml` there whole (JSON, which is YAML) under a lock every
process takes: no detected roots, the install folder's parent as an `other` root (saves a game keeps
beside its files), the backup path, `full` retention `saves.keep` and no differential backups.
A prefix is passed as `--wine-prefix` (its `pfx/` when it has one, Steam's layout) and redirected
both ways to `/universe/prefix`: a backup records the prefix's files at that path and a restore puts
them in the prefix the game has now, so a backup outlives a `move_prefix` or a reset. The title is
ludusavi's: found once with `ludusavi find` by the Steam id (`metadata.steam_appid`, a Steam game's
`source.id`), the GOG id, then the title give or take an edition, and kept in `game.toml` as
`saves.name`, which `set <name> saves.name="…"` changes for a title it does not find. ludusavi
updates its manifest at most once a day; a failed update is no error.

Emulated titles are a custom game of ludusavi's, named after the title, its files the title's own
saves, where the title's id is cheap to read:

| Runner | Title id | Saves |
|---|---|---|
| RPCS3 | `TITLE_ID` of `PS3_GAME/PARAM.SFO` | `~/.config/rpcs3/dev_hdd0/home/*/savedata/<id>*` |
| shadPS4 | `TITLE_ID` of `sce_sys/param.sfo` | `~/.local/share/shadPS4/home/*/savedata/<id>`, `…/user/savedata/*/<id>` |
| Vita3K | the game's exe, a title id | `<pref-path>/ux0/user/00/savedata/<id>` (`config.yml`'s `pref-path`, else `~/.local/share/Vita3K/Vita3K`) |
| Cemu | `title_id` of `meta/meta.xml` (an `.rpx`'s) | `<mlc>/usr/save/<high>/<low>` (the `mlc` option, `settings.xml`'s `mlc_path`, else `~/.local/share/Cemu/mlc01`) |
| Dolphin | the disc header's game code and maker (`.iso`, `.gcm`, `.ciso`, `.wia`, `.rvz`) | GameCube: `<user>/GC/*/Card */<maker>-<code>-*.gci`; Wii: `<user>/Wii/title/00010000/<code in hex>/data` |
| melonDS, mGBA | — | the `.sav` beside the ROM |

The others show the emulator's save folder and back nothing up: Eden's `nand/user/save`, Ryujinx's
`bis/user/save`, PCSX2's and DuckStation's memory cards, PPSSPP's `PSP/SAVEDATA`, Azahar's `sdmc`,
Mupen64Plus's, ScummVM's and xemu's. A backup or an export never holds a key, firmware, BIOS or a
system title: they are only ever the title's own files.

With `saves.auto_backup` (on by default), `session_end` starts `universe saves backup <id> --auto` as
the unit `universe-saves-backup-<session>` once the session-end hooks ran, for a game whose saves
can be backed up; `--auto` passes over a game ludusavi does not know without an error. A first
backup fetches ludusavi and its manifest, too long for `ExecStopPost`. `remove(id, purge)` and
`reset_prefix` back the saves up before the prefix goes, and refuse when that backup fails; a
prefix with no `drive_c`, or a game with nothing to back up, goes on.

## Changes

| Rust | Python | CLI | Role |
|---|---|---|---|
| `changes::watch(core, options)` | — | — | watches the files and hands back a `Watch` and a channel of `Event`s; `Watch::check()` reads the session marker at once. It lives until the `Watch` or the receiver is dropped |
| `changes::sweep_wake(next, now)` | — | — | how long until the sweep runs again for the `next` instant `sweep_journals()` answered: a minute at least, half an hour at most, `None` for `""` |

`core` is an `Arc<Core>`, and the call must run inside the tokio runtime the watch then runs on.
Only what changes after it returns is reported, so a frontend starts it before reading the library.
The Python host derives the same events itself (`docs/frontends.md` § Changes).

| Event | When |
|---|---|
| `Library(ids)` | a game's directory, `game.toml` or `sessions.jsonl` moved, or anything the rows below name: those games are reloaded (`reload_game`) before the event, a new or a removed one among them. Empty when `games/` itself went, after `reload_all` |
| `Media(id)`, `Journal(id)`, `Screenshots(id)` | after that game's `Library`: its `media/`, the picks in `media/picked/` included; its `journal/` or `journal/attachments/`; its `screenshots/` |
| `SessionStarted(Current)` | the marker's session is active (`current()`); first of all when one already runs as the watch starts |
| `SessionEnded(Ended)` | the marker went: `{session_id, id, title, duration_s, end, exit, recording}` from the line `session-end` filed (`end` as `sessions::end_of`: `quit`, `stopped`, `crashed`, `killed`, `ended`; empty, the rest zero, when no line was filed), then the game's `Library` |
| `JournalWriting {id, session, title}` | an entry turned `pending` (`title` is the game's); once per session |
| `JournalDone {id, session, state, text}` | a pending entry is no longer: `written` (`text` its title), `deferred` or `failed` (`text` the reason, `timed out` included) |

The watched directories are `games/`,
`games/<id>/{,journal,journal/attachments,media,media/picked,media/picked/screenshots,screenshots}`
and `state/`. Writes are debounced (`Options.debounce`, 300 ms) into one reload per burst, and a directory
that appears (a new game, its first `journal/` or pick) is watched then and reported as
changed: what landed in it before the watch was not seen. The marker is polled (`Options.poll`,
2 s) for as long as one exists — the game is a systemd unit, not a child, and `launch` writes the
marker before the unit starts. A unit gone while its marker stays is a session still ending
(`session-end` files the line, then removes the marker); five seconds on, it is reported ended all
the same.

With `Options.journal_sweep` (the default) the watch also keeps the owed entries moving, the retry
loop the frontends would otherwise run: whenever nothing is being written it calls
`sweep_journals()`, waits for the entry that started to turn pending before it starts another (a
minute at most), and sweeps again at `sweep_wake(next)`. A session's end holds the sweep ten seconds
for the entry the session's own `post-process` may be starting, and pending entries are reread
every 10 s, since the 30-minute timeout writes no file.

## Sessions

| Rust | Python | CLI | Role |
|---|---|---|---|
| `launch(id, screen, splash)` | `launch(id, screen, splash="")` | `universe play <name> [--screen DP-1] [--no-wait]` | pre-launch hooks, marker, `StartTransientUnit` on the user manager, post-launch hooks; returns the `session_id` at once. `screen` is a DRM connector name or `""` for the first the display server is drawing on (`status` `connected` **and** `enabled` not `disabled`, alphabetical; the cabled ones when that leaves none) — a cable to a dark monitor is not a screen a recorder can name; `splash` a poster for gamescope's keep-alive window (see Gamescope) or `""`. `Busy` if a session is already running |
| `stop(session_id)` | `stop(session_id)` | `universe stop` | SIGTERM to the game's processes (those under `universe splash`; every process of the unit when there is no gamescope of its own), up to 10 s for them to exit on their own terms — an emulator saves its caches — then a stop job on the unit, waited for 10 s at most. The SIGTERM is repeated every 3 s unless the runner's spec says once (`term_twice`): Dolphin takes the first as a "quit?" prompt and needs the second; Eden's handler resets to the default disposition, so a second would kill it mid-shutdown. Eden with its `confirmStop` at the default asks "close?" on the first and is killed by the stop job: doctor's `runner-eden-stop` says so |
| `session_window()` | `session_window()` | `universe session-window [--json]` | the running game's window as the desktop lists it (see Desktop; `{id, pid, wm_class, title, focused, x, y, width, height, hidden, minimized}`, `id` the desktop's own handle as a string): the largest visible toplevel whose pid is in the unit's cgroup — gamescope's when the game runs inside it. `None` before it maps; `Unavailable` where the profile has no window list |
| `wait_session_window(session_id, timeout)` | `wait_session_window(session_id, timeout_ms)` | `universe session-window --wait <secs> [--json]` | blocks until that window is up, then `Activate`s it (focus and raise) and returns it; `None` when the session ended first or the timeout ran out (the CLI prints `null`, exit 0); `Unavailable` where the profile has no window list, at once. Polls the desktop every 150 ms |
| `focus_session()` / `focus_pid(pid)` | `focus_session()` / `focus_pid(pid)` | — | `Activate` on the game's window / on the largest window of a process (a frontend's own, once the game is gone). On the launcher's gamescope (see Gamescope) `focus_session` shows the game again and `focus_pid(own pid)` takes the screen back from it |
| `host_focus()` / `summon()` | `host_focus()` / `summon()` | — | `{launcher, session}`: whether the desktop's focused window is the launcher's or its running game's; `summon` raises the launcher's window (gamescope's when it runs nested). See Desktop |
| `freeze(on)` | `freeze(on)` | — | `FreezeUnit` / `ThawUnit` on the running game's unit: every process of it stops in place, then the `freeze` / `thaw` hooks run (the capture module pauses its recorder). A stop job thaws on its own, so `stop` works on a frozen game |
| `volume(change, value)` | `volume(change, value=0)` | — | the default sink through `wpctl`: `up` / `down` by `controller.volume_step`, `mute` toggles, `set` to `value` percent, `get`; returns `{percent, muted, output}` |
| `outputs()` / `set_output(id)` | `outputs()` / `set_output(id)` | `universe output [<id>] [--json]` | the playback outputs, from one `pw-dump`: each output route of a card whose `available` is not `no` (jacks without detection say `unknown`) that the card's current profile plays or an available profile could, then each sink no route stands for (a filter, a pro-audio profile). `[{id, label, device, current}]`: `id` is `<device.name>/<route name>`, or the sink's `node.name`; `label` the route's description, `device` the card's; `current` marks the effective default sink's route (`default.audio.sink`, not the configured one, which can name a sink of a profile gone). `set_output` switches the card's profile when the route needs it (the one keeping the current input, else the highest priority), waits up to 3 s for the sink to come up, sets its route with `wpctl set-route`, then `wpctl set-default`: WirePlumber keeps it, the desktop follows. Returns the new sink's `volume("get")`; `Invalid` for an id not listed |
| `set_fps_limit()` | `set_fps_limit()` | — | rewrites the running game's `<state>/MangoHud.conf` from its `fps_limit` as launch resolves it: the layer watches that file (inotify) and rereads it by itself, a frozen game on the thaw, so no key is typed and no uinput is needed |
| `set_mangohud(on)` | `set_mangohud(on=None)` | — | the running game's HUD: `None` flips it. Written as the game's `launch.mangohud` (reread from disk first: the dock and the watcher each hold a library), then applied in the game — mangoapp told over its control queue where one draws (see MangoHud), the layer over its control socket elsewhere, each also through its conf, which it rereads — and the new state returned. `NotFound` without a session; `Unavailable`, nothing written, when nothing can draw the HUD (no mangoapp where one would, no `mangohud` for the layer) |
| `nest()` / `nest_game_shown()` / `nest_overlay(window, input, opacity)` / `nest_frame()` / `nest_filter(filter, sharpness)` | `nested()` / `nest_game_shown()` / … | — | the gamescope this process runs in (see Gamescope): whether there is one; whether it shows a window of the running game's; `STEAM_OVERLAY` on a window of this process, with its `STEAM_INPUT_FOCUS` and `_NET_WM_WINDOW_OPACITY`; the game's last painted frame into `<state>/frame.png` (`None` when no paint came within 5 s); `GAMESCOPE_SCALING_FILTER` and `GAMESCOPE_FSR_SHARPNESS` (no sharpness deletes the card: gamescope reads its default, 2, back). `Unavailable` on the desktop |
| `under_steam()` | `under_steam()` | — | inside Steam's gamescope (Game Mode, see Gamescope): Steam owns power, sound, screenshots and the HUD there |
| `nest::session()` | `session()` | — | inside the Universe session a display manager started (see Gamescope): the launcher is alone on the screen, and quitting it logs out |
| `power_actions()` / `power(action)` | `power_actions()` / `power(action)` | — | logind on the system bus. `power_actions()` lists which of `suspend`, `reboot` and `power_off` its `Can*` does not answer `no` or `na` (`inhibited` and `challenge` stay: the call says why, or polkit asks); empty when logind cannot be asked. `power(action)` calls `Suspend` / `Reboot` / `PowerOff` interactive, so a desktop's polkit agent may ask for a password; logind's refusal is `Unavailable` with its message, an unknown action `Invalid` |
| `host_gamescope(screen)` | `host_gamescope(screen)` | — | the gamescope a launcher starts itself in: `[env, MANGOHUD_CONFIGFILE=<state>/mangoapp.conf, XKB_DEFAULT_LAYOUT=…, XKB_DEFAULT_VARIANT=…, gamescope, args…]` from `launch.gamescope_bin`, the global `gamescope_*` fields at the screen's mode, `launch.gamescope_args`, `--mangoapp` whenever mangoapp is installed and `--hdr-enabled` when `launch.hdr` is, `--prefer-output` and `--hide-cursor-delay` when it gets the screen to itself (see Gamescope), the keyboard layout (see below); writes that conf with the HUD hidden (a game shows it). `None` when the binary is not installed |
| `keyboard_layout()` | `keyboard_layout()` | — | `{layout, variant}`, the session's xkb keyboard layout (`fr` / `bepo`, `us` / `intl`…): `XKB_DEFAULT_LAYOUT` and `XKB_DEFAULT_VARIANT` when set, else what the desktop keeps — GNOME's `org.gnome.desktop.input-sources` (the most recently used source, else the first), Cinnamon's `org.cinnamon.desktop.input-sources`, Hyprland's `input:kb_layout`, KDE's `kxkbrc`, the active layout sway (`swaymsg -t get_inputs`) and niri (`niri msg keyboard-layouts`) name, turned back into its code through xkeyboard-config's `rules/evdev.lst` (`XKB_CONFIG_ROOT`, else `X11/xkb` or `xkeyboard-config-2` under `XDG_DATA_DIRS`) — else `localectl`'s X11 layout or the console keymap up to its charset, else `us`. gamescope builds a US keymap of its own whatever the session's, so both the launcher's gamescope and a game's own get it as `XKB_DEFAULT_LAYOUT` / `XKB_DEFAULT_VARIANT` (which libxkbcommon reads), and a frontend draws its on-screen keyboard from it |
| `adopt_scope()` | `adopt_scope()` | — (`universe play` does it unless `--no-wait`) | moves the calling process into the transient scope `universe-launcher-<pid>.scope` (`StartTransientUnit` on the user manager) and returns its name; every later `launch` binds the game to it. Idempotent. `Unavailable` without a user systemd |
| `screenshot()` | `screenshot()` | `universe screenshot` | runs the `screenshot` hook of each enabled module declaring one (the screenshot module), the first path printed winning; returns the PNG path (see Screenshots). `Unavailable` when no module is on for the game, `Io` naming each hook's last stderr line when every one failed |
| `current()` | `current()` | `universe status` | `{session_id, id, title, unit, screen, started_at, gamescope_pid, launcher_pid}` (`gamescope_pid` is the launcher's gamescope the game was started into and `launcher_pid` that launcher, both `0` for a gamescope of the game's own), or `None`. The CLI wraps it: `status --json` prints `{"current": … or null, "recent": [the 10 newest session rows across the library], "pending_journals": [see Journal]}` |
| `sessions(id)` | `sessions(id)` | `universe sessions <name>` | `[SessionRow]`, newest first (by `ended_at`); `id = ""` spans every visible game (not removed, not hidden) |
| `session_log(id, session_id, tail)` | `session_log(id, session_id="", tail=0)` | `universe logs <name> [session] [-n N] [-f]` | what a session's processes wrote (see Logs): `[{time, source, priority, message}]`, oldest first, the last `tail` lines (0: all). `session_id` `""` is the running session, else the newest played; `NotFound` for a session that never was, `[]` once the journal has let the unit go, and for an import |
| `session_end(id, session_id, exit, ended)` | — | `universe session-end <id> <session>` | closes the session, idempotent. Run by systemd's `ExecStopPost`, or by reconciliation |

One `sessions.jsonl` line:

```json
{"session":"20260910-213045","game":"the-technomancer","started_at":"RFC3339",
 "ended_at":"RFC3339","duration_s":1234,"source":"universe",
 "unit":"universe-game-the-technomancer-20260910-213045.service","screen":"DP-1",
 "exit":0,"stopped":false,"command":"gamescope -f … -- universe splash -- umu-run …",
 "recording":"path or null","recording_duration_s":1230,
 "recording_started_at":"RFC3339 or empty","recording_pauses":[["RFC3339","RFC3339"]]}
```

`recording_duration_s` is the media's length as `ffprobe` reported it when the file was filed or
imported, `0` when unknown (older lines lack the key). `recording_started_at` and
`recording_pauses` are the recorder's clock against the wall's: when it began, and the stretches it
skipped while the game was frozen (the capture module pauses on `freeze`, see Recordings) — a
wall-clock moment maps onto the file at `moment − started_at − the pauses before it`. Empty and
`[]` when unknown (an import, an older line). A `SessionRow` is the line with what every
listing joins onto it, in the line's place:

```json
{"session":…, "game":…, "title":"The Technomancer", …, "end":"quit",
 "recording":{"path":"…/20260910-213045.mkv","size":2147483648,"exists":true,"duration_s":1230,
              "started_at":"RFC3339 or empty","pauses":[["RFC3339","RFC3339"]]} or null,
 "journal":{"state":"written","title":"Into the Dome","written_at":"RFC3339"} or null,
 "debug_log":"path or null"}
```

`recording` stands for the file (`duration_s`, `started_at` and `pauses` are the line's
`recording_*` keys); `journal` is the entry's state, `title` and `written_at` (see Journal),
`null` when the session has none; `command` stays on the line (the log's first line, see Logs).

`source ∈ universe, import-recording, import-lutris`. `exit` is the main process's exit code, `-1`
when it was killed by a signal; `stopped` says whether `stop` asked for that end. The row's `end`
reads both: `quit` (0), `stopped` (asked), `crashed` (a code), `killed` (a signal nobody asked
for: the launcher's scope went, the OOM killer), `ended` (a signal on a line older than `stopped`).
The CLI tables and every look say so; the end-of-session toast names a crash or a kill. What
`exit` sees is the unit's main process — gamescope or umu-run — so a game that dies inside a
wrapper that exits 0 files as `quit`: the log tells.

### Logs

A game's output is the systemd user journal's: the transient unit writes nothing else, so every
process under it — gamescope, umu-run, pressure-vessel, Wine, the emulator, MangoHud, and
`session-end` with its hooks as `ExecStopPost` — lands under `_SYSTEMD_USER_UNIT=<unit>`, and
stays as long as journald keeps it (`doctor`'s `journal-persistent` wants `/var/log/journal` on
disk, else a reboot drops it). `session_log` reads it back (`journalctl --user -u <unit> -o json`,
colours stripped, non-UTF-8 lines dropped) with the launched command line as a first line from the
session's `command`, which survives the journal. `universe logs <name>` prints the newest session's
(or the running one's), `universe logs <name> <session>` an older one's, `-f` follows the running
one, `--json` the rows; `universe sessions` and `status` carry the `End` column, and a failed
`universe play` points at `logs`. In the UI a game's Sessions (Reprise: the Sessions pill on its
details, or More → Sessions and logs; Switch 2: Software Options → Play Log) lists every session
with its end and opens the log's last 400 lines, the running session first.

`launch.debug_log` (global or per game, Proton and Wine only, off by default) adds the runners'
own verbose logs for the session: `PROTON_LOG=1` with `PROTON_LOG_DIR` (Proton's
`steam-<GAMEID>.log`, its `WINEDEBUG` set), `UMU_LOG=debug`, `DXVK_LOG_PATH` (`<exe>_d3d11.log`,
`_dxgi.log`…), and for plain Wine `WINEDEBUG=+timestamp,+pid,+tid,+seh,+debugstr,+loaddll,+mscoree`,
`DXVK_LOG_LEVEL=info`, `VKD3D_DEBUG=warn`. The files land in
`$XDG_STATE_HOME/universe/logs/<id>/<session>/` beside a `launch.txt` (the command line, then the
unit's environment); the row's `debug_log` is that directory when it exists, and `remove --purge`
trashes `logs/<id>/`. A key of `[launch.env]` with the same name wins over the switch.

### Gamescope

A launcher that runs **inside** gamescope is the one window: `universe-ui` fullscreen starts
`gamescope` around itself (`host_gamescope`: `-f --force-composition -W -H -w -h -r` from the screen's
mode and the global `gamescope_*` fields, `launch.gamescope_args`, `--mangoapp` when installed)
and runs itself as its child (on the desktop instead when gamescope fails to bring it up), and every game it launches lands on that gamescope: the plan is
the plain command — no gamescope of the game's own, no `splash`, no `setpriv` — with the launcher's
`DISPLAY`, `GAMESCOPE_WAYLAND_DISPLAY`, `STEAM_GAME_DISPLAY_0`, `SDL_VIDEODRIVER` and
`SDL_VIDEO_MINIMIZE_ON_FOCUS_LOSS` passed to the unit, MangoHud's layer silent (`no_display`, the
limit still applies) since mangoapp draws the HUD, `PROTON_ENABLE_WAYLAND` dropped. The marker keeps
`gamescope_pid` and `launcher_pid`, so `session_window` from a hook's process returns the
gamescope's toplevel (what shows the game) and `wait_session_window` waits until gamescope shows
the game's window inside. The game's windows are those of the processes in the session's unit (its
cgroup): another program's, a store client's the session started, is not the game's, so
`focus_session` does not bring it up and gamescope showing it does not end the wait. A source that
waits on the user in a window of another program says so with a `window` event (see Sources);
inside gamescope the core then keeps that window the one shown among the program's windows until
the verb ends, since gamescope shows the newest one mapped and the program's main window would
cover its dialog.

The gamescope the launcher starts for itself carries `UNIVERSE_OWN_GAMESCOPE` to its children:
`drm` when it drives the screen (a session of its own: no `WAYLAND_DISPLAY` or `DISPLAY` above it),
`nested` in a desktop's window. Any other gamescope the launcher finds itself in is someone else's.
On the screen of its own, `host_gamescope` adds `--prefer-output` with the connector the mode was
read from, so gamescope drives the screen `-W` and `-H` describe, and `--hide-cursor-delay 3000`,
since no desktop hides the cursor there; `launch.gamescope_args` setting either wins.

#### The Universe session

`packaging/system/universe.desktop` is a login session (`share/wayland-sessions`, `Exec=universe-ui
--session`, `DesktopNames=Universe`), picked at the display manager's login screen like SteamOS's
Game Mode. `--session` drops the `WAYLAND_DISPLAY` and `DISPLAY` it inherited, so its gamescope is
always `drm`, and sets `XDG_CURRENT_DESKTOP=Universe` where the display manager did not, so the
desktop profile is `none`. Its gamescope also carries `UNIVERSE_SESSION=1`, which `nest::session()`
reads: the UI's power menus say Log out there, and doctor's `desktop` check passes on `none` and
skips the cursor one.

No desktop is left to fall back to. A gamescope that is missing, exits before the launcher comes up
or shows nothing within 30 s ends the session with 1, and the display manager shows its greeter
again. gamescope exits 0 whatever its child did, so the launcher inside writes its own exit code to
the file `UNIVERSE_HOST_DONE` names when it quits on purpose; one that died instead is started
again with `--no-boot`, and five runs in a row shorter than 60 s end the session. The startup
animation plays once, at login. There is no lock screen, so a resume, or an autologin at boot,
opens on the launcher. Nor is there a Wi-Fi or Bluetooth prompt yet: the session relies on
the connections and pairings made from a desktop session. No polkit agent runs either, so nothing
there can ask for a password: what the session needs root for goes through polkit actions it is
granted outright while it is local and active. Its one such action is
`io.github.ilyasturki.universe.system-install` (see Components › System tools); power, reboot and
suspend are already granted to an active session by logind's own policy, and a block inhibitor held
by another user (PackageKit or fwupd mid-write) keeps refusing them with logind's message, since
overriding it is `*-ignore-inhibit`, which the session is not granted. A recording needs no prompt
either: the capture module's check fails a `gsr-kms-server` without `cap_sys_admin`, which
gpu-screen-recorder would otherwise run through pkexec.

The AUR and RPM packages install it under `/usr/share/wayland-sessions`. `install.sh`, from a
release or a checkout, puts it there through sudo with `Exec` pointing at its own `universe-ui`, under
`/usr/local/share/wayland-sessions` where `/usr` is read-only (SDDM and GDM read both, LightDM only
the first). The packages and `install.sh` ship the system-install helper and its policy with it.
On NixOS `programs.universe.session.enable` adds it to
`services.displayManager.sessionPackages`, running the flake's `universe-ui` (`session.package`);
`services.displayManager.defaultSession = "universe"` with an autologin boots straight into it.
It adds no helper nor policy: on NixOS the system tools come from the module's options
(`gamescope.enable`, `capture.enable`, whose `programs.gpu-screen-recorder` gives `gsr-kms-server`
its capability), so there is nothing for the session to install as root.

#### Steam's Game Mode

Added to Steam as a non-Steam game, the launcher runs inside Steam's gamescope, which Steam starts
with `--steam`: `under_steam()` is a foreign gamescope (no `UNIVERSE_OWN_GAMESCOPE`) whose root
carries `GAMESCOPECTRL_BASELAYER_APPID`, or whose launcher Steam handed a `SteamGameId`. Steam keeps
the Steam and … buttons, its overlay, the HUD, the frame limit, volume, screenshots and power there,
so the core steps aside:

- Focus: under `--steam` gamescope shows the windows of the app Steam focuses. A game started as a
  transient unit has no Steam reaper above it and no app id, and such a window is not even on
  `GAMESCOPE_FOCUSABLE_WINDOWS`: the core finds the game's windows on the X tree instead (mapped
  top-levels whose `_NET_WM_PID` runs in the session's unit, by its cgroup) and stamps the
  launcher's app id (`STEAM_GAME` of the launcher's window, else its `SteamGameId`) on them, which
  gamescope then shows as the app's newer window. `wait_session_window` stamps as it waits, so the
  game comes up as its window maps; `focus_session` does the same, `focus_pid` puts 0 back, and
  `nest_game_shown()` counts only those windows. Steam's own UI is never touched.
- The plan: no MangoHud layer and no limit (`mangohud` false, `fps_limit` none), and the launcher's
  mangoapp is not told a thing, since the SysV queue would reach Steam's.
- `set_mangohud`, `set_fps_limit`, `volume` (all but `get`), `set_output`, `screenshot`,
  `power(action)` and `set_system` are `Unavailable` ("… belongs to Steam in Game Mode");
  `power_actions()` and `system_controls()` are empty; `launch_keys` leaves out `mangohud`,
  `fps_limit` and `pause_on_home`. The watcher drops the volume, mute, MangoHud and screenshot
  macros.
- Steam's Exit Game ends the shortcut, the launcher with it; the game's unit is `BindsTo=` the
  launcher's scope and goes down after it, `session-end` included.

gamescope gives every keyboard the US layout: its keymap comes from `XKB_DEFAULT_LAYOUT` and
`XKB_DEFAULT_VARIANT` alone, which no session sets, so an AZERTY typist gets QWERTY inside it. Both
the launcher's gamescope and a game's own are started with the two set from `keyboard_layout()`,
and the launcher's children inherit them.

The game keeps rendering behind the launcher; `freeze` stops it. The per-game `gamescope_resolution`, `gamescope_refresh`,
`gamescope_scaler` and `gamescope_adaptive_sync` fields cannot reach a gamescope that is already
running: only the global ones apply there, and `gamescope_filter` / `gamescope_sharpness` go through
`nest_filter` at runtime. `pause_on_home` (global or per game, on by default) is the frontend's cue
to `freeze` the game whenever the launcher covers it — its dock, its home menu, its home over the
game — and to `thaw` it on the way back. Nothing else takes the pad away from a running game:
gamescope's `STEAM_INPUT_FOCUS` routes keyboard and mouse only, and the game keeps its evdev and
hidraw readers open, so an unfrozen game answers every press the menu gets. A frozen game's input
queues still fill: an evdev reader replays the last 64 events on thaw (or resyncs past a
`SYN_DROPPED`), a hidraw reader's 64-report buffer fills with the first ~¼ s after the freeze.

A game launched from **outside** gamescope (`universe play` from a terminal) gets a gamescope of its
own, as follows. Every runner's command runs inside gamescope by default: `gamescope -f
-W <screen width> -H <screen height> -w <game width> -h <game height> -r <refresh> [-S scaler] [-F filter]
[--sharpness N] [--adaptive-sync] [launch.gamescope_args] [the game's
gamescope_args] [a pre-launch hook's UNIVERSE_GAMESCOPE_ARGS] [--mangoapp] -- universe splash [--image <poster>] -- <program> <args…>`. One
window, from gamescope's first frame to the game's last, whatever the game, Proton or umu put up
first, and the launcher hands over on it. Left to itself gamescope scans the game's buffer out
directly, no composite of its own per frame; Mutter's window screencast (`capture`'s window source)
blits such a buffer as one flat colour, so for a window recording the capture module's pre-launch
hook writes `UNIVERSE_GAMESCOPE_ARGS=--force-composition`. The launcher's own gamescope
(`host_gamescope`) is up before any game is known and keeps the flag.

On a hybrid machine — the firmware's display GPU (`boot_vga` in sysfs) is not the strongest card,
NVIDIA ranked first, then VRAM — `discrete_gpu` (on by default, global or per game) points the game
at the stronger one: `DRI_PRIME=pci-<slot>` for a Mesa driver (GL and Vulkan alike),
`__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia __VK_LAYER_NV_optimus=NVIDIA_only`
for NVIDIA's own, neither over a value the game's `env` sets; and its gamescope, and the
launcher's (from the global key), get `--prefer-vk-device <vendor>:<device>` unless the flags
already name one. A single GPU, or a display GPU that is already the strongest, changes nothing.

gamescope unmaps its own window whenever no client inside it is focused, and a game that closes
its first window before opening the real one (Dead Cells) would flash the desktop through, so
`universe splash` — gamescope's primary child, the game its child — keeps a window up for the
whole session: a disabled (`_WINE_HWND_STYLE` WS_DISABLED), skip-taskbar X window that gamescope
ranks below any window of the game's, so the game's windows take over the moment they appear and
this one shows only when the game has none. `--image` fills it with a poster the frontend
grabbed — `<width> <height>\n` then width×height×4 bytes of little-endian BGRX (Qt's
`Format_RGB32`), nearest-neighbour scaled onto gamescope's nested screen, the file deleted once
read — so the launcher's poster is on screen from Proton's startup to the game's first frame;
without it (`universe play`) the window is black. Left to itself gamescope's nested screen
is 1280×720 whatever the window covers, so the session screen's mode is passed explicitly: the
output (`-W -H`) is always the screen, and the game's resolution and refresh follow it unless set.
The mode is the connector's `is-current` one from Mutter's DisplayConfig (`GetCurrentState`,
physical pixels — gamescope handles the desktop's scale itself), else the mode the connector's
CRTC scans out, read from the card whatever the desktop (the preferred mode of a connector nothing
drives; `/sys/class/drm/*/modes` at 60 Hz when the card cannot be opened); no screen at all leaves
gamescope's own defaults.
`universe doctor` prints the mode it read. The `gamescope_*` fields, global in `[launch]` and per
game in `game.toml`'s `[launch]` (a field left empty takes the global one), are launch keys:
`universe launch-keys` prints each one's values and default. Each is a flag: `gamescope_resolution`
(what the game renders at, upscaled to the screen when smaller) `-w -h`, `gamescope_refresh` `-r`,
`gamescope_scaler` `-S`, `gamescope_filter` `-F`, `gamescope_sharpness` (for `fsr` and `nis`)
`--sharpness`, `gamescope_adaptive_sync` `--adaptive-sync` (`auto`, the default, when the screen
takes a variable refresh rate: a mode Mutter marks `refresh-rate-mode = variable`, else the
connector's `vrr_capable` DRM property; `on`/`off` regardless); a scaler or filter left empty leaves
gamescope its own default.

`gamescope_args` (global, then the game's) comes after these and wins: gamescope takes the last
of a repeated flag, so `-w 1280 -h 720` there overrides the field. `launch.gamescope` (global),
`[runners.<id>] gamescope` (per runner: an emulator that misbehaves under it) and the game's
`launch.gamescope` switch it off, the game's own key winning; `launch.gamescope_bin` names the
binary (`gamescope` on PATH, `/run/wrappers/bin` included). The game itself runs under
`setpriv --ambient-caps=-all --inh-caps=-all` when util-linux is on PATH: a capability wrapper on
gamescope (NixOS `capSysNice`) hands CAP_SYS_NICE down to the game, and bwrap — umu's runtime —
refuses to start holding one. With gamescope off — or not found: a warning, and the game runs on
the desktop as before — the plain command runs. Inside gamescope
the HUD is gamescope's `--mangoapp` rather than the game's layer when mangoapp is installed, `launch.hdr` adds `--hdr-enabled`, and
`PROTON_ENABLE_WAYLAND` is dropped (Proton goes X11 through gamescope's Xwayland) unless the
arguments carry `--expose-wayland`. `doctor` checks the binary and `mangoapp`.

### MangoHud

Two MangoHuds can be in play, and Universe owns the state of both — nothing depends on
`~/.config/MangoHud/MangoHud.conf` but the layout and the keys. **mangoapp** draws the HUD inside gamescope
when it is installed (Debian and Ubuntu ship it apart, as `mangoapp`): the launcher's own
(`host_gamescope` passes `--mangoapp`, the HUD on or off) or the game's. It reads
`<state>/mangoapp.conf` (`MANGOHUD_CONFIGFILE` on the gamescope, harmless there: the file alone
loads no layer) — the user's lines minus `no_display`, `fps_limit` and `control`,
plus `no_display` when the HUD is off — and is told live over its SysV control queue, the one
`mangohudctl` speaks (key `-1`, what their `ftok("mangoapp", 65)` fails to without such a file in
their cwd; message type 2, `no_display` 1 hides, 2 shows): no key, no focus, and it lands while the
game is frozen. The **layer** inside the game process is loaded whenever it has a job — the limit
anywhere, the HUD itself where no mangoapp draws (the desktop, or a gamescope without mangoapp) — on
`<state>/MangoHud.conf`: the same lines, `no_display` when mangoapp draws or the HUD is off,
`fps_limit` ours, and where no mangoapp draws `control=universe-mangohud-<id>` (an abstract socket
its first Vulkan instance binds; `:hud;` flips it at once, and a frozen game reads it on the thaw).
Every MangoHud also watches its conf (inotify `IN_MODIFY`) and rereads it about 100 ms after a
write — measured: a limit rewritten mid-game holds from then on, and one written while the process
was stopped holds from the thaw — so a write alone is a reload, and the conf has the last word over
the queue and the socket.

`launch.mangohud` is the HUD's state: shown at launch when true — on the launcher's gamescope
mangoapp is told at `begin` and hidden again when the session ends (`Undo::Hud`), between sessions
it stays hidden — and flipped in game by `set_mangohud`, which writes the key back, so the game
reopens as it was left. The user's own `no_display` no longer hides a HUD that is on, and the
`toggle_hud` key MangoHud itself listens to is nothing Universe types or reads.

### Frame rate limit

`fps_limit` — `[launch]`'s, the game's own when set — is `auto`, `none`, or frames per second,
and holds the game to it through MangoHud's limiter inside the game process, overlay or not:
gamescope's `--framerate-limit` paces nothing on a nested gamescope (measured: an uncapped
client stays uncapped, a vsynced one at the refresh, with or without its WSI layer), and its `-r`
only paces clients that vsync. `auto` is the refresh the game sees: its `gamescope_refresh` when
set, else the screen's; unknown (no screen read) means no limit, and so does an emulator runner,
which paces itself (a second limiter on top of its own jitters against it). The launcher writes the layer's
`<state>/MangoHud.conf` before each launch and gives the game `MANGOHUD=1
MANGOHUD_CONFIGFILE=<that>`: on the unit when no gamescope runs there, else through `env` in
front of the program, after `setpriv`, since gamescope (a Vulkan client itself) would draw the
layer. Inside gamescope the layer limits and draws nothing while mangoapp shows the HUD. A change
mid-game (`set_fps_limit`) rewrites the conf, which the layer rereads by itself. A 32-bit game needs
MangoHud's 32-bit layer (`MangoHud.x86.json`), which `doctor` looks for. A native
or emulator program (not one run through Proton) with a limit goes through the `mangohud` wrapper
so an OpenGL game is limited too; Proton and Wine get the Vulkan layer alone, nothing preloaded
into the runtime. No `mangohud` on PATH: a warning, no limit, no layer; `doctor` checks for it
unless the limit is `none`.

### Proton and Wine

`proton` runs `umu-run <exe>` with `WINEPREFIX` (`launch.prefix`, else `<prefixes_root>/<id>`),
`PROTONPATH`, `GAMEID` (`launch.umu_id`, else `umu-default`), `STORE`, then the switches as the env
Proton reads: `esync`/`fsync`/`ntsync` off are `PROTON_NO_ESYNC`/`_FSYNC`/`_NTSYNC=1`; `wayland`,
`hdr`, `dlss_upgrade`, `fsr4_upgrade`, `xess_upgrade` and `optiscaler` on are
`PROTON_ENABLE_WAYLAND`, `PROTON_ENABLE_HDR`, `PROTON_DLSS_UPGRADE`, `PROTON_FSR4_UPGRADE`,
`PROTON_XESS_UPGRADE`, `PROTON_USE_OPTISCALER=1`; on an RDNA 3 card (`gpu()`, below) `fsr4_upgrade`
is `PROTON_FSR4_RDNA3_UPGRADE=1` instead, Proton's variant for it (its own DLL build and
workarounds; the generic one does nothing there). The three upgrades are `auto`, `on` or `off`
(a bool reads as on/off), `off` by default as NVIDIA, AMD and Proton ship them — a swapped DLL can
upset a game or an anti-cheat; `auto` is on where the upgrade is a plain win — DLSS on NVIDIA,
FSR 4 on RDNA 4, XeSS on Intel — and off elsewhere and when no GPU is known.

`launch.proton` names a build: `proton/<name>` under the data home, then `[proton]`'s path, then a
path as given, then `<name>` under each Proton directory — the builds Universe installed
(`components/<id>/`, see Components), Lutris's `runners/wine`, Steam's
`compatibilitytools.d` (native and Flatpak), umu's `umu/compatibilitytools`, Heroic's `tools/proton`
(native and Flatpak) — then the newest build of the name's family in any of them: `proton-ge` is
`GE-Proton10-12` over `GE-Proton9-27`, `proton-cachyos` is `proton-cachyos-10.0-…`. None found is
no error: umu-run downloads one — `PROTONPATH=GE-Proton` for the GE family, the latest GE-Proton
into Steam's `compatibilitytools.d`, where the next launch finds it; any other name leaves
`PROTONPATH` unset (unless `[launch.env]` sets it) for umu's own UMU-Proton, with a warning. `doctor`
says which build, or which download. `settings()`'s `protons` lists the names a game can take here:
`[proton]`'s, then every build in those directories. A switch off removes the same name from
`[launch.env]`; `launch.env` on the game still wins. `launch.dll_overrides` (`d3d11 = "n,b"`, keys
without `.dll`) is `WINEDLLOVERRIDES`. `wine` runs `<launch.runner_exe or wine> <exe>` with
`WINEPREFIX`, `WINEARCH` (`launch.arch`), `WINEESYNC`/`WINEFSYNC` as `1`/`0` and
`WINEDLLOVERRIDES`; the Proton switches do not apply. Both, like every runner, take
`launch.wrapper` — `gamemoderun`, `taskset -c 0-7` — split like a shell line and put in front of
the program, inside gamescope and setpriv.

`migrate` maps Lutris's wine runner onto these: `wine.version` is `wine` with `launch.runner_exe`
when `~/.local/share/lutris/runners/wine/<version>/bin/wine` exists with no `proton` script beside it, `wine` for
`system`, `proton` otherwise; `wine.proton_hdr` is `hdr`;
`system.prefix_command`'s leading `VAR=val` words become `launch.env` (`WINEDLLOVERRIDES` its
`dll_overrides`) and the rest `launch.wrapper`; a `PROTON_*` entry in `system.env` that has a switch
becomes the switch. Lutris's `fps_limit` is `launch.fps_limit`; its `fsr`, `battleye`, `eac`,
DXVK/VKD3D versions and registry options have no counterpart (Proton bundles its own DXVK, and reads
none of those variables).

### Fetched tools

Programs most distributions do not package are fetched by Universe itself when they are not on
PATH: `umu-run` (umu-launcher's zipapp, which needs python3 3.10 or later) before a Proton
launch that uses `launch.umu_run`, `ludusavi` (its Linux build, which needs the system's GTK 3; on
NixOS the package's runs, the fetched one would go through `universe-fhs`) before a save backup, and before any command of a source that requires one of them
`gogdl` (heroic-gogdl's x86_64 build), `legendary` (legendary-gl's Linux build, a python3 zipapp)
and `butler` (itch.io's archive from broth.itch.zone, a zip kept whole: the program loads the 7-Zip
libraries beside it). They are components (see Components): the catalogue's newest build, else the
pinned one when the catalogue cannot be reached or lacks the tool: umu-launcher 1.4.4 and ludusavi 0.31.0 the core's, the
others in the `[[tools]]` of the source that requires them (heroic-gogdl 1.3.0 in GOG's, legendary-gl
0.21.1 in Epic's, butler 15.31.0 in itch.io's). `<data>/bin` holds a link to each tool's newest build; every lookup
of a program searches it after PATH (an installed one wins), and it is appended to the `PATH` of the
modules, the sources and the units Universe starts. A required binary Universe fetches never makes a
source unavailable, and `doctor` reports it as fetched on first use.

## Sources

| Rust | Python | CLI | Role |
|---|---|---|---|
| `sources()` | `sources()` | `universe sources`, `universe source ls` | `[{id, name, version, description, dir, enabled, available, missing: [bin], capabilities: [name], hooks: {}, settings: [Setting], login: {kind, hint, purpose}, logged_in, user, games_dir, library_cached, library_at}]` (`login` with its defaults filled); `library_at` is when the store was last listed (RFC 3339, empty before the first); the login probe reaches the network once per process, on the first call |
| `enable_source(id, enabled)` | `enable_source(id, enabled)` | `universe source enable\|disable <id>` | writes `[sources] enabled` in `config.toml` |
| `source_settings(source, game_id)` | `source_settings(source, game_id="")` | `universe source settings <id> [game]` | the source's settings, defaults under `config.toml [sources.<id>]`, then a game's own game-scope keys (`game.toml [sources.<id>]`) when `game_id` names one; an empty `games_dir` default reads `paths.games_root` |
| `set_source_setting(source, game_id, key, value)` | `set_source_setting(source, key, value, game_id="")` | `universe source set <id> k=v [--game g]` | validated against `[[settings]]`; `game_id=""` writes `config.toml [sources.<id>]`, otherwise a game-scope key into `game.toml [sources.<id>]` (`set(id, "sources.<id>.<key>", value)` is the same write) |
| `source_setting_choices(source, key)` | `source_setting_choices(…)` | — | the setting's choices, live through `choices_exec` (as for modules) |
| `source_login_url(source)` | `login_url(source)` | `universe login <source>` | URL to open |
| `source_login(source, code)` | `login(source, code)` | `universe login <source> <code>` | returns the user name |
| `source_library(source, refresh)` | `library(source, refresh)` | `universe library [source] [--refresh]` | `[SourceGame]`: the store's listing from cache unless `refresh` (the cache is the core's `library.json` under the source's data dir; a failed refresh keeps it), crossed with the disk through the source's `scan` every time: an install's `disk_size`, a stopped download's `partial_dir` and `partial_bytes` |
| `source_search(source, query)` | `search(source, query)` | `universe search <query> [--source]` | `[SourceGame]` |
| `source_info(source, game_id)` | `info(source, game_id)` | — | the source's raw `info` payload, plus the `download_size` and `disk_size` it reports; those two are remembered in the library cache, so a listing carries them from then on |
| `source_cancel(source, game_id)` | `cancel(source, game_id)` | Ctrl-C | SIGTERMs the source process installing or updating `game_id`; it stops its downloader and keeps the files, so the next `install` resumes. False when nothing was running for it. The interrupted `install`/`update` call fails |
| `source_install(source, game_id, progress)` | `install(source, game_id, progress)` | `universe install <id> [--source]` | id of the installed game, whose art the caller fetches after (the CLI and both apps do) |
| `source_update(source, game_id, progress)` | `update(source, game_id, progress)` | `universe update [name] [-y]` | how many were updated; `game_id=""` updates everything pending |
| `source_updates()` | `updates()` | `universe update` | `[{id, title, local_build, remote_build, version, date, source}]`, every enabled source's |
| `source_scan(source, progress)` | `scan(source, progress)` | `universe scan [source]` | the ids of the games that became the source's (one it already had is updated, not listed again), whose art the caller fetches after (`media_refresh_many`; the CLI, both apps' jobs and their first run do); `source=""` scans all |

`progress` is called `(done, total, message)` as the job runs. The CLI's `[source]` and `--source`
may be left out while a single source is enabled.

`SourceGame` = `{"id": "1434554947", "title": "Mini Metro", "owned": true, "installed": true,
"dir": "path|null", "build": "…|null", "remote_build": "…|null", "disk_size": bytes|null,
"download_size": bytes|null, "partial_dir": "path|null", "partial_bytes": bytes|null}`. `disk_size` is what
the install takes (measured) or would take (from `info`); `partial_*` name a download stopped by `cancel`
that `install` resumes.

A game a source reports installed and owned enters the library with `source.kind`, the source's
id, and `source.id`, the store's id of the game (a `game.toml` written before this carried it as
`gog_id`, which still reads). The next report finds it again by that pair, else by its title's
slug among the games no store has claimed (`kind` `manual` or `lutris`), which the source takes over.
A game another store holds keeps its entry: the same title from a second store is a second game,
`<slug>-<source>`. The report's `umu_id`, `store` and `runner` fill `launch.umu_id`, `launch.store` and
`launch.runner` while they are empty (see Proton and Wine). A game entering the library takes the
report's `prefix`, an absolute path, as `launch.prefix` instead of `<prefixes_root>/<id>`, and its
`proton`, one too, as `launch.proton`; a game already there keeps its own.

A source's `Setting` is a module's, `scope: global` unless the manifest says `game`: a game-scope key
holds for the games the source installed or found, each of which can set its own. A module
named to a source call (or the reverse) is refused with the command that does take it.

### Achievements

| Rust | Python | CLI | Role |
|---|---|---|---|
| `achievements(id, refresh)` | `achievements(id, refresh=False)` | `universe achievements <name> [--refresh] [--json]` | `{source, fetched_at, total, unlocked, items: [Achievement]}` from `games/<id>/achievements.json`; through the game's source (its `achievements` verb, when the source declares that capability) when `refresh` or when there is no cache yet. A refresh keeps an unlock the store has not heard of yet. `Unavailable` when no source lists the game's: a manual game, a Lutris one, a source without the capability |
| `achievements_replay(id, count)` | — | `universe achievements <name> --replay [--count N] [--json]` | stamps `replay: {at, keys}` into the cache: the `count` latest unlocks (all by default), oldest first, for the UI's `UnlockWatch` to show again as banners over the running game. Nothing reaches the store or comet: a way to see the banners without a fresh unlock. `Invalid` when the game is not the one running, or has nothing unlocked. A store refresh drops the stamp |
| `achievement_unlocked(id, item)` | — | `universe achievement-unlocked <id> '<json>'` (hidden) | files one unlock a session saw into the cache at once, ahead of the store's list: sets `unlocked_at` (now when empty) on the item with that `key`, or adds the item. Prints whether it is new; the first unlock stands |

`Achievement` = `{"key", "name", "description", "unlocked_at": "RFC 3339, empty while locked",
"hidden": bool, "icon": "url", "icon_locked": "url", "rarity": percent|null}`. `hidden` is the
store's "withheld until unlocked": a frontend masks the name and description of a locked one. The
cache is rewritten by a rename, so the `games/<id>/` watch sees every unlock; the game's
`achievements` summary follows it.

The **gog** source fills it. Its `achievements <gog id>` verb asks GOG's gameplay API with a token
for the game's own Galaxy client (its id and secret are read once from the build manifest and kept
in `clients.json`; gogdl issues the token), paging through the list. Its icons are GOG's 256 px
originals: the API names the 60 px thumbnail (`…_gac_60.jpg`), and the source drops the suffix. For a game with the Galaxy SDK
(`Galaxy.dll`, `Galaxy64.dll`, `libGalaxy.so` or `libGalaxy64.so` in its folder or up to three folders down) and its
game-scope `achievements` setting on (the default), its session hooks run **comet**, the open
implementation of GOG Galaxy's communication service, so the SDK in the game has something to talk
to: `pre-launch` stops any `universe-comet.service` left over and starts one (`systemd-run --user`,
`comet --from-heroic`, its config pointed at a folder whose `heroic/gog_store/auth.json` links to
gogdl's token file, so no token rides on a command line), then waits up to 5 s for its port. comet
listens on 127.0.0.1:9977 alone, so there is one at a time. `post-launch`, bound to the game's unit,
reads comet's store (`$XDG_DATA_HOME/comet/gameplay/<client>/<user>/gameplay.db`) every 2 s and
files each unlock of this session with `achievement-unlocked`; `session-end` stops comet;
`post-process` refreshes the cache from the store. comet syncs an unlock to GOG as it happens, or on
the next run when offline. Without comet installed, or without the SDK in the game, the hooks do
nothing; `doctor`'s `gog-comet` check says when the setting is on and comet is missing. Some SDKs
(Cuphead's) talk only once a `GalaxyCommunication` Windows service is registered, so for a Proton or
Wine game `pre-launch` first copies comet's do-nothing stand-in (`GalaxyCommunication.exe` in the
source's folder, fetched from comet's release by the flake) to
`C:\ProgramData\GOG.com\Galaxy\redists\` in the prefix, then runs `sc create GalaxyCommunication`
and sets `HKLM\SOFTWARE\WOW6432Node\GOG.com\GalaxyClient\paths` through the game's runner
(`PROTON_VERB=run`) in a `systemd-run --user --wait` unit, since bwrap refuses the capabilities a
hook inherits under gamescope, each at most 10 s and only while `system.reg` lacks it. A prefix not made yet
gets it on the next launch.

The **epic** source lists them too, after the fact: `achievements <app name>` is `legendary
achievements --json` (the store's list with the player's unlocks, each group of it), and its
`post-process` hook refreshes the cache after every session while the game-scope `achievements`
setting is on. Nothing watches a session: an unlock shows once the session has ended.

The **steam** source reads them from the client's own cache, no key needed:
`appcache/stats/UserGameStatsSchema_<app id>.bin` (binary KeyValues: the achievement stats' bits,
each with its name, its display name and description in every language — the client's language is
used, else English —, `hidden`, and the icon hashes, served under
`shared.fastly.steamstatic.com/community_assets/images/apps/<app id>/`) and
`UserGameStats_<account id>_<app id>.bin` (each stat's bitmask and `AchievementTimes`, the unlock
times by bit; an unlock without one takes the file's time). Rarity is Steam's global percentage
(`GetGlobalAchievementPercentagesForApp`, keyless). A game the client has never loaded has no schema
yet, and lists nothing. Its `post-process` hook refreshes the cache after every session, as Epic's.

### Cloud saves

A source with the `cloud-saves` capability syncs a game's saves with its store: down in `pre-launch`,
up in `post-process`, while the game-scope `cloud_saves` setting is on (off by default; the global
switch is `universe source set <id> cloud_saves=true`, a game's own `--game <name>`). Only a Windows
build run through Proton or Wine syncs. A sync never settles a conflict by itself: when the saves
changed on both sides since the last sync, the game launches on the saves it has, the state becomes
`conflict`, and the player keeps one side (`saves_cloud(id, "keep-local")` or `"keep-cloud"`, on the
game's Data page in every frontend). The verb runs with the game's hook environment and settings and
fetches no tool first, as the hooks do. The
ludusavi backup (see Saves) is taken before a game's first sync and before `keep-cloud`, and a
failed backup stops the sync.

`cloud` = `{"enabled": bool, "state": "", "message": "", "at": "RFC 3339", "locations": [{"name", "path"}]}`.
`enabled` is the setting for the game; `state` is empty before the first sync, else `synced`,
`conflict`, `offline` (the store was out of reach), `error` (`message` says why) or `unsupported`
(the store keeps no cloud saves for the game, or the build is not a Windows one); `locations` are
the folders the last sync resolved.

The **gog** source drives `gogdl save-sync`. The folders are those of the game's Galaxy client in
GOG's remote config (`remote-config.gog.com/components/galaxy_client/clients/<client id>`, the id
read from the install's `goggame-<id>.info`; kept in `cloud-locations.json`), Galaxy's
`__default` folder when the game names none. Their `<?DOCUMENTS?>`, `<?APPLICATION_DATA_LOCAL?>`,
`<?APPLICATION_DATA_LOCAL_LOW?>`, `<?APPLICATION_DATA_ROAMING?>`, `<?SAVED_GAMES?>` and
`<?INSTALL?>` resolve in the game's prefix through its `user.reg` shell folders, so the user folder
is the one Wine or Proton last wrote there (`steamuser` under Proton), with no Wine process. Each
folder keeps the timestamp gogdl printed after its last sync in `cloud.json`, which is what gogdl
compares the files against. gogdl's own rules are worked around: a conflict prints a fresh
timestamp, so a run that logged `Files in conflict` keeps the old one and flags the game; a side
that is empty makes gogdl copy the other whatever `--skip-*` says, so the upload after a session
does not run for an empty folder; a download deletes the local files the cloud lacks, so it runs on
a copy of the folder that replaces the folder only once gogdl finished. `pre-launch` checks that
`cloudstorage.gog.com` answers (2 s), runs after comet's start, and stops gogdl at 34 s after the
hook began, under the hook's 40 s `timeout_s`, which would cancel the launch: the game then starts
on the saves it had, and the state says so. A per-game lock makes the next `pre-launch` wait for an
upload still running, until that deadline. `keep-local` is `--force-upload` (refused with no saves
on the device; a folder empty here keeps what the cloud has), `keep-cloud` `--force-download`. A
keep that cannot reach the store fails and leaves the conflict to settle.

The **steam** source has no `cloud-saves` capability: Steam Cloud is the client's, and Universe
does not launch through it (see Steam).

The **epic** source drives `legendary sync-saves <app>`, always with the app name (without one it
syncs every installed game) and `--save-path`. The folder is the game's `CloudSaveFolder` in
legendary's metadata cache (`<config_path>/metadata/<app>.json`), resolved as legendary resolves it
under Wine (`{AppData}` is the prefix's Local AppData, `{UserDir}` its Documents,
`{UserSavedGames}`, `{InstallDir}`, `{EpicId}`; each part matched regardless of case) but by the
source on every run, since the path legendary stores goes stale once a prefix moves. legendary keeps
no baseline: the newer side wins. So each sync first runs it with both `--skip-upload` and
`--skip-download`, which only compares and logs the cloud save's date, and the source keeps its own
baseline in `cloud.json` (the cloud save's date and the newest local mtime after the last sync):
the cloud newer while the local files changed, or the local files newer while the cloud moved on
(by more than legendary's minute), is a conflict, and neither side is touched. A first sync with
saves on both sides that are not the same age is one. A download goes into a copy of the folder
(legendary empties the folder before it downloads), which replaces the folder once legendary
finished. legendary keeps the last `--save-path` as the game's, so the source then writes the game's
own folder back into `installed.json` under legendary's `installed.json.lock`, whether the download
finished or not; a lock another legendary holds past 2 s fails the sync instead. `pre-launch` runs after the
launch arguments (online, then `--offline`), under the same 34 s deadline, and checks that
`account-public-service-prod03.ol.epicgames.com` answers first. `keep-local` is `--force-upload`,
`keep-cloud` `--force-download` (refused while the cloud holds no save).

### Epic Games

The **epic** source drives legendary with `LEGENDARY_CONFIG_PATH` set to its `config_path`
(legendary's own folder by default, so a legendary used by hand shares the sign-in and the
installs). It is off until enabled. `login` without a code gives Epic's sign-in page, which ends on
a JSON page whose `authorizationCode` is the code; the code alone or the whole JSON is taken, a
session already there is dropped first (legendary keeps a valid one over a new code), and `status`
reads legendary's session offline. `library` is `legendary list --json`, the first run fetching
every owned title's metadata (up to 15 min): a title installed through a third-party store (EA,
Ubisoft) or without a Windows build is left out and logged. Epic has no catalogue search for a
launcher, so `search` filters the cached library by title. `install` asks `info` for the sizes,
then runs `legendary -y install <app> --base-path <games_dir> --platform Windows --skip-sdl` with
`--with-dlcs` or `--skip-dlcs`: its progress counts chunks, so `done` is its percentage of the
download size it announces. Stopped, legendary keeps a resume file under `<config_path>/tmp/`, the
folder is reported as `partial_dir` while it is there, and the next `install` picks the download
up. legendary exits 0 on some failures (a held lock), so an install is judged by `list-installed`
afterwards. `update` lists the installed games whose version is behind (`list-installed
--check-updates --csv`; a DLC's update rides on its game's) and runs `legendary -y update`.
`uninstall` runs `legendary -y uninstall`: the files and legendary's record go together. `scan` lists legendary's installs whose folder is still there and first adopts those of another
legendary (`adopt_from`, Heroic's by default) with `legendary import`, which needs the sign-in: a
refused import waits an hour before the next try. The game's `store` is `egs` and its `umu_id` the
umu database's for the app name, looked up once and kept in `umu.json`.

A game needs Epic's launch arguments, one of them a single-use exchange code, so the source's
`pre-launch` asks `legendary launch <app> --json --skip-version-check --no-wine` (online first,
`--offline` when that fails, so an offline-capable game still starts) and hands the game its
`game_parameters` and `egl_parameters` through `UNIVERSE_GAME_ARGS`; a Unix path among them
(`-epicovt=`, the ownership token) becomes `Z:\…` for a Proton or Wine game. The recorded command
line holds the exchange code, spent by then. For a Proton game with an `EasyAntiCheat` or `BattlEye`
folder (up to three levels down), the hook sets `PROTON_EAC_RUNTIME` or `PROTON_BATTLEYE_RUNTIME`
to the runtime Lutris publishes (`https://lutris.net/api/runtimes`), fetched once into the source's
data folder while the `anticheat_runtimes` setting is on.

### itch.io

The **itch** source drives butler's daemon (`butler daemon --json --transport stdio`) on a database
of its own (`db_path`, the source's data folder by default), one request at a time. It is off until
enabled. Signing in takes an API key: `login` without a code gives itch.io's API keys page, and the
key is the code; butler keeps the profile, so `status` reads it offline. `library` lists the owned
keys (purchases and claims; bundle games not claimed yet are not among them), fetched fresh once and
then paged, leaving out what is not a game or a tool (soundtracks, assets, books). `search` asks
itch.io's search API with the key, and filters the cached library when that fails. `install` picks
the build: a full one (no demo, no preorder, no `.deb`/`.rpm`/`.pkg`/`.dmg`) for Linux or Windows,
the `platform` setting's first, 64-bit and `.zip` before the others; a browser game has none. Its
download goes into a butler install location for the install folder, whose id is fixed by the
folder's path (a location's installs resolve through that row), and a paid game's download key is
fetched first when butler's database lacks it. Progress is butler's fraction of the size its task
announces. Stopped, butler keeps the state in the queue's staging folder, reported as `partial_dir`
(with the install folder's bytes) while it holds `operate-context.json`, and the next `install`
performs over the same folder. `uninstall` is butler's `Uninstall.Perform` for each of the game's
installs; one whose folder was removed outside butler is forgotten that way when the same build is
installed again. The
launch target is the itch app's: a `.itch.toml` action, else the shallowest candidate `butler
configure` finds for the build's OS, a lone script before a binary, an installer never, the biggest
program otherwise; a Linux build comes with `runner: linux`. `update` offers what butler calls a
direct update (a newer build on the same channel); its guesses among other uploads are left out, and
a file replaced under the same upload is not seen. `scan` lists butler's installs whose folder is
still there, after adopting the itch app's: the install folders its database (`adopt_from`) names
join butler's locations and `Install.Locations.Scan` registers the installs found by their receipts,
where they lie. The `store` is `itchio`; the umu database has no itch.io entries. There are no
achievements.

### Steam

The **steam** source runs the games of the Steam library itself, through Proton, while the Steam
client runs in the background. It is off until enabled, and needs `steam` on PATH; it reads the
client's folder (`steam_root`, else `~/.steam/root`, `~/.local/share/Steam`…) and writes nothing
there. Steam keeps no list of the account's games on disk, so signing in takes a Steam Web API key:
`login` without a code gives `steamcommunity.com/dev/apikey`, the key is the code, and it is checked
against the account the client last signed in with (`config/loginusers.vdf`, the latest
`Timestamp`), then kept in `key.json` (mode 600) in the source's data folder. `status` is that key
matching the client's account; the client's own sign-in is Steam's. `library` is
`IPlayerService/GetOwnedGames` with the key; `search` is the store's search (`storesearch`,
keyless), `owned` crossed with the cached library, and filters the cached library when the store is
out of reach.

`scan` reads every library of `steamapps/libraryfolders.vdf`: each `appmanifest_<app id>.acf` of a
game (its type in the client's `appcache/appinfo.vdf` — versions 27 to 29, the string table of 29
included — is game, demo or mod: Proton, the runtimes and the redistributables are left out). An
installed one is a `game` with `dir` = `<library>/steamapps/common/<installdir>`, `build` the
manifest's `buildid`, `disk_size` its `SizeOnDisk`, `owned` true when the manifest's `LastOwner` is
the client's account (a family-shared game is not claimed); one not fully installed is a stopped
download (`partial_bytes` = `BytesDownloaded`). The platform is the manifest's
`platform_override_source` when Steam was made to run a native game through Proton, else the
`oslist` of its installed depots, else Windows unless a Linux launch entry's program is there; the
program is the `config/launch` entry for that platform (no beta key, no DLC requirement, not a
server, editor or VR one; the `default` type first, 64-bit first), its path matched part by part
regardless of case, since the entries name files in any case. A Linux build comes with
`runner: linux` and runs outside Steam's runtime, where one that needs the runtime's libraries does
not start (on NixOS, most). The `store` is `steam` and the `umu_id` `umu-<app id>`, which is how
umu-run gives the game its `SteamAppId`: protonfixes then runs its Steam defaults, not the per-game
Steam fixes, which it keys by the bare number.

With `shared_prefix` (on by default), a Windows game's `prefix` is Steam's own for it,
`<library>/steamapps/compatdata/<app id>`, and its `proton` the folder of the tool Steam runs it
with: the game's pick in `config/config.vdf`'s `CompatToolMapping`, else Valve's (the
`app_mappings` of app 891390's appinfo, whose `compat_tools` give each Valve tool's app by name or
alias), else the pick for all other titles (`CompatToolMapping`'s `0`); a custom tool is found
through `compatibilitytools.d/*/compatibilitytool.vdf`, and one not installed, or without a
`proton` script, gives none. Steam and Universe then run the game on one prefix with one Proton and
see the same saves. On two Protons, each switch between them rewrites the prefix for the other's
version, and the first launch after one may quit ("Prefix has an invalid version"): a game added
before a change of Steam's pick keeps the Proton it came with, which its settings change. umu-run
keeps the real `pfx` folder of a prefix Steam made, and makes `pfx` a link to the prefix itself in
one it makes, which Steam's Proton runs as it is. With `shared_prefix` off, a game added from then
on gets a prefix of Universe's own and Universe's Proton. Either way Universe backs the saves up
(see Game data): under `shared_prefix` they live in Steam's compatdata, which stays where Steam keeps
it.

Steam Cloud syncs around the sessions the client launches: an `AC Launch,down` sync before the game
starts, which also records the files there are, and an `up,AC Exit` sync once it ends
(`logs/cloud_log.txt`). A game Universe launches still reaches the client through `steam_api`, which
tracks its process (`logs/gameprocess_log.txt`), but no sync runs before it, and the one at its exit
finds no launch record and skips the files the session added or removed ("No launch record found",
seen with Spacewar). So no cloud save comes down before a Universe session, and a new save file may
not go up after it. Nothing is lost under `shared_prefix`: the saves stay in Steam's compatdata,
which the client's next launch of the game syncs, and the ludusavi backup after each session covers
them meanwhile. With `shared_prefix` off, the saves sit in Universe's own prefix and never reach
Steam Cloud; the ludusavi backups alone keep them. Launching through `steam://rungameid/` would
give up the session's unit, hooks and recording, so Universe does not.

The client has to run for a game to reach it. `pre-launch` starts it when `~/.steam/steam.pid`
names no live Steam: `systemd-run --user --unit=universe-steam … steam -silent` with the launcher's
display variables, and `UnsetEnvironment=` for each one the launcher lacks, which the unit would
otherwise take from the user manager (gamescope keeps `WAYLAND_DISPLAY` from its children: the
manager's is the desktop's), so a client started inside the launcher's gamescope lands there; one
started there dies with it. It then waits up to 70 s for a new `[Logged On,` in
`logs/connection_log.txt`; a client that does not start cancels the launch (the game would only show
"Steam must be running"), one that does not sign in lets it go on (offline mode). For a Proton or Wine game the hook links the client's
`legacycompat/steamclient.dll`, `steamclient64.dll`, `GameOverlayRenderer64.dll`, `Steam.dll` and
`SteamService.exe` (as `steam.exe`) into the prefix's `C:\Program Files (x86)\Steam\` — Proton copies
them there from `STEAM_COMPAT_CLIENT_INSTALL_PATH`, which umu-run leaves empty, and without them the
game's `steam_api` finds no client; a file already there (Proton's copy, in a prefix Steam made) is
kept. A native game gets `SteamAppId` and `SteamGameId`. The launch entry's `arguments` (split as
Windows does, quotes and all) reach the game through `UNIVERSE_GAME_ARGS` when the game's program is
that entry's. A program wrapped in SteamStub (a `.bind` section) is logged: it restarts itself
through Steam, outside the session.

`install` needs no install folder: it starts the client, opens `steam://install/<app id>` (from a
unit of its own, so a `steam` that would start a client never becomes the source's child) and
waits `confirm_timeout_s` for the manifest that confirming Steam's window writes — announced first
as a `window` event (class `steam`, title `Install`), since Steam opens its main window with the
dialog, at the size it last had on the desktop, and inside the launcher's gamescope that one would
be shown —, then follows it:
`BytesDownloaded` of `BytesToDownload`, then `BytesStaged` of `BytesToStage`, until the
`StateFlags` say fully installed and nothing pending. Steam does the download: stopping the install
stops the watch, not Steam, and the next `install` follows it again. `update` lists the installed
games whose manifest asks for an update (`buildid` → `TargetBuildID`); updating one starts the
client and waits for it to run its update — no `steam://` URL asks for one, and a game Steam
updates only at launch waits for `confirm_timeout_s`, then says so. `uninstall` starts the client,
opens `steam://uninstall/<app id>` and waits `confirm_timeout_s` for the manifest to go: Steam asks,
then removes the files.

## Runners

A runner is what starts a game: `proton` (through umu-run), `wine`, `linux` (the program itself),
or an emulator. Each is a spec the core ships — id, name, aliases, the binaries to look for, the
platforms it emulates, the file extensions it takes, the flags that make it start fullscreen and
quit with the game, whether a stop repeats its SIGTERM (`term_twice`, see Sessions), and typed options — with a program the core detects or the user sets.

| Rust | Python | CLI | Role |
|---|---|---|---|
| `runners()` | `runners()` | `universe runner ls` · `runner options <id>` | `[Runner]`, see below |
| `set_runner_setting(id, key, value)` | `set_runner_setting(…)` | `universe runner set <id> k=v …` | writes `config.toml [runners.<id>] <key>`: `exe`, `args`, `gamescope`, `build` (`latest` or a version Universe installed), or an option, validated by type; `""` resets it |

`Runner` = `{"id": "dolphin", "name": "Dolphin", "kind": "proton|wine|linux|emulator", "aliases": ["…"],
"binaries": ["dolphin-emu"], "platforms": ["Nintendo GameCube", "Nintendo Wii"],
"extensions": ["iso", …], "exe": "the configured program or empty", "args": "extra arguments, shell-quoted",
"build": "[runners.<id>] build", "version": "the version of Universe's build that runs, empty for another program",
"builds": ["the versions Universe installed, newest first"],
"gamescope": true | false | null (the global default), "path": "the program that will run, empty when none was found", "source": "config|path|universe|",
"available": true, "options": [{"key", "type": "bool|path", "default", "label", "choices": [],
"value": the global value}]}`.

A few runners spell the file their own way: `xenia` is a Windows build run through Proton, `mame`
gets `-rompath <dir> <name>`, `dosbox` takes a program or a `.conf` (`-conf`), `scummvm` the game's
folder. Ids, aliases and platforms: `universe runner ls`.

The program: `[runners.<id>] exe` if set (a path, or a name on PATH), else by `[runners.<id>] build`.
Unset, the system's program — the spec's binaries on PATH in order — then the newest build Universe
installed (see Components); `latest`, Universe's newest build before the system's program; a
version, that build. A game may name its own with `launch.runner_exe`, or one of Universe's builds
by version with `launch.runner_build`. Flatpak installs are not looked for: `flatpak run` moves the
app into its own scope, which the session's `ExitType=cgroup` would take for the game ending.

The command line of an emulator: `[runners.<id>] args`, the option flags in the spec's order, the
file flag and the game file (`launch.exe`: a ROM, an image, an EBOOT.BIN, a folder), then
`launch.args`. `MANGOHUD=1` and `launch.env` apply as for Proton.

## Components

What runs a game or a source and can be installed: Proton builds, Wine builds, emulators, the
tools Universe fetches, and the system tools only the distribution installs well. An id is the
runner's for an emulator and Wine (`eden`, `wine`), the build's own for Proton (`ge-proton`,
`proton-cachyos`, `proton-em`, `umu-proton`), the program's for a tool (`umu-run`, `gogdl`, `comet`,
`legendary`, `butler`)
and for a system tool (`gamescope`, `mangohud`, `gpu-screen-recorder`).

| Rust | Python | CLI | Role |
|---|---|---|---|
| `components(refresh)` | `components(refresh=False)` | `universe component ls [--refresh]` | `{catalogue: {url, fetched_at, generated_at, error}, auto_update, components: [Component]}` |
| `component_install(id, version, accepted, progress)` | `component_install(id, version="", accepted=False, progress=None)` | `universe component install <id> [version] [--yes]` | installs the build, the latest this machine runs for `""`; returns its version. An entry with a `notice` is `Invalid` until `accepted`: a frontend shows the notice and asks first; the CLI prints it and asks, `--yes` accepts it, and without a terminal (or with `--json`) it refuses |
| `component_remove(id, version)` | `component_remove(…)` | `universe component remove <id> <version>` | removes one of Universe's builds; `Busy` while something names it (see Updates) |
| `component_uninstall(id)` | `component_uninstall(id)` | `universe component uninstall <id>` | removes every build Universe holds and `components/<id>/` with them (its skipped versions too); a runner's `build`, or a `launch.proton` naming one of them, is reset first as `component_use(id, "")` would; `Busy` while anything else names one (see Updates), and for a tool while a game runs; returns the versions removed, oldest first |
| `component_update(id, progress)` | `component_update(id="", progress=None)` | `universe component update [id]` | installs what is newer than Universe's builds, of `id` or of every component it holds builds of, then prunes; `[{id, name, version, error?}]`; `Busy` while a game runs |
| `component_rollback(id)` | `component_rollback(id)` | `universe component rollback <id>` | removes Universe's newest build and skips its version for good; returns the version now newest, `""` when the system's program takes over |
| `component_use(id, build)` | `component_use(id, build)` | `universe component use <id> <latest\|system\|version>` | a runner writes `[runners.<id>] build`, a Proton build `launch.proton` (its family for `latest`) |
| `component_cancel(id)` | `component_cancel(id)` | — | stops an install or update at the next chunk; it fails `Busy` |

**Catalogue.** JSON at `components.catalogue` (a URL or a file; empty: the project's `catalogue`
branch, which a daily CI job bumps), cached in `<cache>/catalogue.json`, fetched again once a day
old or on `refresh`; unreachable, the cache stands in, then the built-in entries (the Proton families
and the tools Universe pins). `{schema: 1, generated_at, components: {<id>: {name, kind:
proton|wine|emulator|tool, family?, bin?, homepage?, notice?, builds: [{version, date, channel: stable|rolling,
assets: [{arch: x86_64|aarch64|any, variant?: x86_64_v3, url, sha256, size, format:
appimage|tar|tar.gz|tar.xz|zip|binary, member?, appimage?, program?}]}]}}}`. A build's asset here:
its arch (or `any`), the `x86_64_v3` variant where the CPU has that level, else the plain one. The
latest build is the newest stable one (by date, then version) this machine runs, the newest rolling
one where upstream publishes nothing else.

**Installs.** `<data>/components/<id>/<version>/`, beside its sidecar `<version>.json` (`{id, name,
kind, version, date, channel, family, bin, program, url, sha256, size, disk, installed_at, auto}`)
written last: a folder without one is an install that did not finish. The download streams into
`<data>/components/.tmp/` after a check for room, and a digest other than the pinned sha256 leaves
nothing. An AppImage is unpacked by its own runtime
(`--appimage-extract`: no FUSE; on NixOS inside `universe-fhs` when it is on PATH); a tarball or zip
whose files share one top folder loses it; `member` takes one file out of a tar, `appimage` unpacks
the AppImage inside the archive in turn. The program: `program`, else `AppRun` for an AppImage,
`bin/wine` for Wine, the build's folder for Proton (its `proton` script), the tool's name for a tool.

**NixOS.** A downloaded build expects an FHS: on NixOS the launch runs it as `universe-fhs <program>
…`, a bubblewrap environment of appimage-run's libraries, plus the ones catalogue AppImages expect
of the host (libpng16, glibmm, libXv), the flake puts on Universe's PATH (and an AppImage is
unpacked inside it), so nothing is asked of the system's configuration — no nix-ld, no
binfmt, no steam-run. `doctor`'s `components-fhs` fails when Universe holds such a build and
`universe-fhs` is not on PATH. A stop leaves its shell and its bwrap alone: signalled, bwrap's
`--die-with-parent` would kill the game before it saves; they end with it.

**System tools.** gamescope, MangoHud (its 32-bit layer included) and gpu-screen-recorder
come from the distribution's packages, installed through PackageKit on the system bus
(its password prompt is the desktop's polkit agent): `component_install` resolves the family's
package names (Arch `lib32-mangohud`, Fedora `mangohud.i686`, Debian `mangohud:i386`) and installs
the ones missing. In the Universe session, where no agent answers that prompt, it runs
`pkexec --disable-internal-agent universe-system-install <id>` instead: the helper
(`/usr/lib/universe/` from the AUR and RPM packages, `/usr/local/lib/universe/` from `install.sh`)
takes one id of the three and nothing else, resolves the packages itself and installs them through
PackageKit as root, printing each percent on a line. Its polkit action
`io.github.ilyasturki.universe.system-install` is `allow_active=yes` (no for inactive and remote
sessions), so a local, active session needs no password, and no package beyond those three goes
through it. In the session a row is `installable` only where that helper is in place. NixOS ships
neither: it has no system tool to install this way (they come from the module's options). What
needs root beyond the package — a setcap outside Arch — stays a doctor fix. With no PackageKit (NixOS, image-based systems) or no
package for the family, the row carries the fix instead: on NixOS the module's option. A missing one
is proposed when the configuration uses it: gamescope with `launch.gamescope`, MangoHud with a frame
rate limit or the HUD, any of them while an enabled module names it in `[requires] system`
(the capture module names gpu-screen-recorder).

**What runs.** An emulator or Wine as `[runners.<id>] build` says (see Runners). A Proton build
through `launch.proton`: `components/<id>/` is one of the Proton directories, so a family name
takes the newest build of the family in any of them, Universe's included, and a build's version is
a name `launch.proton` takes. A tool: PATH first, then `<data>/bin`.

**Updates.** Of a component Universe holds a build of: the latest, when it is newer than Universe's
newest and not skipped. Every build but the newest two then goes, except the ones named — by
`[runners.<id>] build`, a game's `launch.runner_build`, a `runner_exe` or `[runners.<id>] exe`
inside the build, `launch.proton` (global or a game's), a `[proton]` path inside it — and the one the
running game runs on. `components.auto_update` (default true) is for a frontend to run
`component_update("")` itself: daily while it runs, never during a game. A rollback's version is
written to `components/<id>/.skipped`; installing that version by name takes it off.

`Component` = `{id, name, kind, family, bin, homepage, notice (what an install shows first, or ""), runner (the id, when a runner ships under
it), builds: [{version, origin: universe|nix|system|lutris|steam|heroic|umu|local|config, program,
managed, in_use, pinned, disk, size, date, installed_at, auto}] (Universe's newest first, then the
system's), in_use (the build that runs, null when none), latest ({version, date, channel, size}, null
when there is none for this machine), available ([{version, date, channel, size, installed,
skipped}], newest first), update (the version an update would install, or ""), proposal, used_by
(the games on it), setting ([runners.<id>] build, or launch.proton when it names this family),
recent ({version, at} of an update in the last 7 days, or null), skipped}`, and for a system tool
`packages`, `installable` (PackageKit can install them here) and `fix`. `proposal` is `install`
when nothing is installed and the library needs it (a runner a game uses; the default Proton's
family once a game runs on Proton; umu-run), `newer` when what runs is the system's and the latest
is surely newer, `""` otherwise. A system program's version comes from its Nix store path (followed
through a `/run/wrappers` copy, which the `suid-sgid-wrappers` unit names, and through binary
wrappers), else an AppImage's file name, else the distribution's package (pacman, dpkg, rpm); a
snapshot (`unstable`, `git`) or two numbering schemes propose nothing.

## Media

| Rust | Python | CLI | Role |
|---|---|---|---|
| `media_refresh(id, force, progress)` | `media_refresh(id, force, progress)` | `universe media <name> refresh` | `(changed, total)`: fills the empty slots of `media/`, each from the first provider with a picture for it — the game's source (the `art` of its `game` event, recorded under the source's id), `steam` (Steam's CDN by app id: the pin `metadata.steam_appid`, the source's `steam_appid`, else the Steam release GOG GamesDB lists; the 920×430 `header_2x` through the keyless `IStoreBrowseService/GetItems`), `gamesdb` (GOG GamesDB's game: the pin `metadata.gamesdb_id`, the store release by the game's Steam, GOG or Epic id, else a search by title that counts only a hit named like it, one released on the game's platform first, twins told apart by year), `libretro` (an emulator game's box art and logo, by its ROM's file name and the runner's platform), then `sgdb` with the user's key (first with `keys.prefer_sgdb`); a logo with no see-through pixel is passed over (GamesDB's is at times an opaque poster); a square nobody has is made from the box front (`generated`), and a provider's square replaces it once one comes. An empty description, genres or developers come from Steam's store page (`appdetails`: the description as text, `summary`, `developers`, `publishers`, `genres`, `metacritic`, the year), then what GamesDB has left (`release_year` only when 0); `.sync.json`'s `sources` names the provider of each slot, of `screenshots` (Steam's, else GamesDB's, 8 at most) and of `description`. An emulator game still titled after its file (`title_of` its `launch.exe`) takes GamesDB's name when the match is pinned or named alike and released on the game's platform. `force` refetches the filled slots and fields and asks again where a search found nothing; a pick does not stop its slot's default from being fetched; `id=""` does every game. No key is needed; requests to `store.steampowered.com` keep under 200 in 5 minutes |
| `media_refresh_many(ids, force, progress)` | `media_refresh_many(ids, force, progress)` | — | `media_refresh` over the games named, in turn: what an import just added |
| `media_cancel()` | `media_cancel()` | — | stops a running library-wide `media_refresh` or a `media_refresh_many` after the game in hand; it returns normally with what it got to. A single game's refresh is not touched |
| `media_status(id)` | `media_status(id)` | `universe media <name> status` | `[{id, title, entry, sgdb_key, slots: [{slot, path, default, override, origin, default_origin, kind}]}]`; `path` is what shows, `default` the fetched file under `media/`, `override` the pick under `media/picked/`; `kind ∈ picked, default, missing`; `origin` is `picked` or the provider that wrote the default (`steam`, `gamesdb`, `libretro`, `sgdb`, a source's id such as `epic`, or `generated`; empty when nobody recorded it), `default_origin` that provider whatever sits over it; `entry` (`{provider, id, name, year, verified, current}`, absent when unmatched) is the catalogue game the art is matched to — SteamGridDB's when the user's key is set (`sgdb_key`), else GOG GamesDB's — as the last refresh or candidates call cached it (offline: `name` empty until then); `id=""` does every game |
| `media_set_slot(id, slot, path)` | `media_set_slot(…)` | `universe media <name> set <slot> <path>` | copies the file to `games/<id>/media/picked/<slot>.<ext>` (a screenshot into `media/picked/screenshots/`), replacing any file of that slot there, and returns the path; the default under `media/` stays |
| `media_set_url(id, slot, url)` | `media_set_url(…)` | `universe media <name> set <slot> <url>` | the same from an http(s) URL, a candidate's |
| `media_unset(id, slot)` | `media_unset(id, slot)` | `universe media <name> unset <slot>` | removes the pick, so the slot shows its default again; `true` when there was one |
| `media_candidates(id, slot, page)` | `media_candidates(id, slot, page=0)` | `universe media <name> candidates <slot> [--page N]` | `{items: [{provider, id, url, thumb, score, slot}], page, more, entry, sgdb_key}`: on page 0 the slot's pictures from every keyless provider in the refresh's order (the source's, Steam's, GamesDB's — its other artworks too for a background —, libretro's; a Steam logo or a libretro picture only once the CDN has it), then with the user's key one page of SteamGridDB's, best first, English and non-NSFW only (`score` its votes × 1000; first with `keys.prefer_sgdb`); `more` says SteamGridDB has another page; `entry` as in `media_status`, SteamGridDB's being the pin (`metadata.sgdb_id`), else `.sync.json`'s, else a search by title — the hit named like the title, else autocomplete's first |
| `media_search(id, query)` | `media_search(id, query)` | `universe media <name> search [query…]` | `[{provider, id, name, year, verified, current}]`: SteamGridDB's games for the query with the user's key, else GOG GamesDB's (the title when empty), `current` on the one the art comes from — the id `media_pin` takes under `provider` when the match is wrong |
| `media_pin(id, provider, provider_id)` | `media_pin(…)` | `universe media <name> pin <provider> <id>` | `provider ∈ steam, gamesdb, sgdb` → `metadata.steam_appid`, `metadata.gamesdb_id`, `metadata.sgdb_id`; candidates and refresh follow it |

`slot ∈ box_front, square, banner, background, logo, screenshot`. `square` is a 1:1 picture (SteamGridDB's 1024×1024, then 512×512, with the user's key; else 512×512 made from the box front, the cover whole over a blurred fill of itself): Reprise's home rail and the Switch 2 tiles; `banner` the 920×430 header (Steam's, or GamesDB's 796×364 crop of its wide artwork): the Switch 2 news card and info pane when they have no picture, and the backdrops and launch poster when the game has no background (after its screenshots). On disk a slot is read under its own stem or Pegasus's and Lutris's, own stem first: `boxFront`, `cover`; `tile`, `icon` (Pegasus's square); `steam`, `grid` (Pegasus's banner); `hero`, `fanart`.

Two layers per slot: the **default** under `games/<id>/media/`, which `refresh` fills and `.sync.json`'s `sources` attributes to its provider, and the **pick** under `games/<id>/media/picked/`, which `media_set_slot` writes and always shows first (`media_of`: `media/picked/`, then `media/`). Removing the pick falls back to the default, so a pick never loses what was fetched.

## Recordings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `file_recording(session_id, path, timeline)` | `file_recording(session_id, path)` | `universe recording-file <session> <path> [--timeline <json>]` | files the mkv as `<recordings_root>/<id>/<session>.mkv` (rename within a filesystem, copy across), writes `recording`, the probed `recording_duration_s` and the timeline's `recording_started_at` / `recording_pauses` into the session line, prints the final path. The timeline is `{"started_at": RFC3339, "pauses": [[from, to]]}`, a file path on the CLI, `None` for none |
| — | `recordings(id)` (a filter on the client) | `universe recordings <name>` | the `SessionRow`s of `sessions(id)` that have a `recording` |
| `remove_recording(id, session_id)` | `remove_recording(id, session_id)` | `universe recordings <name> --remove <session> [-y]` | trashes the mkv (`trash`), clears `recording` on the session line; the hours stay |
| `frames::Frames::start()` | — | — | a frame sampler on the caller's tokio runtime and the channel its frames land on (`Landed {recording, index, file}`, each once). `thumbnail(path, duration_s)` queues a recording's `THUMB` frame (3, about a fifth in), `select(path, duration_s)` puts all `COUNT` (16) of its frames first and drops the frames another recording was still waiting for, its thumbnail kept, `forget(path)` drops its jobs and its cached frames |
| `frames::file(path, index)`, `frames::dir(path)`, `frames::at(index, duration_s)` | — | — | where a frame is cached, `$XDG_CACHE_HOME/universe/frames/<sha1 of the path>/NN.jpg` — the Qt host's cache, so the two share it — and the instant it is taken at, the middle of its sixteenth of the recording |

`recording-file` is called by the capture module's `session-end` hook, so it lands before any
`post-process` hook runs.

The frames are 640 px wide JPEGs taken by ffmpeg, two at a time, through VAAPI on
`gpu()`'s `vaapi` node (a 4K AV1 frame takes 0.4 s and 180 MB there, against 1 s, 2.4 s of CPU and
630 MB in software), in software when there is none; the first one that fails there before any
has come through turns the sampler to software, and the job runs again. A frame is written under a dot name and renamed into place,
so an extraction cut short leaves nothing that looks whole. Without ffmpeg nothing is sampled.

The recording **pauses with the game**: the module's `freeze` hook tells gpu-screen-recorder
`set-paused true` over its command socket (`-ipc`, through `gsr-cli`) and `thaw` `set-paused false`,
so a frozen game — the launcher over it with `pause_on_home` on — adds nothing to the file; with
`pause_on_home` off the game plays on behind the launcher and so does the recording. The state is
absolute, so a hook that runs twice or lands late cannot flip it the wrong way; the module keeps the
pauses in `pending/<session>.timeline.json` (`{"started_at", "paused", "pauses"}`, under a lock):
the hooks are no-ops until `start` has written it, and `start` reads the game unit's
`FreezerState` once the recorder answers on its socket, for a HOME pressed while it waited for the
window. `stop` asks the recorder to stop and answers with the saved file once it is written (the
unit's stop and a look in `pending/` when the socket is already gone), takes `started_at` from the
recorder's first-frame timestamp (`-write-first-frame-ts`: the file starts at its first frame, not
when the unit did — the window picker may have sat open in between), closes a pause left open and
hands the timeline to `recording-file`. The paused stretches are cut from the file, not held as a
still, so `min_duration_s` measures play recorded, not the sitting. gpu-screen-recorder 6.1 or
later, for `-ipc` and `gsr-cli`.

The capture module records the whole **screen** (`source = "screen"`, the default: gpu-screen-recorder's
KMS capture of the session's output) or the game's **window** (`source = "window"`, per game). The
window source needs a desktop whose windows the core lists (see Desktop; on GNOME the
`universe@ilyasturki.github.io` shell extension, `extension/`, GNOME 45 to 51, installed by the
home-manager module on NixOS, the AUR package under `/usr/share`, `universe setup` into
`~/.local/share/gnome-shell/extensions/` otherwise, loaded
after one logout, which also hides the resting pointer through `HideCursor(b)`): the module waits for
the game's toplevel through `universe session-window --wait` (`window_wait_s`, gamescope's window
stays hidden until the game draws; the core focuses it once it maps), then runs
gpu-screen-recorder on the desktop's screencast portal. The first launch of a game shows GNOME's picker —
pick the game's window, which is on screen by then — and the portal's restore token is kept in
`<data>/modules/capture/portal/<game id>`; GNOME restores the pick by the window's app id and title,
so later launches record without a dialog. A cancelled picker records nothing. Where no window
list is reachable, or when no game window appears in time, the screen is recorded and the desktop's
OSD says so.

A screen recording **follows a monitor switch**. gpu-screen-recorder's KMS capture is pinned to one
connector, so the capture unit runs `bin/record` over it: it polls `/sys/class/drm/*/{status,enabled}` once a
second and, when the recorded connector stops being drawn on — unplugged, or left `disabled` with
the cable still in — or the recorder exits, asks the recorder
to stop, moves the file aside as `pending/<session>.part<n>.mkv` (its `.ts` sidecar with it, the
list under `parts` in the timeline), opens a pause, waits for a connector being drawn on — the same one
back, else the first, as `pick_screen` — and starts the recorder again on it, capped at the first
part's size (`-s`: a limit, a smaller monitor still gives a smaller part, which the stitch logs); a
monitor that has just come up has no CRTC for a few seconds, so an exit right after the restart is
retried. The pause closes at the new recorder's
first frame, a frozen game gets `set-paused true` on it at once and the OSD names the new screen. At
session end `stop` marks the timeline `stopping` first — the recorder's own exit is then no switch
— and, with parts to join, runs `bin/finish` in a unit of its own (`ffmpeg -f concat -c copy` into
one `<session>.mkv`, then the usual checks and `recording-file`) and waits for it as long as the
hook can; a stitch longer than that goes on alone, filed once done, and the journal's
`post-process` runs without the recording. When ffmpeg cannot join the parts the longest one is
filed and the others stay in `pending/`. A window (portal) recording follows its window on its own:
`record` runs gpu-screen-recorder plain.

A recorder that **never starts** is said so rather than passed over: `start` opens the timeline
before the unit, so `record` cannot read the missing file as a session already stopping, and waits
for the `-ipc` socket only as long as `universe-capture-<session>` is alive. Gone by then — a
connector gpu-screen-recorder will not take, no encoder — the hook logs the unit to read and the
shell's OSD says "Recording failed"; the session ends with no recording, and the journal's
`post-process` then has only the screenshots to write from.

`codec` is `auto` by default: the first of `av1_10bit`, `hevc_10bit`, `hevc`, `h264` in the
`video_codecs` section of `gpu-screen-recorder --info` (what the card encodes), `h264` when it
lists none of them. `audio` is `output` by default; `output+input` adds the microphone.
`quality` is a QVBR preset — constant quality up to a bitrate ceiling (`very_high`: 16 Mbps target,
32 Mbps ceiling, ~7-8 GB/h at 4K on AMD). Two config-scope keys, for `config.toml` or `universe
module set capture <key>=<value>` — advanced rows on the module's page — replace what the presets choose:
`ffmpeg_video_opts` (gpu-screen-recorder's `-ffmpeg-video-opts`) and `gsr_extra_args` (appended
to the command). `-bm cbr` stays pinned: it is the base the QVBR override needs.

## Screenshots

| Rust | Python | CLI | Role |
|---|---|---|---|
| `screenshots(id)` | `screenshots(id)` | `universe screenshots [name]` | `[Shot]`, newest first, read from disk on every call; `id = ""` spans every visible game |
| `remove_screenshot(id, name)` | `remove_screenshot(id, name)` | `universe screenshots <name> --remove <file> [-y]` | trashes `screenshots/<name>` and drops it from the `images` of the journal entry naming it, when one does |
| `media(id)` | `media(id)` | — | `[MediaRow]`: the shots, the recordings and the written journal entries as one list, newest first by `when`; `id = ""` spans every visible game. One pass, each game's journal read once |

`Shot` = `{"game", "title", "path", "taken_at", "session", "thumb", "thumb_ready"}`. A shot the player takes — the dock's
camera, a pad macro, `universe screenshot` — lands in `games/<id>/screenshots/YYYYMMDD-HHMMSS.png`:
the `screenshot` hook writes wherever `SCREENSHOTS_DIR` points, and with no session running that is
`<state>/screenshots/`, a shot of the launcher that no listing shows, taken with no game (no `GAME_*`,
the module's global settings). `taken_at` is the name's
moment; `session` is the session whose span covers it, with the journal module's own grace (90 s
before the start, 120 s after the end), or empty. The hook answers once the pixels are grabbed, so
a cue that follows its return never lands in the shot: the launcher plays the shutter and, inside
gamescope, paints a flash over the game (see `frontends.md`); the pad's `screenshot` macro reports
back as a `screenshot` event on the watcher (`{"event": "screenshot", "path": …}`, the path empty
when it failed). A journal entry names them by basename, which resolves to `screenshots/`.

The **screenshot** module takes them, apart from recording: its own `enabled` switch, per game, is
whether a game takes screenshots at all, and capture's settings have no say. Inside the launcher's
gamescope (`GAMESCOPE_WAYLAND_DISPLAY` set) its hook asks gamescope itself, whatever the desktop:
`universe nest-shot <path> [--overlays]` sets `GAMESCOPECTRL_REQUEST_SCREENSHOT` on its root and
copies the `/tmp/gamescope.png` it writes — the game alone at its render size with `window = true`,
the overlay layers (mangoapp's HUD) too with `false`. gamescope takes one shot at a time, so a
screenshot pressed while HOME's frame of the game is being taken is dropped, and the hook falls back
to gpu-screen-recorder as below.
Elsewhere it asks the desktop, `universe desktop-shot` (see Desktop): the focused window with
`window = true` (the default), the session's screen with `false`, the cursor per `cursor` (both per
game). On GNOME the extension grabs it (`org.universe.Windows.Screenshot(path, window, cursor)`):
Mutter reads the framebuffer synchronously, so the grab returns at the press, before the PNG is
encoded. When the desktop takes none it is a gpu-screen-recorder `-o` capture of `SESSION_SCREEN`
when gpu-screen-recorder is installed, else the hook fails.

`thumb` is the shot's thumbnail, a 960 px wide JPEG under `$XDG_CACHE_HOME/universe/thumbs/` (one flat
directory, `<game>--<stem>-<mtime>.jpg`, so a file replaced in place gets a fresh one), and
`thumb_ready` whether it is there: a listing queues the missing ones, newest first, and the core
makes them in the background, three at a time, without holding the call. A frontend shows the
picture once the file lands (`frontends.md`, `api.screens.thumbs`). A 4K png decodes in about
150 ms; a grid that read the originals would spend that per cell.

`MediaRow` = `{"kind", "game", "title", "session", "when", "date", "path", "thumb", "thumb_ready",
"has_journal", "heading", "excerpt", "duration_s"}`: `kind` is `shot`, `recording` or `journal`; `when`
sorts the list (the shot's time, the recording's end, the entry's writing) and `date` is what a row
shows (an entry's is when it was played); `path` the shot, the recording or the entry's first picture;
`thumb` as a `Shot`'s, empty for a recording (its frames are the frontend's); `has_journal` whether an
entry, written or on its way, covers the session; `heading` the entry's title and `excerpt` its first
prose paragraph as plain text (emphasis dropped, links reduced to their text), for a card.

## Journal

| Rust | Python | CLI | Role |
|---|---|---|---|
| `add_entry(session_id, entry)` | `add_entry(session_id, entry)` | `universe journal-add <session> <entry>` | validates the schema, fills `started_at`/`ended_at`/`duration_s` from the session line when the entry lacks them, writes `journal/<session>.json` |
| `journal(id)` | `journal(id)` | `universe journal <name>` | `[Entry]`, last first, read from disk on every call, `images` made absolute; the state files below are entries too |
| `pending_journals()` | `pending_journals()` | `universe status` (a `journal: writing <title>…` line; `pending_journals` in `--json`) | `[{game, title, session, started_at}]` for every `pending` entry across the library; `title` is the game's |
| `remove_journal_entry(id, session_id)` | `remove_journal_entry(id, session_id)` | `universe journal <name> --remove <session> [-y]` | trashes `journal/<session>.json` and the frames it lists — the player's own shots stay, they are the game's, not the entry's; a `pending` entry has its `universe-journal-post-process-<session>` unit stopped, and every state file of the session goes |
| `journal_write(id, session_id, rewrite)` | `journal_write(id, session_id, rewrite=False)` | `universe journal <name> --write <session> [--force]` | starts the journal module's `post-process` hook for that one session and returns its unit name: another try at a `deferred` or `failed` entry, a first entry for a session that never had one, or, with `rewrite`, a new entry over a written one (`JOURNAL_REWRITE=1` in the hook's environment). The game's own "write an entry after each session" switch does not hold it back. `Invalid` for a session id that is not a timestamp or an entry already written without `rewrite`, `NotFound` for an unknown session, `Busy` while that session's unit runs, `Unavailable` when the module is off, missing a binary or still waiting on a setting |
| `sweep_journals()`, `retry_journals(id)` | `sweep_journals(id="")` | `universe journal [<name>] --retry` | starts the oldest owed entry — `deferred` past its instant, or `pending` with no unit behind it — and answers `{started: {game, session} or null, due, next, held?, error?}`: `due` is what is still waiting, `next` the nearest instant a deferred entry falls due (the frontends arm their timer on it; `changes::watch` does), `held` why nothing started. One entry at a time, and none while a game runs |
| `due_journals()`, `next_journal_retry()` | — | — | the sessions the sweep would take, and that nearest instant |

`Entry` = `{"session", "game", "written_at", "started_at", "ended_at", "duration_s", "lang",
"title", "provider", "paragraphs": [], "next_up": "", "images": ["relative path"],
"state": "written", "retry_at": ""}`. On disk and in `add_entry` the images are relative to `games/<id>/journal/`,
except the player's own shots, named by basename (`YYYYMMDD-HHMMSS.<ext>`) and read from
`games/<id>/screenshots/`; `journal(id)` hands them all out absolute. `started_at`, `ended_at` and `duration_s` are the session's span,
filled from `sessions.jsonl` by `add_entry` when the entry lacks them. `journal-add` is called by the journal module's
`post-process` hook, which also passes `started_at`, `ended_at` and `duration_s` so an entry it
writes itself (core unavailable) is self-contained. While the hook runs the session is
`journal/<session>.pending.json` (`{"session", "game", "started_at", "provider"}`), whose mtime the
module refreshes before every try. The file is removed once the entry is in, or replaced when the
run ends without one:

- `<session>.deferred.json` (`{"session", "game", "provider", "written_at", "until", "reason",
  "attempts"}`) for a failure worth another run — a quota wall, codex signed out (its account is
  asked before any work, and a turn refused with a 401 says the same: "codex is signed out: run
  codex login", tried again every 15 minutes), an endpoint that timed out or answered 5xx, a model
  that gave nothing usable, a recording on a filesystem that is not mounted, a hook killed by a
  signal. `until` is the wall's own reset instant when the provider gives one, else a backoff on
  `attempts` (15 min, 1 h, 4 h); a quota wall and a sign-out do not count as a try, and the third
  counted try writes a failed file instead ("… (gave up after 3 tries)").
- `<session>.failed.json` (`{"session", "game", "written_at", "reason"}`) when nothing will ever
  come of it: the core refused the entry, the recording is gone, it holds no picture, or the
  provider is set up wrong (no key, unknown model, missing binary).

A session with neither a recording nor a screenshot gets no entry and no state file, and neither
does one whose module is enabled but still waiting on its `provider`.

What a run extracts and what the model answered stay under
`$XDG_DATA_HOME/universe/modules/journal/work/<session>/` until the entry is delivered, so the next
run reuses the frames (same recording, same image settings) and the answer (same provider, model
and prompt) instead of paying for them twice; a week without being touched and the folder goes. The frames the module samples from the recording sit between the
session's screenshots, placed on the file through `RECORDING_STARTED_AT` and `RECORDING_PAUSES`
(the wall-clock moment less the recorder's start and the pauses before it); without them, the
session's start and no pause.

The core lists those files as entries, sorted with the real ones: `state ∈ written, pending,
deferred, failed`. A `pending` entry has the file's `started_at` and `provider`, an empty title and
no paragraphs; a `failed` one has the file's `written_at` and `paragraphs = [reason]`; a `deferred`
one adds `retry_at`, the instant it is owed another run. A pending file whose mtime is more than 30
minutes old lists as `failed` with the reason `timed out`. A written entry hides the failed one of
the same session, a failed one the deferred one, and that one the pending one. Dotfiles and anything
that is not `*.json` are ignored.

## Virtual pads

The `pads` module gives an emulator one virtual pad per player, whatever pads are held: its
`pre-launch` hook starts `universe-pads-<session>.service`, which reads every physical pad through
SDL3 and creates `Universe Pad <n>` over `/dev/uhid` (vendor `0079`, product `5550` + player, uniq
`universe-pad-<n>`), a third-party PS5-class pad as SDL's PS5 driver knows it: sticks, triggers,
buttons, gyro and accelerometer, touchpad, battery; the emulator's rumble goes back to the physical
pad. The game's environment gets `SDL_GAMECONTROLLER_IGNORE_DEVICES_EXCEPT` listing the eight
players and `SDL_JOYSTICK_HIDAPI_PS5=1`, so its SDL sees the virtual pads alone; nothing is grabbed
or hidden, and the watcher and the launcher read the physical pads as before (the virtual ones have
no evdev node). A pad takes the lowest player with none; one that goes leaves its player's pad in
place, reporting nothing held, for the next to take, so the emulator's bindings survive a change of
pad. Each virtual pad's SDL GUID stays the same from session to session. HOME stays Universe's
unless `guide` is on. While the game is frozen the pads report nothing held, so what is pressed
under the launcher is not queued for the game.

Only an emulator runner is served; a Proton, Wine or native game keeps the physical pads (Wine's
winebus reads hidraw and evdev past SDL's hints). Without `/dev/uhid` open to the user (the AUR and
COPR packages and install.sh ship `70-universe.rules`, which also opens the virtual pads' hidraw
nodes, and load uhid at boot; on NixOS `programs.universe.controller.enable`) or without
SDL3, the hook logs why and the game gets the physical pads.
`$XDG_RUNTIME_DIR/universe/pads-<session>.json` holds `{ready, pid, players: [{player, name, vendor,
product, pad}]}` while the forwarder runs; it stops with the session, or by itself once the game's
unit is gone.

## Emulator controls

The `controls` module sets up an emulator's controllers for the physical pads held, before each
launch of an emulator runner (nothing when no pad is held, or when the `pads` module serves the
game: its virtual pads hide the physical ones). Its `pre-launch` hook lists the gamepads through SDL3 under the joystick hints the emulator's own SDL sets (they pick
the driver that claims a pad, and with it the GUID and indices the emulator sees; the game's
`launch.env` `SDL_*` go over them), then does two things.

Every emulator: the game's environment gets `SDL_GAMECONTROLLERCONFIG_FILE`, pointing at
`$XDG_RUNTIME_DIR/universe/controls-<session>.txt` (removed at `session-end`): each held pad's
SDL mapping without `guide` (unless `guide` is on), `misc1`…`misc6` and `paddle1`…`paddle4`, after
the controller databases an emulator would itself give that hint, so a line for the same GUID
ends up ours. A Nintendo-style pad's line keeps its `hint:!SDL_GAMECONTROLLER_USE_BUTTON_LABELS:=1`:
SDL3 takes it as "this mapping is positional" and reads the pad's face style (B at the bottom) off
it, which sdl2-compat's label swap and the writers that go by label (Ryujinx, Cemu, PPSSPP) rely on.
An emulator reading pads through SDL's gamepad API never sees
HOME, capture or the paddles — its hard-wired uses of them included (Vita3K's pause, xemu's and
Flycast's menus, ScummVM engines' guide keys); the touchpad stays, PS4 and Vita games use it.
Raw-index readers (Eden, Azahar, melonDS, mGBA, snes9x, mupen64plus) ignore the mapping, and
their writers never bind those buttons. Xenia runs through Proton, whose own SDL reads the pads:
nothing is done for it.

The emulators with pad identities or player slots in their config get them written, the way the
emulator's own auto-mapping would. Player 1 is the pad a button was last pressed on, the one the
player drives Universe with (`active-pad`, matched to the sysfs device SDL's hidraw or evdev node for
a pad hangs off); the others follow in SDL's order, the order they connected in, as they all do
when nothing was pressed yet. Twins (pads of one model) keep the numbers SDL gives them, whichever
player they are:

| Runner | File | Written |
|---|---|---|
| `eden` (and citron, sudachi, suyu, yuzu dirs) | `qt-config.ini [Controls]` | `player_0-7_*`, each with `\default=false`; players past the pads disconnected |
| `ryujinx` | `Config.json`, `games/*/Config.json` | `input_config` + `player_input_assignments`; a game's own config too unless `use_input_global_config` |
| `dolphin` | `GCPadNew.ini`, `WiimoteNew.ini`, `Dolphin.ini` | `[GCPadN]`/`[WiimoteN]` on `SDL/<n>/<name>` (calibration and options kept, the extension and `Sideways Wiimote` from `wiimote`), `SIDeviceN`/`Source` 0→on for held pads; a port past them left on an `SDL/` pad (GameCube controller, emulated Wii remote) off, Dolphin binding by pad name |
| `cemu` | `controllerProfiles/controllerN.xml` | `SDLController` uuid `<n>_<guid>`, Cemu's default mapping; type, rumble and deadzones kept |
| `azahar` | `azahar-emu/qt-config.ini [Controls]` | the active profile's buttons, sticks and motion |
| `melonds` | `melonDS.toml` | `Instance0 JoystickID`, `[Instance0.Joystick]`; hotkeys on HOME lose that button |
| `mgba` | `config.ini`, `qt.ini` | `[gba.input.SDLB]` `deviceN` and player 1's keys, the per-GUID and per-name profiles; pad shortcuts on HOME cleared |
| `snes9x` (gtk) | `snes9x.conf` | `[Input]` ports (multitap from 3 pads), `[Joypad K]`, the stick in `[Joypad K+5]` |
| `mupen64plus` | `mupen64plus.cfg` | `[Input-SDL-Control1-4]` in manual mode, mem pak |
| `rpcs3` | `input_configs/global/<active>.yml` | every player on the SDL handler (`<name> <k>`), `PS Button` unbound |
| `pcsx2`, `duckstation` | `PCSX2.ini`, `settings.ini` | `[Pad1-2]` (more with the user's multitap) on `SDL-<n>`; HOME, capture and paddles out of `[Hotkeys]` |
| `ppsspp` | `controls.ini [ControlMapping]` | player 1's pad; HOME on the PSP's Home, so PPSSPP's own menu stays shut |
| `xemu` | `xemu.toml [input.bindings]` | `portN` = GUID |
| `flycast` | `emu.cfg [input]` | stale `maple_sdl_joystick_*` gone, `device2-4` on for held pads |
| `scummvm` | `scummvm.ini` / `~/.scummvmrc` | `joystick_num` = the first pad |

A game's page shows the module's rows on emulator games only (`[applies]`): `wiimote` on Wii games,
`layout` and `shoulders` on the runners whose writer honours them (a value set for all games still
reaches the others' hooks, and their writers ignore it). `layout` picks where a Nintendo A sits,
per game over a global default: `positional` (A on the right, as on a Switch and as these
emulators, Dolphin aside, map by themselves) or `xbox` (A at the bottom), B, X and Y following as
on a Switch diamond. It holds for the Switch, 3DS, Wii U, DS, GBA and SNES, the GameCube
controller and the Wii's Classic Controller; on the Wii remote `layout`'s A is the remote's A and its B the remote's
B (which also stays on the right trigger), 1 and 2 on the left and top buttons. N64 keeps
mupen64plus's own (A bottom, B left). A face button is named by its place on the pad, not by the
label SDL gives it: on a Nintendo-printed pad (8BitDo, Switch Pro) `positional` lands A on the
button printed A in D-input (SDL's HIDAPI 8BitDo driver), in Switch mode, and in X-input mode,
where SDL takes the pad for an Xbox 360 pad and labels its bottom button A. In Dolphin a GameCube
pad (SDL's `face:axby`) is bound by its print under either choice: `layout`'s A, B, X and Y are
the buttons printed so, and the upright remote's 1 and 2 sit on its X and Y. `wiimote` picks how a
Wii game holds the remote, per game over a global default, on every held port: `nunchuk` (upright
with a nunchuk), `sideways` (no extension, Dolphin's `Sideways Wiimote` on, the D-pad and left
stick on its D-pad, 2, 1, A and B on `layout`'s A, B, X and Y, B also on the right trigger) or
`classic` (the Classic Controller); no face button is left without a Wii or GameCube button. `shoulders`, per game over a global default,
`standard` or `swapped`, exchanges the pad's bumpers and triggers where a console tells them
apart (Switch, Wii U, 3DS, GameCube, Wii, N64, PS1-PS3): standard is L/R on the bumpers and ZL/ZR,
L2/R2 on the triggers; GameCube L/R on the triggers and Z on either bumper; N64 L/R on the
bumpers, Z on the left trigger and R on the right one too. A console with only L and R (GBA, DS,
SNES, PSP) takes them on the bumpers and the triggers both, so there is nothing to swap and the
row isn't offered; on a pad with digital triggers mGBA and melonDS, which bind one button per key,
keep the bumper. A game's own input profile
keeps its scheme and only follows the held pad (Dolphin `PadProfileN`/`WiimoteProfileN`, Cemu
`gameProfiles` `controllerN`, RPCS3's other and per-title configs). Dolphin reads every port after
a game's profiled one from that profile's file, so each held port past it gets
`Profiles/<GCPad|Wiimote>/universe-<profile>-N.ini`, the nearest profile before it on pad `N`
(on a keyboard, or missing: a copy of the port's section), named in the game's `[Controls]`; such a
name goes once no profile of the user's is left before it. Everything else in a file is left as it
is; a file is written only when it changes, through a symlink to its target, and the first write
copies it to `<file>.before-universe`. A file the emulator makes on its first start is never
created, the hook logging "start X once" instead; mupen64plus, snes9x and ScummVM files are
written whole, missing or not.

## Modules

| Rust | Python | CLI | Role |
|---|---|---|---|
| `modules()` | `modules()` | `universe module ls` | see below |
| `enable_module(id, enabled)` | `enable_module(id, enabled)` | `universe module enable\|disable <id>` | writes `[modules] enabled` in `config.toml` |
| `module_settings(module, game_id)` | `module_settings(…)` | `universe module settings <id> [game]` | global settings merged with the game's; `game_id=""` is global only |
| `set_module_setting(module, game_id, key, value)` | `set_module_setting(…)` | `universe module set <id> k=v [--game g]` | validated against `[[settings]]`. `game_id=""` writes `config.toml [modules.<id>]`, otherwise `game.toml [modules.<id>]` |
| `module_setting_choices(module, key)` | `module_setting_choices(…)` | — | the global setting's choices; a setting with `choices_exec` gets them from the module, live (see below) |
| `doctor()` | `doctor()` | `universe doctor` | `[{check, label, ok, detail, fix, module, component}]` (`component` the component whose install fixes it — a runner not found, a fetched tool, the default Proton's family when the catalogue has a build for it, a system tool PackageKit can install — else empty; `components-fhs` on NixOS once Universe holds a downloaded runner; `check` a stable id, `label` its plain name, `detail` the problem when not `ok`, `fix` what to do about it, empty when `ok`, worded for the distribution: NixOS options on NixOS, the Arch, Fedora or Debian package names elsewhere): the config file (absent: defaults; read-only), the systemd user manager (250 or later, for `ExitType=cgroup`) and cgroup v2, umu-run (or python3 for the one Universe would fetch), MangoHud and its 32-bit layer, gamescope and mangoapp, required binaries of the enabled modules and sources (`module` names the one, or `core`, `runners`, `media`, `controller`; a tool Universe fetches is fine missing), one line per `required` setting an enabled module is still waiting on, the lines of each enabled module's and source's `check` hook (the journal's `codex-signin`, the capture's `gpu-screen-recorder`, `gsr-cli` and `gsr-kms-server`, GOG's `gog-auth` and `gog-comet`), Proton, the desktop (`desktop`, then per profile the programs it drives, `desktop-<program>`, a notification daemon where the OSD is a notification, `cursor` where the profile cannot hide it), the cursor and Universe extensions on GNOME, tokens, one `runner-<id>` check per runner a library game uses (its program resolved), `runner-eden-stop` when Eden is one (its `[UI] confirmStop` at `2`, else a stop shows its "close?" question), `key-sgdb` (always `ok`: whether the optional SteamGridDB key is set); `modules` and `sources` say what `config.toml` enables that is not found |
| — | — | `universe setup` | after an install: on GNOME, writes the `universe@ilyasturki.github.io` extension the binary carries into `~/.local/share/gnome-shell/extensions/` (rewritten when stale; left to a system copy when there is none there) and adds it to `org.gnome.shell enabled-extensions` (out of `disabled-extensions`, which overrides it), read by the shell at the next login; then prints `doctor` |

A module entry is `{id, name, version, description, dir, enabled, available, missing: [bin], incompatible,
unset: [key], hooks: {}, settings: [Setting], applies: {runner_kinds: [kind]}}`, and
`Setting` = `{"key", "type": "bool|string|int|enum|path", "default", "label", "description",
"scope": "global|game", "choices": [], "choice_labels": {value: label}, "dynamic": bool, "required": bool,
"keywords": [word], "runners": [id], "platforms": [name]}`. `keywords` are words a settings search finds the
setting by that its label and description do not carry (`h265` for a codec).
A game's page (the `Form::Game` form, every look's game settings) shows a module's game-scope settings
only on the games `applies.runner_kinds` names by their runner's kind (`emulator`, `proton`, `wine`,
`linux`), and of those only the settings whose `runners` holds the game's runner (its canonical id) and
whose `platforms` holds its platform; an empty list takes every game. A source's game-scope setting
follows its own `runners` and `platforms` the same way. Nothing else changes: a hidden setting keeps
its value and still reaches the hooks.
`label` names the setting in a few words; `description`, empty when the manifest gives none, says what it
does and shows under it. `choice_labels` is what a choice reads as, by stored value: a frontend shows the
label and writes the value, and a value without one reads as itself.
`missing` holds the manifest's absent binaries plus those the chosen value of a `requires_bins`
setting asks for, so a module is available or not by what it is set to. `incompatible` says why
`[requires] core` rules out the running Universe (`needs Universe >=0.2.0, this is 0.0.9`, or the
comparator it cannot read), empty when it fits: such a module is unavailable, its hooks never run, and
`doctor` gives one `requires-core` line for it; a source's entry carries the same. `unset` lists the
`required` settings still empty on an enabled module: its hooks run and do nothing, `doctor` says
which, and the settings page sends the cursor there when the module is switched on. `choices` binds an `enum`; on an `int`
or a `string` it lists suggestions, any value stays accepted — except that an `int` also
takes a listed non-numeric name (`"auto"`), which the module resolves itself. `dynamic` is
set when the manifest names a `choices_exec`: `<module dir>/<choices_exec> <key>`, run with
`MODULE_SETTINGS_JSON`, `MODULE_DIR`, `MODULE_DATA_DIR` and `UNIVERSE_BIN` (the capture module asks it
`screen-mode` for the fps choices), prints the choices as a JSON array of strings (20 s at most).

## Desktop

`desktop.profile` names the desktop the core works with; `auto` detects it from the compositor's own
socket first (`HYPRLAND_INSTANCE_SIGNATURE`, `NIRI_SOCKET`, `SWAYSOCK`), then `XDG_CURRENT_DESKTOP`
(GNOME, KDE, Cinnamon), then an X11 session (`XDG_SESSION_TYPE=x11`, or `DISPLAY` without
`WAYLAND_DISPLAY`: Xfce, MATE, i3 and the like); anything else, COSMIC or Cinnamon on Wayland
included, is `none`. Inside the launcher's gamescope, gamescope focuses the game, draws the OSD on the
launcher's overlay and takes the screenshots itself: the desktop is asked only whose window has the
focus (`host_focus`) and to bring gamescope's up (`summon`). gamescope gives its children
`XDG_CURRENT_DESKTOP=gamescope`, `XDG_SESSION_TYPE=x11` and its own Xwayland's `DISPLAY`, so the
launcher hands the desktop's on to the gamescope it starts as `UNIVERSE_HOST_XDG_CURRENT_DESKTOP`,
`UNIVERSE_HOST_XDG_SESSION_TYPE`, `UNIVERSE_HOST_DISPLAY` and `UNIVERSE_HOST_WAYLAND_DISPLAY` (empty
where it had none), which detection reads inside it; inside a gamescope that hands nothing on
(Steam's) its Xwayland is no X11 desktop.

| profile | windows (`session_window`, focus) | OSD | screenshots | cursor hiding (`desktop.hide_cursor`) |
|---|---|---|---|---|
| `gnome` | the Universe extension's `List` and `Activate` | its `ShowOSD` | its `Screenshot` | its `HideCursor`, or the `cursor_extension` enabled for the session |
| `kde` | a KWin script loaded through `org.kde.KWin /Scripting`, which calls back with the list | plasmashell's `org.kde.osdService` (`mediaPlayerVolumeChanged`, `showText`) | `spectacle -b -n` | KWin's `hidecursor` effect (Plasma 6.1 and later), its `InactivityDuration` set to 5 s for the session |
| `cinnamon` | EWMH, on an X11 session | `org.Cinnamon.ShowOSD` | the X server's root image | — |
| `sway` | `swaymsg -t get_tree`, `[con_id=N] focus` | a notification | `grim`, the focused window's rectangle with `window` | `seat * hide_cursor 5000` |
| `hyprland` | `hyprctl -j clients`, `dispatch focuswindow` | a notification | `grim`, as Sway | `keyword cursor:inactive_timeout 5` |
| `niri` | `niri msg -j windows`, `action focus-window` | a notification | `grim` of the output | — (its config's `cursor { hide-after-inactive-ms }`) |
| `x11` | EWMH: `_NET_CLIENT_LIST`, a `_NET_ACTIVE_WINDOW` request as a pager's (source 2, past focus-stealing prevention) | a notification | the X server's root image | — |
| `none` | `Unavailable` | — | gpu-screen-recorder | — |

`host_focus()` names the focused toplevel to the launcher: `launcher` when its pid is this
process's or that of the gamescope it runs in (not Steam's), `session` when it is the running
game's (the launcher's gamescope's pid, else a process in the unit's cgroup), both false for
another app's or none; always `{launcher: true}` on the launcher's own screen
(`UNIVERSE_OWN_GAMESCOPE=drm`) and under Steam's Game Mode; `Unavailable` where the profile has no
window list (GNOME without the extension, `none`). Every profile reads it off its window list but
KDE, where one KWin script per call would cost a file, a load and up to 5 s: `universe-focus-<pid>`
stays loaded and calls back with the active window's pid on each `windowActivated`
(`clientActivated` on Plasma 5). Each read asks `isScriptLoaded` and loads it again after a KWin
restart; `$XDG_RUNTIME_DIR/universe/kwin-focus-scripts` lists the ones loaded, and those of
processes that ended without unloading theirs are unloaded at the next load. `summon()` activates
the largest window of the same pids.

A notification OSD is `org.freedesktop.Notifications.Notify` with the `value` hint (mako, dunst,
swaync, fnott and xfce4-notifyd draw it as a bar) and `x-canonical-private-synchronous`, replacing
the previous bubble by the id kept in `$XDG_RUNTIME_DIR/universe/osd-notification`, so bubbles from
the watcher and the modules do not stack. Cursor hiding leaves a timeout the user set alone (Sway's
config file, Hyprland's option, KWin's duration) and records what it changed in the session's
`Undo::Cursor`, so the stop — `ExecStopPost` in another process — puts it back. The modules reach the
profile through two hidden commands: `universe osd [--level <0-1>] <icon> <label>` and
`universe desktop-shot <path> [--screen <connector>] [--window] [--cursor]`. On Sway keep-awake needs
nothing more: swayidle waits on logind's idle lock.

## Settings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `settings()` | `settings()` | `universe config get` | resolved `config.toml`: absolute paths, defaults applied, `config_file` and `data_home`, `config_writable` (false when the file exists and cannot be written, as a home-manager install with `settings` set — every write is refused with a message naming it), `config_owner` (`home-manager` when the file is a link into the Nix store, empty for a file of the user's own), `os` (`nixos`, `arch`, `fedora`, `debian` or `other`: the family fixes are worded for), `protons` (see Proton and Wine), and `set`: the file's own keys as written, so a frontend can tell a chosen value from a default |
| `set_setting(key, value)` | `set_setting(key, value)` | `universe config set <key> <value>` | dotted `config.toml` key (`launch.proton`, `paths.recordings_root`, `desktop.profile`); a `launch.*` key is validated against the catalogue, an unknown or game-only one refused |
| `launch_keys(scope, screen)` | `launch_keys(scope, screen)` | `universe launch-keys [--json]` | the launch keys of `scope` (`game`, `global`, `both`) that have a settings row: `[{key, type, default, choices, label, section, scope, runners, description, advanced}]`, `type` one of bool, toggle (`auto`, `on`, `off`), int, string, path, list, enum, resolution, refresh, fps, proton, map; `screen` (a `screen_mode`, or none) sizes the resolution, refresh and fps choices; `runners` empty means every runner; `advanced` puts the row behind the settings pages' Advanced toggle (every card but Display, Overlay and the Proton basics; a page folds such a card into a basic one). `runner`, `runner_exe`, `exe` and the `options` map are settable but not listed — the frontends build their rows themselves; the CLI's table prints all of them |
| `gpu()` | `gpu()` | — | the GPU the games run on: `{vendor (amd, nvidia, intel), name (the vendor's), rdna (1…4 or null), label (`AMD · RDNA 3`), fits: {dlss_upgrade, fsr4_upgrade, xess_upgrade, optiscaler}, auto: {the same keys}, vaapi (`/dev/dri/renderD129` or null)}`, `fits` whether each upscaler upgrade does anything on it, `auto` what the key's `auto` comes to on it; `null` when sysfs shows no card of a known vendor. Vendor and AMD generation come from `/sys/class/drm` (amdgpu's `ip_discovery` GC major: 10 RDNA 1/2, 11 RDNA 3, 12 RDNA 4); the card with the most VRAM wins (an NVIDIA card, which reports none, beats an iGPU). `vaapi` is the render node of the strongest card not on NVIDIA's own driver, which ships no VAAPI; renderD numbers follow probe order, so it is read from the card, never assumed. Probed once per process |
| `screen_mode(screen)` | `screen_mode(screen)` | `universe screen-mode [<screen>] [--json]` | `{screen, width, height, refresh, vrr}`: the connector's current mode as gamescope is told it (see Gamescope) and whether it takes a variable refresh rate, `screen=""` for the profile default; zeros when none can be read |
| `form(form, screen)` | `form(kind, id, screen)` | — | one settings form's fields, in form order: `Form::Launch` (the global launch keys every runner takes, then the other `config.toml` keys a frontend offers: Overlay, Folders, API keys, Desktop, and the `[proton]` builds), `Form::Runner(id)` (its program, arguments and gamescope switch, the global launch keys of its kind, its options), `Form::Game(id)` (the runner, program, platform, the runner's program and options, the game's launch keys of its runner's kind, the library flags, each enabled module's game settings, its source's), `Form::Module(id)` and `Form::Source(id)` (the switch, then the global and config-only settings, then in a Game defaults section the game settings' global values: what every game takes unless it sets its own, where a value for all games lands and where a reset undoes it). Under Steam a form leaves out what Steam owns, as `launch_keys` does. Python names the form by `kind` (`launch`, `runner`, `game`, `module`, `source`) and `id` (empty for `launch`). `screen` (a `Mode`, a `screen_mode()` dict in Python, or none) sizes the resolution, refresh and frame rate choices and says what `auto` comes to |
| `set_field(form, key, value)` | `set_field(kind, id, key, value)` | — | writes one field through the setter its form stands for (`set_setting`, `set_runner_setting`, `set`, `enable_module`, `set_module_setting`, `enable_source`, `set_source_setting`); `""` clears the form's own value, so the field falls back to what it inherits |
| `promote_field(form, key)` | `promote_field(kind, id, key)` | — | a game's own value of a `promotable` field becomes its global twin's (`config.toml`, the runner's `[runners.<id>]`, the module's or source's `[modules.<id>]`, `[sources.<id>]`), then leaves the game, which follows it; games that set their own keep theirs. `Invalid` on another form, on a key without a twin, or when the game sets none |
| `set_field_all(form, key, value)` | `set_field_all(kind, id, key, value)` | — | `value` for every game a game's field reaches (its `reach`): written to the global twin as `promote_field` writes it, then the game's own value goes, so the game follows it too; games that set their own keep theirs. `Invalid` where `reach` is empty |
| `forms::entry_key(map, name)` | — | — | a map entry's key with the name cleaned to one table key (letters, digits, `_`, `-`): `launch.env.DXVK_HUD`; `None` when nothing is left |
| — | `version()`, `data_home()`, `state_home()` | `universe --version` | `version()` is the build: the semver with the short git rev behind it (`0.0.2 (410391a)`, `-dirty` when the tree was; the flake passes its rev, a checkout asks git) |

A `Field` is `{key, label, type, section, advanced, description, own, value, inherited, origin,
fallback, resettable, promotable, reach, choices, free, dynamic, required, resolved, inherited_resolved, fits, entries,
keywords}`. `type` is a launch key's
(bool, toggle, int, string, path, list, enum, resolution, refresh, fps, proton, map), a module's or
source's setting type, or `secret` for an API key; `section` is the card it sits in and `advanced`
puts it behind the settings pages' Advanced toggle. `own` is what the form's own scope sets
(`game.toml` for a game, `[runners.<id>]` for a runner, `config.toml` for the others), as
`set_field` writes it, empty when it sets nothing; `value` is what applies, `own` or else
`inherited`. `origin` says where `value` comes from — `game`, `runner`, `global` (`config.toml`
sets it), `default` (nothing does), `found` (a runner's program found on PATH) — and is absent for a key with nothing to fall back to (the runner, the program, a
wrapper); `fallback` is where `inherited` comes from, `own` set or not (what a reset would show);
`resettable` is `own` set on a field that has an `origin`; `promotable` is a game's own value of a
key with a global twin (`promote_field`). `reach`, on a game's form, names the games a value for all
games reaches, read after "All": `games`, the runner's (`Dolphin games`) for a runner option, the
kinds a launch key or a module takes (`Proton and Wine games`, `emulated games`), a setting's own
platforms or runners (`Nintendo Wii games`; past two of them, `games it applies to`), a source's
(`GOG games`); empty for a key that is the game's alone (the runner, the program, a module's switch).
`choices` are
`[{value, label}]` (a runner's label is its name), `free` lets a value outside them be typed (a
resolution, a refresh or frame rate, a number, a Proton path), `dynamic` says the module or source
lists them at run time (`module_setting_choices`), and `required` that it does nothing until the
field has a value. `resolved` is what an `auto` — or a gamescope scaling key left unset — comes to
on this machine: the screen's `2560x1440` or `144`, the refresh gamescope is given for a frame
rate limit, the GPU's say for an upscaler upgrade, the screen's variable refresh for adaptive
sync, gamescope's own `linear` filter; empty when nobody knows; `inherited_resolved` is the same for
`inherited`, what a reset would come to. `fits` is whether the GPU gains
anything from the upscaler the key turns on; `keywords` are a module's or source's setting's search words. A `map` field (`launch.env`, `launch.dll_overrides`,
`proton`) holds its `entries` (`{name, value, origin, resettable, promotable}`: on a game's form an entry is
the game's own or the global's, which only the Launch form removes) and no value; an entry is
written as `<key>.<name>` (`entry_key`), `""` removing it.

## Controller

| Rust | Python | CLI | Role |
|---|---|---|---|
| `controller_state()` | `controller_state()` | `universe controller ls` | `{enabled, hold_ms, volume_step (a percent, number), home_summons, families: [{id, name, slots: [{id, label, codes, extra}]}], macros: [Macro], presets: [{id, label, hold_only}]}`; the CLI adds `devices`, the pads readable now with every slot's code or `bound: false` |
| `controller_pads()` | `controller_pads()` | — | the `devices` list alone, in the shape `watch` announces; for a frontend whose watcher waits on the lock |
| `set_controller_macro(macro)` | same | `universe controller bind <family> <button> <press\|hold> <action> [--keys K] [--command C]` | validates, replaces the macro with the same family, button and trigger |
| `remove_controller_macro(family, button, trigger)` | same | `universe controller unbind <family> <button> [trigger]` | an empty trigger removes both |
| `set_controller_button(family, slot, codes)` | same | `universe controller learn <family> <slot>` · `forget` | the codes a slot answers to: a list (empty leaves it unbound), `None` restores the seeds. `learn` reads the pad instead: the next button pressed becomes the slot's, taken from whichever slot had it |
| — | — | `universe controller watch [--json] [--wait]` | the engine |

`Macro` = `{"family": "dualsense-edge" | "*", "button": "paddle_left", "trigger": "press" | "hold",
"action": "volume_up" | "volume_down" | "mute" | "screenshot" | "mangohud" | "stop" | "keys" |
"command", "keys": "Super_L+F12", "command": "…"}`. `stop` is hold-only. `volume_up` and
`volume_down` repeat while held (400 ms, then every 100 ms), unless the slot also carries a hold.
A slot with only a press macro fires on the key down; with a hold macro too, press fires on a release
before `hold_ms` and hold once at `hold_ms`. `volume_up`, `volume_down` and `mute` go straight to
WirePlumber through `wpctl`: the default sink's volume moves by `volume_step` percent, clamped to
[0, 100 %], `mute` toggles the sink; no key is typed, so nothing reaches the game. The new level
is reported as a `volume` event (`{"event": "volume", "percent", "muted", "output"}`, the output
labelled as GNOME's own volume keys print it: the sink's active route, else the sink, as
`outputs()` labels it). Inside the launcher's gamescope the launcher's overlay draws it over the
game, whatever the desktop; on the desktop it shows on the desktop's OSD (see Desktop), and where
there is none the macro runs silently. `screenshot` is `screenshot()`, reported back as a `screenshot` event
(`{"event": "screenshot", "path"}`) that the launcher turns into its flash and shutter. `keys` types through uinput; `mangohud` is `set_mangohud(None)` — no key: the running
game's `launch.mangohud` flipped and the HUD told (see MangoHud) — and the watcher reports the
outcome as a `hud` event, which the launcher toasts: "MangoHud shown · <title>", "MangoHud hidden ·
<title>", "MangoHud: no game running".

Families: `steam-deck` (grip_l4, grip_l5, grip_r4, grip_r5, quick, pad_left, pad_right: hid-steam's
`BTN_GRIPL`, `BTN_GRIPL2`, `BTN_GRIPR`, `BTN_GRIPR2`, `BTN_BASE`, `BTN_THUMB`, `BTN_THUMB2`; its
analog triggers on `ABS_HAT2Y` and `ABS_HAT2X`), `dualsense-edge` (fn_left, fn_right, paddle_left, paddle_right), `dualsense`,
`dualshock4`, `xbox-elite` (paddle_p1…p4), `xbox` (share), `switch-pro` (capture), `8bitdo-pro-3`
(paddle_l4, paddle_r4, paddle_pl, paddle_pr, star), `generic`; every family has the standard slots
`south east north west lb rb lt rt select start guide ls rs dpad_up dpad_down dpad_left dpad_right`.
A slot's codes are candidates: on every connect the first one the pad advertises in its capabilities
wins, so the Edge's paddles (`BTN_TRIGGER_HAPPY1…4`, kernel ≥ 7.2) and the Elite's (`BTN_GRIP*` over
xpadneo or xone, `BTN_TRIGGER_HAPPY5…8` on older drivers) resolve without a hardcoded number, and a
slot with no code present is reported unbound.

On a Deck (`deck_model()`, below) Steam Input's virtual pad, `28de:11ff`, is taken for the
built-in controls: under Steam that is all the watcher sees of them. A frontend with no pad seen
yet shows the Deck's glyphs there, since SDL's hidraw driver may also take the controls from
hid-steam's evdev node. Without Steam and without InputPlumber, hid-steam starts the Deck in its
mouse-and-keyboard mode, and holding Menu for about half a second switches it to a gamepad.
Off a Deck the same pad is Steam's copy of a real one: `watch` and `ls` leave it out while any
real pad is connected, so the pad shown is the one with its own family and extra buttons.

`watch` reads every `/dev/input/event*` that advertises `BTN_GAMEPAD` **without grabbing it** (a
game, SDL or Proton reads the same node untouched), rescans every 2 s (hotplug, and pads
InputPlumber hides by chmod 000 are dropped while hidden), and with `--json` speaks one object per
line: out — `{"event":"ready"}`, `{"event":"device","id":"event30","name","family","family_name",
"bus","vendor","product","slots":{"<slot>":{"code","bound"}},"axes":{"<role>":"ABS_Z"},"sdl":{"<slot>":"b3"},"sdl_axes":{"<role>":"a2"},"battery"}`
(`axes` names the evdev axis behind `lx ly rx ry lt rt`, by the pad's shape — Z/RZ are triggers next
to RX/RY, the right stick without them — or as `[controller.axes.<family>]` learned it, `-` on one
thrown backwards; `sdl`/`sdl_axes` number the same as SDL's Linux joystick does, `~` on a backwards
axis; `battery {percent, charging}` when the watcher reads it itself, since the kernel keeps no
supply for the pad — an 8BitDo's from byte 14 of its HID report, a Bluetooth pad's from the GATT
Battery Service BlueZ reads (`org.bluez.Battery1`, an Xbox pad in BLE mode: no charging state, so
`charging` false) — else null), `gone {id}`, `button {id, slot, code, pressed}`,
`battery {id, percent, charging}` (on change, and when BlueZ's service turns up after the pad),
`unknown {id, code}` (a key no slot owns), `macro {id, slot, trigger, action, keys, command}`,
`hud {shown, title}` (after a `mangohud` fire: `shown` null when no game runs), `learned {family, slot, code, from}` or `learned {family, axis, code}`, `learn_timeout`, `waiting` / `busy` (the lock), `error
{message}`, and while `axes` is on, `axis {id, axis, value}` (`lx ly rx ry` as -1..1, `lt rt` as
0..1, a hundredth's resolution, on change); in — `{"cmd":"suspend", "dock"?}` (report, do not fire; with `dock` true the presets marked docked — volume, mute, MangoHud — still do),
`resume`, `axes {on}` (stream the sticks and triggers: the page's test mode), `reload` (config
changed), `learn {id, slot}` or `learn {id, axis}` (a role `lx ly rx ry lt rt`, taken from the axis
held past 40 % for 150 ms, the furthest thrown when several: `ABS_Z`, or `ABS_Z-` when it went the other way; `except`, a list of
axis names, keeps those from answering — the walk names the stick it has already placed), `cancel`, `rumble {id}`, `run {action, keys, command}` (fire an action as a
macro would — `mangohud`, `keys` with a combo, `screenshot`…: the launcher's home menu types
through the watcher, the process that owns the key typist, and only once the game is thawed), `quit`. Stdin's end stops a `--json` watcher. A
watcher also reloads by itself when `config.toml`'s mtime moves (checked on the 2 s scan), so a
bind from a terminal or from a launcher whose own watcher is waiting reaches the one holding the
pads. Either reload rereads config.toml and the module list only, never the library, so pad
events are not held up behind a rescan.

## System

The machine's own controls, for a frontend running outside Steam (in Game Mode Steam's Quick
Access menu has them).

| Rust | Python | CLI | Role |
|---|---|---|---|
| `deck::model()` | `deck_model()` | — | `lcd` or `oled` on a Steam Deck (DMI: Valve's Jupiter or Galileo board), else none; `UNIVERSE_DECK=lcd\|oled\|none` stands in for the read |
| `system_controls()` | `system_controls()` | — | `[{id, label, detail, kind, value, min, max, step, unit, choices}]`, each only where this machine has it and this user can set it: `brightness` (range, %: the first `/sys/class/backlight`, amdgpu's first), `refresh` (range, Hz: a Deck's panel, 40–60 LCD or 45–90 OLED, on the launcher's own gamescope straight on the screen only), `tdp` (range, W: amdgpu's hwmon `power1_cap`, bounded by `power1_cap_min`/`_max`), `gpu` (choice: `auto` or a clock in 100 MHz steps over `pp_od_clk_voltage`'s `OD_RANGE`), `fan` (toggle: SteamOS's `jupiter-fan-control` service, off leaving the fan to the firmware); empty under Steam |
| `set_system(id, value)` | `set_system(id, value)` | — | the machine's own value: applies it (but not while the running game holds its own `[system]` value, whose session's end puts this one back instead) and, but for `brightness`, keeps it in `[system]`. The backlight goes through logind's `Session.SetBrightness` (no polkit), else the file; the power cap and the clock through the file when it is writable, else SteamOS's `steamos-priv-write` (which leaves the file writable after its first write); a pinned clock writes `manual`, then `s 0 N`, `s 1 N` and `c`; the refresh through gamescope's `GAMESCOPE_DYNAMIC_REFRESH`; the fan through `jupiter-fan-control --enable\|--disable`. `Invalid` on an unknown id or value, `Unavailable` where the machine has no such control or the helper refuses |
| `set_system_for(game, id, value)` | `set_system_for(game, id, value)` | — | the game's own value, kept in its `game.toml [system]`: applied now when the game is the one running, else at its next launch. A launch puts the game's `[system]` values on after reading the machine's, which its session's end puts back (a `system` step in the marker's `undo`); one set mid-session is read the same way first. A Steam Deck's per-game profile, the quick settings' A |
| `set_system_all(game, id, value)` | `set_system_all(game, id, value)` | — | the game lets its own value go and `set_system(id, value)` makes it the machine's own, which every game without its own then runs with: the quick settings' Y |
| `apply_system()` | `apply_system()` | — | `[system]` put back, then the running game's own over it, a failure logged: the power limit and the clocks do not outlive a reboot. The UI calls it at startup |

## config.toml

Defaults as the core ships them:

```toml
schema = 1

[paths]                              # defaults follow XDG and xdg-user-dirs
games_root = "~/Games"               # $XDG_GAMES_DIR: where sources install
prefixes_root = "~/.local/share/universe/prefixes"
saves_root = "~/.local/share/universe/saves"      # ludusavi's backups, one folder per game (see Game data)
recordings_root = "~/Videos/universe"            # $XDG_VIDEOS_DIR/universe

[launch]
proton = "proton-ge"                 # a name under [proton], a path, or a family found under Lutris, Steam or Heroic (see Proton and Wine)
esync = true
fsync = true
ntsync = true                        # a sync mode off is PROTON_NO_*=1 (WINEESYNC/WINEFSYNC=0 for wine)
wayland = true                       # PROTON_ENABLE_WAYLAND=1; dropped inside gamescope unless --expose-wayland
hdr = false                          # PROTON_ENABLE_HDR=1, and --hdr-enabled on gamescope
discrete_gpu = true                  # a hybrid laptop: the game and gamescope render on the stronger GPU, not the screen's (DRI_PRIME or PRIME offload, --prefer-vk-device)
dlss_upgrade = "off"                # PROTON_DLSS_UPGRADE, PROTON_FSR4_UPGRADE, PROTON_XESS_UPGRADE, PROTON_USE_OPTISCALER
fsr4_upgrade = "off"                # auto | on | off (or a bool): auto is on where the GPU makes it a plain win; on RDNA 3 PROTON_FSR4_RDNA3_UPGRADE instead
xess_upgrade = "off"
optiscaler = false
debug_log = false                    # Proton, Wine and DXVK verbose logs into <state>/logs/<id>/<session>/ (see Logs)
mangohud = false                     # the HUD shown at launch (loaded and hidden when false, the limit still holds); flipped in game by the dock and the mangohud macro, which write it back
gamescope = true                     # every game inside gamescope: one window, black until the game draws
gamescope_args = ""                  # after the flags below, and over them; --expose-wayland keeps PROTON_ENABLE_WAYLAND
gamescope_bin = "gamescope"          # a name on PATH (/run/wrappers/bin included) or a path
gamescope_resolution = "auto"        # the screen's mode, or WxH: what the game renders at (-w -h); the output is always the screen
gamescope_refresh = "auto"           # the screen's rate, or Hz (-r)
gamescope_scaler = ""                # auto | integer | fit | fill | stretch (-S); empty: gamescope's default
gamescope_filter = ""                # linear | nearest | fsr | nis | pixel (-F)
# gamescope_sharpness = 2            # 0 (sharpest) to 20, for fsr and nis (--sharpness)
gamescope_adaptive_sync = "auto"    # --adaptive-sync: auto when the screen has variable refresh, or on | off
fps_limit = "auto"                   # MangoHud's limiter in the game: auto (the refresh the game sees), none, or frames per second
pause_on_home = true                 # freeze the game while the launcher covers it (HOME); off for a game that must keep running

[desktop]
profile = "auto"                     # auto | gnome | kde | cinnamon | sway | hyprland | niri | x11 | none (see Desktop)
hide_cursor = true
cursor_extension = ""                # empty: the Universe extension hides the pointer after 5 s at rest; another extension's uuid is enabled for the session instead, restored to its prior state after
keep_awake = true                    # the desktop's idle inhibitors held for the session (see universe keep-awake): a pad is no activity to it, and the screen would blank and suspend mid-game
whats_new = false                    # the first start after an update opens what changed since the version last run (whats_new()); Settings › Launch › Updates in every look and Universe Desktop

[saves]                              # see Game data
auto_backup = true                   # a backup of the game's saves after each session
keep = 5                             # backups kept per game (1 to 255), the oldest going first

[system]                             # the machine's controls as last set (see System), put back at the launcher's start outside Steam; "" leaves one as the system has it; a game's own game.toml [system] holds the same ids, brightness too
tdp = ""                             # watts
gpu = ""                             # auto, or MHz
refresh = ""                         # Hz, a Deck's panel on the launcher's own gamescope
fan = ""                             # on | off: SteamOS's fan curve

[proton]                             # name → path, none by default; a name with no path here is looked for as a family (GE-Proton10-4 for proton-ge) under Lutris, Steam, umu and Heroic
# proton-em = "~/opt/proton-em"

# [runners.<id>]                     # per runner (`universe runner set`): exe (absent: detected), build ("" the system's program first, latest, or a version Universe installed), args, gamescope, its options

[components]
auto_update = true                   # Universe's own builds follow the catalogue, the previous one kept (see Components)
catalogue = ""                       # a URL or a file; empty: the project's catalogue branch

[modules]
enabled = []                         # a module is opt-in: `universe module enable capture`, or Settings › Modules

[modules.capture]                    # the settings rows: `universe module settings capture`
# ffmpeg_video_opts = "rc_mode=CQP;qp=20"   # config-only: replaces the quality preset
# gsr_extra_args = "-cr full -keyint 2"      # config-only: appended to gpu-screen-recorder

[sources]
enabled = ["gog"]

[sources.gog]                        # the settings rows: `universe source settings gog`
# platform = "linux"

[keys]
sgdb = ""                            # optional, your own: SteamGridDB's art joins the picker; or sgdb_file, a file holding it
sgdb_file = "~/.config/steamgriddb/api_key"
prefer_sgdb = false                  # with a key, SteamGridDB's art fills a slot before the stores' and GamesDB's

[controller]
enabled = true
hold_ms = 600                        # a press this long is a hold
volume_step = 2                      # percent of the normal volume per press, 1–100 ("precise" = 2, "normal" = 6 still read)
home_summons = true                  # HOME pressed in another app brings Universe's big screen up; false drops the press
# [controller.buttons.xbox-elite]    # learned codes: a slot's list replaces its seeds, [] leaves it unbound
# paddle_p1 = ["BTN_GRIPR", "BTN_TRIGGER_HAPPY5"]
# [controller.axes.8bitdo-pro-3]     # learned axes: a role's evdev axis, `-` when the pad reads it backwards
# rx = "ABS_Z"
# [[controller.macros]]              # {family, button, trigger, action} (`universe controller bind`); absent: the seeded workflow, `macros = []` none at all
```

## Module protocol

A module runs hooks around a session. It is a directory `modules/<id>/` — system-wide under
`$out/share/universe/modules/<id>`, per-user under `$XDG_CONFIG_HOME/universe/modules/<id>`, where a
user module overrides a system one with the same id. It holds a `module.toml` and its executables.
**The core never loads module code**; it only runs the executables.

```toml
api = 2
id = "capture"
name = "Video capture"
version = "0.0.10"
description = "Records each session."   # optional, one or two sentences; the module's page shows it under the name

[requires]
core = ">=0.0.9, <0.1.0"          # optional: comparators (>=, >, <=, <, =) on Universe's version, comma-separated, a bare version meaning >=
bins = ["gpu-screen-recorder"]    # a missing binary makes the module "unavailable" and it is never run
system = ["gpu-screen-recorder"]  # system tools (gamescope, mangohud, gpu-screen-recorder) Settings › Runners proposes to install while the module is on

[hooks]                           # paths relative to the module directory
pre-launch   = "bin/pre"          # blocking, before the game's unit; may write UNIVERSE_ENV_FILE
post-launch  = "bin/start"        # async (transient unit); anything that must last the session goes in its own unit with BindsTo=$SESSION_UNIT
freeze       = "bin/freeze"       # blocking, right after the game's unit froze (HOME over the game); thaw = right after it thawed
thaw         = "bin/thaw"
session-end  = "bin/stop"         # short and blocking, inside the game unit's ExecStopPost; this is where a recording is filed
post-process = "bin/process"      # async (transient unit), after the session-end hooks
screenshot   = "bin/shot"         # on demand
check        = "bin/check"        # blocking, from doctor, while the module is on (available or not) and set up: one check per stdout line
timeout_s    = 20                 # for blocking hooks; the sum bounds the game unit's TimeoutStopSec

[limits]                          # applied to the transient units of async hooks
cpu_weight   = 100                # default 20
memory_high  = "4G"               # default 2G

[applies]                         # optional: the games whose page shows the game-scope settings
runner_kinds = ["emulator"]       # emulator, proton, wine, linux; absent or empty: every game

[[settings]]
key = "enabled"                   # reserved: always present, game scope
type = "bool"
default = true
label = "Record the session"
scope = "game"                    # global → config.toml [modules.<id>]; game → game.toml [modules.<id>];
                                  # config → config.toml / `universe module set`; its row sits behind the page's Advanced toggle
advanced = true                   # behind the settings page's Advanced toggle (a config-scope setting always is)

[[settings]]
key = "fps"
type = "int"
default = 60
choices = ["auto", "120", "60"]   # suggestions on an int or a string; a name among them is a value too
label = "Frame rate"              # a few words: the row's name
description = "Auto follows the screen's refresh rate."   # optional: what it does, under the row
choice_labels = { auto = "The screen's" }   # optional: what a choice reads as, by stored value; the value is what is written
keywords = ["fps", "hz"]          # optional: words a settings search finds it by besides its label and description

[[settings]]
key = "wiimote"
type = "enum"
default = "nunchuk"
choices = ["nunchuk", "sideways"]
scope = "game"
runners = ["dolphin"]             # optional, game scope: shown only on these runners' games (canonical ids)
platforms = ["Nintendo Wii"]      # optional, game scope: shown only on these platforms' games
label = "Wii controller"

[[settings]]
key = "model"
type = "string"
default = "gpt-5.6-sol"
choices_exec = "bin/choices"      # `bin/choices model` prints the current choices as a JSON array
label = "Model"

[[settings]]
key = "provider"
type = "enum"
default = ""
choices = ["codex", "openai"]
required = true                   # no usable default: the module runs and does nothing until a value is set
requires_bins = { codex = ["codex"] }   # binaries the chosen value needs; a missing one makes the module unavailable
label = "Writing model"
```

### Hook environment

| Variable | Value | Hooks |
|---|---|---|
| `GAME_ID`, `GAME_SLUG`, `GAME_TITLE`, `GAME_DIR`, `GAME_EXE`, `GAME_TOML` | identity and paths (the TOML is read-only) | all |
| `SESSION_ID`, `SESSION_UNIT`, `SESSION_SCREEN`, `SESSION_STARTED_AT`, `SESSION_ENDED_AT`, `SESSION_DURATION_S` | the session; `SESSION_SCREEN` is a DRM connector | as applicable |
| `RECORDING_PATH` | the filed mkv, empty if there is none | `post-process` |
| `RECORDING_STARTED_AT`, `RECORDING_PAUSES` | the session line's `recording_started_at` and `recording_pauses` (the latter as JSON), empty / `[]` when unknown | `post-process` |
| `JOURNAL_DIR` | `games/<id>/journal` | all |
| `SCREENSHOTS_DIR` | `games/<id>/screenshots`; `<state>/screenshots` for a `screenshot` with no session running | all |
| `MODULE_SETTINGS_JSON` | global settings merged with the game's | all |
| `JOURNAL_REWRITE` | `1` when `journal_write` asked for this session again; the hook replaces the entry instead of leaving it alone | `post-process` |
| `UNIVERSE_ENV_FILE` | write `KEY=VALUE` lines here to add them to the game's environment, ahead of `launch.env`; two keys are not the environment: `UNIVERSE_GAMESCOPE_ARGS` is flags for the game's gamescope (see Gamescope), `UNIVERSE_GAME_ARGS` the game's own arguments, shell-quoted, ahead of `launch.args` (the session's recorded command line holds them) | `pre-launch` |
| `MODULE_DIR`, `MODULE_DATA_DIR` | the module's directory, `$XDG_DATA_HOME/universe/modules/<id>` | all |
| `UNIVERSE_BIN`, `UNIVERSE_{DATA,CONFIG,STATE}_HOME`, `UNIVERSE_{MODULES,SOURCES}_PATH`, `PATH` | the CLI to call back (`recording-file`, `journal-add`, `session-window`, `screen-mode`) and the environment that makes it open the same core | all |
| `UNIVERSE_GAME_JSON` | the resolved `Game`, serialized | all |

Exit codes: 0 is success; anything else is logged and the session continues — except a `pre-launch`
hook, where a non-zero exit cancels the launch.

A `check` hook runs outside any game and session, with `MODULE_SETTINGS_JSON` (the global settings),
`MODULE_DIR`, `MODULE_DATA_DIR`, `UNIVERSE_BIN`, `PATH` and `UNIVERSE_DISTRO` (`nixos`, `arch`, `fedora`,
`debian` or `other`, to word a fix for the distribution). It runs while the module is on, even when a
required binary is missing, so it can say how to get it. Each stdout line that is a JSON object
`{"check", "label", "ok", "detail", "fix", "component"}` is a doctor check of the module's (`label` defaults to
`check`, `fix` shows only when not `ok`, `component` optionally names the component or system tool whose
install fixes it); a line whose `check` is a required binary's replaces doctor's own line for it. A hook
that prints none and exits non-zero, or times out, is one failed `check` check naming its error.

## Source protocol

A source installs and updates games from a store. It is a directory `sources/<id>/` — system-wide
under `$out/share/universe/sources/<id>` (`UNIVERSE_SOURCES_PATH` adds roots), per-user under
`$XDG_CONFIG_HOME/universe/sources/<id>`, a user source overriding a system one with the same id —
holding a `source.toml` and its executable; its data lives in `$XDG_DATA_HOME/universe/sources/<id>`.

```toml
api = 2
id = "gog"
name = "GOG"
version = "0.0.10"
description = "Installs GOG games."   # optional, as a module's
exe = "bin/source"                # run as: bin/source <verb> [args]

capabilities = ["achievements", "uninstall"]   # optional verbs it answers, past the required ones

[requires]
bins = ["gogdl"]                  # a missing binary makes the source "unavailable" and it is never run, unless Universe fetches it (see Fetched tools)

[[tools]]                         # optional: a program Universe fetches when it is not on PATH, a catalogue entry (see Components) with its id;
id = "gogdl"                      # the catalogue's entry of the same id wins, this one stands in when the catalogue lacks it or cannot be reached
name = "heroic-gogdl"
kind = "tool"
bin = "gogdl"
builds = [{ version = "1.3.0", assets = [{ format = "binary", url = "https://…/gogdl_linux_x86_64", sha256 = "cba0…" }] }]

[login]                           # optional: how frontends word the sign-in
kind = "code"                     # code: the page shows a code once signed in (the default); key: the page makes an API key
hint = "Sign in, then enter the code."  # how to get it (a default per kind)
purpose = "install games"         # what signing in is for, after "Sign in to" (the default)

[hooks]                           # optional, as a module's (every session hook but `screenshot`); they run for the games whose source is this one
pre-launch  = "bin/pre-launch"
session-end = "bin/session-end"
check       = "bin/check"         # from doctor while the source is on, as a module's, with SOURCE_SETTINGS_JSON
timeout_s   = 20

[[settings]]                      # as a module's; `scope` is global (config.toml [sources.<id>]) or game (game.toml [sources.<id>] over it)
key = "platform"
type = "enum"
default = "windows"
choices = ["windows", "linux"]
label = "Depot platform"
```

`<exe> <verb> [args]` runs in the source's directory with `SOURCE_SETTINGS_JSON`, `SOURCE_DIR`,
`SOURCE_DATA_DIR` and `UNIVERSE_BIN` in the environment (a `choices_exec` gets the same). A source's
hooks run as a module's do — the same environment, with `SOURCE_SETTINGS_JSON` (merged with the
game's own keys), `SOURCE_DIR`, `SOURCE_DATA_DIR` and `SOURCE_GAME_ID` (`source.id`, the store's id of the game)
in place of the `MODULE_*` names — for the games whose `source.kind` is the source's id, after the
modules' hooks of the same name; a source's `session-end` timeout counts in the game unit's stop
budget.
A `games_dir` setting whose manifest default is empty arrives filled with `paths.games_root`.
One JSON object per line on stdout, human-readable logs on stderr, meaningful exit code.
**Every verb ends with `{"event":"done"}`**, `login` included.

| Verb | Argument | Events |
|---|---|---|
| `login` | `[code]` | without a code: `{"event":"login_url","url":…}`; with one: `{"event":"logged_in","user":…}` |
| `status` | — | `{"event":"logged_in","user":…}` if the session is valid, otherwise just `done`. Probed once per process, on the first listing |
| `library` | | `{"event":"game", …}` per owned title |
| `search` | `<text>` | `{"event":"game", …}` |
| `info` | `<id>` | `{"event":"info","data":{…},"download_size":…,"disk_size":…}`, the sizes optional |
| `install` | `<id>` | `progress` lines (`done`/`total` in bytes when the source knows them), then the installed `game`. On SIGTERM the source stops its downloader, keeps the files and exits non-zero; a later `install` of the same id resumes |
| `update` | `[id]` | without an id: `{"event":"update","id","title","local_build","remote_build","version","date"}` per pending update; with one: `progress` then `game` |
| `scan` | | `{"event":"game", …}` per installation found (`disk_size` measured) and per stopped download (`installed: false`, `partial_dir`, `partial_bytes`), `owned` crossed with the cached library |
| `achievements` | `<id>` | `{"event":"achievement", …}` per achievement of the game, an `Achievement` (see Achievements); only asked of a source whose `capabilities` names it |
| `uninstall` | `<id>` | optional `progress` lines, then `done` once the store has removed the game's files and forgotten the install (nothing to do is no error); only asked of a source whose `capabilities` names it, by `uninstall(id)` instead of trashing `source.dir` |
| `cloud-saves` | `<id> [status\|download\|upload\|keep-local\|keep-cloud]` | one `{"event":"cloud","enabled","state","message","at","locations"}`, the game's sync state once the action ran (see Cloud saves); `status`, the default, reads what the last sync kept and reaches no network; only asked of a source whose `capabilities` names it, with the game's hook environment (`GAME_ID`, `GAME_DIR`, `UNIVERSE_GAME_JSON`) and its game-scope settings |

```json
{"event":"game","id":"1434554947","title":"Mini Metro","dir":"/mnt/games/PC/Mini Metro",
 "exe":"MiniMetro.exe","build":"5904…","owned":true,"installed":true,
 "release_year":2015,"dlcs":[],"disk_size":167772160}
{"event":"game","id":"1207658930","title":"Alan Wake","owned":true,"installed":false,
 "partial_dir":"/mnt/games/PC/Alan Wake","partial_bytes":3400000000,"download_size":7900000000,"disk_size":8200000000}
{"event":"progress","done":123,"total":456,"message":"27.0%"}
{"event":"done"}
```

`exe` is relative to `dir`. `owned` may be `null` when the source cannot tell. A `game` may carry
`umu_id` (the game's umu-database id, `umu-<n>`) and `store` (umu's name for the store: `gog`, `egs`,
`itchio`…), which pick its protonfixes, and `runner` (`linux` for a native build), which sets
`launch.runner` and the platform while the game has no runner of its own; without one the game runs
through Proton. A `game` may also carry `prefix` and `proton`, the absolute paths of the prefix the
store keeps for it and of the Proton the store runs it with, which a game entering the library takes
as its own. A `game` may carry `steam_appid` (the game's Steam app id, a number or its digits:
Steam's CDN art, store page and screenshots for it, whatever the store) and `art`, slot (`box_front`,
`square`, `banner`, `background`, `logo`) → the URL of the store's own picture for it, which the
next `media_refresh` takes before any other provider; both land in the game's `media/.sync.json`
(`source_appid`, `source_art`), not in `game.toml`. `image` is the store listing's picture and no
more. The **steam** source hands `steam_appid`; **epic** its `keyImages` as `art` (`DieselGameBoxTall`
the box front, `DieselGameBox` the background, `DieselGameBoxLogo` the logo); **gog** a catalogue
hit's `coverVertical` as the box front — its library has only a wide picture, so a GOG game's art
comes from GOG GamesDB by its id. The core writes `library.json` (the last `library` run's games) in the source's data dir after every listing; a source reads it for ownership and writes nothing there itself.

Any verb may emit `{"event":"window","class":"steam","title":"Install"}` before it waits on the
user in another program's window: the one whose `WM_CLASS` class is `class`, titled `title`, else
that program's newest (titles follow its language; an empty `title` means the newest). Inside the
launcher's gamescope the core keeps that window the one shown among the program's windows, their
`STEAM_GAME` at 0, until the verb ends (see Gamescope); elsewhere the event changes nothing.

---

Writing a frontend on top of this API: [`frontends.md`](frontends.md).
