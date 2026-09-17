import QtQuick
import "../core"
import "../core/Format.js" as Format

Item {
    id: root

    property string kind: "library"
    property bool selected: false
    property real idleScale: 1.0
    property int count: 0
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property real ringOpacity: 1.0

    Behavior on ringOpacity {
        enabled: root.selected
        Ease { duration: Theme.durQuick }
    }

    Item {
        id: body

        anchors.fill: parent
        transformOrigin: Item.Bottom
        opacity: root.selected ? 1.0 : Theme.idleOpacity
        scale: root.selected ? 1.0 : root.idleScale

        Behavior on opacity { Ease { easing.type: Easing.OutQuint } }
        Behavior on scale { Ease { easing.type: Easing.OutQuint } }

        Rectangle {
            anchors.fill: parent
            radius: root.cornerRadius
            color: Theme.surface
            border.width: Math.max(1, Theme.dp(2))
            border.color: Theme.surfaceBorder
        }

        Column {
            anchors.centerIn: parent
            spacing: Theme.dp(16)

            MenuGlyph {
                anchors.horizontalCenter: parent.horizontalCenter
                width: Theme.dp(56)
                height: width
                kind: root.kind === "add" ? "plus" : "library"
                tint: Theme.text
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: root.kind === "add" ? "Add a game" : "Library"
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(30)
            }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: root.kind !== "add"
                text: Format.plural(root.count, "game", "games")
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(22)
            }
        }

        Loader {
            anchors.fill: parent
            active: root.selected
            opacity: root.ringOpacity
            sourceComponent: FocusRing { cornerRadius: root.cornerRadius }
        }
    }
}
