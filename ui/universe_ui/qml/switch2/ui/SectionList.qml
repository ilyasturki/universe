import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: list

    // [{ id, label, detail, group, groupLabel }]: a hairline rules between groups, a `groupLabel` names the group over its first entry.
    property var sections: []
    property int index: 0
    readonly property bool cursorShown: activeFocus

    signal activated(int index)
    signal escapedRight

    readonly property real rowHeight: Theme.dp(123)
    readonly property real labelHeight: Theme.dp(44)
    readonly property real room: Theme.dp(Theme.ringRoom)

    function labelled(i) {
        var s = sections[i];
        return s !== undefined && s.groupLabel !== undefined && s.groupLabel !== "" && (i === 0 || sections[i - 1].group !== s.group);
    }

    function yOf(i) {
        var y = 0;
        for (var k = 0; k < i; k++)
            y += rowHeight + (labelled(k) ? labelHeight : 0);
        return y;
    }

    function step(d) {
        var next = Sound.stepped(index, d, sections.length);
        if (next === index)
            return;
        index = next;
        list.activated(index);
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
        header: Item {
            height: list.room
        }
        footer: Item {
            height: list.room
        }

        function scrollToCurrent() {
            var top = list.yOf(list.index), bottom = top + list.rowHeight + (list.labelled(list.index) ? list.labelHeight : 0) + list.room * 2;
            Theme.reveal(view, top, bottom, height);
        }

        Behavior on contentY {
            Ease {}
        }

        delegate: Item {
            readonly property bool labelled: list.labelled(index)

            width: view.width
            height: list.rowHeight + (labelled ? list.labelHeight : 0)

            Label {
                x: list.room + Theme.dp(36)
                y: Theme.dp(14)
                visible: labelled
                text: (modelData.groupLabel || "").toUpperCase()
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontTiny)
                font.letterSpacing: Theme.dp(2)
            }

            Item {
                id: line

                readonly property bool focused: list.cursorShown && index === list.index
                readonly property bool open: index === list.index
                readonly property bool ruled: index > 0 && modelData.group !== undefined && list.sections[index - 1].group !== modelData.group

                x: list.room
                y: labelled ? list.labelHeight : 0
                width: parent.width - list.room * 2
                height: list.rowHeight

                Hairline {
                    anchors.bottom: undefined
                    anchors.top: parent.top
                    anchors.topMargin: labelled ? -list.labelHeight : 0
                    anchors.leftMargin: Theme.dp(36)
                    anchors.rightMargin: Theme.dp(36)
                    visible: line.ruled
                }

                FocusPill {
                    anchors.fill: parent
                    anchors.margins: Theme.dp(6)
                    focused: line.focused
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
                    x: Theme.dp(68)
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - x - Theme.dp(24)
                    spacing: Theme.dp(2)

                    Label {
                        width: parent.width
                        text: modelData.label
                        color: line.open ? Theme.accent : Theme.text
                        elide: Text.ElideRight
                    }

                    Label {
                        width: parent.width
                        visible: modelData.detail !== undefined && modelData.detail !== ""
                        text: modelData.detail || ""
                        color: Theme.textSecondary
                        elide: Text.ElideRight
                        font.pixelSize: Theme.dp(Theme.fontSmall)
                    }
                }
            }
        }
    }
}
