import QtQuick
import "../core"

// The current pad in its card: lit for the focused row, a flash on a live press, a pulse while a
// button is being learned, dashed when the slot has no button on this connection. In the live
// view it fills the page, with the last press named under it.
Item {
    id: art

    property string family: "dualsense"
    property bool connected: false
    property bool testing: false
    property string focusedSlot: ""
    property string learningSlot: ""
    property var unbound: []
    property var rows: []
    property var pressed: ({})
    property var axes: ({})
    property string lastSlot: ""

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
        if (down)
            lastSlot = slot;
    }

    function axis(name, value) {
        var next = {};
        for (var k in axes)
            next[k] = axes[k];
        next[name] = value;
        axes = next;
    }

    function clear() {
        pressed = ({});
        axes = ({});
        lastSlot = "";
    }

    function labelOf(slot) {
        for (var i = 0; i < rows.length; i++)
            if (rows[i].slot === slot)
                return rows[i].label;
        return slot;
    }

    onFamilyChanged: clear()

    // The learn pulse, shared by the one button that shows it.
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
        height: art.testing ? Math.max(0, art.height - y - caption.height - Theme.dp(24)) : Math.round(width * 0.66)

        PadArt {
            anchors.fill: parent
            family: art.family
            focusedSlot: art.focusedSlot
            learningSlot: art.learningSlot
            unbound: art.connected ? art.unbound : []
            pressed: art.pressed
            axes: art.axes
            pulse: art.pulse
            opacity: art.connected ? 1.0 : 0.38

            Behavior on opacity {
                NumberAnimation { duration: Theme.durScene; easing.type: Easing.OutCubic }
            }
        }
    }

    Column {
        id: caption

        anchors.top: frame.bottom
        anchors.topMargin: Theme.dp(4)
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: art.inset
        anchors.rightMargin: art.inset
        spacing: Theme.dp(6)

        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: art.testing && art.lastSlot !== ""
            spacing: Theme.dp(14)

            PadGlyph {
                anchors.verticalCenter: parent.verticalCenter
                family: art.family
                slot: art.lastSlot
                unit: Theme.dp(34)
            }

            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: art.labelOf(art.lastSlot)
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(26)
            }
        }

        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: !art.connected ? "Connect a controller"
                : art.testing ? "Every press shows here and nowhere else"
                : art.learningSlot !== "" ? "Press the button on the controller"
                : art.unbound.length > 0 ? "Dashed buttons have no code on this connection: learn them"
                : "Press a button on the pad to see it light up"
            color: art.connected && art.learningSlot === "" ? Theme.textMuted : Theme.textSecondary
            font.family: Theme.sans
            font.weight: art.connected ? Font.Normal : Font.Medium
            font.pixelSize: Theme.dp(art.connected ? 19 : 24)
            wrapMode: Text.WordWrap
        }
    }
}
