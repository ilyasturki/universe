import QtQuick
import "../core"
import "../sound"

Modal {
    id: sheet

    property string title: ""
    property string text: ""
    property int max: 64
    property bool numeric: false
    property bool symbols: false
    property bool shift: false
    property bool built: false

    property string zone: "keys"
    property int rowIndex: 1
    property int colIndex: 0
    property int sideIndex: 1

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
    onOpenChanged: if (open) built = true

    function show(spec, done) {
        title = spec.title || "";
        text = spec.value === undefined || spec.value === null ? "" : String(spec.value);
        max = spec.max || 64;
        numeric = spec.numeric === true;
        symbols = spec.path === true;
        shift = false;
        zone = "keys";
        rowIndex = numeric ? 0 : 1;
        colIndex = 0;
        sideIndex = 1;
        present(done);
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
        Sound.play("type");
        text += shift ? ch.toUpperCase() : ch;
        shift = false;
    }

    function backspace() {
        Sound.play("type");
        text = text.slice(0, -1);
    }

    function chars(s) { return s.split(""); }

    readonly property var rows: numeric
        ? [ chars("123"), chars("456"), chars("789"), chars("-0.") ]
        : [ chars("1234567890-"), chars("qwertyuiop/"), chars("asdfghjkl:'"), chars("zxcvbnm,.?!") ]
    readonly property var bottomRow: numeric ? [] : [ { label: "⇧", action: "shift" }, { label: "ABC", action: "abc" }, { label: "#+=", action: "symbols" }, { label: "Space", action: "space", value: " " } ]
    readonly property var side: [ { label: "⌫", action: "backspace" }, { label: "OK", action: "ok" } ]
    readonly property var symbolRows: [ chars("~`!@#$%^&*("), chars(")_+={}[]|\\;"), chars("\"<>/?,.-:'"), chars("¿¡€£¥•…—–") ]
    readonly property var shownRows: symbols && !numeric ? symbolRows : rows

    function keyAt(r, c) {
        if (r < shownRows.length)
            return { label: shownRows[r][c], value: shownRows[r][c] };
        return bottomRow[c];
    }

    function rowLength(r) { return r < shownRows.length ? shownRows[r].length : bottomRow.length; }
    readonly property int rowCount: shownRows.length + (bottomRow.length > 0 ? 1 : 0)

    function press() {
        if (zone === "side") {
            if (side[sideIndex].action === "backspace") {
                backspace();
            } else {
                Sound.play("ok");
                finish(text);
            }
            return;
        }
        var key = keyAt(rowIndex, colIndex);
        if (!key)
            return;
        if (key.value !== undefined) {
            put(key.value);
            return;
        }
        Sound.play("type");
        if (key.action === "shift")
            shift = !shift;
        else if (key.action === "abc")
            symbols = false;
        else
            symbols = !symbols;
    }

    function move(dr, dc) {
        if (zone === "side") {
            if (dc < 0) {
                zone = "keys";
                colIndex = rowLength(rowIndex) - 1;
                Sound.play("tick");
                return;
            }
            sideIndex = Sound.stepped(sideIndex, dr, side.length);
            return;
        }
        if (dr !== 0) {
            var nr = rowIndex + dr;
            if (nr < 0 || nr >= rowCount) {
                Sound.play("edge");
                return;
            }
            var ratio = colIndex / Math.max(1, rowLength(rowIndex) - 1);
            rowIndex = nr;
            colIndex = Math.round(ratio * (rowLength(nr) - 1));
            Sound.play("tick");
            return;
        }
        var nc = colIndex + dc;
        if (nc >= rowLength(rowIndex)) {
            zone = "side";
            sideIndex = Math.min(side.length - 1, Math.round(rowIndex / Math.max(1, rowCount - 1) * (side.length - 1)));
            Sound.play("tick");
            return;
        }
        if (nc < 0) {
            Sound.play("edge");
            return;
        }
        colIndex = nc;
        Sound.play("tick");
    }

    Keys.onLeftPressed: move(0, -1)
    Keys.onRightPressed: move(0, 1)
    Keys.onUpPressed: move(-1, 0)
    Keys.onDownPressed: move(1, 0)

    Keys.onPressed: function(event) {
        event.accepted = true;
        if (event.isAutoRepeat)
            return;
        if (api.keys.isAccept(event))
            press();
        else if (api.keys.isCancel(event))
            backspace();
        else if (api.keys.isDetails(event))
            cancel();
        else if (api.keys.isFilters(event))
            put(" ");
        else if (api.keys.isMenu(event)) {
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

    Rectangle {
        id: panel

        anchors.left: parent.left
        anchors.right: parent.right
        height: Theme.dp(540)
        y: sheet.open ? parent.height - height : parent.height
        color: Theme.ground

        Behavior on y { Ease {} }

        readonly property real keyW: Theme.dp(128)
        readonly property real keyH: Theme.dp(72)
        readonly property real keyGap: Theme.dp(6)
        readonly property real keysLeft: Theme.dp(134)
        readonly property real keysTop: Theme.dp(46)
        readonly property real sideX: keysLeft + 11 * (keyW + keyGap) + Theme.dp(12)
        readonly property real sideW: Theme.dp(180)

        Column {
            x: panel.keysLeft
            y: panel.keysTop
            spacing: panel.keyGap

            Repeater {
                model: sheet.built ? sheet.rowCount : 0

                Row {
                    readonly property int r: index
                    spacing: panel.keyGap

                    Repeater {
                        model: sheet.rowLength(r)

                        Item {
                            id: key

                            readonly property var spec: sheet.keyAt(r, index)
                            readonly property bool wide: spec.action === "space"
                            readonly property bool focused: sheet.zone === "keys" && sheet.rowIndex === r && sheet.colIndex === index
                            readonly property bool latched: (spec.action === "shift" && sheet.shift) || (spec.action === "abc" && !sheet.symbols && !sheet.numeric) || (spec.action === "symbols" && sheet.symbols)

                            width: wide ? panel.keyW * 5 + panel.keyGap * 4 : panel.keyW
                            height: panel.keyH

                            FocusPill {
                                anchors.fill: parent
                                radius: Theme.dp(4)
                                color: key.focused ? Theme.focusFill : Theme.card
                                visible: true
                                focused: key.focused
                            }

                            Label {
                                anchors.centerIn: parent
                                text: key.spec.label !== undefined ? key.spec.label : (sheet.shift && key.spec.value.length === 1 ? key.spec.value.toUpperCase() : key.spec.value)
                                color: key.latched ? Theme.accent : Theme.text
                                font.pixelSize: Theme.dp(key.spec.label !== undefined && key.spec.label.length > 1 ? 28 : 34)
                            }

                            Rectangle {
                                visible: key.latched
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: Theme.dp(8)
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: Theme.dp(52)
                                height: Theme.dp(3)
                                color: Theme.accent
                            }
                        }
                    }
                }
            }
        }

        Column {
            x: panel.sideX
            y: panel.keysTop
            spacing: panel.keyGap

            Repeater {
                model: sheet.built ? sheet.side : []

                Item {
                    id: sideKey

                    readonly property bool focused: sheet.zone === "side" && sheet.sideIndex === index
                    readonly property bool ok: modelData.action === "ok"

                    width: panel.sideW
                    height: ok ? panel.keyH * 2 + panel.keyGap : panel.keyH

                    FocusPill {
                        anchors.fill: parent
                        radius: Theme.dp(4)
                        color: sideKey.ok ? Theme.barBlue : sideKey.focused ? Theme.focusFill : Theme.card
                        visible: true
                        focused: sideKey.focused
                    }

                    Label {
                        anchors.centerIn: parent
                        text: modelData.label
                        color: sideKey.ok ? Theme.accentInk : Theme.text
                        font.pixelSize: Theme.dp(modelData.label.length > 1 ? 30 : 38)
                    }
                }
            }
        }
    }
}
