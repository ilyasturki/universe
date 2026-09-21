pragma Singleton
import QtQuick
import "Format.js" as Format

QtObject {
    id: t

    property real vscale: 1.0
    // The software scenegraph (offscreen tests) drops every ShaderEffect; set by the root.
    property bool software: false
    // A game is on screen over the launcher, inside gamescope (`api.home.underGame`, set by the root): every loop and clock holds, so nothing repaints behind it.
    property bool covered: false
    onCoveredChanged: {
        if (!covered)
            clock = Format.clock();
    }
    function dp(v) {
        return Math.round(v * t.vscale);
    }

    // A Pointer landed on `item`: the root tells the bar from the page by it.
    signal pointed(Item item)

    function reveal(flick, top, bottom, height) {
        var target = top < flick.contentY ? top : bottom > flick.contentY + height ? bottom - height : flick.contentY;
        flick.contentY = Math.max(0, Math.min(target, Math.max(0, flick.contentHeight - height)));
    }

    readonly property color ground: "#0e0f13"
    readonly property color text: "#f2f3f5"
    readonly property color textSecondary: Qt.rgba(0.949, 0.953, 0.961, 0.66)
    readonly property color textMuted: Qt.rgba(0.949, 0.953, 0.961, 0.45)
    readonly property color textFaint: Qt.rgba(0.949, 0.953, 0.961, 0.32)
    readonly property color textTab: Qt.rgba(0.949, 0.953, 0.961, 0.5)
    readonly property color textHint: Qt.rgba(0.949, 0.953, 0.961, 0.78)
    readonly property color surface: Qt.rgba(1, 1, 1, 0.08)
    readonly property color surfaceBorder: Qt.rgba(1, 1, 1, 0.16)
    readonly property color cardBase: "#161616"
    readonly property color onLight: "#101116"

    readonly property real radiusCover: 12
    readonly property real radiusTile: 16

    readonly property real idleOpacity: 0.72
    readonly property real ringIdle: 0.35
    readonly property real tabBarHeight: 96
    readonly property real hintBarHeight: 92
    readonly property real edgeMargin: 80
    readonly property real heroBand: 610
    readonly property real heroDetail: 620

    readonly property int durQuick: 160
    readonly property int durDismiss: 200
    readonly property int durBase: 220
    readonly property int durNudge: 260
    readonly property int durView: 300
    readonly property int durScene: 420
    readonly property int durLaunch: 600
    readonly property int durZoom: 15500
    readonly property int durDriftX: 16800
    readonly property int durDriftY: 17500

    property string clock: Format.clock()
    readonly property Timer clockTimer: Timer {
        interval: 20000
        running: !t.covered
        repeat: true
        triggeredOnStart: true
        onTriggered: t.clock = Format.clock()
    }

    // Loaded for the side effect: the weight faces register under the Archivo family.
    readonly property FontLoader fontRegular: FontLoader {
        source: Qt.resolvedUrl("../assets/fonts/Archivo-Regular.ttf")
    }
    readonly property FontLoader fontMedium: FontLoader {
        source: Qt.resolvedUrl("../assets/fonts/Archivo-Medium.ttf")
    }
    readonly property FontLoader fontSemiBold: FontLoader {
        source: Qt.resolvedUrl("../assets/fonts/Archivo-SemiBold.ttf")
    }
    readonly property FontLoader fontBold: FontLoader {
        source: Qt.resolvedUrl("../assets/fonts/Archivo-Bold.ttf")
    }

    readonly property string sans: fontRegular.name
}
