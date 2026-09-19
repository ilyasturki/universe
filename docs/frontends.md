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

## What QML sees

One context property, `api`:

| Member | What it is |
|---|---|
| `api.keys` | `is{Accept,Cancel,Details,Filters,PageUp,PageDown,PrevPage,NextPage,Menu}(event)` — the Pegasus key-action contract — plus `isScreenUp` / `isScreenDown` (`[` / `]`, the right stick up and down): a screenful in a long list |
| `api.allGames` | the library model |
| `api.collections` | collections, one per platform |
| `api.memory` | `get`/`set`/`has`/`unset`, persisted to `$XDG_STATE_HOME/universe/ui-memory.json` |
| `api.universe` | the client: every core call, plus the signals below. `adoptScope()` and `pendingJournals()` wrap `adopt_scope` and `pending_journals`; their failures get a log line, not a toast. `recordings(id)` is the client's own: `sessions(id)` kept to the rows with a `recording` |
| `api.pad` | `rightX`: the right stick as a value, 0 without a controller |
| `api.power` | the batteries the kernel lists under `/sys/class/power_supply`: `sources` (`kind` `system` or `pad`, `percent`, `charging`, `inputs` — the pad's evdev nodes), `count`; polled every 10 s. Both looks draw them next to every clock (`ui/PowerBadge.qml`), the controller pages next to the pad they belong to; `--fake` reads `fixtures/power_supply` |
| `api.screens` | data for the added screens (settings, sources, media, the folder picker, the controller, the journals being written) |
| `api.fullscreen` | whether the host runs fullscreen (the default; `--windowed` and `--size` turn it off) |
| `api.theme` | the looks: `themes` (`id`, `name`, `entry`, `overlay`, `frame`, `ground`, `detail`), `current`, `frame`, `set(id)`, `landing` / `takeLanding()`, `fontPath` |
| `api.home` | the HOME button over a running game (see "HOME and the dock"): `shown` (`game` / `launcher`), `open`, `paused`, `pauseOnHome`, `flipped`, `frame`, `volumePercent`, `muted`; `pressed()`, `stopping(title)`; `openDock()`, `closeDock()`, `dockClosed()`, `toGame()`, `toLauncher(landing?)` / `takeLanding()`, `covered()`, `stop()`, `setPauseOnHome(on)`, `screenshot()` (→ `screenshotTaken(path)`), `volume(change, value)`, `launchValue(key)`, `launchChoices(key)`, `setLaunchValue(key, value)`, `screenRefresh()` |

A `Game` exposes `id`, `title`, `sortTitle`, `favorite` (writable), `hidden`, `playTime`,
`playCount`, `lastPlayed`, `releaseYear`, `developerList`, `publisherList`, `genreList`, `players`,
`description`, `summary`, `source`, `platform`, `runner`, `runnerName`, `tags`, `extra`, `raw`,
`collections`, and `assets` (`boxFront`, `square`, `banner`, `background`, `logo`, `screenshotList`; `tile` is `square` under Pegasus's name), plus
`launch()`.

`api.screens.recordings` maps the session rows that carry a `recording` (`sessions(id)`, see
`api.md`) to rows — `session`, `path`, `url`, `size`, `sizeText`, `duration_s`, `durationText`,
`dateText`, `created_at`, `hasJournal`, `gameId`, `gameTitle` — and samples 16 frames per recording
with ffmpeg into `$XDG_CACHE_HOME/universe/frames/<sha1 of the path>/NN.jpg`; the seeks are spread
over the row's `recording.duration_s` (the media's length the core probed when the file was filed),
or the session's span for a line filed before the core kept lengths. Two extractions run at a time;
the picked row's frames go first, the others' thumbnails after. `frameMap[session]` carries
`thumbnail`, `frames` (`""` until extracted), `complete` and `duration`.

`api.screens.journal` maps a game's entries to rows — `session`, `title`, `state`, `reason`,
`started_at`, `dateText`, `duration_s`, `durationText`, `paragraphs`, `blocks`, `next_up`,
`images` (`file://` URLs of the paths the core hands out), `hasRecording` (from the session rows),
`gameId`, `gameTitle` — sorted by session id, last first. `state` is `written`, `pending` (the
module is still writing: no title, no paragraphs; the row pulses with the time since `started_at`
and cannot be opened) or `failed` (`reason` is the module's message, its one paragraph).
`durationText` is the session's length — `42 min`, `1 h 05` — next to the date in the row and in
the article header; the date is `written_at`, or `started_at` while there is none.
`api.screens.shots` maps the player's own screenshots to rows — `name`, `path`, `url`,
`taken_at`, `dateText`, `session`, `hasJournal` (a written or pending entry covers the session),
`gameId`, `gameTitle` — newest first, from `screenshots(id)` (`load(id)`) or `screenshots("")`
(`loadAll()`), reloaded on `libraryChanged` for the game it holds; `remove(gameId, name)` is
`remove_screenshot`. `api.screens.media` is every visible game's shots, recordings and written
journal entries as one list (`load()`, `rows`, `count`), newest first by `when` — `kind` is
`shot`, `recording` or `journal`; `image` the shot, the recording's thumbnail (warmed through
`api.screens.recordings` so the two share one cache) or the entry's first picture; `title` the
recording's length or the entry's title; `gameId`, `gameTitle`, `dateText`, `session`, `name`
(a shot's file name), `path`, `hasJournal`. It follows `libraryChanged`, `recordingFiled`,
`entryWritten` and the recordings' `framesChanged` while loaded (`unload()` stops that).
`api.screens.pendingJournals` is `pending_journals()` as `rows` and `count`, refreshed on
`entryWritten`, on `sessionEnded` and every 10 s while any is pending (so the elapsed time and
the module's 30-min timeout show up); `appeared(session, title)` and
`resolved(session, game, state, text)` fire once per session and become the "Journal: writing …",
"Journal: <title>" and "Journal failed: <reason>" toasts, and the tab bar pulses a book next to
the session badge while the count is not zero.

## Themes

`main.qml` is a window with one `Loader` whose source is `api.theme.entry`, so a theme is a root
QML file under `qml/` and switching one for another rebuilds the tree in place: no restart, and
the new look opens on its own Settings › Themes (`set(id)` leaves `landing = "themes"`, which the
theme reads and `takeLanding()` clears). `overlay.qml` is the second window, the one gamescope
paints over the game; its `Loader` takes `api.theme.overlay` — Reprise's `ui/Dock.qml`, nothing for
the Switch 2 look, whose HOME goes straight to its HOME menu. The choice lives in `ui-memory.json` (`theme`; the ids
of the former white and black variants of `switch2` still resolve to it), `--theme ID` overrides it
for one run, and both looks offer it in Settings › Themes. A theme calls the same `api` and the same `api.screens` objects;
`api.screens.album` and `api.screens.news` are the recordings and journal lists across every
visible game (`loadAll()`, over `sessions("")`), which the Switch 2 look shows as its Album and News;
the Album lays `api.screens.shots` (`loadAll()`) on the same grid, newest first, a Show pick
narrowing it to screenshots or videos, A on a shot opening it full-screen (◀ ▶ step between shots).

## Changes

The core pushes nothing — the files are the truth, and anything may write them: the CLI, systemd's
`session-end`, a hook. A frontend therefore derives its own change notifications. `CoreClient`
(`ui/universe_ui/universe_client.py`) is one client over one core object — `universe_core.Core`
in production, `FakeCore` (`fake_core.py`, the same methods over `fixtures/library.json`, laying
out `games/`, `state/` and the overrides directory under a temporary root and writing them the way
the core does) under `--fake` and in the tests — and its slots call the core's methods directly.
What it derives is derived this way, and any frontend needs the equivalent:

| Signal | Derived from |
|---|---|
| `sessionStarted` | a successful `launch` |
| `sessionShown` | `(session_id, ok)`: the game's window is on screen and has the focus — `wait_session_window` on a host thread, up to 60 s. Inside gamescope `ok` is true once gamescope shows the game's window (a stand-in toplevel carrying the gamescope's pid when no extension lists it); on the desktop, false when nobody can tell: no GNOME, no shell extension, or the session ended first |
| `sessionEnded` | the current-session marker going empty — the `state/` watch sees `session-end` remove it (debounced 300 ms), a 2 s poll stands behind it, since the game is a systemd unit, not a child. `currentSessionChanged` fires first; the pinned tile and the badge follow that property, and only the toast, the stats refresh and a pending launch follow the signal |
| `libraryChanged`, `mediaChanged`, `entryWritten`, `recordingFiled` | a `QFileSystemWatcher` on `games/`, `games/<id>/{,journal,journal/attachments,media,screenshots}`, `state/` and the overrides directory with its `<id>/` subdirectories (a pick made from the CLI shows up), debounced 300 ms; `mediaChanged` also follows a pick or its removal made through the client |
| `progress`, `jobFinished` | the job's own callback — install, update, scan and media refresh run on a host thread |
| `launched`, `launchFailed`, `error` | the call's result |

A frontend decides who owns the game's lifetime. `adopt_scope()`, called once at startup, moves the
frontend into `universe-launcher-<pid>.scope`; every game it launches from then on is bound to that
scope and goes down with the frontend, `session-end` included. `host.py` calls `adoptScope()` once
the client exists — not with `--fake` — so closing `universe-ui` closes the
game; a frontend that never calls it leaves the game to systemd, as `universe play --no-wait` and
the hooks do. The host also quits cleanly on SIGINT and SIGTERM (a wakeup-fd `QSocketNotifier`
lets the Python handlers run under the Qt loop) and, with a session running, calls `stop("")`
before `api.shutdown()`, so `session-end` has run before the scope goes. No confirmation is asked.

Two consequences worth knowing before you design around them. **`launch` blocks on the pre-launch
hooks** — up to the summed `timeout_s` of every blocking hook, 20 s by default — so it must not run
on the UI thread if you want the launch animation to keep moving. And **long jobs die with the
process**: closing the frontend mid-install interrupts it, by design.

## Launch and the running view

The launcher is gamescope's base app (`docs/api.md` § Gamescope): fullscreen, `host.py` starts
`gamescope` around itself (`client.hostGamescope("")`, `os.execv`) unless it is already inside one
(`GAMESCOPE_WAYLAND_DISPLAY`), forces the `xcb` platform there, and every game lands on that
gamescope, which shows the most recently mapped window — the game's, once it has one. The launcher
never lowers, raises or hides itself; `api.home` flips which window gamescope shows (`toGame`,
`toLauncher`: `focus_session` / `focus_pid`), and `Api.screenName()` is `""` inside gamescope, where
the window's screen is the Xwayland's and not a connector. `--windowed` and `--size` skip gamescope
and keep the desktop path below, where the game gets a gamescope of its own and the extension
focuses windows.

`launchGame` raises `ui/LaunchOverlay.qml` over the page: the poster (`ui/LaunchFrame.qml`) fades
in over `Theme.durLaunch` as the page fades out, then grabs itself at the screen's pixel size
(`grabToImage`, `Game.launchWith(result)`: the QImage is written off the UI thread in `universe
splash`'s format under `$XDG_RUNTIME_DIR/universe/` and its path goes with `launch`; a grab that
fails launches without one). The poster — art, logo and title — then holds (`waiting`) until
`sessionShown` says the game's window is up and focused: gamescope's window maps over it within
about a second, showing that same grab from its keep-alive window (`docs/api.md` § Gamescope) until
the game's own window, so the handover is poster over poster; the launcher's poster fades out under
it. `sessionShown` with `ok` false (on the desktop: no GNOME,
no extension) holds 1500 ms instead; a session that ends before its window, or `launchFailed`,
ends the poster at once (a toast for the failure). Every key is swallowed while it runs.

From there the launcher is home again, with the game pinned first on the rail (`RecentGames.
playingId`, played before or not) under a PLAYING mark (PAUSED while frozen), its art the last
frame gamescope painted when HOME brought the launcher up (`api.home.frame`, `nest_frame`), its
hero pill reading "Resume", and the tab bar's badge "<title> · m:ss" on every tab; the badge is a
chrome slot past the glass: A resumes, Start opens the game menu. A resumes on the pinned game
wherever it is (`api.home.toGame()`), the game menu offers "Resume" and "Quit <title>" for it.
Play on another game asks "Quit X and start Y?" (`ui/ConfirmDialog.qml`); yes stops the session,
and `sessionEnded` starts the pending launch. Every quit goes through `api.home.stop()`: the
launcher comes up first (a flip, when the game is on screen), the unit is stopped once it has,
`stopping(title)` is the theme's cue for a "Quitting…" toast, and nothing freezes a game that is
on its way out. The game's own exit brings the launcher back by
itself (gamescope shows what is left). A session already running when the host starts
(`CoreClient` tracks the marker at construction), or one the CLI started (the `state/` watch), is
the same state: home, pinned, badge.

## HOME and the dock

The Guide button is HOME. It comes from the evdev watcher (`button` events with slot `guide`),
never from the SDL mapper — the game holds the focus, so no key would reach the launcher — and
`api.home` turns it into `pressed()`. The button is the launcher's alone: no macro binds on the
`guide` slot (the core refuses one, `macros_for` ignores an old config's, the Controller section
offers only Learn on its row), and a hold of `controller.hold_ms` from the game goes home
(`toLauncher`) whatever the press opened. What a press does is the theme's: the Switch 2 look
flips to its HOME menu over the game (`toLauncher`) and back (`toGame`); Reprise opens its
**dock** over the live game (`openDock`), a second press or B closes it, and from home a press
resumes. With no session, Reprise treats it as Start (the game menu).

The dock is `ui/Dock.qml` in the overlay window: the game's card at the left, a row of round
buttons at the right (`row` in `Dock.qml`), a group's settings in a card above its button. ◀ ▶ move
along the row or change the focused value, ▲ ▼ the rows of a card, A acts, flips or opens, B closes
the card or the dock, X takes a screenshot with the band faded out so the shell grabs the game alone.
▼ from the row raises `ui/DockShots.qml` over the whole frame: the playing game's own screenshots
(`api.screens.shots`, `load(id)` on opening) on a `ui/ShotGrid.qml` — THIS SESSION first (taken
since the session's `started_at`: a running session is not in `sessions.jsonl` yet, so its shots
carry no `session`), EARLIER below — A a `Lightbox`, Y "Remove this screenshot?" through the
dock's `ConfirmDialog` (Keep it focused, Trash the screenshot → `shots.remove`), B or ▲ past the top
row lowers it onto the dock. A shot taken meanwhile lands through the screenshots watcher. The
Game card's Details, Journal and Recordings rows call `toLauncher(landing)`: the launcher comes up
as for Home, and the theme's `landHome` takes the landing (`takeLanding()`, once) and opens the
detail or the sub page on the playing game over Home, the frame fading rather than shrinking into
the tile.

The shutter is the launcher's, not the shell's, so it is the same for the dock's camera, a pad
macro and `universe screenshot`: `api.home` plays `qml/assets/sounds/shutter.wav` and emits
`screenshotTaken(path)` when the shot returns — the dock's through its own call, the pad's through
the watcher's `screenshot` event (`api.screens.controller.screenshotTaken`) — and `overlay.qml`,
outside the theme's loader, answers a non-empty path with a white flash. Inside gamescope
`api.home` lifts the overlay window to opaque for 450 ms around it when the dock is not already
holding it up; on the desktop there is no window over the game, so only the shutter is heard (the
hook answers before either, see api.md § Screenshots). A failed shot emits an empty path: the dock
toasts it, the flash stays off.
Its volume row is the controller's macro by another route (`volume("up" | "down" | "mute")`,
`controller.volume_step` per step, GNOME's OSD through `desktop::show_osd` on every change but a
`get`). Quit asks, then `api.home.stop()`.

The swap between the game and the launcher is gamescope's, one cut, so the last frame it painted
bridges it: the Guide press from the game asks for it (`nest_frame`) and the flip — the hold, the
dock's Home or Quit — reuses what the press took, or waits for it to land. A look registered with
`frame: False` (the Switch 2 one) paints nothing and waits for none. `toLauncher` then sets
`frame` (`""` when none came) and `shown`; with a frame it waits for the theme's `covered()` —
Reprise's `ui/HomeFlip.qml` paints the frame full screen and calls it once the image is up — before
`focusLauncher` (`COVER_MS` later regardless), without one it swaps at once; `flipped` says the
host did it (a game that exits by itself leaves the launcher on screen too, with nothing to zoom). Reprise then lands on Home with the
cursor on the playing game (`HomePage.landOnPlaying`, every detail, sub page and search closed)
and shrinks the frame into that tile, which shows the same frame as its art; resuming grows the
tile back to full screen (`HomePage.playingTileRect`) and only then `toGame()`.

The host owns what the QML cannot: the overlay window is created once (`create_overlay`,
`Home.attachOverlay`) with `STEAM_OVERLAY=1`, mapped at opacity 0 and never unmapped — gamescope
keeps painting an unmapped overlay's last buffer; opening sets `STEAM_INPUT_FOCUS=1` and full
opacity, the fade-out done (`dockClosed()`) drops both and the game gets its input back. The
watcher's macros are suspended while the dock has the pad. Nothing takes the pad away from the
game — gamescope routes keyboard and mouse only, and the game keeps its own evdev or hidraw
readers — so `pause_on_home` (a launch key, global and per game, on by default) freezes the game
whenever the launcher covers it: as the dock opens, and as `toLauncher` flips (after the frame is
taken: a frozen game paints nothing); `dockClosed` and `toGame` thaw it — on the Guide release
when that is what closed the dock, so the game never sees Guide held. The recording pauses and
resumes with the freeze (the capture module's `freeze` / `thaw` hooks), so menu time never lands
in the file. Off, the game runs on behind the
launcher and answers every press the menu gets; turning it on from the dock while the launcher
is up freezes at once. The session ending drops the overlay whatever state it was in. Without an
overlay window (the launcher on the desktop) `openDock` is `toLauncher`, and only `toGame`
thaws: a game window raised from the desktop's own switcher stays frozen until Resume.

A frozen game reads no key, so what the dock asks of the game's MangoHud goes two ways. The
MangoHud row is a toggle of `launch.mangohud` (`setLaunchValue("mangohud", …)` →
`set_mangohud`): the core writes the key and tells the HUD itself, no key typed, so it lands
frozen or not and the row shows the state it wrote. The FPS limit row rewrites the layer's conf
and types its `reload_cfg` combo through the watcher — held back while the game is paused and
typed once the thaw has landed (`freeze(False)`'s reply), one press however many changes.

## Qt and QML notes

These cost real time to discover; they are properties of Qt 6.11 / PySide6 6.11, not of Universe.

- **`QtQml.Models.SortFilterProxyModel` cannot carry the theme's filters.** It has no `get()`, no
  `ExpressionFilter`, and its `FunctionFilter` segfaults. Every proxy the theme needs is instead a
  `QSortFilterProxyModel` subclass. `SortedGames`, `RecentGames`, `LimitedGames`,
  `FavouriteGames`, `SearchGames` and `LibraryGames` are exported to QML as `import Universe`;
  `CollectionGames` is instantiated Python-side, one per collection. `get(i)` returns the `Game`;
  `sourceRow(i)` stands in for `mapToSource(i)`, whose C++ name is virtual and breaks sorting if
  shadowed.
- **QML import paths.** The host sets none: `QML2_IMPORT_PATH` comes from the flake — the dev
  shell's hook (every `just` recipe), `wrapQtAppsHook` for the package, the pytest check — and so
  does `QT_PLUGIN_PATH` for the first two; a plugin must link the *same* qtbase as PySide6, or it is
  refused. `.venv/bin/universe-ui` from a plain shell finds no QML modules.
- **Software rendering has no shaders.** The `offscreen` QPA loads the software scenegraph, where
  `OpacityMask`, `FastBlur`, `ColorOverlay` and `ShaderEffect` render nothing. The affected components
  test `GraphicsInfo.api === GraphicsInfo.Software` and degrade — square corners, no blur, untinted
  icons, a still focus ring instead of the Switch 2 look's shader. On screen nothing changes.
- **Keys posted while a game holds focus are dropped** by Qt: there is no active item, so
  `--keys` scripting cannot drive the host behind a running game, and neither can the gamepad
  (`focusWindow()` is null). That is the wanted behaviour: home is operable once the game has
  handed the focus back (Alt-Tab, or its own exit, after which `focusLauncher()` asks for it).
- **A view's cursor is a row, not a game.** `RecentGames` pins the playing game first and the
  stats re-sort a game after its session; no view index follows, and a positional `model.get(index)`
  binding does not re-evaluate on a reorder. `GameAnchor` (`import Universe`) does: bind `model` and
  `index` to the view's and `client` to `api.universe`, read `game`, follow `moved(index)`; `hold(id)`
  picks the game to follow, and the anchor holds the one whose session just ended.
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
| Start | F1 | context menu of the game on screen, whichever part of the page has focus |
| Guide | — | HOME, through the watcher (`api.home`), not the mapper |
| d-pad, left stick | arrows | navigation |
| right stick up / down | `[` / `]` | a screenful up or down, same column, repeating like the arrows: the library and software grids, the settings rows and cards, the media grids and lists |
| right stick left / right | `api.pad.rightX` | analog, past a 0.18 deadzone: scrubs the recording player |

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
splits them: the page's hints on the left in the order the page wrote them, the near-global ones
— `LT RT` (the section or collection cycled by the triggers) and `LB RB` (the tabs, appended by
the shell) — on the right before the clock, so they never move as the labels around them change.
A hint whose action the page has nothing for
right now (no journal entry for this recording, no refresh in this section) is kept in place and
dimmed (`dim: true`), never dropped; hints for what the pad makes obvious — moving with the d-pad
— are not written, only a d-pad with a specific meaning is (`Seek 10 s`, `Previous / next`).

## The Settings tab

`pages/SettingsPage.qml` is a sidebar (`ui/SectionList.qml`: Runners, Launch, Modules, Sources, Install,
Updates, Controller, Themes, Doctor, Artwork, Quit) beside one column of `ui/SettingsCards.qml`
(`columns: 1`; the game settings page keeps two). Quit is one row, confirmed in place (`Stay` /
`Quit Universe`, which says when the running game closes with it), then `Qt.quit()` — the host
stops the session and shuts the core down after the loop. Up and Down in the sidebar switch the section as they go, Right or A
enter the cards, Left or B come back, L2/R2 cycle the section from anywhere, and □ refreshes the
sections that fetch (Install, Updates, Doctor). A source's search is the first row of its
first card, reached by going up: the cursor lands on the first game (`SettingsCards.reset` skips
`type: "search"` rows). The tab bar's search glass shows only on the tabs that list games
(`TabBar.showSearch`), so Settings has none. A row's `icon` names a `MenuGlyph` kind; a pad
button's row prints its `press` and `hold` macros as chips (`PRESS`/`HOLD`, the action's glyph
from `ui/Macros.js`, the macro's `label`). A source's game row shows the library's art when the
game is in it — the `square` slot, else the banner, else the cover — and the thumbnail takes the
art's shape, square or 2:3; the home rail's tiles pick their art the same way.

## The Install page

`api.screens.sources` (`SourcesBrowser`) lists one source's games: `rows` of `{id, title, game_id,
image, installed, pending, partial, busy, status, action, disk_size, download_size, partial_bytes,
size, sizeText, sizeKind}`, installed first. `load()` (opening the page) serves the core's cache and the
disk, re-listed once 15 min old, never the store; `refresh()` (Y) asks the store, so a game
bought since shows up — `libraryAt` and `libraryAge` say when that last happened, for the header.
A refresh the store refuses keeps the listing shown, toasts the reason and leaves it in `error`
until a reload succeeds. `freeSpace` is the install folder's free bytes (`games_dir`; 0 when it
is missing), for the confirm and a disk strip.
Sizes: an install's is measured (`sizeKind` `disk`); a game not installed shows its download size
once known (`download`), and `peek(index)` — call it as the cursor lands on a row — fetches one
`info` at a time, off the busy flag, and the core remembers it. `install(index)` installs, resumes
a stopped download (the same call: the source continues over its folder) or updates; while it runs
`job` is `{id, game, title, label, message, done, total, ok, cancelled}`, `done`/`total` in bytes and
`message` the live line ("Installing X · 42% · 3.4 GB of 8.2 GB"); the rows are rebuilt at its start
and end only, the busy row's `status` "Installing…" and its `action` `Cancel`. `cancel()` stops it: the job ends
failed with `cancelled`, the toast says what was kept, and the row turns `partial` — `status`
"Paused · X of Y kept", `action` `Resume`. Reloads after a job, an uninstall or a removal stay off
the network.

Reprise keeps it as Settings › Install: an Installing card (running and paused, a hairline of
progress under a paused row) over Installed and Owned, every game with its size in figures of one
width, the library's age or the store's failure in the card meta; A opens the row's menu (Cancel
install · Resume · Update · Game settings · Uninstall… · Remove…), Install first asks in a
`ConfirmDialog` with the download, the disk and the free space; X refreshes; the cursor on a game
without a size peeks it. The Switch 2 look's `pages/InstallPage.qml` is two tabs on the bumpers:
Store, the tiles of what is owned and not installed (a download or a pause painted on the tile's
foot) and the search's results, A installs after the same confirm; Manage, the install folder's
strip (used · free), then Installing and Installed as rows with a size and an action — A is the
row's menu, X cancels the running job from either tab.

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

The two pages preview each other for the session under the cursor, so a jump (Y, `jumpRequested`)
is a choice and not a guess. The journal article ends with a RECORDING card (`ui/RecordingCard.qml`)
between NEXT UP and the screenshots — ▼ from the end of the text lands on it, A opens the recordings
page on that session. Under the recordings pane a JOURNAL block shows the entry's title and first
paragraph; ▼ from the video focuses it, A reads it. Each page loads the other's store for the game
unless it already holds it, so the jump finds them warm.

The hero's Screenshots pill (`screenshotsRequested`, when `screenshots(id)` lists any; the game
menu has the same entry) opens `pages/ScreenshotsPage.qml` on `api.screens.shots`: a four-wide
16:9 grid of the player's own shots, newest first, A a `Lightbox` (◀ ▶ step, B closes), Y the
journal entry covering the shot (`jumpRequested` to the journal page on that session), Start an
`ActionMenu` — View, Journal entry, Remove screenshot… (Keep it / Trash the screenshot, through
`shots.remove`). An `args.name` lands the cursor on that file. While this game is the one playing
the grid splits at the session's start, THIS SESSION over EARLIER, as the dock's panel does. The
detail strip keeps the store's promotional shots only (`assets.screenshotList`).

## The Media tab

`pages/MediaPage.qml`, the fourth tab, is `api.screens.media` on one four-wide grid of
`ui/ShotCard.qml` (a 16:9 picture with the kind's glyph in a corner, a book when a journal entry
covers it, the game and the date below): screenshots, recordings and journal entries of every
game, newest first. Two chips above it, reached with ▲ from the top row, narrow the list — the
kind (All, Screenshots, Recordings, Journal; also cycled by `LT RT`, kept in `ui-memory.json`
as `mediaKind`) and the game (every game with something on the list). A opens the row: a shot in
the `Lightbox` (◀ ▶ step between the shots on the list), a recording on the game's recordings
page at that session (`recordingsRequested(game, session)`), an entry on its journal page
(`journalRequested(game, session)`); X is the game's details, Y the journal entry covering a shot
or a recording; Start an `ActionMenu` with those and, for a shot, All of this game's
(`screenshotsRequested(game, name)`) and Remove screenshot…. The shell opens each request as a
sub-page over the tab, so B comes back to the list where it was.

## The launch and modules sections

`api.screens.launch` is Settings › Launch, what every game starts with, each row a `config.toml`
key written through `set_setting`. The rows are the core's launch-key catalogue
(`launch_keys(scope, screen)`, `client.launchKeys`) in the catalogue's order, the keys tied to no
runner (`runners` empty): each entry's `section` is the card — Display (the gamescope switch,
resolution, refresh rate, adaptive sync; the card's meta is the screen, `screen`: `DP-1 3840×2160 @
60 Hz`), Overlay (MangoHud, the frame rate limit, pause on HOME, then `desktop.hide_cursor` added by
the screen), Advanced (scaler, filter, sharpness, the raw gamescope arguments) — beginner first,
expert last. Its `label` and `description` (the row's `detail`) come with it, and its `choices` are
sized by the screen the window is on (`screen_mode`: `auto`, the screen's mode, the standard heights
below it at its aspect ratio; the rates below its own). `screens/settings.py`'s `launch_row` is the
presentation over an entry: an `enum` or `int` with choices lists a `default` choice that clears the
key (through `choiceValues`), a `proton` entry lists the config's `[proton]` names, `fps_limit`'s
`auto` displays as `auto · 60`, the rate it stands for. The keys tied to a runner (`proton`, the
sync modes, Wayland, HDR, the upscaler upgrades) are set on that runner's page instead (below).
`load()` reads the config again.

The game settings page reads the same catalogue with scope `game`, filtered by the runner's kind
(`runners`), and mirrors the cards — Display, Overlay, Advanced — then the runner's own, named
after it (Proton: the build, Wayland, HDR, the Wine prefix; Sync; Upscaling), then Launch (the
runner picker, the program, an emulator's options, the wrapper, arguments and working directory);
a `both` key inherited from the global value until set.

Upscaling's meta names the GPU (`client.gpu()`: `label`, `AMD Radeon RX 7900 GRE · RDNA 3`) and each
of its rows ends its `detail` with `Works on your GPU.` or `Not for your GPU.` (`fits`), nothing when
no GPU is known. In Reprise, `ui/SettingsCards.qml` shows the focused row's `detail` as a caption
under the cards (two lines, reserved whenever a row has one); the Switch 2 look prints it under
every row.

`api.screens.modules` is Settings › Modules and `api.screens.sourceList` Settings › Sources, the
same list (`screens/settings.py`'s `ListForm`): one `action` row per entry (`module` its id, its
name, `value` whether it runs, `switch: true` so the row draws its state as a switch ahead of the
chevron, `display` On / Off / Unavailable, `meta` the version, `warning` what is missing, `source`
which list it is), the ones running first, the others in an "Off" card. A opens the entry's page;
△ (Y in Reprise, X in the Switch 2 look) toggles it in the list (`toggle(index)`, refused with a
warning while it is off). `indexOf(id)` finds an entry's row for the cursor to land on again.
`pages/FormPage.qml` with `{ module }` or `{ source }` (`theme.qml` `openSub`;
`switch2/pages/FormPage.qml` on the stack) is on `api.screens.module` or `api.screens.source`
(`PageForm`): `load(id)` builds its head (`info`: name, meta, `description` from the manifest, warning,
`enabled`, `source`, and a source's `logged_in` and `user`) and cards for the switch (`enabled`, `disabled` while the entry's
programs are missing) and, once on, its settings — a module's global ones, a source's all — a
`dynamic` setting's choices fetched off the UI thread. A source's page adds a Sign-in card: `Signed
in` (an `info` row, the user as its detail), `Get a sign-in link` (`link`: `api.screens.login.begin`,
the QR code and the URL then show under the cards) and `Enter the code` (`code`: a prompt into
`login.submit`); `login.finished` reloads both source screens. Listing the sources probes their
logins once per process, on the network, so the sources list and page load off the UI thread and
announce `rowsChanged` when they land. Doctor's checks stay on `api.screens.modules`
(`loadDoctor`, `doctor`, `doctorGroups`), grouped by module or source name and run off the UI thread too.

## The runners section

`api.screens.runners` is the Runners section of the Settings tab: one row per runner, its logo
(`assets/runners/<id>.svg|png`, `logo(id)` says which exists), its name and how many library
games run through it (`effective.runner`), sorted by that count, then by those games' hours, then
by name; the runners whose program was not found come last, in a dimmed "Not found" card.
`indexOf(id)` finds a runner's row. A runner's row opens `pages/FormPage.qml` with `{ runner }` over the
tab (`theme.qml` `openSub`, the same loader as the game's sub pages), on `api.screens.runner`:
`load(id)` builds its head (`info`: name, platforms and where its program was found, a warning)
and cards for the program (`exe`, a path; the detected one shown as the value, inherited, its
origin as the detail) and arguments, the gamescope switch, then — for Proton and Wine — the launch
keys tied to its kind (`launchKeys("global")` filtered by `runners`, cards Proton, Sync, Upscaling as
on the game page, the global `[launch]` values written through `set_setting`), each option by
its type, a Games card — one `game` row per library game running through it (`gameId`, its
square or box art as `image`, its hours as the display, `installed` when it has an install folder),
by title — and an "Add a game…" action. `setValue(index, value)` writes through `set_runner_setting`
(a `launch.*` row through `set_setting`); on the add row it keeps the picked file and `pendingTitle()` proposes a title from it, which
`addGame(title)` sends to `add_game`. A game row opens the Install page's options: Game settings
(Reprise: `settingsRequested(game)`, which `theme.qml` `pushSub`s over the runner page, B coming
back to the row; Switch 2: its GameSettingsPage), Uninstall… when installed, Remove from library…,
through `uninstall(id)` / `remove(id)` on the form (a `message` when done). Both lists follow
`libraryChanged` — the counts, the Games card — so neither has a Refresh. Back on the tab, the cursor finds
the runner again. The Switch 2 look has the same list as System Settings › Runners and the same
page as `switch2/pages/FormPage.qml`, pushed on its stack. The game settings page's Launch group follows the runner: a Runner picker (names
shown, ids written), then the rows the runner takes. The detail page shows the runner's logo next
to the platform.

## Adding a game

The runner page's row is for someone who already knows the runner. Everyone else adds a game from
the Library: `ui/CoverGrid.qml` and `switch2/pages/SoftwareGrid.qml` take `addTile`, one more cell
after the last game (a "+" tile, the cursor on it as `addSelected` / `atAddTile`, never a model
index; `currentGame` is null there and A emits `addRequested`). With nothing in the library, Home
and the Library are the prompt: Reprise's hero band reads "Add your first game" and its rail
tile (`ui/LibraryTile.qml` `kind: "add"`) adds one instead of opening the Library; the Switch 2
HOME row's disc does the same and All Software says so under its tile.

Both open `api.screens.add` — Reprise as `pages/AddGamePage.qml` over the tab (`openSub` with
`{ add: true }`), the Switch 2 look as `switch2/pages/AddGamePage.qml` on its stack. `load()`
builds three cards: a "Pick a game file…" action, one action row per source of `sources()`
(`store`, `loggedIn`; its `display` says whether it is signed in) and "Import from Lutris". The
file flow is file first: `setFile(path)` keeps it and sorts `runners()` into `runnerChoices` /
`runnerIds` — the ones whose `extensions` take the file first (`linux` takes a bare binary, `.sh`,
`.x86_64`, `.AppImage`), found before missing, Proton before Wine, then by name — with `runnerIndex` on the
first; the page shows that list as a picker, `pickRunner(i)` takes the choice, `pendingTitle()`
proposes the title and `addGame(title)` calls `add_game`, toasting `message`. A store row leaves
the hub for Settings › Install (Reprise: `installRequested(source, section)` → `theme.qml`
`openSettings(section)`, whose `deliverLanding` calls the Settings page's `land(name)` once it is
the active page; the Switch 2 look pushes its Install page), signed out for Login / Sign-in, and
with the source's module off (`available` false) for Modules. The
Lutris row is two presses: `previewLutris()` runs `import_lutris(false)` off the UI thread
(`busy`) into `lutris` (the report) or `lutrisError` (Lutris absent: shown on the row, never
toasted — the client's `attempt(work)` returns the error instead of emitting it), the row then
reads "N games to import" and the same press asks to confirm; `importLutris()` applies, emits
`libraryChanged([])` — a full reload, the import updates games too — and toasts the count and hours. Nothing here touches the core: the
CLI's `universe add` and `universe migrate` are the same calls.

## The artwork page and section

`api.screens.artwork` is one game's artwork page (Reprise: the Artwork entry of a game's menu, or a
tile of the overview). `load(id)` reads `media_status` into `slots` — one row per slot with `url`
(what shows), `defaultUrl` and `overrideUrl` (the two layers, see `docs/api.md`), `kind`
(`picked`, `default`, `missing`) and `kindLabel`, `originLabel` and `defaultOriginLabel`,
`hasOverride`, `hasDefault`, `aspect`, `use` (where the themes show the slot) — and `entry`, the
SteamGridDB entry the candidates come from ("Name (year)"), which heads the candidates so a wrong
match is seen. `loadCandidates(slot)` fetches `media_candidates` off the UI thread into `candidates`
(`url` is the provider's, `thumb` what the grid shows, `votes`), `more` and `candidatesBusy`;
`moreCandidates()` takes the next page. `apply(slot, url)` runs `media_set_url` on a thread and
emits `mediaChanged` for the game once the pick landed, `removeOverride(slot)` runs `media_unset`;
both report through `message`. The wrong-match flow is `search(query)` → `hits` (`name`, `year`,
`verified`, `current`) → `pin(id)`, which writes `metadata.sgdb_id` through `media_pin` and reloads
the candidates. Local URLs carry the file's mtime as a query (`models.file_url`), so a pick that
replaces a file at the same path repaints instead of showing the image cache's copy.

`api.screens.artworkOverview` is the Artwork section of the Settings tab (`ArtworkOverview.qml`, a
column of its own next to the sidebar): `slot` and `filter` (`all`, `missing`, `picked`, `default`)
pick what `tiles` holds (`id`, `title`, `url`, `kind`), `counts` says how many games stand in each
state for the slot, `slotUse` where the slot shows, `refreshAll()` fetches the missing art of every
game (the section's X, a button top right). `load()` reads
`media_status` for the whole library on a thread; a `mediaChanged` or `libraryChanged` reloads it
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
`suspend`, `resume`. Macros and families come from the core's `controller_state`; a write goes
through `set_controller_macro`/`remove_controller_macro` and is followed by a `reload` to the watcher.

While another watcher holds the pads (a game launched from the CLI is running) the child reports
`waiting`: the screen turns passive, lists the pads from `controller_pads` under an info row
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
(a component of `PadArt`) sits on every button — white on a press, a stick leaning with its axes, a trigger filling from the
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

`man universe-ui` lists the options, `KeyScript` in `universe_ui/gamepad.py` the `--keys` names, `just --list` the dev recipes.
