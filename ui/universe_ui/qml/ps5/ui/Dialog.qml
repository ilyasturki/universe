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

    readonly property var hints: []

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

    card.width: Math.min(Theme.dp(1040), width - Theme.dp(160))
    card.height: Math.min(dialog.height - Theme.dp(120), body.height + buttonRow.height + Theme.dp(56 + 56 + 48))

    Keys.onPressed: function (event) {
        event.accepted = true;
        var arrow = event.key === Qt.Key_Left || event.key === Qt.Key_Right || event.key === Qt.Key_Up || event.key === Qt.Key_Down;
        if (event.isAutoRepeat && !arrow)
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
        objectName: "scroll"

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: buttonRow.top
        anchors.leftMargin: Theme.dp(64)
        anchors.rightMargin: Theme.dp(64)
        anchors.topMargin: Theme.dp(56)
        anchors.bottomMargin: Theme.dp(56)
        contentWidth: width
        contentHeight: body.height
        interactive: false
        clip: true

        Behavior on contentY {
            id: scrollEase
            NumberAnimation {
                duration: Theme.durScroll
                easing.type: Easing.OutCubic
            }
        }

        Column {
            id: body

            width: bodyView.width
            spacing: Theme.dp(18)

            Label {
                width: parent.width
                text: dialog.message
                wrapMode: Text.WordWrap
                lineHeight: 1.2
                font.pixelSize: Theme.dp(Theme.fontTitle)
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
        anchors.rightMargin: Theme.dp(30)
        anchors.top: bodyView.top
        anchors.bottom: bodyView.bottom
        flickable: bodyView
    }

    Row {
        id: buttonRow

        anchors.right: parent.right
        anchors.rightMargin: Theme.dp(64)
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Theme.dp(48)
        spacing: Theme.dp(28)
        layoutDirection: Qt.LeftToRight

        Repeater {
            model: dialog.buttons

            PillButton {
                text: modelData
                focused: dialog.open && index === dialog.index
                danger: index === dialog.dangerIndex
                height: Theme.dp(64)
                fontSize: Theme.dp(26)
                direct: true
                onPicked: dialog.index = index
            }
        }
    }
}
