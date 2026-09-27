import QtQuick
import "../core"

// The console's focus: a thin light line a few units out from the item, a faint glow past it.
Item {
    id: frame

    property Item target: parent
    property real radius: Theme.dp(Theme.radiusTile)
    property real gap: Theme.dp(Theme.ringGap)
    property real line: Theme.dp(Theme.ringLine)
    property bool shown: true
    property color color: Theme.ring
    readonly property real pad: gap + line

    anchors.fill: target
    anchors.margins: -pad
    opacity: shown ? 1.0 : 0.0
    visible: opacity > 0.01
    z: 5

    Behavior on opacity {
        Ease {}
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: -Theme.dp(5)
        radius: frame.radius + frame.pad + Theme.dp(5)
        color: "transparent"
        border.width: Theme.dp(5)
        border.color: Qt.rgba(1, 1, 1, 0.06)
    }

    Rectangle {
        anchors.fill: parent
        radius: frame.radius + frame.pad
        color: "transparent"
        border.width: frame.line
        border.color: frame.color
        antialiasing: true
    }
}
