import QtQuick
import "../core"

// A screen's name, the console's two ways: alone and large ("Settings"), or small beside an icon or a game's tile ("Game Library").
Item {
    id: header

    property string title: ""
    property string icon: ""
    property var game: null
    property string trailing: ""

    readonly property bool badged: icon !== "" || game !== null

    implicitHeight: Theme.dp(badged ? 150 : 176)

    Rectangle {
        id: slot
        visible: header.badged
        x: Theme.dp(48)
        y: Theme.dp(48)
        width: Theme.dp(Theme.headerIcon)
        height: width
        radius: Theme.dp(12)
        color: header.game ? "transparent" : Qt.rgba(1, 1, 1, 0.08)

        Glyph {
            anchors.centerIn: parent
            visible: header.icon !== ""
            width: Theme.dp(44)
            height: width
            kind: header.icon
        }

        TileArt {
            anchors.fill: parent
            visible: header.game !== null
            game: header.game
            radius: parent.radius
        }
    }

    Label {
        x: header.badged ? Theme.dp(Theme.edge) : Theme.dp(96)
        anchors.verticalCenter: header.badged ? slot.verticalCenter : undefined
        y: header.badged ? 0 : Theme.dp(72)
        width: trailingText.x - x - Theme.dp(40)
        text: header.title
        elide: Text.ElideRight
        font.weight: Font.Light
        font.pixelSize: Theme.dp(header.badged ? Theme.fontTitle : Theme.fontPage)
    }

    Label {
        id: trailingText
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(Theme.columnRight)
        y: header.badged ? slot.y + (slot.height - height) / 2 : Theme.dp(84)
        text: header.trailing
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }
}
