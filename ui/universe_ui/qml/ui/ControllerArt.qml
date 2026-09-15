import QtQuick
import "../core"

Item {
    id: art

    property string family: "dualsense"
    property bool connected: false
    property var rows: []
    property var unbound: []
    property var pressed: ({})
    property var axes: ({})
    property string lastSlot: ""

    readonly property real pad: Theme.dp(8)
    readonly property real inset: Theme.dp(20)
    readonly property bool lastIsTrigger: lastSlot === "lt" || lastSlot === "rt"
    readonly property real lastPull: lastIsTrigger ? (axes[lastSlot] || 0) : 0

    function press(slot, down) {
        var next = Object.assign({}, pressed);
        if (down)
            next[slot] = true;
        else
            delete next[slot];
        pressed = next;
        if (down)
            lastSlot = slot;
    }

    // A trigger names itself as soon as it moves, so a light pull reads under the pad too.
    function axis(name, value) {
        var next = Object.assign({}, axes);
        next[name] = value;
        axes = next;
        if ((name === "lt" || name === "rt") && value > 0.02)
            lastSlot = name;
    }

    function clear() {
        pressed = ({});
        axes = ({});
        lastSlot = "";
    }

    function labelOf(slot) {
        var r = rows.find(function(r) { return r.slot === slot; });
        return r ? r.label : slot;
    }

    onFamilyChanged: clear()

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
        height: Math.max(0, art.height - y - caption.height - Theme.dp(24))

        PadArt {
            anchors.fill: parent
            family: art.family
            unbound: art.connected ? art.unbound : []
            pressed: art.pressed
            axes: art.axes
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
        spacing: Theme.dp(10)

        Item {
            width: parent.width
            height: Theme.dp(40)

            Row {
                anchors.centerIn: parent
                visible: art.lastSlot !== ""
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

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: art.lastIsTrigger
                    text: "· " + Math.round(art.lastPull * 100) + " %"
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(26)
                }
            }

            Text {
                anchors.centerIn: parent
                visible: art.lastSlot === ""
                text: !art.connected ? "Connect a controller" : "Press anything on the pad"
                color: art.connected ? Theme.textSecondary : Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(24)
            }
        }
    }
}
