import QtQuick
import "../core"

Rectangle {
    id: root

    property bool on: false

    width: Theme.dp(78)
    height: Theme.dp(42)
    radius: height / 2
    color: on ? Theme.accentStrong : Theme.toggleOff

    Behavior on color {
        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Rectangle {
        width: parent.height - Theme.dp(8)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: root.on ? parent.width - width - Theme.dp(4) : Theme.dp(4)
        color: "#ffffff"

        Behavior on x {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }
}
