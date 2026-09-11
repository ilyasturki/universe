import QtQuick
import "../core"

Item {
    id: root

    // [{ glyph: "A", label: "Launch" }, ...]
    property var hints: []
    property bool showClock: false
    property real sideMargin: Theme.dp(Theme.edgeMargin)

    implicitHeight: Theme.dp(Theme.hintBarHeight)

    Row {
        anchors.left: parent.left
        anchors.leftMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(40)

        Repeater {
            model: root.hints

            Row {
                spacing: Theme.dp(12)

                ButtonGlyph {
                    glyph: modelData.glyph
                    anchors.verticalCenter: parent.verticalCenter
                }

                Text {
                    text: modelData.label
                    color: Theme.textHint
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(21)
                    anchors.verticalCenter: parent.verticalCenter
                }
            }
        }
    }

    Text {
        visible: root.showClock
        anchors.right: parent.right
        anchors.rightMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        text: Theme.clock
        color: Theme.textSecondary
        font.family: Theme.sans
        font.weight: Font.Medium
        font.pixelSize: Theme.dp(22)
    }
}
