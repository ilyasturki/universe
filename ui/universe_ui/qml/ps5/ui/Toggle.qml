import QtQuick
import "../core"

Item {
    id: root

    property bool on: false

    width: Theme.dp(64)
    height: Theme.dp(32)

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.on ? Qt.rgba(1, 1, 1, 0.34) : Qt.rgba(1, 1, 1, 0.1)

        Behavior on color {
            ColorAnimation {
                duration: Theme.durQuick
            }
        }
    }

    Rectangle {
        width: Theme.dp(26)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: root.on ? parent.width - width - Theme.dp(3) : Theme.dp(3)
        color: root.on ? "#e9eaec" : "transparent"
        border.width: root.on ? 0 : Theme.dp(2)
        border.color: Qt.rgba(1, 1, 1, 0.55)

        Behavior on x {
            Ease {
                duration: Theme.durQuick
            }
        }
    }
}
