import QtQuick
import "../core"
import "../sound"

// The Game Library's left column of round icons; the focused one turns white and says its name beside it.
FocusScope {
    id: rail

    // [{ id, glyph, label }]
    property var items: []
    property int index: 0
    readonly property bool cursorShown: activeFocus

    signal activated(string id)
    signal escapedRight
    signal pointed

    readonly property real size: Theme.dp(80)
    readonly property real pitch: Theme.dp(104)

    width: size
    height: items.length * pitch

    function point(i) {
        Sound.play("tick");
        index = i;
        if (!activeFocus)
            rail.pointed();
        forceActiveFocus();
    }

    Keys.onUpPressed: index = Sound.stepped(index, -1, items.length)
    Keys.onDownPressed: index = Sound.stepped(index, 1, items.length)
    Keys.onRightPressed: {
        Sound.play("tick");
        rail.escapedRight();
    }
    Keys.onLeftPressed: Sound.play("edge")
    Keys.onPressed: function (event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event) && items[index]) {
            event.accepted = true;
            Sound.play("ok");
            rail.activated(items[index].id);
        }
    }

    Repeater {
        model: rail.items

        Item {
            id: item

            readonly property bool focused: rail.cursorShown && index === rail.index

            y: index * rail.pitch
            width: rail.size
            height: rail.size

            Rectangle {
                anchors.fill: parent
                radius: width / 2
                color: "#ffffff"
                opacity: item.focused ? 1.0 : 0.0
                scale: item.focused ? 1.0 : 0.85

                Behavior on opacity {
                    Ease {}
                }
                Behavior on scale {
                    Ease {}
                }
            }

            Rectangle {
                anchors.fill: parent
                anchors.margins: -Theme.dp(6)
                radius: width / 2
                color: "transparent"
                border.width: Theme.dp(2)
                border.color: Qt.rgba(1, 1, 1, 0.42)
                opacity: item.focused ? 1.0 : 0.0

                Behavior on opacity {
                    Ease {}
                }
            }

            Glyph {
                anchors.centerIn: parent
                width: Theme.dp(40)
                height: width
                kind: modelData.glyph
                tint: item.focused ? Theme.onLight : Theme.text
                stroke: 2
            }

            Rectangle {
                x: rail.size + Theme.dp(18)
                anchors.verticalCenter: parent.verticalCenter
                width: tip.implicitWidth + Theme.dp(24)
                height: tip.implicitHeight + Theme.dp(10)
                radius: Theme.dp(4)
                color: Qt.rgba(0.05, 0.06, 0.08, 0.82)
                opacity: item.focused ? 1.0 : 0.0
                visible: opacity > 0.01
                z: 2

                Behavior on opacity {
                    Ease {}
                }

                Label {
                    id: tip
                    anchors.centerIn: parent
                    text: modelData.label
                    font.pixelSize: Theme.dp(Theme.fontSmall)
                }
            }

            Touch {
                current: item.focused
                onPicked: rail.point(index)
            }
        }
    }
}
