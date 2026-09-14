import QtQuick
import "../core"

Rectangle {
    id: root

    property string label: "Play"
    // "play" | "info" | "library" | "stop" | a MenuGlyph kind | ""
    property string icon: "play"
    property bool ghost: false
    property bool focused: false
    property bool dimmed: false
    // 0..1 of the width shaded, left to right: the progress of a hold.
    property real fill: 0.0

    readonly property color ink: root.ghost ? Theme.text : Theme.onLight

    height: Theme.dp(78)
    width: content.width + Theme.dp(root.ghost ? 76 : 92)
    radius: height / 2
    color: root.ghost ? Theme.surface : Theme.text
    border.width: root.ghost ? Math.max(1, Theme.dp(2)) : 0
    border.color: Theme.surfaceBorder

    opacity: root.dimmed ? 0.5 : 1.0
    scale: root.focused ? 1.04 : 1.0

    Behavior on opacity {
        NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
    }
    Behavior on scale {
        NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutQuint }
    }

    Loader {
        anchors.fill: parent
        active: root.focused
        sourceComponent: FocusRing {
            cornerRadius: root.radius
            gapWidth: Theme.dp(6)
        }
    }

    Item {
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: parent.width * root.fill
        clip: true
        visible: root.fill > 0

        Rectangle {
            width: root.width
            height: root.height
            radius: root.radius
            color: root.ink
            opacity: 0.18
        }
    }

    Row {
        id: content
        anchors.centerIn: parent
        spacing: Theme.dp(16)

        Canvas {
            visible: root.icon === "stop"
            width: Theme.dp(22); height: Theme.dp(22)
            anchors.verticalCenter: parent.verticalCenter
            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                ctx.fillStyle = root.ink;
                ctx.beginPath();
                ctx.roundedRect(0, 0, width, height, width * 0.18, height * 0.18);
                ctx.fill();
            }
        }

        Canvas {
            visible: root.icon === "play"
            width: Theme.dp(22); height: Theme.dp(24)
            anchors.verticalCenter: parent.verticalCenter
            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                ctx.fillStyle = root.ink;
                ctx.beginPath();
                ctx.moveTo(width * 0.09, 0);
                ctx.lineTo(width, height / 2);
                ctx.lineTo(width * 0.09, height);
                ctx.closePath();
                ctx.fill();
            }
        }

        Canvas {
            visible: root.icon === "info"
            width: Theme.dp(26); height: Theme.dp(26)
            anchors.verticalCenter: parent.verticalCenter
            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = width / 24;
                ctx.strokeStyle = root.ink;
                ctx.fillStyle = root.ink;
                ctx.lineWidth = 2 * s;
                ctx.lineCap = "round";
                ctx.beginPath();
                ctx.arc(12 * s, 12 * s, 10 * s, 0, Math.PI * 2);
                ctx.stroke();
                ctx.beginPath();
                ctx.moveTo(12 * s, 11 * s);
                ctx.lineTo(12 * s, 16.5 * s);
                ctx.stroke();
                ctx.beginPath();
                ctx.arc(12 * s, 7.6 * s, 1.3 * s, 0, Math.PI * 2);
                ctx.fill();
            }
        }

        Canvas {
            visible: root.icon === "library"
            width: Theme.dp(24); height: Theme.dp(24)
            anchors.verticalCenter: parent.verticalCenter
            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = width / 24;
                ctx.fillStyle = root.ink;
                var cells = [[2, 2], [13, 2], [2, 13], [13, 13]];
                for (var i = 0; i < cells.length; i++) {
                    ctx.beginPath();
                    ctx.roundedRect(cells[i][0] * s, cells[i][1] * s, 9 * s, 9 * s, 2 * s, 2 * s);
                    ctx.fill();
                }
            }
        }

        MenuGlyph {
            visible: root.icon !== "" && ["play", "info", "library", "stop"].indexOf(root.icon) < 0
            width: Theme.dp(26); height: Theme.dp(26)
            anchors.verticalCenter: parent.verticalCenter
            kind: visible ? root.icon : ""
            tint: root.ink
        }

        Text {
            text: root.label
            color: root.ink
            font.family: Theme.sans
            font.weight: root.ghost ? Font.Medium : Font.DemiBold
            font.pixelSize: Theme.dp(root.ghost ? 25 : 27)
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
