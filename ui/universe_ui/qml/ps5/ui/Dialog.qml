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
        present(done);
    }

    card.width: Math.min(Theme.dp(1040), width - Theme.dp(160))
    card.height: body.height + buttonRow.height + Theme.dp(64 + 56 + 40)

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
            Sound.play("edge");
        }
    }

    Column {
        id: body

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.leftMargin: Theme.dp(64)
        anchors.rightMargin: Theme.dp(64)
        anchors.topMargin: Theme.dp(56)
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
