import QtQuick
import "../core"

Column {
    id: root

    property var game: null
    property string label: ""

    spacing: Theme.dp(4)

    CapsLabel {
        text: root.label
    }

    Text {
        width: parent.width
        text: root.game ? root.game.title : ""
        color: Theme.text
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: Theme.dp(46)
        elide: Text.ElideRight
    }
}
