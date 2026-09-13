# Writing a frontend

The core is a library, not a service: a frontend **opens it in its own process**. There is no IPC to
speak, no schema to negotiate, and no second implementation to keep in step — a frontend either
links the Rust crate, or imports `universe_core` (the PyO3 module) and calls the same methods the
CLI calls. [`api.md`](api.md) is the full surface.

The shipped frontend, `universe-ui`, lives in `ui/`: a PySide6 host around the
[Reprise](https://github.com/ilyasturki/pegasus-theme-reprise) QML theme. What follows is its
contract and the parts of it that were expensive to get right; a GTK or Windows frontend owes none
of it except the "Changes" section, which is a property of the core.

```
ui/
  pyproject.toml            the universe_ui package, the universe-ui script
  universe_ui/
    host.py                 QGuiApplication + QQmlApplicationEngine, options, screen capture, key scripting
    api.py                  the `api` object QML sees
    models.py               Game, GameListModel and the QML proxies (import Universe)
    universe_client.py      CoreClient (the real core) and FakeClient (fixtures)
    gamepad.py              SDL2 → QKeyEvent
    screens/                data for the added screens: settings.py, sources.py, media.py, paths.py, controller.py, runners.py
    fixtures/               library.json and generated artwork, for --fake
    qml/                    the ported theme plus the added screens; ui/Pad*.qml and PadGeometry.js draw the pads, assets/runners/ holds the runner logos (SOURCES.md says where each is from)
  tests/                    pytest, offscreen
```

## What QML sees

One context property, `api`:

| Member | What it is |
|---|---|
| `api.keys` | `is{Accept,Cancel,Details,Filters,PageUp,PageDown,PrevPage,NextPage,Menu}(event)` — the Pegasus key-action contract |
| `api.allGames` | the library model |
| `api.collections` | collections, one per platform |
| `api.memory` | `get`/`set`/`has`/`unset`, persisted to `$XDG_STATE_HOME/universe/ui-memory.json` |
| `api.universe` | the client: every core call, plus the signals below. `adoptScope()` and `pendingJournals()` wrap `adopt_scope` and `pending_journals_json`; a core without them gets a log line, not a toast |
| `api.pad` | `rightX` / `rightY`: the right stick as a value, 0 without a controller |
| `api.screens` | data for the added screens (settings, sources, media, the folder picker, the controller, the journals being written) |
| `api.fullscreen` | whether the host runs fullscreen (the default; `--windowed`, `--size` and `--screenshot` turn it off) |

A `Game` exposes `id`, `title`, `sortTitle`, `favorite` (writable), `hidden`, `playTime`,
`playCount`, `lastPlayed`, `releaseYear`, `developerList`, `publisherList`, `genreList`, `players`,
`description`, `summary`, `source`, `platform`, `runner`, `runnerName`, `tags`, `extra`, `raw`,
`collections`, and `assets` (`boxFront`, `tile`, `background`, `logo`, `screenshotList`), plus
`launch()`.

`api.screens.recordings` samples 16 frames per recording with ffmpeg into
`$XDG_CACHE_HOME/universe/frames/<sha1 of the path>/NN.jpg` (`stats.json` keeps the probed duration and
each frame's 8×8 gray stddev, so the list thumbnail is the first frame that is not black or a fade).
Two extractions run at a time; the picked row's frames go first, the others' thumbnails after.
`frameMap[session]` carries `thumbnail`, `frames` (`""` until extracted), `complete` and `duration`.

`api.screens.journal` maps a game's entries to rows — `session`, `title`, `state`, `reason`,
`started_at`, `dateText`, `duration_s`, `durationText`, `paragraphs`, `blocks`, `next_up`,
`images`, `hasRecording` — sorted by session id, last first. `state` is `written`, `pending` (the
module is still writing: no title, no paragraphs; the row pulses with the time since `started_at`
and cannot be opened) or `failed` (`reason` is the module's message, its one paragraph).
`durationText` is the session's length — `42 min`, `1 h 05` — next to the date in the row and in
the article header; the date is `written_at`, or `started_at` while there is none.
`api.screens.pendingJournals` is `pending_journals_json()` as `rows` and `count`, refreshed on
`entryWritten`, on `sessionEnded` and every 10 s while any is pending (so the elapsed time and
the module's 30-min timeout show up); `appeared(session, title)` and
`resolved(session, game, state, text)` fire once per session and become the "Journal: writing …",
"Journal: <title>" and "Journal failed: <reason>" toasts, and the tab bar pulses a book next to
the session badge while the count is not zero.

## Changes

The core pushes nothing — the files are the truth, and anything may write them: the CLI, systemd's
`session-end`, a hook. A frontend therefore derives its own change notifications. `CoreClient` does
it this way, and any frontend needs the equivalent:

| Signal | Derived from |
|---|---|
| `sessionStarted` | a successful `launch` |
| `sessionEnded` | the current-session marker going empty — polled every 2 s while a session is tracked, since the game is a systemd unit, not a child. `currentSessionChanged` fires first; the theme's running view follows that property, and only the toast and the stats refresh follow the signal |
| `libraryChanged`, `mediaChanged`, `entryWritten`, `recordingFiled` | a `QFileSystemWatcher` on `games/`, `games/<id>/{,journal,media}` and `state/`, debounced 300 ms |
| `progress`, `jobFinished` | the job's own callback — install, update, scan and media refresh run on a host thread |
| `launched`, `launchFailed`, `error` | the call's result |

A frontend decides who owns the game's lifetime. `adopt_scope()`, called once at startup, moves the
frontend into `universe-launcher-<pid>.scope`; every game it launches from then on is bound to that
scope and goes down with the frontend, `session-end` included. `host.py` calls `adoptScope()` once
the client exists — not with `--fake`, nor for `--screenshot` — so closing `universe-ui` closes the
game; a frontend that never calls it leaves the game to systemd, as `universe play --no-wait` and
the hooks do. The host also quits cleanly on SIGINT and SIGTERM (a wakeup-fd `QSocketNotifier`
lets the Python handlers run under the Qt loop) and, with a session running, calls `stop("")`
before `api.shutdown()`, so `session-end` has run before the scope goes. No confirmation is asked.

Two consequences worth knowing before you design around them. **`launch` blocks on the pre-launch
hooks** — up to the summed `timeout_s` of every blocking hook, 20 s by default — so it must not run
on the UI thread if you want the launch animation to keep moving. And **long jobs die with the
process**: closing the frontend mid-install interrupts it, by design.

## Launch and the running view

`launchGame` raises `ui/LaunchOverlay.qml` over the page: the poster (`ui/LaunchFrame.qml`) fades
in over `Theme.durLaunch` as the page fades out, holds 450 ms, dips its art to plain ground over
300 ms — whatever the compositor animates between this window and the splash is then black on
black — and calls `launch()`. From there the overlay follows `api.universe.currentSession`, not a
timer: while it is set the poster stays as the running view — the art back up under a dim, the
logo, "PLAYING · elapsed" above it, one focused pill "Quit <title>" and a hint bar with the
clock. The overlay holds the focus. Accept held for one second fills the pill and calls
`stop(session_id)` ("Stopping…" until the session goes); a release, or the window losing focus,
cancels the hold, and every other key does nothing. When the pad in hand has a `stop` hold macro
bound (`api.screens.controller.state.macros`), the hint bar names its button too. A session
already running when the host starts (`CoreClient` tracks the marker at construction) shows the
running view at once, resolved through `api.allGames.byId`.

When `currentSession` empties — up to 2 s after the game exits, the poll interval — the overlay
signals `ended(game)`: the theme drops `launching` and opens the game's `DetailPage` under the
fading poster, so its stats and its journal are one press away; the journal entry the module is
writing shows up there as a pending row. `launchFailed` still aborts the poster to a toast
(`finished` then `failed`). A session the CLI started is tracked the same way (the `state/`
watch), so the overlay covers the page for it too; the game menu's "Stop <title>" item is no
longer reachable.

## Qt and QML notes

These cost real time to discover; they are properties of Qt 6.11 / PySide6 6.11, not of Universe.

- **`QtQml.Models.SortFilterProxyModel` cannot carry the theme's filters.** It has no `get()`, no
  `ExpressionFilter`, and its `FunctionFilter` segfaults. Every proxy the theme needs is instead a
  `QSortFilterProxyModel` subclass. `SortedGames`, `RecentGames`, `LimitedGames`,
  `FavouriteGames`, `SearchGames` and `LibraryGames` are exported to QML as `import Universe`;
  `CollectionGames` is instantiated Python-side, one per collection. `get(i)` returns the `Game`;
  `sourceRow(i)` stands in for `mapToSource(i)`, whose C++ name is virtual and breaks sorting if
  shadowed.
- **QML import paths.** Under a bare `nix shell` nothing sets `QML2_IMPORT_PATH`, so `host.py`
  derives the paths by running `ldd` over PySide6's own `.so` files and adding the `qt5compat` and
  `qtsvg` plugins from the store that link the *same* qtbase — several coexist and a plugin bound to
  a different qtbase is refused. Once `QML2_IMPORT_PATH` or `QML_IMPORT_PATH` is set (the flake's
  `wrapQtAppsHook`, `nix flake check`), it leaves them alone.
- **Software rendering has no shaders.** The `offscreen` QPA loads the software scenegraph, where
  `OpacityMask`, `FastBlur` and `ColorOverlay` render nothing. The affected components test
  `GraphicsInfo.api === GraphicsInfo.Software` and degrade — square corners, no blur, untinted
  icons. On screen nothing changes.
- **Keys posted while a game holds focus are dropped** by Qt: there is no active item, so
  `--keys` scripting cannot drive the host behind a running game, and neither can the gamepad
  (`focusWindow()` is null). That is the wanted behaviour: the running view is operable once the
  game has handed the focus back (Alt-Tab, or its own exit).
- **Collections are platforms.** The theme labels a collection by its `shortName` and looks for
  `assets/platforms/<shortName>.svg`; the core gives a platform string, which `api.py` maps
  (`windows`, `switch`, `wii`, `gamecube`, `nds`, `ps3`, …) and falls back to the source id.
- **Gamepad.** SDL2 in a `QThread`, `SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1`, hot-plug. Each
  button becomes the keyboard key Pegasus already bound to that action, so the theme's key logic is
  untouched. Axes use hysteresis — 0.5 to press, 0.3 to release — and arrows repeat after 350 ms
  every 90 ms. Nothing is posted when no window has focus.

| Pad | Key | Action |
|---|---|---|
| A / B | Return / Esc | Accept / Cancel |
| X / Y | I / F | Details / Filters |
| LB / RB | Q / E | previous / next tab |
| LT / RT | PageUp / PageDown | collection, section, keyboard page |
| Start, Guide | F1 | context menu of the game on screen, whichever part of the page has focus |
| d-pad, left stick | arrows | navigation |
| right stick | `api.pad.rightX` / `rightY` | analog, past a 0.18 deadzone: scrubs the recording player |

`api.pad.muted` (set by `Api` while the controller section's live view is on) drops the presses;
releases still land, so nothing stays held across it. The hint bar names buttons the Xbox way
(`ButtonGlyph`: `A`, `LB RB`, `Start Select`, `dpad`) and draws them the way the connected pad
prints them (`PadGlyph`, from `PadNames.js`: × ○ △ □ and L1/R1 on a DualSense, B/A and L/R on a
Switch pad); with no pad the watcher sees, they stay Xbox letters.

## The runners section

`api.screens.runners` is the Runners section of the Settings tab. `load()` reads `Runners1.List`
and builds one card per runner: its logo (`assets/runners/<id>.svg|png`, `logo(id)` says which
exists), its platforms and where its program was found in the header, then rows for the program
(`exe`, a path; inherited when detected), the arguments, each option by its type, and an "Add a
game…" action. `setValue(index, value)` writes through `Runners1.Set`; on the add row it keeps the
picked file and `pendingTitle()` proposes a title from it, which `addGame(title)` sends to
`Library1.Add`. The game settings page's Launch group follows the runner: a Runner picker (names
shown, ids written), then the rows the runner takes. The detail page shows the runner's logo next
to the platform.

## The controller section

`api.screens.controller` is the Controller section of the Settings tab. The host starts
`universe controller watch --json --wait` as a child for its lifetime (`$UNIVERSE_BIN`, else
`universe` on `PATH`) and reads its event lines: `device`, `gone`, `button`, `axis`, `unknown`,
`macro`, `learned`, `learn_timeout`, `error`, `waiting`, `ready`; it writes `suspend`, `resume`,
`axes`, `reload`, `learn` and `cancel` commands on its stdin. The screen exposes `devices`,
`current`, `family` (the current pad's, else the last one seen, kept in `api.memory` as
`controllerFamily`), `connected`, `status` (`off`, `waiting`, `ready`), `passive`, `learning`,
`testing`, the `rows`/`groups` of one card (a Controller picker row when two pads are connected,
a "Test the buttons" row while the watcher is `ready`, then a row per button of the family — each
with its `slot` and `family`, so the row draws the button's glyph — the extras (back buttons, Fn)
first, then the standard buttons) and `bind`, `unbind`, `learn`, `cancelLearn`, `setTesting`,
`suspend`, `resume`. Macros and families come from the core's `Controller1.State`; a write goes
through `Controller1.Bind`/`Unbind` and is followed by a `reload` to the watcher.

While another watcher holds the pads (a game launched from the CLI is running) the child reports
`waiting`: the screen turns passive, lists the pads from `Controller1.Pads` under an info row
saying macros run in the game session, binds as usual (the holder reloads on the config's mtime)
and refuses `learn` until `ready`. A watcher that exits reports `off`: the card says so and the
screen restarts it after `restart_ms`, doubling up to 30 s until it stays up. A shown pad that
reconnects under a new node becomes current again; a learn the watcher times out clears the
learning state with a message.

The page suspends the watcher while the section has the focus, so a back button pressed while
looking at its row fires nothing; a press lights the button on the art and nothing else, since
the SDL mapper is still turning the pad into keys. The live view (`setTesting(true)`, from the
test row) is where presses are looked at: the art takes the page, the mapper is muted, the watcher
streams the sticks and triggers (`axes {on}` → `axis {id, axis, value}`, `lx ly rx ry` as -1..1 and
`lt rt` as 0..1), and the last press is named under the pad. It ends on a Circle/B held for a
second or Start and Select together (both read from the watcher's `button` events), on Escape,
or by itself when the section is left, the shown pad goes, another is picked, or the watcher
stops or turns passive; `Api` mutes `api.pad` for exactly as long as `testing` is true.

The art is `ui/ControllerArt.qml` around `ui/PadArt.qml`: `ui/PadGeometry.js` holds each family's
body (an SVG path) and buttons on a 1000 × 700 sheet, one canvas draws the body and a `PadButton`
sits on every button — lit for the focused row, white on a press, pulsing while learned, dashed
without a code, a stick leaning with its axes, a trigger filling with its pull. Two bodies serve
the families: Sony's (DualSense Edge, DualSense, DualShock 4, 8BitDo Pro 3) and Xbox's (Elite,
Xbox, Switch Pro, generic), each family adding its own extras.

With `--fake` a `FakeWatcher` stands in: one DualSense Edge with every button bound, or the pad
named by `UNIVERSE_FAKE_PAD` (a family id, or `none` for the empty state), with the slots listed in
`UNIVERSE_FAKE_UNBOUND` left without a button.

## Running and testing the shipped host

```sh
nix run .#universe-ui                  # fullscreen; add --windowed
just ui                                # against .dev/
just ui-fake                           # against fixtures, no core
just seed [id…]                        # real games in .dev/: journal and media copied, recordings read in place
```

Options: `--windowed`, `--size WxH` (1920x1080, implies `--windowed`), `--no-gamepad`, `--fake`,
`--fake-launch`, `--screenshot PATH --after MS`, `--quit-after MS`, and `--keys "Right Right Return
Wait I"` with `--key-gap MS` / `--key-delay MS`. Key names are `A B X Y LB RB LT RT Start Up Down
Left Right Return Esc`; `Wait` pauses, `Wait:N` pauses N times, `Hold:A` / `Release:A` split a
press, `Stick:rightX=0.6` tilts a stick, `Shot:path.png` grabs the window; with `--fake`,
`Press:slot` / `Unpress:slot` and `Axis:lx=0.6` play the fake pad through the watcher.

`just test` runs the suite. `conftest.py` forces `QT_QPA_PLATFORM=offscreen` and redirects
`XDG_{STATE,CACHE,DATA}_HOME` to a temporary directory, so tests never touch real user state.
