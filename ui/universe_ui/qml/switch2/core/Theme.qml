pragma Singleton
import QtQuick
import "../../core" as Base
import "../../core/Format.js" as Format

QtObject {
    id: t

    property real vscale: 1.0
    readonly property bool software: Base.Theme.software
    function dp(v) {
        return Math.round(v * t.vscale);
    }

    function reveal(flick, top, bottom, height) {
        var target = top < flick.contentY ? top : bottom > flick.contentY + height ? bottom - height : flick.contentY;
        flick.contentY = Math.max(0, Math.min(target, Math.max(0, flick.contentHeight - height)));
    }

    readonly property color ground: "#ebebeb"
    readonly property color bar: "#f4f4f4"
    readonly property color slot: "#f2f2f2"
    readonly property color card: "#fafafa"
    readonly property color focusFill: "#fcfcfc"
    readonly property color hairline: "#cdcdcd"
    readonly property color hairlineSoft: "#dcdcdc"
    readonly property color scrim: Qt.rgba(0.1, 0.1, 0.1, 0.5)
    readonly property color artShade: "#d8d8d8"
    readonly property color artInk: "#4a4a4a"
    readonly property color disc: "#ffffff"
    readonly property color discEdge: "#dcdcdc"
    readonly property color thumb: "#b8b8b8"
    readonly property color toggleOff: "#c4c4c4"

    readonly property color text: "#2d2d2d"
    readonly property color textSecondary: "#7a7a7a"
    readonly property color textMuted: "#a0a0a0"
    readonly property color textDisabled: "#b8b8b8"
    readonly property color accent: "#2f6fd6"
    readonly property color accentStrong: "#0d61c4"
    readonly property color accentInk: "#ffffff"
    readonly property color glyphFill: "#2d2d2d"
    readonly property color glyphInk: "#ffffff"
    readonly property color danger: "#e60012"
    readonly property color okGreen: "#3cbc3c"

    readonly property color barRed: "#e60012"
    readonly property color barOrange: "#f08a2c"
    readonly property color barGreen: "#3ab54a"
    readonly property color barBlue: "#2f6fd6"
    readonly property color barGrey: "#5a5a5a"

    // The band's colours in perimeter order, sampled from the Switch 2 at 60 fps
    readonly property color ringBlue: "#0a6edf"
    readonly property color ringCyan: "#52bcff"
    readonly property color ringPink: "#f6d9ee"
    readonly property color ringLavender: "#b9a6ea"
    readonly property color ringInner: "#ffffff"

    readonly property real radiusTile: 27
    readonly property real radiusRow: 6

    // The focus ring, outward from the item: a white gap, then the band.
    readonly property real ringGap: 6
    readonly property real ringLine: 7
    // What a clipping view reserves past its items so the ring is never cut; tight: a ring with no gap
    readonly property real ringRoom: ringGap + ringLine
    readonly property real ringRoomTight: ringLine

    readonly property real edgeMargin: 72
    readonly property real headerHeight: 126
    readonly property real hintBarHeight: 108
    readonly property real tileSize: 384
    readonly property real tileGap: 22
    readonly property real tileRowY: 300
    readonly property real tileRowX: 159
    readonly property real barHeight: 129
    readonly property real barY: 776

    readonly property int durFocus: 120
    readonly property int durQuick: 150
    readonly property int durPage: 200
    readonly property int durFade: 300

    readonly property real fontTitle: 42
    readonly property real fontBody: 33
    readonly property real fontSmall: 26
    readonly property real fontTiny: 22
    readonly property real fontClock: 44

    // One clock for every ring, so a focus move never restarts the sweep.
    property real ringPhase: 0
    readonly property NumberAnimation ringClock: NumberAnimation {
        target: t
        property: "ringPhase"
        from: 0
        to: 1
        duration: 2400
        loops: Animation.Infinite
        running: Qt.application.state === Qt.ApplicationActive
    }

    property string clock: Format.clock()
    readonly property Timer clockTimer: Timer {
        interval: 20000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: t.clock = Format.clock()
    }

    readonly property FontLoader fontRegular: FontLoader {
        source: fontOverride !== "" ? fontOverride : Qt.resolvedUrl("../assets/fonts/BIZUDPGothic-Regular.ttf")
    }
    readonly property FontLoader fontBold: FontLoader {
        source: fontOverride !== "" ? "" : Qt.resolvedUrl("../assets/fonts/BIZUDPGothic-Bold.ttf")
    }
    readonly property string fontOverride: api.theme.fontPath !== "" ? "file://" + api.theme.fontPath : ""
    readonly property string sans: fontRegular.status === FontLoader.Ready ? fontRegular.name : "sans-serif"
    // Sawarabi Gothic: narrow, light digits like the Switch's clock; the body font's are wide.
    readonly property FontLoader fontClockFace: FontLoader {
        source: Qt.resolvedUrl("../assets/fonts/SawarabiGothic-Regular.ttf")
    }
    readonly property string clockSans: fontClockFace.status === FontLoader.Ready ? fontClockFace.name : sans
}
