# The Switch 2 look

A second look on the same host; how looks load and switch is `docs/frontends.md` › Themes.
Everything is authored at 1080p and scaled by the window height through `Theme.dp()`; the sizes
in `core/Theme.qml` are measured from captures of the real HOME menu.

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
