import QtQuick
import "../core"
import "PadNames.js" as Names
import "PadDraw.js" as Draw

// One button of one family drawn small, the way the pad prints it: the rows' and the hint bar's glyph.
Item {
    id: root

    property string family: "xbox"
    property string slot: "south"
    property real unit: Theme.dp(30)
    property color ink: Theme.text
    property real outline: 0.55

    readonly property var spec: Names.glyph(family, slot)
    readonly property real boxWidth: spec.shape === "bumper" ? unit * 1.3
        : spec.shape === "trigger" ? unit * 0.9
        : spec.shape === "tab" ? unit * 1.1
        : spec.shape === "paddle" ? unit * 0.78
        : spec.shape === "small" ? unit * 0.84
        : unit
    readonly property real boxHeight: spec.shape === "bumper" ? unit * 0.6
        : spec.shape === "tab" ? unit * 0.6
        : spec.shape === "small" ? unit * 0.84
        : unit
    readonly property real textSize: unit * (spec.text.length >= 3 ? 0.33 : spec.text.length === 2 ? 0.4 : 0.48)

    implicitWidth: boxWidth
    implicitHeight: unit

    Canvas {
        id: canvas

        anchors.centerIn: parent
        width: root.boxWidth
        height: root.boxHeight

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var w = width, h = height, line = Math.max(1, Theme.dp(2));
            var inset = line / 2 + 0.5;
            ctx.lineWidth = line;
            ctx.lineCap = "round";
            ctx.lineJoin = "round";
            ctx.strokeStyle = Qt.rgba(root.ink.r, root.ink.g, root.ink.b, root.outline);
            ctx.fillStyle = Qt.rgba(root.ink.r, root.ink.g, root.ink.b, root.outline);
            var shape = root.spec.shape;
            if (shape === "circle" || shape === "stick") {
                Draw.circle(ctx, w / 2, h / 2, w / 2 - inset);
                ctx.stroke();
            } else if (shape === "dpad") {
                Draw.cross(ctx, w / 2, h / 2, w / 2 - inset, w * 0.3);
                ctx.stroke();
                if (root.spec.dir !== "") {
                    Draw.arm(ctx, w / 2, h / 2, w / 2 - inset, w * 0.3 - line, root.spec.dir);
                    ctx.fillStyle = root.ink;
                    ctx.fill();
                }
            } else if (shape === "trigger") {
                Draw.roundRect(ctx, inset, inset, w - inset * 2, h - inset * 2, w * 0.3);
                ctx.stroke();
            } else if (shape === "paddle") {
                Draw.paddle(ctx, w / 2, inset, w - inset * 2, h - inset * 2);
                ctx.stroke();
            } else {
                Draw.roundRect(ctx, inset, inset, w - inset * 2, h - inset * 2, shape === "small" ? h * 0.26 : h * 0.4);
                ctx.stroke();
            }
            if (root.spec.symbol !== "") {
                ctx.strokeStyle = root.ink;
                ctx.fillStyle = root.ink;
                ctx.lineWidth = Math.max(1, Theme.dp(1.8));
                Draw.symbol(ctx, root.spec.symbol, w / 2, h / 2, Math.min(w, h) / 2 - inset);
            }
        }

        Connections {
            target: root
            function onSpecChanged() { canvas.requestPaint(); }
            function onInkChanged() { canvas.requestPaint(); }
            function onOutlineChanged() { canvas.requestPaint(); }
        }
    }

    Text {
        anchors.centerIn: canvas
        visible: root.spec.symbol === "" && root.spec.text !== ""
        text: root.spec.text
        color: root.ink
        font.family: Theme.sans
        font.weight: Font.DemiBold
        font.pixelSize: root.textSize
    }
}
