import QtQuick
import "../core"
import "../sound"

// A two-column page's left column: its sub-sections, the open one white, the rest dimmed.
FocusScope {
    id: list

    // [{ label, detail, changed }]: `changed` marks one holding a value set here.
    property var sections: []
    property int index: 0
    readonly property bool cursorShown: activeFocus

    signal activated(int index)
    signal escapedRight
    signal pointed

    readonly property real rowHeight: Theme.dp(104)

    function step(d) {
        var next = Sound.stepped(index, d, sections.length);
        if (next === index)
            return;
        index = next;
        list.activated(index);
    }

    function point(i) {
        Sound.play("tick");
        if (i !== index) {
            index = i;
            list.activated(index);
        }
        if (!activeFocus)
            list.pointed();
        forceActiveFocus();
    }

    function stepScreen(d) {
        var next = Sound.paged(index, d, 1, Math.floor(height / rowHeight), sections.length);
        if (next === index)
            return;
        index = next;
        list.activated(index);
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onRightPressed: {
        Sound.play("tick");
        list.escapedRight();
    }
    Keys.onLeftPressed: Sound.play("edge")

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            stepScreen(screen);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            Sound.play("ok");
            list.activated(index);
            list.escapedRight();
        }
    }

    onIndexChanged: Theme.reveal(view, index * rowHeight, (index + 1) * rowHeight, view.height)

    ListView {
        id: view

        anchors.fill: parent
        anchors.leftMargin: -Theme.dp(24)
        anchors.rightMargin: -Theme.dp(24)
        model: list.sections
        interactive: false
        clip: true
        currentIndex: list.index
        highlightFollowsCurrentItem: false

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        delegate: Item {
            id: line

            readonly property bool focused: list.cursorShown && index === list.index
            readonly property bool open: index === list.index

            width: view.width
            height: list.rowHeight

            Rectangle {
                anchors.fill: parent
                anchors.topMargin: Theme.dp(8)
                anchors.bottomMargin: Theme.dp(8)
                radius: Theme.dp(Theme.radiusRow)
                color: line.focused ? Theme.focusFill : "transparent"
                border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
                border.color: Theme.ringSoft

                Behavior on color {
                    ColorAnimation {
                        duration: Theme.durFocus
                    }
                }
            }

            Column {
                x: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - Theme.dp(48) - (dot.visible ? dot.width + Theme.dp(16) : 0)
                spacing: Theme.dp(2)

                Label {
                    width: parent.width
                    text: modelData.label
                    color: line.open ? Theme.text : Theme.textMuted
                    elide: Text.ElideRight
                    font.weight: Font.Light
                    font.pixelSize: Theme.dp(Theme.fontTitle)

                    Behavior on color {
                        ColorAnimation {
                            duration: Theme.durFocus
                        }
                    }
                }

                Label {
                    width: parent.width
                    visible: modelData.detail !== undefined && modelData.detail !== ""
                    text: modelData.detail || ""
                    color: Theme.textMuted
                    elide: Text.ElideRight
                    font.pixelSize: Theme.dp(Theme.fontTiny)
                }
            }

            Rectangle {
                id: dot
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(24)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(12)
                height: width
                radius: width / 2
                visible: modelData.changed === true
                color: Theme.accent
            }

            Touch {
                current: line.focused
                onPicked: list.point(index)
            }
        }
    }

    Swipe {
        flickable: view
        ease: scrollEase
    }
}
