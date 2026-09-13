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
    screens/                data for the added screens: settings.py, sources.py, media.py, paths.py, controller.py
    fixtures/               library.json and generated artwork, for --fake
    qml/                    the ported theme plus the added screens; assets/controllers/ holds the pad art
  tools/controller-art.py   draws the pad SVGs and the hotspot table from one geometry
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
| `api.universe` | the client: every core call, plus the signals below |
| `api.pad` | `rightX` / `rightY`: the right stick as a value, 0 without a controller |
| `api.screens` | data for the added screens (settings, sources, media, the folder picker, the controller) |
| `api.fullscreen` | whether the host runs fullscreen (the default; `--windowed`, `--size` and `--screenshot` turn it off) |

A `Game` exposes `id`, `title`, `sortTitle`, `favorite` (writable), `hidden`, `playTime`,
`playCount`, `lastPlayed`, `releaseYear`, `developerList`, `publisherList`, `genreList`, `players`,
`description`, `summary`, `source`, `platform`, `tags`, `extra`, `raw`, `collections`, and
`assets` (`boxFront`, `tile`, `background`, `logo`, `screenshotList`), plus `launch()`.

`api.screens.recordings` samples 16 frames per recording with ffmpeg into
`$XDG_CACHE_HOME/universe/frames/<sha1 of the path>/NN.jpg` (`stats.json` keeps the probed duration and
each frame's 8×8 gray stddev, so the list thumbnail is the first frame that is not black or a fade).
Two extractions run at a time; the picked row's frames go first, the others' thumbnails after.
`frameMap[session]` carries `thumbnail`, `frames` (`""` until extracted), `complete` and `duration`.

## Changes

The core pushes nothing — the files are the truth, and anything may write them: the CLI, systemd's
`session-end`, a hook. A frontend therefore derives its own change notifications. `CoreClient` does
it this way, and any frontend needs the equivalent:

| Signal | Derived from |
|---|---|
| `sessionStarted` | a successful `launch` |
| `sessionEnded` | the current-session marker going empty — polled every 2 s while a session is tracked, since systemd owns the game |
| `libraryChanged`, `mediaChanged`, `entryWritten`, `recordingFiled` | a `QFileSystemWatcher` on `games/`, `games/<id>/{,journal,media}` and `state/`, debounced 300 ms |
| `progress`, `jobFinished` | the job's own callback — install, update, scan and media refresh run on a host thread |
| `launched`, `launchFailed`, `error` | the call's result |

Two consequences worth knowing before you design around them. **`launch` blocks on the pre-launch
hooks** — up to the summed `timeout_s` of every blocking hook, 20 s by default — so it must not run
on the UI thread if you want the launch animation to keep moving. And **long jobs die with the
process**: closing the frontend mid-install interrupts it, by design.

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
  (`focusWindow()` is null). That is the wanted behaviour until there is an overlay.
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

## The controller section

`api.screens.controller` is the Controller section of the Settings tab. The host starts
`universe controller watch --json --wait` as a child for its lifetime (`$UNIVERSE_BIN`, else
`universe` on `PATH`) and reads its event lines: `device`, `gone`, `button`, `unknown`, `macro`,
`learned`, `waiting`, `ready`; it writes `suspend`, `resume`, `reload`, `learn` and `cancel`
commands on its stdin. The screen exposes `devices`, `current`, `family` (the current pad's, else
the last one seen, kept in `api.memory` as `controllerFamily`), `connected`, `status`, `learning`,
the `rows`/`groups` of one card (a Controller picker row when two pads are connected, then a row
per button of the family, the extras — paddles, Fn — first, then the standard buttons) and `bind`, `unbind`, `learn`,
`cancelLearn`, `suspend`, `resume`. Macros and families come from the core's `Controller1.State`;
a write goes through `Controller1.Bind`/`Unbind` and is followed by a `reload` to the watcher.

The page suspends the watcher while the section has the focus, so a paddle pressed to find its row
fires nothing; a press moves the cursor to the button's row and lights it on the art. The art is
`ui/ControllerArt.qml`: `assets/controllers/<family>.svg` under QML hotspots placed from
`ui/ControllerHotspots.js`, both generated by `tools/controller-art.py` so the two never drift.

With `--fake` a `FakeWatcher` stands in: one DualSense Edge with every button bound, or the pad
named by `UNIVERSE_FAKE_PAD` (a family id, or `none` for the empty state), with the slots listed in
`UNIVERSE_FAKE_UNBOUND` left without a button.

## Running and testing the shipped host

```sh
nix run .#universe-ui                  # fullscreen; add --windowed
just ui                                # against .dev/
just ui-fake                           # against fixtures, no core
```

Options: `--windowed`, `--size WxH` (1920x1080, implies `--windowed`), `--no-gamepad`, `--fake`,
`--fake-launch`, `--screenshot PATH --after MS`, `--quit-after MS`, and `--keys "Right Right Return
Wait I"` with `--key-gap MS` / `--key-delay MS`. Key names are `A B X Y LB RB LT RT Start Up Down
Left Right Return Esc`; `Wait` pauses, `Wait:N` pauses N times, `Hold:A` / `Release:A` split a
press, `Stick:rightX=0.6` tilts a stick, `Shot:path.png` grabs the window.

`just test` runs the suite. `conftest.py` forces `QT_QPA_PLATFORM=offscreen` and redirects
`XDG_{STATE,CACHE,DATA}_HOME` to a temporary directory, so tests never touch real user state.
