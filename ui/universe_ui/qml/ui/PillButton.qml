import QtQuick
import "../core"

Rectangle {
    id: root

    property string label: "Play"
    // A MenuGlyph kind, or "" for none.
    property string icon: "play"
    property bool ghost: false
    property bool focused: false
    property bool dimmed: false

    readonly property color ink: root.ghost ? Theme.text : Theme.onLight

    height: Theme.dp(78)
    width: content.width + Theme.dp(root.ghost ? 76 : 92)
    radius: height / 2
    color: root.ghost ? Theme.surface : Theme.text
    border.width: root.ghost ? Math.max(1, Theme.dp(2)) : 0
    border.color: Theme.surfaceBorder

    opacity: root.dimmed ? 0.5 : 1.0
    scale: root.focused ? 1.04 : 1.0

    Behavior on opacity {
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }
    Behavior on scale {
        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
    }

    Loader {
        anchors.fill: parent
        active: root.focused
        sourceComponent: FocusRing {
            cornerRadius: root.radius
            gapWidth: Theme.dp(6)
        }
    }

    Row {
        id: content
        anchors.centerIn: parent
        spacing: Theme.dp(16)

        MenuGlyph {
            visible: root.icon !== ""
            width: Theme.dp(26); height: Theme.dp(26)
            anchors.verticalCenter: parent.verticalCenter
            kind: root.icon
            tint: root.ink
        }

        Text {
            text: root.label
            color: root.ink
            font.family: Theme.sans
            font.weight: root.ghost ? Font.Medium : Font.DemiBold
            font.pixelSize: Theme.dp(root.ghost ? 25 : 27)
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
