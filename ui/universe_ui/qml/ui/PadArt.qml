import QtQuick
import "../core"
import "PadNames.js" as Names
import "PadDraw.js" as Draw
import "PadGeometry.js" as Geometry

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
                    Draw.roundRect(ctx, d.x - d.w / 2, d.y - d.h / 2, d.w, d.h, d.r);
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

    component PadButton: Item {
        id: button

        property var spec: ({})
        property bool lit: false
        property bool down: false
        property bool learning: false
        property bool missing: false
        property real pull: 0
        property real leanX: 0
        property real leanY: 0

        readonly property var glyph: Names.glyph(art.family, spec.slot || "")
        readonly property var box: Geometry.box(spec)
        // Room around the shape for the focus halo and a leaning stick cap.
        readonly property real margin: Theme.dp(10)
        readonly property real ink: 0.9
        readonly property color onDown: Theme.onLight

        property real glow: down ? 1.0 : 0.0
        Behavior on glow { Ease { duration: button.down ? 30 : Theme.durQuick } }
        Behavior on leanX { NumberAnimation { duration: 60 } }
        Behavior on leanY { NumberAnimation { duration: 60 } }
        Behavior on pull { NumberAnimation { duration: 60 } }

        x: art.ox + box.x * art.k - margin
        y: art.oy + box.y * art.k - margin
        width: box.w * art.k + margin * 2
        height: box.h * art.k + margin * 2

        function mix(a, b, t) { return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t); }

        // A trigger's mark goes dark once its pull has filled past the middle, where the mark sits.
        readonly property color markColor: mix(Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, ink), onDown,
                                               spec.kind === "trigger" ? Math.max(glow, pull >= 0.5 ? 1 : 0) : glow)
        readonly property real lean: 0.5

        readonly property var repaintKey: [glow, lit, learning, missing, learning ? art.pulse : 0, pull, leanX, leanY, art.k, glyph]
        onRepaintKeyChanged: canvas.requestPaint()

        Canvas {
            id: canvas

            anchors.fill: parent

            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = button.spec, k = art.k, m = button.margin, g = button.glow;
                var white = Theme.text;
                var t = s.kind === "paddle" && s.ghost ? 0.10 : 0.14;
                var fillA = button.missing ? 0.05 : button.learning ? art.pulse : button.lit ? 0.26 : t;
                var strokeA = button.lit || button.learning ? 0.95 : s.ghost ? 0.5 : 0.36;
                var fill = button.mix(Qt.rgba(white.r, white.g, white.b, fillA), white, g);
                var stroke = button.mix(Qt.rgba(white.r, white.g, white.b, strokeA), white, g);
                var line = Math.max(1, Theme.dp(1.6));
                ctx.lineJoin = "round";
                ctx.lineCap = "round";
                if (button.missing && g < 0.5)
                    ctx.setLineDash([Theme.dp(4), Theme.dp(4)]);

                function halo(shapeFn) {
                    if (!button.lit)
                        return;
                    ctx.save();
                    ctx.lineWidth = Theme.dp(8);
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                    shapeFn();
                    ctx.stroke();
                    ctx.restore();
                }

                function paintPath(shapeFn) {
                    halo(shapeFn);
                    shapeFn();
                    if (!s.ghost) {
                        ctx.fillStyle = Theme.ground;
                        ctx.fill();
                    }
                    ctx.fillStyle = fill;
                    ctx.fill();
                    ctx.lineWidth = button.lit ? Theme.dp(2.5) : line;
                    ctx.strokeStyle = stroke;
                    ctx.stroke();
                }

                var w = width, h = height;
                var cx = w / 2, cy = h / 2;
                if (s.kind === "face") {
                    var r = s.r * k;
                    paintPath(function() { Draw.circle(ctx, cx, cy, r); });
                } else if (s.kind === "stick") {
                    var R = s.r * k;
                    ctx.fillStyle = Qt.rgba(white.r, white.g, white.b, 0.05);
                    ctx.strokeStyle = Qt.rgba(white.r, white.g, white.b, button.lit ? 0.5 : 0.22);
                    ctx.lineWidth = line;
                    Draw.circle(ctx, cx, cy, R);
                    ctx.fill();
                    ctx.stroke();
                    var lean = R * button.lean;
                    var px = cx + button.leanX * lean, py = cy + button.leanY * lean;
                    paintPath(function() { Draw.circle(ctx, px, py, R * 0.66); });
                    ctx.beginPath();
                    ctx.arc(px, py, R * 0.42, 0, Math.PI * 2);
                    ctx.strokeStyle = button.mix(Qt.rgba(white.r, white.g, white.b, 0.18), button.onDown, g * 0.6);
                    ctx.lineWidth = line;
                    ctx.stroke();
                } else if (s.kind === "arm") {
                    var l = s.l * k, a = s.a * k, gap = s.split ? a * 0.28 : 0;
                    var dx = s.dir === "left" ? -1 : s.dir === "right" ? 1 : 0;
                    var dy = s.dir === "up" ? -1 : s.dir === "down" ? 1 : 0;
                    if (s.split) {
                        paintPath(function() {
                            var len = l - gap - a * 0.5;
                            var bx = cx + dx * (gap + a * 0.5 + len / 2), by = cy + dy * (gap + a * 0.5 + len / 2);
                            var bw = dx !== 0 ? len : a, bh = dy !== 0 ? len : a;
                            Draw.roundRect(ctx, bx - bw / 2, by - bh / 2, bw, bh, a * 0.22);
                        });
                    } else if (g > 0.01 || button.lit || button.learning || button.missing) {
                        Draw.arm(ctx, cx, cy, l, a, s.dir);
                        ctx.fillStyle = button.lit && g < 0.01 ? Qt.rgba(white.r, white.g, white.b, 0.3) : button.learning && g < 0.01 ? Qt.rgba(white.r, white.g, white.b, art.pulse) : Qt.rgba(white.r, white.g, white.b, g);
                        ctx.fill();
                        if (button.lit || button.missing) {
                            ctx.lineWidth = button.lit ? Theme.dp(2.5) : line;
                            ctx.strokeStyle = stroke;
                            ctx.stroke();
                        }
                    }
                    var tipX = cx + dx * (l * 0.72), tipY = cy + dy * (l * 0.72), v = a * 0.16;
                    ctx.beginPath();
                    ctx.moveTo(tipX - dx * v + dy * v * 1.4, tipY - dy * v + dx * v * 1.4);
                    ctx.lineTo(tipX + dx * v * 0.6, tipY + dy * v * 0.6);
                    ctx.lineTo(tipX - dx * v - dy * v * 1.4, tipY - dy * v - dx * v * 1.4);
                    ctx.strokeStyle = button.markColor;
                    ctx.lineWidth = Math.max(1, Theme.dp(1.8));
                    ctx.stroke();
                } else if (s.kind === "trigger") {
                    var tw = s.w * k, th = s.h * k;
                    var body = function() { Draw.roundRect(ctx, cx - tw / 2, cy - th / 2, tw, th, th * 0.32); };
                    halo(body);
                    body();
                    ctx.fillStyle = Theme.ground;
                    ctx.fill();
                    ctx.fillStyle = Qt.rgba(white.r, white.g, white.b, fillA);
                    ctx.fill();
                    if (button.pull > 0.005) {
                        ctx.save();
                        body();
                        ctx.clip();
                        ctx.fillStyle = Qt.rgba(white.r, white.g, white.b, 0.92);
                        ctx.fillRect(cx - tw / 2, cy + th / 2 - th * button.pull, tw, th * button.pull);
                        ctx.restore();
                    }
                    body();
                    ctx.lineWidth = button.lit || g > 0.5 ? Theme.dp(2.5) : line;
                    ctx.strokeStyle = stroke;
                    ctx.stroke();
                } else if (s.kind === "paddle") {
                    var pw = s.w * k, ph = s.h * k;
                    paintPath(function() { Draw.paddle(ctx, cx, m, pw, ph); });
                } else {
                    var bw2 = s.w * k, bh2 = s.h * k;
                    var rr = s.round ? Math.min(bw2, bh2) / 2 : s.kind === "bumper" ? bh2 * 0.45 : Math.min(bw2, bh2) * 0.3;
                    ctx.save();
                    if (s.angle) {
                        ctx.translate(cx, cy);
                        ctx.rotate(s.angle * Math.PI / 180);
                        ctx.translate(-cx, -cy);
                    }
                    paintPath(function() { Draw.roundRect(ctx, cx - bw2 / 2, cy - bh2 / 2, bw2, bh2, rr); });
                    ctx.restore();
                }

                if (button.glyph.symbol !== "" && !s.plain || s.plain && button.glyph.symbol !== "" && s.kind === "face") {
                    ctx.setLineDash([]);
                    ctx.strokeStyle = button.markColor;
                    ctx.fillStyle = button.markColor;
                    ctx.lineWidth = Math.max(1, Theme.dp(1.8));
                    var sr = s.kind === "face" ? s.r * k : s.kind === "small" ? Math.min(s.w, s.h) * k / 2 : Math.min(w, h) / 2 - m;
                    if (s.kind === "small" && !s.round && Math.max(s.w, s.h) > Math.min(s.w, s.h) * 1.6)
                        sr = Math.min(s.w, s.h) * k * 0.5;
                    if (s.kind === "small" && s.angle)
                        sr = 0;
                    if (sr > 0)
                        Draw.symbol(ctx, button.glyph.symbol, cx, cy, sr);
                }
            }
        }

        Text {
            anchors.centerIn: parent
            anchors.horizontalCenterOffset: button.spec.kind === "stick" ? button.leanX * button.spec.r * art.k * button.lean : 0
            anchors.verticalCenterOffset: button.spec.kind === "stick" ? button.leanY * button.spec.r * art.k * button.lean : 0
            visible: button.glyph.symbol === "" && button.glyph.text !== "" && button.spec.kind !== "arm"
            text: button.glyph.text
            color: button.markColor
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Math.max(8, Math.round((button.spec.kind === "face" ? button.spec.r * 1.05
                : button.spec.kind === "stick" ? button.spec.r * 0.42
                : button.spec.kind === "paddle" ? button.spec.w * 0.42
                : button.spec.kind === "arm" ? 10
                : Math.min(button.spec.w, button.spec.h) * (button.glyph.text.length > 1 ? 0.62 : 0.8)) * art.k))
        }
    }

    Repeater {
        model: art.geo.buttons

        PadButton {
            readonly property string slot: modelData.slot
            readonly property var stick: modelData.axes || null

            spec: modelData
            lit: art.focusedSlot === slot
            down: art.pressed[slot] === true
            learning: art.learningSlot === slot
            missing: art.unbound.indexOf(slot) !== -1
            pull: slot === "lt" || slot === "rt" ? (art.axes[slot] || 0) : 0
            leanX: stick ? (art.axes[stick[0]] || 0) : 0
            leanY: stick ? (art.axes[stick[1]] || 0) : 0
        }
    }
}
