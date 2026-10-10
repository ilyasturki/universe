import QtQuick
import "../core"
import "../sound"

// Settings' first screen: one row per section, an icon and a name, fading out under the title as the list scrolls.
FocusScope {
    id: list
    objectName: "rootList"

    // [{ id, label, icon, detail, group }]: a `group` heads the run of sections that share it.
    property var sections: []
    property int index: 0
    readonly property bool cursorShown: activeFocus

    signal moved(int index)
    signal chosen(int index)

    readonly property real rowHeight: Theme.dp(98)
    readonly property real headerHeight: Theme.dp(64)
    readonly property real fade: Theme.dp(90)

    function heads(i) {
        return i >= 0 && i < sections.length && !!sections[i].group && (i === 0 || sections[i - 1].group !== sections[i].group);
    }

    // The top of row `i`'s own line, below the headers before it and its own.
    function topOf(i) {
        var y = i * rowHeight;
        for (var k = 0; k <= i; k++)
            if (heads(k))
                y += headerHeight;
        return y;
    }

    function step(d) {
        var next = Sound.stepped(index, d, sections.length);
        if (next !== index) {
            index = next;
            list.moved(index);
        }
    }

    function point(i) {
        Sound.play("tick");
        if (i !== index) {
            index = i;
            list.moved(index);
        }
        forceActiveFocus();
    }

    onIndexChanged: Theme.reveal(view, topOf(index) - (heads(index) ? headerHeight : 0) - fade, topOf(index) + rowHeight + Theme.dp(40), view.height)

    Keys.onUpPressed: step(-1)
    Keys.onDownPressed: step(1)
    Keys.onRightPressed: {
        list.chosen(index);
    }
    Keys.onLeftPressed: Sound.play("edge")

    Keys.onPressed: function (event) {
        var screen = api.keys.isScreenUp(event) ? -1 : api.keys.isScreenDown(event) ? 1 : 0;
        if (screen) {
            event.accepted = true;
            var next = Sound.paged(index, screen, 1, Math.floor(view.height / rowHeight), sections.length);
            if (next !== index) {
                index = next;
                list.moved(index);
            }
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            event.accepted = true;
            list.chosen(index);
        }
    }

    ListView {
        id: view

        anchors.fill: parent
        model: list.sections
        interactive: false
        clip: true
        currentIndex: list.index
        highlightFollowsCurrentItem: false
        topMargin: list.fade
        bottomMargin: Theme.dp(60)

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
            readonly property real head: list.heads(index) ? list.headerHeight : 0
            // Rows passing under the title fade out as the console's do.
            readonly property real shown: Math.max(0, Math.min(1, (y + head - view.contentY) / list.fade))

            width: view.width
            height: list.rowHeight + head
            opacity: 0.15 + 0.85 * shown

            Label {
                x: Theme.dp(28)
                y: line.head - height - Theme.dp(12)
                visible: line.head > 0
                text: modelData.group || ""
                color: Theme.textMuted
                font.pixelSize: Theme.dp(Theme.fontSmall)
            }

            Rectangle {
                y: line.head
                width: parent.width
                height: list.rowHeight
                radius: Theme.dp(Theme.radiusRow)
                color: line.focused ? Qt.rgba(1, 1, 1, 0.12) : "transparent"
                border.width: line.focused ? Theme.dp(Theme.ringLine) : 0
                border.color: Theme.ringSoft

                Behavior on color {
                    ColorAnimation {
                        duration: Theme.durFocus
                    }
                }
            }

            Glyph {
                x: Theme.dp(28)
                y: line.head + (list.rowHeight - height) / 2
                width: Theme.dp(40)
                height: width
                kind: modelData.icon || ""
            }

            Column {
                x: Theme.dp(96)
                y: line.head + (list.rowHeight - height) / 2
                width: parent.width - x - Theme.dp(30)
                spacing: Theme.dp(2)

                Label {
                    width: parent.width
                    text: modelData.label
                    elide: Text.ElideRight
                    font.pixelSize: Theme.dp(Theme.fontTitle)
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
                x: Theme.dp(96)
                anchors.bottom: parent.bottom
                width: parent.width - x - Theme.dp(20)
                height: 1
                color: Theme.hairline
                visible: !line.focused && index !== list.index - 1 && index < list.sections.length - 1 && !list.heads(index + 1)
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

    Scrollbar {
        anchors.left: parent.right
        anchors.leftMargin: Theme.dp(10)
        anchors.top: parent.top
        anchors.topMargin: list.fade
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(60)
        flickable: view
    }
}
