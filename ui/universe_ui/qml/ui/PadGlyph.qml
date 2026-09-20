import QtQuick
import QtQuick.Shapes
import "../core"
import "PadNames.js" as Names
import "PadDraw.js" as Draw
import "Prompts.js" as Prompts

Item {
    id: root

    property string family: "xbox"
    property string slot: "south"
    property real unit: Theme.dp(30)
    property color ink: Theme.text
    // Prompts.js: "o" the outline, "f" the disc with the mark cut out.
    property string variant: "o"

    readonly property var spec: Names.glyph(family, slot)
    readonly property var art: Prompts.paths(Names.style(family), slot)
    // The sheet's 48-unit face button is `unit` on screen.
    readonly property real sheetScale: unit / 48
    readonly property real artWidth: art ? (art.b[2] - art.b[0]) * sheetScale : 0
    readonly property real artHeight: art ? (art.b[3] - art.b[1]) * sheetScale : 0

    readonly property real boxWidth: art ? artWidth : spec.shape === "bumper" ? unit * 1.3 : spec.shape === "trigger" ? unit * 0.9 : spec.shape === "tab" ? unit * 1.1 : spec.shape === "paddle" ? unit * 0.78 : spec.shape === "small" ? unit * 0.84 : unit
    readonly property real boxHeight: art ? artHeight : spec.shape === "bumper" ? unit * 0.6 : spec.shape === "tab" ? unit * 0.6 : spec.shape === "small" ? unit * 0.84 : unit
    readonly property real textSize: unit * (spec.text.length >= 3 ? 0.33 : spec.text.length === 2 ? 0.4 : 0.48)

    onSpecChanged: canvas.requestPaint()
    onInkChanged: canvas.requestPaint()

    implicitWidth: boxWidth
    implicitHeight: unit

    Shape {
        id: shape

        visible: root.art !== null
        width: 64
        height: 64
        x: root.art ? (root.width - root.artWidth) / 2 - root.art.b[0] * root.sheetScale : 0
        y: root.art ? (root.height - root.artHeight) / 2 - root.art.b[1] * root.sheetScale : 0
        transformOrigin: Item.TopLeft
        scale: root.sheetScale
        preferredRendererType: Shape.CurveRenderer
        // The software scenegraph (offscreen tests) paints a Shape past its parents' clip; a layer clips it.
        layer.enabled: GraphicsInfo.api === GraphicsInfo.Software
        layer.smooth: true

        ShapePath {
            fillColor: root.ink
            strokeColor: "transparent"
            fillRule: ShapePath.OddEvenFill

            PathSvg {
                path: root.art ? (root.variant === "f" ? root.art.f : root.art.o) : ""
            }
        }
    }

    Canvas {
        id: canvas

        visible: root.art === null
        anchors.centerIn: parent
        width: root.boxWidth
        height: root.boxHeight

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            if (root.art)
                return;
            var w = width, h = height, line = Math.max(1, Theme.dp(2));
            var inset = line / 2 + 0.5;
            ctx.lineWidth = line;
            ctx.lineCap = "round";
            ctx.lineJoin = "round";
            ctx.strokeStyle = root.ink;
            ctx.fillStyle = root.ink;
            var shape = root.spec.shape;
            if (shape === "circle" || shape === "stick") {
                Draw.circle(ctx, w / 2, h / 2, w / 2 - inset);
                ctx.stroke();
            } else if (shape === "dpad") {
                Draw.cross(ctx, w / 2, h / 2, w / 2 - inset, w * 0.3);
                ctx.stroke();
                if (root.spec.dir !== "") {
                    Draw.arm(ctx, w / 2, h / 2, w / 2 - inset, w * 0.3 - line, root.spec.dir);
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
                ctx.lineWidth = Math.max(1, Theme.dp(1.8));
                Draw.symbol(ctx, root.spec.symbol, w / 2, h / 2, Math.min(w, h) / 2 - inset);
            }
        }
    }

    Text {
        anchors.centerIn: canvas
        visible: root.art === null && root.spec.symbol === "" && root.spec.text !== ""
        text: root.spec.text
        color: root.ink
        font.family: Theme.sans
        font.weight: Font.DemiBold
        font.pixelSize: root.textSize
    }
}
