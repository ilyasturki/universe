import QtQuick
import "../core"
import "../sound"

Modal {
    id: sheet

    property string title: ""
    property string text: ""
    property int max: 64
    // Two fields (`labels` names them) over one panel: `text` is the one at `typing`, `values` keeps both.
    property var labels: []
    property var values: ["", ""]
    property int typing: 0
    readonly property bool pair: labels.length === 2
    readonly property alias numeric: panel.numeric
    readonly property alias symbols: panel.symbols
    readonly property alias shift: panel.shift
    readonly property alias zone: panel.zone
    readonly property alias rowIndex: panel.rowIndex
    readonly property alias colIndex: panel.colIndex
    readonly property alias sideIndex: panel.sideIndex

    readonly property var hints: [
        {
            glyph: "Y",
            label: "Space"
        },
        {
            glyph: "X",
            label: "Cancel"
        },
        {
            glyph: "B",
            label: "Delete"
        },
        {
            glyph: "Start",
            label: !pair ? "OK" : typing === 0 ? "Next" : "Save"
        },
        {
            glyph: "A",
            label: "Select"
        }
    ]

    carded: false
    scrimColor: Theme.ground
    scrimOpacity: 0.96
    onOpenChanged: if (open)
        panel.built = true

    function show(spec, done) {
        labels = [];
        title = spec.title || "";
        text = spec.value === undefined || spec.value === null ? "" : String(spec.value);
        max = spec.max || 64;
        panel.numeric = spec.numeric === true;
        panel.symbols = spec.path === true;
        panel.reset();
        present(done);
    }

    // { title, labels: [a, b], first, second }: OK on the first field moves to the second, on the second gives `done([first, second])`.
    function showPair(spec, done) {
        labels = spec.labels;
        values = [String(spec.first || ""), String(spec.second || "")];
        typing = 0;
        title = spec.title || "";
        text = values[0];
        max = spec.max || 64;
        panel.numeric = false;
        panel.symbols = false;
        panel.reset();
        present(done);
    }

    function switchField(k) {
        var kept = values.slice();
        kept[typing] = text;
        values = kept;
        typing = k;
        text = values[k];
    }

    // The first of a pair names the entry: it cannot be left empty.
    function accept() {
        if (pair && typing === 0) {
            if (text === "")
                Sound.play("edge");
            else {
                Sound.play("ok");
                switchField(1);
            }
            return;
        }
        Sound.play("ok");
        if (pair) {
            switchField(1);
            finish([values[0], values[1]]);
        } else
            finish(text);
    }

    function cancel() {
        Sound.play("back");
        finish(null);
    }

    // The panel plays the key; put() only refuses past `max`.
    function put(ch) {
        if (text.length >= max) {
            Sound.play("edge");
            return;
        }
        text += ch;
    }

    function backspace() {
        text = text.slice(0, -1);
    }

    function press() {
        panel.press();
    }
    function move(dr, dc) {
        panel.move(dr, dc);
    }

    Keys.onLeftPressed: panel.move(0, -1)
    Keys.onRightPressed: panel.move(0, 1)
    Keys.onUpPressed: panel.move(-1, 0)
    Keys.onDownPressed: panel.move(1, 0)

    Keys.onPressed: function (event) {
        event.accepted = true;
        if (pair && (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab)) {
            Sound.play("type");
            switchField(1 - typing);
            return;
        }
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event))
            panel.press();
        else if (api.keys.isCancel(event)) {
            Sound.play("type");
            backspace();
        } else if (api.keys.isDetails(event))
            cancel();
        else if (api.keys.isFilters(event)) {
            Sound.play("type");
            put(" ");
        } else if (api.keys.isMenu(event))
            accept();
    }

    Item {
        id: field

        x: Theme.dp(124)
        y: Theme.dp(60)
        width: parent.width - x * 2
        height: Theme.dp(280)

        Label {
            y: Theme.dp(20)
            text: sheet.title
        }

        // The pair's second field takes the line the count sat on; the count moves under it.
        Label {
            id: firstName
            x: Theme.dp(8)
            y: Theme.dp(150)
            visible: sheet.pair
            text: sheet.pair ? sheet.labels[0] : ""
            color: sheet.typing === 0 ? Theme.textSecondary : Theme.textDisabled
            font.pixelSize: Theme.dp(Theme.fontTiny)
            font.letterSpacing: Theme.dp(2)
        }

        Label {
            id: valueText
            x: Theme.dp(8)
            y: sheet.pair ? Theme.dp(90) : Theme.dp(190)
            width: parent.width - Theme.dp(140)
            text: sheet.pair ? (sheet.typing === 0 ? sheet.text : sheet.values[0]) : sheet.text
            color: !sheet.pair || sheet.typing === 0 ? Theme.text : Theme.textSecondary
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(42)
        }

        Rectangle {
            x: valueText.x + Math.min(valueText.implicitWidth, valueText.width)
            y: valueText.y + Theme.dp(2)
            width: Theme.dp(3)
            height: Theme.dp(50)
            color: Theme.accent
            visible: caret.on && (!sheet.pair || sheet.typing === 0)
        }

        Rectangle {
            y: valueText.y + Theme.dp(62)
            width: parent.width
            height: Theme.dp(3)
            color: !sheet.pair || sheet.typing === 0 ? Theme.text : Theme.hairline
        }

        Label {
            id: secondName
            x: Theme.dp(8)
            y: Theme.dp(270)
            visible: sheet.pair
            text: sheet.pair ? sheet.labels[1] : ""
            color: sheet.typing === 1 ? Theme.textSecondary : Theme.textDisabled
            font.pixelSize: Theme.dp(Theme.fontTiny)
            font.letterSpacing: Theme.dp(2)
        }

        Label {
            id: secondText
            x: Theme.dp(8)
            y: Theme.dp(210)
            width: parent.width - Theme.dp(140)
            visible: sheet.pair
            text: sheet.typing === 1 ? sheet.text : sheet.values[1]
            color: sheet.typing === 1 ? Theme.text : Theme.textSecondary
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(42)
        }

        Rectangle {
            x: secondText.x + Math.min(secondText.implicitWidth, secondText.width)
            y: secondText.y + Theme.dp(2)
            width: Theme.dp(3)
            height: Theme.dp(50)
            color: Theme.accent
            visible: caret.on && sheet.pair && sheet.typing === 1
        }

        Rectangle {
            y: secondText.y + Theme.dp(62)
            width: parent.width
            height: Theme.dp(3)
            visible: sheet.pair
            color: sheet.typing === 1 ? Theme.text : Theme.hairline
        }

        Label {
            anchors.right: parent.right
            y: (sheet.pair ? secondText.y : valueText.y) + Theme.dp(74)
            text: sheet.text.length + "/" + sheet.max
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
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

    KeyPanel {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        y: sheet.open ? parent.height - height : parent.height

        Behavior on y {
            Ease {}
        }

        // The panel types blindly; the sheet's own put() holds the line at `max`.
        onTyped: function (value) {
            sheet.put(value);
        }
        onBackspaced: sheet.backspace()
        onAccepted: sheet.accept()
        onEscapedUp: Sound.play("edge")
    }
}
