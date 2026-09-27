import QtQuick
import "../core"

// The Welcome hub's own background: deep blue, the four face-button shapes drifting slowly through it.
Item {
    id: scene

    property bool running: true

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: "#0b2466"
            }
            GradientStop {
                position: 0.55
                color: "#071641"
            }
            GradientStop {
                position: 1.0
                color: "#030a22"
            }
        }
    }

    Canvas {
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            if (width <= 0 || height <= 0)
                return;
            var g = ctx.createRadialGradient(width * 0.72, height * 0.3, 0, width * 0.72, height * 0.3, height * 0.8);
            g.addColorStop(0.0, Qt.rgba(0.25, 0.45, 1.0, 0.35));
            g.addColorStop(1.0, Qt.rgba(0.1, 0.2, 0.6, 0));
            ctx.fillStyle = g;
            ctx.fillRect(0, 0, width, height);
        }
    }

    component Shape: Canvas {
        id: shape

        property string kind: "circle"
        property real driftX: Theme.dp(60)
        property real driftY: Theme.dp(40)
        property int period: 24000
        property real baseX: 0
        property real baseY: 0

        width: Theme.dp(420)
        height: width
        x: baseX
        y: baseY
        opacity: 0.5
        onWidthChanged: requestPaint()

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            var s = width, w = s * 0.07;
            ctx.lineWidth = w;
            ctx.lineJoin = "round";
            ctx.lineCap = "round";
            var grad = ctx.createLinearGradient(0, 0, s, s);
            grad.addColorStop(0, Qt.rgba(0.62, 0.78, 1.0, 0.55));
            grad.addColorStop(1, Qt.rgba(0.3, 0.5, 1.0, 0.12));
            ctx.strokeStyle = grad;
            ctx.beginPath();
            if (kind === "circle")
                ctx.arc(s / 2, s / 2, s / 2 - w, 0, Math.PI * 2);
            else if (kind === "square")
                ctx.rect(w, w, s - 2 * w, s - 2 * w);
            else if (kind === "triangle") {
                ctx.moveTo(s / 2, w * 1.5);
                ctx.lineTo(s - w, s - w * 1.5);
                ctx.lineTo(w, s - w * 1.5);
                ctx.closePath();
            } else {
                ctx.moveTo(w * 1.5, w * 1.5);
                ctx.lineTo(s - w * 1.5, s - w * 1.5);
                ctx.moveTo(s - w * 1.5, w * 1.5);
                ctx.lineTo(w * 1.5, s - w * 1.5);
            }
            ctx.stroke();
        }

        transform: Translate {
            id: drift
        }

        SequentialAnimation {
            running: scene.running
            paused: running && (!scene.visible || Theme.covered)
            loops: Animation.Infinite

            ParallelAnimation {
                NumberAnimation {
                    target: drift
                    property: "x"
                    to: shape.driftX
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    target: drift
                    property: "y"
                    to: shape.driftY
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    target: shape
                    property: "rotation"
                    to: 18
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
            }
            ParallelAnimation {
                NumberAnimation {
                    target: drift
                    property: "x"
                    to: 0
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    target: drift
                    property: "y"
                    to: 0
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
                NumberAnimation {
                    target: shape
                    property: "rotation"
                    to: 0
                    duration: shape.period
                    easing.type: Easing.InOutQuad
                }
            }
        }
    }

    Shape {
        kind: "triangle"
        baseX: scene.width * 0.62
        baseY: -scene.height * 0.08
        width: scene.height * 0.5
        period: 26000
    }

    Shape {
        kind: "circle"
        baseX: scene.width * 0.8
        baseY: scene.height * 0.38
        width: scene.height * 0.42
        driftX: -Theme.dp(80)
        period: 31000
        opacity: 0.4
    }

    Shape {
        kind: "cross"
        baseX: scene.width * 0.46
        baseY: scene.height * 0.5
        width: scene.height * 0.34
        driftY: -Theme.dp(50)
        period: 23000
        opacity: 0.3
    }

    Shape {
        kind: "square"
        baseX: scene.width * 0.06
        baseY: scene.height * 0.58
        width: scene.height * 0.38
        driftX: Theme.dp(40)
        period: 29000
        opacity: 0.25
    }
}
