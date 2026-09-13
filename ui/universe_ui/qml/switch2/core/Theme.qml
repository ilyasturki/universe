pragma Singleton
import QtQuick
import "../../core/Format.js" as Format

QtObject {
    id: t

    property real vscale: 1.0
    function dp(v) { return Math.round(v * t.vscale) }

    function reveal(flick, top, bottom, height) {
        var target = top < flick.contentY ? top : bottom > flick.contentY + height ? bottom - height : flick.contentY;
        flick.contentY = Math.max(0, Math.min(target, Math.max(0, flick.contentHeight - height)));
    }

    readonly property bool dark: api.theme.variant === "black"

    readonly property color ground: dark ? "#1b1b1b" : "#ebebeb"
    readonly property color bar: dark ? "#2c2c2c" : "#f4f4f4"
    readonly property color slot: dark ? "#242424" : "#f2f2f2"
    readonly property color card: dark ? "#2a2a2a" : "#fafafa"
    readonly property color focusFill: dark ? "#1e1e1e" : "#fcfcfc"
    readonly property color hairline: dark ? "#3a3a3a" : "#cdcdcd"
    readonly property color hairlineSoft: dark ? "#303030" : "#dcdcdc"
    readonly property color scrim: dark ? Qt.rgba(0, 0, 0, 0.6) : Qt.rgba(0.1, 0.1, 0.1, 0.5)

    readonly property color text: dark ? "#f2f2f2" : "#2d2d2d"
    readonly property color textSecondary: dark ? "#b4b4b4" : "#7a7a7a"
    readonly property color textMuted: dark ? "#8a8a8a" : "#a0a0a0"
    readonly property color textDisabled: dark ? "#5a5a5a" : "#b8b8b8"
    readonly property color accent: dark ? "#4bbacd" : "#2f6fd6"
    readonly property color accentStrong: dark ? "#5bc8dc" : "#0d61c4"
    readonly property color accentInk: "#ffffff"
    readonly property color glyphFill: dark ? "#f2f2f2" : "#2d2d2d"
    readonly property color glyphInk: dark ? "#1b1b1b" : "#ffffff"
    readonly property color danger: "#e60012"
    readonly property color okGreen: "#3cbc3c"

    readonly property color barRed: "#e60012"
    readonly property color barOrange: "#f08a2c"
    readonly property color barGreen: "#3ab54a"
    readonly property color barBlue: "#2f6fd6"
    readonly property color barGrey: dark ? "#c8c8c8" : "#5a5a5a"

    readonly property color outlineCyan: "#4de3ff"
    readonly property color outlineBlue: "#2f7cf0"
    readonly property color outlineViolet: "#a389e8"
    readonly property color outlinePink: "#f1c4f6"

    readonly property real radiusTile: 9
    readonly property real radiusRow: 6
    readonly property real outlineWidth: 5

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

    property string clock: Format.clock()
    readonly property Timer clockTimer: Timer {
        interval: 20000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: t.clock = Format.clock()
    }

    // BIZ UDPGothic: the free relative of the Switch's UD Shin Go.
    readonly property FontLoader fontRegular: FontLoader { source: fontOverride !== "" ? fontOverride : Qt.resolvedUrl("../assets/fonts/BIZUDPGothic-Regular.ttf") }
    readonly property FontLoader fontBold: FontLoader { source: fontOverride !== "" ? "" : Qt.resolvedUrl("../assets/fonts/BIZUDPGothic-Bold.ttf") }
    readonly property string fontOverride: api.theme.fontPath !== "" ? "file://" + api.theme.fontPath : ""
    readonly property string sans: fontRegular.status === FontLoader.Ready ? fontRegular.name : "sans-serif"
}
