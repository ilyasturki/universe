import QtQuick
import "../core"
import "../sound"

// The console's text entry: the field as a search bar across the top, the keyboard under it.
Modal {
    id: sheet

    property string title: ""
    property string text: ""
    property int max: 64
    // Two fields (`labels` names them) over one keyboard: `text` is the one at `typing`, `values` keeps both.
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
    readonly property alias rowIndex: panel.rowIndex
    readonly property alias colIndex: panel.colIndex

    readonly property var hints: (secret ? [
            {
                glyph: "RB",
                label: revealed ? "Hide" : "Show"
            }
        ] : []).concat([
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
            label: !pair ? "Done" : typing === 0 ? "Next" : "Save"
        },
        {
            glyph: "A",
            label: "Select"
        }
    ])

    carded: false
    scrimColor: Theme.ground
    scrimOpacity: 0.93
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

    // { title, labels: [a, b], first, second }: Done on the first field moves to the second, on the second gives `done([first, second])`.
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

    component Field: Item {
        id: field

        property string name: ""
        property string value: ""
        property bool active: true

        width: parent ? parent.width : 0
        height: Theme.dp(106)

        Label {
            visible: field.name !== ""
            text: field.name
            color: field.active ? Theme.textSecondary : Theme.textMuted
            font.pixelSize: Theme.dp(Theme.fontTiny)
        }

        Rectangle {
            id: bar
            y: field.name !== "" ? Theme.dp(32) : 0
            width: parent.width
            height: Theme.dp(70)
            color: Qt.rgba(0, 0, 0, field.active ? 0.42 : 0.25)

            Glyph {
                id: lens
                x: Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(30)
                height: width
                visible: sheet.labels.length === 0
                kind: "search"
            }

            Label {
                id: shown
                x: lens.visible ? lens.x + lens.width + Theme.dp(16) : Theme.dp(22)
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - x - Theme.dp(140)
                text: field.value
                color: field.active ? Theme.text : Theme.textSecondary
                elide: Text.ElideLeft
                font.weight: Font.DemiBold
                font.pixelSize: Theme.dp(Theme.fontTitle)
            }

            Rectangle {
                x: shown.x + Math.min(shown.implicitWidth, shown.width) + Theme.dp(2)
                anchors.verticalCenter: parent.verticalCenter
                width: Theme.dp(2)
                height: Theme.dp(38)
                color: Theme.text
                visible: caret.on && field.active
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: Theme.dp(2)
                color: field.active ? Theme.text : Theme.hairline
            }
        }
    }

    Column {
        id: fields

        x: Theme.dp(Theme.edge)
        y: Theme.dp(58)
        width: parent.width - x - Theme.dp(Theme.columnRight)
        spacing: Theme.dp(16)

        Label {
            text: sheet.title
            visible: sheet.title !== ""
            color: Theme.textSecondary
            font.pixelSize: Theme.dp(Theme.fontSmall)
        }

        Field {
            name: sheet.pair ? sheet.labels[0] : ""
            value: sheet.pair ? (sheet.typing === 0 ? sheet.text : sheet.values[0]) : sheet.masked
            active: !sheet.pair || sheet.typing === 0
        }

        Field {
            visible: sheet.pair
            name: sheet.pair ? sheet.labels[1] : ""
            value: sheet.typing === 1 ? sheet.text : sheet.values[1]
            active: sheet.typing === 1
        }
    }

    Label {
        anchors.right: fields.right
        y: fields.y + fields.height + Theme.dp(12)
        text: sheet.text.length + "/" + sheet.max
        color: Theme.textMuted
        font.pixelSize: Theme.dp(Theme.fontTiny)
    }

    Label {
        id: revealButton
        objectName: "revealButton"
        anchors.left: fields.left
        y: fields.y + fields.height + Theme.dp(12)
        visible: sheet.secret
        text: sheet.revealed ? "Hide Password" : "Show Password"
        color: Theme.textSecondary
        font.pixelSize: Theme.dp(Theme.fontTiny)

        TapHandler {
            onTapped: sheet.reveal()
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

        x: fields.x
        y: fields.y + fields.height + Theme.dp(56) + (sheet.open ? 0 : Theme.dp(24))
        opacity: sheet.open ? 1.0 : 0.0

        Behavior on y {
            NumberAnimation {
                duration: Theme.durPage
                easing.type: Easing.OutCubic
            }
        }
        Behavior on opacity {
            NumberAnimation {
                duration: Theme.durChrome
            }
        }

        onTyped: function (value) {
            sheet.put(value);
        }
        onBackspaced: sheet.backspace()
        onAccepted: sheet.accept()
        onEscapedUp: Sound.play("edge")
    }
}
