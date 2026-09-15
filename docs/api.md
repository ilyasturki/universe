# Universe — core API and module protocol

`api = 1`. The core is a Rust library (`crates/universe`, `universe::core::Core`). **No Universe
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
  order (the cursor extension, InputPlumber, `post_command` for `pre_command`) as the marker lists
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
  is watching.
- Long jobs (`install`, `update`, `scan`, media refresh) run **in the calling process** with a
  progress callback. Closing the frontend interrupts them.
- The one exception to "nothing in the background" is the **controller watcher**
  (`universe controller watch`), which lives exactly as long as the launcher or a session does: the
  UI starts one for its lifetime, `launch` starts one bound to the game's unit
  (`universe-controller-<session>`, `BindsTo=` the game), and a lock (`$XDG_RUNTIME_DIR/universe/
  controller.lock`) hands the pads over between them. Once both are gone nothing runs.
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
| `remove(id, purge)` | `remove(id, purge)` | `universe rm <name> [--purge]` | parks recordings and journal under `.archive/`, marks `removed_at`; `purge` also trashes the prefix |
| `uninstall(id)` | `uninstall(id)` | `universe uninstall <name>` | trashes `source.dir` and clears `source.dir`, `source.build_id` and `launch.exe`; the game stays in the library, not installed. Refuses a root, a home or the games root |
| `reload_all()` | `reload()` | `universe rescan` | rereads config and `games/*/game.toml`, runs each source's `scan` |
| `reload_game(id)` | `reload_game(id)` | — | rereads one game |
| `import_lutris(apply)` | `import_lutris(apply)` | `universe migrate [--apply]` | a report: imported games, per-game env diff (`{id, lutris_env, universe_env, added, removed, changed}`), imported hours, games whose art was copied from `[lutris] pegasus_library` (`<platform>/media/<slug>/`, once, never over an existing `media/`), `runners_promoted` (emulator games from before runners that now name theirs), `options_promoted` (games already imported that take what a field now holds — a `wrapper`, a DLL override, a Proton switch — only where the file had nothing) and `runners` (what Lutris's runner configs say: a wrapper script is seen through, the program is written to `[runners.<id>] exe` when it is not on PATH, its extra arguments to `args`). Without `apply` it only reports |
| `add_game(spec)` | `add_game(spec)` | `universe add <file> --runner <id> [--title T] [--platform P] [--media]` | `{"runner", "exe", "title"?, "platform"?}` → the new id. The title defaults to the file's name cleaned of release tags; the platform to the runner's first. Refuses an id already in the library |

`set` takes dotted keys: the `launch.*` keys of `universe launch-keys` — the one catalogue
(`launch_keys.rs`) every `[launch]` key is declared in, with its type, default, scope and choices;
a game key left empty takes `[launch]`'s, a value is validated as the key's type says, an unknown
or global-only key is refused — with the maps `launch.dll_overrides.d3d11`, `launch.env.FOO` and
`launch.options.<key>` (validated against the runner's options); then `desktop.hide_cursor`, `hidden`,
`favorite`, `tags`, `sort_title`, `platform`, `metadata.sgdb_id`, and `capture.cursor` as a
validated shorthand for `modules.capture.cursor`. Values are strings: `true`/`false` for booleans,
comma-separated for lists, `""` deletes the key. A runner is written under its shipped id (`yuzu` →
`eden`), and writing one retires the pre-runner `launch.backend` key.

`Game` (JSON) is the contents of `game.toml` plus:

```json
{"stats": {"hours": 12.5, "play_count": 7, "last_played": "RFC3339 or null"},
 "media": {"box_front": "path|null", "square": null, "banner": null, "background": null, "logo": null,
           "screenshots": ["path"]},
 "modules": {"capture": {"enabled": true, "cursor": false}},
 "effective": {"runner": "dolphin", "runner_name": "Dolphin", "runner_kind": "emulator",
               "runner_path": "/…/bin/dolphin-emu", "platform": "Nintendo GameCube",
               "options": {"batch": true, "user_directory": "", "inputplumber": true}, "inputplumber": true,
               "proton": "proton-ge", "proton_path": "…", "esync": true, "fsync": true, "ntsync": true,
               "wayland": true, "hdr": false, "dlss_upgrade": false, "fsr4_upgrade": false, "xess_upgrade": false,
               "optiscaler": false, "mangohud": true, "gamescope": true, "gamescope_args": "", "gamescope_resolution": "auto", "gamescope_refresh": "auto", "gamescope_scaler": "", "gamescope_filter": "", "gamescope_sharpness": null, "gamescope_adaptive_sync": false, "fps_limit": "auto", "hide_cursor": true, "env": {}},
 "removed": false}
```

`effective` is what the launch will use: the game's own keys over the global defaults, the runner
resolved (`runner_path` empty when its program was not found), the platform the runner implies when
the game sets none.

There is no change notification: the files are the truth, so a frontend watches `games/`,
`games/<id>/{,journal,media}`, `state/` and the overrides directory and rereads. Everything the CLI, `session-end` and the
hooks write shows up that way, with no other channel.

## Sessions

| Rust | Python | CLI | Role |
|---|---|---|---|
| `launch(id, screen, splash)` | `launch(id, screen, splash="")` | `universe play <name> [--screen DP-1] [--no-wait]` | pre-launch hooks, marker, `systemd-run`, post-launch hooks; returns the `session_id` at once. `screen` is a DRM connector name or `""` for the profile default; `splash` a poster for gamescope's keep-alive window (see Gamescope) or `""`. `Busy` if a session is already running |
| `stop(session_id)` | `stop(session_id)` | `universe stop` | `systemctl --user stop` on the unit; waits for it, a second SIGTERM after ~3 s |
| `session_window()` | `session_window()` | — | the running game's window as the Universe shell extension lists it (`{id, pid, wm_class, title, focused, width, height, hidden, minimized}`): the largest visible toplevel whose pid is in the unit's cgroup — gamescope's when the game runs inside it. `None` before it maps; `Unavailable` off GNOME |
| `wait_session_window(session_id, timeout)` | `wait_session_window(session_id, timeout_ms)` | — | blocks until that window is up, then `Activate`s it (focus and raise) and returns it; `None` when the session ended first or the timeout ran out; `Unavailable` off GNOME, at once. Polls the extension every 150 ms |
| `focus_session()` / `focus_pid(pid)` | `focus_session()` / `focus_pid(pid)` | — | `Activate` on the game's window / on the largest window of a process (a frontend's own, once the game is gone) |
| `adopt_scope()` | `adopt_scope()` | — (`universe play` does it unless `--no-wait`) | moves the calling process into the transient scope `universe-launcher-<pid>.scope` (`StartTransientUnit` on the user manager) and returns its name; every later `launch` binds the game to it. Idempotent. `Unavailable` without a user systemd |
| `screenshot()` | `screenshot()` | `universe screenshot` | runs the `screenshot` hook of whichever module declares one; returns the PNG path |
| `current()` | `current()` | `universe status` | `{session_id, id, title, unit, screen, started_at}`, or `None`. The CLI wraps it: `status --json` prints `{"current": … or null, "recent": [the last 10 sessions], "pending_journals": [see Journal]}` |
| `sessions(id)` | `sessions(id)` | `universe sessions <name>` | `[Session]` from `sessions.jsonl`, last first |
| `session_end(id, session_id, exit, ended)` | — | `universe session-end <id> <session>` | closes the session, idempotent. Run by systemd's `ExecStopPost`, or by reconciliation |

One `sessions.jsonl` line:

```json
{"session":"20260910-213045","game":"the-technomancer","started_at":"RFC3339",
 "ended_at":"RFC3339","duration_s":1234,"source":"universe",
 "unit":"universe-game-the-technomancer-20260910-213045.service","screen":"DP-1",
 "exit":0,"recording":"path or null"}
```

`source ∈ universe, import-recording, import-lutris`. `exit` is the main process's exit code, `-1`
when it was killed by a signal (a `stop`).

### Gamescope

Every runner's command runs inside gamescope by default: `gamescope -f --force-composition
-W <screen width> -H <screen height> -w <game width> -h <game height> -r <refresh> [-S scaler] [-F filter]
[--sharpness N] [--adaptive-sync] [launch.gamescope_args] [the game's
gamescope_args] [--mangoapp] -- universe splash [--image <poster>] -- <program> <args…>`. One
window, from gamescope's first frame to the game's last, whatever the game, Proton or umu put up
first, and the launcher hands over on it. `--force-composition` keeps gamescope drawing its own
frame instead of scanning the game's buffer out directly: Mutter's window screencast (`capture`'s
window source) blits a scanned-out buffer as one flat colour.

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
physical pixels — gamescope handles the desktop's scale itself), else its preferred DRM mode
(`/sys/class/drm/*/modes`) at 60 Hz; no screen at all leaves gamescope's own defaults.
`universe doctor` prints the mode it read. The `gamescope_*` fields, global in `[launch]` and per
game in `game.toml`'s `[launch]` (a field left empty takes the global one), are launch keys:
`universe launch-keys` prints each one's values and default. Each is a flag: `gamescope_resolution`
(what the game renders at, upscaled to the screen when smaller) `-w -h`, `gamescope_refresh` `-r`,
`gamescope_scaler` `-S`, `gamescope_filter` `-F`, `gamescope_sharpness` (for `fsr` and `nis`)
`--sharpness`, `gamescope_adaptive_sync` `--adaptive-sync`; a scaler or filter left empty leaves
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
MangoHud is `--mangoapp` rather than `MANGOHUD=1`, `launch.hdr` adds `--hdr-enabled`, and
`PROTON_ENABLE_WAYLAND` is dropped (Proton goes X11 through gamescope's Xwayland) unless the
arguments carry `--expose-wayland`. `doctor` checks the binary, and `mangoapp` when MangoHud is on.

### Frame rate limit

`fps_limit` — `[launch]`'s, the game's own when set — is `auto`, `none`, or frames per second,
and holds the game to it through MangoHud's limiter inside the game process, overlay or not:
gamescope's `--framerate-limit` paces nothing on a nested gamescope (measured: an uncapped
client stays uncapped, a vsynced one at the refresh, with or without its WSI layer), and its `-r`
only paces clients that vsync. `auto` is the refresh the game sees: its `gamescope_refresh` when
set, else the screen's; unknown (no screen read) means no limit. The launcher writes
`<state>/MangoHud.conf` before each launch — `~/.config/MangoHud/MangoHud.conf`'s lines, so the
layout and `fps_limit_method` hold, with `fps_limit` swapped for ours and `no_display` added unless
the HUD is the game's own (desktop, MangoHud on) — and gives the game `MANGOHUD=1
MANGOHUD_CONFIGFILE=<that>` through `env` in front of the program: after `setpriv`, never on the
unit, where gamescope (a Vulkan client itself) and mangoapp would read it. Inside gamescope the
layer limits and draws nothing while mangoapp shows the HUD. A native or emulator program (not one
run through Proton) goes through the `mangohud` wrapper so an OpenGL game is limited too; Proton
and Wine get the Vulkan layer alone, nothing preloaded into the runtime. No `mangohud` on PATH: a
warning, no limit; `doctor` checks for it unless the limit is `none`.

### Proton and Wine

`proton` runs `umu-run <exe>` with `WINEPREFIX` (`launch.prefix`, else `<prefixes_root>/<id>`),
`PROTONPATH`, `GAMEID` (`launch.umu_id`, else `umu-default`), `STORE`, then the switches as the env
Proton reads: `esync`/`fsync`/`ntsync` off are `PROTON_NO_ESYNC`/`_FSYNC`/`_NTSYNC=1`; `wayland`,
`hdr`, `dlss_upgrade`, `fsr4_upgrade`, `xess_upgrade` and `optiscaler` on are
`PROTON_ENABLE_WAYLAND`, `PROTON_ENABLE_HDR`, `PROTON_DLSS_UPGRADE`, `PROTON_FSR4_UPGRADE`,
`PROTON_XESS_UPGRADE`, `PROTON_USE_OPTISCALER=1`. A switch off removes the same name from
`[launch.env]`; `launch.env` on the game still wins. `launch.dll_overrides` (`d3d11 = "n,b"`, keys
without `.dll`) is `WINEDLLOVERRIDES`. `wine` runs `<launch.runner_exe or wine> <exe>` with
`WINEPREFIX`, `WINEARCH` (`launch.arch`), `WINEESYNC`/`WINEFSYNC` as `1`/`0` and
`WINEDLLOVERRIDES`; the Proton switches do not apply. Both, like every runner, take
`launch.wrapper` — `gamemoderun`, `taskset -c 0-7` — split like a shell line and put in front of
the program, inside gamescope and setpriv.

`migrate` maps Lutris's wine runner onto these: `wine.version` is `wine` with `launch.runner_exe`
when `<runners_dir>/<version>/bin/wine` exists with no `proton` script beside it, `wine` for
`system`, `proton` otherwise; `wine.proton_hdr` is `hdr`;
`system.prefix_command`'s leading `VAR=val` words become `launch.env` (`WINEDLLOVERRIDES` its
`dll_overrides`) and the rest `launch.wrapper`; a `PROTON_*` entry in `system.env` that has a switch
becomes the switch. Lutris's `fps_limit` is `launch.fps_limit`; its `fsr`, `battleye`, `eac`,
DXVK/VKD3D versions and registry options have no counterpart (Proton bundles its own DXVK, and reads
none of those variables).

## Sources

| Rust | Python | CLI | Role |
|---|---|---|---|
| `sources()` | `sources()` | `universe sources` | `[{id, name, available, enabled, missing, logged_in, user, games_dir, library_cached}]` |
| `source_login_url(source)` | `login_url(source)` | `universe login <source>` | URL to open |
| `source_login(source, code)` | `login(source, code)` | `universe login <source> <code>` | returns the user name |
| `source_library(source, refresh)` | `library(source, refresh)` | `universe library [source] [--refresh]` | `[SourceGame]`, served from cache unless `refresh` |
| `source_search(source, query)` | `search(source, query)` | `universe search <query> [--source]` | `[SourceGame]` |
| `source_info(source, game_id)` | `info(source, game_id)` | — | the source's raw `info` payload |
| `source_install(source, game_id, progress)` | `install(source, game_id, progress)` | `universe install <id> [--source]` | id of the installed game |
| `source_update(source, game_id, progress)` | `update(source, game_id, progress)` | `universe update [name] [-y]` | how many were updated; `game_id=""` updates everything pending |
| `source_updates()` | `updates()` | `universe update` | `[{id, title, local_build, remote_build, version, date}]` |
| `source_scan(source, progress)` | `scan(source, progress)` | `universe scan [source]` | how many games entered the library; `source=""` scans all |

`progress` is called `(done, total, message)` as the job runs.

`SourceGame` = `{"id": "1434554947", "title": "Mini Metro", "owned": true, "installed": true,
"dir": "path|null", "build": "…|null", "remote_build": "…|null"}`.

## Runners

A runner is what starts a game: `proton` (through umu-run), `wine`, `linux` (the program itself),
or an emulator. Each is a spec the core ships — id, name, aliases, the binaries to look for, the
platforms it emulates, the file extensions it takes, the flags that make it start fullscreen and
quit with the game, and typed options — with a program the core detects or the user sets.

| Rust | Python | CLI | Role |
|---|---|---|---|
| `runners()` | `runners()` | `universe runner ls` · `runner options <id>` | `[Runner]`, see below |
| `set_runner_setting(id, key, value)` | `set_runner_setting(…)` | `universe runner set <id> k=v …` | writes `config.toml [runners.<id>] <key>`: `exe`, `args`, `gamescope`, or an option, validated by type; `""` resets it |

`Runner` = `{"id": "dolphin", "name": "Dolphin", "kind": "proton|wine|linux|emulator", "aliases": ["…"],
"lutris": "dolphin", "binaries": ["dolphin-emu"], "platforms": ["Nintendo GameCube", "Nintendo Wii"],
"extensions": ["iso", …], "exe": "the configured program or empty", "args": "extra arguments, shell-quoted",
"gamescope": true | false | null (the global default), "path": "the program that will run, empty when none was found", "source": "config|path|lutris|",
"available": true, "options": [{"key", "type": "bool|path", "default", "label", "choices": [],
"value": the global value}]}`.

A few runners spell the file their own way: `xenia` is a Windows build run through Proton, `mame`
gets `-rompath <dir> <name>`, `dosbox` takes a program or a `.conf` (`-conf`), `scummvm` the game's
folder. Ids, aliases and platforms: `universe runner ls`.

The program: `[runners.<id>] exe` if set (a path, or a name on PATH), else the spec's binaries on
PATH in order, else an executable of that name or an AppImage under
`~/.local/share/lutris/runners/<lutris id>/`. A game may name its own with `launch.runner_exe`.
Flatpak installs are not looked for: `flatpak run` moves the app into its own scope, which the
session's `ExitType=cgroup` would take for the game ending.

The command line of an emulator: `[runners.<id>] args`, the option flags in the spec's order, the
file flag and the game file (`launch.exe`: a ROM, an image, an EBOOT.BIN, a folder), then
`launch.args`. `MANGOHUD=1` and `launch.env` apply as for Proton.

Every emulator carries the `inputplumber` option (default true): before the game unit starts, the
core restarts the InputPlumber system unit, enables `manage-all` and waits for the first composite
device (7-9 s on a fresh daemon), so the emulator sees one composite pad and the raw nodes are
hidden; `session-end` gives the pads back (`manage-all` off, then a wait for `/dev/inputplumber/
by-hidden` to empty). The marker remembers that it was engaged, so a `session-end` run by systemd
alone releases it. The controller watcher reads the composite device like any pad. Without
`inputplumber` on PATH the option is skipped and doctor says so.

## Media

| Rust | Python | CLI | Role |
|---|---|---|---|
| `media_refresh(id, force, progress)` | `media_refresh(id, force, progress)` | `universe media <name> refresh` | `(changed, total)`: fills the empty slots of `media/` from SteamGridDB, RAWG and Steam screenshots (`force` refetches the filled ones); an override does not stop its slot's default from being fetched; `id=""` does every game |
| `media_status(id)` | `media_status(id)` | `universe media <name> status` | `[{id, title, sgdb_id, sgdb_name, sgdb_year, slots: [{slot, path, default, override, origin, default_origin, kind}]}]`; `path` is what shows, `default` the fetched file under `media/`, `override` the pick under the overrides directory; `kind ∈ picked, default, missing`; `origin` is `picked` or the provider that wrote the default (`sgdb`, `steam`, `pegasus`; empty when nobody recorded it), `default_origin` that provider whatever sits over it; `sgdb_name` is the SteamGridDB entry the art comes from, as the last refresh or candidates call cached it (offline: empty until then); `id=""` does every game |
| `media_set_slot(id, slot, path)` | `media_set_slot(…)` | `universe media <name> set <slot> <path>` | copies the file to `<overrides>/<id>/<slot>.<ext>` (a screenshot into `<overrides>/<id>/screenshots/`), replacing any file of that slot there, and returns the path; the default under `media/` stays |
| `media_set_url(id, slot, url)` | `media_set_url(…)` | `universe media <name> set <slot> <url>` | the same from an http(s) URL, a candidate's |
| `media_unset(id, slot)` | `media_unset(id, slot)` | `universe media <name> unset <slot>` | removes the override, so the slot shows its default again; `true` when there was one |
| `media_candidates(id, slot, page)` | `media_candidates(id, slot, page=0)` | `universe media <name> candidates <slot> [--page N]` | `{items: [{provider, id, url, thumb, score, slot}], page, more, entry: {id, name, year}}`: one page of SteamGridDB's art for the slot, best first, English and non-NSFW only, and the entry it belongs to; the entry is the pin (`metadata.sgdb_id`, else pegasus-sync's `<overrides>/<id>/sgdb_id` file), else `.sync.json`'s, else a search by title — the hit named like the title, else autocomplete's first |
| `media_search(id, query)` | `media_search(id, query)` | `universe media <name> search [query…]` | `[{provider, id, name, year, verified, current}]`: SteamGridDB's games for the query (the title when empty), `current` on the one the slots come from — to find the id to pin when the match is wrong |
| `media_pin(id, provider, provider_id)` | `media_pin(…)` | `universe media <name> pin <provider> <id>` | `provider ∈ sgdb, rawg, steam` → `metadata.<provider>_id`; candidates and refresh follow it |

`slot ∈ box_front, square, banner, background, logo, screenshot`. `square` is the 1:1 grid (SteamGridDB 1024×1024, then 512×512): Reprise's home rail and the Switch 2 tiles; `banner` the 920×430 grid, shown nowhere yet. On disk a slot is read under its own stem or Pegasus's and Lutris's, own stem first: `boxFront`, `cover`; `tile`, `icon` (Pegasus's square); `steam`, `grid` (Pegasus's banner); `hero`, `fanart`.

Two layers per slot: the **default** under `games/<id>/media/`, which `refresh` fills and `.sync.json`'s `sources` attributes to its provider, and the **override** under `<paths.overrides>/<id>/`, which a pick writes and always shows first (`media_of`: overrides by id, then by the Lutris slug, then `media/`; a slot is read under its own stem or Pegasus's and Lutris's — `boxFront`, `cover`, `banner`, `hero`…). Removing the override falls back to the default, so a pick never loses what was fetched.

## Recordings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `file_recording(session_id, path)` | `file_recording(session_id, path)` | `universe recording-file <session> <path>` | files the mkv as `<recordings_root>/<id>/<session>.mkv` (rename within a filesystem, copy across), writes `recording` into the session line, prints the final path |
| `recordings(id)` | `recordings(id)` | `universe recordings <name>` | `[{session, path, size, duration_s, created_at}]` |
| `remove_recording(id, session_id)` | `remove_recording(id, session_id)` | `universe recordings <name> --remove <session> [-y]` | trashes the mkv (`trash`), clears `recording` on the session line; the hours stay |

`recording-file` is called by the capture module's `session-end` hook, so it lands before any
`post-process` hook runs.

The capture module records the whole **screen** (`source = "screen"`, the default: gpu-screen-recorder's
KMS capture of the session's output) or the game's **window** (`source = "window"`, per game). The
window source needs GNOME and the `universe@ilyasturki.github.io` shell extension (shipped by the
home-manager module, loaded after one logout): the module enables it, waits through it for the game's
toplevel to be up (`window_wait_s`, gamescope's window stays hidden until the game draws), then runs
gpu-screen-recorder on GNOME's screencast portal. The first launch of a game shows GNOME's picker —
pick the game's window, which is on screen by then — and the portal's restore token is kept in
`<data>/modules/capture/portal/<game id>`; GNOME restores the pick by the window's app id and title,
so later launches record without a dialog. A cancelled picker records nothing. Off GNOME, with the
extension absent or not yet loaded, or when no game window appears in time, the screen is recorded
and the shell's OSD says so.

`quality` is a QVBR preset — constant quality up to a bitrate ceiling (`very_high`: 16 Mbps target,
32 Mbps ceiling, ~7-8 GB/h at 4K on AMD). Two config-scope keys, for `config.toml` or `universe
module set capture <key>=<value>` and never a settings row, replace what the presets choose:
`ffmpeg_video_opts` (gpu-screen-recorder's `-ffmpeg-video-opts`) and `gsr_extra_args` (appended
to the command). `-bm cbr` stays pinned: it is the base the QVBR override needs.

The module's `screenshot` hook grabs the frame in the shell through the extension
(`org.universe.Windows.Screenshot(path, window, cursor)`): the focused window's client area with
`source = "window"`, every monitor with `"screen"`, the cursor per `cursor`. Mutter reads the
framebuffer synchronously, so the shell's own cue — a flash over the captured area and the shutter —
fires and the hook returns at the press, before the PNG is encoded; a write that fails afterwards
is a shell notification. Off GNOME or before the shell has loaded the extension it is a
gpu-screen-recorder `-o` capture of the session's screen, with no cue.

## Journal

| Rust | Python | CLI | Role |
|---|---|---|---|
| `add_entry(session_id, entry)` | `add_entry(session_id, entry)` | `universe journal-add <session> <entry>` | validates the schema, fills `started_at`/`ended_at`/`duration_s` from the session line when the entry lacks them, writes `journal/<session>.json` |
| `journal(id)` | `journal(id)` | `universe journal <name>` | `[Entry]`, last first, read from disk on every call; the state files below are entries too |
| `pending_journals()` | `pending_journals()` | `universe status` (a `journal: writing <title>…` line; `pending_journals` in `--json`) | `[{game, title, session, started_at}]` for every `pending` entry across the library; `title` is the game's |
| `render_journal(id)` | `render_journal(id)` | `universe journal <name> --render` | renders `<journal_root>/<id>/<Title>.md` from the `written` entries, returns the path |
| `remove_journal_entry(id, session_id)` | `remove_journal_entry(id, session_id)` | `universe journal <name> --remove <session> [-y]` | trashes `journal/<session>.json` and the images it lists (their mirrors beside the note too); a `pending` entry has its `universe-journal-post-process-<session>` unit stopped and its state file removed; the note is rendered again when its folder exists |

`Entry` = `{"session", "game", "written_at", "started_at", "ended_at", "duration_s", "lang",
"title", "provider", "paragraphs": [], "next_up": "", "images": ["relative path"],
"state": "written"}`. `started_at`, `ended_at` and `duration_s` are the session's span; an entry
written before the core stamped them gets them at read time from `sessions.jsonl` (or the module's
migration sidecar), so every listing has one shape. `journal-add` is called by the journal module's
`post-process` hook, which also passes `started_at`, `ended_at` and `duration_s` so an entry it
writes itself (core unavailable) is self-contained. While the hook runs the session is
`journal/<session>.pending.json` (`{"session", "game", "started_at", "provider"}`); the file is
removed once the entry is in, or replaced by `<session>.failed.json` (`{"session", "game",
"written_at", "reason"}`, the reason being "codex quota reached", "provider error: …" or "no
images") when the run ends without one. A session with neither a recording nor a screenshot gets
no entry and no failed file.

The core lists those files as entries, sorted with the real ones: `state ∈ written, pending,
failed`. A `pending` entry has the file's `started_at` and `provider`, an empty title and no
paragraphs; a `failed` one has the file's `written_at` and `paragraphs = [reason]`. A pending file
whose mtime is more than 30 minutes old lists as `failed` with the reason `timed out`. A written
entry hides the failed one of the same session, a failed one the pending one. Dotfiles and anything
that is not `*.json` are ignored, and `render_journal` only renders `written` entries.

## Modules

| Rust | Python | CLI | Role |
|---|---|---|---|
| `modules()` | `modules()` | `universe module ls` | see below |
| `enable_module(id, enabled)` | `enable_module(id, enabled)` | `universe module enable\|disable <id>` | writes `[modules] enabled` in `config.toml` |
| `module_settings(module, game_id)` | `module_settings(…)` | `universe module settings <id> [game]` | global settings merged with the game's; `game_id=""` is global only |
| `set_module_setting(module, game_id, key, value)` | `set_module_setting(…)` | `universe module set <id> k=v [--game g]` | validated against `[[settings]]`. `game_id=""` writes `config.toml [modules.<id>]`, otherwise `game.toml [modules.<id>]` |
| `module_setting_choices(module, key)` | `module_setting_choices(…)` | — | the global setting's choices; a setting with `choices_exec` gets them from the module, live (see below) |
| `doctor()` | `doctor()` | `universe doctor` | `[{check, ok, detail, module}]`: required binaries, `gsr-kms-server`, Proton, cursor extension, tokens, one `runner-<id>` check per runner a library game uses (its program resolved), `inputplumber` when an emulator wants it |

A module entry is `{id, name, kind: [], version, dir, enabled, available, missing: [bin],
hooks: {}, settings: [Setting]}`, and
`Setting` = `{"key", "type": "bool|string|int|enum|path", "default", "label",
"scope": "global|game", "choices": [], "dynamic": bool}`. `choices` binds an `enum`; on an `int`
or a `string` it lists suggestions, any value stays accepted — except that an `int` also
takes a listed non-numeric name (`"auto"`), which the module resolves itself. `dynamic` is
set when the manifest names a `choices_exec`: `<module dir>/<choices_exec> <key>`, run with
`MODULE_SETTINGS_JSON`, `MODULE_DIR` and `MODULE_DATA_DIR`, prints the choices as a JSON
array of strings (20 s at most).

## Settings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `settings()` | `settings()` | `universe config get` | resolved `config.toml`: absolute paths, defaults applied |
| `set_setting(key, value)` | `set_setting(key, value)` | `universe config set <key> <value>` | dotted `config.toml` key (`launch.proton`, `paths.recordings_root`, `desktop.profile`); a `launch.*` key is validated against the catalogue, an unknown or game-only one refused |
| `launch_keys(scope, screen)` | `launch_keys(scope, screen)` | `universe launch-keys [--json]` | the launch keys of `scope` (`game`, `global`, `both`) that have a settings row: `[{key, type, default, choices, label, section, scope, runners, description}]`, `type` one of bool, int, string, path, list, enum, resolution, refresh, fps, proton; `screen` (a `screen_mode`, or none) sizes the resolution, refresh and fps choices; `runners` empty means every runner. The maps (`env`, `dll_overrides`, `options`) and the rowless keys (`runner`, `exe`, `umu_run`…) are settable but not listed; the CLI's table prints all of them |
| `screen_mode(screen)` | `screen_mode(screen)` | — | `{screen, width, height, refresh}`: the connector's current mode as gamescope is told it (see Gamescope), `screen=""` for the profile default; zeros when none can be read |
| — | `version()`, `data_home()`, `state_home()` | `universe --version` | |

## Controller

| Rust | Python | CLI | Role |
|---|---|---|---|
| `controller_state()` | `controller_state()` | `universe controller ls` | `{enabled, hold_ms, volume_step (a percent, number), mangohud_toggle, families: [{id, name, slots: [{id, label, codes, extra}]}], macros: [Macro], presets: [{id, label, hold_only}]}`; the CLI adds `devices`, the pads readable now with every slot's code or `bound: false` |
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
the PulseAudio server (PipeWire's included) through libpulse: the default sink's volume moves by
`volume_step` percent of the normal level on every channel, clamped to [0, 100 %], `mute` toggles
the sink; no key is typed, so nothing reaches the game. On GNOME the new level shows on the shell's
OSD through `org.universe.Windows.ShowOSD` on the Universe extension, labelled with the output as
GNOME's own volume keys print it (the sink's active port, else the sink); without the extension the
macro runs silently. `screenshot` has the capture's own cue (the capture module's flash and
shutter). `keys` types through uinput; `mangohud` sends MangoHud's own `toggle_hud` (from
`~/.config/MangoHud/MangoHud.conf`, `Shift_R+F12` by default) and holds it 200 ms; the launcher,
which that key never reaches, shows a toast on the fire — "MangoHud toggled · <title>", "MangoHud is
off for <title>" when the running game has it disabled, "MangoHud: no game running".

Families: `dualsense-edge` (fn_left, fn_right, paddle_left, paddle_right), `dualsense`,
`dualshock4`, `xbox-elite` (paddle_p1…p4), `xbox` (share), `switch-pro` (capture), `8bitdo-pro-3`
(paddle_l4, paddle_r4, paddle_pl, paddle_pr, star), `generic`; every family has the standard slots
`south east north west lb rb lt rt select start guide ls rs dpad_up dpad_down dpad_left dpad_right`.
A slot's codes are candidates: on every connect the first one the pad advertises in its capabilities
wins, so the Edge's paddles (`BTN_TRIGGER_HAPPY1…4`, kernel ≥ 7.2) and the Elite's (`BTN_GRIP*` over
xpadneo or xone, `BTN_TRIGGER_HAPPY5…8` on older drivers) resolve without a hardcoded number, and a
slot with no code present is reported unbound.

`watch` reads every `/dev/input/event*` that advertises `BTN_GAMEPAD` **without grabbing it** (a
game, SDL or Proton reads the same node untouched), rescans every 2 s (hotplug, and pads
InputPlumber hides by chmod 000 are dropped while hidden), and with `--json` speaks one object per
line: out — `{"event":"ready"}`, `{"event":"device","id":"event30","name","family","family_name",
"bus","slots":{"<slot>":{"code","bound"}}}`, `gone {id}`, `button {id, slot, code, pressed}`,
`unknown {id, code}` (a key no slot owns), `macro {id, slot, trigger, action, keys, command}`,
`learned {family, slot, code, from}`, `learn_timeout`, `waiting` / `busy` (the lock), `error
{message}`, and while `axes` is on, `axis {id, axis, value}` (`lx ly rx ry` as -1..1, `lt rt` as
0..1, a hundredth's resolution, on change); in — `{"cmd":"suspend"}` (report, do not fire),
`resume`, `axes {on}` (stream the sticks and triggers: the page's test mode), `reload` (config
changed), `learn {id, slot}`, `cancel`, `rumble {id}`, `quit`. Stdin's end stops a `--json` watcher. A
watcher also reloads by itself when `config.toml`'s mtime moves (checked on the 2 s scan), so a
bind from a terminal or from a launcher whose own watcher is waiting reaches the one holding the
pads. Either reload rereads config.toml and the module list only, never the library, so pad
events are not held up behind a rescan.

## config.toml

Defaults as the core ships them:

```toml
schema = 1

[paths]                              # defaults follow XDG and xdg-user-dirs
games_root = "~/Games"               # $XDG_GAMES_DIR: where sources install
prefixes_root = "~/.local/share/universe/prefixes"
recordings_root = "~/Videos/universe"            # $XDG_VIDEOS_DIR/universe
journal_root = "~/Documents/universe/journal"    # $XDG_DOCUMENTS_DIR/universe/journal
overrides = "~/.config/universe/overrides"       # picked art, shown over media/: <id>/{box_front,square,banner,background,logo}.*, <id>/screenshots/

[launch]
proton = "proton-ge"                 # a name under [proton], or a path
esync = true
fsync = true
ntsync = true                        # a sync mode off is PROTON_NO_*=1 (WINEESYNC/WINEFSYNC=0 for wine)
wayland = true                       # PROTON_ENABLE_WAYLAND=1; dropped inside gamescope unless --expose-wayland
hdr = false                          # PROTON_ENABLE_HDR=1, and --hdr-enabled on gamescope
dlss_upgrade = false                 # PROTON_DLSS_UPGRADE, PROTON_FSR4_UPGRADE, PROTON_XESS_UPGRADE, PROTON_USE_OPTISCALER
fsr4_upgrade = false
xess_upgrade = false
optiscaler = false
mangohud = true                      # --mangoapp inside gamescope, MANGOHUD=1 without
gamescope = true                     # every game inside gamescope: one window, black until the game draws
gamescope_args = ""                  # after the flags below, and over them; --expose-wayland keeps PROTON_ENABLE_WAYLAND
gamescope_bin = "gamescope"          # a name on PATH (/run/wrappers/bin included) or a path
gamescope_resolution = "auto"        # the screen's mode, or WxH: what the game renders at (-w -h); the output is always the screen
gamescope_refresh = "auto"           # the screen's rate, or Hz (-r)
gamescope_scaler = ""                # auto | integer | fit | fill | stretch (-S); empty: gamescope's default
gamescope_filter = ""                # linear | nearest | fsr | nis | pixel (-F)
# gamescope_sharpness = 2            # 0 (sharpest) to 20, for fsr and nis (--sharpness)
gamescope_adaptive_sync = false      # --adaptive-sync: variable refresh when the screen has it
fps_limit = "auto"                   # MangoHud's limiter in the game: auto (the refresh the game sees), none, or frames per second

[desktop]
profile = "auto"                     # auto | gnome | none
hide_cursor = true
cursor_extension = "hide-cursor@elcste.com"   # enabled for the session, restored to its prior state after

[proton]                             # name → path
proton-ge = "~/.local/share/lutris/runners/wine/proton-ge"

# [runners.<id>]                     # per runner (`universe runner set`): exe (absent: detected), args, gamescope, its options

[modules]
enabled = ["gog", "capture", "journal"]

[modules.capture]                    # the settings rows: `universe module settings capture`
# ffmpeg_video_opts = "rc_mode=CQP;qp=20"   # config-only: replaces the quality preset
# gsr_extra_args = "-cr full -keyint 2"      # config-only: appended to gpu-screen-recorder

[lutris]                             # what `universe migrate` reads
config_dir = "~/.config/lutris"
pga_db = "~/.local/share/lutris/pga.db"
runners_dir = "~/.local/share/lutris/runners/wine"
pegasus_library = "~/.local/share/pegasus-library"   # art fetched by pegasus-sync, copied into media/ on migrate

[keys]
sgdb = ""                            # or sgdb_file, pointing at a file holding the key
rawg = ""

[controller]
enabled = true
hold_ms = 600                        # a press this long is a hold
volume_step = 2                      # percent of the normal volume per press, 1–100 ("precise" = 2, "normal" = 6 still read)
mangohud_toggle = ""                 # empty: toggle_hud from ~/.config/MangoHud/MangoHud.conf, else Shift_R+F12
# [controller.buttons.xbox-elite]    # learned codes: a slot's list replaces its seeds, [] leaves it unbound
# paddle_p1 = ["BTN_GRIPR", "BTN_TRIGGER_HAPPY5"]
# [[controller.macros]]              # {family, button, trigger, action} (`universe controller bind`); absent: the seeded workflow, `macros = []` none at all
```

## Module protocol

A module is a directory `modules/<id>/` — system-wide under `$out/share/universe/modules/<id>`,
per-user under `$XDG_CONFIG_HOME/universe/modules/<id>`, where a user module overrides a system one
with the same id. It holds a `module.toml` and its executables. **The core never loads module
code**; it only runs the executables.

```toml
api = 1
id = "capture"
name = "Video capture"
kind = ["hooks"]                  # hooks | source; a module may be both
version = "0.0.1"

[requires]
bins = ["gpu-screen-recorder"]    # a missing binary makes the module "unavailable" and it is never run

[hooks]                           # paths relative to the module directory
pre-launch   = "bin/pre"          # blocking, before the game's unit; may write UNIVERSE_ENV_FILE
post-launch  = "bin/start"        # async (transient unit); anything that must last the session goes in its own unit with BindsTo=$SESSION_UNIT
session-end  = "bin/stop"         # short and blocking, inside the game unit's ExecStopPost; this is where a recording is filed
post-process = "bin/process"      # async (transient unit), after the session-end hooks
screenshot   = "bin/shot"         # on demand
timeout_s    = 20                 # for blocking hooks; the sum bounds the game unit's TimeoutStopSec

[limits]                          # applied to the transient units of async hooks
cpu_weight   = 100                # default 20
memory_high  = "4G"               # default 2G

[source]                          # kind = source
exe = "bin/source"                # run as: bin/source <verb> [args]

[[settings]]
key = "enabled"                   # reserved: always present, game scope
type = "bool"
default = true
label = "Record the session"
scope = "game"                    # global → config.toml [modules.<id>]; game → game.toml [modules.<id>];
                                  # config → config.toml / `universe module set` only, no settings row

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
```

### Hook environment

| Variable | Value | Hooks |
|---|---|---|
| `GAME_ID`, `GAME_SLUG`, `GAME_TITLE`, `GAME_DIR`, `GAME_EXE`, `GAME_TOML` | identity and paths (the TOML is read-only) | all |
| `SESSION_ID`, `SESSION_UNIT`, `SESSION_SCREEN`, `SESSION_STARTED_AT`, `SESSION_ENDED_AT`, `SESSION_DURATION_S` | the session; `SESSION_SCREEN` is a DRM connector | as applicable |
| `RECORDING_PATH` | the filed mkv, empty if there is none | `post-process` |
| `JOURNAL_DIR` | `games/<id>/journal` | all |
| `MODULE_SETTINGS_JSON` | global settings merged with the game's | all |
| `UNIVERSE_ENV_FILE` | write `KEY=VALUE` lines here to add them to the game's environment, ahead of `launch.env` | `pre-launch` |
| `MODULE_DIR`, `MODULE_DATA_DIR` | the module's directory, `$XDG_DATA_HOME/universe/modules/<id>` | all |
| `UNIVERSE_BIN`, `UNIVERSE_{DATA,CONFIG,STATE,CACHE}_HOME`, `UNIVERSE_MODULES_PATH`, `PATH` | the CLI to call back (`recording-file`, `journal-add`) and the environment that makes it open the same core | all |
| `UNIVERSE_GAME_JSON`, `UNIVERSE_JOURNAL_ROOT` | the resolved `Game`, serialized; `paths.journal_root` | all |

Exit codes: 0 is success; anything else is logged and the session continues — except a `pre-launch`
hook, where a non-zero exit cancels the launch.

### Source protocol

`bin/source <verb> [args]`, with `MODULE_SETTINGS_JSON` and `MODULE_DATA_DIR` in the environment.
A `games_dir` setting whose manifest default is empty arrives filled with `paths.games_root`.
One JSON object per line on stdout, human-readable logs on stderr, meaningful exit code.
**Every verb ends with `{"event":"done"}`**, `login` included.

| Verb | Argument | Events |
|---|---|---|
| `login` | `[code]` | without a code: `{"event":"login_url","url":…}`; with one: `{"event":"logged_in","user":…}` |
| `status` | — | `{"event":"logged_in","user":…}` if the session is valid, otherwise just `done`. Probed once per process, on the first listing |
| `library` | | `{"event":"game", …}` per owned title |
| `search` | `<text>` | `{"event":"game", …}` |
| `info` | `<id>` | `{"event":"info","data":{…}}` |
| `install` | `<id>` | `progress` lines, then the installed `game` |
| `update` | `[id]` | without an id: `{"event":"update","id","title","local_build","remote_build","version","date"}` per pending update; with one: `progress` then `game` |
| `scan` | | `{"event":"game", …}` per installation found, `owned` crossed with the cached library |

```json
{"event":"game","id":"1434554947","title":"Mini Metro","dir":"/mnt/games/PC/Mini Metro",
 "exe":"MiniMetro.exe","build":"5904…","owned":true,"installed":true,
 "release_year":2015,"dlcs":[]}
{"event":"progress","done":123,"total":456,"message":"27.0%"}
{"event":"done"}
```

`exe` is relative to `dir`. `owned` may be `null` when the module cannot tell.

---

Writing a frontend on top of this API: [`frontends.md`](frontends.md).
