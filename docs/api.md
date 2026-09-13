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
| `import_lutris(apply)` | `import_lutris(apply)` | `universe migrate [--apply]` | JSON report: imported games, per-game env diff (`{id, lutris_env, universe_env, added, removed, changed}`), imported hours, games whose art was copied from `[lutris] pegasus_library` (`<platform>/media/<slug>/`, once, never over an existing `media/`). Without `apply` it only reports |

`set` takes dotted keys: `launch.proton`, `launch.env.FOO`, `desktop.hide_cursor`, `hidden`,
`favorite`, `tags`, `sort_title`, `metadata.sgdb_id`, and `capture.cursor` as a validated shorthand
for `modules.capture.cursor`. Values are strings: `true`/`false` for booleans, comma-separated for
lists, `""` deletes the key.

`Game` (JSON) is the contents of `game.toml` plus:

```json
{"stats": {"hours": 12.5, "play_count": 7, "last_played": "RFC3339 or null"},
 "media": {"box_front": "path|null", "tile": null, "background": null, "logo": null,
           "screenshots": ["path"]},
 "modules": {"capture": {"enabled": true, "cursor": false}},
 "removed": false}
```

There is no change notification: the files are the truth, so a frontend watches `games/`,
`games/<id>/{,journal,media}` and `state/` and rereads. Everything the CLI, `session-end` and the
hooks write shows up that way, with no other channel.

## Sessions

| Rust | Python | CLI | Role |
|---|---|---|---|
| `launch(id, screen)` | `launch(id, screen)` | `universe play <name> [--screen DP-1] [--no-wait]` | pre-launch hooks, marker, `systemd-run`, post-launch hooks; returns the `session_id` at once. `screen` is a DRM connector name or `""` for the profile default. `Busy` if a session is already running |
| `stop(session_id)` | `stop(session_id)` | `universe stop` | `systemctl --user stop` on the unit |
| `screenshot()` | `screenshot()` | `universe screenshot` | runs the `screenshot` hook of whichever module declares one; returns the PNG path |
| `current()` / `current_json()` | `current_json()` | `universe status` | `{session_id, id, title, unit, screen, started_at}`, or `""`. The CLI wraps it: `status --json` prints `{"current": … or null, "recent": [the last 10 sessions]}` |
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

## Media

| Rust | Python | CLI | Role |
|---|---|---|---|
| `media_refresh(id, force, progress)` | `media_refresh(id, force, progress)` | `universe media <name> refresh` | `(changed, total)` from SteamGridDB, RAWG, Steam screenshots and the overrides directory; `id=""` does every game |
| `media_set_slot(id, slot, path)` | `media_set_slot(…)` | `universe media <name> set <slot> <path>` | copies into `media/<slot>.<ext>` |
| `media_unset(id, slot)` | `media_unset(id, slot)` | `universe media <name> unset <slot>` | |
| `media_candidates(id, slot)` | `media_candidates_json(…)` | `universe media <name> candidates <slot>` | `[{provider, url or path, score}]`, cached in `.sync.json` |
| `media_pin(id, provider, provider_id)` | `media_pin(…)` | `universe media <name> pin <provider> <id>` | `provider ∈ sgdb, rawg, steam` → `metadata.<provider>_id` |

`slot ∈ box_front, tile, background, logo, screenshot`.

## Recordings

| Rust | Python | CLI | Role |
|---|---|---|---|
| `file_recording(session_id, path)` | `file_recording(session_id, path)` | `universe recording-file <session> <path>` | files the mkv as `<recordings_root>/<id>/<session>.mkv` (rename within a filesystem, copy across), writes `recording` into the session line, prints the final path |
| `recordings_json(id)` | `recordings_json(id)` | `universe recordings <name>` | `[{session, path, size, duration_s, created_at}]` |

`recording-file` is called by the capture module's `session-end` hook, so it lands before any
`post-process` hook runs.

## Journal

| Rust | Python | CLI | Role |
|---|---|---|---|
| `add_entry(session_id, json)` | `add_entry(session_id, json)` | `universe journal-add <session> <entry>` | validates the schema, writes `journal/<session>.json` |
| `journal_json(id)` | `journal_json(id)` | `universe journal <name>` | `[Entry]`, last first |
| `render_journal(id)` | `render_journal(id)` | `universe journal <name> --render` | renders `<journal_root>/<id>/<Title>.md`, returns the path |

