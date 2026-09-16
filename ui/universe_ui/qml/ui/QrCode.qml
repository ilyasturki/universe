import QtQuick
import "../core"

// matrix: rows of booleans.
Rectangle {
    id: root

    property var matrix: []
    readonly property int modules: matrix ? matrix.length : 0
    readonly property real quiet: Theme.dp(16)

    color: "white"
    radius: Theme.dp(12)

    onMatrixChanged: canvas.requestPaint()
    onWidthChanged: canvas.requestPaint()

    Canvas {
        id: canvas
        anchors.fill: parent
        anchors.margins: root.quiet

        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            if (root.modules === 0)
                return;
            var cell = width / root.modules;
            ctx.fillStyle = "#000000";
            for (var y = 0; y < root.modules; y++) {
                var row = root.matrix[y];
                for (var x = 0; x < row.length; x++) {
                    if (row[x])
                        ctx.fillRect(Math.floor(x * cell), Math.floor(y * cell), Math.ceil(cell), Math.ceil(cell));
                }
            }
        }
    }

    Text {
        anchors.centerIn: parent
        visible: root.modules === 0
        width: parent.width - root.quiet * 2
        text: "No QR encoder (python qrcode missing)"
        color: "#101116"
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        font.family: Theme.sans
        font.pixelSize: Theme.dp(18)
    }
}
