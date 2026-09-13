import QtQuick
import "../core"
import "PadDraw.js" as Draw
import "PadGeometry.js" as Geometry

// The pad drawn from its geometry, scaled to fit: the body on one canvas, a PadButton per button
// over it, fed the focused, pressed, learning and unbound slots and the live axes.
Item {
    id: art

    property string family: "dualsense"
    property string focusedSlot: ""
    property string learningSlot: ""
    property var unbound: []
    property var pressed: ({})
    property var axes: ({})
    property real pulse: 0.15

    readonly property var geo: Geometry.of(family)
    readonly property real k: Math.max(0.01, Math.min(width / geo.view[0], height / geo.view[1]))
    readonly property real ox: (width - geo.view[0] * k) / 2
    readonly property real oy: (height - geo.view[1] * k) / 2

    Canvas {
        id: body

        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        Connections {
            target: art
            function onGeoChanged() { body.requestPaint(); }
        }

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var k = art.k;
            if (k <= 0.01)
                return;
            ctx.translate(art.ox, art.oy);
            ctx.scale(k, k);
            ctx.lineJoin = "round";
            ctx.lineCap = "round";
            var top = 90, bottom = 680;
            var shade = ctx.createLinearGradient(0, top, 0, bottom);
            shade.addColorStop(0, Qt.rgba(1, 1, 1, 0.115));
            shade.addColorStop(0.55, Qt.rgba(1, 1, 1, 0.075));
            shade.addColorStop(1, Qt.rgba(1, 1, 1, 0.045));
            ctx.path = art.geo.body;
            ctx.fillStyle = shade;
            ctx.fill();
            ctx.lineWidth = 2.2 / k;
            ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.26);
            ctx.stroke();
            // A rim light along the top edge.
            ctx.save();
            ctx.path = art.geo.body;
            ctx.clip();
            var rim = ctx.createLinearGradient(0, top, 0, top + 70);
            rim.addColorStop(0, Qt.rgba(1, 1, 1, 0.10));
            rim.addColorStop(1, Qt.rgba(1, 1, 1, 0.0));
            ctx.fillStyle = rim;
            ctx.fillRect(0, top, 1000, 70);
            ctx.restore();
            var details = art.geo.details;
            for (var i = 0; i < details.length; i++) {
                var d = details[i];
                ctx.beginPath();
                if (d.kind === "panel") {
                    var x = d.x - d.w / 2, y = d.y - d.h / 2, r = d.r;
                    ctx.moveTo(x + r, y);
                    ctx.lineTo(x + d.w - r, y);
                    ctx.arcTo(x + d.w, y, x + d.w, y + r, r);
                    ctx.lineTo(x + d.w, y + d.h - r);
                    ctx.arcTo(x + d.w, y + d.h, x + d.w - r, y + d.h, r);
                    ctx.lineTo(x + r, y + d.h);
                    ctx.arcTo(x, y + d.h, x, y + d.h - r, r);
                    ctx.lineTo(x, y + r);
                    ctx.arcTo(x, y, x + r, y, r);
                    ctx.closePath();
                    ctx.fillStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.05);
                    ctx.fill();
                    ctx.lineWidth = 1.6 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.2);
                    ctx.stroke();
                } else if (d.kind === "dish") {
                    ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
                    ctx.fillStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.035);
                    ctx.fill();
                    ctx.lineWidth = 1.4 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.14);
                    ctx.stroke();
                } else if (d.kind === "cross") {
                    Draw.cross(ctx, d.x, d.y, d.l, d.a);
                    ctx.fillStyle = Qt.rgba(1, 1, 1, 0.14);
                    ctx.fill();
                    ctx.lineWidth = 1.6 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.36);
                    ctx.stroke();
                } else if (d.kind === "dot") {
                    ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
                    ctx.fillStyle = Qt.rgba(1, 1, 1, 0.08);
                    ctx.fill();
                    ctx.lineWidth = 1.4 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.22);
                    ctx.stroke();
                } else if (d.kind === "line") {
                    ctx.path = d.path;
                    ctx.lineWidth = (d.width || 2) / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.2);
                    ctx.stroke();
                }
            }
        }
    }

    Repeater {
        model: art.geo.buttons

        PadButton {
            readonly property string slot: modelData.slot
            readonly property var stick: modelData.axes || null

            spec: modelData
            family: art.family
            k: art.k
            ox: art.ox
            oy: art.oy
            lit: art.focusedSlot === slot
            down: art.pressed[slot] === true
            learning: art.learningSlot === slot
            missing: art.unbound.indexOf(slot) !== -1
            pulse: art.pulse
            pull: slot === "lt" || slot === "rt" ? (art.axes[slot] || 0) : 0
            leanX: stick ? (art.axes[stick[0]] || 0) : 0
            leanY: stick ? (art.axes[stick[1]] || 0) : 0
        }
    }
}
