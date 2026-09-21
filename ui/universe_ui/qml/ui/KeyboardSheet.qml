import QtQuick
import "../core"
import "../sound"

Sheet {
    id: sheet

    property string text: ""
    property bool symbols: false
    property bool numeric: false
    // Two fields (`labels` names them) over one keyboard: `text` is the one at `typing`, `values` keeps both.
    property var labels: []
    property var values: ["", ""]
    property int typing: 0
    readonly property bool pair: labels.length === 2
    readonly property string doneLabel: !pair ? "Done" : typing === 0 ? "Next" : "Save"

    signal accepted(string value)
    signal acceptedPair(string first, string second)
    signal dismissed

    readonly property var hints: api.keys.mode === "keyboard" ? [
        {
            glyph: "A",
            label: doneLabel
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
            label: doneLabel
        },
        {
            glyph: "B",
            label: "Cancel"
        }
    ]

    readonly property real fieldHeight: Theme.dp(66)
    readonly property real fieldGap: Theme.dp(14)

    contentHeight: Theme.dp(18) + fieldHeight + (pair ? fieldHeight + fieldGap : 0) + Theme.dp(22) + keyboard.height

    // mode: "text", "path" (adds the path symbols) or "number" (a keypad).
    function show(label, value, mode) {
        labels = [];
        title = label;
        text = value === undefined || value === null ? "" : String(value);
        symbols = mode === "path";
        numeric = mode === "number";
        keyboard.shift = false;
        open = true;
        forceActiveFocus();
    }

    // Two fields at once, the first one typed into first; Done on it moves to the second, on the second saves both.
    function showPair(label, names, first, second) {
        labels = names;
        values = [String(first || ""), String(second || "")];
        typing = 0;
        title = label;
        text = values[0];
        symbols = false;
        numeric = false;
        keyboard.shift = false;
        open = true;
        forceActiveFocus();
    }

    function switchField(k) {
        var kept = values.slice();
        kept[typing] = text;
        values = kept;
        typing = k;
        text = values[k];
    }

    // The first of a pair names the entry: it cannot be left empty.
    function finish() {
        if (pair && typing === 0 && text === "") {
            Sound.edge();
            return;
        }
        Sound.enter();
        if (pair && typing === 0) {
            switchField(1);
            return;
        }
        open = false;
        if (pair) {
            switchField(1);
            acceptedPair(values[0], values[1]);
        } else
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
        if (pair && (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)) {
            Sound.tick();
            switchField(1 - typing);
            return;
        }
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
            finish();
        }
    }

    // One field, or the pair's two: the one being typed into carries the caret, the other its kept value, dimmer.
    component Field: Rectangle {
        id: box

        property int slot: 0
        readonly property bool active: !sheet.pair || sheet.typing === slot
        readonly property string shown: active ? sheet.text : sheet.values[slot]
        readonly property string name: sheet.pair ? sheet.labels[slot] : ""

        anchors.horizontalCenter: parent.horizontalCenter
        width: sheet.inner
        height: sheet.fieldHeight
        radius: Theme.dp(16)
        color: Theme.surface
        border.width: sheet.pair && active ? 1 : 0
        border.color: Theme.surfaceBorder

        Pointer {
            enabled: sheet.pair && !box.active
            radius: Theme.dp(16)
            onPicked: {
                Sound.tick();
                sheet.switchField(box.slot);
            }
        }

        CapsLabel {
            id: nameText
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(26)
            anchors.verticalCenter: parent.verticalCenter
            visible: box.name !== ""
            text: box.name.toUpperCase()
            size: Theme.dp(14)
            color: box.active ? Theme.textMuted : Theme.textFaint
        }

        Text {
            id: valueText
            anchors.left: parent.left
            anchors.leftMargin: Theme.dp(26) + (box.name !== "" ? nameText.width + Theme.dp(22) : 0)
            anchors.right: parent.right
            anchors.rightMargin: Theme.dp(26)
            anchors.verticalCenter: parent.verticalCenter
            text: box.shown
            color: box.active ? Theme.text : Theme.textMuted
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
            visible: sheet.open && box.active && caret.on
        }
    }

    Field {
        id: firstField

        anchors.top: sheet.head.bottom
        anchors.topMargin: Theme.dp(18)
        slot: 0
    }

    Field {
        id: secondField

        anchors.top: firstField.bottom
        anchors.topMargin: sheet.fieldGap
        visible: sheet.pair
        slot: 1
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

        anchors.top: sheet.pair ? secondField.bottom : firstField.bottom
        anchors.topMargin: Theme.dp(22)
        anchors.horizontalCenter: parent.horizontalCenter
        width: sheet.inner
        height: implicitHeight
        keyHeight: Theme.dp(52)
        keyGap: Theme.dp(9)
        showDone: true
        doneLabel: sheet.doneLabel.toLowerCase()
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
        onDone: sheet.finish()
    }
}
