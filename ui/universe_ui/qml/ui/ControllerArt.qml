import QtQuick
import "../core"
import "PadHistory.js" as History

Item {
    id: live

    property string family: "dualsense"
    property bool connected: false
    property var rows: []
    property var unbound: []
    property var pressed: ({})
    property var axes: ({})
    property string learningSlot: ""
    // The walk's step, { prompt, index, count, seconds }, or null outside one.
    property var step: null
    // Newest first: { slot, label, dt }.
    property var entries: []
    property var log: History.fresh()

    signal stopRequested

    property real pulse: 0.15
    SequentialAnimation on pulse {
        running: live.learningSlot !== ""
        loops: Animation.Infinite
        NumberAnimation {
            to: 0.55
            duration: 500
            easing.type: Easing.InOutSine
        }
        NumberAnimation {
            to: 0.15
            duration: 500
            easing.type: Easing.InOutSine
        }
    }

    readonly property real pad: Theme.dp(8)
    readonly property real inset: Theme.dp(20)
    readonly property real columnWidth: Theme.dp(360)
    readonly property string lastSlot: entries.length > 0 ? entries[0].slot : ""
    readonly property bool lastIsTrigger: lastSlot === "lt" || lastSlot === "rt"
    readonly property real lastPull: lastIsTrigger ? (axes[lastSlot] || 0) : 0

    function press(slot, down) {
        var next = Object.assign({}, pressed);
        if (down)
            next[slot] = true;
        else
            delete next[slot];
        pressed = next;
        var logged = History.press(log, slot, down, labelOf(slot), Date.now());
        if (logged)
            entries = logged;
    }

    function axis(name, value) {
        var next = Object.assign({}, axes);
        next[name] = value;
        axes = next;
        var logged = History.axis(log, name, value, labelOf, Date.now());
        if (logged)
            entries = logged;
    }

    function clear() {
        pressed = ({});
        axes = ({});
        log = History.fresh();
        entries = [];
    }

    function labelOf(slot) {
        var r = rows.find(function (r) {
            return r.slot === slot;
        });
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

        x: live.inset
        y: live.pad + 1 + Theme.dp(6)
        width: Math.max(0, column.x - live.inset * 2)
        height: Math.max(0, live.height - y - Theme.dp(16))

        PadArt {
            anchors.fill: parent
            family: live.family
            unbound: live.connected ? live.unbound : []
            learningSlot: live.learningSlot
            pulse: live.pulse
            pressed: live.pressed
            axes: live.axes
            readouts: live.step === null
            opacity: live.connected ? 1.0 : 0.38

            Behavior on opacity {
                Ease {
                    duration: Theme.durScene
                }
            }
        }

        Text {
            anchors.centerIn: parent
            anchors.verticalCenterOffset: parent.height * 0.42
            visible: live.entries.length === 0 && live.step === null
            text: !live.connected ? "Connect a controller" : "Press anything on the pad"
            color: live.connected ? Theme.textSecondary : Theme.text
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(24)
        }

        Column {
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: Theme.dp(8)
            spacing: Theme.dp(6)
            visible: live.step !== null

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                text: live.step ? live.step.prompt : ""
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(30)
            }

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.dp(6)

                Text {
                    text: live.step ? "Step " + live.step.index + " of " + live.step.count + " · skipped in " + live.step.seconds + " s · " : ""
                    color: Theme.textSecondary
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(18)
                }

                Text {
                    text: "Stop (Esc)"
                    color: Theme.text
                    font.family: Theme.sans
                    font.weight: Font.Medium
                    font.pixelSize: Theme.dp(18)

                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: live.stopRequested()
                    }
                }
            }
        }
    }

    Rectangle {
        id: column

        x: parent.width - width - live.inset
        y: live.inset
        width: live.columnWidth
        height: parent.height - live.inset * 2
        radius: Theme.dp(18)
        color: Qt.rgba(1, 1, 1, 0.04)
        border.width: 1
        border.color: Qt.rgba(1, 1, 1, 0.10)

        Text {
            id: heading
            x: Theme.dp(30)
            y: Theme.dp(22)
            text: "HISTORY"
            color: Theme.textTab
            font.family: Theme.sans
            font.weight: Font.DemiBold
            font.pixelSize: Theme.dp(13)
            font.letterSpacing: Theme.dp(1.5)
        }

        Column {
            anchors.top: heading.bottom
            anchors.topMargin: Theme.dp(12)
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Theme.dp(16)
            anchors.rightMargin: Theme.dp(16)
            spacing: Theme.dp(4)

            Repeater {
                model: live.entries

                Rectangle {
                    readonly property bool newest: index === 0
                    readonly property var entry: modelData || ({})
                    readonly property color ink: newest ? Theme.text : Theme.textSecondary
                    // A gap under 30 ms is not a human's second press: the pad double-fired.
                    readonly property bool doubled: entry.dt !== null && entry.dt !== undefined && entry.dt < 30

                    width: parent.width
                    height: Theme.dp(48)
                    radius: Theme.dp(12)
                    color: newest ? Qt.rgba(1, 1, 1, 0.06) : "transparent"

                    PadGlyph {
                        id: glyph
                        x: Theme.dp(14)
                        anchors.verticalCenter: parent.verticalCenter
                        family: live.family
                        slot: entry.slot || ""
                        unit: Theme.dp(30)
                        ink: parent.ink
                    }

                    Text {
                        anchors.left: glyph.right
                        anchors.leftMargin: Theme.dp(14)
                        anchors.right: gap.left
                        anchors.rightMargin: Theme.dp(10)
                        anchors.verticalCenter: parent.verticalCenter
                        text: entry.label || ""
                        color: parent.ink
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.weight: Font.Medium
                        font.pixelSize: Theme.dp(20)
                    }

                    Text {
                        id: gap
                        anchors.right: parent.right
                        anchors.rightMargin: Theme.dp(14)
                        anchors.verticalCenter: parent.verticalCenter
                        text: entry.dt === null || entry.dt === undefined ? "" : "+" + entry.dt + " ms"
                        color: parent.doubled ? "#ff8a65" : Theme.textTab
                        font.family: Theme.sans
                        font.weight: Font.DemiBold
                        font.pixelSize: Theme.dp(16)
                    }
                }
            }
        }
    }
}
