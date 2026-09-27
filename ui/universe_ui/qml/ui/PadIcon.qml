import QtQuick
import "PadDraw.js" as Draw
import "PadGeometry.js" as Geometry

Canvas {
    id: icon

    property string family: ""
    property color tint: "#ffffff"

    readonly property var geo: Geometry.of(family)
    readonly property var box: Geometry.bounds(geo.body)
    readonly property string mark: {
        if (Geometry.SONY_KINDS[family])
            return "touchpad";
        if (family === "xbox" || family === "xbox-elite")
            return "circle";
        if (family === "8bitdo-pro-3")
            return "star";
        if (family === "switch-pro")
            return "square";
        if (family === "steam-deck")
            return "screen";
        return "";
    }

    onFamilyChanged: requestPaint()
    onTintChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d");
        ctx.reset();
        var line = width / 12;
        var w = box[2] - box[0], h = box[3] - box[1];
        var k = Math.min((width - line) / w, (height - line) / h);
        if (k <= 0)
            return;
        ctx.translate((width - w * k) / 2, (height - h * k) / 2);
        ctx.scale(k, k);
        ctx.translate(-box[0], -box[1]);
        ctx.fillStyle = tint;
        ctx.strokeStyle = tint;
        ctx.lineWidth = line / k;
        ctx.lineJoin = "round";
        ctx.path = geo.body;
        ctx.stroke();
        var cx = (box[0] + box[2]) / 2, cy = box[1] + h * 0.36;
        if (mark === "touchpad") {
            ctx.path = geo.details[0].path;
            ctx.fill();
        } else if (mark === "circle") {
            Draw.circle(ctx, cx, cy, h * 0.21);
            ctx.fill();
        } else if (mark === "star") {
            ctx.save();
            ctx.translate(cx, cy);
            ctx.scale(h * 0.27, h * 0.27);
            ctx.path = Draw.STAR;
            ctx.fill();
            ctx.restore();
        } else if (mark === "square") {
            Draw.roundRect(ctx, cx - h * 0.17, cy - h * 0.17, h * 0.34, h * 0.34, h * 0.05);
            ctx.fill();
        } else if (mark === "screen") {
            ctx.path = geo.details[0].path;
            ctx.fill();
        }
    }
}
