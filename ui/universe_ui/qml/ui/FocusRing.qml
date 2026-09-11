import QtQuick
import "../core"

Item {
    id: root

    property real cornerRadius: Theme.dp(Theme.radiusCover)
    property real gapWidth: 0

    readonly property real ringWidth: Theme.dp(4)
    readonly property real haloWidth: Theme.dp(10)

    Rectangle {
        anchors.fill: parent
        anchors.margins: -(root.gapWidth + root.ringWidth + root.haloWidth)
        radius: root.cornerRadius + root.gapWidth + root.ringWidth + root.haloWidth
        color: "transparent"
        border.width: root.haloWidth
        border.color: Qt.rgba(1, 1, 1, 0.10)
        antialiasing: true
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: -(root.gapWidth + root.ringWidth)
        radius: root.cornerRadius + root.gapWidth + root.ringWidth
        color: "transparent"
        border.width: root.ringWidth
        border.color: "#ffffff"
        antialiasing: true
    }
}
