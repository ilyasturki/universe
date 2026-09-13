# Universe — core API and module protocol

`api = 1`. The core is a Rust library (`crates/universe`, `universe::core::Core`). **No Universe
process runs in the background.** There are three ways into it, all in-process:

- **the crate** — `Core::open().await`, every method `async`;
- **`universe_core`** — the Python module built from `crates/universe-py` (PyO3). `Core()` opens the
  core; every method releases the GIL for the duration of the call. This is what the PySide6 host
  binds;
- **the `universe` CLI** — the same library, one process per command, `--json` on every command.
  This is what hooks and systemd call.

Heavy payloads are JSON strings. Each table below gives all three spellings of the same operation;
a dash means the surface doesn't expose it.

## Process model

- A play session is a **transient systemd unit**, `universe-game-<id>-<session>.service`, with
  `ExitType=cgroup` — it lives as long as any process of the game lives. Its
  `ExecStopPost=universe session-end <id> <session>` runs when the cgroup empties, whatever became
  of the launcher: it appends the `sessions.jsonl` line, restores the cursor extension, runs
  `post_command`, the `session-end` hooks, then the `post-process` hooks.
- **Who owns the game's lifetime** depends on the launcher. One that called `adopt_scope()` — the UI,
  and `universe play` without `--no-wait` — was moved into `universe-launcher-<pid>.scope`, and every
  game it launches carries `BindsTo=` + `After=` that scope: the game goes down with the launcher
  (a crash, a kill, Ctrl-C), `session-end` still runs. `universe play --no-wait`, hooks and anything
  else that never adopted a scope leave the game to systemd alone: it outlives them.
