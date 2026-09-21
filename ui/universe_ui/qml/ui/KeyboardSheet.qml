import QtQuick
import "../core"
import "../sound"

Sheet {
    id: sheet

    property string text: ""
    property bool symbols: false
    property bool numeric: false

    signal accepted(string value)
    signal dismissed

    readonly property var hints: api.keys.mode === "keyboard" ? [
        {
            glyph: "A",
            label: "Done"
        },
        {
            glyph: "B",
            label: "Cancel"
        }
    ] : [
        {
            glyph: "A",
            label: "Type"
        },
        {
            glyph: "X",
            label: "Backspace"
        },
        {
            glyph: "Y",
            label: "Done"
        },
        {
            glyph: "B",
            label: "Cancel"
        }
    ]

    readonly property real fieldHeight: Theme.dp(66)

    contentHeight: Theme.dp(18) + fieldHeight + Theme.dp(22) + keyboard.height

    // mode: "text", "path" (adds the path symbols) or "number" (a keypad).
    function show(label, value, mode) {
        title = label;
        text = value === undefined || value === null ? "" : String(value);
        symbols = mode === "path";
        numeric = mode === "number";
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

    Keys.onLeftPressed: keyboard.move(0, -1) ? Sound.kbtick() : Sound.edge()
    Keys.onRightPressed: keyboard.move(0, 1) ? Sound.kbtick() : Sound.edge()
    Keys.onUpPressed: keyboard.move(-1, 0) ? Sound.kbtick() : Sound.edge()
    Keys.onDownPressed: keyboard.move(1, 0) ? Sound.kbtick() : Sound.edge()

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (keyboard.typed(event))
            return;
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
        id: field

        anchors.top: sheet.head.bottom
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
            text: sheet.text
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
        numeric: sheet.numeric

        onCharEntered: function (value) {
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
