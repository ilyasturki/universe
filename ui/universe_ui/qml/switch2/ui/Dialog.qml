import QtQuick
import "../core"
import "../sound"

Modal {
    id: dialog

    property string message: ""
    property string detail: ""
    property var buttons: []
    property int index: 0
    property int dangerIndex: -1

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

    function show(spec, done) {
        message = spec.message || "";
        detail = spec.detail || "";
        buttons = spec.buttons || ["OK"];
        index = spec.index !== undefined ? spec.index : buttons.length - 1;
        dangerIndex = spec.danger !== undefined ? spec.danger : -1;
        bodyView.contentY = 0;
        present(done);
    }

    function scroll(d) {
        var next = Math.max(0, Math.min(bodyView.contentHeight - bodyView.height, bodyView.contentY + d * Theme.dp(120)));
        if (next === bodyView.contentY) {
            Sound.play("edge");
            return;
        }
        Sound.play("tick");
        bodyView.contentY = next;
    }

    card.width: Theme.dp(1072)
    card.height: Math.min(dialog.height - Theme.dp(120), Math.max(Theme.dp(420), body.height + Theme.dp(120) + buttonRow.height))

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            Sound.play("ok");
            finish(index);
        } else if (api.keys.isCancel(event)) {
            Sound.play("back");
            api.keys.dropHold();
            finish(0);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            index = Sound.stepped(index, event.key === Qt.Key_Left ? -1 : 1, buttons.length);
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            scroll(event.key === Qt.Key_Up ? -1 : 1);
        }
    }

    Flickable {
        id: bodyView

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: buttonRow.top
        anchors.leftMargin: Theme.dp(160)
        anchors.rightMargin: Theme.dp(120)
        anchors.topMargin: Theme.dp(60)
        anchors.bottomMargin: Theme.dp(60)
        contentWidth: width
        contentHeight: body.height
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            Ease {}
        }

        Column {
            id: body

            width: bodyView.width
            y: Math.max(0, (bodyView.height - height) / 2)
            spacing: Theme.dp(10)

            Label {
                width: parent.width
                text: dialog.message
                wrapMode: Text.WordWrap
                lineHeight: 1.25
            }

            Label {
                width: parent.width
                visible: dialog.detail !== ""
                text: dialog.detail
                color: Theme.textSecondary
                font.pixelSize: Theme.dp(Theme.fontSmall)
                wrapMode: Text.WordWrap
                lineHeight: 1.25
            }
        }
    }

    Swipe {
        flickable: bodyView
        ease: scrollEase
    }

    Scrollbar {
        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(60)
        anchors.top: bodyView.top
        anchors.bottom: bodyView.bottom
        flickable: bodyView
    }

    Hairline {
        anchors.bottom: buttonRow.top
    }

    Row {
        id: buttonRow

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: Theme.dp(113)

        Repeater {
            model: dialog.buttons

            Item {
                id: button

                readonly property bool focused: index === dialog.index
                readonly property bool danger: index === dialog.dangerIndex

                width: buttonRow.width / dialog.buttons.length
                height: buttonRow.height

                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    width: 1
                    visible: index > 0
                    color: Theme.hairlineSoft
                }

                FocusPill {
                    anchors.fill: parent
                    anchors.margins: Theme.dp(Theme.ringRoomTight)
                    focused: button.focused
                }

                Label {
                    anchors.centerIn: parent
                    text: modelData
                    color: button.danger ? Theme.danger : Theme.accent
                }

                Touch {
                    direct: true
                    onPicked: dialog.index = index
                }
            }
        }
    }
}
