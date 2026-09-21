import QtQuick
import "../core"
import "PadNames.js" as Names

// glyph: "A" | "LB RB" (alternatives) | "Start+Select" (a chord) | "dpad", named the Xbox way.
// Under a keyboard or mouse a button with a key becomes that key's cap; the d-pad the arrows, a chord stays the pad's.
Item {
    id: root

    property string glyph: "A"
    readonly property bool chord: glyph.indexOf("+") >= 0
    readonly property var names: glyph.split(chord ? "+" : " ")
    readonly property string family: api.screens.controller.family
    readonly property real unit: Theme.dp(30)
    readonly property bool keyed: api.keys.mode !== "pad" && !chord

    function keyLabel(name) {
        return name === "dpad" ? "↑↓←→" : (api.keys.labels[name] || "");
    }

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
                    visible: !cap.visible
                    family: root.family
                    slot: Names.hintSlot(modelData, root.family)
                    unit: root.unit
                }

                Rectangle {
                    id: cap

                    readonly property string label: root.keyLabel(modelData)

                    anchors.verticalCenter: parent.verticalCenter
                    visible: root.keyed && label !== ""
                    width: Math.max(root.unit, capText.implicitWidth + root.unit * 0.5)
                    height: root.unit
                    radius: root.unit * 0.22
                    color: Theme.surface
                    border.width: 1
                    border.color: Theme.surfaceBorder

                    Text {
                        id: capText
                        anchors.centerIn: parent
                        text: cap.label
                        color: Theme.textHint
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: root.unit * (cap.label.length > 3 ? 0.42 : 0.5)
                    }
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
