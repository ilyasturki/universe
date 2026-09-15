import QtQuick
import "../core"

Item {
    id: bar

    property var hints: []
    property string key: ""
    property bool hairline: false
    property color ink: Theme.text
    property color muted: Theme.textDisabled
    property color glyphFill: Theme.glyphFill
    property color glyphInk: Theme.glyphInk

    implicitHeight: Theme.dp(Theme.hintBarHeight)

    onHintsChanged: {
        var k = JSON.stringify(hints);
        if (k !== key) {
            key = k;
            rep.model = hints;
        }
    }

    Hairline {
        anchors.bottom: undefined
        anchors.top: parent.top
        anchors.leftMargin: Theme.dp(Theme.edgeMargin)
        anchors.rightMargin: Theme.dp(Theme.edgeMargin)
        visible: bar.hairline
        color: Theme.hairline
    }

    Item {
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(78)
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.dp(72)
        height: Theme.dp(60)

        readonly property bool connected: api.screens.controller.connected

        Row {
            anchors.left: parent.left
            anchors.top: parent.top
            spacing: Theme.dp(4)
            Repeater {
                model: 4
                Rectangle {
                    width: Theme.dp(9)
                    height: Theme.dp(9)
                    radius: Theme.dp(1.5)
                    color: index === 0 && parent.parent.connected ? Theme.okGreen : bar.muted
                }
            }
        }

        Glyph {
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.topMargin: Theme.dp(12)
            width: Theme.dp(56)
            height: Theme.dp(56)
            kind: "gamepad"
            tint: parent.connected ? bar.ink : bar.muted
        }
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(96)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(42)

        Repeater {
            id: rep

            Row {
                spacing: Theme.dp(14)

                readonly property var hint: modelData

                Repeater {
                    model: String(hint.glyph).split(" ")

                    HintGlyph {
                        anchors.verticalCenter: parent.verticalCenter
                        glyph: modelData
                        dim: hint.dim === true
                        fill: bar.glyphFill
                        ink: bar.glyphInk
                    }
                }

                Label {
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label
                    color: modelData.dim === true ? bar.muted : bar.ink
                }
            }
        }
    }
}
