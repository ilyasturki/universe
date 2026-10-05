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
    // A password (`spec.secret`): dots on the TV until R shows it.
    property bool secret: false
    property bool revealed: false
    readonly property string masked: secret && !revealed ? "•".repeat(text.length) : text
    readonly property alias numeric: panel.numeric
    readonly property alias symbols: panel.symbols
    readonly property alias shift: panel.shift
    readonly property alias zone: panel.zone
    readonly property alias rowIndex: panel.rowIndex
    readonly property alias colIndex: panel.colIndex
    readonly property alias sideIndex: panel.sideIndex

    // Y, B and + wear their badges on Space, ⌫ and Done.
    readonly property var hints: [
        {
            glyph: "X",
            label: "Cancel"
        },
        {
            glyph: "A",
            label: "Select"
        }
    ]
    readonly property color ink: "#ffffff"
    readonly property color inkIdle: "#9a9a9c"

    carded: false
    scrimColor: "#3e3e40"
    scrimOpacity: 0.97
    onOpenChanged: if (open)
        panel.built = true

    function show(spec, done) {
        labels = [];
        title = spec.title || "";
        text = spec.value === undefined || spec.value === null ? "" : String(spec.value);
        max = spec.max || 64;
        secret = spec.secret === true;
        revealed = false;
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
        secret = false;
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

    function reveal() {
        Sound.play("type");
        revealed = !revealed;
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
        else if (api.keys.isNextPage(event) && secret)
            reveal();
    }

    Item {
        id: field

        x: Theme.dp(124)
        y: Theme.dp(60)
        width: parent.width - x * 2
        height: Theme.dp(280)

        Label {
            y: Theme.dp(10)
            text: sheet.title
            color: sheet.ink
            font.pixelSize: Theme.dp(40)
        }

        // The pair's second field takes the line the count sat on; the count moves under it.
        Label {
            id: firstName
            x: Theme.dp(8)
            y: Theme.dp(150)
            visible: sheet.pair
            text: sheet.pair ? sheet.labels[0] : ""
            color: sheet.typing === 0 ? sheet.ink : sheet.inkIdle
            font.pixelSize: Theme.dp(Theme.fontTiny)
            font.letterSpacing: Theme.dp(2)
        }

        Label {
            id: valueText
            x: Theme.dp(8)
            y: sheet.pair ? Theme.dp(90) : Theme.dp(185)
            width: parent.width - Theme.dp(140)
            text: sheet.pair ? (sheet.typing === 0 ? sheet.text : sheet.values[0]) : sheet.masked
            color: !sheet.pair || sheet.typing === 0 ? sheet.ink : sheet.inkIdle
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(48)
            font.letterSpacing: Theme.dp(1)
        }

        Rectangle {
            x: valueText.x + Math.min(valueText.implicitWidth, valueText.width)
            y: valueText.y + Theme.dp(4)
            width: Theme.dp(4)
            height: Theme.dp(54)
            color: Theme.ringDeep
            visible: caret.on && (!sheet.pair || sheet.typing === 0)
        }

        Rectangle {
            y: valueText.y + Theme.dp(68)
            width: parent.width
            height: Theme.dp(3)
            color: !sheet.pair || sheet.typing === 0 ? sheet.ink : sheet.inkIdle
        }

        Label {
            id: secondName
            x: Theme.dp(8)
            y: Theme.dp(270)
            visible: sheet.pair
            text: sheet.pair ? sheet.labels[1] : ""
            color: sheet.typing === 1 ? sheet.ink : sheet.inkIdle
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
            color: sheet.typing === 1 ? sheet.ink : sheet.inkIdle
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(48)
            font.letterSpacing: Theme.dp(1)
        }

        Rectangle {
            x: secondText.x + Math.min(secondText.implicitWidth, secondText.width)
            y: secondText.y + Theme.dp(4)
            width: Theme.dp(4)
            height: Theme.dp(54)
            color: Theme.ringDeep
            visible: caret.on && sheet.pair && sheet.typing === 1
        }

        Rectangle {
            y: secondText.y + Theme.dp(68)
            width: parent.width
            height: Theme.dp(3)
            visible: sheet.pair
            color: sheet.typing === 1 ? sheet.ink : sheet.inkIdle
        }

        Label {
            anchors.right: parent.right
            y: (sheet.pair ? secondText.y : valueText.y) + Theme.dp(80)
            text: sheet.text.length + "/" + sheet.max
            color: sheet.ink
            font.pixelSize: Theme.dp(30)
        }

        Row {
            id: revealButton
            objectName: "revealButton"
            anchors.right: parent.right
            y: valueText.y + Theme.dp(10)
            visible: sheet.secret
            spacing: Theme.dp(10)

            HintGlyph {
                anchors.verticalCenter: parent.verticalCenter
                glyph: "RB"
            }

            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: sheet.revealed ? "Hide" : "Show"
                color: sheet.ink
                font.pixelSize: Theme.dp(30)
            }

            TapHandler {
                onTapped: sheet.reveal()
            }
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
        doneLabel: sheet.pair && sheet.typing === 0 ? "Next" : "Done"

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
