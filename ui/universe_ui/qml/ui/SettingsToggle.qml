import QtQuick
import "../core"

Rectangle {
    id: root
    objectName: "settingsToggle"

    property bool on: false
    property bool focused: false

    width: Theme.dp(64)
    height: Theme.dp(34)
    radius: height / 2
    color: on ? (focused ? Theme.accentOnLight : Theme.accent) : (focused ? "#e4e5e9" : "#24262d")
    border.width: on ? 0 : Theme.dp(2)
    border.color: focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Qt.rgba(1, 1, 1, 0.5)

    Behavior on color {
        ColorEase {}
    }

    Rectangle {
        width: parent.height - Theme.dp(10)
        height: width
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: root.on ? parent.width - width - Theme.dp(5) : Theme.dp(5)
        color: root.on ? "#ffffff" : root.focused ? Theme.onLight : "#b4b6bd"

        Behavior on x {
            Ease {
                duration: Theme.durQuick
            }
        }
    }
}
