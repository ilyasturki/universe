import QtQuick
import "../core"

Item {
    id: bubble

    property alias text: name.text
    // The notch's centre, in the bubble's own x
    property real tipX: width / 2
    property real maxTextWidth: Theme.dp(760)
    readonly property color face: "#fbfbfb"
    readonly property real notch: Theme.dp(16)

    width: name.width + Theme.dp(84)
    height: Theme.dp(84)

    Rectangle {
        anchors.fill: parent
        anchors.margins: -Theme.dp(3)
        anchors.topMargin: -Theme.dp(1)
        radius: Theme.dp(19)
        color: Qt.rgba(0, 0, 0, 0.04)
    }

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: Theme.dp(2)
        anchors.bottomMargin: -Theme.dp(2)
        radius: Theme.dp(16)
        color: Qt.rgba(0, 0, 0, 0.08)
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(16)
        color: bubble.face
    }

    Canvas {
        x: Math.max(Theme.dp(16), Math.min(bubble.width - Theme.dp(16) - width, bubble.tipX - width / 2))
        y: -height + 1
        width: bubble.notch * 2
        height: bubble.notch
        onPaint: {
            var ctx = getContext("2d");
            ctx.reset();
            ctx.fillStyle = String(bubble.face);
            ctx.beginPath();
            ctx.moveTo(0, height);
            ctx.lineTo(width / 2, 0);
            ctx.lineTo(width, height);
            ctx.closePath();
            ctx.fill();
        }
    }

    Marquee {
        id: name

        anchors.centerIn: parent
        maxWidth: bubble.maxTextWidth
        fade: bubble.face
        color: Theme.accent
        font.pixelSize: Theme.dp(36)
    }
}
