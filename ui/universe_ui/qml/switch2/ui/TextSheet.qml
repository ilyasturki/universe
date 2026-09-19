import QtQuick
import "../core"
import "../sound"

Modal {
    id: sheet

    property string title: ""
    property string text: ""
    property int max: 64
    readonly property alias numeric: panel.numeric
    readonly property alias symbols: panel.symbols
    readonly property alias shift: panel.shift
    readonly property alias zone: panel.zone
    readonly property alias rowIndex: panel.rowIndex
    readonly property alias colIndex: panel.colIndex
    readonly property alias sideIndex: panel.sideIndex

    readonly property var hints: [
        { glyph: "Y", label: "Space" },
        { glyph: "X", label: "Cancel" },
        { glyph: "B", label: "Delete" },
        { glyph: "Start", label: "OK" },
        { glyph: "A", label: "Select" }
    ]

    carded: false
    scrimColor: Theme.ground
    scrimOpacity: 0.96
    onOpenChanged: if (open) panel.built = true

    function show(spec, done) {
        title = spec.title || "";
        text = spec.value === undefined || spec.value === null ? "" : String(spec.value);
        max = spec.max || 64;
        panel.numeric = spec.numeric === true;
        panel.symbols = spec.path === true;
        panel.reset();
        present(done);
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

    function press() { panel.press(); }
    function move(dr, dc) { panel.move(dr, dc); }

    Keys.onLeftPressed: panel.move(0, -1)
    Keys.onRightPressed: panel.move(0, 1)
    Keys.onUpPressed: panel.move(-1, 0)
    Keys.onDownPressed: panel.move(1, 0)

    Keys.onPressed: function(event) {
        event.accepted = true;
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
        } else if (api.keys.isMenu(event)) {
            Sound.play("ok");
            finish(text);
        }
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

        Label {
            id: valueText
            x: Theme.dp(8)
            y: Theme.dp(190)
            width: parent.width - Theme.dp(140)
            text: sheet.text
            elide: Text.ElideLeft
            font.pixelSize: Theme.dp(42)
        }

        Rectangle {
            x: valueText.x + Math.min(valueText.implicitWidth, valueText.width) + Theme.dp(4)
            y: valueText.y + Theme.dp(2)
            width: Theme.dp(3)
            height: Theme.dp(50)
            color: Theme.accent
            visible: caret.on
        }

        Rectangle {
            y: valueText.y + Theme.dp(62)
            width: parent.width
            height: Theme.dp(3)
            color: Theme.text
        }

        Label {
            anchors.right: parent.right
            y: valueText.y + Theme.dp(74)
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

        Behavior on y { Ease {} }

        // The panel types blindly; the sheet's own put() holds the line at `max`.
        onTyped: function(value) { sheet.put(value); }
        onBackspaced: sheet.backspace()
        onAccepted: sheet.finish(sheet.text)
        onEscapedUp: Sound.play("edge")
    }
}
