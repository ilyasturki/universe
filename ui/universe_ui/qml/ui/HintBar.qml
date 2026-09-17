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

    // The glyph delegates are rebuilt only when the glyph set changes; a label change repaints its Text.
    component HintRow: Row {
        property var hints: []
        property var glyphs: []

        spacing: Theme.dp(40)

        onHintsChanged: {
            var g = hints.map(function(h) { return h.glyph; });
            if (g.join() !== glyphs.join())
                glyphs = g;
        }

        Repeater {
            model: parent.glyphs

            Row {
                readonly property var hint: parent.hints[index] || ({})

                spacing: Theme.dp(12)
                opacity: hint.dim === true ? 0.35 : 1.0

                ButtonGlyph {
                    glyph: modelData
                    anchors.verticalCenter: parent.verticalCenter
                }

                Text {
                    text: hint.label || ""
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
        hints: root.arranged.left
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(48)

        HintRow {
            anchors.verticalCenter: parent.verticalCenter
            hints: root.arranged.right
        }

        PowerBadge {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.showClock && api.power.count > 0
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
