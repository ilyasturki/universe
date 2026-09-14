import QtQuick
import "../core"

// The focus ring: a thin band whose colours travel around the perimeter, a white gap inside it.
// A shader on screen; under GraphicsInfo.Software (the offscreen tests) a still ring of plain rectangles.
Item {
    id: ring

    property Item target: parent
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property real gap: Theme.dp(Theme.ringGap)
    property real lineWidth: Theme.dp(Theme.ringLine)
    property bool shown: true

    readonly property real pad: gap + lineWidth
    readonly property bool software: GraphicsInfo.api === GraphicsInfo.Software

    anchors.fill: target
    anchors.margins: -pad
    visible: shown
    z: 5

    ShaderEffect {
        anchors.fill: parent
        visible: !ring.software

        readonly property vector2d size: Qt.vector2d(width, height)
        readonly property real radius: ring.cornerRadius + ring.pad
        readonly property real line: ring.lineWidth
        readonly property real gap: ring.gap
        readonly property real phase: ring.visible ? Theme.ringPhase : 0
        readonly property real cycles: 1
        readonly property real sharp: 0
        readonly property color c0: Theme.ringBlue
        readonly property color c1: Theme.ringCyan
        readonly property color c2: Theme.ringPink
        readonly property color c3: Theme.ringLavender
        readonly property color inner: Theme.ringInner

        fragmentShader: Qt.resolvedUrl("../assets/shaders/ring.frag.qsb")
    }

    Rectangle {
        anchors.fill: parent
        visible: ring.software
        radius: ring.cornerRadius + ring.pad
        color: "transparent"
        border.width: ring.lineWidth
        border.color: Theme.ringBlue

        Rectangle {
            anchors.fill: parent
            anchors.margins: ring.lineWidth
            visible: ring.gap > 0
            radius: ring.cornerRadius + ring.gap
            color: "transparent"
            border.width: ring.gap
            border.color: Theme.ringInner
        }
    }
}
