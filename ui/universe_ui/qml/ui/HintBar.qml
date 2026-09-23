import QtQuick
import "../core"
import "Hints.js" as Hints

Item {
    id: root

    // [{ glyph: "A", label: "Launch", dim: false }, ...]
    property var hints: []
    property real sideMargin: Theme.dp(Theme.edgeMargin)

    readonly property var arranged: Hints.arrange(hints)

    implicitHeight: Theme.dp(Theme.hintBarHeight)

    // The glyph delegates are rebuilt only when the glyph set changes; a label change repaints its Text.
    component HintRow: Row {
        property var hints: []
        property var glyphs: []

        spacing: Theme.dp(40)

        onHintsChanged: {
            var g = hints.map(function (h) {
                return h.glyph;
            });
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
        id: leftHints
        anchors.left: parent.left
        anchors.leftMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        hints: root.arranged.left
    }

    HintRow {
        id: rightHints
        anchors.right: parent.right
        anchors.rightMargin: root.sideMargin
        anchors.verticalCenter: parent.verticalCenter
        hints: root.arranged.right
    }

    // Centred on the bar, pushed aside and then elided by the hints, never over them.
    NoticePill {
        readonly property real from: leftHints.width > 0 ? leftHints.x + leftHints.width + Theme.dp(40) : root.sideMargin
        readonly property real to: rightHints.width > 0 ? rightHints.x - Theme.dp(40) : root.width - root.sideMargin

        maxWidth: Math.max(0, to - from)
        x: Math.max(from, Math.min((root.width - width) / 2, to - width))
        anchors.verticalCenter: parent.verticalCenter
    }
}