`Entry` = `{"session", "game", "written_at", "lang", "title", "provider", "paragraphs": [],
"next_up": "", "images": ["relative path"]}`. `journal-add` is called by the journal module's
`post-process` hook.

## Modules

| Rust | Python | CLI | Role |
|---|---|---|---|
| `modules_json()` | `modules_json()` | `universe module ls` | see below |
| `enable_module(id, enabled)` | `enable_module(id, enabled)` | `universe module enable\|disable <id>` | writes `[modules] enabled` in `config.toml` |
| `module_settings_json(module, game_id)` | `module_settings_json(…)` | `universe module settings <id> [game]` | global settings merged with the game's; `game_id=""` is global only |
| `set_module_setting(module, game_id, key, value)` | `set_module_setting(…)` | `universe module set <id> k=v [--game g]` | validated against `[[settings]]`. `game_id=""` writes `config.toml [modules.<id>]`, otherwise `game.toml [modules.<id>]` |
| `module_setting_choices(module, key)` | `module_setting_choices_json(…)` | — | the global setting's choices; a setting with `choices_exec` gets them from the module, live (see below) |
| `doctor_json()` | `doctor_json()` | `universe doctor` | `[{check, ok, detail, module}]`: required binaries, `gsr-kms-server`, Proton, cursor extension, tokens |

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
| `controller_state_json()` | `controller_state_json()` | `universe controller ls` | `{enabled, hold_ms, volume_step, mangohud_toggle, families: [{id, name, slots: [{id, label, codes, extra}]}], macros: [Macro], presets: [{id, label, hold_only}]}`; the CLI adds `devices`, the pads readable now with every slot's code or `bound: false` |
| `set_controller_macro(json)` | same | `universe controller bind <family> <button> <press\|hold> <action> [--keys K] [--command C]` | validates, replaces the macro with the same family, button and trigger |
| `remove_controller_macro(family, button, trigger)` | same | `universe controller unbind <family> <button> [trigger]` | an empty trigger removes both |
| `set_controller_button(family, slot, codes_json)` | same | `universe controller learn <family> <slot>` · `forget` | the codes a slot answers to: a JSON list (empty leaves it unbound), `null` restores the seeds. `learn` reads the pad instead: the next button pressed becomes the slot's, taken from whichever slot had it |
| — | — | `universe controller watch [--json] [--wait]` | the engine |

`Macro` = `{"family": "dualsense-edge" | "*", "button": "paddle_left", "trigger": "press" | "hold",
"action": "volume_up" | "volume_down" | "mute" | "screenshot" | "mangohud" | "stop" | "keys" |
"command", "keys": "Super_L+F12", "command": "…"}`. `stop` is hold-only. `volume_up` and
`volume_down` repeat while held (400 ms, then every 100 ms), unless the slot also carries a hold.
A slot with only a press macro fires on the key down; with a hold macro too, press fires on a release
before `hold_ms` and hold once at `hold_ms`. Keys type through uinput; `mangohud` sends MangoHud's
own `toggle_hud` (from `~/.config/MangoHud/MangoHud.conf`, `Shift_R+F12` by default) and holds it
200 ms; `volume_step = "precise"` sends Shift with the volume key (GNOME's fine step).

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
{message}`; in — `{"cmd":"suspend"}` (report, do not fire), `resume`, `reload` (config changed),
`learn {id, slot}`, `cancel`, `rumble {id}`, `quit`. Stdin's end stops a `--json` watcher.

## config.toml

Defaults as the core ships them:

```toml
schema = 1

[paths]                              # defaults follow XDG and xdg-user-dirs
games_root = "~/Games"               # $XDG_GAMES_DIR: where sources install
prefixes_root = "~/.local/share/universe/prefixes"
recordings_root = "~/Videos/universe"            # $XDG_VIDEOS_DIR/universe
journal_root = "~/Documents/universe/journal"    # $XDG_DOCUMENTS_DIR/universe/journal
overrides = "~/.config/universe/overrides"       # hand-picked art: <id>/{boxFront,tile,background,logo}.*, <id>/screenshots/

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

[modules]
enabled = ["gog", "capture", "journal", "tracker-md"]

[modules.capture]
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
volume_step = "precise"              # Shift + the volume key (GNOME's fine step); "normal" for the plain key
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
version = "0.1.0"

[requires]
core = ">=0.1"
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
