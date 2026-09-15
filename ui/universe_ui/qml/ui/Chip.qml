import QtQuick
import "../core"

Item {
    id: root

    property string label: ""
    property string icon: ""
    property string trailing: ""
    property string badge: ""
    property bool showSortIcon: false
    property bool focused: false
    property bool active: false

    implicitHeight: Theme.dp(45)
    implicitWidth: body.width + Theme.dp(48)

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: root.active ? Theme.text : root.focused ? Qt.rgba(1, 1, 1, 0.16) : Theme.surface
        border.width: 1
        border.color: root.active ? Theme.text : Qt.rgba(1, 1, 1, 0.14)

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

        MenuGlyph {
            visible: root.icon !== ""
            width: Theme.dp(22)
            height: width
            anchors.verticalCenter: parent.verticalCenter
            kind: root.icon
            tint: root.active ? Theme.onLight : Qt.rgba(0.949, 0.953, 0.961, 0.72)
        }

        Text {
            text: root.label
            color: root.active ? Theme.onLight : Qt.rgba(0.949, 0.953, 0.961, 0.86)
            font.family: Theme.sans
            font.weight: root.active ? Font.DemiBold : Font.Medium
            font.pixelSize: Theme.dp(21)
            anchors.verticalCenter: parent.verticalCenter
        }

        Rectangle {
            visible: root.badge !== ""
            width: Math.max(height, badgeText.width + Theme.dp(16))
            height: Theme.dp(26)
            radius: height / 2
            color: root.active ? Theme.onLight : Qt.rgba(1, 1, 1, 0.14)
            anchors.verticalCenter: parent.verticalCenter

            Text {
                id: badgeText
                anchors.centerIn: parent
                text: root.badge
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(17)
            }
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
