import QtQuick
import "../core"

// A sub page's head: the game's tile, what the page is, its title and the meta line.
Item {
    id: root

    property var game: null
    // "GAME SETTINGS", "RECORDINGS"…
    property string label: ""
    // Muted, after the meta line: "12 sessions".
    property string detail: ""

    height: Theme.dp(88)

    CoverCard {
        id: tile
        width: Theme.dp(88)
        height: width
        game: root.game
        cornerRadius: Theme.dp(14)
        selected: true
        selectedScale: 1.0
        ringOpacity: 0
        showHeart: false
    }

    Column {
        anchors.left: tile.right
        anchors.leftMargin: Theme.dp(28)
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(4)

        CapsLabel {
            text: root.label
        }

        Row {
            width: parent.width
            spacing: Theme.dp(20)

            Text {
                id: title
                text: root.game ? root.game.title : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Bold
                font.pixelSize: Theme.dp(42)
                elide: Text.ElideRight
                width: Math.min(implicitWidth, parent.width - meta.width - detail.width - parent.spacing * 2)
            }

            GameMetaLine {
                id: meta
                anchors.bottom: title.bottom
                anchors.bottomMargin: Theme.dp(6)
                game: root.game
                showYear: false
            }

            Text {
                id: detail
                anchors.bottom: title.bottom
                anchors.bottomMargin: Theme.dp(6)
                visible: text !== ""
                text: root.detail !== "" ? "·  " + root.detail : ""
                color: Theme.textMuted
                font.family: Theme.sans
                font.pixelSize: Theme.dp(24)
            }
        }
    }
}
