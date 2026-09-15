import QtQuick
import "../core"

Rectangle {
    id: root

    property bool on: false
    property bool focused: false

    width: Theme.dp(64)
    height: Theme.dp(34)
    radius: height / 2
    color: on ? (focused ? Theme.onLight : Theme.text)
              : (focused ? Qt.rgba(0.063, 0.067, 0.086, 0.25) : Qt.rgba(1, 1, 1, 0.18))

    Behavior on color {
        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Rectangle {
        width: parent.height - Theme.dp(8)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: root.on ? parent.width - width - Theme.dp(4) : Theme.dp(4)
        color: root.on ? (root.focused ? Theme.text : Theme.onLight) : Theme.text

        Behavior on x {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }
}
