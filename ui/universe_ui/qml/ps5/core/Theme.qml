pragma Singleton
import QtQuick
import "../../core" as Base
import "../../core/Format.js" as Format

QtObject {
    id: t

    property real vscale: 1.0
    readonly property bool covered: Base.Theme.covered
    readonly property bool software: Base.Theme.software
    function dp(v) {
        return Math.round(v * t.vscale);
    }

    function reveal(flick, top, bottom, height) {
        var target = top < flick.contentY ? top : bottom > flick.contentY + height ? bottom - height : flick.contentY;
        flick.contentY = Math.max(0, Math.min(target, Math.max(0, flick.contentHeight - height)));
    }

    function alpha(c, a) {
        return Qt.rgba(c.r, c.g, c.b, a);
    }

    readonly property color ground: "#0b0d12"
    readonly property color groundHigh: "#262b36"
    readonly property color groundLow: "#11141b"
    readonly property color glass: Qt.rgba(0.086, 0.094, 0.125, 0.86)
    readonly property color glassHigh: Qt.rgba(0.16, 0.17, 0.21, 0.92)
    readonly property color glassEdge: Qt.rgba(1, 1, 1, 0.07)
    readonly property color well: Qt.rgba(1, 1, 1, 0.06)
    readonly property color focusFill: Qt.rgba(1, 1, 1, 0.1)
    readonly property color ring: Qt.rgba(1, 1, 1, 0.92)
    readonly property color ringSoft: Qt.rgba(1, 1, 1, 0.45)
    readonly property color hairline: Qt.rgba(1, 1, 1, 0.1)
    readonly property color scrim: Qt.rgba(0, 0, 0, 0.62)
    readonly property color button: Qt.rgba(0.07, 0.075, 0.09, 0.62)
    readonly property color buttonFocus: "#ffffff"
    readonly property color onLight: "#16171b"
    readonly property color artShade: "#1d2029"

    readonly property color text: "#ffffff"
    readonly property color textSecondary: Qt.rgba(1, 1, 1, 0.72)
    readonly property color textMuted: Qt.rgba(1, 1, 1, 0.5)
    readonly property color textDisabled: Qt.rgba(1, 1, 1, 0.32)
    readonly property color accent: "#3b8ff0"
    readonly property color danger: "#ef5a5a"
    readonly property color okGreen: "#4fd177"
    readonly property color plus: "#f6c343"

    readonly property color trophyPlatinum: "#a9c8ec"
    readonly property color trophyGold: "#e3b650"
    readonly property color trophySilver: "#bfc6ce"
    readonly property color trophyBronze: "#c98a64"

    // Measured on 1080p frames of the console (Sony's 2020 tour, PlayStation Access's 2025 guide).
    readonly property real edge: 172
    readonly property real columnRight: 172
    readonly property real tabX: 93
    readonly property real barY: 63
    readonly property real railY: 126
    readonly property real tileSize: 104
    readonly property real tileGap: 10
    readonly property real tileFocus: 168
    readonly property real tileFocusX: 172
    readonly property real titleY: 263
    readonly property real heroTagY: 768
    readonly property real heroButtonsY: 846
    readonly property real buttonHeight: 70
    readonly property real playWidth: 333
    readonly property real sideTile: 296
    readonly property real hubY: 982
    readonly property real headerIcon: 80
    readonly property real headerY: 88

    readonly property real radiusTile: 14
    readonly property real radiusCard: 10
    readonly property real radiusRow: 5
    readonly property real ringLine: 2.5
    readonly property real ringGap: 4

    readonly property real fontPage: 46
    readonly property real fontTab: 38
    readonly property real fontTitle: 32
    readonly property real fontBody: 28
    readonly property real fontSmall: 24
    readonly property real fontTiny: 20
    readonly property real fontClock: 40

    // Timed frame by frame on the 60 fps sources.
    readonly property int durMove: 150
    readonly property int durFocus: 130
    readonly property int durHeroOut: 80
    readonly property int backdropRest: 200
    readonly property int durBackdrop: 300
    readonly property int heroRest: 450
    readonly property int durHero: 180
    readonly property int sideRest: 600
    readonly property int durSide: 200
    readonly property int titleRest: 200
    readonly property int durScroll: 170
    readonly property int durTab: 240
    readonly property int durPage: 240
    readonly property int durQuick: 120
    readonly property int durChrome: 150
    readonly property int durSplash: 250

    // The Control Center: the dim and the first two cards at once, then a card every 230 ms, each fading in 100 ms.
    readonly property int durDim: 100
    readonly property int durCard: 100
    readonly property int cardStagger: 230
    readonly property int durReflow: 170
    readonly property int durClose: 200

    // Back from a game: black, then the row, the backdrop, the bar, the hero, the side tile.
    readonly property int beatBlack: 300
    readonly property int beatRail: 300
    readonly property int beatBackdrop: 450
    readonly property int beatChrome: 700
    readonly property int beatHero: 1200
    readonly property int beatSide: 1700

    property string clock: Format.clock()
    readonly property Timer clockTimer: Timer {
        interval: 20000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: t.clock = Format.clock()
    }

    readonly property string fontOverride: api.theme.fontPath !== "" ? "file://" + api.theme.fontPath : ""
    readonly property FontLoader fontLight: FontLoader {
        source: t.fontOverride !== "" ? "" : Qt.resolvedUrl("../assets/fonts/SourceSans3-Light.ttf")
    }
    readonly property FontLoader fontRegular: FontLoader {
        source: t.fontOverride !== "" ? t.fontOverride : Qt.resolvedUrl("../assets/fonts/SourceSans3-Regular.ttf")
    }
    readonly property FontLoader fontSemibold: FontLoader {
        source: t.fontOverride !== "" ? "" : Qt.resolvedUrl("../assets/fonts/SourceSans3-Semibold.ttf")
    }
    readonly property string sans: fontRegular.status === FontLoader.Ready ? fontRegular.name : "sans-serif"
}
