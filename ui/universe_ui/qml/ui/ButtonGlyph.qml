import QtQuick
import "../core"
import "PadNames.js" as Names

// glyph: "A" | "LB RB" (alternatives) | "Start+Select" (a chord) | "dpad", named the Xbox way.
Item {
    id: root

    property string glyph: "A"
    readonly property bool chord: glyph.indexOf("+") >= 0
    readonly property var names: glyph.split(chord ? "+" : " ")
    readonly property string family: api.screens.controller.family
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
