import QtQuick
import "../core"
import "../../ui" as Pad
import "../../ui/PadNames.js" as Names

Item {
    id: root
    objectName: "hintGlyph"

    property string glyph: "A"
    property real unit: Theme.dp(34)
    property color fill: Theme.text
    property color ink: Theme.onLight
    property bool dim: false

    readonly property string family: Theme.padFamily
    readonly property string slot: Names.hintSlot(glyph, family)
    readonly property bool cross: slot.indexOf("dpad") === 0

    implicitWidth: Math.max(unit, back.implicitWidth)
    implicitHeight: unit
    opacity: dim ? 0.35 : 1.0

    Pad.PadGlyph {
        id: back

        anchors.centerIn: parent
        unit: root.unit
        family: root.family
        slot: root.slot
        ink: root.cross ? root.fill : root.ink
        variant: "o"
    }

    Pad.PadGlyph {
        anchors.centerIn: parent
        visible: !root.cross
        unit: root.unit
        family: root.family
        slot: root.slot
        ink: root.fill
        variant: "f"
    }
}
