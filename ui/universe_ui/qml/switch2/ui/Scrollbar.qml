import QtQuick
import "../core"

Item {
    id: bar

    property Flickable flickable: null

    readonly property real span: flickable ? flickable.contentHeight : 0
    readonly property bool needed: flickable && span > flickable.height + 1

    width: Theme.dp(6)
    visible: needed

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        y: bar.needed ? bar.height * flickable.contentY / bar.span : 0
        height: bar.needed ? Math.max(Theme.dp(40), bar.height * flickable.height / bar.span) : 0
        radius: width / 2
        color: Theme.thumb

        Behavior on y {
            Ease {}
        }
    }
}
