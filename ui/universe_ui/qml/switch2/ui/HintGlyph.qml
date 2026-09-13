import QtQuick
import "../core"
import "../../ui/PadNames.js" as Names

Item {
    id: root

    property string glyph: "A"
    property real unit: Theme.dp(42)
    property color fill: Theme.glyphFill
    property color ink: Theme.glyphInk
    property bool dim: false

    readonly property string family: api.screens.controller.connected ? api.screens.controller.family : "xbox"
    readonly property string slot: Names.hintSlot(glyph)
    readonly property var spec: Names.glyph(family, slot)
    readonly property bool pill: spec.shape === "bumper" || spec.shape === "trigger"
    readonly property bool cross: spec.shape === "dpad"
    readonly property string letter: spec.text

    implicitWidth: pill ? unit * 1.45 : unit
    implicitHeight: unit
    opacity: dim ? 0.35 : 1.0

    Rectangle {
        anchors.fill: parent
        radius: root.pill ? root.unit * 0.3 : root.unit / 2
        color: root.fill
        visible: !root.cross
    }

    Text {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: -root.unit * 0.02
        visible: root.spec.symbol === "" && root.letter !== ""
        text: root.letter
        color: root.ink
        font.family: Theme.sans
        font.weight: Font.Bold
        font.pixelSize: root.unit * (root.letter.length > 1 ? 0.4 : 0.58)
    }

    Canvas {
        anchors.fill: parent
        visible: root.spec.symbol !== "" || root.spec.shape === "dpad"
        onVisibleChanged: requestPaint()
        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var w = width, h = height, c = w / 2, m = h / 2, r = Math.min(w, h) * 0.24;
            ctx.strokeStyle = root.cross ? root.fill : root.ink;
            ctx.fillStyle = root.cross ? root.fill : root.ink;
            ctx.lineWidth = Math.max(1.5, h * 0.09);
            ctx.lineCap = "round";
            ctx.lineJoin = "round";
            var sym = root.spec.symbol;
            if (root.spec.shape === "dpad") {
                var a = h * 0.3, l = h * 0.47, g = a * 0.12;
                ctx.beginPath();
                ctx.moveTo(c - a / 2, m - l);
                ctx.lineTo(c + a / 2, m - l);
                ctx.lineTo(c + a / 2, m - a / 2 - g);
                ctx.lineTo(c + l, m - a / 2);
                ctx.lineTo(c + l, m + a / 2);
                ctx.lineTo(c + a / 2, m + a / 2 + g);
                ctx.lineTo(c + a / 2, m + l);
                ctx.lineTo(c - a / 2, m + l);
                ctx.lineTo(c - a / 2, m + a / 2 + g);
                ctx.lineTo(c - l, m + a / 2);
                ctx.lineTo(c - l, m - a / 2);
                ctx.lineTo(c - a / 2, m - a / 2 - g);
                ctx.closePath();
                ctx.fill();
            } else if (sym === "plus" || sym === "menu") {
                ctx.beginPath(); ctx.moveTo(c - r, m); ctx.lineTo(c + r, m); ctx.moveTo(c, m - r); ctx.lineTo(c, m + r); ctx.stroke();
            } else if (sym === "minus" || sym === "view" || sym === "create") {
                ctx.beginPath(); ctx.moveTo(c - r, m); ctx.lineTo(c + r, m); ctx.stroke();
            } else if (sym === "cross") {
                ctx.beginPath(); ctx.moveTo(c - r, m - r); ctx.lineTo(c + r, m + r); ctx.moveTo(c + r, m - r); ctx.lineTo(c - r, m + r); ctx.stroke();
            } else if (sym === "circle") {
                ctx.beginPath(); ctx.arc(c, m, r, 0, Math.PI * 2); ctx.stroke();
            } else if (sym === "triangle") {
                ctx.beginPath(); ctx.moveTo(c, m - r * 1.05); ctx.lineTo(c + r * 1.1, m + r * 0.8); ctx.lineTo(c - r * 1.1, m + r * 0.8); ctx.closePath(); ctx.stroke();
            } else if (sym === "square") {
                ctx.beginPath(); ctx.rect(c - r * 0.9, m - r * 0.9, r * 1.8, r * 1.8); ctx.stroke();
            } else if (sym === "home") {
                ctx.beginPath(); ctx.moveTo(c - r, m); ctx.lineTo(c, m - r); ctx.lineTo(c + r, m); ctx.lineTo(c + r * 0.7, m); ctx.lineTo(c + r * 0.7, m + r); ctx.lineTo(c - r * 0.7, m + r); ctx.lineTo(c - r * 0.7, m); ctx.closePath(); ctx.stroke();
            } else if (sym === "xbox") {
                ctx.beginPath(); ctx.arc(c, m, r, 0, Math.PI * 2); ctx.stroke();
                ctx.beginPath(); ctx.moveTo(c - r * 0.6, m - r * 0.6); ctx.lineTo(c + r * 0.6, m + r * 0.6); ctx.moveTo(c + r * 0.6, m - r * 0.6); ctx.lineTo(c - r * 0.6, m + r * 0.6); ctx.stroke();
            }
        }
    }
}
