import QtQuick
import "../core"
import "PadNames.js" as Names

// The hint bar's glyph: "A" | "B" | "X" | "Y" | "RS" | "LB RB" | "LT RT" | "Start Select" | "dpad",
// named the Xbox way and drawn the way the connected pad prints them.
Item {
    id: root

    property string glyph: "A"
    readonly property var names: glyph.split(" ")
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

            PadGlyph {
                anchors.verticalCenter: parent.verticalCenter
                family: root.family
                slot: Names.hintSlot(modelData)
                unit: root.unit
            }
        }
    }
}
