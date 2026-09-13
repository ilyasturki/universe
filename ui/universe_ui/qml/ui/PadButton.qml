import QtQuick
import "../core"
import "PadNames.js" as Names
import "PadDraw.js" as Draw
import "PadGeometry.js" as Geometry

// One button on the pad art: its resting shape, the mark the pad prints on it, and its states —
// lit for the focused row, white on a press, pulsing while learned, dashed with no code. A stick
// leans with its axes, a trigger fills with its pull.
Item {
    id: button

    property var spec: ({})
    property string family: "xbox"
    property real k: 1
    property real ox: 0
    property real oy: 0
    property bool lit: false
    property bool down: false
    property bool learning: false
    property bool missing: false
    property real pulse: 0.15
    property real pull: 0
    property real leanX: 0
    property real leanY: 0

    readonly property var glyph: Names.glyph(family, spec.slot || "")
    readonly property var box: Geometry.box(spec)
    // Room around the shape for the focus halo and a leaning stick cap.
    readonly property real margin: Theme.dp(10)
    readonly property real ink: 0.9
    readonly property color onDown: Theme.onLight

    property real glow: down ? 1.0 : 0.0
    Behavior on glow {
        NumberAnimation { duration: button.down ? 30 : Theme.durQuick; easing.type: Easing.OutCubic }
    }
    Behavior on leanX { NumberAnimation { duration: 60 } }
    Behavior on leanY { NumberAnimation { duration: 60 } }
    Behavior on pull { NumberAnimation { duration: 60 } }

    x: ox + box.x * k - margin
    y: oy + box.y * k - margin
    width: box.w * k + margin * 2
    height: box.h * k + margin * 2

    function mix(a, b, t) { return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t); }

    // A trigger's mark goes dark once its pull has filled past the middle, where the mark sits.
    readonly property color markColor: mix(Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, ink), onDown,
                                           spec.kind === "trigger" ? Math.max(glow, pull >= 0.5 ? 1 : 0) : glow)
    readonly property real lean: 0.5

    Canvas {
        id: canvas

        anchors.fill: parent

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var s = button.spec, k = button.k, m = button.margin, g = button.glow;
            var white = Theme.text;
            var t = s.kind === "paddle" && s.ghost ? 0.10 : 0.14;
            var fillA = button.missing ? 0.05 : button.learning ? button.pulse : button.lit ? 0.26 : t;
            var strokeA = button.lit || button.learning ? 0.95 : s.ghost ? 0.5 : 0.36;
            var fill = button.mix(Qt.rgba(white.r, white.g, white.b, fillA), white, g);
            var stroke = button.mix(Qt.rgba(white.r, white.g, white.b, strokeA), white, g);
            var line = Math.max(1, Theme.dp(1.6));
            ctx.lineJoin = "round";
            ctx.lineCap = "round";
            if (button.missing && g < 0.5)
                ctx.setLineDash([Theme.dp(4), Theme.dp(4)]);

            // Solid over the body: the ground first, then the button's own shade.
            function paintPath(shapeFn) {
                if (button.lit) {
                    ctx.save();
                    ctx.lineWidth = Theme.dp(8);
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                    shapeFn();
                    ctx.stroke();
                    ctx.restore();
                }
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
                // The well, then the cap where the stick leans.
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
                if (s.split) {
                    // One of Sony's four keys: a rounded block from the centre gap outward.
                    paintPath(function() {
                        var dx = s.dir === "left" ? -1 : s.dir === "right" ? 1 : 0;
                        var dy = s.dir === "up" ? -1 : s.dir === "down" ? 1 : 0;
                        var len = l - gap - a * 0.5;
                        var bx = cx + dx * (gap + a * 0.5 + len / 2), by = cy + dy * (gap + a * 0.5 + len / 2);
                        var bw = dx !== 0 ? len : a, bh = dy !== 0 ? len : a;
                        Draw.roundRect(ctx, bx - bw / 2, by - bh / 2, bw, bh, a * 0.22);
                    });
                } else if (g > 0.01 || button.lit || button.learning || button.missing) {
                    // One arm of the cross the body draws whole: only its states are painted here.
                    Draw.arm(ctx, cx, cy, l, a, s.dir);
                    ctx.fillStyle = button.lit && g < 0.01 ? Qt.rgba(white.r, white.g, white.b, 0.3) : button.learning && g < 0.01 ? Qt.rgba(white.r, white.g, white.b, button.pulse) : Qt.rgba(white.r, white.g, white.b, g);
                    ctx.fill();
                    if (button.lit || button.missing) {
                        ctx.lineWidth = button.lit ? Theme.dp(2.5) : line;
                        ctx.strokeStyle = stroke;
                        ctx.stroke();
                    }
                }
                // A chevron out along the arm.
                var ex = s.dir === "left" ? -1 : s.dir === "right" ? 1 : 0;
                var ey = s.dir === "up" ? -1 : s.dir === "down" ? 1 : 0;
                var tipX = cx + ex * (l * 0.72), tipY = cy + ey * (l * 0.72), v = a * 0.16;
                ctx.beginPath();
                ctx.moveTo(tipX - ex * v + ey * v * 1.4, tipY - ey * v + ex * v * 1.4);
                ctx.lineTo(tipX + ex * v * 0.6, tipY + ey * v * 0.6);
                ctx.lineTo(tipX - ex * v - ey * v * 1.4, tipY - ey * v - ex * v * 1.4);
                ctx.strokeStyle = button.markColor;
                ctx.lineWidth = Math.max(1, Theme.dp(1.8));
                ctx.stroke();
            } else if (s.kind === "trigger") {
                // The pull is the fill, rising from the bottom; a press past the half only
                // brightens the outline, so the gauge stays readable all the way down.
                var tw = s.w * k, th = s.h * k;
                var body = function() { Draw.roundRect(ctx, cx - tw / 2, cy - th / 2, tw, th, th * 0.32); };
                if (button.lit) {
                    ctx.save();
                    ctx.lineWidth = Theme.dp(8);
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                    body();
                    ctx.stroke();
                    ctx.restore();
                }
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
                // bumper, small, tab
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

        Connections {
            target: button
            function onGlowChanged() { canvas.requestPaint(); }
            function onLitChanged() { canvas.requestPaint(); }
            function onLearningChanged() { canvas.requestPaint(); }
            function onMissingChanged() { canvas.requestPaint(); }
            function onPulseChanged() { if (button.learning) canvas.requestPaint(); }
            function onPullChanged() { canvas.requestPaint(); }
            function onLeanXChanged() { canvas.requestPaint(); }
            function onLeanYChanged() { canvas.requestPaint(); }
            function onKChanged() { canvas.requestPaint(); }
            function onGlyphChanged() { canvas.requestPaint(); }
        }
    }

    Text {
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: button.spec.kind === "stick" ? button.leanX * button.spec.r * button.k * button.lean : 0
        anchors.verticalCenterOffset: button.spec.kind === "stick" ? button.leanY * button.spec.r * button.k * button.lean : 0
        visible: button.glyph.symbol === "" && button.glyph.text !== "" && button.spec.kind !== "arm"
        text: button.glyph.text
        color: button.markColor
        font.family: Theme.sans
        font.weight: Font.DemiBold
        font.pixelSize: Math.max(8, Math.round((button.spec.kind === "face" ? button.spec.r * 1.05
            : button.spec.kind === "stick" ? button.spec.r * 0.42
            : button.spec.kind === "paddle" ? button.spec.w * 0.42
            : button.spec.kind === "arm" ? 10
            : Math.min(button.spec.w, button.spec.h) * (button.glyph.text.length > 1 ? 0.62 : 0.8)) * button.k))
    }
}
