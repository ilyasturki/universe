import QtQuick
import "../core"
import "../sound"

// The column of round buttons left of a grid (Sort, Filter): the focused one white, its name beside it.
FocusScope {
    id: rail

    // [{ id, glyph, label }]
    property var items: []
    property int index: 0
    readonly property real size: Theme.dp(72)

    signal activated(string id)
    signal escapedRight
    signal pointed

    width: size
    height: items.length * (size + Theme.dp(32))

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
        if (api.keys.isAccept(event) && items.length > 0) {
            event.accepted = true;
            Sound.play("ok");
            rail.activated(items[index].id);
        }
    }

    Column {
        spacing: Theme.dp(32)

        Repeater {
            model: rail.items

            Item {
                id: button

                readonly property bool focused: rail.activeFocus && index === rail.index

                width: rail.size
                height: rail.size

                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    color: button.focused ? "#ffffff" : Qt.rgba(0.16, 0.17, 0.21, 0.8)

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.durFocus
                        }
                    }
                }

                Rectangle {
                    anchors.fill: parent
                    anchors.margins: -Theme.dp(6)
                    radius: width / 2
                    color: "transparent"
                    border.width: Theme.dp(2)
                    border.color: Qt.rgba(1, 1, 1, 0.42)
                    opacity: button.focused ? 1.0 : 0.0

                    Behavior on opacity {
                        Ease {}
                    }
                }

                Glyph {
                    anchors.centerIn: parent
                    width: Theme.dp(34)
                    height: width
                    kind: modelData.glyph
                    tint: button.focused ? Theme.onLight : Theme.text
                }

                Label {
                    anchors.left: parent.right
                    anchors.leftMargin: Theme.dp(22)
                    anchors.verticalCenter: parent.verticalCenter
                    text: modelData.label || ""
                    opacity: button.focused ? 1.0 : 0.0
                    font.pixelSize: Theme.dp(Theme.fontSmall)

                    Behavior on opacity {
                        Ease {}
                    }
                }

                Touch {
                    direct: true
                    onPicked: {
                        Sound.play("tick");
                        rail.index = index;
                        if (!rail.activeFocus)
                            rail.pointed();
                        rail.forceActiveFocus();
                    }
                }
            }
        }
    }
}
