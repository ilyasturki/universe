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
        x: Theme.dp(93)
        y: Theme.dp(10)
        width: Theme.dp(78)
        height: Theme.dp(78)

        readonly property bool connected: api.screens.controller.connected

        Row {
            x: Theme.dp(9)
            spacing: Theme.dp(6)
            Repeater {
                model: 4
                Rectangle {
                    width: Theme.dp(10)
                    height: Theme.dp(10)
                    radius: Theme.dp(1.5)
                    color: index === 0 && parent.parent.connected ? "#00ae02" : "#b2b2b2"
                }
            }
        }

        Glyph {
            y: Theme.dp(-3)
            width: parent.width
            height: parent.height
            kind: "gamepad"
            tint: parent.connected ? "#242424" : bar.muted
        }
    }

    Row {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(96)
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: Theme.dp(-7)
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
                        unit: Theme.dp(38)
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
