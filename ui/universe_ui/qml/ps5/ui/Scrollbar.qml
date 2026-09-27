import QtQuick
import "../core"

Item {
    id: bar

    property Flickable flickable: null

    readonly property real span: flickable ? flickable.contentHeight : 0
    readonly property bool needed: flickable !== null && span > flickable.height + 1

    width: Theme.dp(3)
    visible: needed

    Rectangle {
        anchors.fill: parent
        radius: width / 2
        color: Qt.rgba(1, 1, 1, 0.08)
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        y: bar.needed ? bar.height * flickable.contentY / bar.span : 0
        height: bar.needed ? Math.max(Theme.dp(40), bar.height * flickable.height / bar.span) : 0
        radius: width / 2
        color: Qt.rgba(1, 1, 1, 0.5)

        Behavior on y {
            Ease {}
        }
    }
}
