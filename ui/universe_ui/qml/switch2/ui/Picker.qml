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
    readonly property bool hasIcons: icons.some(function (i) {
        return i !== undefined && i !== "";
    })

    readonly property var hints: [
        {
            glyph: "B",
            label: "Back"
        },
        {
            glyph: "A",
            label: "OK"
        }
    ]
    readonly property real rowHeight: Theme.dp(100)
    readonly property real room: Theme.dp(Theme.ringRoom)

    function show(spec, done) {
        title = spec.title || "";
        choices = spec.choices || [];
        icons = spec.icons || [];
        current = spec.index !== undefined ? spec.index : -1;
        index = Math.max(0, current);
        present(done);
        list.positionViewAtIndex(index, ListView.Contain);
    }

    card.width: Theme.dp(1000)
    card.height: heading.height + Theme.dp(20) + list.height - picker.room * 2 + Theme.dp(40)

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            Sound.play("select");
            finish(index);
        } else if (api.keys.isCancel(event)) {
            Sound.play("back");
            finish(-1);
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            index = Sound.stepped(index, event.key === Qt.Key_Up ? -1 : 1, choices.length);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            Sound.play("edge");
        }
    }

    Label {
        id: heading
        x: Theme.dp(60)
        y: Theme.dp(20)
        height: picker.title !== "" ? Theme.dp(80) : 0
        verticalAlignment: Text.AlignVCenter
        visible: picker.title !== ""
        text: picker.title
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontSmall)
    }

    ListView {
        id: list

        x: Theme.dp(40) - picker.room
        y: heading.y + heading.height + Theme.dp(10) - picker.room
        width: parent.width - Theme.dp(80) + picker.room * 2
        height: picker.rowHeight * Math.min(7, Math.max(1, picker.choices.length)) + picker.room * 2
        model: picker.choices
        currentIndex: picker.index
        interactive: false
        clip: true
        highlightFollowsCurrentItem: true
        preferredHighlightBegin: picker.room
        preferredHighlightEnd: height - picker.room
        highlightRangeMode: ListView.ApplyRange
        header: Item {
            height: picker.room
        }
        footer: Item {
            height: picker.room
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
                    visible: !parent.focused && index < picker.choices.length - 1
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
                    x: Theme.dp(30) + (picker.hasIcons ? mark.width + Theme.dp(22) : 0)
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - Theme.dp(120) - (x - Theme.dp(30))
                    text: modelData
                    color: parent.chosen ? Theme.accent : Theme.text
                    elide: Text.ElideRight
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
