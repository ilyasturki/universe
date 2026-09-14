# Writing a frontend

The core is a library, not a service: a frontend **opens it in its own process**. There is no IPC to
speak, no schema to negotiate, and no second implementation to keep in step — a frontend either
links the Rust crate, or imports `universe_core` (the PyO3 module) and calls the same methods the
CLI calls. [`api.md`](api.md) is the full surface.

The shipped frontend, `universe-ui`, lives in `ui/`: a PySide6 host around two QML looks, the
[Reprise](https://github.com/ilyasturki/pegasus-theme-reprise) theme and the Switch 2 HOME menu
(`qml/switch2/`, see its README). What follows is its
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
    screens/                data for the added screens: settings.py, sources.py, media.py, paths.py, controller.py, runners.py, artwork.py
    fixtures/               library.json and generated artwork, for --fake
    themes.py               the looks the host can load and the one on screen (api.theme)
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
| `api.theme` | the looks: `themes` (`id`, `name`, `entry`, `ground`, `detail`), `current`, `set(id)`, `fontPath` |

A `Game` exposes `id`, `title`, `sortTitle`, `favorite` (writable), `hidden`, `playTime`,
`playCount`, `lastPlayed`, `releaseYear`, `developerList`, `publisherList`, `genreList`, `players`,
`description`, `summary`, `source`, `platform`, `runner`, `runnerName`, `tags`, `extra`, `raw`,
`collections`, and `assets` (`boxFront`, `square`, `banner`, `background`, `logo`, `screenshotList`; `tile` is `square` under Pegasus's name), plus
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

## Themes

`main.qml` is a window with one `Loader` whose source is `api.theme.entry`, so a theme is a root
QML file under `qml/` and switching one for another rebuilds the tree in place: no restart, the
navigation comes back at the home screen. The choice lives in `ui-memory.json` (`theme`; the ids
of the former white and black variants of `switch2` still resolve to it), `--theme ID` overrides it
for one run, and both looks offer it in Settings › Themes. A theme calls the same `api` and the same `api.screens` objects;
`api.screens.album` and `api.screens.news` are the recordings and journal lists across every
game (`loadAll()`), which the Switch 2 look shows as its Album and News.

## Changes

The core pushes nothing — the files are the truth, and anything may write them: the CLI, systemd's
`session-end`, a hook. A frontend therefore derives its own change notifications. `CoreClient` does
it this way, and any frontend needs the equivalent:

| Signal | Derived from |
|---|---|
| `sessionStarted` | a successful `launch` |
| `sessionShown` | `(session_id, ok)`: the game's window is on screen and has the focus — `wait_session_window` on a host thread, up to 60 s. `ok` false when nobody can tell: no GNOME, no shell extension, or the session ended first |
| `sessionEnded` | the current-session marker going empty — the `state/` watch sees `session-end` remove it (debounced 300 ms), a 2 s poll stands behind it, since the game is a systemd unit, not a child. `currentSessionChanged` fires first; the pinned tile and the badge follow that property, and only the toast, the stats refresh and a pending launch follow the signal |
| `libraryChanged`, `mediaChanged`, `entryWritten`, `recordingFiled` | a `QFileSystemWatcher` on `games/`, `games/<id>/{,journal,media}`, `state/` and the overrides directory with its `<id>/` subdirectories (a pick made from the CLI shows up), debounced 300 ms; `mediaChanged` also follows a pick or its removal made through the client |
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

The game runs inside gamescope (`launch.gamescope`, see `docs/api.md`): one window, black until
the game draws, whatever Proton, umu or the emulator put up first. The launcher never lowers,
raises or hides itself — Mutter owns stacking and focus on Wayland — it times the handover on that
window and asks the shell extension to focus it.

`launchGame` raises `ui/LaunchOverlay.qml` over the page: the poster (`ui/LaunchFrame.qml`) fades
in over `Theme.durLaunch` as the page fades out, holds 450 ms, dips its art to plain ground over
300 ms and calls `launch()`. The ground then holds (`waiting`) until `sessionShown` says the
game's window is up and focused — the compositor's animation between the two is black on black —
and the poster fades out under the game. `sessionShown` with `ok` false (no GNOME, no extension)
holds 1500 ms instead; a session that ends before its window, or `launchFailed`, ends the poster
at once (a toast for the failure). Every key is swallowed while it runs.

From there the launcher is home again, with the game pinned first on the rail (`RecentGames.
playingId`, played before or not) under a PLAYING mark, its hero pill reading "Resume", and the
tab bar's badge "<title> · m:ss" on every tab; the badge is a chrome slot past the glass: A
resumes, Start opens the game menu. A resumes on the pinned game wherever it is (`focusSession()`:
the extension's `Activate` on the game's window), the game menu offers "Resume" and "Quit
<title>" for it. Play on another game asks "Quit X and start Y?" (`ui/ConfirmDialog.qml`); yes
stops the session, and `sessionEnded` starts the pending launch. Nothing on the pad brings the
launcher back over a running game: Alt-Tab, or the game's own exit, does — its window closes,
Mutter focuses what was under it. A session already running when the host starts (`CoreClient`
tracks the marker at construction), or one the CLI started (the `state/` watch), is the same
state: home, pinned, badge.

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
  `OpacityMask`, `FastBlur`, `ColorOverlay` and `ShaderEffect` render nothing. The affected components
  test `GraphicsInfo.api === GraphicsInfo.Software` and degrade — square corners, no blur, untinted
  icons, a still focus ring instead of the Switch 2 look's shader. On screen nothing changes.
- **Keys posted while a game holds focus are dropped** by Qt: there is no active item, so
  `--keys` scripting cannot drive the host behind a running game, and neither can the gamepad
  (`focusWindow()` is null). That is the wanted behaviour: home is operable once the game has
  handed the focus back (Alt-Tab, or its own exit, after which `focusLauncher()` asks for it).
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
(`ButtonGlyph`: `A`, `LB RB`, `Start+Select`, `dpad` — a space lists buttons that each do the
thing, a `+` joins a chord and is drawn between them) and draws them the way the connected pad
prints them (`PadGlyph`, named by `PadNames.js`: × ○ △ □ and L1/R1 on a DualSense, B/A and L/R on
a Switch pad); with no pad the watcher sees, they stay Xbox letters. The glyphs are vector paths
(`ui/Prompts.js`, Kenney's Input Prompts and the PlayStation mark from Simple Icons, both CC0)
drawn by a `Shape` on a 64-unit sheet — `o` the outline, `f` the filled disc with the mark cut
out, which the Switch 2 look lays over the outline for its discs — so one component serves a
14 px hint and a 48 px caption; a button no set draws (a paddle, a star) keeps its canvas
outline and letters.

## The hint bar

Every page and overlay exposes `hints`, `[{ glyph, label, dim }]`, and the bar on screen shows
the one with the focus (`theme.qml` picks the menu's, the tab bar's or the page's). `ui/Hints.js`
lays them out the same way everywhere: the page's hints on the left in one fixed order of
buttons (A, X, Y, sticks, d-pad, Start, B), the near-global ones — `LT RT` (the section or
collection cycled by the triggers) and `LB RB` (the tabs) — on the right before the clock, so
they never move as the labels around them change. A hint whose action the page has nothing for
right now (no journal entry for this recording, no refresh in this section) is kept in place and
dimmed (`dim: true`), never dropped; hints for what the pad makes obvious — moving with the d-pad
— are not written, only a d-pad with a specific meaning is (`Seek 10 s`, `Previous / next`).

## The Settings tab

`pages/SettingsPage.qml` is a sidebar (`ui/SectionList.qml`: Runners, Modules, Install, Updates,
Login, Controller, Themes, Doctor, Artwork, Quit) beside one column of `ui/SettingsCards.qml`
(`columns: 1`; the game settings page keeps two). Quit is one row, confirmed in place (`Stay` /
`Quit Universe`, which says when the running game closes with it), then `Qt.quit()` — the host
stops the session and shuts the core down after the loop. Up and Down in the sidebar switch the section as they go, Right or A
enter the cards, Left or B come back, L2/R2 cycle the section from anywhere, and □ refreshes the
sections that fetch (Install, Updates, Login, Doctor). A source's search is the first row of its
first card, reached by going up: the cursor lands on the first game (`SettingsCards.reset` skips
`type: "search"` rows). The tab bar's search glass shows only on the tabs that list games
(`TabBar.showSearch`), so Settings has none. A row's `icon` names a `MenuGlyph` kind; a pad
button's row prints its `press` and `hold` macros as chips (`PRESS`/`HOLD`, the action's glyph
from `ui/Macros.js`, the macro's `label`). A source's game row shows the library's art when the
game is in it — the `square` slot, else the banner, else the cover — and the thumbnail takes the
art's shape, square or 2:3; the home rail's tiles pick their art the same way.

## Detail, recordings and journal

The detail page's hero holds Play, the heart and, when the game has any, a Recordings and a
Journal pill (`recordingsRequested` / `journalRequested`, the shell's `openSub`); the counts
follow `recordingFiled` and `entryWritten`. Start is the game's menu, with the same two entries.

`pages/RecordingsPage.qml` plays in a pane beside the list; □ (X) toggles it fullscreen — the
pane fills the page, the hint bar rides the controls' auto-hide, ○ leaves fullscreen first, then
the video, then the page. Start on a row is an `ActionMenu` (Play, Journal entry, Remove
recording…); Remove asks in place — Keep it, Trash the recording, or trash it and its journal
entry when it has one — and calls `api.screens.recordings.remove(gameId, session)`, which goes
through `remove_recording`, drops the row's cached frames and reloads the list on the signal.
`pages/JournalPage.qml` has the same menu (Read, Recording, Remove entry… — Cancel the writing…
on a pending row) through `api.screens.journal.remove`, which cancels a pending entry's writer
before trashing; the offer to take the recording along works the other way round.

## The runners section

`api.screens.runners` is the Runners section of the Settings tab: one row per runner, its logo
(`assets/runners/<id>.svg|png`, `logo(id)` says which exists), its name and how many library
games run through it (`effective.runner`), sorted by that count, then by those games' hours, then
by name; the runners whose program was not found come last, in a dimmed "Not found" card.
`indexOf(id)` finds a runner's row. A runner's row opens `pages/RunnerSettingsPage.qml` over the
tab (`theme.qml` `openRunner`, the same loader as the game's sub pages), on `api.screens.runner`:
`load(id)` builds its head (`info`: name, platforms and where its program was found, a warning)
and cards for the program (`exe`, a path; inherited when detected) and arguments, each option by
its type, and an "Add a game…" action. `setValue(index, value)` writes through `Runners1.Set`; on
the add row it keeps the picked file and `pendingTitle()` proposes a title from it, which
`addGame(title)` sends to `Library1.Add`. Back on the tab, the list reloads and the cursor finds
the runner again. The Switch 2 look has the same list as System Settings › Runners and the same
page as `switch2/pages/RunnerPage.qml`, pushed on its stack. The game settings page's Launch group follows the runner: a Runner picker (names
shown, ids written), then the rows the runner takes. The detail page shows the runner's logo next
to the platform.

## The artwork page and section

`api.screens.artwork` is one game's artwork page (Reprise: the Artwork entry of a game's menu, or a
tile of the overview). `load(id)` reads `Media1.Status` into `slots` — one row per slot with `url`
(what shows), `defaultUrl` and `overrideUrl` (the two layers, see `docs/api.md`), `kind`
(`picked`, `default`, `missing`) and `kindLabel`, `originLabel` and `defaultOriginLabel`,
`hasOverride`, `hasDefault`, `aspect`, `use` (where the themes show the slot) — and `entry`, the
SteamGridDB entry the candidates come from ("Name (year)"), which heads the candidates so a wrong
match is seen. `loadCandidates(slot)` fetches `Media1.Candidates` off the UI thread into `candidates`
(`url` is the provider's, `thumb` what the grid shows, `votes`), `more` and `candidatesBusy`;
`moreCandidates()` takes the next page. `apply(slot, url)` runs `Media1.SetUrl` on a thread and
emits `mediaChanged` for the game once the pick landed, `removeOverride(slot)` runs `Media1.Unset`;
both report through `message`. The wrong-match flow is `search(query)` → `hits` (`name`, `year`,
`verified`, `current`) → `pin(id)`, which writes `metadata.sgdb_id` through `Media1.Pin` and reloads
the candidates. Local URLs carry the file's mtime as a query (`models.file_url`), so a pick that
replaces a file at the same path repaints instead of showing the image cache's copy.

`api.screens.artworkOverview` is the Artwork section of the Settings tab (`ArtworkOverview.qml`, a
column of its own next to the sidebar): `slot` and `filter` (`all`, `missing`, `picked`, `default`)
pick what `tiles` holds (`id`, `title`, `url`, `kind`), `counts` says how many games stand in each
state for the slot, `slotUse` where the slot shows, `refreshAll()` fetches the missing art of every
game (the section's X, a button top right). `load()` reads
`Media1.Status` for the whole library on a thread; a `mediaChanged` or `libraryChanged` reloads it
after a short debounce while the section is on screen.

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
with its `slot` and `family`, so the row draws the button's glyph, and its `press` and `hold`
macros, each carrying its `label` — the extras (back buttons, Fn) first, then the standard
buttons) and `bind`, `unbind`, `learn`, `cancelLearn`, `setTesting`,
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
looking at its row fires nothing, and nothing shows either: the SDL mapper is still turning the
pad into keys, and the rows are for binding. The live view (`setTesting(true)`, from the test
row) is where presses are looked at: the pad takes the column, the mapper is muted, the watcher
streams the sticks and triggers (`axes {on}` → `axis {id, axis, value}`, `lx ly rx ry` as -1..1 and
`lt rt` as 0..1), and the last thing touched is named under the pad — a trigger as soon as it
moves, with its pull in percent. It ends on a Circle/B held for a second or Start and Select
together (both read from the watcher's `button` events), on Escape, or by itself when the section
is left, the shown pad goes, another is picked, or the watcher stops or turns passive; `Api` mutes
`api.pad` for exactly as long as `testing` is true.

The art is `ui/ControllerArt.qml` around `ui/PadArt.qml`: `ui/PadGeometry.js` holds each family's
body (an SVG path) and buttons on a 1000 × 700 sheet, one canvas draws the body and a `PadButton`
sits on every button — white on a press, a stick leaning with its axes, a trigger filling from the
bottom with its pull (a press past the half only brightens its outline, so the gauge reads all the
way down; dashed when the slot has no code on this connection — `PadArt` also takes
`focusedSlot` and `learningSlot`, which the live view leaves empty). While a button is being
learned, its row says so in place of its chips. The caption under the pad keeps its height from
the start, so the first press does
not resize the pad, and spells both ways out with the pad's own glyphs. Two bodies serve the
families: Sony's (DualSense Edge, DualSense, DualShock 4, 8BitDo Pro 3) and Xbox's (Elite, Xbox,
Switch Pro, generic), each family adding its own extras.

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
