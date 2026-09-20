import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: rail

    property var items: []
    property int index: 0

    signal activated(string id)
    signal escapedRight

    width: Theme.dp(110)

    function step(d) {
        index = Sound.stepped(index, d, items.length);
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onLeftPressed: Sound.play("edge")
    Keys.onRightPressed: {
        Sound.play("tick");
        rail.escapedRight();
    }

    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            if (items.length > 0) {
                Sound.play("ok");
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

                FocusPill {
                    anchors.fill: parent
                    radius: width / 2
                    focused: parent.focused
                }

                Glyph {
                    anchors.centerIn: parent
                    width: Theme.dp(44)
                    height: width
                    kind: modelData.icon
                    tint: Theme.text
                }

                Label {
                    anchors.left: parent.right
                    anchors.leftMargin: Theme.dp(20)
                    anchors.verticalCenter: parent.verticalCenter
                    visible: parent.focused && modelData.label !== undefined
                    text: modelData.label || ""
                    color: Theme.accent
                }
            }
        }
    }
}
