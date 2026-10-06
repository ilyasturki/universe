import QtQuick
import "../core"
import "../sound"

Modal {
    id: picker

    property string title: ""
    property var choices: []
    // One file per choice (a runner's logo), drawn before its label; empty entries draw nothing.
    property var icons: []
    property int index: 0
    property int current: -1
    // The row (a rect in the picker's space) the values open beside, as a popover; null: a centred card
    property var anchor: null
    readonly property bool anchored: anchor !== null
    readonly property bool hasIcons: icons.some(function (i) {
        return i !== undefined && i !== "";
    })

    // Y picks for all the games a game's setting reaches (`alt` says so); A keeps the pick to the game.
    property string alt: ""

    readonly property var hints: (alt !== "" ? [
            {
                glyph: "Y",
                label: alt
            }
        ] : []).concat([
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: alt !== "" ? "This game" : "OK"
        }
    ])
    readonly property real rowHeight: anchored ? Theme.dp(94) : Theme.dp(100)
    readonly property real inset: anchored ? Theme.dp(18) : Theme.dp(40)
    readonly property real pad: Theme.dp(22)
    readonly property real room: Theme.dp(Theme.ringRoom)
    readonly property int shownRows: Math.min(7, Math.max(1, choices.length))

    function show(spec, done) {
        if (!spec.choices || spec.choices.length === 0) {
            Sound.play("edge");
            return;
        }
        scrollEase.enabled = false;
        anchor = spec.anchor || null;
        title = anchored ? "" : spec.title || "";
        choices = spec.choices || [];
        icons = spec.icons || [];
        current = spec.index !== undefined ? spec.index : -1;
        alt = spec.alt || "";
        index = Math.max(0, current);
        present(done);
        list.contentY = list.originY + Math.max(0, index - shownRows + 1) * rowHeight;
        scrollEase.enabled = true;
    }

    scrimColor: anchored ? "transparent" : Theme.scrim
    card.anchors.centerIn: anchored ? undefined : picker
    card.x: anchored ? anchor.x + anchor.width / 2 : 0
    card.y: anchored ? Math.max(Theme.dp(Theme.headerHeight), Math.min(anchor.y - Theme.dp(12), picker.height - Theme.dp(Theme.hintBarHeight) - card.height - Theme.dp(12))) : 0
    card.width: anchored ? anchor.width / 2 + Theme.dp(14) : Theme.dp(1000)
    card.height: list.height - picker.room * 2 + (anchored ? picker.pad * 2 : heading.height + Theme.dp(60))
    card.radius: anchored ? Theme.dp(22) : Theme.dp(16)
    card.color: anchored ? "#f4f4f2" : Theme.dialog

    Keys.onPressed: function (event) {
        event.accepted = true;
        var vertical = event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !vertical)
            return;
        if (api.keys.isAccept(event)) {
            Sound.play("select");
            finish(index);
        } else if (api.keys.isFilters(event) && alt !== "") {
            Sound.play("select");
            finish(index, true);
        } else if (api.keys.isCancel(event)) {
            Sound.play("back");
            finish(-1);
        } else if (vertical) {
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, choices.length);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            Sound.play("edge");
        }
    }

    Repeater {
        model: picker.anchored ? 4 : 0

        Rectangle {
            z: -1
            anchors.fill: parent
            anchors.margins: -Theme.dp(3 + index * 4)
            anchors.topMargin: -Theme.dp(index * 3)
            radius: picker.card.radius + Theme.dp(3 + index * 4)
            color: Qt.rgba(0, 0, 0, 0.035)
        }
    }

    Label {
        id: heading
        x: Theme.dp(60)
        y: Theme.dp(20)
        width: parent.width - x * 2
        height: picker.title !== "" ? Theme.dp(80) : 0
        verticalAlignment: Text.AlignVCenter
        visible: picker.title !== ""
        text: picker.title
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
        elide: Text.ElideRight
    }

    ListView {
        id: list
        objectName: "choices"

        x: picker.inset - picker.room
        y: picker.anchored ? picker.pad - picker.room : heading.y + heading.height + Theme.dp(10) - picker.room
        width: parent.width - picker.inset * 2 + picker.room * 2
        height: picker.rowHeight * picker.shownRows + picker.room * 2
        model: picker.choices
        currentIndex: picker.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: false
        // show() places the list itself, once its rows are in.
        onCurrentIndexChanged: if (scrollEase.enabled)
            Theme.reveal(list, currentIndex * picker.rowHeight - picker.room, (currentIndex + 1) * picker.rowHeight + picker.room, height)
        header: Item {
            height: picker.room
        }
        footer: Item {
            height: picker.room
        }

        Behavior on contentY {
            id: scrollEase
            Ease {}
        }

        delegate: Item {
            width: list.width
            height: picker.rowHeight

            Item {
                readonly property bool focused: index === picker.index
                readonly property bool chosen: index === picker.current

                x: picker.room
                width: parent.width - picker.room * 2
                height: picker.rowHeight

                FocusPill {
                    anchors.fill: parent
                    focused: parent.focused
                }

                Hairline {
                    visible: !picker.anchored && !parent.focused && index < picker.choices.length - 1
                }

                Image {
                    id: mark
                    x: Theme.dp(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: picker.hasIcons ? Theme.dp(48) : 0
                    height: Theme.dp(48)
                    source: picker.icons[index] ? Qt.resolvedUrl("../../" + picker.icons[index]) : ""
                    asynchronous: true
                    fillMode: Image.PreserveAspectFit
                    sourceSize.height: 128
                    smooth: true
                    mipmap: true
                    visible: picker.hasIcons
                }

                Label {
                    x: (picker.anchored ? Theme.dp(36) : Theme.dp(30)) + (picker.hasIcons ? mark.width + Theme.dp(22) : 0)
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Theme.dp(120) - (x - Theme.dp(30))
                    text: modelData
                    color: (picker.anchored ? parent.focused : parent.chosen) ? Theme.accent : Theme.text
                    elide: Text.ElideRight
                }

                Touch {
                    direct: true
                    onPicked: picker.index = index
                }

                Rectangle {
                    anchors.right: parent.right
                    anchors.rightMargin: picker.anchored ? Theme.dp(37) : Theme.dp(30)
                    anchors.verticalCenter: parent.verticalCenter
                    width: picker.anchored ? Theme.dp(46) : Theme.dp(40)
                    height: width
                    radius: width / 2
                    color: parent.chosen ? "#0a5fd6" : "transparent"
                    border.width: Theme.dp(2)
                    border.color: parent.chosen ? "#0a5fd6" : Theme.hairline

                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width * 0.33
                        height: width
                        radius: width / 2
                        color: "#ffffff"
                        visible: parent.parent.chosen
                    }
                }
            }
        }
    }

    Swipe {
        flickable: list
        ease: scrollEase
    }
}
