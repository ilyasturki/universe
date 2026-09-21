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
    // The stick and trigger numbers, shown beside them (at rest too, so nothing jumps).
    property bool readouts: false

    readonly property var geo: Geometry.of(family)
    readonly property var bodyBox: Geometry.bounds(geo.body)
    readonly property real k: Math.max(0.01, Math.min(width / geo.view[0], height / geo.view[1]))
    readonly property real ox: (width - geo.view[0] * k) / 2
    readonly property real oy: (height - geo.view[1] * k) / 2

    function outline(ctx, k) {
        ctx.path = art.geo.body;
        ctx.lineWidth = 2 / k;
        ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.30);
        ctx.stroke();
    }

    Canvas {
        id: body

        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        Connections {
            target: art
            function onGeoChanged() {
                body.requestPaint();
            }
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
            var box = art.bodyBox, top = box[1], bottom = box[3];
            var shade = ctx.createLinearGradient(0, top, 0, bottom);
            shade.addColorStop(0, Qt.rgba(1, 1, 1, 0.20));
            shade.addColorStop(0.5, Qt.rgba(1, 1, 1, 0.13));
            shade.addColorStop(1, Qt.rgba(1, 1, 1, 0.08));
            ctx.path = art.geo.body;
            ctx.fillStyle = shade;
            ctx.fill();
            ctx.save();
            ctx.path = art.geo.body;
            ctx.clip();
            var glow = ctx.createRadialGradient(500, top + (bottom - top) * 0.12, 0, 500, top + (bottom - top) * 0.12, 1000 * 0.55);
            glow.addColorStop(0, Qt.rgba(1, 1, 1, 0.10));
            glow.addColorStop(1, Qt.rgba(1, 1, 1, 0));
            ctx.fillStyle = glow;
            ctx.fillRect(0, top, 1000, bottom - top);
            var grip = ctx.createLinearGradient(0, top, 0, bottom);
            grip.addColorStop(0.5, Qt.rgba(0, 0, 0, 0));
            grip.addColorStop(1, Qt.rgba(0, 0, 0, 0.34));
            ctx.fillStyle = grip;
            ctx.fillRect(0, top, 1000, bottom - top);
            ctx.restore();
            art.outline(ctx, k);
            var details = art.geo.details;
            for (var i = 0; i < details.length; i++) {
                var d = details[i];
                ctx.beginPath();
                if (d.kind === "shape") {
                    ctx.path = d.path;
                    ctx.fillStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.05);
                    ctx.fill();
                    ctx.lineWidth = 1.6 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.2);
                    ctx.stroke();
                } else if (d.kind === "dish") {
                    ctx.arc(d.x, d.y, d.r, 0, Math.PI * 2);
                    ctx.fillStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.035);
                    ctx.fill();
                    ctx.lineWidth = (d.ring ? 1.6 : 1.4) / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, d.ring ? 0.3 : 0.14);
                    ctx.stroke();
                } else if (d.kind === "cross") {
                    Draw.cross(ctx, d.x, d.y, d.l, d.a);
                    ctx.fillStyle = Qt.rgba(1, 1, 1, 0.14);
                    ctx.fill();
                    ctx.lineWidth = 1.6 / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.36);
                    ctx.stroke();
                } else if (d.kind === "dots") {
                    ctx.fillStyle = Qt.rgba(1, 1, 1, 0.3);
                    for (var r = 0; r < d.rows; r++)
                        for (var c = 0; c < d.cols; c++) {
                            ctx.beginPath();
                            ctx.arc(d.x + (c - (d.cols - 1) / 2) * d.gap, d.y + (r - (d.rows - 1) / 2) * d.gap, d.r, 0, Math.PI * 2);
                            ctx.fill();
                        }
                } else if (d.kind === "line") {
                    if (d.glow) {
                        ctx.save();
                        ctx.shadowColor = Qt.rgba(1, 1, 1, 0.5);
                        ctx.shadowBlur = 8 * k;
                        ctx.path = d.path;
                        ctx.lineWidth = 6 / k;
                        ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                        ctx.stroke();
                        ctx.restore();
                    }
                    ctx.path = d.path;
                    ctx.lineWidth = (d.width || 2) / k;
                    ctx.strokeStyle = Qt.rgba(1, 1, 1, d.alpha !== undefined ? d.alpha : 0.2);
                    ctx.stroke();
                }
            }
            var buttons = art.geo.buttons;
            for (var j = 0; j < buttons.length; j++) {
                var s = buttons[j];
                if (s.kind !== "stick")
                    continue;
                ctx.beginPath();
                ctx.arc(s.x, s.y, s.r + 16, 0, Math.PI * 2);
                ctx.fillStyle = Qt.rgba(0, 0, 0, 0.22);
                ctx.fill();
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
        readonly property bool leftSide: spec.side === "l" || spec.x < 500

        property real glow: down ? 1.0 : 0.0
        Behavior on glow {
            Ease {
                duration: button.down ? 30 : Theme.durQuick
            }
        }
        Behavior on leanX {
            NumberAnimation {
                duration: 60
            }
        }
        Behavior on leanY {
            NumberAnimation {
                duration: 60
            }
        }
        Behavior on pull {
            NumberAnimation {
                duration: 60
            }
        }

        x: art.ox + box.x * art.k - margin
        y: art.oy + box.y * art.k - margin
        width: box.w * art.k + margin * 2
        height: box.h * art.k + margin * 2

        function mix(a, b, t) {
            return Qt.rgba(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t);
        }

        // A trigger's mark goes dark once its pull has filled past the middle, where the mark sits.
        readonly property color markColor: mix(Qt.rgba(Theme.text.r, Theme.text.g, Theme.text.b, ink), onDown, spec.kind === "trigger" ? Math.max(glow, pull >= 0.5 ? 1 : 0) : glow)
        readonly property real lean: 0.45

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
                var strokeA = button.lit || button.learning ? 0.95 : s.ghost ? 0.5 : 0.40;
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

                // Into sheet coordinates, for the shapes given as paths on the sheet.
                function sheet() {
                    ctx.translate(m - button.box.x * k, m - button.box.y * k);
                    ctx.scale(k, k);
                }

                var w = width, h = height;
                var cx = w / 2, cy = h / 2;
                if (s.kind === "face") {
                    var r = s.r * k;
                    paintPath(function () {
                        Draw.circle(ctx, cx, cy, r);
                    });
                } else if (s.kind === "stick") {
                    var R = s.r * k;
                    var px = cx + button.leanX * R * button.lean, py = cy + button.leanY * R * button.lean;
                    paintPath(function () {
                        Draw.circle(ctx, px, py, R);
                    });
                } else if (s.kind === "arm") {
                    var l = s.l * k, a = s.a * k, gap = s.split ? a * 0.28 : 0;
                    var dx = s.dir === "left" ? -1 : s.dir === "right" ? 1 : 0;
                    var dy = s.dir === "up" ? -1 : s.dir === "down" ? 1 : 0;
                    if (s.split) {
                        paintPath(function () {
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
                    ctx.save();
                    sheet();
                    var shape = function () {
                        ctx.path = s.path;
                    };
                    if (button.lit) {
                        ctx.save();
                        ctx.lineWidth = Theme.dp(8) / k;
                        ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                        shape();
                        ctx.stroke();
                        ctx.restore();
                    }
                    shape();
                    ctx.fillStyle = Theme.ground;
                    ctx.fill();
                    ctx.fillStyle = Qt.rgba(white.r, white.g, white.b, fillA);
                    ctx.fill();
                    if (button.pull > 0.005) {
                        ctx.save();
                        shape();
                        ctx.clip();
                        var th = s.b[3] - s.b[1];
                        ctx.fillStyle = Qt.rgba(white.r, white.g, white.b, 0.92);
                        ctx.fillRect(s.b[0], s.b[3] - th * button.pull, s.b[2] - s.b[0], th * button.pull + 2);
                        ctx.restore();
                    }
                    shape();
                    ctx.lineWidth = (button.lit || g > 0.5 ? Theme.dp(2.5) : line) / k;
                    ctx.strokeStyle = stroke;
                    ctx.stroke();
                    ctx.restore();
                } else if (s.kind === "bumper") {
                    ctx.save();
                    sheet();
                    // A raised band along the shoulder, cut by the silhouette, the body's own outline over it.
                    if (button.lit) {
                        ctx.save();
                        ctx.lineWidth = Theme.dp(8) / k;
                        ctx.strokeStyle = Qt.rgba(1, 1, 1, 0.12);
                        ctx.path = s.path;
                        ctx.stroke();
                        ctx.restore();
                    }
                    ctx.save();
                    ctx.path = art.geo.body;
                    ctx.clip();
                    ctx.path = s.path;
                    ctx.fillStyle = button.mix(Qt.rgba(white.r, white.g, white.b, button.missing ? 0.05 : button.learning ? art.pulse : button.lit ? 0.26 : 0.12), white, g);
                    ctx.fill();
                    ctx.lineWidth = (button.lit || g > 0.5 ? Theme.dp(2.5) : line) / k;
                    ctx.strokeStyle = stroke;
                    ctx.stroke();
                    ctx.restore();
                    ctx.setLineDash([]);
                    art.outline(ctx, k);
                    ctx.restore();
                } else if (s.kind === "paddle") {
                    var pw = s.w * k, ph = s.h * k;
                    paintPath(function () {
                        Draw.paddle(ctx, cx, m, pw, ph);
                    });
                } else {
                    var bw2 = s.w * k, bh2 = s.h * k;
                    var rr = s.round ? Math.min(bw2, bh2) / 2 : Math.min(bw2, bh2) * 0.3;
                    ctx.save();
                    if (s.angle) {
                        ctx.translate(cx, cy);
                        ctx.rotate(s.angle * Math.PI / 180);
                        ctx.translate(-cx, -cy);
                    }
                    paintPath(function () {
                        Draw.roundRect(ctx, cx - bw2 / 2, cy - bh2 / 2, bw2, bh2, rr);
                    });
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
            anchors.verticalCenterOffset: button.spec.kind === "stick" ? button.leanY * button.spec.r * art.k * button.lean : button.spec.kind === "trigger" ? (15 - button.box.h / 2) * art.k : 0
            visible: button.glyph.symbol === "" && button.glyph.text !== "" && button.spec.kind !== "arm"
            text: button.glyph.text
            color: button.markColor
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Math.max(8, Math.round((button.spec.kind === "face" ? button.spec.r * 1.05 : button.spec.kind === "stick" ? button.spec.r * 0.42 : button.spec.kind === "paddle" ? button.spec.w * 0.42 : button.spec.kind === "trigger" ? 18 : button.spec.kind === "bumper" ? 16 : button.spec.kind === "arm" ? 10 : Math.min(button.spec.w, button.spec.h) * (button.glyph.text.length > 1 ? 0.62 : 0.8)) * art.k))
        }

        // The numbers beside a stick or a trigger, outside its box on the side away from the pad's middle.
        Column {
            readonly property real gap: 24 * art.k - button.margin

            visible: art.readouts && (button.spec.kind === "stick" || button.spec.kind === "trigger")
            anchors.left: button.leftSide ? undefined : parent.right
            anchors.right: button.leftSide ? parent.left : undefined
            anchors.leftMargin: gap
            anchors.rightMargin: gap
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: button.spec.kind === "trigger" ? (15 - button.box.h / 2) * art.k : 0
            spacing: 0

            Repeater {
                model: button.spec.kind === "stick" ? ["x " + (button.leanX >= 0 ? "+" : "−") + Math.round(Math.abs(button.leanX) * 100) + " %", "y " + (button.leanY >= 0 ? "+" : "−") + Math.round(Math.abs(button.leanY) * 100) + " %"] : [Math.round(button.pull * 100) + " %"]

                Text {
                    anchors.left: button.leftSide ? undefined : parent.left
                    anchors.right: button.leftSide ? parent.right : undefined
                    text: modelData
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.DemiBold
                    font.pixelSize: Math.max(8, Math.round(22 * art.k))
                    font.features: {
                        "tnum": 1
                    }
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
