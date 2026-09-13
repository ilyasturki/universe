import QtQuick
import "../core"

// A Canvas, not a shader: it must draw the same under GraphicsInfo.Software.
Item {
    id: ring

    property Item target: parent
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property real gap: Theme.dp(2)
    property real lineWidth: Theme.dp(Theme.outlineWidth)
    property bool shown: true
    property real phase: 0

    readonly property real pad: gap + lineWidth + Theme.dp(6)

    anchors.fill: target
    anchors.margins: -pad
    visible: shown
    z: 5

    NumberAnimation on phase {
        running: ring.visible
        loops: Animation.Infinite
        from: 0
        to: 1
        duration: 2600
    }

    Loader {
        anchors.fill: parent
        active: ring.shown
        sourceComponent: canvasComponent
    }

    readonly property Component canvasComponent: Canvas {
        id: canvas

        renderStrategy: Canvas.Cooperative

        Connections {
            target: ring
            function onPhaseChanged() { canvas.requestPaint(); }
            function onWidthChanged() { canvas.requestPaint(); }
            function onHeightChanged() { canvas.requestPaint(); }
        }

        function stop(g, at, color) {
            g.addColorStop(Math.max(0, Math.min(1, at)), color);
        }

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var w = width, h = height, lw = ring.lineWidth;
            var inset = Theme.dp(6) + lw / 2;
            var r = ring.cornerRadius + ring.gap + lw / 2;
            var p = ring.phase;
            var g = ctx.createLinearGradient(0, 0, w, h);
            var c = [Theme.outlineCyan, Theme.outlineBlue, Theme.outlineViolet, Theme.outlinePink, Theme.outlineCyan];
            for (var k = -1; k <= 1; k++) {
                for (var i = 0; i < c.length; i++) {
                    var at = (i / (c.length - 1)) + k - p;
                    if (at >= -0.01 && at <= 1.01)
                        stop(g, at, c[i]);
                }
            }
            stop(g, 0, c[Math.floor(((1 - p) % 1) * (c.length - 1))]);
            ctx.lineJoin = "round";
            ctx.beginPath();
            ctx.roundedRect(inset, inset, w - inset * 2, h - inset * 2, r, r);
            ctx.lineWidth = lw * 2.6;
            ctx.strokeStyle = Theme.dark ? Qt.rgba(0.35, 0.75, 1, 0.2) : Qt.rgba(0.3, 0.6, 1, 0.2);
            ctx.stroke();
            ctx.beginPath();
            ctx.roundedRect(inset, inset, w - inset * 2, h - inset * 2, r, r);
            ctx.lineWidth = lw;
            ctx.strokeStyle = g;
            ctx.stroke();
        }
    }
}
