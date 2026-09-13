# The Switch 2 look

A second look on the same host; how looks load and switch is `docs/frontends.md` › Themes.
Everything is authored at 1080p and scaled by the window height through `Theme.dp()`; the sizes
in `core/Theme.qml` are measured from captures of the real HOME menu.

```
switch2/
  theme.qml        HOME (TopBar, HomePage, BottomBar) and the page stack over it, the shell API
  core/Theme.qml   palette (white/black), sizes, timings, the font
  sound/Sound.qml  tick, ok, back, edge, type, select, open, home, launch (assets/sounds/generate.py)
  ui/              the kit
  pages/           one file per screen
```

## A page

```qml
FocusScope {
    property var shell: null          // set by theme.qml at load
    property var args: ({})           // what push() passed, plain data: { gameId, session, … }
    readonly property var hints: [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
    readonly property bool bare: false   // true: no hint-bar hairline, the page owns the bottom (a player)
    signal closeRequested()           // B at the page's top level; theme.qml pops
    focus: true
}
```

Pages get a `Game` by id (`api.allGames.byId(args.gameId)`), never hold one across a library
reload. Keys reach the page first; what it does not accept falls to `theme.qml`: **B** pops the
page, **Start/Guide** (one key, F1) goes HOME. A page with an inner level (a grid that opens a
player, a feed that opens an article) accepts B itself while inside, and accepts Start when it
means "+ Options".

The shell:

| Call | What it does |
|---|---|
| `shell.push(source, args)` | a page over this one; `source` relative to `switch2/` (`"pages/AlbumPage.qml"`) |
| `shell.pop()` | back one page |
| `shell.launch(game)` | the launch screen, then `game.launch()` |
| `shell.closeSoftware(game)` | the "Close the software?" dialog, then `stop` |
| `shell.dialogAsk({ message, detail, buttons, index, danger }, done(i))` | the dialog; B answers 0 |
| `shell.pick({ title, choices, index }, done(i))` | the small list; B answers -1 |
| `shell.prompt({ title, value, max, numeric, path }, done(value))` | the keyboard; cancel answers `null` |
| `shell.showToast(text)` | a line at the top left |

`hints` glyphs are the Xbox names (`A B X Y LB RB LT RT Start Select dpad`).

## Running

```sh
just ui --theme switch2-white                                   # the dev library, real art
just ui-fake --theme switch2-black --screenshot out.png --keys "Down Return Wait Right"
```

From HOME, `Down` reaches the bar: All Software, News, Install, Album, Controllers, System
Settings, Quit. A bare `universe-ui` writes the pad it sees, and any pick in Settings › Themes,
into the real `ui-memory.json`.
