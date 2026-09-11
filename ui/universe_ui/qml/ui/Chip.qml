import QtQuick
import "../core"

Item {
    id: root

    property string label: ""
    property string trailing: ""
    property bool showSortIcon: false
    property bool focused: false

    implicitHeight: Theme.dp(45)
    implicitWidth: body.width + Theme.dp(48)

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.focused ? Qt.rgba(1, 1, 1, 0.16) : Theme.surface
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.14)

        Behavior on color {
            ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    Loader {
        anchors.fill: parent
        active: root.focused
        sourceComponent: FocusRing { cornerRadius: root.height / 2 }
    }

    Row {
        id: body
        anchors.centerIn: parent
        spacing: Theme.dp(12)

        Item {
            visible: root.showSortIcon
            width: visible ? Theme.dp(20) : 0
            height: Theme.dp(20)
            anchors.verticalCenter: parent.verticalCenter

            Column {
                anchors.centerIn: parent
                spacing: Theme.dp(4)

                Repeater {
                    model: [18, 12, 5]

                    Rectangle {
                        width: Theme.dp(modelData)
                        height: Math.max(1, Theme.dp(2))
                        radius: height / 2
                        color: Theme.textSecondary
                    }
                }
            }
        }

        Text {
            text: root.label
            color: Qt.rgba(0.949, 0.953, 0.961, 0.86)
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(21)
            anchors.verticalCenter: parent.verticalCenter
        }

        Text {
            visible: root.trailing !== ""
            text: root.trailing
            color: Theme.textMuted
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(21)
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
