import QtQuick
import "../core"

// A Canvas, not a shader: it must draw the same under GraphicsInfo.Software.
Item {
    id: ring

    property Item target: parent
    property real cornerRadius: Theme.dp(Theme.radiusTile)
    property real gap: Theme.dp(Theme.ringGap)
    property real lineWidth: Theme.dp(Theme.ringLine)
    property bool shown: true

    readonly property real edge: Theme.dp(Theme.ringEdge)
    readonly property real glow: Theme.dp(Theme.ringGlow)
    readonly property real pad: gap + lineWidth + edge + glow

    anchors.fill: target
    anchors.margins: -pad
    visible: shown
    z: 5

    // Sticky: a grid allocates canvases as focus reaches its tiles, and a focus move never
    // rebuilds one (a fresh canvas shows a blank frame before its first paint).
    onShownChanged: if (shown) loader.active = true
    Component.onCompleted: if (shown) loader.active = true

    Loader {
        id: loader
        anchors.fill: parent
        active: false
        sourceComponent: canvasComponent
    }

    readonly property Component canvasComponent: Canvas {
        id: canvas

        renderStrategy: Canvas.Cooperative

        onVisibleChanged: if (visible) requestPaint()

        Connections {
            target: Theme
            enabled: canvas.visible
            function onRingPhaseChanged() { canvas.requestPaint(); }
        }
        Connections {
            target: ring
            function onWidthChanged() { canvas.requestPaint(); }
            function onHeightChanged() { canvas.requestPaint(); }
        }

        function mix(a, b, k) {
            return Qt.rgba(a.r + (b.r - a.r) * k, a.g + (b.g - a.g) * k, a.b + (b.b - a.b) * k, 1);
        }

        // A rounded frame `inset` in from the canvas edge, its corners concentric with the target's
        function frame(ctx, inset, lw, style) {
            ctx.beginPath();
            var r = ring.cornerRadius + (ring.pad - inset);
            ctx.roundedRect(inset, inset, width - inset * 2, height - inset * 2, r, r);
            ctx.lineWidth = lw;
            ctx.strokeStyle = style;
            ctx.stroke();
        }

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            ctx.clearRect(0, 0, width, height);
            ctx.lineJoin = "round";
            var gap = ring.gap, lw = ring.lineWidth, edge = ring.edge, glow = ring.glow;
            // The band brightens and dims once per clock turn: the Switch's breathing ring.
            var k = 0.5 - 0.5 * Math.cos(Theme.ringPhase * Math.PI * 2);
            var lift = 0.45 * k;
            var g = ctx.createLinearGradient(0, 0, width, height);
            g.addColorStop(0, mix(Theme.ringCyan, Theme.ringBright, lift));
            g.addColorStop(0.55, mix(Theme.ringBlue, Theme.ringBright, lift * 0.7));
            g.addColorStop(1, mix(Theme.ringCyan, Theme.ringBright, lift));
            var steps = 3, w = glow / steps, c = Theme.ringGlowColor;
            for (var i = 0; i < steps; i++)
                frame(ctx, w * i + w / 2, w + 0.5, Qt.rgba(c.r, c.g, c.b, c.a * (i + 1) / steps));
            frame(ctx, glow + edge / 2, edge, Theme.ringEdgeColor);
            frame(ctx, glow + edge + lw / 2, lw, g);
            if (gap > 0)
                frame(ctx, glow + edge + lw + gap / 2, gap + 0.5, Theme.ringInner);
        }
    }
}
