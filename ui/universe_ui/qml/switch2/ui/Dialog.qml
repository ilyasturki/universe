import QtQuick
import "../core"
import "../sound"

FocusScope {
    id: dialog

    property bool open: false
    property string message: ""
    property string detail: ""
    property var buttons: []
    property int index: 0
    property int dangerIndex: -1
    property var callback: null

    readonly property var hints: [ { glyph: "B", label: "Back" }, { glyph: "A", label: "OK" } ]

    function show(spec, done) {
        message = spec.message || "";
        detail = spec.detail || "";
        buttons = spec.buttons || ["OK"];
        index = spec.index !== undefined ? spec.index : buttons.length - 1;
        dangerIndex = spec.danger !== undefined ? spec.danger : -1;
        callback = done || null;
        Sound.open();
        open = true;
        forceActiveFocus();
    }

    function choose(i) {
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
            Sound.ok();
            choose(index);
        } else if (api.keys.isCancel(event)) {
            Sound.back();
            choose(0);
        } else if (event.key === Qt.Key_Left || event.key === Qt.Key_Right) {
            var next = Math.max(0, Math.min(buttons.length - 1, index + (event.key === Qt.Key_Left ? -1 : 1)));
            next === index ? Sound.edge() : Sound.tick();
            index = next;
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
            Sound.edge();
        }
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Theme.scrim
        opacity: dialog.open ? 1.0 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
    }

    Rectangle {
        id: card

        anchors.centerIn: parent
        width: Theme.dp(1072)
        height: Math.max(Theme.dp(420), body.height + Theme.dp(120) + buttonRow.height)
        radius: Theme.dp(6)
        color: Theme.card
        opacity: dialog.open ? 1.0 : 0.0
        scale: dialog.open ? 1.0 : 0.98

        Behavior on opacity {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }
        Behavior on scale {
            NumberAnimation { duration: Theme.durQuick; easing.type: Easing.OutCubic }
        }

        Column {
            id: body

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: Theme.dp(160)
            anchors.rightMargin: Theme.dp(120)
            anchors.verticalCenter: parent.verticalCenter
            anchors.verticalCenterOffset: -buttonRow.height / 2
            spacing: Theme.dp(10)

            Text {
                width: parent.width
                text: dialog.message
                color: Theme.text
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontBody)
                wrapMode: Text.WordWrap
                lineHeight: 1.25
            }

            Text {
                width: parent.width
                visible: dialog.detail !== ""
                text: dialog.detail
                color: Theme.textSecondary
                font.family: Theme.sans
                font.pixelSize: Theme.dp(Theme.fontSmall)
                wrapMode: Text.WordWrap
                lineHeight: 1.25
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: buttonRow.top
            height: 1
            color: Theme.hairlineSoft
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

                    Rectangle {
                        id: fill
                        anchors.fill: parent
                        // Inset so the ring stays within the card
                        anchors.margins: Theme.dp(Theme.ringRoomTight)
                        radius: Theme.dp(Theme.radiusRow)
                        color: Theme.focusFill
                        visible: button.focused
                    }

                    FocusOutline {
                        target: fill
                        cornerRadius: fill.radius
                        gap: 0
                        shown: button.focused && dialog.open
                    }

                    Text {
                        anchors.centerIn: parent
                        text: modelData
                        color: button.danger ? Theme.danger : Theme.accent
                        font.family: Theme.sans
                        font.pixelSize: Theme.dp(Theme.fontBody)
                    }
                }
            }
        }
    }
}
