import QtQuick
import "../core"

Item {
    id: root

    // "A" | "B" | "X" | "Y" | "RS" | "LB RB" | "LT RT" | "dpad"
    property string glyph: "A"
    readonly property bool isPair: glyph.indexOf(" ") !== -1
    readonly property real unit: Theme.dp(30)

    implicitHeight: unit
    implicitWidth: isPair ? unit * 2.2 : unit

    Component {
        id: faceButton
        Rectangle {
            width: root.unit
            height: root.unit
            radius: width / 2
            color: "transparent"
            border.width: Math.max(1, Theme.dp(2))
            border.color: Qt.rgba(0.949, 0.953, 0.961, 0.55)

            Text {
                anchors.centerIn: parent
                text: root.glyph
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(14)
            }
        }
    }

    Component {
        id: shoulderPair
        Row {
            spacing: root.unit * 0.27

            Repeater {
                model: root.glyph.split(" ")

                Rectangle {
                    width: root.unit * 0.95
                    height: root.unit * 0.8
                    radius: Theme.dp(8)
                    color: "transparent"
                    border.width: Math.max(1, Theme.dp(2))
                    border.color: Qt.rgba(0.949, 0.953, 0.961, 0.55)

                    Text {
                        anchors.centerIn: parent
                        text: modelData
                        color: Theme.text
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(12)
                    }
                }
            }
        }
    }

    Component {
        id: dpadCross
        Canvas {
            width: root.unit
            height: root.unit

            onPaint: {
                var ctx = getContext("2d");
                ctx.reset();
                var s = width / 30;
                ctx.strokeStyle = Qt.rgba(0.949, 0.953, 0.961, 0.55);
                ctx.lineWidth = Math.max(1, 2 * s);
                ctx.lineJoin = "round";
                ctx.beginPath();
                ctx.moveTo(11 * s, 4 * s);
                ctx.lineTo(19 * s, 4 * s);
                ctx.lineTo(19 * s, 11 * s);
                ctx.lineTo(26 * s, 11 * s);
                ctx.lineTo(26 * s, 19 * s);
                ctx.lineTo(19 * s, 19 * s);
                ctx.lineTo(19 * s, 26 * s);
                ctx.lineTo(11 * s, 26 * s);
                ctx.lineTo(11 * s, 19 * s);
                ctx.lineTo(4 * s, 19 * s);
                ctx.lineTo(4 * s, 11 * s);
                ctx.lineTo(11 * s, 11 * s);
                ctx.closePath();
                ctx.stroke();
            }
        }
    }

    Loader {
        anchors.centerIn: parent
        sourceComponent: root.glyph === "dpad" ? dpadCross
                       : root.isPair ? shoulderPair
                       : faceButton
    }
}
