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
  order (the cursor hiding, InputPlumber, `post_command` for `pre_command`) as the marker lists
  them, then runs the `session-end` hooks, then the `post-process` hooks.
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
  controller.lock`) hands the pads over between them. Once both are gone nothing runs.
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
| `remove(id, purge)` | `remove(id, purge)` | `universe rm <name> [--purge]` | parks recordings under `.archive/` (the journal stays in `games/<id>/journal/`), marks `removed_at`; `purge` also trashes the prefix |
| `uninstall(id)` | `uninstall(id)` | `universe uninstall <name>` | trashes `source.dir` and clears `source.dir`, `source.build_id` and `launch.exe`; the game stays in the library, not installed. Refuses a root, a home or the games root |
| `reload_all()` | `reload()` | — | rereads config and `games/*/game.toml` |
| `rescan()` | `rescan()` | `universe rescan` | `reload_all`, then `import_roms(true)`: its report |
| `reload_game(id)` | `reload_game(id)` | — | rereads one game |
| `import_lutris(apply)` | `import_lutris(apply)` | `universe migrate [--apply]` | a report of what it read from Lutris's own folders (`~/.config/lutris`, `~/.local/share/lutris/pga.db`, under the XDG homes): imported games, per-game env diff (`{id, lutris_env, universe_env, added, removed, changed}`), imported hours and `runners` (what Lutris's runner configs say: a wrapper script is seen through, the program is written to `[runners.<id>] exe` when it is not on PATH, its extra arguments to `args`). Without `apply` it only reports |
| `discover()` | `discover()` | `universe discover [--json]` | what other launchers hold on this machine, nothing written: `{launchers: [{id, name, found, dir, games, titles, importable, via, detail}], gog_dirs}`. `lutris` (`via: "lutris"`: the installed games `import_lutris` would add), `steam` (the `appmanifest_*.acf` of every library in `libraryfolders.vdf`, Proton and the runtimes left out), `heroic-gog` (`via: "gog"`: `goggame-*.info` installs one level under `gog_dirs` and `paths.games_root`), `heroic-epic` and `heroic-amazon` (legendary's and nile's `installed.json`), `roms` (`via: "roms"`: the games `import_roms` would add, `dir` the folders it read). `importable` names what Universe launches itself; `titles` holds the first six. `gog_dirs` are Heroic's GOG install folders (its `gog_store/installed.json` and default install path), for the gog source's `scan_dirs` before a `scan` |
| `import_roms(apply)` | `import_roms(apply)` | — (`universe rescan` applies; `universe discover` reports) | the games under the folders the installed emulators list in their own configuration — Eden and its forks' `Paths\gamedirs` (with `deep_scan`), Dolphin's `ISOPath*` (`RecursiveISOPaths`), Ryujinx's `game_dirs`, RPCS3's `games.yml` entries and `dev_hdd0/game`, PCSX2's and DuckStation's `[GameList]` paths, Cemu's `GamePaths`, shadPS4's `installDirs`, Flycast's `ContentPath`; melonDS and mGBA keep no list — as `{folders: [{runner, dir, recursive}], imported: [{id, title, runner, path}], skipped: [{path, reason}], applied}`. A file is a game by the runner's extensions; RPCS3 and shadPS4 games are folders (`PS3_GAME/USRDIR/EBOOT.BIN`, `<id>/eboot.bin`, titled from `PARAM.SFO`), Cemu's `code/*.rpx` from `meta/meta.xml`. `updates`, `dlc`, `mods`, `firmware`, `amiibo`, `backups`, `downloads`, `prefixes`, `saves`, `textures`, `shaders` and hidden folders are not entered, a `.bin` beside a `.cue`/`.gdi`/`.m3u` is a track, a shadPS4 `-UPDATE` folder a patch. A file already in the library (same inode: a bind mount counts once) is left out, and so is one whose title is taken by another file or by a library game (`skipped` says which); a plain name beats a tagged one for the same title. With `apply`, each is `add_game`d and its art fetched |
| `add_game(spec)` | `add_game(spec)` | `universe add <file> --runner <id> [--title T] [--platform P] [--media]` | `{"runner", "exe", "title"?, "platform"?}` → the new id. The title defaults to the file's name cleaned of release tags; the platform to the runner's first. Refuses an id already in the library |

`set` takes dotted keys: the `launch.*` keys of `universe launch-keys` — the one catalogue
(`launch_keys.rs`) every `[launch]` key is declared in, with its type, default, scope and choices;
a game key left empty takes `[launch]`'s, a value is validated as the key's type says, an unknown
or global-only key is refused — with the maps `launch.dll_overrides.d3d11`, `launch.env.FOO` and
`launch.options.<key>` (validated against the runner's options); then `desktop.hide_cursor`, `hidden`,
`favorite`, `tags`, `sort_title`, `platform`, `metadata.sgdb_id`, and `capture.cursor` as a
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
               "options": {"batch": true, "user_directory": "", "inputplumber": true}, "inputplumber": true,
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

There is no change notification: the files are the truth, so a frontend watches `games/`,
`games/<id>/{,journal,journal/attachments,media,media/picked,media/picked/screenshots,screenshots}` and `state/` and rereads. Everything the CLI, `session-end` and the
hooks write shows up that way, with no other channel.

## Sessions

| Rust | Python | CLI | Role |
|---|---|---|---|
| `launch(id, screen, splash)` | `launch(id, screen, splash="")` | `universe play <name> [--screen DP-1] [--no-wait]` | pre-launch hooks, marker, `StartTransientUnit` on the user manager, post-launch hooks; returns the `session_id` at once. `screen` is a DRM connector name or `""` for the first the display server is drawing on (`status` `connected` **and** `enabled` not `disabled`, alphabetical; the cabled ones when that leaves none) — a cable to a dark monitor is not a screen a recorder can name; `splash` a poster for gamescope's keep-alive window (see Gamescope) or `""`. `Busy` if a session is already running |
| `stop(session_id)` | `stop(session_id)` | `universe stop` | SIGTERM to the game's processes (those under `universe splash`; every process of the unit when there is no gamescope of its own), up to 10 s for them to exit on their own terms — an emulator saves its caches — then a stop job on the unit, waited for 10 s at most. The SIGTERM is repeated every 3 s unless the runner's spec says once (`term_twice`): Dolphin takes the first as a "quit?" prompt and needs the second; Eden's handler resets to the default disposition, so a second would kill it mid-shutdown. Eden with its `confirmStop` at the default asks "close?" on the first and is killed by the stop job: doctor's `runner-eden-stop` says so |
| `session_window()` | `session_window()` | `universe session-window [--json]` | the running game's window as the desktop lists it (see Desktop; `{id, pid, wm_class, title, focused, x, y, width, height, hidden, minimized}`, `id` the desktop's own handle as a string): the largest visible toplevel whose pid is in the unit's cgroup — gamescope's when the game runs inside it. `None` before it maps; `Unavailable` where the profile has no window list |
| `wait_session_window(session_id, timeout)` | `wait_session_window(session_id, timeout_ms)` | `universe session-window --wait <secs> [--json]` | blocks until that window is up, then `Activate`s it (focus and raise) and returns it; `None` when the session ended first or the timeout ran out (the CLI prints `null`, exit 0); `Unavailable` where the profile has no window list, at once. Polls the desktop every 150 ms |
| `focus_session()` / `focus_pid(pid)` | `focus_session()` / `focus_pid(pid)` | — | `Activate` on the game's window / on the largest window of a process (a frontend's own, once the game is gone). On the launcher's gamescope (see Gamescope) `focus_session` shows the game again and `focus_pid(own pid)` takes the screen back from it |
| `freeze(on)` | `freeze(on)` | — | `FreezeUnit` / `ThawUnit` on the running game's unit: every process of it stops in place, then the `freeze` / `thaw` hooks run (the capture module pauses its recorder). A stop job thaws on its own, so `stop` works on a frozen game |
| `volume(change, value)` | `volume(change, value=0)` | — | the default sink through `wpctl`: `up` / `down` by `controller.volume_step`, `mute` toggles, `set` to `value` percent, `get`; returns `{percent, muted, output}` |
| `outputs()` / `set_output(id)` | `outputs()` / `set_output(id)` | `universe output [<id>] [--json]` | the playback outputs, from one `pw-dump`: each output route of a card whose `available` is not `no` (jacks without detection say `unknown`) that the card's current profile plays or an available profile could, then each sink no route stands for (a filter, a pro-audio profile). `[{id, label, device, current}]`: `id` is `<device.name>/<route name>`, or the sink's `node.name`; `label` the route's description, `device` the card's; `current` marks the effective default sink's route (`default.audio.sink`, not the configured one, which can name a sink of a profile gone). `set_output` switches the card's profile when the route needs it (the one keeping the current input, else the highest priority), waits up to 3 s for the sink to come up, sets its route with `wpctl set-route`, then `wpctl set-default`: WirePlumber keeps it, the desktop follows. Returns the new sink's `volume("get")`; `Invalid` for an id not listed |
| `set_fps_limit()` | `set_fps_limit()` | — | rewrites the running game's `<state>/MangoHud.conf` from its `fps_limit` as launch resolves it: the layer watches that file (inotify) and rereads it by itself, a frozen game on the thaw, so no key is typed and no uinput is needed |
| `set_mangohud(on)` | `set_mangohud(on=None)` | — | the running game's HUD: `None` flips it. Written as the game's `launch.mangohud` (reread from disk first: the dock and the watcher each hold a library), then applied in the game — mangoapp told over its control queue where one draws (see MangoHud), the layer over its control socket elsewhere, each also through its conf, which it rereads — and the new state returned. `NotFound` without a session; `Unavailable`, nothing written, when nothing can draw the HUD (no mangoapp where one would, no `mangohud` for the layer) |
| `nest()` / `nest_game_shown()` / `nest_overlay(window, input, opacity)` / `nest_frame()` / `nest_filter(filter, sharpness)` | `nested()` / `nest_game_shown()` / … | — | the gamescope this process runs in (see Gamescope): whether there is one; whether it shows a window of another process; `STEAM_OVERLAY` on a window of this process, with its `STEAM_INPUT_FOCUS` and `_NET_WM_WINDOW_OPACITY`; the game's last painted frame into `<state>/frame.png` (`None` when no paint came within 5 s); `GAMESCOPE_SCALING_FILTER` and `GAMESCOPE_FSR_SHARPNESS` (no sharpness deletes the card: gamescope reads its default, 2, back). `Unavailable` on the desktop |
| `under_steam()` | `under_steam()` | — | inside Steam's gamescope (Game Mode, see Gamescope): Steam owns power, sound, screenshots and the HUD there |
| `power_actions()` / `power(action)` | `power_actions()` / `power(action)` | — | logind on the system bus. `power_actions()` lists which of `suspend`, `reboot` and `power_off` its `Can*` does not answer `no` or `na` (`inhibited` and `challenge` stay: the call says why, or polkit asks); empty when logind cannot be asked. `power(action)` calls `Suspend` / `Reboot` / `PowerOff` interactive, so a desktop's polkit agent may ask for a password; logind's refusal is `Unavailable` with its message, an unknown action `Invalid` |
| `host_gamescope(screen)` | `host_gamescope(screen)` | — | the gamescope a launcher starts itself in: `[env, MANGOHUD_CONFIGFILE=<state>/mangoapp.conf, XKB_DEFAULT_LAYOUT=…, XKB_DEFAULT_VARIANT=…, gamescope, args…]` from `launch.gamescope_bin`, the global `gamescope_*` fields at the screen's mode, `launch.gamescope_args`, `--mangoapp` whenever mangoapp is installed and `--hdr-enabled` when `launch.hdr` is, the keyboard layout (see below); writes that conf with the HUD hidden (a game shows it). `None` when the binary is not installed |
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
The CLI tables and both looks say so; the end-of-session toast names a crash or a kill. What
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
`gamescope_pid` and `launcher_pid` — whose windows are the one set that is not the game's, whichever
process asks — so `session_window` from a hook's process returns the gamescope's toplevel (what
shows the game) and `wait_session_window` waits until gamescope shows the game's window inside.

The gamescope the launcher starts for itself carries `UNIVERSE_OWN_GAMESCOPE` to its children:
`drm` when it drives the screen (a session of its own: no `WAYLAND_DISPLAY` or `DISPLAY` above it),
`nested` in a desktop's window. Any other gamescope the launcher finds itself in is someone else's.

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
path as given, then `<name>` under each Proton directory — Lutris's `runners/wine`, Steam's
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

Two programs most distributions do not package are fetched by Universe itself when they are not on
PATH: `umu-run` (umu-launcher 1.4.4's zipapp, which needs python3 3.10 or later) before a Proton
launch that uses `launch.umu_run`, and `gogdl` (heroic-gogdl 1.3.0's x86_64 build) before any command
of a source that requires it. Each is pinned to one GitHub release asset and its sha256: a download
that does not match is refused, nothing written. They land in `<data>/bin`, which every lookup of a
program searches after PATH (an installed one wins) and which is appended to the `PATH` of the
modules, the sources and the units Universe starts. A required binary Universe fetches never makes
a source unavailable, and `doctor` reports it as fetched on first use.

## Sources

| Rust | Python | CLI | Role |
|---|---|---|---|
| `sources()` | `sources()` | `universe sources`, `universe source ls` | `[{id, name, version, description, dir, enabled, available, missing: [bin], capabilities: [name], hooks: {}, settings: [Setting], logged_in, user, games_dir, library_cached, library_at}]`; `library_at` is when the store was last listed (RFC 3339, empty before the first); the login probe reaches the network once per process, on the first call |
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
| `source_install(source, game_id, progress)` | `install(source, game_id, progress)` | `universe install <id> [--source]` | id of the installed game |
| `source_update(source, game_id, progress)` | `update(source, game_id, progress)` | `universe update [name] [-y]` | how many were updated; `game_id=""` updates everything pending |
| `source_updates()` | `updates()` | `universe update` | `[{id, title, local_build, remote_build, version, date}]` |
| `source_scan(source, progress)` | `scan(source, progress)` | `universe scan [source]` | how many games entered the library; `source=""` scans all |

`progress` is called `(done, total, message)` as the job runs.

`SourceGame` = `{"id": "1434554947", "title": "Mini Metro", "owned": true, "installed": true,
"dir": "path|null", "build": "…|null", "remote_build": "…|null", "disk_size": bytes|null,
"download_size": bytes|null, "partial_dir": "path|null", "partial_bytes": bytes|null}`. `disk_size` is what
the install takes (measured) or would take (from `info`); `partial_*` name a download stopped by `cancel`
that `install` resumes.

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

## Runners

A runner is what starts a game: `proton` (through umu-run), `wine`, `linux` (the program itself),
or an emulator. Each is a spec the core ships — id, name, aliases, the binaries to look for, the
platforms it emulates, the file extensions it takes, the flags that make it start fullscreen and
quit with the game, whether a stop repeats its SIGTERM (`term_twice`, see Sessions), and typed options — with a program the core detects or the user sets.

| Rust | Python | CLI | Role |
|---|---|---|---|
| `runners()` | `runners()` | `universe runner ls` · `runner options <id>` | `[Runner]`, see below |
| `set_runner_setting(id, key, value)` | `set_runner_setting(…)` | `universe runner set <id> k=v …` | writes `config.toml [runners.<id>] <key>`: `exe`, `args`, `gamescope`, or an option, validated by type; `""` resets it |

`Runner` = `{"id": "dolphin", "name": "Dolphin", "kind": "proton|wine|linux|emulator", "aliases": ["…"],
"binaries": ["dolphin-emu"], "platforms": ["Nintendo GameCube", "Nintendo Wii"],
"extensions": ["iso", …], "exe": "the configured program or empty", "args": "extra arguments, shell-quoted",
"gamescope": true | false | null (the global default), "path": "the program that will run, empty when none was found", "source": "config|path|",
"available": true, "options": [{"key", "type": "bool|path", "default", "label", "choices": [],
"value": the global value}]}`.

A few runners spell the file their own way: `xenia` is a Windows build run through Proton, `mame`
gets `-rompath <dir> <name>`, `dosbox` takes a program or a `.conf` (`-conf`), `scummvm` the game's
folder. Ids, aliases and platforms: `universe runner ls`.

The program: `[runners.<id>] exe` if set (a path, or a name on PATH), else the spec's binaries on
PATH in order. A game may name its own with `launch.runner_exe`.
Flatpak installs are not looked for: `flatpak run` moves the app into its own scope, which the
session's `ExitType=cgroup` would take for the game ending.

The command line of an emulator: `[runners.<id>] args`, the option flags in the spec's order, the
file flag and the game file (`launch.exe`: a ROM, an image, an EBOOT.BIN, a folder), then
`launch.args`. `MANGOHUD=1` and `launch.env` apply as for Proton.

Every emulator carries the `inputplumber` option (default true): before the game unit starts, the
core restarts the InputPlumber system unit over the system bus (a refusal is logged and the
session goes on), sets its `ManageAllDevices` property and waits for the first composite device
(7-9 s on a fresh daemon), so the emulator sees one composite pad and the raw nodes are hidden;
`session-end` gives the pads back (`ManageAllDevices` off, then a wait for `/dev/inputplumber/
by-hidden` to empty). The marker remembers that it was engaged, so a `session-end` run by systemd
alone releases it. The controller watcher reads the composite device like any pad. Without the
daemon on the system bus the option is skipped and doctor says so.

## Media

| Rust | Python | CLI | Role |
|---|---|---|---|
| `media_refresh(id, force, progress)` | `media_refresh(id, force, progress)` | `universe media <name> refresh` | `(changed, total)`: fills the empty slots of `media/` from SteamGridDB, RAWG and Steam screenshots (`force` refetches the filled ones); a pick does not stop its slot's default from being fetched; `id=""` does every game |
| `media_cancel()` | `media_cancel()` | — | stops a running library-wide `media_refresh` after the game in hand; it returns normally with what it got to. A single game's refresh is not touched |
| `media_status(id)` | `media_status(id)` | `universe media <name> status` | `[{id, title, sgdb_id, sgdb_name, sgdb_year, slots: [{slot, path, default, override, origin, default_origin, kind}]}]`; `path` is what shows, `default` the fetched file under `media/`, `override` the pick under `media/picked/`; `kind ∈ picked, default, missing`; `origin` is `picked` or the provider that wrote the default (`sgdb`, `steam`; empty when nobody recorded it), `default_origin` that provider whatever sits over it; `sgdb_name` is the SteamGridDB entry the art comes from, as the last refresh or candidates call cached it (offline: empty until then); `id=""` does every game |
| `media_set_slot(id, slot, path)` | `media_set_slot(…)` | `universe media <name> set <slot> <path>` | copies the file to `games/<id>/media/picked/<slot>.<ext>` (a screenshot into `media/picked/screenshots/`), replacing any file of that slot there, and returns the path; the default under `media/` stays |
| `media_set_url(id, slot, url)` | `media_set_url(…)` | `universe media <name> set <slot> <url>` | the same from an http(s) URL, a candidate's |
| `media_unset(id, slot)` | `media_unset(id, slot)` | `universe media <name> unset <slot>` | removes the pick, so the slot shows its default again; `true` when there was one |
| `media_candidates(id, slot, page)` | `media_candidates(id, slot, page=0)` | `universe media <name> candidates <slot> [--page N]` | `{items: [{provider, id, url, thumb, score, slot}], page, more, entry: {id, name, year}}`: one page of SteamGridDB's art for the slot, best first, English and non-NSFW only, and the entry it belongs to; the entry is the pin (`metadata.sgdb_id`), else `.sync.json`'s, else a search by title — the hit named like the title, else autocomplete's first |
| `media_search(id, query)` | `media_search(id, query)` | `universe media <name> search [query…]` | `[{provider, id, name, year, verified, current}]`: SteamGridDB's games for the query (the title when empty), `current` on the one the slots come from — to find the id to pin when the match is wrong |
| `media_pin(id, provider, provider_id)` | `media_pin(…)` | `universe media <name> pin <provider> <id>` | `provider ∈ sgdb, rawg, steam` → `metadata.<provider>_id`; candidates and refresh follow it |

`slot ∈ box_front, square, banner, background, logo, screenshot`. `square` is the 1:1 grid (SteamGridDB 1024×1024, then 512×512): Reprise's home rail and the Switch 2 tiles; `banner` the 920×430 grid: the Switch 2 news card and info pane when they have no picture, and the backdrops and launch poster when the game has no background (after its screenshots). On disk a slot is read under its own stem or Pegasus's and Lutris's, own stem first: `boxFront`, `cover`; `tile`, `icon` (Pegasus's square); `steam`, `grid` (Pegasus's banner); `hero`, `fanart`.

Two layers per slot: the **default** under `games/<id>/media/`, which `refresh` fills and `.sync.json`'s `sources` attributes to its provider, and the **pick** under `games/<id>/media/picked/`, which `media_set_slot` writes and always shows first (`media_of`: `media/picked/`, then `media/`). Removing the pick falls back to the default, so a pick never loses what was fetched.

## Recordings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `file_recording(session_id, path, timeline)` | `file_recording(session_id, path)` | `universe recording-file <session> <path> [--timeline <json>]` | files the mkv as `<recordings_root>/<id>/<session>.mkv` (rename within a filesystem, copy across), writes `recording`, the probed `recording_duration_s` and the timeline's `recording_started_at` / `recording_pauses` into the session line, prints the final path. The timeline is `{"started_at": RFC3339, "pauses": [[from, to]]}`, a file path on the CLI, `None` for none |
| — | `recordings(id)` (a filter on the client) | `universe recordings <name>` | the `SessionRow`s of `sessions(id)` that have a `recording` |
| `remove_recording(id, session_id)` | `remove_recording(id, session_id)` | `universe recordings <name> --remove <session> [-y]` | trashes the mkv (`trash`), clears `recording` on the session line; the hours stay |

`recording-file` is called by the capture module's `session-end` hook, so it lands before any
`post-process` hook runs.

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
`universe@ilyasturki.github.io` shell extension, `extension/`, GNOME 45 to 50, installed by the
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
| `sweep_journals()`, `retry_journals(id)` | `sweep_journals(id="")` | `universe journal [<name>] --retry` | starts the oldest owed entry — `deferred` past its instant, or `pending` with no unit behind it — and answers `{started: {game, session} or null, due, next, held?, error?}`: `due` is what is still waiting, `next` the nearest instant a deferred entry falls due (the frontends arm their timer on it), `held` why nothing started. One entry at a time, and none while a game runs |
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
  "attempts"}`) for a failure worth another run — a quota wall, an endpoint that timed out or
  answered 5xx, a model that gave nothing usable, a recording on a filesystem that is not mounted,
  a hook killed by a signal. `until` is the wall's own reset instant when the provider gives one,
  else a backoff on `attempts` (15 min, 1 h, 4 h); a quota wall does not count as a try, and the
  third counted try writes a failed file instead ("… (gave up after 3 tries)").
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

Only an emulator runner with its `inputplumber` option off is served; a Proton, Wine or native
game keeps the physical pads (Wine's winebus reads hidraw and evdev past SDL's hints). Without
`/dev/uhid` open to the user (on NixOS `programs.universe.controller.enable`, which also opens the
virtual pads' hidraw nodes) or without SDL3, the hook logs why and the game gets the physical pads.
`$XDG_RUNTIME_DIR/universe/pads-<session>.json` holds `{ready, pid, players: [{player, name, vendor,
product, pad}]}` while the forwarder runs; it stops with the session, or by itself once the game's
unit is gone.

## Emulator controls

The `controls` module sets up an emulator's controllers for the physical pads held, before each
launch of an emulator runner whose `inputplumber` option is off (nothing when no pad is held, or
when the `pads` module serves the game: its virtual pads hide the physical ones). Its `pre-launch`
hook lists the gamepads through SDL3 under the joystick hints the emulator's own SDL sets (they pick
the driver that claims a pad, and with it the GUID and indices the emulator sees; the game's
`launch.env` `SDL_*` go over them), then does two things.

Every emulator: the game's environment gets `SDL_GAMECONTROLLERCONFIG_FILE`, pointing at
`$XDG_RUNTIME_DIR/universe/controls-<session>.txt` (removed at `session-end`): each held pad's
SDL mapping without `guide` (unless `guide` is on), `misc1`…`misc6` and `paddle1`…`paddle4`, after
the controller databases an emulator would itself give that hint, so a line for the same GUID
ends up ours. An emulator reading pads through SDL's gamepad API never sees
HOME, capture or the paddles — its hard-wired uses of them included (Vita3K's pause, xemu's and
Flycast's menus, ScummVM engines' guide keys); the touchpad stays, PS4 and Vita games use it.
Raw-index readers (Eden, Azahar, melonDS, mGBA, snes9x, mupen64plus) ignore the mapping, and
their writers never bind those buttons. Xenia runs through Proton, whose own SDL reads the pads:
nothing is done for it.

The emulators with pad identities or player slots in their config get them written, the way the
emulator's own auto-mapping would, player `n` being the `n`th pad in SDL's order:

| Runner | File | Written |
|---|---|---|
| `eden` (and citron, sudachi, suyu, yuzu dirs) | `qt-config.ini [Controls]` | `player_0-7_*`, each with `\default=false`; players past the pads disconnected |
| `ryujinx` | `Config.json`, `games/*/Config.json` | `input_config` + `player_input_assignments`; a game's own config too unless `use_input_global_config` |
| `dolphin` | `GCPadNew.ini`, `WiimoteNew.ini`, `Dolphin.ini` | `[GCPadN]`/`[WiimoteN]` on `SDL/<n>/<name>` (calibration and options kept), `SIDeviceN`/`Source` 0→on for held pads |
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

`layout` picks the face buttons of a Nintendo diamond (Switch, 3DS, Wii U, DS, GBA, SNES, the
Wii's Classic Controller): `positional` (A on the right, as on a Switch and as these emulators map
by themselves) or `xbox` (A at the bottom), per game over a global default; GameCube keeps
Dolphin's own preset and N64 mupen64plus's (A bottom, B left). A game's own input profile keeps its
scheme and only follows the held pad (Dolphin `PadProfileN`/`WiimoteProfileN`, Cemu
`gameProfiles` `controllerN`, RPCS3's other and per-title configs). Everything else in a file is
left as it is; a file is written only when it changes, through a symlink to its target, and the
first write copies it to `<file>.before-universe`. A file the emulator makes on its first start is
never created, the hook logging "start X once" instead; mupen64plus, snes9x and ScummVM files are
written whole, missing or not.

## Modules

| Rust | Python | CLI | Role |
|---|---|---|---|
| `modules()` | `modules()` | `universe module ls` | see below |
| `enable_module(id, enabled)` | `enable_module(id, enabled)` | `universe module enable\|disable <id>` | writes `[modules] enabled` in `config.toml` |
| `module_settings(module, game_id)` | `module_settings(…)` | `universe module settings <id> [game]` | global settings merged with the game's; `game_id=""` is global only |
| `set_module_setting(module, game_id, key, value)` | `set_module_setting(…)` | `universe module set <id> k=v [--game g]` | validated against `[[settings]]`. `game_id=""` writes `config.toml [modules.<id>]`, otherwise `game.toml [modules.<id>]` |
| `module_setting_choices(module, key)` | `module_setting_choices(…)` | — | the global setting's choices; a setting with `choices_exec` gets them from the module, live (see below) |
| `doctor()` | `doctor()` | `universe doctor` | `[{check, label, ok, detail, fix, module}]` (`check` a stable id, `label` its plain name, `detail` the problem when not `ok`, `fix` what to do about it, empty when `ok`, worded for the distribution: NixOS options on NixOS, the Arch, Fedora or Debian package names elsewhere): the config file (absent: defaults; read-only), the systemd user manager (250 or later, for `ExitType=cgroup`) and cgroup v2, umu-run (or python3 for the one Universe would fetch), MangoHud and its 32-bit layer, gamescope and mangoapp, required binaries of the enabled modules and sources (`module` names the one, or `core`, `runners`, `media`, `controller`; a tool Universe fetches is fine missing), one line per `required` setting an enabled module is still waiting on, `gsr-kms-server`, Proton, the desktop (`desktop`, then per profile the programs it drives, `desktop-<program>`, a notification daemon where the OSD is a notification, `cursor` where the profile cannot hide it), the cursor and Universe extensions on GNOME, tokens, one `runner-<id>` check per runner a library game uses (its program resolved), `runner-eden-stop` when Eden is one (its `[UI] confirmStop` at `2`, else a stop shows its "close?" question), `inputplumber` when an emulator wants it; `modules` and `sources` say what `config.toml` enables that is not found |
| — | — | `universe setup` | after an install: on GNOME, writes the `universe@ilyasturki.github.io` extension the binary carries into `~/.local/share/gnome-shell/extensions/` (rewritten when stale; left to a system copy when there is none there) and adds it to `org.gnome.shell enabled-extensions` (out of `disabled-extensions`, which overrides it), read by the shell at the next login; then prints `doctor` |

A module entry is `{id, name, version, description, dir, enabled, available, missing: [bin],
unset: [key], hooks: {}, settings: [Setting]}`, and
`Setting` = `{"key", "type": "bool|string|int|enum|path", "default", "label",
"scope": "global|game", "choices": [], "dynamic": bool, "required": bool}`.
`missing` holds the manifest's absent binaries plus those the chosen value of a `requires_bins`
setting asks for, so a module is available or not by what it is set to. `unset` lists the
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
included, is `none`. Inside the launcher's gamescope none of it is asked: gamescope focuses, draws the
OSD on the launcher's overlay and takes the screenshots itself.

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
| `settings()` | `settings()` | `universe config get` | resolved `config.toml`: absolute paths, defaults applied, `config_file` and `data_home`, `config_writable` (false when the file exists and cannot be written, as a home-manager install with `settings` set — every write is refused with a message naming it), `os` (`nixos`, `arch`, `fedora`, `debian` or `other`: the family fixes are worded for), `protons` (see Proton and Wine), and `set`: the file's own keys as written, so a frontend can tell a chosen value from a default |
| `set_setting(key, value)` | `set_setting(key, value)` | `universe config set <key> <value>` | dotted `config.toml` key (`launch.proton`, `paths.recordings_root`, `desktop.profile`); a `launch.*` key is validated against the catalogue, an unknown or game-only one refused |
| `launch_keys(scope, screen)` | `launch_keys(scope, screen)` | `universe launch-keys [--json]` | the launch keys of `scope` (`game`, `global`, `both`) that have a settings row: `[{key, type, default, choices, label, section, scope, runners, description, advanced}]`, `type` one of bool, toggle (`auto`, `on`, `off`), int, string, path, list, enum, resolution, refresh, fps, proton, map; `screen` (a `screen_mode`, or none) sizes the resolution, refresh and fps choices; `runners` empty means every runner; `advanced` puts the row behind the settings pages' Advanced toggle (every card but Display, Overlay and the Proton basics; a page folds such a card into a basic one). `runner`, `runner_exe`, `exe` and the `options` map are settable but not listed — the frontends build their rows themselves; the CLI's table prints all of them |
| `gpu()` | `gpu()` | — | the GPU the games run on: `{vendor (amd, nvidia, intel), name (the vendor's), rdna (1…4 or null), label (`AMD · RDNA 3`), fits: {dlss_upgrade, fsr4_upgrade, xess_upgrade, optiscaler}, auto: {the same keys}}`, `fits` whether each upscaler upgrade does anything on it, `auto` what the key's `auto` comes to on it; `null` when sysfs shows no card of a known vendor. Vendor and AMD generation come from `/sys/class/drm` (amdgpu's `ip_discovery` GC major: 10 RDNA 1/2, 11 RDNA 3, 12 RDNA 4); the card with the most VRAM wins (an NVIDIA card, which reports none, beats an iGPU). Probed once per process |
| `screen_mode(screen)` | `screen_mode(screen)` | `universe screen-mode [<screen>] [--json]` | `{screen, width, height, refresh, vrr}`: the connector's current mode as gamescope is told it (see Gamescope) and whether it takes a variable refresh rate, `screen=""` for the profile default; zeros when none can be read |
| — | `version()`, `data_home()`, `state_home()` | `universe --version` | `version()` is the build: the semver with the short git rev behind it (`0.0.2 (410391a)`, `-dirty` when the tree was; the flake passes its rev, a checkout asks git) |

## Controller

| Rust | Python | CLI | Role |
|---|---|---|---|
| `controller_state()` | `controller_state()` | `universe controller ls` | `{enabled, hold_ms, volume_step (a percent, number), families: [{id, name, slots: [{id, label, codes, extra}]}], macros: [Macro], presets: [{id, label, hold_only}]}`; the CLI adds `devices`, the pads readable now with every slot's code or `bound: false` |
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
| `set_system(id, value)` | `set_system(id, value)` | — | applies it and, but for `brightness`, keeps it in `[system]`. The backlight goes through logind's `Session.SetBrightness` (no polkit), else the file; the power cap and the clock through the file when it is writable, else SteamOS's `steamos-priv-write` (which leaves the file writable after its first write); a pinned clock writes `manual`, then `s 0 N`, `s 1 N` and `c`; the refresh through gamescope's `GAMESCOPE_DYNAMIC_REFRESH`; the fan through `jupiter-fan-control --enable\|--disable`. `Invalid` on an unknown id or value, `Unavailable` where the machine has no such control or the helper refuses |
| `apply_system()` | `apply_system()` | — | `[system]` put back, a failure logged: the power limit and the clocks do not outlive a reboot. The UI calls it at startup |

## config.toml

Defaults as the core ships them:

```toml
schema = 1

[paths]                              # defaults follow XDG and xdg-user-dirs
games_root = "~/Games"               # $XDG_GAMES_DIR: where sources install
prefixes_root = "~/.local/share/universe/prefixes"
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

[system]                             # the machine's controls as last set (see System), put back at the launcher's start outside Steam; "" leaves one as the system has it
tdp = ""                             # watts
gpu = ""                             # auto, or MHz
refresh = ""                         # Hz, a Deck's panel on the launcher's own gamescope
fan = ""                             # on | off: SteamOS's fan curve

[proton]                             # name → path, none by default; a name with no path here is looked for as a family (GE-Proton10-4 for proton-ge) under Lutris, Steam, umu and Heroic
# proton-em = "~/opt/proton-em"

# [runners.<id>]                     # per runner (`universe runner set`): exe (absent: detected), args, gamescope, its options

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
sgdb = ""                            # or sgdb_file, pointing at a file holding the key
rawg = ""
sgdb_file = "~/.config/steamgriddb/api_key"
rawg_file = "~/.config/rawg/api_key"

[controller]
enabled = true
hold_ms = 600                        # a press this long is a hold
volume_step = 2                      # percent of the normal volume per press, 1–100 ("precise" = 2, "normal" = 6 still read)
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
version = "0.0.6"
description = "Records each session."   # optional, one or two sentences; the module's page shows it under the name

[requires]
bins = ["gpu-screen-recorder"]    # a missing binary makes the module "unavailable" and it is never run

[hooks]                           # paths relative to the module directory
pre-launch   = "bin/pre"          # blocking, before the game's unit; may write UNIVERSE_ENV_FILE
post-launch  = "bin/start"        # async (transient unit); anything that must last the session goes in its own unit with BindsTo=$SESSION_UNIT
freeze       = "bin/freeze"       # blocking, right after the game's unit froze (HOME over the game); thaw = right after it thawed
thaw         = "bin/thaw"
session-end  = "bin/stop"         # short and blocking, inside the game unit's ExecStopPost; this is where a recording is filed
post-process = "bin/process"      # async (transient unit), after the session-end hooks
screenshot   = "bin/shot"         # on demand
timeout_s    = 20                 # for blocking hooks; the sum bounds the game unit's TimeoutStopSec

[limits]                          # applied to the transient units of async hooks
cpu_weight   = 100                # default 20
memory_high  = "4G"               # default 2G

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
label = "Frame rate"

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
| `UNIVERSE_ENV_FILE` | write `KEY=VALUE` lines here to add them to the game's environment, ahead of `launch.env`; the one key `UNIVERSE_GAMESCOPE_ARGS` is flags for the game's gamescope instead (see Gamescope) | `pre-launch` |
| `MODULE_DIR`, `MODULE_DATA_DIR` | the module's directory, `$XDG_DATA_HOME/universe/modules/<id>` | all |
| `UNIVERSE_BIN`, `UNIVERSE_{DATA,CONFIG,STATE}_HOME`, `UNIVERSE_{MODULES,SOURCES}_PATH`, `PATH` | the CLI to call back (`recording-file`, `journal-add`, `session-window`, `screen-mode`) and the environment that makes it open the same core | all |
| `UNIVERSE_GAME_JSON` | the resolved `Game`, serialized | all |

Exit codes: 0 is success; anything else is logged and the session continues — except a `pre-launch`
hook, where a non-zero exit cancels the launch.

## Source protocol

A source installs and updates games from a store. It is a directory `sources/<id>/` — system-wide
under `$out/share/universe/sources/<id>` (`UNIVERSE_SOURCES_PATH` adds roots), per-user under
`$XDG_CONFIG_HOME/universe/sources/<id>`, a user source overriding a system one with the same id —
holding a `source.toml` and its executable; its data lives in `$XDG_DATA_HOME/universe/sources/<id>`.

```toml
api = 2
id = "gog"
name = "GOG"
version = "0.0.6"
description = "Installs GOG games."   # optional, as a module's
exe = "bin/source"                # run as: bin/source <verb> [args]

capabilities = ["achievements"]   # optional verbs it answers, past the required ones

[requires]
bins = ["gogdl"]                  # a missing binary makes the source "unavailable" and it is never run, unless Universe fetches it (see Fetched tools)

[hooks]                           # optional, as a module's (every session hook but `screenshot`); they run for the games whose source is this one
pre-launch  = "bin/pre-launch"
session-end = "bin/session-end"
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
game's own keys), `SOURCE_DIR`, `SOURCE_DATA_DIR` and `SOURCE_GAME_ID` (the store's id of the game)
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

```json
{"event":"game","id":"1434554947","title":"Mini Metro","dir":"/mnt/games/PC/Mini Metro",
 "exe":"MiniMetro.exe","build":"5904…","owned":true,"installed":true,
 "release_year":2015,"dlcs":[],"disk_size":167772160}
{"event":"game","id":"1207658930","title":"Alan Wake","owned":true,"installed":false,
 "partial_dir":"/mnt/games/PC/Alan Wake","partial_bytes":3400000000,"download_size":7900000000,"disk_size":8200000000}
{"event":"progress","done":123,"total":456,"message":"27.0%"}
{"event":"done"}
```

`exe` is relative to `dir`. `owned` may be `null` when the source cannot tell. The core writes `library.json` (the last `library` run's games) in the source's data dir after every listing; a source reads it for ownership and writes nothing there itself.

---

Writing a frontend on top of this API: [`frontends.md`](frontends.md).
