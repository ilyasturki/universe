import QtQuick
import "../core"

Item {
    id: root

    property string label: ""
    property bool focused: false
    property bool active: false

    signal picked

    implicitHeight: Theme.dp(45)
    implicitWidth: text.width + Theme.dp(48)

    Pointer {
        current: root.focused
        direct: true
        radius: height / 2
        onPicked: root.picked()
    }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.active ? Theme.text : root.focused ? Qt.rgba(1, 1, 1, 0.16) : Theme.surface
        border.width: 1
        border.color: root.active ? Theme.text : Qt.rgba(1, 1, 1, 0.14)

        Behavior on color {
            ColorEase {}
        }
    }

    Loader {
        anchors.fill: parent
        active: root.focused
        sourceComponent: FocusRing {
            cornerRadius: root.height / 2
        }
    }

    Text {
        id: text
        anchors.centerIn: parent
        text: root.label
        color: root.active ? Theme.onLight : Qt.rgba(0.949, 0.953, 0.961, 0.86)
        font.family: Theme.sans
        font.weight: root.active ? Font.DemiBold : Font.Medium
        font.pixelSize: Theme.dp(21)
    }
}
