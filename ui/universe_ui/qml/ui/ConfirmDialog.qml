import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: dialog

    property bool open: false
    property string message: ""
    property string detail: ""
    property string yesLabel: "OK"
    property string noLabel: "Cancel"
    property int index: 1
    property var callback: null

    readonly property var hints: [
        { glyph: "A", label: "Select" },
        { glyph: "dpad", label: "Navigate" },
        { glyph: "B", label: "Cancel" }
    ]

    signal closed()

    function ask(spec, done) {
        message = spec.message || "";
        detail = spec.detail || "";
        yesLabel = spec.yes || "OK";
        noLabel = spec.no || "Cancel";
        index = spec.index !== undefined ? spec.index : 1;
        callback = done || null;
        Sound.panel();
        open = true;
        forceActiveFocus();
    }

    function choose(yes) {
        var cb = callback;
        callback = null;
        open = false;
        closed();
        if (cb)
            cb(yes);
    }

    focus: open
    visible: scrim.opacity > 0.01

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: dialog.open ? 0.62 : 0.0

        Behavior on opacity { Ease {} }
    }

    Rectangle {
        id: panel

        anchors.centerIn: parent
        width: Theme.dp(760)
        height: column.height + Theme.dp(96)
        radius: Theme.dp(28)
        color: "#1b1d24"
        border.width: 1
        border.color: Theme.surfaceBorder
        opacity: dialog.open ? 1.0 : 0.0
        scale: dialog.open ? 1.0 : 0.96

        Behavior on opacity { Ease {} }
        Behavior on scale { Ease {} }

        Column {
            id: column

            anchors.centerIn: parent
            width: parent.width - Theme.dp(96)
            spacing: Theme.dp(16)

            Text {
                width: parent.width
                text: dialog.message
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(30)
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }

            Text {
                width: parent.width
                visible: dialog.detail !== ""
                text: dialog.detail
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(22)
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                lineHeight: 1.2
            }

            Item { width: 1; height: Theme.dp(14) }

            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: Theme.dp(36)

                PillButton {
                    ghost: true
                    icon: ""
                    label: dialog.noLabel
                    focused: dialog.index === 0
                    dimmed: dialog.index !== 0
                }

                PillButton {
                    icon: ""
                    label: dialog.yesLabel
                    focused: dialog.index === 1
                    dimmed: dialog.index !== 1
                }
            }
        }
    }

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            index = Sound.stepped(index, event.key === Qt.Key_Left ? -1 : 1, 2);
        } else if (api.keys.isAccept(event)) {
            index === 1 ? Sound.enter() : Sound.cancel();
            choose(index === 1);
        } else if (api.keys.isCancel(event)) {
            Sound.cancel();
            api.keys.dropHold();
            choose(false);
        }
    }
    Keys.onReleased: function(event) { event.accepted = true; }
}
