import QtQuick
import "../core"
import "../sound"

// The Switch's keyboard panel: rows of keys with a side column (⌫, OK). Emits what it types; the owner keeps the text.
Rectangle {
    id: panel

    property bool numeric: false
    property bool symbols: false
    property bool shift: false
    property bool built: false
    property bool cursorShown: true
    property real scale: 1.0

    property string zone: "keys"
    property int rowIndex: 1
    property int colIndex: 0
    property int sideIndex: 1

    signal typed(string value)
    signal backspaced
    signal accepted
    // Up from the top row: the owner may move on to what sits above the panel.
    signal escapedUp

    readonly property real keyW: Theme.dp(128 * scale)
    readonly property real keyH: Theme.dp(72 * scale)
    readonly property real keyGap: Theme.dp(6)
    readonly property real keysLeft: Theme.dp(134)
    readonly property real keysTop: Theme.dp(46 * scale)
    readonly property real sideX: keysLeft + 11 * (keyW + keyGap) + Theme.dp(12)
    readonly property real sideW: Theme.dp(180)

    height: keysTop + keyH * 5 + keyGap * 4 + Theme.dp(110) * scale
    color: Theme.ground

    function reset() {
        zone = "keys";
        rowIndex = numeric ? 0 : 1;
        colIndex = 0;
        sideIndex = 1;
        shift = false;
    }

    function chars(s) {
        return s.split("").map(function (c) {
            return {
                value: c
            };
        });
    }

    // The physical keyboard's rows (api.keys.layout), eleven keys each; a key's `shift` is what ⇧ types on it.
    function keys(row) {
        return (api.keys.layout.rows[row] || []).slice(0, 11);
    }

    readonly property var rows: numeric ? [chars("123"), chars("456"), chars("789"), chars("-0.")] : [keys("AE"), keys("AD"), keys("AC"), keys("AB")]
    readonly property var bottomRow: numeric ? [] : [
        {
            label: "⇧",
            action: "shift"
        },
        {
            label: "ABC",
            action: "abc"
        },
        {
            label: "#+=",
            action: "symbols"
        },
        {
            label: "Space",
            action: "space",
            value: " "
        }
    ]
    readonly property var side: [
        {
            label: "⌫",
            action: "backspace"
        },
        {
            label: "OK",
            action: "ok"
        }
    ]
    readonly property var symbolRows: [chars("~`!@#$%^&*("), chars(")_+={}[]|\\;"), chars("\"<>/?,.-:'"), chars("¿¡€£¥•…—–")]
    readonly property var shownRows: symbols && !numeric ? symbolRows : rows

    function keyAt(r, c) {
        return r < shownRows.length ? shownRows[r][c] : bottomRow[c];
    }

    // What a key types: its shift level while ⇧ is on, for a key that has one.
    function valueOf(key) {
        return shift && key.shift !== undefined ? key.shift : key.value;
    }

    function rowLength(r) {
        return r < shownRows.length ? shownRows[r].length : bottomRow.length;
    }
    readonly property int rowCount: shownRows.length + (bottomRow.length > 0 ? 1 : 0)

    function put(ch) {
        Sound.play("type");
        panel.typed(ch);
        shift = false;
    }

    function press() {
        if (zone === "side") {
            if (side[sideIndex].action === "backspace") {
                Sound.play("type");
                panel.backspaced();
            } else {
                Sound.play("ok");
                panel.accepted();
            }
            return;
        }
        var key = keyAt(rowIndex, colIndex);
        if (!key)
            return;
        if (key.value !== undefined) {
            put(valueOf(key));
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
            if (dr < 0 && sideIndex === 0) {
                panel.escapedUp();
                return;
            }
            sideIndex = Sound.stepped(sideIndex, dr, side.length);
            return;
        }
        if (dr !== 0) {
            var nr = rowIndex + dr;
            if (nr < 0) {
                panel.escapedUp();
                return;
            }
            if (nr >= rowCount) {
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

    Column {
        x: panel.keysLeft
        y: panel.keysTop
        spacing: panel.keyGap

        Repeater {
            model: panel.built ? panel.rowCount : 0

            Row {
                readonly property int r: index
                spacing: panel.keyGap

                Repeater {
                    model: panel.rowLength(r)

                    Item {
                        id: key

                        readonly property var spec: panel.keyAt(r, index)
                        readonly property bool wide: spec.action === "space"
                        readonly property bool focused: panel.cursorShown && panel.zone === "keys" && panel.rowIndex === r && panel.colIndex === index
                        readonly property bool latched: (spec.action === "shift" && panel.shift) || (spec.action === "abc" && !panel.symbols && !panel.numeric) || (spec.action === "symbols" && panel.symbols)

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
                            text: key.spec.label !== undefined ? key.spec.label : panel.valueOf(key.spec)
                            color: key.latched ? Theme.accent : Theme.text
                            font.pixelSize: Theme.dp((key.spec.label !== undefined && key.spec.label.length > 1 ? 28 : 34) * panel.scale)
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
            model: panel.built ? panel.side : []

            Item {
                id: sideKey

                readonly property bool focused: panel.cursorShown && panel.zone === "side" && panel.sideIndex === index
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
                    font.pixelSize: Theme.dp((modelData.label.length > 1 ? 30 : 38) * panel.scale)
                }
            }
        }
    }
}
