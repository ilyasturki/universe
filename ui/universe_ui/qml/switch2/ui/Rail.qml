import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: rail

    property var items: []
    property int index: 0

    signal activated(string id)
    signal escapedRight()

    width: Theme.dp(110)

    function step(d) {
        var next = Math.max(0, Math.min(items.length - 1, index + d));
        next === index ? Sound.edge() : Sound.tick();
        index = next;
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: Sound.edge()
    Keys.onRightPressed: {
        Sound.tick();
        rail.escapedRight();
    }

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (items.length > 0) {
                Sound.ok();
                rail.activated(items[index].id);
            }
        }
    }

    Column {
        spacing: Theme.dp(30)

        Repeater {
            model: rail.items

            Item {
                readonly property bool focused: rail.activeFocus && index === rail.index

                width: Theme.dp(80)
                height: Theme.dp(80)

                Rectangle {
                    id: disc
                    anchors.fill: parent
                    radius: width / 2
                    color: Theme.focusFill
                    visible: parent.focused
                }

                FocusOutline {
                    target: disc
                    cornerRadius: disc.radius
                    gap: 0
                    shown: parent.focused
                }

                Glyph {
                    anchors.centerIn: parent
                    width: Theme.dp(44)
                    height: width
                    kind: modelData.icon
                    tint: Theme.text
                }

                Text {
                    anchors.left: parent.right
                    anchors.leftMargin: Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    visible: parent.focused && modelData.label !== undefined
                    text: modelData.label || ""
                    color: Theme.accent
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(Theme.fontBody)
                }
            }
        }
    }
}
