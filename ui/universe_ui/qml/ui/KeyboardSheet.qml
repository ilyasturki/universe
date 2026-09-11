import QtQuick
import "../core"
import "../sound"

// A value typed at the gamepad: the search sheet's field and keyboard, with a done key.
FocusScope {
    id: sheet

    property bool open: false
    property string title: ""
    property string text: ""
    property bool symbols: false
    property bool secret: false

    signal accepted(string value)
    signal dismissed()

    readonly property var hints: [
        { glyph: "A", label: "Type" },
        { glyph: "X", label: "Backspace" },
        { glyph: "Y", label: "Done" },
        { glyph: "B", label: "Cancel" }
    ]

    readonly property real inner: Math.min(Theme.dp(1000), width - Theme.dp(280))
    readonly property real pad: Theme.dp(28)
    readonly property real fieldHeight: Theme.dp(66)

    function show(label, value, withSymbols) {
        title = label;
        text = value === undefined || value === null ? "" : String(value);
        symbols = withSymbols === true;
        keyboard.shift = false;
        open = true;
        forceActiveFocus();
    }

    function finish() {
        open = false;
        accepted(text);
    }

    function cancel() {
        open = false;
        Sound.cancel();
        dismissed();
    }

    visible: scrim.opacity > 0.01
    focus: open

    Keys.onLeftPressed: keyboard.move(0, -1) ? Sound.kbtick() : Sound.edge()
    Keys.onRightPressed: keyboard.move(0, 1) ? Sound.kbtick() : Sound.edge()
    Keys.onUpPressed: keyboard.move(-1, 0) ? Sound.kbtick() : Sound.edge()
    Keys.onDownPressed: keyboard.move(1, 0) ? Sound.kbtick() : Sound.edge()

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event)) {
            keyboard.press();
        } else if (api.keys.isCancel(event)) {
            cancel();
        } else if (api.keys.isDetails(event)) {
            Sound.backspace();
            text = text.slice(0, -1);
        } else if (api.keys.isFilters(event)) {
            Sound.enter();
            finish();
        }
    }

    Rectangle {
        id: scrim
        anchors.fill: parent
        color: Qt.rgba(0.02, 0.02, 0.03, 1)
        opacity: sheet.open ? 0.72 : 0.0

        Behavior on opacity {
            NumberAnimation { duration: Theme.durBase; easing.type: Easing.OutCubic }
        }
    }

    Item {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        height: sheet.pad * 2 + heading.height + Theme.dp(18) + sheet.fieldHeight + Theme.dp(22) + keyboard.height
        y: sheet.open ? parent.height - height - Theme.dp(Theme.hintBarHeight) : parent.height

        Behavior on y {
            NumberAnimation { duration: Theme.durView; easing.type: Easing.OutQuint }
        }

        Rectangle {
            anchors.fill: parent
            anchors.bottomMargin: -Theme.dp(120)
            radius: Theme.dp(30)
            color: Qt.rgba(0.071, 0.075, 0.094, 1.0)
            border.width: 1
            border.color: Theme.surfaceBorder
        }

        Text {
            id: heading
            anchors.top: parent.top
            anchors.topMargin: sheet.pad
            anchors.horizontalCenter: parent.horizontalCenter
            width: sheet.inner
            text: sheet.title
            color: Theme.textSecondary
            font.family: Theme.sans
            font.weight: Font.Medium
            font.pixelSize: Theme.dp(22)
        }

        Rectangle {
            id: field

            anchors.top: heading.bottom
            anchors.topMargin: Theme.dp(18)
            anchors.horizontalCenter: parent.horizontalCenter
            width: sheet.inner
            height: sheet.fieldHeight
            radius: Theme.dp(16)
            color: Theme.surface

            Text {
                id: valueText
                anchors.left: parent.left
                anchors.leftMargin: Theme.dp(26)
                anchors.right: parent.right
                anchors.rightMargin: Theme.dp(26)
                anchors.verticalCenter: parent.verticalCenter
                text: sheet.secret ? sheet.text.replace(/./g, "•") : sheet.text
                color: Theme.text
                font.family: Theme.sans
                font.weight: Font.Medium
                font.pixelSize: Theme.dp(27)
                elide: Text.ElideLeft
            }

            Rectangle {
                anchors.left: valueText.left
                anchors.leftMargin: Math.min(valueText.implicitWidth, valueText.width) + Theme.dp(10)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(3)
                height: Theme.dp(30)
                color: Theme.text
                visible: sheet.open && caret.on
            }
        }

        Timer {
            id: caret
            property bool on: true
            interval: 560
            running: sheet.open
            repeat: true
            onTriggered: on = !on
        }

        VirtualKeyboard {
            id: keyboard

            anchors.top: field.bottom
            anchors.topMargin: Theme.dp(22)
            anchors.horizontalCenter: parent.horizontalCenter
            width: sheet.inner
            height: implicitHeight
            keyHeight: Theme.dp(52)
            keyGap: Theme.dp(9)
            showDone: true
            symbols: sheet.symbols

            onCharEntered: function(value) {
                Sound.type();
                sheet.text += value;
            }
            onBackspaced: {
                Sound.backspace();
                sheet.text = sheet.text.slice(0, -1);
            }
            onCleared: {
                Sound.backspace();
                sheet.text = "";
            }
            onDone: {
                Sound.enter();
                sheet.finish();
            }
        }
    }
}