- `state/current-session.json` (written `O_EXCL`) is the marker for the running session; the current
  session is the marker whose unit is still active. On every open the core **reconciles**: a marker
  with no live unit is closed from `journalctl` timestamps.
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
| `list_json()` | `list_json()` | `universe ls [--all]` | JSON `[Game]`; unhidden first, last played first. `--json` always prints every game — `--all` only stops the table from hiding the hidden ones |
| `get(id)` | `get_json(id)` | `universe info <name> --json` | resolved `Game`: global defaults merged in, session stats, media, active modules |
| `resolve(query)` | `resolve(query)` | — | candidate ids: exact › whole word › substring › path. Empty means unknown, more than one means ambiguous |
| `set(id, key, value)` | `set(id, key, value)` | `universe set <name> k=v …` | writes one `game.toml` key |
| `remove(id, purge)` | `remove(id, purge)` | `universe rm <name> [--purge]` | parks recordings and journal under `.archive/`, marks `removed_at`; `purge` also trashes the prefix |
| `uninstall(id)` | `uninstall(id)` | `universe uninstall <name>` | trashes `source.dir` and clears `source.dir`, `source.build_id` and `launch.exe`; the game stays in the library, not installed. Refuses a root, a home or the games root |
| `reload_all()` | `reload()` | `universe rescan` | rereads config and `games/*/game.toml`, rebuilds the index, runs each source's `scan` |
| `reload_game(id)` | `reload_game(id)` | — | rereads one game |
| `import_lutris(apply)` | `import_lutris(apply)` | `universe migrate [--apply]` | JSON report: imported games, per-game env diff (`{id, lutris_env, universe_env, added, removed, changed}`), imported hours, games whose art was copied from `[lutris] pegasus_library` (`<platform>/media/<slug>/`, once, never over an existing `media/`), `runners_promoted` (emulator games from before runners that now name theirs) and `runners` (what Lutris's runner configs say: a wrapper script is seen through, the program is written to `[runners.<id>] exe` when it is not on PATH, its extra arguments to `args`). Without `apply` it only reports |
| `add_game(json)` | `add_game(json)` | `universe add <file> --runner <id> [--title T] [--platform P] [--media]` | `{"runner", "exe", "title"?, "platform"?}` → the new id. The title defaults to the file's name cleaned of release tags; the platform to the runner's first. Refuses an id already in the library |

`set` takes dotted keys: `launch.runner`, `launch.exe`, `launch.proton`, `launch.env.FOO`,
`launch.options.<key>` (validated against the runner's options), `desktop.hide_cursor`, `hidden`,
`favorite`, `tags`, `sort_title`, `platform`, `metadata.sgdb_id`, and `capture.cursor` as a
validated shorthand for `modules.capture.cursor`. Values are strings: `true`/`false` for booleans,
comma-separated for lists, `""` deletes the key. A runner is written under its shipped id (`yuzu` →
`eden`), and writing one retires the pre-runner `launch.backend` key.

`Game` (JSON) is the contents of `game.toml` plus:

```json
{"stats": {"hours": 12.5, "play_count": 7, "last_played": "RFC3339 or null"},
 "media": {"box_front": "path|null", "square": null, "tile": null, "background": null, "logo": null,
           "screenshots": ["path"]},
 "modules": {"capture": {"enabled": true, "cursor": false}},
 "effective": {"runner": "dolphin", "runner_name": "Dolphin", "runner_kind": "emulator",
               "runner_path": "/…/bin/dolphin-emu", "platform": "Nintendo GameCube",
               "options": {"batch": true, "user_directory": "", "inputplumber": true}, "inputplumber": true,
               "proton": "proton-ge", "proton_path": "…", "esync": true, "fsync": true, "mangohud": true,
               "hide_cursor": true, "env": {}},
 "removed": false}
```

`effective` is what the launch will use: the game's own keys over the global defaults, the runner
resolved (`runner_path` empty when its program was not found), the platform the runner implies when
the game sets none.

There is no change notification: the files are the truth, so a frontend watches `games/`,
`games/<id>/{,journal,media}` and `state/` and rereads. Everything the CLI, `session-end` and the
hooks write shows up that way, with no other channel.

## Sessions

| Rust | Python | CLI | Role |
|---|---|---|---|
| `launch(id, screen)` | `launch(id, screen)` | `universe play <name> [--screen DP-1] [--no-wait]` | pre-launch hooks, marker, `systemd-run`, post-launch hooks; returns the `session_id` at once. `screen` is a DRM connector name or `""` for the profile default. `Busy` if a session is already running |
| `stop(session_id)` | `stop(session_id)` | `universe stop` | `systemctl --user stop` on the unit |
| `adopt_scope()` | `adopt_scope()` | — (`universe play` does it unless `--no-wait`) | moves the calling process into the transient scope `universe-launcher-<pid>.scope` (`StartTransientUnit` on the user manager) and returns its name; every later `launch` binds the game to it. Idempotent. `Unavailable` without a user systemd |
| `screenshot()` | `screenshot()` | `universe screenshot` | runs the `screenshot` hook of whichever module declares one; returns the PNG path |
| `current()` / `current_json()` | `current_json()` | `universe status` | `{session_id, id, title, unit, screen, started_at}`, or `""`. The CLI wraps it: `status --json` prints `{"current": … or null, "recent": [the last 10 sessions], "pending_journals": [see Journal]}` |
| `sessions_json(id)` | `sessions_json(id)` | `universe sessions <name>` | JSON `[Session]` from `sessions.jsonl`, last first |
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

## Sources

| Rust | Python | CLI | Role |
|---|---|---|---|
| `sources_json()` | `sources_json()` | `universe sources` | `[{id, name, available, enabled, missing, logged_in, user, games_dir, library_cached}]` |
| `source_login_url(source)` | `login_url(source)` | `universe login <source>` | URL to open |
| `source_login(source, code)` | `login(source, code)` | `universe login <source> <code>` | returns the user name |
| `source_library(source, refresh)` | `library_json(source, refresh)` | `universe library [source] [--refresh]` | `[SourceGame]`, served from cache unless `refresh` |
| `source_search(source, query)` | `search_json(source, query)` | `universe search <query> [--source]` | `[SourceGame]` |
| `source_info(source, game_id)` | `info_json(source, game_id)` | — | the source's raw `info` payload |
| `source_install(source, game_id, progress)` | `install(source, game_id, progress)` | `universe install <id> [--source]` | id of the installed game |
| `source_update(source, game_id, progress)` | `update(source, game_id, progress)` | `universe update [name] [-y]` | how many were updated; `game_id=""` updates everything pending |
| `source_updates()` | `updates_json()` | `universe update` | `[{id, title, local_build, remote_build, version, date}]` |
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
| `runners_json()` | `runners_json()` | `universe runner ls` · `runner options <id>` | `[Runner]`, see below |
| `set_runner_setting(id, key, value)` | `set_runner_setting(…)` | `universe runner set <id> k=v …` | writes `config.toml [runners.<id>] <key>`: `exe`, `args`, or an option, validated by type; `""` resets it |

`Runner` = `{"id": "dolphin", "name": "Dolphin", "kind": "proton|wine|linux|emulator", "aliases": ["…"],
"lutris": "dolphin", "binaries": ["dolphin-emu"], "platforms": ["Nintendo GameCube", "Nintendo Wii"],
"extensions": ["iso", …], "exe": "the configured program or empty", "args": "extra arguments, shell-quoted",
"path": "the program that will run, empty when none was found", "source": "config|path|lutris|",
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
| `media_refresh(id, force, progress)` | `media_refresh(id, force, progress)` | `universe media <name> refresh` | `(changed, total)` from SteamGridDB, RAWG, Steam screenshots and the overrides directory; `id=""` does every game |
| `media_set_slot(id, slot, path)` | `media_set_slot(…)` | `universe media <name> set <slot> <path>` | copies into `media/<slot>.<ext>` |
| `media_unset(id, slot)` | `media_unset(id, slot)` | `universe media <name> unset <slot>` | |
| `media_candidates(id, slot)` | `media_candidates_json(…)` | `universe media <name> candidates <slot>` | `[{provider, url or path, score}]`, cached in `.sync.json` |
| `media_pin(id, provider, provider_id)` | `media_pin(…)` | `universe media <name> pin <provider> <id>` | `provider ∈ sgdb, rawg, steam` → `metadata.<provider>_id` |

`slot ∈ box_front, square, tile, background, logo, screenshot`. `square` is the 1:1 grid (SteamGridDB 1024×1024, then 512×512), the Switch 2 theme's tile; `tile` stays the 920×430 banner.

## Recordings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `file_recording(session_id, path)` | `file_recording(session_id, path)` | `universe recording-file <session> <path>` | files the mkv as `<recordings_root>/<id>/<session>.mkv` (rename within a filesystem, copy across), writes `recording` into the session line, prints the final path |
| `recordings_json(id)` | `recordings_json(id)` | `universe recordings <name>` | `[{session, path, size, duration_s, created_at}]` |

`recording-file` is called by the capture module's `session-end` hook, so it lands before any
`post-process` hook runs.

The capture module records either the game's **window** (`source = "window"`, the default) or the
whole **screen** (`source = "screen"`). Window capture needs GNOME and the `universe@ilyasturki.github.io`
shell extension (shipped by the home-manager module, loaded after one logout): the module enables it,
lists the game's toplevels through it, and records the largest one through Mutter's private
`org.gnome.Mutter.ScreenCast` into `<session>-N.mkv` segments (a new segment when the window is
replaced), which `session-end` concatenates into `<session>.mkv`. It follows the window across
workspaces and occlusion, and starts a fresh segment on a new window. Off GNOME, with the extension
absent or not yet loaded, or when no game window appears within 60 s, it falls back to the
gpu-screen-recorder screen path. Window capture is the monitor's resolution with the window composited
on it (exact for a fullscreen game).

## Journal

| Rust | Python | CLI | Role |
|---|---|---|---|
| `add_entry(session_id, json)` | `add_entry(session_id, json)` | `universe journal-add <session> <entry>` | validates the schema, fills `started_at`/`ended_at`/`duration_s` from the session line when the entry lacks them, writes `journal/<session>.json` |
| `journal_json(id)` | `journal_json(id)` | `universe journal <name>` | `[Entry]`, last first, read from disk on every call; the state files below are entries too |
| `pending_journals_json()` | `pending_journals_json()` | `universe status` (a `journal: writing <title>…` line; `pending_journals` in `--json`) | `[{game, title, session, started_at}]` for every `pending` entry across the library; `title` is the game's |
| `render_journal(id)` | `render_journal(id)` | `universe journal <name> --render` | renders `<journal_root>/<id>/<Title>.md` from the `written` entries, returns the path |

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
| `modules_json()` | `modules_json()` | `universe module ls` | see below |
| `enable_module(id, enabled)` | `enable_module(id, enabled)` | `universe module enable\|disable <id>` | writes `[modules] enabled` in `config.toml` |
| `module_settings_json(module, game_id)` | `module_settings_json(…)` | `universe module settings <id> [game]` | global settings merged with the game's; `game_id=""` is global only |
| `set_module_setting(module, game_id, key, value)` | `set_module_setting(…)` | `universe module set <id> k=v [--game g]` | validated against `[[settings]]`. `game_id=""` writes `config.toml [modules.<id>]`, otherwise `game.toml [modules.<id>]` |
| `module_setting_choices(module, key)` | `module_setting_choices_json(…)` | — | the global setting's choices; a setting with `choices_exec` gets them from the module, live (see below) |
| `doctor_json()` | `doctor_json()` | `universe doctor` | `[{check, ok, detail, module}]`: required binaries, `gsr-kms-server`, Proton, cursor extension, tokens, one `runner-<id>` check per runner a library game uses (its program resolved), `inputplumber` when an emulator wants it |

A module entry is `{id, name, kind: [], version, dir, enabled, available, missing: [bin],
hooks: {}, verbs: [], settings: [Setting], frontend_qml: "path or null"}`, and
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
| `settings_json()` | `settings_json()` | `universe config get` | resolved `config.toml`: absolute paths, defaults applied |
| `set_setting(key, value)` | `set_setting(key, value)` | `universe config set <key> <value>` | dotted `config.toml` key (`launch.proton`, `paths.recordings_root`, `desktop.profile`) |
| — | `version()`, `data_home()`, `state_home()` | `universe --version` | |

## Controller

| Rust | Python | CLI | Role |
|---|---|---|---|
| `controller_state_json()` | `controller_state_json()` | `universe controller ls` | `{enabled, hold_ms, volume_step (a percent, number), mangohud_toggle, families: [{id, name, slots: [{id, label, codes, extra}]}], macros: [Macro], presets: [{id, label, hold_only}]}`; the CLI adds `devices`, the pads readable now with every slot's code or `bound: false` |
| `controller_pads_json()` | `controller_pads_json()` | — | the `devices` list alone, in the shape `watch` announces; for a frontend whose watcher waits on the lock |
| `set_controller_macro(json)` | same | `universe controller bind <family> <button> <press\|hold> <action> [--keys K] [--command C]` | validates, replaces the macro with the same family, button and trigger |
| `remove_controller_macro(family, button, trigger)` | same | `universe controller unbind <family> <button> [trigger]` | an empty trigger removes both |
| `set_controller_button(family, slot, codes_json)` | same | `universe controller learn <family> <slot>` · `forget` | the codes a slot answers to: a JSON list (empty leaves it unbound), `null` restores the seeds. `learn` reads the pad instead: the next button pressed becomes the slot's, taken from whichever slot had it |
| — | — | `universe controller watch [--json] [--wait]` | the engine |

`Macro` = `{"family": "dualsense-edge" | "*", "button": "paddle_left", "trigger": "press" | "hold",
"action": "volume_up" | "volume_down" | "mute" | "screenshot" | "mangohud" | "stop" | "keys" |
"command", "keys": "Super_L+F12", "command": "…"}`. `stop` is hold-only. `volume_up` and
`volume_down` repeat while held (400 ms, then every 100 ms), unless the slot also carries a hold.
A slot with only a press macro fires on the key down; with a hold macro too, press fires on a release
before `hold_ms` and hold once at `hold_ms`. `volume_up`, `volume_down` and `mute` go straight to
the PulseAudio server (PipeWire's included) through libpulse: the default sink's volume moves by
`volume_step` percent of the normal level on every channel, clamped to [0, 100 %], `mute` toggles
the sink; no key is typed, so nothing reaches the game. On GNOME the new level (and a screenshot's
`camera-photo-symbolic`) shows on the shell's OSD through `org.universe.Windows.ShowOSD` on the
Universe extension; without it the macro runs silently. `keys` types through uinput; `mangohud`
sends MangoHud's own `toggle_hud` (from `~/.config/MangoHud/MangoHud.conf`, `Shift_R+F12` by
default) and holds it 200 ms.

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
overrides = "~/.config/universe/overrides"       # hand-picked art: <id>/{boxFront,square,tile,background,logo}.*, <id>/screenshots/

[launch]
proton = "proton-ge"                 # a name under [proton], or a path
esync = true
fsync = true
mangohud = true

[desktop]
profile = "auto"                     # auto | gnome | none
hide_cursor = true
cursor_extension = "hide-cursor@elcste.com"   # enabled for the session, restored to its prior state after

[proton]                             # name → path
proton-ge = "~/.local/share/lutris/runners/wine/proton-ge"

# [runners.dolphin]                  # per runner: the program, extra arguments, its options
# exe = "/opt/dolphin/dolphin-emu"   # empty or absent: detected (PATH, then Lutris's runners dir)
# args = "--config Dolphin.Display.Fullscreen=True"
# batch = true

[modules]
enabled = ["gog", "capture", "journal"]

[modules.capture]
source = "window"                    # window (GNOME + the Universe shell extension) | screen (gpu-screen-recorder)
codec = "av1_10bit"

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
# [[controller.macros]]              # absent: the seeded workflow (Edge: Fn = screenshot / MangoHud,
# family = "dualsense-edge"          # paddles = volume; Elite P1…P4 = MangoHud, volume up, screenshot,
# button = "paddle_left"             # volume down; Pro 3 R4 / PR / PL); `macros = []` is none at all
# trigger = "press"
# action = "volume_down"
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
version = "0.0.0"

[requires]
core = "=0.0.0"
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

[frontend]
qml = "ui/Page.qml"               # optional: a screen added to the frontend

[[settings]]
key = "enabled"                   # reserved: always present, game scope
type = "bool"
default = true
label = "Record the session"
scope = "game"                    # global → config.toml [modules.<id>]; game → game.toml [modules.<id>]

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
| `UNIVERSE_GAME_JSON` | the resolved `Game`, serialized | all |

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
