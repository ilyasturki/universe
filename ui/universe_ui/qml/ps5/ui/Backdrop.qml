import QtQuick
import "../core"

// The system's own background (Settings, the Library, the Media Gallery): slate blue, lit from the top left.
Item {
    id: root

    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop {
                position: 0.0
                color: "#23272f"
            }
            GradientStop {
                position: 0.55
                color: "#181b22"
            }
            GradientStop {
                position: 1.0
                color: "#101217"
            }
        }
    }

    Canvas {
        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()

        function bloom(ctx, x, y, r, a) {
            var g = ctx.createRadialGradient(x, y, 0, x, y, r);
            g.addColorStop(0.0, Qt.rgba(0.62, 0.66, 0.74, a));
            g.addColorStop(0.45, Qt.rgba(0.45, 0.49, 0.58, a * 0.45));
            g.addColorStop(1.0, Qt.rgba(0.3, 0.33, 0.4, 0));
            ctx.fillStyle = g;
            ctx.fillRect(0, 0, width, height);
        }

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            if (width <= 0 || height <= 0)
                return;
            bloom(ctx, width * 0.17, -height * 0.08, height * 0.62, 0.34);
            bloom(ctx, width * 0.05, height * 1.02, height * 0.5, 0.14);
            bloom(ctx, width * 0.9, height * 0.2, height * 0.55, 0.06);
        }
    }
}
