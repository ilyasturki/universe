import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: list

    property var sections: []
    property int index: 0
    property int current: 0
    readonly property bool cursorShown: activeFocus

    signal activated(int index)
    signal escapedRight()

    readonly property real rowHeight: Theme.dp(123)
    readonly property real textX: Theme.dp(68)
    // The view clips; it reaches this far past the rows so the focus ring is never cut.
    readonly property real room: Theme.dp(Theme.ringRoom)

    function step(d) {
        var next = Math.max(0, Math.min(sections.length - 1, index + d));
        if (next === index) {
            Sound.edge();
            return;
        }
        Sound.tick();
        index = next;
        list.activated(index);
    }

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onRightPressed: {
        Sound.tick();
        list.escapedRight();
    }
    Keys.onLeftPressed: Sound.edge()

    Keys.onPressed: function(event) {
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            Sound.ok();
            list.activated(index);
            list.escapedRight();
        }
    }

    onIndexChanged: view.scrollToCurrent()

    ListView {
        id: view

        anchors.fill: parent
        anchors.margins: -list.room
        model: list.sections
        interactive: false
        clip: true
        currentIndex: list.index
        highlightFollowsCurrentItem: false
        header: Item { height: list.room }
        footer: Item { height: list.room }

        function scrollToCurrent() {
            var top = list.index * list.rowHeight, bottom = top + list.rowHeight + list.room * 2;
            Theme.reveal(view, top, bottom, height);
        }

        Behavior on contentY {
            NumberAnimation { duration: Theme.durPage; easing.type: Easing.OutCubic }
        }

        delegate: Item {
            width: view.width
            height: list.rowHeight

            Item {
                id: line

                readonly property bool focused: list.cursorShown && index === list.index
                readonly property bool open: index === list.current
                readonly property bool ruled: index > 0 && modelData.group !== undefined && list.sections[index - 1].group !== modelData.group

                x: list.room
                width: parent.width - list.room * 2
                height: list.rowHeight

                Rectangle {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.leftMargin: Theme.dp(36)
                    anchors.rightMargin: Theme.dp(36)
                    height: 1
                    visible: line.ruled
                    color: Theme.hairlineSoft
                }

                Rectangle {
                    id: pill
                    anchors.fill: parent
                    anchors.margins: Theme.dp(6)
                    radius: Theme.dp(Theme.radiusRow)
                    color: Theme.focusFill
                    visible: line.focused
                }

                FocusOutline {
                    target: pill
                    cornerRadius: pill.radius
                    gap: 0
                    shown: line.focused
                }

                Rectangle {
                    x: Theme.dp(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(6)
                    height: Theme.dp(64)
                    color: Theme.accent
                    visible: line.open
                }

                Column {
                    x: list.textX
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - Theme.dp(24)
                    spacing: Theme.dp(2)

                    Text {
                        width: parent.width
                        text: modelData.label
                        color: line.open ? Theme.accent : Theme.text
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }

                    Text {
                        width: parent.width
                        visible: modelData.detail !== undefined && modelData.detail !== ""
                        text: modelData.detail || ""
                        color: Theme.textSecondary
                        elide: Text.ElideRight
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }
            }
        }
    }
}
