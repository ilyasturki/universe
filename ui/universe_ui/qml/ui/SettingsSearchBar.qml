import QtQuick
import "../core"

// The search field across the top of a source's library: the query it holds, or a prompt.
Rectangle {
    id: bar

    property var entry: ({})
    property bool focused: false

    readonly property bool empty: !entry.display

    radius: Theme.dp(16)
    color: focused ? Theme.text : Theme.surface

    Behavior on color {
        ColorAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }

    Row {
        anchors.left: parent.left
        anchors.leftMargin: Theme.dp(22)
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(22)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Theme.dp(16)

        Canvas {
            readonly property color stroke: bar.focused ? Qt.rgba(0.063, 0.067, 0.086, 0.55) : Theme.textMuted
            onStrokeChanged: requestPaint()

            anchors.verticalCenter: parent.verticalCenter
            width: Theme.dp(26)
            height: Theme.dp(26)

            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = width / 24;
                ctx.strokeStyle = stroke;
                ctx.lineWidth = 2 * s;
                ctx.lineCap = "round";
                ctx.beginPath();
                ctx.arc(11 * s, 11 * s, 7 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(16.5 * s, 16.5 * s);
                ctx.lineTo(21 * s, 21 * s);
                ctx.stroke();
            }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - Theme.dp(42)
            text: bar.empty ? (bar.entry.label || "") : bar.entry.display
            color: bar.focused ? Qt.rgba(0.063, 0.067, 0.086, bar.empty ? 0.55 : 1.0)
                               : (bar.empty ? Theme.textMuted : Theme.text)
            font.family: Theme.sans
            font.pixelSize: Theme.dp(22)
            elide: Text.ElideRight
        }
    }
}
