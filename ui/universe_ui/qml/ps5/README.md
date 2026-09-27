# The PS5 look

A third look on the same host; how looks load and switch is `docs/frontends.md` › Themes.
Everything is authored at 1080p and scaled by the window height through `Theme.dp()`; the sizes
and timings in `core/Theme.qml` are measured on 1080p frames of the console (Sony's 2020 tour at
60 fps, PlayStation Access's 2025 guide at 60 fps, a 30 fps no-commentary tour). A 16:10 screen
(a Steam Deck's 1280×800) is 1728 wide in those units: the right margin (`Theme.columnRight`)
holds and the columns and grids narrow.

The type is Source Sans 3 (OFL, `assets/fonts/`), the nearest open face to the console's SST, in
its Light, Regular and Semibold; Settings › Themes › Font takes a `.ttf` you own instead
(`api.theme.fontPath`, stored as `ps5Font`). The sounds are synthesized by `assets/sounds/generate.py`.

## The screens

- **Home** (`pages/HomePage.qml`): Games and Media at the top left, search, settings and power at
  the right with the batteries and the clock. The row: the Welcome hub first, the last twelve played
  (an install under way heads them), then Store, Media Gallery and Game Library. The focused tile
  is large at a fixed place and the rest slide past it; its world (the game's background, else a
  screenshot, its banner, its box) fills the screen behind. Down enters the hero (logo, one line,
  Play and "…", the side tile), Down again the **Game Hub**: Continue where you left off, Trophies,
  Captures, Journal, About, Manage. The Welcome hub is one fixed layout: what needs attention, the
  latest capture, trophies, the controller, free space, the week's play time.
- **Control Center** (`ui/ControlCenter.qml`, the overlay window over the game): the game's cards
  (the game, its hub, trophies, captures, the journal's next step), the icon bar (Home, the game,
  Screenshot, Trophies, Performance, Sound, System, Power) with a panel over each icon; the
  Trophies and Captures cards grow into their lists. It carries every row of Reprise's dock.
- **Game Library**, **Media Gallery** and **Journal**, **Store**, **Search**, **Settings** (a list
  of sections, each on two columns: its parts left, their rows right), and the per-game pages the
  hub and the "…" menu open (Information, Trophies, Game Settings, Artwork, Play Log).

The console shows no button hints. A page sets `strip: true` only where the console has no such
screen (settings forms, the controller walk, the recordings player, the keyboard) and the slim
strip comes up at the bottom right; everywhere else every action is also in the Options menu.

## Motion, as timed on the 60 fps sources

| | |
|---|---|
| A move on the home row | the hero and the name go at once; the tiles slide 150 ms (OutCubic); the world waits 200 ms, then cross-fades 300 ms; the hero comes back 450 ms after the move, the side tile 600 ms after it |
| Down into the hero | 170 ms scroll; the row and the tabs go in 80 ms, the game's tile and name take their place |
| Play | the button greys, the chrome fades 150 ms, the game's art fills the screen in 250 ms and zooms slowly while the game loads, then a hard cut |
| Control Center | the dim and the first two cards at once (100 ms), then a card every 230 ms, each fading in 100 ms; focus leaving the cards reflows them in 170 ms; closing fades everything in 200 ms |
| A card grown | 250 ms from the card to the full panel |
| Back from a game | black, the row at 300 ms, the world at 450 ms, the bar at 700 ms, the hero at 1.2 s, the side tile at 1.7 s |
| Tabs, pages | Games/Media slide and cross-fade 240 ms; a page fades in from 102 % in 240 ms |

## A page

```qml
FocusScope {
    property var shell: null          // set by theme.qml at load
    property var args: ({})           // what push() passed, plain data: { gameId, session, … }
    readonly property var hints: []   // shown only with strip: true
    readonly property bool strip: false
    signal closeRequested()           // B at the page's top level; theme.qml pops
    focus: true
}
```

Pages take a `Game` by id (`api.allGames.byId(args.gameId)`). Keys reach the page first; what it
does not accept falls to `theme.qml`: **B** pops the page, **Start** (F1) goes home. A page with an
inner level accepts B itself while inside, and accepts Start when it means Options.

The shell is `theme.qml`'s functions: `push`, `pop`, `goHome`, `openGame(id)` (home, the game's
tile focused, its hero open; a game off the row joins it first), `launch`, `resume`,
`closeSoftware`, `stopSession`, `showToast`, `askPower`; the questions `dialogAsk` (B answers 0),
`pick` ({title, choices, icons, index, at}: −1), `showMenu` ({title, items, index, at, width}: −1;
items `{label, detail, glyph, icon, check, toggle, danger, gap}`), `menu`, `prompt` and
`promptPair` (null), `browse` (null).

Touch and the mouse go through `ui/Touch.qml` (a tap picks, a tap on what holds the cursor or on a
`direct` item presses `action`), `ui/Swipe.qml` and `ui/Block.qml`, as in the Switch 2 look.

## Running

```sh
just ui-fake --theme ps5 --windowed
UNIVERSE_UI_ARGS="--fake --no-gamepad --theme ps5" just ui-shot out Down Shot:hero.png
```
