# The Switch 2 look

A second look on the same host; how looks load and switch is `docs/frontends.md` › Themes.
Everything is authored at 1080p and scaled by the window height through `Theme.dp()`; the sizes
in `core/Theme.qml` are measured from captures of the real HOME menu. A 16:10 screen (a Steam
Deck's 1280×800) is 1728 wide in those units, not 1920: a column or a grid takes its width from the
page's and keeps the right margin it has at 1920 (`Theme.columnRight`), its tiles narrowing rather
than running off the edge.

Touch and the mouse go through `ui/Touch.qml` (a tap picks, a tap on what holds the cursor or on a
`direct` item presses `action`, a long press on one with `menu` picks and presses +),
`ui/Swipe.qml` (a drag scrolls the view it sits over, with a fling) and `ui/Block.qml` (a scrim or
a card that keeps a tap or a drag from what is beneath). Each surface's `point(i)` moves the cursor
as its keys would and raises `pointed`, on which the page sets its zone. A list, a grid, a picker or
a text scrolls just far enough to show the cursor's row, in 200 ms OutCubic (`Theme.reveal` under
`ui/Ease.qml`); the HOME row slides its tiles in 120 ms.

The sounds are synthesized by `assets/sounds/generate.py` (run it to rewrite the WAVs), each recipe
modelled on the console's own sound for that action: `tick` for lists, `tick-tile` on the HOME row,
`tick-side` in a sidebar, `edge`, `ok`, `back`, `tab`, `select` / `deselect` for a toggle,
`open`, `type`, `home` (reaching HOME, by the button or B), `launch`, and `icon-<icon>` for each
bottom-bar icon on A. Settings › Themes › Sound folder swaps any of them for a WAV of the same name.

The focus ring's shader ships as `assets/shaders/ring.frag.qsb`, rebuilt from `ring.frag` with
`qsb --glsl "100 es,120,150" --hlsl 50 --msl 12 -o ring.frag.qsb ring.frag`.

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

The shell is theme.qml's functions; B answers `dialogAsk` with 0, `pick` with −1, `prompt` and
`browse` with `null`.

`hints` glyphs are the Xbox names (`A B X Y LB RB LT RT Start Select dpad`); `dim: true` keeps a
hint in place at low opacity when the page has nothing for it, and the d-pad is not written for
plain navigation. `ui/HintGlyph.qml` draws them the HOME way from the same prompts as Reprise's
`PadGlyph` (the outline in ink under the filled disc, the mark showing through).

## Running

```sh
just ui-fake --theme switch2 --windowed --keys "Down Return Wait Right Shot:out.png" --quit-after 3000
```

A bare `universe-ui` writes the pad it sees, and any pick in Settings › Themes, into the real
`ui-memory.json`.
