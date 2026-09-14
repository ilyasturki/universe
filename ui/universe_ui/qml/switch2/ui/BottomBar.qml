import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: bar

    property var items: []
    property int index: 0

    signal activated(var item)
    signal escapedUp()

    readonly property var hints: [ { glyph: "A", label: "OK" } ]
    readonly property real iconSize: Theme.dp(58)
    readonly property real discSize: Theme.dp(96)
    // 123: the Switch's icon pitch; fewer icons shorten the pill, never widen the gap.
    readonly property real pitch: Theme.dp(123)
    readonly property real endPad: Theme.dp(50)

    width: endPad * 2 + Math.max(0, items.length - 1) * pitch + iconSize
    height: Theme.dp(Theme.barHeight)

    function step(d) {
        var next = index + d;
        if (next < 0 || next >= items.length) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
    }

    Keys.onLeftPressed: step(-1)
    Keys.onRightPressed: step(1)
    Keys.onUpPressed: {
        Sound.tick();
        bar.escapedUp();
    }
    Keys.onDownPressed: Sound.edge()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (items.length > 0) {
                Sound.ok();
                bar.activated(items[index]);
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        radius: height / 2
        color: Theme.bar
    }

    Repeater {
        model: bar.items

        Item {
            id: cell

            readonly property bool focused: bar.activeFocus && index === bar.index

            x: bar.endPad + index * bar.pitch + bar.iconSize / 2 - width / 2
            y: (bar.height - height) / 2
            width: bar.discSize
            height: bar.discSize

            Rectangle {
                id: disc
                anchors.fill: parent
                radius: width / 2
                color: Theme.disc
                opacity: cell.focused ? 1.0 : 0.0
            }

            FocusOutline {
                target: disc
                cornerRadius: disc.radius
                shown: cell.focused
            }

            Glyph {
                anchors.centerIn: parent
                width: bar.iconSize
                height: bar.iconSize
                kind: modelData.icon
                tint: modelData.color || Theme.barGrey
                stroke: 1.7
            }

            Text {
                anchors.top: parent.bottom
                anchors.topMargin: Theme.dp(Theme.ringRoom + 4)
                anchors.horizontalCenter: parent.horizontalCenter
                visible: cell.focused
                text: modelData.label
                color: Theme.accent
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontBody)
            }
        }
    }
}
