import QtQuick
import "../core"
import "ControllerHotspots.js" as Hotspots

// The current pad drawn flat, a hotspot over every button: lit for the focused row, a flash
// on a live press, a pulse while a button is being learned, dashed when the slot has no
// button on this connection. Sits in the right column of the Controller section.
Item {
    id: art

    property string family: "dualsense"
    property bool connected: false
    property string focusedSlot: ""
    property string learningSlot: ""
    property var unbound: []
    property var pressed: ({})

    readonly property string drawn: Hotspots.has(family) ? family : "generic"
    readonly property var spots: Hotspots.slots(drawn)
    readonly property real pad: Theme.dp(8)
    readonly property real inset: Theme.dp(20)

    implicitHeight: pad * 2 + 2 + frame.height + caption.height + Theme.dp(20)

    function press(slot, down) {
        var next = {};
        for (var k in pressed)
            if (k !== slot)
                next[k] = true;
        if (down)
            next[slot] = true;
        pressed = next;
    }

    onFamilyChanged: pressed = ({})

    // The learn pulse, shared by the one hotspot that shows it.
    property real pulse: 0.15
    SequentialAnimation on pulse {
        running: art.learningSlot !== ""
        loops: Animation.Infinite
        NumberAnimation { to: 0.55; duration: 500; easing.type: Easing.InOutSine }
        NumberAnimation { to: 0.15; duration: 500; easing.type: Easing.InOutSine }
    }

    Rectangle {
        anchors.fill: parent
        radius: Theme.dp(24)
        color: Qt.rgba(1, 1, 1, 0.04)
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.10)
    }

    Item {
        id: frame

        x: art.inset
        y: art.pad + 1 + Theme.dp(6)
        width: parent.width - art.inset * 2
        height: Math.round(width * 0.62)

        Image {
            id: img

            anchors.fill: parent
            source: Qt.resolvedUrl("../assets/controllers/" + art.drawn + ".svg")
            fillMode: Image.PreserveAspectFit
            sourceSize.width: width
            sourceSize.height: height
            smooth: true
            opacity: art.connected ? 1.0 : 0.32

            Behavior on opacity {
                NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
            }
        }

        Item {
            id: overlay

            x: (img.width - img.paintedWidth) / 2
            y: (img.height - img.paintedHeight) / 2
            width: img.paintedWidth
            height: img.paintedHeight
            visible: art.connected

            Repeater {
                model: art.spots

                Item {
                    id: spot

                    readonly property string slot: modelData.slot
                    readonly property bool lit: art.focusedSlot === slot
                    readonly property bool down: art.pressed[slot] === true
                    readonly property bool learning: art.learningSlot === slot
                    readonly property bool missing: art.unbound.indexOf(slot) !== -1
                    readonly property real radius: modelData.r * overlay.width

                    x: modelData.x * overlay.width
                    y: modelData.y * overlay.height
                    width: modelData.w * overlay.width
                    height: modelData.h * overlay.height

                    Rectangle {
                        anchors.fill: parent
                        anchors.margins: -Theme.dp(3)
                        radius: Math.min(spot.radius + Theme.dp(3), Math.min(width, height) / 2)
                        color: Theme.text
                        opacity: spot.down ? 0.8 : spot.lit ? 0.3 : spot.learning ? art.pulse : 0.0
                        border.width: spot.lit || spot.down || spot.learning ? Theme.dp(2) : 0
                        border.color: Theme.text

                        Behavior on opacity {
                            NumberAnimation { duration: spot.down ? 30 : Theme.durQuick; easing.type: Easing.OutCubic }
                        }
                    }

                    Canvas {
                        anchors.fill: parent
                        anchors.margins: -Theme.dp(6)
                        visible: spot.missing && !spot.lit && !spot.down && !spot.learning
                        onVisibleChanged: requestPaint()
                        onWidthChanged: requestPaint()
                        onPaint: {
                            var ctx = getContext("2d");
                            ctx.reset();
                            ctx.strokeStyle = Qt.rgba(0.949, 0.953, 0.961, 0.45);
                            ctx.lineWidth = Theme.dp(1.5);
                            ctx.setLineDash([Theme.dp(4), Theme.dp(4)]);
                            var r = Math.min(spot.radius + Theme.dp(6), Math.min(width, height) / 2);
                            ctx.beginPath();
                            ctx.moveTo(r, 0);
                            ctx.lineTo(width - r, 0);
                            ctx.arcTo(width, 0, width, r, r);
                            ctx.lineTo(width, height - r);
                            ctx.arcTo(width, height, width - r, height, r);
                            ctx.lineTo(r, height);
                            ctx.arcTo(0, height, 0, height - r, r);
                            ctx.lineTo(0, r);
                            ctx.arcTo(0, 0, r, 0, r);
                            ctx.stroke();
                        }
                    }
                }
            }
        }
    }

    Text {
        id: caption

        anchors.top: frame.bottom
        anchors.topMargin: Theme.dp(4)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: art.inset
        anchors.rightMargin: art.inset
        horizontalAlignment: Text.AlignHCenter
        text: !art.connected ? "Connect a controller"
            : art.learningSlot !== "" ? "Press the button on the controller"
            : art.unbound.length > 0 ? "Dashed buttons have no code on this connection: learn them"
            : "Press a button on the pad to jump to it"
        color: art.connected && art.learningSlot === "" ? Theme.textMuted : Theme.textSecondary
        font.family: Theme.sans
        font.weight: art.connected ? Font.Normal : Font.Medium
        font.pixelSize: Theme.dp(art.connected ? 19 : 24)
        wrapMode: Text.WordWrap
    }
}
