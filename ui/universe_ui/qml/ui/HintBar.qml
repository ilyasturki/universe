import QtQuick
import "../core"
import "Hints.js" as Hints

Item {
    id: root

    // [{ glyph: "A", label: "Launch", dim: false }, ...]
    property var hints: []
    property bool showClock: false
    property real sideMargin: Theme.dp(Theme.edgeMargin)

    readonly property var arranged: Hints.arrange(hints)

    implicitHeight: Theme.dp(Theme.hintBarHeight)

    component HintRow: Row {
        property var model: []

        spacing: Theme.dp(40)

        Repeater {
            model: parent.model

            Row {
                spacing: Theme.dp(12)
                opacity: modelData.dim === true ? 0.35 : 1.0

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

    HintRow {
        anchors.left: parent.left
        anchors.leftMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        model: root.arranged.left
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(48)

        HintRow {
            anchors.verticalCenter: parent.verticalCenter
            model: root.arranged.right
        }

        Text {
            visible: root.showClock
            anchors.verticalCenter: parent.verticalCenter
            text: Theme.clock
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
        }
    }
}
