import QtQuick
import "../core"
import "PadNames.js" as Names

// The hint bar's glyph: "A" | "B" | "X" | "Y" | "RS" | "LB RB" | "LT RT" | "Start+Select" | "dpad",
// named the Xbox way and drawn the way the connected pad prints them. A space lists buttons that
// each do the thing; a "+" joins the ones pressed together, and is drawn between them.
Item {
    id: root

    property string glyph: "A"
    readonly property bool chord: glyph.indexOf("+") >= 0
    readonly property var names: glyph.split(chord ? "+" : " ")
    readonly property string family: api.screens.controller.connected ? api.screens.controller.family : "xbox"
    readonly property real unit: Theme.dp(30)

    implicitHeight: unit
    implicitWidth: row.width

    Row {
        id: row

        anchors.centerIn: parent
        spacing: root.unit * 0.27

        Repeater {
            model: root.names

            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: root.unit * 0.27

                PadGlyph {
                    anchors.verticalCenter: parent.verticalCenter
                    family: root.family
                    slot: Names.hintSlot(modelData)
                    unit: root.unit
                }

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.chord && index < root.names.length - 1
                    text: "+"
                    color: Theme.textHint
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: root.unit * 0.7
                }
            }
        }
    }
}
