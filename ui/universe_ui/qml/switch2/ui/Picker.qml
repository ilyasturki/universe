import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: picker

    property bool open: false
    property string title: ""
    property var choices: []
    property int index: 0
    property int current: -1
    property var callback: null

    readonly property var hints: [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]
    readonly property real rowHeight: Theme.dp(100)
    readonly property int visibleRows: 7

    function show(spec, done) {
        title = spec.title || "";
        choices = spec.choices || [];
        current = spec.index !== undefined ? spec.index : -1;
        index = Math.max(0, current);
        callback = done || null;
        Sound.open();
        open = true;
        forceActiveFocus();
        list.positionViewAtIndex(index, ListView.Contain);
    }

    function finish(i) {
        var cb = callback;
        callback = null;
        open = false;
        if (cb)
            cb(i);
    }

    anchors.fill: parent
    visible: scrim.opacity > 0.01
    focus: open

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            Sound.select();
            finish(index);
        } else if (api.keys.isCancel(event)) {
            Sound.back();
            finish(-1);
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            var next = Math.max(0, Math.min(choices.length - 1, index + (event.key === Qt.Key_Up ? -1 : 1)));
            next === index ? Sound.edge() : Sound.tick();
            index = next;
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            Sound.edge();
        }
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Theme.scrim
        opacity: picker.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    Rectangle {
        id: card

        anchors.centerIn: parent
        width: Theme.dp(1000)
        height: heading.height + Theme.dp(20) + list.height + Theme.dp(40)
        radius: Theme.dp(6)
        color: Theme.card
        opacity: picker.open ? 1.0 : 0.0
        scale: picker.open ? 1.0 : 0.98

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
        Behavior on scale {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        Text {
            id: heading
            x: Theme.dp(60)
            y: Theme.dp(20)
            height: picker.title !== "" ? Theme.dp(80) : 0
            verticalAlignment: Text.AlignVCenter
            visible: picker.title !== ""
            text: picker.title
            color: Theme.textSecondary
            font.family: Theme.sans
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        ListView {
            id: list

            x: Theme.dp(40)
            y: heading.y + heading.height + Theme.dp(10)
            width: parent.width - x * 2
            height: picker.rowHeight * Math.min(picker.visibleRows, Math.max(1, picker.choices.length))
            model: picker.choices
            currentIndex: picker.index
            interactive: false
            clip: true
            highlightFollowsCurrentItem: true
            preferredHighlightBegin: 0
            preferredHighlightEnd: height
            highlightRangeMode: ListView.ApplyRange

            delegate: Item {
                readonly property bool focused: index === picker.index
                readonly property bool chosen: index === picker.current

                width: list.width
                height: picker.rowHeight

                Rectangle {
                    id: pill
                    anchors.fill: parent
                    radius: Theme.dp(Theme.radiusRow)
                    color: Theme.focusFill
                    visible: parent.focused
                }

                FocusOutline {
                    target: pill
                    cornerRadius: pill.radius
                    gap: 0
                    shown: parent.focused && picker.open
                }

                Rectangle {
                    anchors.bottom: parent.bottom
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: 1
                    visible: !parent.focused && index < picker.choices.length - 1
                    color: Theme.hairlineSoft
                }

                Text {
                    x: Theme.dp(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Theme.dp(120)
                    text: modelData
                    color: parent.chosen ? Theme.accent : Theme.text
                    elide: Text.ElideRight
                    font.family: Theme.sans
                    font.pixelSize: Theme.dp(Theme.fontBody)
                }

                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: Theme.dp(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: Theme.dp(40)
                    height: width
                    radius: width / 2
                    color: parent.chosen ? Theme.accentStrong : "transparent"
                    border.width: Theme.dp(2)
                    border.color: parent.chosen ? Theme.accentStrong : Theme.hairline

                    Rectangle {
                        anchors.centerIn: parent
                        width: Theme.dp(14)
                        height: width
                        radius: width / 2
                        color: "#ffffff"
                        visible: parent.parent.chosen
                    }
                }
            }
        }
    }
}
